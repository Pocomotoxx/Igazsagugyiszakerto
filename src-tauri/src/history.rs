//! Előzmények és rekordvizsgálat: szűrhető forráskeresés idővonallal, egy rekord
//! teljes vizsgálata (időpontok, integritás, kapcsolódó rekordok, auditesemények),
//! keresési futamok összehasonlítása és a forrás-állapot mátrix.

use crate::export::ExportRecord;
use crate::provenance::freshness_label;
use crate::story;
use chrono::Utc;
use rusqlite::{params, params_from_iter, types::ToSql, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

pub const MAX_PAGE_SIZE: i64 = 200;
const MAX_RELATED: usize = 10;
const MAX_DELTA_ITEMS: usize = 200;

#[derive(Deserialize, Debug, Default, Clone)]
pub struct HistoryFilters {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub tier: i32,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub decision: String,
    #[serde(default)]
    pub domain: String,
    #[serde(default)]
    pub archive_kind: String,
    /// YYYY-MM-DD, a begyűjtés napja szerint, a határokat is beleértve.
    #[serde(default)]
    pub date_from: String,
    #[serde(default)]
    pub date_to: String,
}

impl HistoryFilters {
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        if !self.text.trim().is_empty() {
            parts.push(format!("szöveg=\"{}\"", self.text.trim()));
        }
        if self.tier > 0 {
            parts.push(format!("tier<={}", self.tier));
        }
        for (name, value) in [
            ("státusz", &self.status),
            ("döntés", &self.decision),
            ("domain", &self.domain),
            ("típus", &self.archive_kind),
            ("-tól", &self.date_from),
            ("-ig", &self.date_to),
        ] {
            if !value.trim().is_empty() {
                parts.push(format!("{name}={}", value.trim()));
            }
        }
        if parts.is_empty() {
            "nincs szűrő".to_string()
        } else {
            parts.join("; ")
        }
    }
}

#[derive(Serialize, Debug)]
pub struct TimelinePoint {
    pub date: String,
    pub count: i64,
}

#[derive(Serialize, Debug)]
pub struct HistoryPage {
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub items: Vec<ExportRecord>,
    pub timeline: Vec<TimelinePoint>,
}

fn like_pattern(value: &str) -> String {
    let escaped = value
        .trim()
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

/// A WHERE-feltétel és a paraméterei. Minden felhasználói érték paraméter, nem SQL-szöveg.
fn build_where(case_id: i64, f: &HistoryFilters) -> (String, Vec<Box<dyn ToSql>>) {
    let mut clauses = vec!["case_id = ?".to_string()];
    let mut args: Vec<Box<dyn ToSql>> = vec![Box::new(case_id)];

    if f.tier > 0 {
        clauses.push("tier <= ?".to_string());
        args.push(Box::new(f.tier));
    }
    for (column, value) in [
        ("source_status", &f.status),
        ("usability_decision", &f.decision),
        ("archive_kind", &f.archive_kind),
    ] {
        if !value.trim().is_empty() {
            clauses.push(format!("{column} = ?"));
            args.push(Box::new(value.trim().to_string()));
        }
    }
    if !f.domain.trim().is_empty() {
        clauses.push("url LIKE ? ESCAPE '\\'".to_string());
        args.push(Box::new(like_pattern(&f.domain)));
    }
    if !f.date_from.trim().is_empty() {
        clauses.push("substr(timestamp, 1, 10) >= ?".to_string());
        args.push(Box::new(f.date_from.trim().to_string()));
    }
    if !f.date_to.trim().is_empty() {
        clauses.push("substr(timestamp, 1, 10) <= ?".to_string());
        args.push(Box::new(f.date_to.trim().to_string()));
    }
    (clauses.join(" AND "), args)
}

/// A Wayback 14 számjegyű időbélyege (ÉÉÉÉHHNNÓÓPPMM) RFC 3339 szöveggé; más érték üres.
pub fn observed_from_snapshot(snapshot: &str) -> String {
    let s = snapshot.trim();
    if s.len() == 14 && s.chars().all(|c| c.is_ascii_digit()) {
        format!(
            "{}-{}-{}T{}:{}:{}Z",
            &s[0..4],
            &s[4..6],
            &s[6..8],
            &s[8..10],
            &s[10..12],
            &s[12..14]
        )
    } else {
        String::new()
    }
}

const RECORD_COLUMNS: &str = "id, case_id, search_run_id, title, url, original_url, archived_url, archive_kind, tier, \
    source_status, usability_decision, hash, file_path, timestamp, snapshot_timestamp, published_at, modified_at, \
    time_source, reviewer_name, proof_summary, relevance_note, probative_value, missing_points";

fn row_to_record(row: &rusqlite::Row) -> rusqlite::Result<ExportRecord> {
    let tier: i32 = row.get::<_, Option<i32>>(8)?.unwrap_or(3);
    let published: String = row.get::<_, Option<String>>(15)?.unwrap_or_default();
    let snapshot: String = row.get::<_, Option<String>>(14)?.unwrap_or_default();
    Ok(ExportRecord {
        id: row.get(0)?,
        case_id: row.get::<_, Option<i64>>(1)?.unwrap_or_default(),
        search_run_id: row.get(2)?,
        title: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
        url: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
        original_url: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
        archived_url: row.get::<_, Option<String>>(6)?.unwrap_or_default(),
        archive_kind: row.get::<_, Option<String>>(7)?.unwrap_or_default(),
        tier,
        source_status: row.get::<_, Option<String>>(9)?.unwrap_or_default(),
        usability_decision: row.get::<_, Option<String>>(10)?.unwrap_or_default(),
        hash: row.get::<_, Option<String>>(11)?.unwrap_or_default(),
        file_path: row.get::<_, Option<String>>(12)?.unwrap_or_default(),
        collected_at: row.get::<_, Option<String>>(13)?.unwrap_or_default(),
        observed_at: observed_from_snapshot(&snapshot),
        freshness: freshness_label(tier, &published, Utc::now()).to_string(),
        published_at: published,
        modified_at: row.get::<_, Option<String>>(16)?.unwrap_or_default(),
        time_source: row.get::<_, Option<String>>(17)?.unwrap_or_default(),
        reviewer_name: row.get::<_, Option<String>>(18)?.unwrap_or_default(),
        proof_summary: row.get::<_, Option<String>>(19)?.unwrap_or_default(),
        relevance_note: row.get::<_, Option<String>>(20)?.unwrap_or_default(),
        probative_value: row.get::<_, Option<String>>(21)?.unwrap_or_default(),
        missing_points: row.get::<_, Option<String>>(22)?.unwrap_or_default(),
    })
}

/// A szűrőnek megfelelő rekordok azonosítója és begyűjtési ideje, a legújabb elöl.
/// A szövegszűrő Rust-oldalon fut (kis/nagybetű- és ékezetfüggetlen), mert az SQLite
/// `LIKE` csak az ASCII betűk kis/nagybetűit egyezteti.
fn matching_ids(
    conn: &Connection,
    case_id: i64,
    filters: &HistoryFilters,
) -> Result<Vec<(i64, String)>, String> {
    let (where_sql, args) = build_where(case_id, filters);
    let needle = story::fold(filters.text.trim());
    let mut out = Vec::new();

    if needle.is_empty() {
        let mut stmt = conn
            .prepare(&format!(
                "SELECT id, timestamp FROM sources WHERE {where_sql} ORDER BY timestamp DESC, id DESC"
            ))
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params_from_iter(args.iter().map(|a| a.as_ref())), |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?.unwrap_or_default()))
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            out.push(row.map_err(|e| e.to_string())?);
        }
        return Ok(out);
    }

    let mut stmt = conn
        .prepare(&format!(
            "SELECT id, timestamp, title, url, content, proof_summary FROM sources WHERE {where_sql} \
             ORDER BY timestamp DESC, id DESC"
        ))
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params_from_iter(args.iter().map(|a| a.as_ref())), |row| {
            let get = |i: usize| row.get::<_, Option<String>>(i).map(|v| v.unwrap_or_default());
            Ok((
                row.get::<_, i64>(0)?,
                get(1)?,
                format!("{} {} {} {}", get(2)?, get(3)?, get(4)?, get(5)?),
            ))
        })
        .map_err(|e| e.to_string())?;
    for row in rows {
        let (id, timestamp, haystack) = row.map_err(|e| e.to_string())?;
        if story::fold(&haystack).contains(&needle) {
            out.push((id, timestamp));
        }
    }
    Ok(out)
}

fn records_by_ids(conn: &Connection, ids: &[(i64, String)]) -> Result<Vec<ExportRecord>, String> {
    let mut stmt = conn
        .prepare(&format!("SELECT {RECORD_COLUMNS} FROM sources WHERE id = ?1"))
        .map_err(|e| e.to_string())?;
    let mut out = Vec::with_capacity(ids.len());
    for (id, _) in ids {
        out.push(stmt.query_row(params![id], row_to_record).map_err(|e| e.to_string())?);
    }
    Ok(out)
}

fn slice(ids: &[(i64, String)], limit: i64, offset: i64) -> &[(i64, String)] {
    let start = (offset.max(0) as usize).min(ids.len());
    let end = (start + limit.max(0) as usize).min(ids.len());
    &ids[start..end]
}

/// Szűrt rekordok (lapozva, a legújabb elöl). `limit`/`offset` a hívó dolga.
pub fn query_records(
    conn: &Connection,
    case_id: i64,
    filters: &HistoryFilters,
    limit: i64,
    offset: i64,
) -> Result<Vec<ExportRecord>, String> {
    let ids = matching_ids(conn, case_id, filters)?;
    records_by_ids(conn, slice(&ids, limit, offset))
}

pub fn count_records(conn: &Connection, case_id: i64, filters: &HistoryFilters) -> Result<i64, String> {
    Ok(matching_ids(conn, case_id, filters)?.len() as i64)
}

pub fn search(
    conn: &Connection,
    case_id: i64,
    filters: &HistoryFilters,
    page: i64,
    page_size: i64,
) -> Result<HistoryPage, String> {
    let page = page.max(0);
    let page_size = page_size.clamp(1, MAX_PAGE_SIZE);
    let ids = matching_ids(conn, case_id, filters)?;
    let items = records_by_ids(conn, slice(&ids, page_size, page * page_size))?;

    let mut days: BTreeMap<String, i64> = BTreeMap::new();
    for (_, timestamp) in &ids {
        let day: String = timestamp.chars().take(10).collect();
        if !day.is_empty() {
            *days.entry(day).or_insert(0) += 1;
        }
    }
    let timeline = days
        .into_iter()
        .map(|(date, count)| TimelinePoint { date, count })
        .collect();

    Ok(HistoryPage {
        total: ids.len() as i64,
        page,
        page_size,
        items,
        timeline,
    })
}

// ---- Rekordvizsgálat ----

#[derive(Serialize, Debug)]
pub struct Integrity {
    /// "ok", "eltér", "hiányzik" vagy "nincs hash"
    pub state: String,
    pub stored_hash: String,
    pub computed_hash: String,
    pub detail: String,
}

#[derive(Serialize, Debug)]
pub struct RelatedRecord {
    pub id: i64,
    pub title: String,
    pub url: String,
    pub reason: String,
}

#[derive(Serialize, Debug)]
pub struct AuditEntry {
    pub timestamp: String,
    pub action_type: String,
    pub details: String,
}

#[derive(Serialize, Debug)]
pub struct Inspection {
    pub record: ExportRecord,
    pub integrity: Integrity,
    pub related: Vec<RelatedRecord>,
    pub audit: Vec<AuditEntry>,
    pub content_excerpt: String,
}

pub fn domain_of(url: &str) -> String {
    url.split("://")
        .nth(1)
        .unwrap_or(url)
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .trim_start_matches("www.")
        .to_ascii_lowercase()
}

fn check_integrity(record: &ExportRecord, archive_dir: &Path) -> Integrity {
    let stored = record.hash.trim().to_string();
    if stored.is_empty() {
        return Integrity {
            state: "nincs hash".into(),
            stored_hash: String::new(),
            computed_hash: String::new(),
            detail: "Ehhez a rekordhoz nem tartozik rögzített hash.".into(),
        };
    }
    let path = Path::new(&record.file_path);
    let inside = match (path.canonicalize(), archive_dir.canonicalize()) {
        (Ok(p), Ok(a)) => p.starts_with(a),
        _ => false,
    };
    if record.file_path.trim().is_empty() || !path.exists() || !inside {
        return Integrity {
            state: "hiányzik".into(),
            stored_hash: stored,
            computed_hash: String::new(),
            detail: "Az archivált fájl nem található az archívum mappában.".into(),
        };
    }
    match std::fs::read(path) {
        Ok(bytes) => {
            let computed = hex::encode(Sha256::digest(&bytes));
            let ok = computed.eq_ignore_ascii_case(&stored);
            Integrity {
                state: if ok { "ok" } else { "eltér" }.into(),
                stored_hash: stored,
                computed_hash: computed,
                detail: if ok {
                    "A fájl SHA-256 értéke megegyezik a rögzítettel.".into()
                } else {
                    "FIGYELEM: a fájl tartalma nem egyezik a rögzített hash-sel.".into()
                },
            }
        }
        Err(e) => Integrity {
            state: "hiányzik".into(),
            stored_hash: stored,
            computed_hash: String::new(),
            detail: format!("A fájl nem olvasható: {e}"),
        },
    }
}

pub fn inspect(conn: &Connection, source_id: i64, archive_dir: &Path) -> Result<Inspection, String> {
    let record = conn
        .query_row(
            &format!("SELECT {RECORD_COLUMNS} FROM sources WHERE id = ?1"),
            params![source_id],
            row_to_record,
        )
        .map_err(|e| format!("A forrás nem található: {e}"))?;

    let excerpt: String = conn
        .query_row("SELECT content FROM sources WHERE id = ?1", params![source_id], |row| {
            row.get::<_, Option<String>>(0)
        })
        .map_err(|e| e.to_string())?
        .unwrap_or_default()
        .chars()
        .take(1500)
        .collect();

    let integrity = check_integrity(&record, archive_dir);

    // kapcsolódó rekordok
    let siblings = query_records(
        conn,
        record.case_id,
        &HistoryFilters::default(),
        MAX_PAGE_SIZE * 5,
        0,
    )?;
    let mut related: Vec<RelatedRecord> = Vec::new();
    let mut seen: HashMap<i64, usize> = HashMap::new();
    let mut push = |other: &ExportRecord, reason: &str, related: &mut Vec<RelatedRecord>| {
        if other.id == record.id || related.len() >= MAX_RELATED * 3 {
            return;
        }
        if let Some(index) = seen.get(&other.id) {
            if !related[*index].reason.contains(reason) {
                related[*index].reason = format!("{}, {reason}", related[*index].reason);
            }
            return;
        }
        seen.insert(other.id, related.len());
        related.push(RelatedRecord {
            id: other.id,
            title: other.title.clone(),
            url: other.url.clone(),
            reason: reason.to_string(),
        });
    };

    for other in &siblings {
        if !record.hash.is_empty() && other.hash == record.hash {
            push(other, "azonos tartalom (hash)", &mut related);
        }
    }
    let story_texts: Vec<String> = siblings
        .iter()
        .map(|s| format!("{} {}", s.title, s.proof_summary))
        .collect();
    if let Some(self_index) = siblings.iter().position(|s| s.id == record.id) {
        let ids = story::cluster(&story_texts);
        for (index, other) in siblings.iter().enumerate() {
            if index != self_index && ids[index] == ids[self_index] {
                push(other, "azonos történet", &mut related);
            }
        }
    }
    let domain = domain_of(&record.url);
    if !domain.is_empty() {
        for other in &siblings {
            if domain_of(&other.url) == domain {
                push(other, "azonos domain", &mut related);
            }
        }
    }
    if let Some(run) = record.search_run_id {
        for other in &siblings {
            if other.search_run_id == Some(run) {
                push(other, "azonos keresési futam", &mut related);
            }
        }
    }
    related.truncate(MAX_RELATED * 3);

    let mut stmt = conn
        .prepare(
            "SELECT timestamp, action_type, details FROM audit_logs \
             WHERE case_id = ?1 AND (query = ?2 OR query = ?3 OR details LIKE ?4 ESCAPE '\\') \
             ORDER BY id DESC LIMIT 30",
        )
        .map_err(|e| e.to_string())?;
    let audit = stmt
        .query_map(
            params![record.case_id, record.url, record.original_url, like_pattern(&record.url)],
            |row| {
                Ok(AuditEntry {
                    timestamp: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    action_type: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    details: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                })
            },
        )
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(Inspection {
        record,
        integrity,
        related,
        audit,
        content_excerpt: excerpt,
    })
}

// ---- Keresési futamok összehasonlítása ----

#[derive(Deserialize, Debug, Clone)]
pub struct RunResultInput {
    pub url: String,
    pub title: String,
    pub tier: i32,
    #[serde(default)]
    pub source_type: String,
    #[serde(default)]
    pub query_used: String,
    #[serde(default)]
    pub published_at: String,
    #[serde(default)]
    pub evidence_level: String,
    #[serde(default)]
    pub story_id: i64,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct RunItem {
    pub url: String,
    pub title: String,
    pub tier: i32,
    pub evidence_level: String,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct TierChange {
    pub url: String,
    pub title: String,
    pub from: i32,
    pub to: i32,
}

#[derive(Serialize, Debug)]
pub struct RunDelta {
    pub run_a: i64,
    pub run_b: i64,
    pub new_items: Vec<RunItem>,
    pub dropped_items: Vec<RunItem>,
    pub kept_count: usize,
    pub tier_changes: Vec<TierChange>,
    pub new_total: usize,
    pub dropped_total: usize,
}

/// Összehasonlításra alkalmas URL-kulcs: kisbetűs host `www.` nélkül, nyomkövető
/// paraméterek és töredék nélkül, záró perjel nélkül.
pub fn url_key(url: &str) -> String {
    let url = url.trim();
    let without_fragment = url.split('#').next().unwrap_or(url);
    let (base, query) = match without_fragment.split_once('?') {
        Some((b, q)) => (b, q),
        None => (without_fragment, ""),
    };
    let (scheme_less, _) = match base.split_once("://") {
        Some((_, rest)) => (rest, ()),
        None => (base, ()),
    };
    let (host, path) = match scheme_less.split_once('/') {
        Some((h, p)) => (h, format!("/{p}")),
        None => (scheme_less, String::new()),
    };
    let host = host.trim_start_matches("www.").to_ascii_lowercase();
    let path = path.trim_end_matches('/').to_string();
    let mut kept: Vec<&str> = query
        .split('&')
        .filter(|pair| {
            let name = pair.split('=').next().unwrap_or("").to_ascii_lowercase();
            !pair.is_empty()
                && !name.starts_with("utm_")
                && !matches!(name.as_str(), "fbclid" | "gclid" | "mc_cid" | "mc_eid" | "ref" | "igshid")
        })
        .collect();
    kept.sort_unstable();
    if kept.is_empty() {
        format!("{host}{path}")
    } else {
        format!("{host}{path}?{}", kept.join("&"))
    }
}

pub fn save_run_results(conn: &Connection, run_id: i64, results: &[RunResultInput]) -> Result<(), String> {
    let mut stmt = conn
        .prepare(
            "INSERT INTO run_results (run_id, url_key, url, title, tier, source_type, query_used, published_at, evidence_level, story_id) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )
        .map_err(|e| e.to_string())?;
    let mut seen = std::collections::HashSet::new();
    for item in results.iter().take(2000) {
        let key = url_key(&item.url);
        if key.is_empty() || !seen.insert(key.clone()) {
            continue;
        }
        stmt.execute(params![
            run_id,
            key,
            item.url,
            item.title,
            item.tier,
            item.source_type,
            item.query_used,
            item.published_at,
            item.evidence_level,
            item.story_id,
        ])
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn load_run(conn: &Connection, run_id: i64) -> Result<BTreeMap<String, RunItem>, String> {
    let mut stmt = conn
        .prepare("SELECT url_key, url, title, tier, evidence_level FROM run_results WHERE run_id = ?1")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![run_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                RunItem {
                    url: row.get(1)?,
                    title: row.get(2)?,
                    tier: row.get(3)?,
                    evidence_level: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                },
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut map = BTreeMap::new();
    for row in rows {
        let (key, item) = row.map_err(|e| e.to_string())?;
        map.insert(key, item);
    }
    Ok(map)
}

/// `run_a` a régebbi (alap), `run_b` az újabb. Új = csak B-ben van; kiesett = csak A-ban.
pub fn compare_runs(conn: &Connection, run_a: i64, run_b: i64) -> Result<RunDelta, String> {
    let a = load_run(conn, run_a)?;
    let b = load_run(conn, run_b)?;
    let mut new_items = Vec::new();
    let mut dropped_items = Vec::new();
    let mut tier_changes = Vec::new();
    let mut kept = 0;
    for (key, item) in &b {
        match a.get(key) {
            None => new_items.push(item.clone()),
            Some(old) => {
                kept += 1;
                if old.tier != item.tier {
                    tier_changes.push(TierChange {
                        url: item.url.clone(),
                        title: item.title.clone(),
                        from: old.tier,
                        to: item.tier,
                    });
                }
            }
        }
    }
    for (key, item) in &a {
        if !b.contains_key(key) {
            dropped_items.push(item.clone());
        }
    }
    let new_total = new_items.len();
    let dropped_total = dropped_items.len();
    new_items.truncate(MAX_DELTA_ITEMS);
    dropped_items.truncate(MAX_DELTA_ITEMS);
    Ok(RunDelta {
        run_a,
        run_b,
        new_items,
        dropped_items,
        kept_count: kept,
        tier_changes,
        new_total,
        dropped_total,
    })
}

// ---- Forrás-állapot mátrix ----

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE sources (id INTEGER PRIMARY KEY AUTOINCREMENT, case_id INTEGER, search_run_id INTEGER, title TEXT, url TEXT, \
               original_url TEXT, archived_url TEXT, archive_kind TEXT, tier INTEGER, content TEXT, file_path TEXT, hash TEXT, timestamp TEXT, \
               snapshot_timestamp TEXT, source_status TEXT, usability_decision TEXT, proof_summary TEXT, reviewer_name TEXT, \
               relevance_note TEXT, probative_value TEXT, missing_points TEXT, published_at TEXT, modified_at TEXT, time_source TEXT);
             CREATE TABLE audit_logs (id INTEGER PRIMARY KEY AUTOINCREMENT, case_id INTEGER, timestamp TEXT, action_type TEXT, query TEXT, details TEXT);
             CREATE TABLE search_runs (id INTEGER PRIMARY KEY AUTOINCREMENT, case_id INTEGER, created_at TEXT, summary TEXT, base_query TEXT, health TEXT);
             CREATE TABLE run_results (id INTEGER PRIMARY KEY AUTOINCREMENT, run_id INTEGER, url_key TEXT, url TEXT, title TEXT, tier INTEGER, \
               source_type TEXT, query_used TEXT, published_at TEXT, evidence_level TEXT, story_id INTEGER);",
        )
        .unwrap();
        conn
    }

    fn add_source(conn: &Connection, case: i64, title: &str, url: &str, tier: i32, ts: &str, hash: &str) {
        conn.execute(
            "INSERT INTO sources (case_id, title, url, original_url, archived_url, archive_kind, tier, content, file_path, hash, timestamp, \
              snapshot_timestamp, source_status, usability_decision, proof_summary, published_at) \
             VALUES (?1,?2,?3,'','','live',?4,'tartalom',?5,?6,?7,'','nyers','hatteranyag','','')",
            params![case, title, url, tier, "", hash, ts],
        )
        .unwrap();
    }

    #[test]
    fn url_key_drops_tracking_and_normalizes() {
        assert_eq!(
            url_key("https://www.Example.hu/a/?utm_source=x&b=2&a=1#frag"),
            "example.hu/a?a=1&b=2"
        );
        assert_eq!(url_key("http://example.hu/a"), "example.hu/a");
        assert_eq!(url_key("https://example.hu"), "example.hu");
        assert_eq!(url_key("https://example.hu/?fbclid=zzz"), "example.hu");
    }

    #[test]
    fn filters_and_pagination_and_timeline() {
        let c = conn();
        add_source(&c, 1, "Óradíj rendelet", "https://njt.hu/a", 1, "2026-05-01 10:00:00", "h1");
        add_source(&c, 1, "Egyéb cikk", "https://blog.hu/x", 3, "2026-05-02 10:00:00", "h2");
        add_source(&c, 1, "Óradíj hír", "https://telex.hu/y", 3, "2026-05-02 11:00:00", "h3");
        add_source(&c, 2, "Másik ügy", "https://njt.hu/z", 1, "2026-05-02 11:00:00", "h4");

        let all = search(&c, 1, &HistoryFilters::default(), 0, 50).unwrap();
        assert_eq!(all.total, 3);
        assert_eq!(all.timeline.len(), 2);
        assert_eq!(all.timeline[1].count, 2);

        let f = HistoryFilters { text: "óradíj".into(), ..Default::default() };
        assert_eq!(search(&c, 1, &f, 0, 50).unwrap().total, 2);
        // kis/nagybetű- és ékezetfüggetlen
        let f = HistoryFilters { text: "ORADIJ".into(), ..Default::default() };
        assert_eq!(search(&c, 1, &f, 0, 50).unwrap().total, 2);

        let f = HistoryFilters { tier: 1, ..Default::default() };
        assert_eq!(search(&c, 1, &f, 0, 50).unwrap().total, 1);

        let f = HistoryFilters { domain: "telex.hu".into(), ..Default::default() };
        assert_eq!(search(&c, 1, &f, 0, 50).unwrap().items[0].title, "Óradíj hír");

        let f = HistoryFilters { date_from: "2026-05-02".into(), ..Default::default() };
        assert_eq!(search(&c, 1, &f, 0, 50).unwrap().total, 2);

        let page = search(&c, 1, &HistoryFilters::default(), 1, 2).unwrap();
        assert_eq!(page.items.len(), 1);
    }

    #[test]
    fn like_wildcards_are_escaped() {
        let c = conn();
        add_source(&c, 1, "100% igaz", "https://a.hu/", 3, "2026-05-01 10:00:00", "h1");
        add_source(&c, 1, "teljesen más", "https://b.hu/", 3, "2026-05-01 10:00:00", "h2");
        let f = HistoryFilters { text: "%".into(), ..Default::default() };
        assert_eq!(search(&c, 1, &f, 0, 50).unwrap().total, 1);
        let f = HistoryFilters { domain: "_".into(), ..Default::default() };
        assert_eq!(search(&c, 1, &f, 0, 50).unwrap().total, 0);
    }

    #[test]
    fn inspect_detects_hash_mismatch_and_related() {
        let dir = std::env::temp_dir().join(format!("irbr_inspect_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("src.txt");
        std::fs::write(&file, "tartalom").unwrap();
        let good = hex::encode(Sha256::digest(b"tartalom"));

        let c = conn();
        add_source(&c, 1, "Egy", "https://a.hu/1", 3, "2026-05-01 10:00:00", &good);
        add_source(&c, 1, "Kettő", "https://a.hu/2", 3, "2026-05-01 11:00:00", &good);
        c.execute("UPDATE sources SET file_path = ?1 WHERE id = 1", params![file.to_string_lossy()]).unwrap();

        let ok = inspect(&c, 1, &dir).unwrap();
        assert_eq!(ok.integrity.state, "ok");
        assert!(ok.related.iter().any(|r| r.id == 2 && r.reason.contains("hash")));

        std::fs::write(&file, "megváltoztatva").unwrap();
        assert_eq!(inspect(&c, 1, &dir).unwrap().integrity.state, "eltér");

        // az archívum mappán kívüli fájl nem számít
        let other = std::env::temp_dir().join(format!("irbr_outside_{}", std::process::id()));
        std::fs::create_dir_all(&other).unwrap();
        assert_eq!(inspect(&c, 1, &other).unwrap().integrity.state, "hiányzik");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&other);
    }

    #[test]
    fn compare_runs_reports_new_dropped_and_tier_change() {
        let c = conn();
        let item = |url: &str, tier: i32| RunResultInput {
            url: url.into(),
            title: url.into(),
            tier,
            source_type: String::new(),
            query_used: String::new(),
            published_at: String::new(),
            evidence_level: "RETRIEVED".into(),
            story_id: 0,
        };
        save_run_results(&c, 1, &[item("https://a.hu/1", 3), item("https://a.hu/2", 3), item("https://www.a.hu/3/?utm_x=1", 2)]).unwrap();
        save_run_results(&c, 2, &[item("https://a.hu/1", 3), item("https://a.hu/3", 1), item("https://a.hu/4", 3)]).unwrap();
        let delta = compare_runs(&c, 1, 2).unwrap();
        assert_eq!(delta.kept_count, 2);
        assert_eq!(delta.new_total, 1);
        assert_eq!(delta.dropped_total, 1);
        assert_eq!(delta.tier_changes.len(), 1);
        assert_eq!(delta.tier_changes[0].from, 2);
        assert_eq!(delta.tier_changes[0].to, 1);
    }

    #[test]
    fn snapshot_conversion() {
        assert_eq!(observed_from_snapshot("20240102030405"), "2024-01-02T03:04:05Z");
        assert_eq!(observed_from_snapshot("abc"), "");
    }
}
