use chrono::Local;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use tauri::{Emitter, Manager};

mod export;
mod history;
mod platforms;
mod provenance;
mod story;

/// A megszakított sentinel-navigáció hosztja: így jut vissza az oldal szövege IPC nélkül.
const SENTINEL_HOST: &str = "irbr-scrape.invalid";

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                if let Ok(v) = u8::from_str_radix(&input[i + 1..i + 3], 16) {
                    out.push(v);
                    i += 3;
                    continue;
                }
                out.push(b'%');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

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
        "CREATE TABLE IF NOT EXISTS run_results (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id INTEGER NOT NULL,
            url_key TEXT NOT NULL,
            url TEXT NOT NULL,
            title TEXT NOT NULL DEFAULT '',
            tier INTEGER NOT NULL DEFAULT 3,
            source_type TEXT NOT NULL DEFAULT '',
            query_used TEXT NOT NULL DEFAULT '',
            published_at TEXT NOT NULL DEFAULT '',
            evidence_level TEXT NOT NULL DEFAULT '',
            story_id INTEGER NOT NULL DEFAULT 0
        )",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_run_results_run_id ON run_results(run_id)",
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

const SCHEMA_VERSION: i64 = 2;

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

    // v2: szolgáltatói idők, forrás-állapot mátrix
    for (col, def) in [
        ("published_at", "TEXT NOT NULL DEFAULT ''"),
        ("modified_at", "TEXT NOT NULL DEFAULT ''"),
        ("time_source", "TEXT NOT NULL DEFAULT ''"),
    ] {
        add_column_if_missing(conn, "sources", col, def)?;
    }
    add_column_if_missing(conn, "search_runs", "health", "TEXT NOT NULL DEFAULT '[]'")?;

    conn.pragma_update(None, "user_version", SCHEMA_VERSION)
        .map_err(|e| e.to_string())?;
    Ok(())
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

#[derive(Serialize)]
struct PlatformInfo {
    id: &'static str,
    label: &'static str,
    kind: &'static str,
    domains: Vec<&'static str>,
}

#[tauri::command]
fn list_platforms() -> Vec<PlatformInfo> {
    platforms::PLATFORMS
        .iter()
        .map(|p| PlatformInfo {
            id: p.id,
            label: p.label,
            kind: match p.kind {
                platforms::Kind::Engine => "kereső",
                platforms::Kind::Site => "domain",
            },
            domains: p.domains.to_vec(),
        })
        .collect()
}

#[tauri::command]
fn init_db(state: tauri::State<'_, DbState>) -> Result<String, String> {
    let conn = get_connection(&state.db_path)?;
    init_schema(&conn)?;
    Ok("Adatbázis sikeresen inicializálva.".to_string())
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
    results: Option<Vec<history::RunResultInput>>,
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
    if let Some(results) = results {
        history::save_run_results(&conn, new_id, &results)?;
    }
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
fn archive_source(
    state: tauri::State<'_, DbState>,
    case_id: i64,
    search_run_id: Option<i64>,
    title: String,
    url: String,
    tier: i32,
    content: String,
    published_at: Option<String>,
    modified_at: Option<String>,
    time_source: Option<String>,
) -> Result<Source, String> {
    let now_utc = chrono::Utc::now();
    let published_at = published_at
        .and_then(|v| provenance::normalize_time(&v, now_utc))
        .unwrap_or_default();
    let modified_at = modified_at
        .and_then(|v| provenance::normalize_time(&v, now_utc))
        .unwrap_or_default();
    let time_source = if published_at.is_empty() && modified_at.is_empty() {
        String::new()
    } else {
        time_source
            .map(|v| v.chars().take(20).collect::<String>())
            .unwrap_or_default()
    };
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
        "INSERT INTO sources (case_id, search_run_id, title, url, original_url, archived_url, snapshot_timestamp, archive_kind, tier, content, file_path, hash, timestamp, published_at, modified_at, time_source)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
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
            timestamp,
            published_at,
            modified_at,
            time_source
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

    // `get_webview_window` csak az egy webviewos ablakot adja vissza; az első böngészőfül után
    // a főablaknak több webviewja van, ezért az ablakot közvetlenül kérjük el.
    let window = app
        .get_window("main")
        .ok_or_else(|| "A főablak nem található.".to_string())?;

    let nav_app = app.clone();
    let nav_label = label.clone();
    let load_app = app.clone();
    let load_label = label.clone();
    let title_app = app.clone();
    let title_label = label.clone();

    let builder = tauri::webview::WebviewBuilder::new(&label, tauri::WebviewUrl::External(parsed))
        .on_navigation(move |target| {
            // az élő oldal szövegének visszajuttatása (archiváláshoz): megszakított sentinel-navigáció
            if target.host_str() == Some(SENTINEL_HOST) {
                if let Some(fragment) = target.fragment() {
                    let _ = nav_app.emit(
                        "browser-page-text",
                        serde_json::json!({
                            "label": nav_label.clone(),
                            "payload": percent_decode(fragment),
                        }),
                    );
                }
                return false;
            }
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

/// Kéri az aktív oldal megjelenített szövegét; az eredmény a `browser-page-text` eseményben érkezik.
#[tauri::command]
async fn browser_capture_text(app: tauri::AppHandle, tab_id: String) -> Result<(), String> {
    let label = browser_label(&tab_id)?;
    let webview = app
        .get_webview(&label)
        .ok_or_else(|| "A böngészőfül nem található.".to_string())?;
    let script = r#"(function () {
  function meta(names) {
    for (var i = 0; i < names.length; i++) {
      var el = document.querySelector('meta[property="' + names[i] + '"], meta[name="' + names[i] + '"], meta[itemprop="' + names[i] + '"]');
      if (el && el.content) return el.content;
    }
    var t = document.querySelector("time[datetime]");
    return names[0].indexOf("published") >= 0 && t ? t.getAttribute("datetime") : "";
  }
  var data = {
    title: document.title || "",
    url: location.href,
    text: ((document.body && document.body.innerText) || "").replace(/[ \t]+/g, " ").replace(/\n{3,}/g, "\n\n").slice(0, 20000),
    published: meta(["article:published_time", "datePublished", "og:published_time", "date"]),
    modified: meta(["article:modified_time", "dateModified", "og:updated_time"]) || document.lastModified || ""
  };
  location.href = "https://irbr-scrape.invalid/#" + encodeURIComponent(JSON.stringify(data));
})();"#;
    webview.eval(script).map_err(|e| e.to_string())
}

#[tauri::command]
fn browser_tab_links(query: String, enabled_platforms: Option<Vec<String>>) -> Vec<platforms::PlatformLink> {
    platforms::tab_links(query.trim(), &enabled_platforms.unwrap_or_default())
}

#[tauri::command]
async fn browser_close_tab(app: tauri::AppHandle, tab_id: String) -> Result<(), String> {
    let label = browser_label(&tab_id)?;
    if let Some(webview) = app.get_webview(&label) {
        webview.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn search_history(
    state: tauri::State<'_, DbState>,
    case_id: i64,
    filters: Option<history::HistoryFilters>,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<history::HistoryPage, String> {
    let conn = get_connection(&state.db_path)?;
    history::search(
        &conn,
        case_id,
        &filters.unwrap_or_default(),
        page.unwrap_or(0),
        page_size.unwrap_or(25),
    )
}

#[tauri::command]
fn inspect_source(
    state: tauri::State<'_, DbState>,
    source_id: i64,
) -> Result<history::Inspection, String> {
    let conn = get_connection(&state.db_path)?;
    let inspection = history::inspect(&conn, source_id, &archive_dir_for(&state.db_path))?;
    log_action(
        &state.db_path,
        inspection.record.case_id,
        "source_inspected",
        &inspection.record.url,
        &format!("Rekordvizsgálat. Integritás: {}.", inspection.integrity.state),
    )?;
    Ok(inspection)
}

#[tauri::command]
fn export_sources(
    state: tauri::State<'_, DbState>,
    case_id: i64,
    format: String,
    filters: Option<history::HistoryFilters>,
) -> Result<String, String> {
    let conn = get_connection(&state.db_path)?;
    let filters = filters.unwrap_or_default();
    let total = history::count_records(&conn, case_id, &filters)?;
    let records = history::query_records(&conn, case_id, &filters, export::EXPORT_CAP as i64, 0)?;
    let case_title: String = conn
        .query_row("SELECT title FROM cases WHERE id = ?1", params![case_id], |row| row.get(0))
        .map_err(|e| format!("Az ügy nem található: {e}"))?;
    let metadata = export::ExportMetadata {
        generated_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        case_title,
        total: total as usize,
        exported: records.len(),
        truncated: (total as usize) > records.len(),
        filters: filters.describe(),
    };
    let (content, extension) = export::render(format.trim(), &records, &metadata)?;

    let dir = Path::new(&state.db_path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("exports");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file_path = dir.join(format!(
        "irbr_export_{case_id}_{}.{extension}",
        Local::now().format("%Y%m%d_%H%M%S_%3f")
    ));
    fs::write(&file_path, content).map_err(|e| e.to_string())?;
    let path_str = file_path.to_string_lossy().into_owned();

    log_action(
        &state.db_path,
        case_id,
        "sources_exported",
        &path_str,
        &format!(
            "Forrásexport ({}): {} rekord. Szűrők: {}.",
            format.trim(),
            records.len(),
            metadata.filters
        ),
    )?;
    Ok(path_str)
}

#[tauri::command]
fn compare_search_runs(
    state: tauri::State<'_, DbState>,
    run_a: i64,
    run_b: i64,
) -> Result<history::RunDelta, String> {
    let conn = get_connection(&state.db_path)?;
    history::compare_runs(&conn, run_a, run_b)
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
            get_cases,
            create_case,
            update_case_opinions,
            delete_case,
            get_sources,
            delete_source,
            get_audit_logs,
            get_search_runs,
            record_search_run,
            update_source_review,
            archive_source,
            browser_open_tab,
            browser_set_bounds,
            browser_set_visible,
            browser_hide_all,
            browser_navigate,
            browser_action,
            browser_close_tab,
            browser_capture_text,
            browser_tab_links,
            search_history,
            list_platforms,
            inspect_source,
            export_sources,
            compare_search_runs,
            save_report
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn migrate_adds_v2_columns_to_a_v1_database() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        // v1 állapot szimulálása
        conn.pragma_update(None, "user_version", 1).unwrap();
        migrate_schema(&conn).unwrap();
        assert!(table_has_column(&conn, "sources", "published_at").unwrap());
        assert!(table_has_column(&conn, "sources", "time_source").unwrap());
        assert!(table_has_column(&conn, "search_runs", "health").unwrap());
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM run_results", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0);
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

}
