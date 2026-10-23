use chrono::Local;
use reqwest::blocking::Client;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use std::io::Read;
use std::io::Write;
use std::net::IpAddr;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use tauri::{Emitter, Manager};

const MAX_SEARCH_QUERY_LEN: usize = 2000;
const ENCRYPTED_SECRET_PREFIX: &str = "dpapi:";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Case {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub deadline: String,
    pub status: String,
    pub preliminary_opinion: String,
    pub final_opinion: String,
    pub report_status: String,
    pub finalizable: String,
    pub open_questions: String,
    pub closed_points: String,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AuditLog {
    pub id: i64,
    pub case_id: i64,
    pub timestamp: String,
    pub action_type: String,
    pub query: String,
    pub details: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Source {
    pub id: i64,
    pub case_id: i64,
    pub search_run_id: Option<i64>,
    pub title: String,
    pub url: String,
    pub original_url: String,
    pub archived_url: String,
    pub snapshot_timestamp: String,
    pub archive_kind: String,
    pub tier: i32,
    pub content: String,
    pub file_path: String,
    pub hash: String,
    pub timestamp: String,
    pub source_status: String,
    pub priority_level: i32,
    pub usability_decision: String,
    pub proof_summary: String,
    pub reviewer_name: String,
    pub reviewed_at: String,
    pub relevance_note: String,
    pub probative_value: String,
    pub missing_points: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub original_url: String,
    pub archived_url: String,
    pub archive_kind: String,
    pub snapshot_timestamp: String,
    pub snippet: String,
    pub tier: i32,
    pub source_type: String,
    pub relevance: i32,
    pub query_used: String,
    pub action_label: String,
    /// Provenance-szint: RETRIEVED (valódi kereső-API), CONSTRUCTED (épített
    /// kereső/hivatalos URL), SUGGESTED (AI-javaslat), CAPTURED (rögzített).
    #[serde(default)]
    pub evidence_level: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SearchRun {
    pub id: i64,
    pub case_id: i64,
    pub created_at: String,
    pub summary: String,
    pub base_query: String,
    pub engine: String,
    pub tier_filter: i32,
    pub plan_count: i32,
    pub result_count: i32,
    pub note: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppSettings {
    pub search_mode: String,
    pub ai_provider: String,
    pub api_key: String,
    pub ai_model: String,
    pub system_prompt: String,
    #[serde(default)]
    pub api_key_set: bool,
    #[serde(default)]
    pub google_cse_key: String,
    #[serde(default)]
    pub google_cse_cx: String,
    #[serde(default)]
    pub google_cse_key_set: bool,
    #[serde(default)]
    pub brave_key: String,
    #[serde(default)]
    pub brave_key_set: bool,
    #[serde(default)]
    pub tavily_key: String,
    #[serde(default)]
    pub tavily_key_set: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SearchRunOutput {
    pub results: Vec<SearchResult>,
    pub ai_error: Option<String>,
    #[serde(default)]
    pub cse_error: Option<String>,
    #[serde(default)]
    pub brave_error: Option<String>,
    #[serde(default)]
    pub tavily_error: Option<String>,
    pub generated_count: i32,
    pub filtered_out_count: i32,
}

#[derive(Deserialize, Debug)]
struct CseItem {
    title: Option<String>,
    link: Option<String>,
    snippet: Option<String>,
}

#[derive(Deserialize, Debug)]
struct CseResponse {
    items: Option<Vec<CseItem>>,
}

#[derive(Deserialize, Debug)]
struct BraveItem {
    title: Option<String>,
    url: Option<String>,
    description: Option<String>,
}

#[derive(Deserialize, Debug)]
struct BraveWeb {
    results: Option<Vec<BraveItem>>,
}

#[derive(Deserialize, Debug)]
struct BraveResponse {
    web: Option<BraveWeb>,
}

#[derive(Deserialize, Debug)]
struct TavilyItem {
    title: Option<String>,
    url: Option<String>,
    content: Option<String>,
}

#[derive(Deserialize, Debug)]
struct TavilyResponse {
    results: Option<Vec<TavilyItem>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FetchedPage {
    pub title: String,
    pub text: String,
    pub status: u16,
    pub final_url: String,
}

#[derive(Deserialize, Debug)]
struct AiSearchResponse {
    search_brief: Option<String>,
    suggestions: Option<Vec<AiSuggestion>>,
}

#[derive(Deserialize, Debug)]
struct AiSuggestion {
    title: Option<String>,
    snippet: Option<String>,
    tier: Option<i32>,
    relevance: Option<i32>,
    query_used: Option<String>,
    // A modell 'url'/'source_type'/'action_label' mezoit szandekosan nem hasznaljuk
    // (a serde ignoralja): az AI-javaslat mindig keresesse alakul (FIX-AI-LINKS).
}

pub struct DbState {
    pub db_path: String,
}

fn get_connection(db_path: &str) -> Result<Connection, String> {
    let conn = Connection::open(db_path).map_err(|e| format!("Adatbázis megnyitási hiba: {e}"))?;
    let _ = conn.pragma_update(None, "foreign_keys", "ON");
    let _ = conn.busy_timeout(std::time::Duration::from_millis(5000));
    Ok(conn)
}

fn init_schema(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS cases (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            description TEXT,
            deadline TEXT,
            status TEXT,
            preliminary_opinion TEXT NOT NULL DEFAULT '',
            final_opinion TEXT NOT NULL DEFAULT '',
            report_status TEXT NOT NULL DEFAULT 'tervezet',
            finalizable TEXT NOT NULL DEFAULT 'nem',
            open_questions TEXT NOT NULL DEFAULT '',
            closed_points TEXT NOT NULL DEFAULT '',
            created_at TEXT
        )",
        [],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS audit_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            case_id INTEGER,
            timestamp TEXT,
            action_type TEXT,
            query TEXT,
            details TEXT
        )",
        [],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS sources (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            case_id INTEGER,
            search_run_id INTEGER,
            title TEXT,
            url TEXT,
            original_url TEXT NOT NULL DEFAULT '',
            archived_url TEXT NOT NULL DEFAULT '',
            snapshot_timestamp TEXT NOT NULL DEFAULT '',
            archive_kind TEXT NOT NULL DEFAULT 'live',
            tier INTEGER,
            content TEXT,
            file_path TEXT,
            hash TEXT,
            timestamp TEXT,
            source_status TEXT NOT NULL DEFAULT 'nyers',
            priority_level INTEGER NOT NULL DEFAULT 2,
            usability_decision TEXT NOT NULL DEFAULT 'hatteranyag',
            proof_summary TEXT NOT NULL DEFAULT '',
            reviewer_name TEXT NOT NULL DEFAULT '',
            reviewed_at TEXT NOT NULL DEFAULT '',
            relevance_note TEXT NOT NULL DEFAULT '',
            probative_value TEXT NOT NULL DEFAULT '',
            missing_points TEXT NOT NULL DEFAULT ''
        )",
        [],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS search_runs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            case_id INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            summary TEXT NOT NULL,
            base_query TEXT NOT NULL,
            engine TEXT NOT NULL,
            tier_filter INTEGER NOT NULL,
            plan_count INTEGER NOT NULL,
            result_count INTEGER NOT NULL,
            note TEXT NOT NULL DEFAULT ''
        )",
        [],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS app_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
        [],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_sources_case_id ON sources(case_id)",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_audit_logs_case_id ON audit_logs(case_id)",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_search_runs_case_id ON search_runs(case_id)",
        [],
    )
    .map_err(|e| e.to_string())?;

    migrate_schema(conn)?;

    Ok(())
}

const SCHEMA_VERSION: i64 = 1;

/// true, ha a `table` tartalmazza a `column` oszlopot. A `table`/`column`
/// kizárólag kódban rögzített literál lehet (nincs felhasználói input) → nincs SQLi.
fn table_has_column(conn: &Connection, table: &str, column: &str) -> Result<bool, String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|e| e.to_string())?;
    let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        let name: String = row.get(1).map_err(|e| e.to_string())?;
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Hozzáadja az oszlopot, ha hiányzik. A valódi hibák propagálnak (nincs elnyelés).
fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column: &str,
    column_def: &str,
) -> Result<(), String> {
    if table_has_column(conn, table, column)? {
        return Ok(());
    }
    conn.execute(
        &format!("ALTER TABLE {table} ADD COLUMN {column} {column_def}"),
        [],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Verziózott, determinisztikus séma-migráció. A `PRAGMA user_version` kapun áll:
/// ha a DB már a célverzión van, nem végez séma-műveletet.
fn migrate_schema(conn: &Connection) -> Result<(), String> {
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| e.to_string())?;
    if version >= SCHEMA_VERSION {
        return Ok(());
    }

    for (col, def) in [
        ("preliminary_opinion", "TEXT NOT NULL DEFAULT ''"),
        ("final_opinion", "TEXT NOT NULL DEFAULT ''"),
        ("report_status", "TEXT NOT NULL DEFAULT 'tervezet'"),
        ("finalizable", "TEXT NOT NULL DEFAULT 'nem'"),
        ("open_questions", "TEXT NOT NULL DEFAULT ''"),
        ("closed_points", "TEXT NOT NULL DEFAULT ''"),
    ] {
        add_column_if_missing(conn, "cases", col, def)?;
    }

    add_column_if_missing(conn, "sources", "search_run_id", "INTEGER")?;
    for (col, def) in [
        ("original_url", "TEXT NOT NULL DEFAULT ''"),
        ("archived_url", "TEXT NOT NULL DEFAULT ''"),
        ("snapshot_timestamp", "TEXT NOT NULL DEFAULT ''"),
        ("archive_kind", "TEXT NOT NULL DEFAULT 'live'"),
        ("source_status", "TEXT NOT NULL DEFAULT 'nyers'"),
        ("priority_level", "INTEGER NOT NULL DEFAULT 2"),
        ("usability_decision", "TEXT NOT NULL DEFAULT 'hatteranyag'"),
        ("proof_summary", "TEXT NOT NULL DEFAULT ''"),
        ("reviewer_name", "TEXT NOT NULL DEFAULT ''"),
        ("reviewed_at", "TEXT NOT NULL DEFAULT ''"),
        ("relevance_note", "TEXT NOT NULL DEFAULT ''"),
        ("probative_value", "TEXT NOT NULL DEFAULT ''"),
        ("missing_points", "TEXT NOT NULL DEFAULT ''"),
    ] {
        add_column_if_missing(conn, "sources", col, def)?;
    }

    add_column_if_missing(conn, "search_runs", "note", "TEXT NOT NULL DEFAULT ''")?;

    conn.pragma_update(None, "user_version", SCHEMA_VERSION)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Maszkolja a mentett API-kulcsokat egy hiba-/naplószövegben, mielőtt az a
/// felhasználóhoz vagy az audit_logs-ba kerül (SEC-1). A rövid/üres kulcsot sose
/// cseréli, hogy ne maszkoljon el ártatlan szövegrészt.
fn redact_secrets(input: &str, settings: &AppSettings) -> String {
    let mut out = input.to_string();
    for secret in [
        &settings.api_key,
        &settings.google_cse_key,
        &settings.brave_key,
        &settings.tavily_key,
    ] {
        let s = secret.trim();
        if s.len() >= 6 {
            out = out.replace(s, "***");
        }
    }
    out
}

fn default_app_settings() -> AppSettings {
    AppSettings {
        search_mode: "native".to_string(),
        ai_provider: "openai".to_string(),
        api_key: String::new(),
        ai_model: "gpt-4o-mini".to_string(),
        system_prompt: "Te egy igazsagugyi szakerto munkajat tamogato kutatasi asszisztens vagy. Adj rovid, gyakorlatias keresesi javaslatokat, ellenorzesi szempontokat, keresokifejezes-variantokat es lehetoseg szerint magyar vagy relevans nemzetkozi forrashelyeket. JSON formatumban valaszolj.".to_string(),
        api_key_set: false,
        google_cse_key: String::new(),
        google_cse_cx: String::new(),
        google_cse_key_set: false,
        brave_key: String::new(),
        brave_key_set: false,
        tavily_key: String::new(),
        tavily_key_set: false,
    }
}

/// Megosztott, 30 s-os HTTP-kliens a keresési providerekhez (PERF-1).
fn search_client() -> Result<&'static Client, String> {
    static CELL: OnceLock<Client> = OnceLock::new();
    if let Some(c) = CELL.get() {
        return Ok(c);
    }
    let c = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("HTTP kliens inicializálási hiba: {e}"))?;
    let _ = CELL.set(c);
    Ok(CELL.get().expect("just set"))
}

/// Megosztott, 45 s-os HTTP-kliens az AI-hívásokhoz (PERF-1).
fn ai_client() -> Result<&'static Client, String> {
    static CELL: OnceLock<Client> = OnceLock::new();
    if let Some(c) = CELL.get() {
        return Ok(c);
    }
    let c = Client::builder()
        .timeout(std::time::Duration::from_secs(45))
        .build()
        .map_err(|e| format!("HTTP kliens inicializálási hiba: {e}"))?;
    let _ = CELL.set(c);
    Ok(CELL.get().expect("just set"))
}

fn archive_dir_for(db_path: &str) -> PathBuf {
    Path::new(db_path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("archive")
}

/// true, ha a `candidate` az archive mappán belül van (kanonizált út-összevetés, SEC-3).
fn is_within_archive(db_path: &str, candidate: &str) -> bool {
    let base = match archive_dir_for(db_path).canonicalize() {
        Ok(b) => b,
        Err(_) => return false,
    };
    match Path::new(candidate).canonicalize() {
        Ok(c) => c.starts_with(&base),
        Err(_) => false,
    }
}

/// A PowerShell spawn-hibáját érthető üzenetre képezi (DPAPI Windows-kötöttség, PLAT-1).
fn map_powershell_spawn_error(kind: std::io::ErrorKind, raw: &str) -> String {
    if kind == std::io::ErrorKind::NotFound {
        "A titkos kulcsok titkosítása Windows PowerShellt (DPAPI) igényel, ami ezen a rendszeren nem található. A kulcs nem menthető biztonságosan.".to_string()
    } else {
        format!("PowerShell futtatási hiba: {raw}")
    }
}

fn run_powershell_with_stdin(script: &str, stdin_payload: &str) -> Result<String, String> {
    let mut child = Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| map_powershell_spawn_error(e.kind(), &e.to_string()))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(stdin_payload.as_bytes())
            .map_err(|e| format!("PowerShell stdin irasi hiba: {e}"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("PowerShell varakozasi hiba: {e}"))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn encrypt_secret_for_storage(secret: &str) -> Result<String, String> {
    if secret.trim().is_empty() {
        return Ok(String::new());
    }

    if secret.starts_with(ENCRYPTED_SECRET_PREFIX) {
        return Ok(secret.to_string());
    }

    let script = r#"
$plain = [Console]::In.ReadToEnd()
$bytes = [System.Text.Encoding]::UTF8.GetBytes($plain)
$protected = [System.Security.Cryptography.ProtectedData]::Protect($bytes, $null, [System.Security.Cryptography.DataProtectionScope]::CurrentUser)
[Convert]::ToBase64String($protected)
"#;

    let encrypted = run_powershell_with_stdin(script, secret)?;
    Ok(format!("{ENCRYPTED_SECRET_PREFIX}{encrypted}"))
}

fn decrypt_secret_from_storage(secret: &str) -> Result<String, String> {
    if secret.trim().is_empty() {
        return Ok(String::new());
    }

    if !secret.starts_with(ENCRYPTED_SECRET_PREFIX) {
        return Ok(secret.to_string());
    }

    let encrypted = &secret[ENCRYPTED_SECRET_PREFIX.len()..];
    let script = r#"
$cipher = [Console]::In.ReadToEnd().Trim()
$bytes = [Convert]::FromBase64String($cipher)
$plain = [System.Security.Cryptography.ProtectedData]::Unprotect($bytes, $null, [System.Security.Cryptography.DataProtectionScope]::CurrentUser)
[System.Text.Encoding]::UTF8.GetString($plain)
"#;

    run_powershell_with_stdin(script, encrypted)
}

fn load_app_settings(db_path: &str) -> Result<AppSettings, String> {
    let conn = get_connection(db_path)?;
    let mut stmt = conn
        .prepare("SELECT key, value FROM app_settings")
        .map_err(|e| e.to_string())?;

    let iter = stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?;

    let mut settings = default_app_settings();

    for item in iter {
        let (key, value) = item.map_err(|e| e.to_string())?;
        match key.as_str() {
            "search_mode" => settings.search_mode = value,
            "ai_provider" => settings.ai_provider = value,
            "api_key" => settings.api_key = decrypt_secret_from_storage(&value)?,
            "ai_model" => settings.ai_model = value,
            "system_prompt" => settings.system_prompt = value,
            "google_cse_key" => settings.google_cse_key = decrypt_secret_from_storage(&value)?,
            "google_cse_cx" => settings.google_cse_cx = value,
            "brave_key" => settings.brave_key = decrypt_secret_from_storage(&value)?,
            "tavily_key" => settings.tavily_key = decrypt_secret_from_storage(&value)?,
            _ => {}
        }
    }

    settings.api_key_set = !settings.api_key.trim().is_empty();
    settings.google_cse_key_set = !settings.google_cse_key.trim().is_empty();
    settings.brave_key_set = !settings.brave_key.trim().is_empty();
    settings.tavily_key_set = !settings.tavily_key.trim().is_empty();
    Ok(settings)
}

fn load_raw_setting(db_path: &str, key: &str) -> Result<String, String> {
    let conn = get_connection(db_path)?;
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        params![key],
        |row| row.get::<_, String>(0),
    )
    .or_else(|error| match error {
        rusqlite::Error::QueryReturnedNoRows => Ok(String::new()),
        other => Err(other.to_string()),
    })
}

fn save_app_settings_internal(db_path: &str, settings: &AppSettings) -> Result<(), String> {
    let conn = get_connection(db_path)?;
    let encrypted_api_key = encrypt_secret_for_storage(&settings.api_key)?;
    let encrypted_cse_key = encrypt_secret_for_storage(&settings.google_cse_key)?;
    let encrypted_brave_key = encrypt_secret_for_storage(&settings.brave_key)?;
    let encrypted_tavily_key = encrypt_secret_for_storage(&settings.tavily_key)?;
    let entries = [
        ("search_mode", settings.search_mode.as_str()),
        ("ai_provider", settings.ai_provider.as_str()),
        ("api_key", encrypted_api_key.as_str()),
        ("ai_model", settings.ai_model.as_str()),
        ("system_prompt", settings.system_prompt.as_str()),
        ("google_cse_key", encrypted_cse_key.as_str()),
        ("google_cse_cx", settings.google_cse_cx.as_str()),
        ("brave_key", encrypted_brave_key.as_str()),
        ("tavily_key", encrypted_tavily_key.as_str()),
    ];

    for (key, value) in entries {
        conn.execute(
            "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn log_action(
    db_path: &str,
    case_id: i64,
    action_type: &str,
    query: &str,
    details: &str,
) -> Result<(), String> {
    let conn = get_connection(db_path)?;
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    conn.execute(
        "INSERT INTO audit_logs (case_id, timestamp, action_type, query, details) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![case_id, timestamp, action_type, query, details],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

fn url_encode(input: &str) -> String {
    let mut encoded = String::new();

    for byte in input.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(*byte as char)
            }
            b' ' => encoded.push('+'),
            _ => encoded.push_str(&format!("%{:02X}", byte)),
        }
    }

    encoded
}

fn push_result(
    results: &mut Vec<SearchResult>,
    tier_filter: i32,
    title: &str,
    url: String,
    original_url: String,
    archived_url: String,
    archive_kind: String,
    snapshot_timestamp: String,
    snippet: String,
    tier: i32,
    source_type: &str,
    relevance: i32,
    query_used: String,
    action_label: &str,
) {
    if tier_filter == 0 || tier_filter == tier {
        results.push(SearchResult {
            title: title.to_string(),
            url,
            original_url,
            archived_url,
            archive_kind,
            snapshot_timestamp,
            snippet,
            tier,
            source_type: source_type.to_string(),
            relevance,
            query_used,
            action_label: action_label.to_string(),
            evidence_level: String::new(),
        });
    }
}

/// A `from` indextől a végéig beállítja az evidence-szintet a most hozzáadott
/// találatokra (KRISTA-alapelv: a provenance elsőrangú, találat-fajtánként).
fn tag_evidence(results: &mut [SearchResult], from: usize, level: &str) {
    for result in results.iter_mut().skip(from) {
        result.evidence_level = level.to_string();
    }
}

fn normalize_for_match(input: &str) -> String {
    input
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' => 'a',
            'é' => 'e',
            'í' => 'i',
            'ó' | 'ö' | 'ő' => 'o',
            'ú' | 'ü' | 'ű' => 'u',
            other => other,
        })
        .collect()
}

fn build_query_variants(query: &str) -> Vec<(&'static str, String)> {
    let mut variants = vec![("alap", query.to_string())];
    let normalized = normalize_for_match(query);

    if normalized.contains("oradij")
        || normalized.contains("oraber")
        || normalized.contains("ber")
        || normalized.contains("dij")
    {
        variants.push((
            "piaci",
            format!("{query} árlista OR bérkalkuláció OR díjtáblázat"),
        ));
    }

    if normalized.contains("mezogazdas")
        || normalized.contains("gep")
        || normalized.contains("munka")
        || normalized.contains("koltseg")
    {
        variants.push((
            "szakmai",
            format!("{query} költség OR üzemeltetés OR normatíva OR óraköltség"),
        ));
    }

    if normalized.contains("szakert")
        || normalized.contains("igazsagugy")
        || normalized.contains("jog")
        || normalized.contains("kar")
    {
        variants.push((
            "jogi",
            format!("{query} jogszabály OR módszertan OR szakmai iránymutatás"),
        ));
    }

    variants
}

fn build_search_url(engine: &str, query: &str) -> String {
    let encoded = url_encode(query);
    match engine {
        "bing" => format!("https://www.bing.com/search?q={encoded}"),
        "duckduckgo" => format!("https://duckduckgo.com/?q={encoded}"),
        _ => format!("https://www.google.com/search?q={encoded}"),
    }
}

fn build_internet_archive_url(query: &str) -> String {
    let encoded = url_encode(query);
    format!("https://archive.org/search?query={encoded}")
}

fn normalize_domain(value: &str) -> String {
    value
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("www.")
        .trim_end_matches('/')
        .to_string()
}

fn extract_url_targets(query: &str) -> Vec<String> {
    let mut targets = Vec::new();

    for token in query.split_whitespace() {
        let cleaned = token.trim_matches(|char| {
            char == ')' || char == '(' || char == '"' || char == '\'' || char == ','
        });

        if (cleaned.starts_with("https://") || cleaned.starts_with("http://"))
            && !targets.contains(&cleaned.to_string())
        {
            targets.push(cleaned.to_string());
        }
    }

    targets
}

fn extract_site_targets(query: &str) -> Vec<String> {
    let mut targets = Vec::new();

    for token in query.split_whitespace() {
        let trimmed_token =
            token.trim_matches(|char| char == ')' || char == '(' || char == '"' || char == '\'');
        if let Some(site) = trimmed_token.strip_prefix("site:") {
            let cleaned = normalize_domain(site);
            if !cleaned.is_empty() && !targets.contains(&cleaned) {
                targets.push(cleaned);
            }
        }
    }

    targets
}

fn strip_query_token(token: &str) -> &str {
    token.trim_matches(|char| char == ')' || char == '(' || char == '"' || char == '\'')
}

fn split_query_for_domain_search(query: &str) -> (String, Vec<String>) {
    let strip = strip_query_token;

    let tokens: Vec<&str> = query.split_whitespace().collect();
    let mut keep: Vec<bool> = tokens
        .iter()
        .map(|token| !strip(token).starts_with("site:"))
        .collect();

    for index in 0..tokens.len() {
        if keep[index] && strip(tokens[index]).eq_ignore_ascii_case("or") {
            let prev_dropped = index > 0 && !keep[index - 1];
            let next_dropped = index + 1 < tokens.len() && !keep[index + 1];
            if prev_dropped || next_dropped {
                keep[index] = false;
            }
        }
    }

    let cleaned = tokens
        .iter()
        .zip(keep.iter())
        .filter(|(_, kept)| **kept)
        .map(|(token, _)| *token)
        .collect::<Vec<_>>()
        .join(" ");

    let mut domains = Vec::new();
    for target in extract_site_targets(query) {
        let host = target.split('/').next().unwrap_or(&target).to_string();
        if !host.is_empty() && !domains.contains(&host) {
            domains.push(host);
        }
    }

    let cleaned = cleaned.trim().to_string();
    let effective_query = if cleaned.is_empty() {
        query.trim().to_string()
    } else {
        cleaned
    };

    (effective_query, domains)
}

fn build_wayback_domain_url(domain: &str) -> String {
    format!("https://web.archive.org/web/*/{}*", normalize_domain(domain))
}

fn build_wayback_exact_url(target_url: &str) -> String {
    format!("https://web.archive.org/web/*/{}", target_url.trim())
}

fn detect_archive_metadata(url: &str) -> (String, String, String) {
    let trimmed = url.trim();

    if let Some(rest) = trimmed.strip_prefix("https://web.archive.org/web/") {
        let mut parts = rest.splitn(2, '/');
        let snapshot = parts.next().unwrap_or("").trim().to_string();
        let archived_part = parts.next().unwrap_or("").trim();
        let normalized_original = if archived_part.starts_with("http://")
            || archived_part.starts_with("https://")
        {
            archived_part.to_string()
        } else if archived_part.is_empty() {
            String::new()
        } else {
            format!("https://{archived_part}")
        };

        return (
            normalized_original,
            trimmed.to_string(),
            if snapshot == "*" { String::new() } else { snapshot },
        );
    }

    (trimmed.to_string(), String::new(), String::new())
}

fn push_detected_result(
    results: &mut Vec<SearchResult>,
    tier_filter: i32,
    title: &str,
    url: String,
    snippet: String,
    tier: i32,
    source_type: &str,
    relevance: i32,
    query_used: String,
    action_label: &str,
) {
    let (original_url, archived_url, snapshot_timestamp) = detect_archive_metadata(&url);
    let archive_kind = if archived_url.is_empty() {
        "live".to_string()
    } else if snapshot_timestamp.is_empty() {
        "wayback_index".to_string()
    } else {
        "wayback_snapshot".to_string()
    };

    push_result(
        results,
        tier_filter,
        title,
        url,
        original_url,
        archived_url,
        archive_kind,
        snapshot_timestamp,
        snippet,
        tier,
        source_type,
        relevance,
        query_used,
        action_label,
    );
}

fn fallback_result_url(query: &str) -> String {
    build_search_url("google", query)
}

/// Az AI-javaslatokhoz mindig valódi keresési URL-t ad a legpontosabb elérhető
/// szövegre (query_used → title → alap lekérdezés). Így az AI által esetleg
/// hallucinált nyers URL sosem jelenik meg találati linkként (FIX-AI-LINKS).
fn ai_suggestion_url(query_used: Option<&str>, title: Option<&str>, base_query: &str) -> String {
    let seed = query_used
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| title.map(str::trim).filter(|s| !s.is_empty()))
        .unwrap_or(base_query);
    fallback_result_url(seed)
}

fn request_ai_search_response(
    provider: &str,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    query: &str,
) -> Result<String, String> {
    let user_prompt = format!(
        "Keresesi feladat: {query}\n\nAdj vissza egy JSON objektumot ebben a formatumban: \
{{\"search_brief\":\"...\",\"suggestions\":[{{\"title\":\"...\",\"snippet\":\"...\",\"tier\":2,\"source_type\":\"...\",\"relevance\":88,\"query_used\":\"...\",\"action_label\":\"...\"}}]}}. \
Legyen 5-8 javaslat. NE adj meg URL-t es ne talalj ki linket; a 'query_used' mezobe adj pontos, lefuttathato keresokifejezest. A rendszer minden javaslathoz keresest nyit a kifejezesre."
    );

    let client = ai_client()?;

    let response_text = match provider {
        "openrouter" => {
            let response = client
                .post("https://openrouter.ai/api/v1/chat/completions")
                .bearer_auth(api_key)
                .header("Content-Type", "application/json")
                .json(&json!({
                    "model": model,
                    "temperature": 0.2,
                    "messages": [
                        {"role": "system", "content": system_prompt},
                        {"role": "user", "content": user_prompt}
                    ]
                }))
                .send()
                .map_err(|e| format!("OpenRouter AI hivas sikertelen: {e}"))?;

            response
                .error_for_status()
                .map_err(|e| format!("OpenRouter HTTP hiba: {e}"))?
                .text()
                .map_err(|e| format!("OpenRouter valasz olvasasi hiba: {e}"))?
        }
        "gemini" => {
            let response = client
                .post(format!(
                    "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent"
                ))
                .header("x-goog-api-key", api_key)
                .header("Content-Type", "application/json")
                .json(&json!({
                    "systemInstruction": {
                        "parts": [{"text": system_prompt}]
                    },
                    "contents": [
                        {
                            "parts": [{"text": user_prompt}]
                        }
                    ],
                    "generationConfig": {
                        "temperature": 0.2,
                        "responseMimeType": "application/json"
                    }
                }))
                .send()
                .map_err(|e| format!("Gemini AI hivas sikertelen: {e}"))?;

            response
                .error_for_status()
                .map_err(|e| format!("Gemini HTTP hiba: {e}"))?
                .text()
                .map_err(|e| format!("Gemini valasz olvasasi hiba: {e}"))?
        }
        _ => {
            let response = client
                .post("https://api.openai.com/v1/chat/completions")
                .bearer_auth(api_key)
                .header("Content-Type", "application/json")
                .json(&json!({
                    "model": model,
                    "temperature": 0.2,
                    "response_format": {"type": "json_object"},
                    "messages": [
                        {"role": "system", "content": system_prompt},
                        {"role": "user", "content": user_prompt}
                    ]
                }))
                .send()
                .map_err(|e| format!("OpenAI AI hivas sikertelen: {e}"))?;

            response
                .error_for_status()
                .map_err(|e| format!("OpenAI HTTP hiba: {e}"))?
                .text()
                .map_err(|e| format!("OpenAI valasz olvasasi hiba: {e}"))?
        }
    };

    if response_text.is_empty() {
        return Err("Az AI szolgaltato ures valaszt adott.".to_string());
    }

    let parsed: serde_json::Value =
        serde_json::from_str(&response_text).map_err(|e| format!("AI valasz parse hiba: {e}"))?;

    if provider == "gemini" {
        return parsed["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "A Gemini valaszbol nem olvashato ki szoveges tartalom.".to_string());
    }

    parsed["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Az AI valaszbol nem olvashato ki uzenet tartalom.".to_string())
}

fn extract_json_object(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return trimmed.to_string();
    }

    let start = trimmed.find('{');
    let end = trimmed.rfind('}');
    match (start, end) {
        (Some(from), Some(to)) if to > from => trimmed[from..=to].to_string(),
        _ => trimmed.to_string(),
    }
}

fn add_ai_results(
    results: &mut Vec<SearchResult>,
    tier_filter: i32,
    query: &str,
    settings: &AppSettings,
) -> Result<Option<String>, String> {
    let ai_raw = request_ai_search_response(
        &settings.ai_provider,
        &settings.api_key,
        &settings.ai_model,
        &settings.system_prompt,
        query,
    )?;

    let json_payload = extract_json_object(&ai_raw);
    let parsed = serde_json::from_str::<AiSearchResponse>(&json_payload).unwrap_or(AiSearchResponse {
        search_brief: Some(ai_raw.clone()),
        suggestions: None,
    });

    let brief = parsed.search_brief.clone();
    let suggestions = parsed.suggestions.unwrap_or_default();

    if suggestions.is_empty() {
        push_detected_result(
            results,
            tier_filter,
            "AI kutatasi osszefoglalo",
            fallback_result_url(query),
            ai_raw,
            2,
            "AI javaslat",
            84,
            query.to_string(),
            "Kereses megnyitasa",
        );
        return Ok(brief);
    }

    for item in suggestions {
        let item_query = item
            .query_used
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| query.to_string());

        // FIX-AI-LINKS: az AI nyers url-jét sosem használjuk (gyakran hallucinált);
        // mindig valódi keresést nyitunk a javasolt kifejezésre.
        let url = ai_suggestion_url(item.query_used.as_deref(), item.title.as_deref(), query);

        push_detected_result(
            results,
            tier_filter,
            item.title.as_deref().unwrap_or("AI keresesi javaslat"),
            url,
            item.snippet
                .clone()
                .unwrap_or_else(|| "AI altal javasolt keresesi vagy ellenorzesi lepés.".to_string()),
            item.tier.unwrap_or(2),
            "AI keresési javaslat",
            item.relevance.unwrap_or(80),
            item_query,
            "Keresés megnyitása",
        );
    }

    Ok(brief)
}

fn classify_result_tier(url: &str) -> (i32, &'static str) {
    let lower = url.to_lowercase();
    let host = lower
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or("")
        .trim_start_matches("www.");

    let official_hosts = [
        "njt.hu",
        "magyarkozlony.hu",
        "ksh.hu",
        "kozbeszerzes.hu",
        "birosag.hu",
        "kuria-birosag.hu",
    ];
    if official_hosts.iter().any(|item| host == *item || host.ends_with(&format!(".{item}")))
        || host.ends_with(".gov.hu")
        || host.ends_with(".gov")
    {
        return (1, "Hivatalos forrás");
    }

    if host.contains("scholar.google")
        || host == "archive.org"
        || host == "web.archive.org"
        || host.ends_with(".edu")
        || host == "doi.org"
        || host.contains("ncbi.nlm.nih.gov")
        || host.contains("mta.hu")
    {
        return (2, "Tudományos / archív forrás");
    }

    if host.contains("reddit.com") || host.contains("facebook.com") || host.contains("jofogas.hu") {
        return (4, "Közösségi / kontrollforrás");
    }

    (3, "Webes találat")
}

fn add_cse_results(
    results: &mut Vec<SearchResult>,
    query: &str,
    api_key: &str,
    cx: &str,
) -> Result<usize, String> {
    let client = search_client()?;

    let response = client
        .get("https://www.googleapis.com/customsearch/v1")
        .query(&[
            ("key", api_key),
            ("cx", cx),
            ("q", query),
            ("num", "10"),
            ("hl", "hu"),
        ])
        .send()
        .map_err(|e| format!("Google CSE hívás sikertelen: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        let message = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|value| value["error"]["message"].as_str().map(String::from))
            .unwrap_or_else(|| format!("HTTP {status}"));
        return Err(format!("Google CSE hiba: {message}"));
    }

    let parsed: CseResponse = response
        .json()
        .map_err(|e| format!("Google CSE válasz feldolgozási hiba: {e}"))?;

    let items = parsed.items.unwrap_or_default();
    let count = items.len();

    for (index, item) in items.into_iter().enumerate() {
        let url = item.link.unwrap_or_default();
        if url.trim().is_empty() {
            continue;
        }

        let (tier, source_type) = classify_result_tier(&url);
        let relevance = (96 - (index as i32) * 3).max(60);

        push_detected_result(
            results,
            0,
            item.title.as_deref().unwrap_or("Google találat"),
            url,
            item.snippet
                .unwrap_or_else(|| "Valódi Google találat a Programmable Search Engine-ből.".to_string()),
            tier,
            source_type,
            relevance,
            query.to_string(),
            "Megnyitás",
        );
    }

    Ok(count)
}

fn add_brave_results(
    results: &mut Vec<SearchResult>,
    query: &str,
    api_key: &str,
) -> Result<usize, String> {
    let client = search_client()?;

    let response = client
        .get("https://api.search.brave.com/res/v1/web/search")
        .header("Accept", "application/json")
        .header("X-Subscription-Token", api_key)
        .query(&[
            ("q", query),
            ("count", "10"),
            ("country", "hu"),
            ("search_lang", "hu"),
        ])
        .send()
        .map_err(|e| format!("Brave Search hívás sikertelen: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        let message = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|value| {
                value["error"]["detail"]
                    .as_str()
                    .or_else(|| value["error"]["meta"]["errors"][0]["msg"].as_str())
                    .or_else(|| value["message"].as_str())
                    .map(String::from)
            })
            .unwrap_or_else(|| format!("HTTP {status}"));
        return Err(format!("Brave Search hiba: {message}"));
    }

    let parsed: BraveResponse = response
        .json()
        .map_err(|e| format!("Brave Search válasz feldolgozási hiba: {e}"))?;

    let items = parsed.web.and_then(|web| web.results).unwrap_or_default();
    let count = items.len();

    for (index, item) in items.into_iter().enumerate() {
        let url = item.url.unwrap_or_default();
        if url.trim().is_empty() {
            continue;
        }

        let (tier, source_type) = classify_result_tier(&url);
        let relevance = (95 - (index as i32) * 3).max(60);

        push_detected_result(
            results,
            0,
            item.title.as_deref().unwrap_or("Brave találat"),
            url,
            item.description
                .unwrap_or_else(|| "Valódi webes találat a Brave Search indexéből.".to_string()),
            tier,
            source_type,
            relevance,
            query.to_string(),
            "Megnyitás",
        );
    }

    Ok(count)
}

fn add_tavily_results(
    results: &mut Vec<SearchResult>,
    query: &str,
    api_key: &str,
) -> Result<usize, String> {
    let client = search_client()?;

    let (effective_query, site_domains) = split_query_for_domain_search(query);

    let mut body = json!({
        "query": effective_query,
        "max_results": 10,
        "search_depth": "basic"
    });
    if !site_domains.is_empty() {
        body["include_domains"] = json!(site_domains);
    }

    let response = client
        .post("https://api.tavily.com/search")
        .bearer_auth(api_key)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .map_err(|e| format!("Tavily hívás sikertelen: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        let message = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|value| {
                value["detail"]["error"]
                    .as_str()
                    .or_else(|| value["detail"].as_str())
                    .or_else(|| value["error"].as_str())
                    .or_else(|| value["message"].as_str())
                    .map(String::from)
            })
            .unwrap_or_else(|| format!("HTTP {status}"));
        return Err(format!("Tavily hiba: {message}"));
    }

    let parsed: TavilyResponse = response
        .json()
        .map_err(|e| format!("Tavily válasz feldolgozási hiba: {e}"))?;

    let items = parsed.results.unwrap_or_default();
    let count = items.len();

    for (index, item) in items.into_iter().enumerate() {
        let url = item.url.unwrap_or_default();
        if url.trim().is_empty() {
            continue;
        }

        let (tier, source_type) = classify_result_tier(&url);
        let relevance = (95 - (index as i32) * 3).max(60);

        let snippet: String = item
            .content
            .unwrap_or_else(|| "Valódi webes találat a Tavily keresésből.".to_string())
            .chars()
            .take(500)
            .collect();

        push_detected_result(
            results,
            0,
            item.title.as_deref().unwrap_or("Tavily találat"),
            url,
            snippet,
            tier,
            source_type,
            relevance,
            query.to_string(),
            "Megnyitás",
        );
    }

    Ok(count)
}

fn pack_allowed(selected_source_packs: &[String], pack: &str) -> bool {
    selected_source_packs.is_empty()
        || selected_source_packs
            .iter()
            .any(|selected| selected.trim() == pack)
}

fn add_engine_launchers(
    results: &mut Vec<SearchResult>,
    tier_filter: i32,
    engine: &str,
    query: &str,
) {
    let engine_name = match engine {
        "bing" => "Bing",
        "duckduckgo" => "DuckDuckGo",
        _ => "Google",
    };

    push_detected_result(
        results,
        tier_filter,
        &format!("{engine_name} keresés indítása"),
        build_search_url(engine, query),
        "Közvetlen indítás a kiválasztott keresőmotorban. Ezt érdemes első körben megnyitni, majd az archiválható találatokat kézzel értékelni.".to_string(),
        3,
        "Keresőmotor",
        92,
        query.to_string(),
        "Megnyitás",
    );

    for (label, variant) in build_query_variants(query) {
        push_detected_result(
            results,
            tier_filter,
            &format!("{engine_name} variáns: {label}"),
            build_search_url(engine, &variant),
            format!(
                "Alternatív lekérdezés ugyanarra az ügyre. Cél: jobb recall és eltérő szóhasználattal indexelt találatok felszínre hozása. Lekérdezés: {variant}"
            ),
            3,
            "Lekérdezési variáns",
            78,
            variant.clone(),
            "Variáns megnyitása",
        );
    }
}

fn add_curated_sources(
    results: &mut Vec<SearchResult>,
    tier_filter: i32,
    query: &str,
    selected_source_packs: &[String],
) {
    let official_sources = vec![
        (
            "official",
            "Nemzeti Jogszabálytár célzott keresés",
            format!("https://njt.hu/"),
            "Nyissa meg a jogszabálytárat, és futtassa le a lekérdezést a kulcsfogalmakra. Jogalap, definíció és hatály ellenőrzésére való.",
            1,
            "Hivatalos forrás",
            95,
        ),
        (
            "official",
            "Magyar Közlöny célzott keresés",
            format!("https://magyarkozlony.hu/"),
            "Rendeletek, módosítások és végrehajtási szabályok ellenőrzésére. Különösen hasznos, ha a kérdés időponthoz vagy hatálybalépéshez kötött.",
            1,
            "Hivatalos forrás",
            93,
        ),
        (
            "official",
            "KSH célzott keresés",
            format!("https://www.ksh.hu/"),
            "Statisztikai háttér, árszint, munkaerőpiaci vagy ágazati adatok validálására.",
            1,
            "Statisztikai forrás",
            90,
        ),
        (
            "official",
            "Közbeszerzési Értesítő célzott keresés",
            format!(
                "https://www.google.com/search?q={}",
                url_encode(&format!("{query} site:kozbeszerzes.hu"))
            ),
            "Közbeszerzési hirdetmények, szerződéses összegek, beszerzési tárgyak és nyertes ajánlatok összevetésére. Különösen hasznos ár- és piaci kontrollnál.",
            1,
            "Hivatalos beszerzési forrás",
            89,
        ),
        (
            "science",
            "Google Scholar / tudományos keresés",
            format!(
                "https://scholar.google.com/scholar?q={}",
                url_encode(query)
            ),
            "Szakirodalmi alátámasztás, módszertani cikkek és hivatkozható publikációk keresésére.",
            2,
            "Tudományos forrás",
            82,
        ),
        (
            "shared",
            "Google Drive / dokumentummegosztók",
            format!(
                "https://www.google.com/search?q={}",
                url_encode(&format!("{query} (site:drive.google.com OR site:docs.google.com)"))
            ),
            "Megosztott dokumentumok, táblázatok, prezentációk és segédanyagok felkutatására. Csak ellenőrzött tartalmat szabad bizonyítékként kezelni.",
            2,
            "Megosztott dokumentum",
            72,
        ),
        (
            "market",
            "Profession / munkaerőpiaci összevetés",
            format!(
                "https://www.google.com/search?q={}",
                url_encode(&format!("{query} site:profession.hu"))
            ),
            "Piaci bérezési, óradíjas vagy munkaköri analógiák gyűjtésére.",
            3,
            "Piaci forrás",
            68,
        ),
        (
            "control",
            "Jófogás / apróhirdetés összevetés",
            format!(
                "https://www.google.com/search?q={}",
                url_encode(&format!("{query} site:jofogas.hu"))
            ),
            "Alacsonyabb bizonyító erejű, de piaci becsléshez hasznos kontrollforrás. Mindig külön kezelendő.",
            4,
            "Piaci kontroll",
            52,
        ),
        (
            "reddit",
            "Reddit kontrollkeresés",
            format!(
                "https://www.google.com/search?q={}",
                url_encode(&format!("{query} site:reddit.com"))
            ),
            "Gyakorlati tapasztalatok, felhasználói beszámolók és közösségi kontrollpontok keresésére. Önálló bizonyítékként nem, csak háttéranyagként vagy kontrollforrásként kezelendő.",
            4,
            "Közösségi kontroll",
            48,
        ),
        (
            "facebook_groups",
            "Facebook csoport kontrollkeresés",
            format!(
                "https://www.google.com/search?q={}",
                url_encode(&format!("{query} (site:facebook.com/groups OR site:facebook.com)"))
            ),
            "Google altal indexelt Facebook csoportok, oldalak es nyilvanos kozossegi talalatok felkutatasara. Korlatozott bizonyito ereju kontrollforras.",
            4,
            "Közösségi kontroll",
            44,
        ),
    ];

    for (pack, title, url, snippet, tier, source_type, relevance) in official_sources {
        if !pack_allowed(selected_source_packs, pack) {
            continue;
        }
        push_detected_result(
            results,
            tier_filter,
            title,
            url,
            format!("{snippet} Kiinduló lekérdezés: {query}"),
            tier,
            source_type,
            relevance,
            query.to_string(),
            "Forrás megnyitása",
        );
    }
}

fn add_domain_specific_sources(
    results: &mut Vec<SearchResult>,
    tier_filter: i32,
    query: &str,
    selected_source_packs: &[String],
) {
    let lower = normalize_for_match(query);

    if pack_allowed(selected_source_packs, "official")
        && (lower.contains("mezogazdas") || lower.contains("gep") || lower.contains("oraber"))
    {
        push_detected_result(
            results,
            tier_filter,
            "Mezőgazdasági gépi munkák célzott keresés",
            format!(
                "https://www.google.com/search?q={}",
                url_encode(&format!(
                    "{query} (site:ksh.hu OR site:aki.gov.hu OR site:nak.hu)"
                ))
            ),
            "Ágazati költség- és óradíjadatokhoz. Szakmai anyagok, éves jelentések és normatívák felkutatására.".to_string(),
            1,
            "Ágazati forrás",
            94,
            query.to_string(),
            "Ágazati keresés",
        );
    }

    if pack_allowed(selected_source_packs, "official")
        && (lower.contains("kozbeszer")
        || lower.contains("beszerzes")
        || lower.contains("ajanlat")
        || lower.contains("szerzodeses ar")
        || lower.contains("nyertes ajanlat"))
    {
        push_detected_result(
            results,
            tier_filter,
            "Közbeszerzési Értesítő ár- és szerződéskontroll",
            format!(
                "https://www.google.com/search?q={}",
                url_encode(&format!(
                    "{query} site:kozbeszerzes.hu (ár OR szerződés OR ajánlat OR nyertes)"
                ))
            ),
            "Közbeszerzési és szerződéses árkontroll, összehasonlítható beszerzések és nyertes ajánlati információk keresésére.".to_string(),
            1,
            "Hivatalos beszerzési forrás",
            92,
            query.to_string(),
            "Közbeszerzési keresés",
        );
    }

    if pack_allowed(selected_source_packs, "market")
        && (lower.contains("apolas")
        || lower.contains("gondozas")
        || lower.contains("bebiszitter")
        || lower.contains("takaritas")
        || lower.contains("hazvezetes"))
    {
        push_detected_result(
            results,
            tier_filter,
            "Háztartási és gondozási díjak keresése",
            format!(
                "https://www.google.com/search?q={}",
                url_encode(&format!(
                    "{query} (site:profession.hu OR site:jooble.org OR site:bebicsosz.hu OR site:allasok.hu)"
                ))
            ),
            "Munkaerőpiaci, szolgáltatási és hirdetési kontrollok gyűjtésére. A találatokat időpont és földrajzi környezet szerint is szűrni kell.".to_string(),
            3,
            "Piaci forrás",
            76,
            query.to_string(),
            "Piaci keresés",
        );
    }

    let site_targets = extract_site_targets(query);
    let url_targets = extract_url_targets(query);

    if pack_allowed(selected_source_packs, "archive") {
        if let Some(target_url) = url_targets.first() {
        push_detected_result(
            results,
            tier_filter,
            "Webarchív keresés",
            build_wayback_exact_url(target_url),
            format!(
                "Egyetlen összefogott webarchív modul a megadott URL archivált példányaihoz. Eltűnt aloldal, korábbi tartalom vagy később módosított publikáció ellenőrzésére: {target_url}."
            ),
            2,
            "Webarchív",
            91,
            query.to_string(),
            "Webarchív megnyitása",
        );
        } else if let Some(domain) = site_targets.first() {
        push_detected_result(
            results,
            tier_filter,
            "Webarchív keresés",
            build_wayback_domain_url(domain),
            format!(
                "Egyetlen összefogott webarchív modul a(z) {domain} domain archivált idővonalához és snapshotjaihoz. Régebbi tartalom, megszűnt aloldal vagy változáskövetés ellenőrzésére."
            ),
            2,
            "Webarchív",
            88,
            query.to_string(),
            "Webarchív megnyitása",
        );
        } else {
        push_detected_result(
            results,
            tier_filter,
            "Webarchív keresés",
            build_internet_archive_url(&format!("{query} wayback machine archived site")),
            "Egyetlen összefogott webarchív modul korábbi weboldal-verziók, eltűnt aloldalak és időbeli változások visszakeresésére.".to_string(),
            2,
            "Webarchív",
            74,
            query.to_string(),
            "Webarchív keresés",
        );
        }
    }

    if lower.contains("ar")
        || lower.contains("oradij")
        || lower.contains("tapasztalat")
        || lower.contains("velemeny")
        || lower.contains("bebiszitter")
        || lower.contains("gondozas")
        || lower.contains("takaritas")
    {
        push_detected_result(
            results,
            tier_filter,
            "Reddit közösségi kontroll",
            format!(
                "https://www.google.com/search?q={}",
                url_encode(&format!("{query} site:reddit.com"))
            ),
            "Közösségi beszélgetések, helyi tapasztalatok és ár/gyakorlat kontrollja. Csak más forrásokkal együtt, óvatosan értelmezhető.".to_string(),
            4,
            "Közösségi kontroll",
            46,
            query.to_string(),
            "Reddit keresés",
        );
    }

    if lower.contains("facebook")
        || lower.contains("csoport")
        || lower.contains("helyi")
        || lower.contains("ajanlas")
        || lower.contains("vallalkozo")
    {
        push_detected_result(
            results,
            tier_filter,
            "Facebook csoport / oldal kontroll",
            format!(
                "https://www.google.com/search?q={}",
                url_encode(&format!(
                    "{query} (site:facebook.com/groups OR site:facebook.com)"
                ))
            ),
            "Google altal indexelt Facebook csoportok, ajanlasok es helyi kozossegi nyomok keresese. Csak kontroll- vagy hattérforraskent kezeld.".to_string(),
            4,
            "Közösségi kontroll",
            42,
            query.to_string(),
            "Facebook keresés",
        );
    }
}

#[tauri::command]
fn init_db(state: tauri::State<'_, DbState>) -> Result<String, String> {
    let conn = get_connection(&state.db_path)?;
    init_schema(&conn)?;
    let settings = load_app_settings(&state.db_path)?;
    save_app_settings_internal(&state.db_path, &settings)?;
    Ok("Adatbázis sikeresen inicializálva.".to_string())
}

#[tauri::command]
fn get_app_settings(state: tauri::State<'_, DbState>) -> Result<AppSettings, String> {
    let mut settings = load_app_settings(&state.db_path)?;
    settings.api_key = String::new();
    settings.google_cse_key = String::new();
    settings.brave_key = String::new();
    settings.tavily_key = String::new();
    Ok(settings)
}

#[tauri::command]
fn save_app_settings(
    state: tauri::State<'_, DbState>,
    search_mode: String,
    ai_provider: String,
    api_key: String,
    ai_model: String,
    system_prompt: String,
    google_cse_key: Option<String>,
    google_cse_cx: Option<String>,
    brave_key: Option<String>,
    tavily_key: Option<String>,
) -> Result<String, String> {
    let effective_api_key = if api_key.trim().is_empty() {
        load_raw_setting(&state.db_path, "api_key")?
    } else {
        api_key
    };

    let cse_key_input = google_cse_key.unwrap_or_default();
    let effective_cse_key = if cse_key_input.trim().is_empty() {
        load_raw_setting(&state.db_path, "google_cse_key")?
    } else {
        cse_key_input
    };

    let brave_key_input = brave_key.unwrap_or_default();
    let effective_brave_key = if brave_key_input.trim().is_empty() {
        load_raw_setting(&state.db_path, "brave_key")?
    } else {
        brave_key_input
    };

    let tavily_key_input = tavily_key.unwrap_or_default();
    let effective_tavily_key = if tavily_key_input.trim().is_empty() {
        load_raw_setting(&state.db_path, "tavily_key")?
    } else {
        tavily_key_input
    };

    let settings = AppSettings {
        search_mode,
        ai_provider,
        api_key: effective_api_key,
        ai_model,
        system_prompt,
        api_key_set: false,
        google_cse_key: effective_cse_key,
        google_cse_cx: google_cse_cx.unwrap_or_default().trim().to_string(),
        google_cse_key_set: false,
        brave_key: effective_brave_key,
        brave_key_set: false,
        tavily_key: effective_tavily_key,
        tavily_key_set: false,
    };

    save_app_settings_internal(&state.db_path, &settings)?;
    Ok("Alkalmazás beállítások mentve.".to_string())
}

#[tauri::command]
fn get_cases(state: tauri::State<'_, DbState>) -> Result<Vec<Case>, String> {
    let conn = get_connection(&state.db_path)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, title, description, deadline, status, created_at,
                    preliminary_opinion, final_opinion, report_status, finalizable, open_questions, closed_points
             FROM cases
             ORDER BY id DESC",
        )
        .map_err(|e| e.to_string())?;

    let case_iter = stmt
        .query_map([], |row| {
            Ok(Case {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                deadline: row.get(3)?,
                status: row.get(4)?,
                created_at: row.get(5)?,
                preliminary_opinion: row.get(6)?,
                final_opinion: row.get(7)?,
                report_status: row.get(8)?,
                finalizable: row.get(9)?,
                open_questions: row.get(10)?,
                closed_points: row.get(11)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut cases = Vec::new();
    for item in case_iter {
        cases.push(item.map_err(|e| e.to_string())?);
    }
    Ok(cases)
}

#[tauri::command]
fn create_case(
    state: tauri::State<'_, DbState>,
    title: String,
    description: String,
    deadline: String,
) -> Result<i64, String> {
    let conn = get_connection(&state.db_path)?;
    let created_at = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let status = "Aktív".to_string();

    conn.execute(
        "INSERT INTO cases (title, description, deadline, status, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![title, description, deadline, status, created_at],
    )
    .map_err(|e| e.to_string())?;

    let id = conn.last_insert_rowid();
    log_action(
        &state.db_path,
        id,
        "case_creation",
        &title,
        "Új ügy létrehozva a keresési és forrásértékelési workflow-hoz.",
    )?;

    Ok(id)
}

#[tauri::command]
fn update_case_opinions(
    state: tauri::State<'_, DbState>,
    id: i64,
    preliminary_opinion: String,
    final_opinion: String,
    report_status: String,
    finalizable: String,
    open_questions: String,
    closed_points: String,
) -> Result<String, String> {
    let conn = get_connection(&state.db_path)?;

    let title: String = conn
        .query_row(
            "SELECT title FROM cases WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    conn.execute(
        "UPDATE cases
         SET preliminary_opinion = ?1, final_opinion = ?2, report_status = ?3,
             finalizable = ?4, open_questions = ?5, closed_points = ?6
         WHERE id = ?7",
        params![
            preliminary_opinion,
            final_opinion,
            report_status,
            finalizable,
            open_questions,
            closed_points,
            id
        ],
    )
    .map_err(|e| e.to_string())?;

    log_action(
        &state.db_path,
        id,
        "case_opinion_update",
        &title,
        &format!(
            "Ügy álláspont és lezárási státusz frissítve. Riport: {}, véglegesíthető: {}.",
            report_status, finalizable
        ),
    )?;

    Ok("Ügy álláspontjai mentve.".to_string())
}

#[tauri::command]
fn delete_case(state: tauri::State<'_, DbState>, id: i64) -> Result<String, String> {
    let mut conn = get_connection(&state.db_path)?;

    let file_paths: Vec<String> = {
        let mut stmt = conn
            .prepare("SELECT file_path FROM sources WHERE case_id = ?1 AND file_path != ''")
            .map_err(|e| e.to_string())?;
        let iter = stmt
            .query_map(params![id], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        iter.filter_map(|item| item.ok()).collect()
    };

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM cases WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM sources WHERE case_id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM audit_logs WHERE case_id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM search_runs WHERE case_id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;

    let mut failed_files = 0usize;
    for path in &file_paths {
        if !is_within_archive(&state.db_path, path) {
            let _ = log_action(
                &state.db_path,
                id,
                "case_file_delete_skipped_out_of_scope",
                path,
                "Törlés kihagyva: a fájl az archive mappán kívül esik.",
            );
            continue;
        }
        match fs::remove_file(path) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => failed_files += 1,
        }
    }

    if failed_files > 0 {
        return Ok(format!(
            "Ügy és kapcsolódó adatok törölve. {failed_files} archivált fájl törlése nem sikerült, ezek kézzel távolíthatók el az archive mappából."
        ));
    }

    Ok("Ügy és kapcsolódó adatok törölve.".to_string())
}

#[tauri::command]
fn get_sources(state: tauri::State<'_, DbState>, case_id: i64) -> Result<Vec<Source>, String> {
    let conn = get_connection(&state.db_path)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, case_id, search_run_id, title, url, original_url, archived_url, snapshot_timestamp, archive_kind, tier, content, file_path, hash, timestamp,
                    source_status, priority_level, usability_decision, proof_summary, reviewer_name, reviewed_at,
                    relevance_note, probative_value, missing_points
             FROM sources
             WHERE case_id = ?1
             ORDER BY priority_level ASC, id DESC",
        )
        .map_err(|e| e.to_string())?;

    let iter = stmt
        .query_map(params![case_id], |row| {
            Ok(Source {
                id: row.get(0)?,
                case_id: row.get(1)?,
                search_run_id: row.get(2)?,
                title: row.get(3)?,
                url: row.get(4)?,
                original_url: row.get(5)?,
                archived_url: row.get(6)?,
                snapshot_timestamp: row.get(7)?,
                archive_kind: row.get(8)?,
                tier: row.get(9)?,
                content: row.get(10)?,
                file_path: row.get(11)?,
                hash: row.get(12)?,
                timestamp: row.get(13)?,
                source_status: row.get(14)?,
                priority_level: row.get(15)?,
                usability_decision: row.get(16)?,
                proof_summary: row.get(17)?,
                reviewer_name: row.get(18)?,
                reviewed_at: row.get(19)?,
                relevance_note: row.get(20)?,
                probative_value: row.get(21)?,
                missing_points: row.get(22)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut sources = Vec::new();
    for item in iter {
        sources.push(item.map_err(|e| e.to_string())?);
    }
    Ok(sources)
}

#[tauri::command]
fn delete_source(state: tauri::State<'_, DbState>, id: i64) -> Result<String, String> {
    let conn = get_connection(&state.db_path)?;

    let (case_id, file_path): (i64, String) = conn
        .query_row(
            "SELECT case_id, file_path FROM sources WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    if !file_path.is_empty() {
        if is_within_archive(&state.db_path, &file_path) {
            match fs::remove_file(&file_path) {
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    log_action(
                        &state.db_path,
                        case_id,
                        "source_deletion_error",
                        &id.to_string(),
                        &format!("Forrásfájl törlése sikertelen: {file_path}. Hiba: {error}"),
                    )?;
                    return Err(format!("A forrásfájl törlése sikertelen: {error}"));
                }
            }
        } else {
            log_action(
                &state.db_path,
                case_id,
                "source_file_delete_skipped_out_of_scope",
                &id.to_string(),
                &format!("Törlés kihagyva: a fájl az archive mappán kívül esik: {file_path}"),
            )?;
        }
    }

    conn.execute("DELETE FROM sources WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;

    log_action(
        &state.db_path,
        case_id,
        "source_deletion",
        &id.to_string(),
        "Archivált forrás törölve.",
    )?;

    Ok("Forrás törölve.".to_string())
}

#[tauri::command]
fn get_audit_logs(state: tauri::State<'_, DbState>, case_id: i64) -> Result<Vec<AuditLog>, String> {
    let conn = get_connection(&state.db_path)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, case_id, timestamp, action_type, query, details
             FROM audit_logs
             WHERE case_id = ?1
             ORDER BY id DESC",
        )
        .map_err(|e| e.to_string())?;

    let iter = stmt
        .query_map(params![case_id], |row| {
            Ok(AuditLog {
                id: row.get(0)?,
                case_id: row.get(1)?,
                timestamp: row.get(2)?,
                action_type: row.get(3)?,
                query: row.get(4)?,
                details: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut logs = Vec::new();
    for item in iter {
        logs.push(item.map_err(|e| e.to_string())?);
    }
    Ok(logs)
}

#[tauri::command]
fn get_search_runs(
    state: tauri::State<'_, DbState>,
    case_id: i64,
) -> Result<Vec<SearchRun>, String> {
    let conn = get_connection(&state.db_path)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, case_id, created_at, summary, base_query, engine, tier_filter, plan_count, result_count
                   , note
             FROM search_runs
             WHERE case_id = ?1
             ORDER BY id DESC",
        )
        .map_err(|e| e.to_string())?;

    let iter = stmt
        .query_map(params![case_id], |row| {
            Ok(SearchRun {
                id: row.get(0)?,
                case_id: row.get(1)?,
                created_at: row.get(2)?,
                summary: row.get(3)?,
                base_query: row.get(4)?,
                engine: row.get(5)?,
                tier_filter: row.get(6)?,
                plan_count: row.get(7)?,
                result_count: row.get(8)?,
                note: row.get(9)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut runs = Vec::new();
    for item in iter {
        runs.push(item.map_err(|e| e.to_string())?);
    }
    Ok(runs)
}

#[tauri::command]
fn record_search_run(
    state: tauri::State<'_, DbState>,
    case_id: i64,
    summary: String,
    base_query: String,
    engine: String,
    tier_filter: i32,
    plan_count: i32,
    result_count: i32,
) -> Result<i64, String> {
    let conn = get_connection(&state.db_path)?;
    let created_at = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    conn.execute(
        "INSERT INTO search_runs (case_id, created_at, summary, base_query, engine, tier_filter, plan_count, result_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            case_id,
            created_at,
            summary,
            base_query,
            engine,
            tier_filter,
            plan_count,
            result_count
        ],
    )
    .map_err(|e| e.to_string())?;

    let new_id = conn.last_insert_rowid();
    log_action(
        &state.db_path,
        case_id,
        "search_run_recorded",
        &base_query,
        &format!(
            "Keresési futam rögzítve. Motorok: {engine}, tervlépések: {plan_count}, találatok: {result_count}."
        ),
    )?;

    Ok(new_id)
}

#[tauri::command]
fn update_search_run_note(
    state: tauri::State<'_, DbState>,
    id: i64,
    note: String,
) -> Result<String, String> {
    let conn = get_connection(&state.db_path)?;

    let (case_id, base_query): (i64, String) = conn
        .query_row(
            "SELECT case_id, base_query FROM search_runs WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    conn.execute(
        "UPDATE search_runs SET note = ?1 WHERE id = ?2",
        params![note, id],
    )
    .map_err(|e| e.to_string())?;

    log_action(
        &state.db_path,
        case_id,
        "search_run_note_update",
        &base_query,
        "Keresési futam szakértői megjegyzése frissítve.",
    )?;

    Ok("Futam megjegyzése mentve.".to_string())
}

#[tauri::command]
fn update_source_review(
    state: tauri::State<'_, DbState>,
    id: i64,
    source_status: String,
    priority_level: i32,
    usability_decision: String,
    proof_summary: String,
    reviewer_name: String,
    reviewed_at: String,
    relevance_note: String,
    probative_value: String,
    missing_points: String,
) -> Result<String, String> {
    let conn = get_connection(&state.db_path)?;

    let (case_id, url): (i64, String) = conn
        .query_row(
            "SELECT case_id, url FROM sources WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    let normalized_reviewed_at = if reviewed_at.trim().is_empty()
        && (source_status == "validalt" || !reviewer_name.trim().is_empty())
    {
        Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
    } else {
        reviewed_at.trim().to_string()
    };

    conn.execute(
        "UPDATE sources
         SET source_status = ?1, priority_level = ?2, usability_decision = ?3, proof_summary = ?4,
             reviewer_name = ?5, reviewed_at = ?6, relevance_note = ?7, probative_value = ?8, missing_points = ?9
         WHERE id = ?10",
        params![
            source_status,
            priority_level,
            usability_decision,
            proof_summary,
            reviewer_name,
            normalized_reviewed_at,
            relevance_note,
            probative_value,
            missing_points,
            id
        ],
    )
    .map_err(|e| e.to_string())?;

    log_action(
        &state.db_path,
        case_id,
        "source_review_update",
        &url,
        &format!(
            "Forrásértékelés frissítve. Státusz: {}, prioritás: P{}, döntés: {}, ellenőrző: {}, ellenőrzés ideje: {}.",
            source_status,
            priority_level,
            usability_decision,
            if reviewer_name.trim().is_empty() {
                "nincs megadva"
            } else {
                reviewer_name.trim()
            },
            if normalized_reviewed_at.trim().is_empty() {
                "nincs megadva"
            } else {
                normalized_reviewed_at.trim()
            }
        ),
    )?;

    Ok("Forrásértékelés mentve.".to_string())
}

#[tauri::command]
async fn execute_search(
    state: tauri::State<'_, DbState>,
    case_id: i64,
    search_query: String,
    tier_filter: i32,
    engine: String,
    search_mode: String,
    include_cse: Option<bool>,
    selected_source_packs: Option<Vec<String>>,
) -> Result<SearchRunOutput, String> {
    let db_path = state.db_path.clone();
    let include_cse = include_cse.unwrap_or(true);
    let selected_source_packs = selected_source_packs.unwrap_or_default();
    tauri::async_runtime::spawn_blocking(move || {
        execute_search_blocking(
            &db_path,
            case_id,
            &search_query,
            tier_filter,
            &engine,
            &search_mode,
            include_cse,
            &selected_source_packs,
        )
    })
    .await
    .map_err(|e| format!("Keresési futam belső hiba: {e}"))?
}

fn execute_search_blocking(
    db_path: &str,
    case_id: i64,
    search_query: &str,
    tier_filter: i32,
    engine: &str,
    search_mode: &str,
    include_cse: bool,
    selected_source_packs: &[String],
) -> Result<SearchRunOutput, String> {
    let trimmed = search_query.trim();
    if trimmed.is_empty() {
        return Ok(SearchRunOutput {
            results: Vec::new(),
            ai_error: None,
            cse_error: None,
            brave_error: None,
            tavily_error: None,
            generated_count: 0,
            filtered_out_count: 0,
        });
    }
    if trimmed.chars().count() > MAX_SEARCH_QUERY_LEN {
        return Err(format!(
            "A keresési lekérdezés túl hosszú. Maximum {MAX_SEARCH_QUERY_LEN} karakter engedélyezett."
        ));
    }

    let settings = load_app_settings(db_path)?;
    let effective_mode = if search_mode.trim().is_empty() {
        settings.search_mode.clone()
    } else {
        search_mode.trim().to_string()
    };

    log_action(
        db_path,
        case_id,
        "search_execution",
        trimmed,
        &format!(
            "Keresési futam generálva. Mod: {effective_mode}, motor: {engine}, AI provider: {}, Tier szűrő: {tier_filter}.",
            settings.ai_provider
        ),
    )?;

    let mut results = Vec::new();
    let mut ai_brief: Option<String> = None;
    let mut ai_error: Option<String> = None;
    let mut cse_error: Option<String> = None;
    let mut brave_error: Option<String> = None;
    let mut tavily_error: Option<String> = None;

    if effective_mode != "ai_only" {
        let constructed_start = results.len();
        add_engine_launchers(&mut results, 0, engine, trimmed);
        add_curated_sources(&mut results, 0, trimmed, selected_source_packs);
        add_domain_specific_sources(&mut results, 0, trimmed, selected_source_packs);
        tag_evidence(&mut results, constructed_start, "CONSTRUCTED");

        let retrieved_start = results.len();
        if include_cse && !settings.tavily_key.trim().is_empty() {
            match add_tavily_results(&mut results, trimmed, settings.tavily_key.trim()) {
                Ok(count) => {
                    let _ = log_action(
                        db_path,
                        case_id,
                        "tavily_search",
                        trimmed,
                        &format!("Tavily lekérdezés lefutott, {count} valódi találat érkezett."),
                    );
                }
                Err(error) => {
                    let error = redact_secrets(&error, &settings);
                    tavily_error = Some(error.clone());
                    let _ = log_action(
                        db_path,
                        case_id,
                        "tavily_search_error",
                        trimmed,
                        &format!("A Tavily lekérdezés nem futott le, a keresési terv kártyái megmaradtak. Hiba: {error}"),
                    );
                }
            }
        }

        if include_cse && !settings.brave_key.trim().is_empty() {
            match add_brave_results(&mut results, trimmed, settings.brave_key.trim()) {
                Ok(count) => {
                    let _ = log_action(
                        db_path,
                        case_id,
                        "brave_search",
                        trimmed,
                        &format!("Brave Search lekérdezés lefutott, {count} valódi találat érkezett."),
                    );
                }
                Err(error) => {
                    let error = redact_secrets(&error, &settings);
                    brave_error = Some(error.clone());
                    let _ = log_action(
                        db_path,
                        case_id,
                        "brave_search_error",
                        trimmed,
                        &format!("A Brave Search lekérdezés nem futott le, a keresési terv kártyái megmaradtak. Hiba: {error}"),
                    );
                }
            }
        }

        let cse_configured = !settings.google_cse_key.trim().is_empty()
            && !settings.google_cse_cx.trim().is_empty();
        if include_cse && cse_configured {
            match add_cse_results(
                &mut results,
                trimmed,
                settings.google_cse_key.trim(),
                settings.google_cse_cx.trim(),
            ) {
                Ok(count) => {
                    let _ = log_action(
                        db_path,
                        case_id,
                        "cse_search",
                        trimmed,
                        &format!("Google CSE lekérdezés lefutott, {count} valódi találat érkezett."),
                    );
                }
                Err(error) => {
                    let error = redact_secrets(&error, &settings);
                    cse_error = Some(error.clone());
                    let _ = log_action(
                        db_path,
                        case_id,
                        "cse_search_error",
                        trimmed,
                        &format!("A Google CSE lekérdezés nem futott le, a keresési terv kártyái megmaradtak. Hiba: {error}"),
                    );
                }
            }
        }
        tag_evidence(&mut results, retrieved_start, "RETRIEVED");
    }

    let suggested_start = results.len();
    if effective_mode != "native" {
        if settings.api_key.trim().is_empty() {
            ai_error = Some(
                "AI módhoz nincs elmentett API kulcs. Mentsd el az AI beállításokat a keresési panelen."
                    .to_string(),
            );
        } else {
            match add_ai_results(&mut results, 0, trimmed, &settings) {
                Ok(brief) => ai_brief = brief,
                Err(error) => {
                    let error = redact_secrets(&error, &settings);
                    ai_error = Some(error.clone());
                    let _ = log_action(
                        db_path,
                        case_id,
                        "ai_search_error",
                        trimmed,
                        &format!("Az AI kiegészítés nem futott le, a natív találatok megmaradtak. Hiba: {error}"),
                    );
                }
            }
        }
    }

    if let Some(brief) = ai_brief {
        if !brief.trim().is_empty() {
            push_detected_result(
                &mut results,
                0,
                "AI kutatasi iranyosszefoglalo",
                fallback_result_url(trimmed),
                brief,
                2,
                "AI osszegzes",
                86,
                trimmed.to_string(),
                "Alapkereses megnyitasa",
            );
        }
    }
    tag_evidence(&mut results, suggested_start, "SUGGESTED");

    let generated_count = results.len() as i32;
    if tier_filter > 0 {
        results.retain(|result| result.tier <= tier_filter);
    }
    let filtered_out_count = generated_count - results.len() as i32;

    if results.is_empty() && generated_count > 0 {
        let _ = log_action(
            db_path,
            case_id,
            "search_zero_results",
            trimmed,
            &format!(
                "A futam minden kártyáját elrejtette a szűrő. Generált: {generated_count}, Tier szűrő: {tier_filter}, mód: {effective_mode}."
            ),
        );
    }

    results.sort_by(|a, b| b.relevance.cmp(&a.relevance));
    Ok(SearchRunOutput {
        results,
        ai_error,
        cse_error,
        brave_error,
        tavily_error,
        generated_count,
        filtered_out_count,
    })
}

#[tauri::command]
fn archive_source(
    state: tauri::State<'_, DbState>,
    case_id: i64,
    search_run_id: Option<i64>,
    title: String,
    url: String,
    tier: i32,
    content: String,
) -> Result<Source, String> {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let (original_url, archived_url, snapshot_timestamp) = detect_archive_metadata(&url);
    let normalized_judgement_url = if !original_url.is_empty() {
        original_url.clone()
    } else {
        url.clone()
    };
    let archive_kind = if archived_url.is_empty() {
        "live".to_string()
    } else if snapshot_timestamp.is_empty() {
        "wayback_index".to_string()
    } else {
        "wayback_snapshot".to_string()
    };
    let is_reddit = normalized_judgement_url.contains("reddit.com");
    let is_facebook = normalized_judgement_url.contains("facebook.com");
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let hash_hex = hex::encode(hasher.finalize());

    let archive_dir = Path::new(&state.db_path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("archive");

    if !archive_dir.exists() {
        fs::create_dir_all(&archive_dir).map_err(|e| e.to_string())?;
    }

    let file_name = format!("src_{case_id}_{}.txt", Local::now().timestamp_millis());
    let file_path = archive_dir.join(file_name);
    fs::write(&file_path, &content).map_err(|e| e.to_string())?;
    let file_path_str = file_path.to_string_lossy().into_owned();

    let conn = get_connection(&state.db_path)?;
    conn.execute(
        "INSERT INTO sources (case_id, search_run_id, title, url, original_url, archived_url, snapshot_timestamp, archive_kind, tier, content, file_path, hash, timestamp)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            case_id,
            search_run_id,
            title,
            url,
            original_url,
            archived_url,
            snapshot_timestamp,
            archive_kind,
            tier,
            content,
            file_path_str,
            hash_hex,
            timestamp
        ],
    )
    .map_err(|e| e.to_string())?;

    let new_id = conn.last_insert_rowid();
    log_action(
        &state.db_path,
        case_id,
        "source_archiving",
        &url,
        &format!(
            "Forrás archiválva. Cím: {title}, hash: {hash_hex}, archiv tipus: {}, eredeti URL: {}",
            archive_kind,
            if original_url.is_empty() { "n/a" } else { &original_url }
        ),
    )?;

    Ok(Source {
        id: new_id,
        case_id,
        search_run_id,
        title,
        url,
        original_url,
        archived_url,
        snapshot_timestamp,
        archive_kind,
        tier,
        content,
        file_path: file_path_str,
        hash: hash_hex,
        timestamp,
        source_status: "nyers".to_string(),
        priority_level: if is_reddit || is_facebook { 3 } else { 2 },
        usability_decision: if is_reddit {
            "hatteranyag".to_string()
        } else {
            "hatteranyag".to_string()
        },
        proof_summary: String::new(),
        reviewer_name: String::new(),
        reviewed_at: String::new(),
        relevance_note: if is_reddit || is_facebook {
            "Kozossegi kontrollforras, onallo bizonyitokkent csak korlatozottan hasznalhato.".to_string()
        } else {
            String::new()
        },
        probative_value: if is_reddit || is_facebook {
            "Gyenge vagy korlatozott bizonyito ero, mas forrasokkal egyutt ellenorizendo.".to_string()
        } else {
            String::new()
        },
        missing_points: String::new(),
    })
}

#[tauri::command]
async fn open_viewer_window(
    app: tauri::AppHandle,
    state: tauri::State<'_, DbState>,
    case_id: i64,
    url: String,
    title: String,
) -> Result<(), String> {
    let parsed: tauri::Url = url
        .parse()
        .map_err(|e| format!("Érvénytelen URL: {e}"))?;

    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err("Csak http vagy https URL nyitható meg a találatnézőben.".to_string());
    }

    if let Some(existing) = app.get_webview_window("viewer") {
        let _ = existing.destroy();
    }

    tauri::WebviewWindowBuilder::new(&app, "viewer", tauri::WebviewUrl::External(parsed))
        .title(format!("IRBR találatnéző – {title}"))
        .inner_size(1100.0, 800.0)
        .build()
        .map_err(|e| format!("A találatnéző ablak megnyitása sikertelen: {e}"))?;

    log_action(
        &state.db_path,
        case_id,
        "source_viewed",
        &url,
        &format!("Találat megnyitva az appon belüli találatnézőben: {title}"),
    )?;

    Ok(())
}

// ---- Beépített böngészőpanel (több webview a főablakban) ----

const BROWSER_LABEL_PREFIX: &str = "browser-";

#[derive(Clone, serde::Serialize)]
struct BrowserNavEvent {
    label: String,
    url: String,
    loading: bool,
}

#[derive(Clone, serde::Serialize)]
struct BrowserTitleEvent {
    label: String,
    title: String,
}

fn browser_label(tab_id: &str) -> Result<String, String> {
    if tab_id.is_empty() || !tab_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err("Érvénytelen böngészőfül-azonosító.".to_string());
    }
    Ok(format!("{BROWSER_LABEL_PREFIX}{tab_id}"))
}

fn parse_browser_url(url: &str) -> Result<tauri::Url, String> {
    let parsed: tauri::Url = url
        .trim()
        .parse()
        .map_err(|e| format!("Érvénytelen URL: {e}"))?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err("A beépített böngészőben csak http vagy https URL nyitható meg.".to_string());
    }
    Ok(parsed)
}

#[tauri::command]
async fn browser_open_tab(
    app: tauri::AppHandle,
    state: tauri::State<'_, DbState>,
    case_id: i64,
    tab_id: String,
    url: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let label = browser_label(&tab_id)?;
    let parsed = parse_browser_url(&url)?;

    if app.get_webview(&label).is_some() {
        return Err("Ez a böngészőfül már létezik.".to_string());
    }

    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "A főablak nem található.".to_string())?
        .as_ref()
        .window();

    let nav_app = app.clone();
    let nav_label = label.clone();
    let load_app = app.clone();
    let load_label = label.clone();
    let title_app = app.clone();
    let title_label = label.clone();

    let builder = tauri::webview::WebviewBuilder::new(&label, tauri::WebviewUrl::External(parsed))
        .on_navigation(move |target| {
            let allowed = target.scheme() == "http" || target.scheme() == "https";
            if allowed {
                let _ = nav_app.emit(
                    "browser-nav",
                    BrowserNavEvent {
                        label: nav_label.clone(),
                        url: target.to_string(),
                        loading: true,
                    },
                );
            }
            allowed
        })
        .on_document_title_changed(move |_webview, title| {
            let _ = title_app.emit(
                "browser-title",
                BrowserTitleEvent {
                    label: title_label.clone(),
                    title,
                },
            );
        })
        .on_page_load(move |_webview, payload| {
            let loading = matches!(payload.event(), tauri::webview::PageLoadEvent::Started);
            let _ = load_app.emit(
                "browser-nav",
                BrowserNavEvent {
                    label: load_label.clone(),
                    url: payload.url().to_string(),
                    loading,
                },
            );
        });

    window
        .add_child(
            builder,
            tauri::LogicalPosition::new(x, y),
            tauri::LogicalSize::new(width.max(1.0), height.max(1.0)),
        )
        .map_err(|e| format!("A böngészőfül megnyitása sikertelen: {e}"))?;

    log_action(
        &state.db_path,
        case_id,
        "browser_tab_opened",
        &url,
        "Oldal megnyitva a beépített böngészőben.",
    )?;

    Ok(())
}

#[tauri::command]
async fn browser_set_bounds(
    app: tauri::AppHandle,
    tab_id: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let label = browser_label(&tab_id)?;
    let Some(webview) = app.get_webview(&label) else {
        return Ok(());
    };
    webview
        .set_position(tauri::LogicalPosition::new(x, y))
        .map_err(|e| e.to_string())?;
    webview
        .set_size(tauri::LogicalSize::new(width.max(1.0), height.max(1.0)))
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn browser_set_visible(
    app: tauri::AppHandle,
    tab_id: String,
    visible: bool,
) -> Result<(), String> {
    let label = browser_label(&tab_id)?;
    let Some(webview) = app.get_webview(&label) else {
        return Ok(());
    };
    if visible {
        webview.show().map_err(|e| e.to_string())
    } else {
        webview.hide().map_err(|e| e.to_string())
    }
}

#[tauri::command]
async fn browser_hide_all(app: tauri::AppHandle) -> Result<(), String> {
    for (label, webview) in app.webviews() {
        if label.starts_with(BROWSER_LABEL_PREFIX) {
            let _ = webview.hide();
        }
    }
    Ok(())
}

#[tauri::command]
async fn browser_navigate(
    app: tauri::AppHandle,
    state: tauri::State<'_, DbState>,
    case_id: i64,
    tab_id: String,
    url: String,
) -> Result<(), String> {
    let label = browser_label(&tab_id)?;
    let parsed = parse_browser_url(&url)?;
    let webview = app
        .get_webview(&label)
        .ok_or_else(|| "A böngészőfül nem található.".to_string())?;
    webview.navigate(parsed).map_err(|e| e.to_string())?;
    log_action(
        &state.db_path,
        case_id,
        "browser_navigated",
        &url,
        "Navigálás a beépített böngészőben.",
    )?;
    Ok(())
}

#[tauri::command]
async fn browser_action(app: tauri::AppHandle, tab_id: String, action: String) -> Result<(), String> {
    let label = browser_label(&tab_id)?;
    let webview = app
        .get_webview(&label)
        .ok_or_else(|| "A böngészőfül nem található.".to_string())?;
    let script = match action.as_str() {
        "back" => "history.back()",
        "forward" => "history.forward()",
        "reload" => "location.reload()",
        _ => return Err("Ismeretlen böngészőművelet.".to_string()),
    };
    webview.eval(script).map_err(|e| e.to_string())
}

#[tauri::command]
async fn browser_close_tab(app: tauri::AppHandle, tab_id: String) -> Result<(), String> {
    let label = browser_label(&tab_id)?;
    if let Some(webview) = app.get_webview(&label) {
        webview.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn extract_tag_content(html: &str, lower: &str, tag: &str) -> Option<String> {
    let open_pos = lower.find(&format!("<{tag}"))?;
    let content_start = open_pos + lower[open_pos..].find('>')? + 1;
    let content_end = content_start + lower[content_start..].find(&format!("</{tag}"))?;
    Some(html[content_start..content_end].trim().to_string())
}

fn strip_html_block(html: &mut String, lower: &mut String, tag: &str) {
    loop {
        let Some(start) = lower.find(&format!("<{tag}")) else {
            break;
        };
        let close_marker = format!("</{tag}");
        let end = match lower[start..].find(&close_marker) {
            Some(offset) => {
                let close_start = start + offset;
                match lower[close_start..].find('>') {
                    Some(gt) => close_start + gt + 1,
                    None => lower.len(),
                }
            }
            None => lower.len(),
        };
        html.replace_range(start..end, " ");
        lower.replace_range(start..end, " ");
    }
}

fn decode_basic_entities(text: &str) -> String {
    text.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

const MAX_FETCHED_TEXT_CHARS: usize = 20000;

fn extract_text_from_html(html: &str) -> (String, String) {
    let mut body = html.to_string();
    let mut lower = html.to_ascii_lowercase();

    let title = extract_tag_content(&body, &lower, "title")
        .map(|value| decode_basic_entities(&value))
        .unwrap_or_default();

    strip_html_block(&mut body, &mut lower, "script");
    strip_html_block(&mut body, &mut lower, "style");
    strip_html_block(&mut body, &mut lower, "noscript");

    let mut text = String::new();
    let mut in_tag = false;
    let mut last_was_space = true;
    for c in body.chars() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                if !last_was_space {
                    text.push(' ');
                    last_was_space = true;
                }
            }
            _ if in_tag => {}
            c if c.is_whitespace() => {
                if !last_was_space {
                    text.push(' ');
                    last_was_space = true;
                }
            }
            c => {
                text.push(c);
                last_was_space = false;
            }
        }
    }

    let decoded = decode_basic_entities(text.trim());
    let truncated: String = decoded.chars().take(MAX_FETCHED_TEXT_CHARS).collect();

    (title, truncated)
}

#[tauri::command]
async fn fetch_page_text(url: String) -> Result<FetchedPage, String> {
    tauri::async_runtime::spawn_blocking(move || fetch_page_text_blocking(&url))
        .await
        .map_err(|e| format!("Oldalletöltési belső hiba: {e}"))?
}

const MAX_FETCH_BYTES: u64 = 5 * 1024 * 1024;

/// true, ha az IP belső/privát/loopback/link-local (SSRF-gát a fetch_page_text-hez).
fn is_blocked_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_documentation()
        }
        IpAddr::V6(v6) => {
            if let Some(mapped) = v6.to_ipv4_mapped() {
                return is_blocked_ip(&IpAddr::V4(mapped));
            }
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                || (v6.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

/// A kérés előtt feloldja a hosztot, és elutasítja a belső/privát célt (SSRF-gát).
fn ensure_host_allowed(url: &reqwest::Url) -> Result<(), String> {
    use std::net::ToSocketAddrs;
    let host = url.host_str().ok_or("Hiányzó hoszt az URL-ben.")?;
    if host.eq_ignore_ascii_case("localhost") {
        return Err("Belső cím nem tölthető le.".to_string());
    }
    let port = url.port_or_known_default().unwrap_or(80);
    let addrs = (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("Névfeloldás sikertelen: {e}"))?;
    let mut any = false;
    for addr in addrs {
        any = true;
        if is_blocked_ip(&addr.ip()) {
            return Err("Belső vagy privát hálózati cím nem tölthető le.".to_string());
        }
    }
    if !any {
        return Err("A hoszt nem feloldható.".to_string());
    }
    Ok(())
}

/// Legfeljebb `max` bájtot olvas a válaszból (memória-korlát).
fn read_body_capped<R: Read>(reader: R, max: u64) -> std::io::Result<Vec<u8>> {
    let mut buffer = Vec::new();
    reader.take(max).read_to_end(&mut buffer)?;
    Ok(buffer)
}

fn fetch_page_text_blocking(url: &str) -> Result<FetchedPage, String> {
    let parsed: reqwest::Url = url
        .parse()
        .map_err(|e| format!("Érvénytelen URL: {e}"))?;

    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err("Csak http vagy https URL tölthető le.".to_string());
    }

    ensure_host_allowed(&parsed)?;

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) IRBR-archiver/0.1")
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            let blocked = attempt
                .url()
                .host_str()
                .map(|h| {
                    h.eq_ignore_ascii_case("localhost")
                        || h.parse::<IpAddr>().map(|ip| is_blocked_ip(&ip)).unwrap_or(false)
                })
                .unwrap_or(false);
            if blocked {
                attempt.error("blokkolt redirect cél")
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(|e| format!("HTTP kliens inicializálási hiba: {e}"))?;

    let response = client
        .get(parsed)
        .send()
        .map_err(|e| format!("Az oldal letöltése sikertelen: {e}"))?;

    if let Some(len) = response.content_length() {
        if len > MAX_FETCH_BYTES {
            return Err("A letöltendő tartalom túl nagy.".to_string());
        }
    }

    let status = response.status().as_u16();
    let final_url = response.url().to_string();
    let bytes = read_body_capped(response, MAX_FETCH_BYTES)
        .map_err(|e| format!("Az oldal tartalmának olvasása sikertelen: {e}"))?;
    let body = String::from_utf8_lossy(&bytes).into_owned();

    let (title, text) = extract_text_from_html(&body);

    Ok(FetchedPage {
        title,
        text,
        status,
        final_url,
    })
}

#[tauri::command]
fn save_report(
    state: tauri::State<'_, DbState>,
    case_id: i64,
    report_content: String,
) -> Result<String, String> {
    let reports_dir = Path::new(&state.db_path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("reports");

    if !reports_dir.exists() {
        fs::create_dir_all(&reports_dir).map_err(|e| e.to_string())?;
    }

    let file_name = format!(
        "irbr_riport_{case_id}_{}.md",
        Local::now().format("%Y%m%d_%H%M%S_%3f")
    );
    let file_path = reports_dir.join(file_name);
    fs::write(&file_path, report_content).map_err(|e| e.to_string())?;
    let path_str = file_path.to_string_lossy().into_owned();

    log_action(
        &state.db_path,
        case_id,
        "report_generation",
        &path_str,
        "Kutatási összefoglaló mentve Markdown formátumban.",
    )?;

    Ok(path_str)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("AppData eleresi utvonal hiba: {e}"))?;

            fs::create_dir_all(&app_data_dir)
                .map_err(|e| format!("AppData mappa letrehozasi hiba: {e}"))?;

            let db_path = app_data_dir.join("irbr_database.db");
            app.manage(DbState {
                db_path: db_path.to_string_lossy().into_owned(),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            init_db,
            get_app_settings,
            save_app_settings,
            get_cases,
            create_case,
            update_case_opinions,
            delete_case,
            get_sources,
            delete_source,
            get_audit_logs,
            get_search_runs,
            record_search_run,
            update_search_run_note,
            update_source_review,
            execute_search,
            archive_source,
            open_viewer_window,
            browser_open_tab,
            browser_set_bounds,
            browser_set_visible,
            browser_hide_all,
            browser_navigate,
            browser_action,
            browser_close_tab,
            fetch_page_text,
            save_report
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_secrets_masks_configured_keys() {
        let mut settings = default_app_settings();
        settings.google_cse_key = "AIzaSyTESTKEY1234567890".to_string();
        let message = "Google CSE hivas sikertelen: key=AIzaSyTESTKEY1234567890 (HTTP 403)";
        let redacted = redact_secrets(message, &settings);
        assert!(!redacted.contains("AIzaSyTESTKEY1234567890"));
        assert!(redacted.contains("***"));
    }

    #[test]
    fn redact_secrets_ignores_empty_and_short_keys() {
        let mut settings = default_app_settings();
        settings.api_key = String::new();
        settings.brave_key = "abc".to_string();
        let message = "Hiba: abc tortent, nincs kulcs";
        assert_eq!(redact_secrets(message, &settings), message);
    }

    #[test]
    fn redact_secrets_masks_all_occurrences() {
        let mut settings = default_app_settings();
        settings.tavily_key = "tvly-SECRETVALUE99".to_string();
        let message = "tvly-SECRETVALUE99 ... ismet tvly-SECRETVALUE99";
        let redacted = redact_secrets(message, &settings);
        assert!(!redacted.contains("tvly-SECRETVALUE99"));
    }

    #[test]
    fn tag_evidence_only_tags_from_index_and_default_is_empty() {
        let mut results = Vec::new();
        push_result(
            &mut results, 0, "a", "u".to_string(), String::new(), String::new(),
            "live".to_string(), String::new(), String::new(), 3, "x", 50, "q".to_string(), "open",
        );
        assert_eq!(results[0].evidence_level, ""); // push_result default üres
        let from = results.len();
        push_result(
            &mut results, 0, "b", "u2".to_string(), String::new(), String::new(),
            "live".to_string(), String::new(), String::new(), 3, "x", 50, "q".to_string(), "open",
        );
        tag_evidence(&mut results, from, "RETRIEVED");
        assert_eq!(results[0].evidence_level, ""); // korábbi érintetlen
        assert_eq!(results[1].evidence_level, "RETRIEVED");
    }

    #[test]
    fn ai_suggestion_url_is_always_a_search_never_raw() {
        // query_used elsőbbség, és mindig Google-keresés (nem nyers AI-URL)
        let u = ai_suggestion_url(Some("mezőgazdasági óradíj 2020"), Some("Cím"), "alap");
        assert!(u.contains("www.google.com/search?q="));
        assert!(u.contains("2020"));
        // üres query_used -> title
        let u2 = ai_suggestion_url(Some("   "), Some("KSH statisztika"), "alap");
        assert!(u2.contains("www.google.com/search?q="));
        // egyik sincs -> alap lekérdezés
        let u3 = ai_suggestion_url(None, None, "alap lekérdezés");
        assert!(u3.contains("www.google.com/search?q="));
    }

    #[test]
    fn search_and_ai_clients_are_reused() {
        assert!(std::ptr::eq(search_client().unwrap(), search_client().unwrap()));
        assert!(std::ptr::eq(ai_client().unwrap(), ai_client().unwrap()));
    }

    #[test]
    fn map_powershell_spawn_error_explains_missing_powershell() {
        let missing = map_powershell_spawn_error(std::io::ErrorKind::NotFound, "not found");
        assert!(missing.contains("DPAPI") && missing.contains("PowerShell"));
        let other = map_powershell_spawn_error(std::io::ErrorKind::PermissionDenied, "boom");
        assert!(other.contains("boom"));
    }

    #[test]
    fn is_within_archive_allows_only_archive_files() {
        use std::fs;
        let base = std::env::temp_dir().join(format!("irbr_sec3_test_{}", std::process::id()));
        let archive = base.join("archive");
        fs::create_dir_all(&archive).unwrap();
        let db_path = base.join("irbr_database.db");
        fs::write(&db_path, b"x").unwrap();
        let inside = archive.join("src_1.txt");
        fs::write(&inside, b"a").unwrap();
        let outside = base.join("titok.txt");
        fs::write(&outside, b"b").unwrap();

        let db = db_path.to_string_lossy().into_owned();
        assert!(is_within_archive(&db, &inside.to_string_lossy()));
        assert!(!is_within_archive(&db, &outside.to_string_lossy()));
        let traversal = archive.join("..").join("titok.txt");
        assert!(!is_within_archive(&db, &traversal.to_string_lossy()));
        assert!(!is_within_archive(&db, &archive.join("nincs.txt").to_string_lossy()));

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn add_column_if_missing_adds_then_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE cases (id INTEGER PRIMARY KEY, title TEXT)", [])
            .unwrap();
        assert!(!table_has_column(&conn, "cases", "report_status").unwrap());
        add_column_if_missing(&conn, "cases", "report_status", "TEXT NOT NULL DEFAULT 'tervezet'")
            .unwrap();
        assert!(table_has_column(&conn, "cases", "report_status").unwrap());
        // második hívás no-op, hibamentes
        add_column_if_missing(&conn, "cases", "report_status", "TEXT NOT NULL DEFAULT 'tervezet'")
            .unwrap();
    }

    #[test]
    fn add_column_if_missing_propagates_real_errors() {
        let conn = Connection::open_in_memory().unwrap();
        // nem létező tábla -> valódi hiba, NEM elnyelve
        assert!(add_column_if_missing(&conn, "nem_letezo_tabla", "x", "TEXT").is_err());
    }

    #[test]
    fn migrate_schema_upgrades_legacy_db_and_preserves_data() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE cases (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT NOT NULL, description TEXT, deadline TEXT, status TEXT, created_at TEXT)",
            [],
        )
        .unwrap();
        conn.execute("CREATE TABLE sources (id INTEGER PRIMARY KEY, case_id INTEGER)", [])
            .unwrap();
        conn.execute("CREATE TABLE search_runs (id INTEGER PRIMARY KEY, case_id INTEGER)", [])
            .unwrap();
        conn.execute(
            "INSERT INTO cases (title, description) VALUES ('Teszt ügy', 'leiras')",
            [],
        )
        .unwrap();

        migrate_schema(&conn).unwrap();

        assert!(table_has_column(&conn, "cases", "report_status").unwrap());
        assert!(table_has_column(&conn, "sources", "missing_points").unwrap());
        assert!(table_has_column(&conn, "search_runs", "note").unwrap());
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        let title: String = conn
            .query_row("SELECT title FROM cases WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(title, "Teszt ügy");
    }

    #[test]
    fn migrate_schema_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE cases (id INTEGER PRIMARY KEY)", []).unwrap();
        conn.execute("CREATE TABLE sources (id INTEGER PRIMARY KEY)", []).unwrap();
        conn.execute("CREATE TABLE search_runs (id INTEGER PRIMARY KEY)", []).unwrap();
        migrate_schema(&conn).unwrap();
        migrate_schema(&conn).unwrap(); // no-op a user_version kapun
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
    }

    #[test]
    fn is_blocked_ip_blocks_internal_ranges() {
        for ip in [
            "127.0.0.1",
            "10.0.0.5",
            "192.168.1.10",
            "172.16.0.1",
            "169.254.169.254",
            "0.0.0.0",
            "::1",
            "fc00::1",
            "fe80::1",
            "::ffff:127.0.0.1",
        ] {
            assert!(
                is_blocked_ip(&ip.parse::<IpAddr>().unwrap()),
                "blokkolandó: {ip}"
            );
        }
    }

    #[test]
    fn is_blocked_ip_allows_public_addresses() {
        for ip in ["8.8.8.8", "1.1.1.1", "93.184.216.34", "2606:4700:4700::1111"] {
            assert!(
                !is_blocked_ip(&ip.parse::<IpAddr>().unwrap()),
                "engedélyezett: {ip}"
            );
        }
    }

    #[test]
    fn read_body_capped_enforces_limit() {
        use std::io::Cursor;
        let capped = read_body_capped(Cursor::new(vec![b'a'; 10]), 4).unwrap();
        assert_eq!(capped.len(), 4);
    }

    #[test]
    fn read_body_capped_returns_small_body_intact() {
        use std::io::Cursor;
        let data = vec![b'x'; 3];
        let capped = read_body_capped(Cursor::new(data.clone()), 100).unwrap();
        assert_eq!(capped, data);
    }

    #[test]
    fn normalize_for_match_strips_hungarian_accents() {
        assert_eq!(normalize_for_match("Óradíj MEZŐGAZDASÁGI bér"), "oradij mezogazdasagi ber");
    }

    #[test]
    fn query_variants_trigger_without_accents() {
        let variants = build_query_variants("mezogazdasagi gepi munka oradij");
        let labels: Vec<&str> = variants.iter().map(|(label, _)| *label).collect();
        assert!(labels.contains(&"piaci"));
        assert!(labels.contains(&"szakmai"));
    }

    #[test]
    fn query_variants_trigger_with_accents() {
        let variants = build_query_variants("mezőgazdasági gépi munka óradíj");
        let labels: Vec<&str> = variants.iter().map(|(label, _)| *label).collect();
        assert!(labels.contains(&"piaci"));
        assert!(labels.contains(&"szakmai"));
    }

    #[test]
    fn detect_archive_metadata_parses_wayback_snapshot() {
        let (original, archived, snapshot) =
            detect_archive_metadata("https://web.archive.org/web/20200101000000/https://example.com/page");
        assert_eq!(original, "https://example.com/page");
        assert_eq!(archived, "https://web.archive.org/web/20200101000000/https://example.com/page");
        assert_eq!(snapshot, "20200101000000");
    }

    #[test]
    fn detect_archive_metadata_passes_live_urls_through() {
        let (original, archived, snapshot) = detect_archive_metadata("https://example.com/page");
        assert_eq!(original, "https://example.com/page");
        assert!(archived.is_empty());
        assert!(snapshot.is_empty());
    }

    #[test]
    fn extract_site_targets_finds_domains() {
        let targets = extract_site_targets("óradíj (site:ksh.hu OR site:https://njt.hu)");
        assert_eq!(targets, vec!["ksh.hu".to_string(), "njt.hu".to_string()]);
    }

    #[test]
    fn extract_text_from_html_strips_scripts_and_tags() {
        let html = "<html><head><title>Cím &amp; teszt</title><style>p{color:red}</style></head>\
<body><script>var x = 1;</script><p>Első bekezdés.</p><p>Második&nbsp;rész.</p></body></html>";
        let (title, text) = extract_text_from_html(html);
        assert_eq!(title, "Cím & teszt");
        assert!(text.contains("Első bekezdés."));
        assert!(text.contains("Második rész."));
        assert!(!text.contains("var x"));
        assert!(!text.contains("color:red"));
    }

    #[test]
    fn brave_response_parses_expected_shape() {
        let raw = r#"{"web":{"results":[{"title":"Teszt cím","url":"https://www.ksh.hu/adat","description":"Kivonat szöveg."},{"title":"Másik","url":"https://example.com","description":"Leírás"}]},"query":{"original":"teszt"}}"#;
        let parsed: BraveResponse = serde_json::from_str(raw).expect("parse");
        let items = parsed.web.and_then(|web| web.results).unwrap_or_default();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].url.as_deref(), Some("https://www.ksh.hu/adat"));
        assert_eq!(items[0].title.as_deref(), Some("Teszt cím"));
    }

    #[test]
    fn split_query_for_domain_search_extracts_facebook_domains() {
        let (cleaned, domains) =
            split_query_for_domain_search("takarítás óradíj (site:facebook.com/groups OR site:facebook.com)");
        assert_eq!(cleaned, "takarítás óradíj");
        assert_eq!(domains, vec!["facebook.com".to_string()]);
    }

    #[test]
    fn split_query_for_domain_search_keeps_real_or_clauses() {
        let (cleaned, domains) = split_query_for_domain_search("óradíj OR órabér site:ksh.hu");
        assert_eq!(cleaned, "óradíj OR órabér");
        assert_eq!(domains, vec!["ksh.hu".to_string()]);
    }

    #[test]
    fn split_query_for_domain_search_passes_plain_queries_through() {
        let (cleaned, domains) = split_query_for_domain_search("mezőgazdasági gépi munka óradíj");
        assert_eq!(cleaned, "mezőgazdasági gépi munka óradíj");
        assert!(domains.is_empty());
    }

    #[test]
    fn tavily_response_parses_expected_shape() {
        let raw = r#"{"query":"teszt","results":[{"title":"Teszt cím","url":"https://njt.hu/jogszabaly","content":"Tartalom kivonat.","score":0.91}],"response_time":0.8}"#;
        let parsed: TavilyResponse = serde_json::from_str(raw).expect("parse");
        let items = parsed.results.unwrap_or_default();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].url.as_deref(), Some("https://njt.hu/jogszabaly"));
        assert_eq!(items[0].content.as_deref(), Some("Tartalom kivonat."));
    }

    #[test]
    fn classify_result_tier_recognizes_source_categories() {
        assert_eq!(classify_result_tier("https://njt.hu/jogszabaly/2020-1").0, 1);
        assert_eq!(classify_result_tier("https://www.ksh.hu/stadat").0, 1);
        assert_eq!(classify_result_tier("https://aki.gov.hu/kiadvany").0, 1);
        assert_eq!(classify_result_tier("https://scholar.google.com/scholar?q=x").0, 2);
        assert_eq!(classify_result_tier("https://web.archive.org/web/2020/https://x.hu").0, 2);
        assert_eq!(classify_result_tier("https://www.reddit.com/r/hungary").0, 4);
        assert_eq!(classify_result_tier("https://www.example.com/cikk").0, 3);
    }

    #[test]
    fn tier_filter_keeps_better_tiers() {
        let mut results = Vec::new();
        add_engine_launchers(&mut results, 0, "google", "teszt lekérdezés");
        add_curated_sources(&mut results, 0, "teszt lekérdezés", &[]);
        let generated = results.len();
        assert!(generated > 0);

        results.retain(|result| result.tier <= 2);
        assert!(results.iter().all(|result| result.tier <= 2));
        assert!(results.len() < generated);
    }
}
