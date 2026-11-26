//! Ügyhatárok az adatbázisban: egy forrás csak a saját ügyének futamára hivatkozhat, a kiadott
//! riportverziók pedig nem módosíthatók. A szabályokat triggerek érvényesítik, így akkor is
//! védenek, ha a felület hibásan hív; a régi adatbázis ellentmondásos rekordjait a migráció
//! nem javítja, hanem auditbejegyzésben felsorolja (az eredeti rekord érintetlen marad).

use rusqlite::{params, Connection};

const TRIGGERS: &[&str] = &[
    // forrás: a futam ugyanahhoz az ügyhöz tartozzon
    "CREATE TRIGGER IF NOT EXISTS trg_sources_run_same_case_ins
     BEFORE INSERT ON sources
     WHEN NEW.search_run_id IS NOT NULL
      AND NOT EXISTS (SELECT 1 FROM search_runs WHERE id = NEW.search_run_id AND case_id = NEW.case_id)
     BEGIN SELECT RAISE(ABORT, 'A forrás futama másik ügyhöz tartozik vagy nem létezik.'); END",
    "CREATE TRIGGER IF NOT EXISTS trg_sources_run_same_case_upd
     BEFORE UPDATE OF case_id, search_run_id ON sources
     WHEN NEW.search_run_id IS NOT NULL
      AND NOT EXISTS (SELECT 1 FROM search_runs WHERE id = NEW.search_run_id AND case_id = NEW.case_id)
     BEGIN SELECT RAISE(ABORT, 'A forrás futama másik ügyhöz tartozik vagy nem létezik.'); END",
    // futam-találat: létező futamhoz
    "CREATE TRIGGER IF NOT EXISTS trg_run_results_run_exists
     BEFORE INSERT ON run_results
     WHEN NOT EXISTS (SELECT 1 FROM search_runs WHERE id = NEW.run_id)
     BEGIN SELECT RAISE(ABORT, 'A találat futama nem létezik.'); END",
    // kiadott riportverzió: nem módosítható; csak az ügy törlésével együtt törölhető
    "CREATE TRIGGER IF NOT EXISTS trg_report_versions_no_update
     BEFORE UPDATE ON report_versions
     BEGIN SELECT RAISE(ABORT, 'A kiadott riportverzió nem módosítható.'); END",
    "CREATE TRIGGER IF NOT EXISTS trg_report_versions_no_delete
     BEFORE DELETE ON report_versions
     WHEN EXISTS (SELECT 1 FROM cases WHERE id = OLD.case_id)
     BEGIN SELECT RAISE(ABORT, 'A kiadott riportverzió nem törölhető (csak az ügy törlésével együtt).'); END",
];

pub fn install_triggers(conn: &Connection) -> Result<(), String> {
    for sql in TRIGGERS {
        conn.execute(sql, []).map_err(|e| format!("Trigger létrehozási hiba: {e}"))?;
    }
    Ok(())
}

pub fn ensure_case_exists(conn: &Connection, case_id: i64) -> Result<String, String> {
    conn.query_row("SELECT title FROM cases WHERE id = ?1", params![case_id], |row| {
        row.get::<_, Option<String>>(0)
    })
    .map(|title| title.unwrap_or_default())
    .map_err(|_| "Az ügy nem található.".to_string())
}

/// Hibát ad, ha a futam nem az adott ügyhöz tartozik (vagy nem létezik). `None` futam nem hiba.
pub fn ensure_run_belongs(conn: &Connection, case_id: i64, run_id: Option<i64>) -> Result<(), String> {
    let Some(run_id) = run_id else { return Ok(()) };
    let run_case: Option<i64> = conn
        .query_row("SELECT case_id FROM search_runs WHERE id = ?1", params![run_id], |row| row.get(0))
        .ok();
    match run_case {
        Some(owner) if owner == case_id => Ok(()),
        Some(_) => Err("A keresési futam másik ügyhöz tartozik, a mentés megtagadva.".to_string()),
        None => Err("A keresési futam nem létezik, a mentés megtagadva.".to_string()),
    }
}

#[derive(Debug, PartialEq)]
pub struct LegacyMismatch {
    pub source_id: i64,
    pub source_case_id: i64,
    pub run_id: i64,
    pub run_case_id: Option<i64>,
}

/// A már meglévő, ellentmondó forrásrekordok (a trigger csak az újakat akadályozza meg).
pub fn legacy_mismatches(conn: &Connection) -> Result<Vec<LegacyMismatch>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT s.id, s.case_id, s.search_run_id, r.case_id
             FROM sources s LEFT JOIN search_runs r ON r.id = s.search_run_id
             WHERE s.search_run_id IS NOT NULL AND (r.id IS NULL OR r.case_id != s.case_id)
             ORDER BY s.id",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(LegacyMismatch {
                source_id: row.get(0)?,
                source_case_id: row.get::<_, Option<i64>>(1)?.unwrap_or_default(),
                run_id: row.get(2)?,
                run_case_id: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

fn table_exists(conn: &Connection, name: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
        params![name],
        |_| Ok(()),
    )
    .is_ok()
}

/// A régi ellentmondásos rekordokat auditbejegyzésbe írja (rekordonként egy, az érintett ügynél);
/// a rekordokat nem módosítja. Visszaadja a felsorolt rekordok számát.
pub fn log_legacy_mismatches(conn: &Connection, timestamp: &str) -> Result<usize, String> {
    if !table_exists(conn, "audit_logs") || !table_exists(conn, "sources") || !table_exists(conn, "search_runs") {
        return Ok(0);
    }
    let found = legacy_mismatches(conn)?;
    for item in &found {
        let details = format!(
            "Ellentmondó régi rekord: a(z) {} azonosítójú forrás a(z) {} ügyé, de a(z) {} azonosítójú keresési futamra hivatkozik, amely {}. A rekord változatlan maradt; kézi felülvizsgálat szükséges.",
            item.source_id,
            item.source_case_id,
            item.run_id,
            match item.run_case_id {
                Some(case) => format!("a(z) {case} ügyé"),
                None => "nem létezik".to_string(),
            }
        );
        conn.execute(
            "INSERT INTO audit_logs (case_id, timestamp, action_type, query, details) VALUES (?1, ?2, 'integrity_legacy_mismatch', ?3, ?4)",
            params![item.source_case_id, timestamp, format!("source:{}", item.source_id), details],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(found.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE cases (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT);
             CREATE TABLE audit_logs (id INTEGER PRIMARY KEY AUTOINCREMENT, case_id INTEGER, timestamp TEXT, action_type TEXT, query TEXT, details TEXT);
             CREATE TABLE search_runs (id INTEGER PRIMARY KEY AUTOINCREMENT, case_id INTEGER NOT NULL, created_at TEXT);
             CREATE TABLE sources (id INTEGER PRIMARY KEY AUTOINCREMENT, case_id INTEGER, search_run_id INTEGER, title TEXT);
             CREATE TABLE run_results (id INTEGER PRIMARY KEY AUTOINCREMENT, run_id INTEGER NOT NULL, url TEXT);
             CREATE TABLE report_versions (id INTEGER PRIMARY KEY AUTOINCREMENT, case_id INTEGER NOT NULL, version INTEGER NOT NULL, content TEXT);
             INSERT INTO cases (title) VALUES ('A ügy'), ('B ügy');
             INSERT INTO search_runs (case_id, created_at) VALUES (1, 't');",
        )
        .unwrap();
        conn
    }

    #[test]
    fn source_of_another_case_cannot_reference_the_run() {
        let conn = base();
        install_triggers(&conn).unwrap();
        // ugyanaz az ügy: rendben
        conn.execute("INSERT INTO sources (case_id, search_run_id, title) VALUES (1, 1, 'ok')", []).unwrap();
        // idegen ügy futamára hivatkozva: elutasítva (az audit 1. hibája)
        let error = conn
            .execute("INSERT INTO sources (case_id, search_run_id, title) VALUES (2, 1, 'idegen')", [])
            .unwrap_err()
            .to_string();
        assert!(error.contains("másik ügyhöz"), "{error}");
        // nem létező futam: elutasítva
        assert!(conn.execute("INSERT INTO sources (case_id, search_run_id, title) VALUES (1, 99, 'x')", []).is_err());
        // futam nélküli forrás megengedett
        conn.execute("INSERT INTO sources (case_id, search_run_id, title) VALUES (2, NULL, 'kézi')", []).unwrap();
        // átállítás idegen ügyre: elutasítva
        assert!(conn.execute("UPDATE sources SET case_id = 2 WHERE title = 'ok'", []).is_err());
        // érték-módosítás a hivatkozás érintése nélkül megengedett
        conn.execute("UPDATE sources SET title = 'átnevezve' WHERE title = 'ok'", []).unwrap();
    }

    #[test]
    fn run_results_need_an_existing_run() {
        let conn = base();
        install_triggers(&conn).unwrap();
        conn.execute("INSERT INTO run_results (run_id, url) VALUES (1, 'https://a.hu')", []).unwrap();
        assert!(conn.execute("INSERT INTO run_results (run_id, url) VALUES (42, 'https://b.hu')", []).is_err());
    }

    #[test]
    fn issued_report_versions_are_immutable_but_go_with_the_case() {
        let conn = base();
        install_triggers(&conn).unwrap();
        conn.execute("INSERT INTO report_versions (case_id, version, content) VALUES (1, 1, 'eredeti')", []).unwrap();
        assert!(conn.execute("UPDATE report_versions SET content = 'átírt'", []).is_err());
        assert!(conn.execute("DELETE FROM report_versions WHERE case_id = 1", []).is_err());
        // az ügy törlése után (a delete_case ebben a sorrendben dolgozik) törölhető
        conn.execute("DELETE FROM cases WHERE id = 1", []).unwrap();
        conn.execute("DELETE FROM report_versions WHERE case_id = 1", []).unwrap();
        let left: i64 = conn.query_row("SELECT COUNT(*) FROM report_versions", [], |r| r.get(0)).unwrap();
        assert_eq!(left, 0);
    }

    #[test]
    fn ensure_run_belongs_checks_ownership_and_existence() {
        let conn = base();
        assert!(ensure_run_belongs(&conn, 1, Some(1)).is_ok());
        assert!(ensure_run_belongs(&conn, 1, None).is_ok());
        assert!(ensure_run_belongs(&conn, 2, Some(1)).unwrap_err().contains("másik ügyhöz"));
        assert!(ensure_run_belongs(&conn, 1, Some(5)).unwrap_err().contains("nem létezik"));
        assert_eq!(ensure_case_exists(&conn, 1).unwrap(), "A ügy");
        assert!(ensure_case_exists(&conn, 9).is_err());
    }

    #[test]
    fn legacy_mismatches_are_listed_in_the_audit_log_and_left_untouched() {
        let conn = base();
        // a trigger előtt keletkezett ellentmondó rekord (régi adatbázis)
        conn.execute("INSERT INTO sources (case_id, search_run_id, title) VALUES (2, 1, 'régi idegen')", []).unwrap();
        conn.execute("INSERT INTO sources (case_id, search_run_id, title) VALUES (1, 1, 'rendes')", []).unwrap();
        let found = legacy_mismatches(&conn).unwrap();
        assert_eq!(found, vec![LegacyMismatch { source_id: 1, source_case_id: 2, run_id: 1, run_case_id: Some(1) }]);

        assert_eq!(log_legacy_mismatches(&conn, "2026-01-01 00:00:00").unwrap(), 1);
        let (case, action, details): (i64, String, String) = conn
            .query_row("SELECT case_id, action_type, details FROM audit_logs", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap();
        assert_eq!((case, action.as_str()), (2, "integrity_legacy_mismatch"));
        assert!(details.contains("változatlan"));
        // a rekord érintetlen
        let kept: i64 = conn
            .query_row("SELECT case_id FROM sources WHERE title = 'régi idegen'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(kept, 2);
    }

    #[test]
    fn legacy_logging_is_skipped_on_minimal_databases() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE sources (id INTEGER PRIMARY KEY, case_id INTEGER)", []).unwrap();
        assert_eq!(log_legacy_mismatches(&conn, "t").unwrap(), 0);
    }
}
