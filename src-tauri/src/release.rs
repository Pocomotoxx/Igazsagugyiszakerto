//! Véglegesítés és kiadás. A szabályokat a háttérrendszer érvényesíti (a felület csak előzetesen
//! jelez): véglegesített állapot és végleges kiadás csak teljesülő feltételek mellett jöhet létre.
//! A kiadás egy változtathatatlan, verziózott pillanatkép a szakértő saját jóváhagyásával: az
//! ügyállapot, a felhasznált mellékletek és az értékelések pontos változatával.

use crate::history::{check_file, Integrity};
use crate::integrity;
use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

pub struct Opinion<'a> {
    pub final_opinion: &'a str,
    pub finalizable: &'a str,
    pub open_questions: &'a str,
}

struct SourceRow {
    id: i64,
    title: String,
    decision: String,
    status: String,
    proof_summary: String,
    reviewer: String,
    reviewed_at: String,
    file_path: String,
    hash: String,
}

fn load_sources(conn: &Connection, case_id: i64) -> Result<Vec<SourceRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, title, usability_decision, source_status, proof_summary, reviewer_name, reviewed_at, file_path, hash
             FROM sources WHERE case_id = ?1 ORDER BY id",
        )
        .map_err(|e| e.to_string())?;
    let text = |row: &rusqlite::Row, i: usize| row.get::<_, Option<String>>(i).map(|v| v.unwrap_or_default());
    let rows = stmt
        .query_map(params![case_id], |row| {
            Ok(SourceRow {
                id: row.get(0)?,
                title: text(row, 1)?,
                decision: text(row, 2)?,
                status: text(row, 3)?,
                proof_summary: text(row, 4)?,
                reviewer: text(row, 5)?,
                reviewed_at: text(row, 6)?,
                file_path: text(row, 7)?,
                hash: text(row, 8)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

/// A véglegesítés akadályai (üres lista = véglegesíthető). `check_files`: az archivált fájlok
/// hash-ének visszaellenőrzése is (a végleges kiadáskor és a véglegesített állapot mentésekor).
pub fn finalization_issues(
    conn: &Connection,
    archive_dir: &Path,
    case_id: i64,
    opinion: &Opinion,
    check_files: bool,
) -> Result<Vec<String>, String> {
    integrity::ensure_case_exists(conn, case_id)?;
    let sources = load_sources(conn, case_id)?;
    let mut issues = Vec::new();

    if sources.is_empty() {
        issues.push("Nincs archivált forrás az ügyhöz.".to_string());
    }
    if !sources.iter().any(|s| s.decision == "felhasznalhato") {
        issues.push("Nincs felhasználhatónak jelölt forrás, a konklúziónak nincs alátámasztása.".to_string());
    }
    let missing_proof = sources
        .iter()
        .filter(|s| s.decision == "felhasznalhato" && s.proof_summary.trim().is_empty())
        .count();
    if missing_proof > 0 {
        issues.push(format!("Hiányzó bizonyítási kivonat {missing_proof} felhasználható forrásnál."));
    }
    let unreviewed = sources
        .iter()
        .filter(|s| s.status == "validalt" && (s.reviewer.trim().is_empty() || s.reviewed_at.trim().is_empty()))
        .count();
    if unreviewed > 0 {
        issues.push(format!("Hiányos validálási metaadat {unreviewed} validált forrásnál."));
    }
    if opinion.final_opinion.trim().is_empty() {
        issues.push("Nincs végleges szakértői konklúzió.".to_string());
    }
    if opinion.finalizable != "igen" {
        issues.push("Az ügy nincs „véglegesíthető” jelölésűre állítva.".to_string());
    }
    if !opinion.open_questions.trim().is_empty() {
        issues.push("Maradtak nyitott kérdések; véglegesítés előtt le kell zárni vagy indokolni kell őket.".to_string());
    }
    if check_files {
        for source in &sources {
            if source.file_path.trim().is_empty() && source.hash.trim().is_empty() {
                continue;
            }
            let state = check_file(&source.file_path, &source.hash, archive_dir);
            match state.state.as_str() {
                "ok" => {}
                "eltér" => issues.push(format!(
                    "A(z) „{}” (#{}) forrás archív fájlja megváltozott a rögzítés óta.",
                    source.title, source.id
                )),
                "hiányzik" => issues.push(format!(
                    "A(z) „{}” (#{}) forrás archív fájlja hiányzik.",
                    source.title, source.id
                )),
                _ => issues.push(format!(
                    "A(z) „{}” (#{}) forráshoz nem tartozik rögzített hash.",
                    source.title, source.id
                )),
            }
        }
    }
    Ok(issues)
}

#[derive(Serialize, Debug, Clone)]
pub struct VersionInfo {
    pub id: i64,
    pub case_id: i64,
    pub version: i64,
    pub approved_by: String,
    pub approved_at: String,
    pub snapshot_hash: String,
    pub content_hash: String,
    pub file_path: String,
    /// "ok", "eltér" vagy "hiányzik": a kiadott fájl még azonos-e a rögzített tartalommal.
    pub file_state: String,
}

fn sha256_hex(text: &str) -> String {
    hex::encode(Sha256::digest(text.as_bytes()))
}

/// Az ügy és a felhasznált mellékletek állapota a kiadás pillanatában (értékelésekkel együtt).
fn build_snapshot(conn: &Connection, archive_dir: &Path, case_id: i64) -> Result<Value, String> {
    let case = conn
        .query_row(
            "SELECT title, description, deadline, status, preliminary_opinion, final_opinion, report_status, finalizable, open_questions, closed_points
             FROM cases WHERE id = ?1",
            params![case_id],
            |row| {
                let t = |i: usize| row.get::<_, Option<String>>(i).map(|v| v.unwrap_or_default());
                Ok(json!({
                    "id": case_id, "title": t(0)?, "description": t(1)?, "deadline": t(2)?, "status": t(3)?,
                    "preliminary_opinion": t(4)?, "final_opinion": t(5)?, "report_status": t(6)?,
                    "finalizable": t(7)?, "open_questions": t(8)?, "closed_points": t(9)?,
                }))
            },
        )
        .map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT id, search_run_id, title, url, tier, hash, file_path, timestamp, published_at, modified_at, capture_kind, capture_truncated,
                    source_status, priority_level, usability_decision, proof_summary, reviewer_name, reviewed_at, relevance_note, probative_value, missing_points
             FROM sources WHERE case_id = ?1 ORDER BY id",
        )
        .map_err(|e| e.to_string())?;
    let sources: Vec<Value> = stmt
        .query_map(params![case_id], |row| {
            let t = |i: usize| row.get::<_, Option<String>>(i).map(|v| v.unwrap_or_default());
            let hash = t(5)?;
            let file_path = t(6)?;
            let integrity: Integrity = check_file(&file_path, &hash, archive_dir);
            Ok(json!({
                "id": row.get::<_, i64>(0)?, "search_run_id": row.get::<_, Option<i64>>(1)?,
                "title": t(2)?, "url": t(3)?, "tier": row.get::<_, Option<i64>>(4)?.unwrap_or(3),
                "sha256": hash, "file": file_path, "collected_at": t(7)?, "published_at": t(8)?, "modified_at": t(9)?,
                "capture_kind": t(10)?, "capture_truncated": row.get::<_, Option<i64>>(11)?.unwrap_or(0) != 0,
                "review": {
                    "status": t(12)?, "priority": row.get::<_, Option<i64>>(13)?.unwrap_or(2), "decision": t(14)?,
                    "proof_summary": t(15)?, "reviewer": t(16)?, "reviewed_at": t(17)?,
                    "relevance_note": t(18)?, "probative_value": t(19)?, "missing_points": t(20)?,
                },
                "integrity_at_issue": integrity.state,
            }))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut run_stmt = conn
        .prepare("SELECT id, created_at, base_query, summary FROM search_runs WHERE case_id = ?1 ORDER BY id")
        .map_err(|e| e.to_string())?;
    let runs: Vec<Value> = run_stmt
        .query_map(params![case_id], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "created_at": row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                "query": row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                "summary": row.get::<_, Option<String>>(3)?.unwrap_or_default(),
            }))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(json!({ "case": case, "sources": sources, "runs": runs }))
}

fn clause(version: i64, approver: &str, approved_at: &str, snapshot_hash: &str, final_opinion: &str, snapshot: &Value) -> String {
    let sources = snapshot["sources"].as_array().cloned().unwrap_or_default();
    let mut lines = vec![
        String::new(),
        "---".to_string(),
        "## Kiadási záradék".to_string(),
        format!("- Kiadott verzió: **v{version}** (változtathatatlan)"),
        format!("- Jóváhagyta: **{approver}** (a szakértő maga), {approved_at}"),
        format!("- Ügyállapot-pillanatkép SHA-256: `{snapshot_hash}`"),
        format!("- Jóváhagyott végleges konklúzió: {}", final_opinion.trim()),
        format!("- Felhasznált mellékletek száma: {}", sources.len()),
    ];
    for source in &sources {
        lines.push(format!(
            "  - #{} {} | SHA-256 `{}` | rögzítés: {} | értékelés: {}/{}",
            source["id"],
            source["title"].as_str().unwrap_or(""),
            source["sha256"].as_str().unwrap_or(""),
            match source["capture_kind"].as_str().unwrap_or("") {
                "kijelolt_szoveg" => "kijelölt szövegrészlet",
                "teljes_oldal" => "teljes oldal",
                _ => "ismeretlen",
            },
            source["review"]["status"].as_str().unwrap_or(""),
            source["review"]["decision"].as_str().unwrap_or(""),
        ));
    }
    lines.join("\n")
}

/// Végleges kiadás: csak mentett, véglegesített ügyállapotból, a szakértő nevével jóváhagyva.
/// `report_text` a felület által összeállított riport; a konklúziónak szerepelnie kell benne.
pub fn approve(
    conn: &Connection,
    archive_dir: &Path,
    reports_dir: &Path,
    case_id: i64,
    approver: &str,
    report_text: &str,
    now: &str,
) -> Result<VersionInfo, String> {
    integrity::ensure_case_exists(conn, case_id)?;
    let approver = approver.trim();
    if approver.chars().count() < 3 {
        return Err("A jóváhagyáshoz add meg a szakértő nevét (legalább 3 karakter).".to_string());
    }

    let (status, finalizable, final_opinion, open_questions): (String, String, String, String) = conn
        .query_row(
            "SELECT report_status, finalizable, final_opinion, open_questions FROM cases WHERE id = ?1",
            params![case_id],
            |row| {
                let t = |i: usize| row.get::<_, Option<String>>(i).map(|v| v.unwrap_or_default());
                Ok((t(0)?, t(1)?, t(2)?, t(3)?))
            },
        )
        .map_err(|e| e.to_string())?;
    if status != "veglegesitett" {
        return Err("Végleges kiadás csak mentett, „véglegesített” ügyállapotból készülhet. Előbb mentsd az ügy álláspontját.".to_string());
    }

    let opinion = Opinion { final_opinion: &final_opinion, finalizable: &finalizable, open_questions: &open_questions };
    let issues = finalization_issues(conn, archive_dir, case_id, &opinion, true)?;
    if !issues.is_empty() {
        return Err(format!("A végleges kiadás akadályai: {}", issues.join(" | ")));
    }
    if !report_text.contains(final_opinion.trim()) {
        return Err("A riportszöveg nem a mentett végleges konklúziót tartalmazza (nem mentett módosítás?). Mentsd az álláspontot, majd generáld újra a riportot.".to_string());
    }

    let snapshot = build_snapshot(conn, archive_dir, case_id)?;
    let snapshot_text = serde_json::to_string(&snapshot).map_err(|e| e.to_string())?;
    let snapshot_hash = sha256_hex(&snapshot_text);
    let version: i64 = conn
        .query_row("SELECT COALESCE(MAX(version), 0) + 1 FROM report_versions WHERE case_id = ?1", params![case_id], |r| r.get(0))
        .map_err(|e| e.to_string())?;

    let content = format!(
        "{}\n{}\n",
        report_text.trim_end(),
        clause(version, approver, now, &snapshot_hash, &final_opinion, &snapshot)
    );
    let content_hash = sha256_hex(&content);

    std::fs::create_dir_all(reports_dir).map_err(|e| e.to_string())?;
    let stamp = now.replace([' ', ':', '-'], "");
    let file_path = reports_dir.join(format!("irbr_vegleges_{case_id}_v{version}_{stamp}.md"));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&file_path)
        .map_err(|e| format!("A kiadott riport fájlja nem hozható létre: {e}"))?;
    file.write_all(content.as_bytes()).map_err(|e| e.to_string())?;
    drop(file);
    let path_text = file_path.to_string_lossy().into_owned();

    let inserted = conn.execute(
        "INSERT INTO report_versions (case_id, version, approved_by, approved_at, final_opinion, snapshot, snapshot_hash, content, content_hash, file_path)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![case_id, version, approver, now, final_opinion, snapshot_text, snapshot_hash, content, content_hash, path_text],
    );
    if let Err(error) = inserted {
        let _ = std::fs::remove_file(&file_path); // ne maradjon fájl rekord nélkül
        return Err(error.to_string());
    }
    let id = conn.last_insert_rowid();
    Ok(VersionInfo {
        id,
        case_id,
        version,
        approved_by: approver.to_string(),
        approved_at: now.to_string(),
        snapshot_hash,
        content_hash,
        file_path: path_text,
        file_state: "ok".to_string(),
    })
}

pub fn list_versions(conn: &Connection, case_id: i64) -> Result<Vec<VersionInfo>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, version, approved_by, approved_at, snapshot_hash, content_hash, file_path, content
             FROM report_versions WHERE case_id = ?1 ORDER BY version DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![case_id], |row| {
            let content_hash: String = row.get(5)?;
            let file_path: String = row.get(6)?;
            let state = match std::fs::read(&file_path) {
                Ok(bytes) => {
                    if hex::encode(Sha256::digest(&bytes)).eq_ignore_ascii_case(&content_hash) { "ok" } else { "eltér" }
                }
                Err(_) => "hiányzik",
            };
            Ok(VersionInfo {
                id: row.get(0)?,
                case_id,
                version: row.get(1)?,
                approved_by: row.get(2)?,
                approved_at: row.get(3)?,
                snapshot_hash: row.get(4)?,
                content_hash,
                file_path,
                file_state: state.to_string(),
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

/// Tervezet-jelölés a szabadon szerkeszthető, nem jóváhagyott exportra.
pub const DRAFT_BANNER: &str = "> **TERVEZET – nem végleges kiadás.** Nem jóváhagyott, nem verziózott, szerkeszthető állapotból készült; hivatalos felhasználásra nem alkalmas.\n\n";

pub fn mark_draft(report_text: &str) -> String {
    if report_text.starts_with(DRAFT_BANNER) {
        report_text.to_string()
    } else {
        format!("{DRAFT_BANNER}{report_text}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup(dir: &Path) -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE cases (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT, description TEXT, deadline TEXT, status TEXT,
                preliminary_opinion TEXT, final_opinion TEXT, report_status TEXT, finalizable TEXT, open_questions TEXT, closed_points TEXT);
             CREATE TABLE search_runs (id INTEGER PRIMARY KEY AUTOINCREMENT, case_id INTEGER NOT NULL, created_at TEXT, base_query TEXT, summary TEXT);
             CREATE TABLE sources (id INTEGER PRIMARY KEY AUTOINCREMENT, case_id INTEGER, search_run_id INTEGER, title TEXT, url TEXT, tier INTEGER,
                hash TEXT, file_path TEXT, timestamp TEXT, published_at TEXT, modified_at TEXT, capture_kind TEXT, capture_truncated INTEGER,
                source_status TEXT, priority_level INTEGER, usability_decision TEXT, proof_summary TEXT, reviewer_name TEXT, reviewed_at TEXT,
                relevance_note TEXT, probative_value TEXT, missing_points TEXT);
             CREATE TABLE run_results (id INTEGER PRIMARY KEY AUTOINCREMENT, run_id INTEGER NOT NULL, url TEXT);
             CREATE TABLE report_versions (id INTEGER PRIMARY KEY AUTOINCREMENT, case_id INTEGER NOT NULL, version INTEGER NOT NULL, approved_by TEXT,
                approved_at TEXT, final_opinion TEXT, snapshot TEXT, snapshot_hash TEXT, content TEXT, content_hash TEXT, file_path TEXT, UNIQUE(case_id, version));
             INSERT INTO cases (title, description, deadline, status, preliminary_opinion, final_opinion, report_status, finalizable, open_questions, closed_points)
               VALUES ('Metszés', '', '', '', '', 'A munka értéke 480 000 Ft.', 'veglegesitett', 'igen', '', '');",
        )
        .unwrap();
        crate::integrity::install_triggers(&conn).unwrap();
        fs::create_dir_all(dir).unwrap();
        conn
    }

    fn add_source(conn: &Connection, dir: &Path, name: &str, content: &str, decision: &str, proof: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        fs::write(&path, content).unwrap();
        let hash = sha256_hex(content);
        conn.execute(
            "INSERT INTO sources (case_id, title, url, tier, hash, file_path, timestamp, capture_kind, capture_truncated, source_status, priority_level,
                usability_decision, proof_summary, reviewer_name, reviewed_at, relevance_note, probative_value, missing_points)
             VALUES (1, ?1, 'https://x.hu', 1, ?2, ?3, '2026-01-01 10:00:00', 'kijelolt_szoveg', 0, 'validalt', 1, ?4, ?5, 'Dr. Teszt', '2026-01-02', '', '', '')",
            params![name, hash, path.to_string_lossy(), decision, proof],
        )
        .unwrap();
        path
    }

    fn tmp(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("irbr_release_{tag}_{}", std::process::id()))
    }

    const REPORT: &str = "# Riport\nA munka értéke 480 000 Ft.\n";

    #[test]
    fn the_audits_counterexample_is_not_finalizable() {
        let dir = tmp("counter");
        let conn = setup(&dir);
        add_source(&conn, &dir, "nyers.txt", "x", "hatteranyag", "");
        // üres konklúzió, nem véglegesíthető, nyers háttérforrás (az audit ellenpéldája)
        let op = Opinion { final_opinion: "", finalizable: "nem", open_questions: "" };
        let issues = finalization_issues(&conn, &dir, 1, &op, true).unwrap();
        let joined = issues.join(" | ");
        assert!(joined.contains("Nincs végleges szakértői konklúzió"), "{joined}");
        assert!(joined.contains("véglegesíthető"), "{joined}");
        assert!(joined.contains("felhasználhatónak jelölt forrás"), "{joined}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_complete_case_has_no_issues_and_each_rule_can_block() {
        let dir = tmp("rules");
        let conn = setup(&dir);
        add_source(&conn, &dir, "szamla.txt", "számla: 480 000 Ft", "felhasznalhato", "a számla az elszámolt összeget igazolja");
        let ok = Opinion { final_opinion: "konklúzió", finalizable: "igen", open_questions: "" };
        assert!(finalization_issues(&conn, &dir, 1, &ok, true).unwrap().is_empty());

        let open = Opinion { final_opinion: "konklúzió", finalizable: "igen", open_questions: "Mi a díj?" };
        assert!(finalization_issues(&conn, &dir, 1, &open, true).unwrap().join(" ").contains("nyitott kérdések"));

        conn.execute("UPDATE sources SET proof_summary = ''", []).unwrap();
        assert!(finalization_issues(&conn, &dir, 1, &ok, true).unwrap().join(" ").contains("bizonyítási kivonat"));
        conn.execute("UPDATE sources SET proof_summary = 'x', reviewer_name = ''", []).unwrap();
        assert!(finalization_issues(&conn, &dir, 1, &ok, true).unwrap().join(" ").contains("validálási metaadat"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn tampered_or_missing_attachment_blocks_finalization() {
        let dir = tmp("files");
        let conn = setup(&dir);
        let path = add_source(&conn, &dir, "szamla.txt", "számla", "felhasznalhato", "igazolja");
        let ok = Opinion { final_opinion: "k", finalizable: "igen", open_questions: "" };
        assert!(finalization_issues(&conn, &dir, 1, &ok, true).unwrap().is_empty());
        fs::write(&path, "megváltoztatva").unwrap();
        assert!(finalization_issues(&conn, &dir, 1, &ok, true).unwrap().join(" ").contains("megváltozott"));
        fs::remove_file(&path).unwrap();
        assert!(finalization_issues(&conn, &dir, 1, &ok, true).unwrap().join(" ").contains("hiányzik"));
        // fájlellenőrzés nélkül (pl. állapotmentés közben) nem akadály
        assert!(finalization_issues(&conn, &dir, 1, &ok, false).unwrap().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn approval_creates_an_immutable_versioned_snapshot() {
        let dir = tmp("approve");
        let reports = dir.join("reports");
        let conn = setup(&dir);
        add_source(&conn, &dir, "szamla.txt", "számla: 480 000 Ft", "felhasznalhato", "igazolja az összeget");

        let v1 = approve(&conn, &dir, &reports, 1, "Dr. Minta Szakértő", REPORT, "2026-03-01 09:00:00").unwrap();
        assert_eq!(v1.version, 1);
        let text = fs::read_to_string(&v1.file_path).unwrap();
        assert!(text.contains("Kiadási záradék") && text.contains("v1") && text.contains("Dr. Minta Szakértő"));
        assert!(text.contains(&v1.snapshot_hash) && text.contains("A munka értéke 480 000 Ft."));
        assert!(text.contains("kijelölt szövegrészlet"));
        assert_eq!(sha256_hex(&text), v1.content_hash);

        // második kiadás: új verzió, az első érintetlen
        let v2 = approve(&conn, &dir, &reports, 1, "Dr. Minta Szakértő", REPORT, "2026-03-02 09:00:00").unwrap();
        assert_eq!(v2.version, 2);
        let list = list_versions(&conn, 1).unwrap();
        assert_eq!(list.iter().map(|v| v.version).collect::<Vec<_>>(), vec![2, 1]);
        assert!(list.iter().all(|v| v.file_state == "ok"));

        // változtathatatlanság az adatbázisban és az ellenőrzés a fájlon
        assert!(conn.execute("UPDATE report_versions SET content = 'átírt'", []).is_err());
        fs::write(&v1.file_path, "átírt fájl").unwrap();
        let list = list_versions(&conn, 1).unwrap();
        assert_eq!(list.iter().find(|v| v.version == 1).unwrap().file_state, "eltér");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn approval_refuses_unsaved_invalid_or_anonymous_requests() {
        let dir = tmp("refuse");
        let reports = dir.join("reports");
        let conn = setup(&dir);
        add_source(&conn, &dir, "szamla.txt", "számla", "felhasznalhato", "igazolja");

        assert!(approve(&conn, &dir, &reports, 1, "  ", REPORT, "t").unwrap_err().contains("nevét"));
        // a riportszöveg nem a mentett konklúziót tartalmazza
        assert!(approve(&conn, &dir, &reports, 1, "Dr. Minta", "# Riport\nmás konklúzió", "t").unwrap_err().contains("mentett végleges konklúziót"));
        // nem véglegesített mentett állapot: csak tervezet
        conn.execute("UPDATE cases SET report_status = 'tervezet'", []).unwrap();
        assert!(approve(&conn, &dir, &reports, 1, "Dr. Minta", REPORT, "t").unwrap_err().contains("véglegesített"));
        conn.execute("UPDATE cases SET report_status = 'veglegesitett', final_opinion = ''", []).unwrap();
        assert!(approve(&conn, &dir, &reports, 1, "Dr. Minta", REPORT, "t").unwrap_err().contains("akadályai"));
        assert!(list_versions(&conn, 1).unwrap().is_empty());
        assert!(!reports.exists() || fs::read_dir(&reports).unwrap().next().is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn drafts_get_a_visible_banner_once() {
        let text = mark_draft("# Riport\nStátusz: Véglegesített");
        assert!(text.starts_with("> **TERVEZET"));
        assert_eq!(mark_draft(&text), text);
    }
}
