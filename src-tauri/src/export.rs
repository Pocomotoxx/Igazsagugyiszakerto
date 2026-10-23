//! Forrásrekordok exportja: JSON, CSV, nyomtatható HTML és STIX 2.1.

use chrono::{DateTime, Local, NaiveDateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ExportRecord {
    pub id: i64,
    pub case_id: i64,
    pub search_run_id: Option<i64>,
    pub title: String,
    pub url: String,
    pub original_url: String,
    pub archived_url: String,
    pub archive_kind: String,
    pub tier: i32,
    pub source_status: String,
    pub usability_decision: String,
    pub hash: String,
    pub file_path: String,
    pub collected_at: String,
    pub observed_at: String,
    pub published_at: String,
    pub modified_at: String,
    pub time_source: String,
    pub freshness: String,
    pub reviewer_name: String,
    pub proof_summary: String,
    pub relevance_note: String,
    pub probative_value: String,
    pub missing_points: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct ExportMetadata {
    pub generated_at: String,
    pub case_title: String,
    pub total: usize,
    pub exported: usize,
    pub truncated: bool,
    pub filters: String,
}

pub const EXPORT_CAP: usize = 5000;

const FIELDS: &[&str] = &[
    "id", "title", "url", "original_url", "archived_url", "archive_kind", "tier", "source_status",
    "usability_decision", "hash", "collected_at", "observed_at", "published_at", "modified_at",
    "time_source", "freshness", "reviewer_name", "proof_summary", "relevance_note",
    "probative_value", "missing_points", "search_run_id", "file_path",
];

fn field_value(record: &ExportRecord, field: &str) -> String {
    match field {
        "id" => record.id.to_string(),
        "title" => record.title.clone(),
        "url" => record.url.clone(),
        "original_url" => record.original_url.clone(),
        "archived_url" => record.archived_url.clone(),
        "archive_kind" => record.archive_kind.clone(),
        "tier" => record.tier.to_string(),
        "source_status" => record.source_status.clone(),
        "usability_decision" => record.usability_decision.clone(),
        "hash" => record.hash.clone(),
        "collected_at" => record.collected_at.clone(),
        "observed_at" => record.observed_at.clone(),
        "published_at" => record.published_at.clone(),
        "modified_at" => record.modified_at.clone(),
        "time_source" => record.time_source.clone(),
        "freshness" => record.freshness.clone(),
        "reviewer_name" => record.reviewer_name.clone(),
        "proof_summary" => record.proof_summary.clone(),
        "relevance_note" => record.relevance_note.clone(),
        "probative_value" => record.probative_value.clone(),
        "missing_points" => record.missing_points.clone(),
        "search_run_id" => record.search_run_id.map(|v| v.to_string()).unwrap_or_default(),
        "file_path" => record.file_path.clone(),
        _ => String::new(),
    }
}

/// CSV-cella: a képletként értelmezhető kezdetű (`=`, `+`, `-`, `@`, tab, kocsi-vissza)
/// szöveget aposztróffal védi, az idézőjeleket duplázza.
pub fn csv_cell(value: &str) -> String {
    let mut safe = value.to_string();
    let first_significant = safe.trim_start_matches(|c: char| c.is_whitespace() || c.is_control());
    if first_significant.starts_with(['=', '+', '-', '@']) || safe.starts_with(['\t', '\r']) {
        safe = format!("'{safe}");
    }
    format!("\"{}\"", safe.replace('"', "\"\""))
}

pub fn to_csv(records: &[ExportRecord], metadata: &ExportMetadata) -> String {
    // UTF-8 BOM: az Excel ennek hiányában elrontja az ékezeteket.
    let mut out = String::from("\u{feff}");
    let mut header: Vec<&str> = FIELDS.to_vec();
    header.extend(["export_generated_at", "export_total", "export_count", "export_truncated", "export_filters"]);
    out.push_str(&header.iter().map(|h| csv_cell(h)).collect::<Vec<_>>().join(","));
    out.push_str("\r\n");
    for record in records {
        let mut cells: Vec<String> = FIELDS.iter().map(|f| csv_cell(&field_value(record, f))).collect();
        cells.push(csv_cell(&metadata.generated_at));
        cells.push(csv_cell(&metadata.total.to_string()));
        cells.push(csv_cell(&metadata.exported.to_string()));
        cells.push(csv_cell(&metadata.truncated.to_string()));
        cells.push(csv_cell(&metadata.filters));
        out.push_str(&cells.join(","));
        out.push_str("\r\n");
    }
    out
}

pub fn to_json(records: &[ExportRecord], metadata: &ExportMetadata) -> Result<String, String> {
    serde_json::to_string_pretty(&json!({ "metadata": metadata, "records": records }))
        .map_err(|e| e.to_string())
}

pub fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub fn to_html(records: &[ExportRecord], metadata: &ExportMetadata) -> String {
    let mut out = String::new();
    out.push_str("<!doctype html><html lang=\"hu\"><head><meta charset=\"utf-8\">");
    out.push_str(&format!(
        "<title>IRBR forráskivonat – {}</title>",
        html_escape(&metadata.case_title)
    ));
    out.push_str(
        "<style>body{font-family:Segoe UI,Arial,sans-serif;max-width:900px;margin:2rem auto;color:#0f172a}\
         h1{font-size:1.4rem}.meta{color:#475569;font-size:.85rem;margin-bottom:1.5rem}\
         article{border:1px solid #cbd5e1;border-radius:8px;padding:.8rem 1rem;margin:.8rem 0;page-break-inside:avoid}\
         h2{font-size:1rem;margin:0 0 .3rem}dl{display:grid;grid-template-columns:11rem 1fr;gap:.15rem .8rem;margin:0;font-size:.82rem}\
         dt{color:#64748b}dd{margin:0;word-break:break-word}code{font-size:.78rem}\
         @media print{body{margin:0}}</style></head><body>",
    );
    out.push_str(&format!(
        "<h1>IRBR forráskivonat – {}</h1><div class=\"meta\">Létrehozva: {} · Rekordok: {} / {}{} · Szűrők: {}</div>",
        html_escape(&metadata.case_title),
        html_escape(&metadata.generated_at),
        metadata.exported,
        metadata.total,
        if metadata.truncated { " (csonkolva)" } else { "" },
        html_escape(&metadata.filters),
    ));
    for record in records {
        out.push_str(&format!("<article><h2>{}</h2><dl>", html_escape(&record.title)));
        let rows: [(&str, String); 14] = [
            ("URL", record.url.clone()),
            ("Tier / státusz", format!("{} / {}", record.tier, record.source_status)),
            ("Felhasználhatóság", record.usability_decision.clone()),
            ("Közzététel", record.published_at.clone()),
            ("Módosítás", record.modified_at.clone()),
            ("Megfigyelés (snapshot)", record.observed_at.clone()),
            ("Begyűjtés", record.collected_at.clone()),
            ("Időforrás", record.time_source.clone()),
            ("Frissesség", record.freshness.clone()),
            ("SHA-256", record.hash.clone()),
            ("Bizonyító erő", record.probative_value.clone()),
            ("Összegzés", record.proof_summary.clone()),
            ("Hiányzó pontok", record.missing_points.clone()),
            ("Átvizsgálta", record.reviewer_name.clone()),
        ];
        for (label, value) in rows {
            if value.trim().is_empty() || value.trim() == "/ " {
                continue;
            }
            out.push_str(&format!(
                "<dt>{}</dt><dd>{}</dd>",
                html_escape(label),
                html_escape(&value)
            ));
        }
        out.push_str("</dl></article>");
    }
    out.push_str("</body></html>");
    out
}

/// A helyi "YYYY-MM-DD HH:MM:SS" vagy RFC 3339 időt STIX-időbélyeggé (UTC, ezredmásodperc) alakítja.
pub fn to_stix_time(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let utc: DateTime<Utc> = DateTime::parse_from_rfc3339(value)
        .map(|d| d.with_timezone(&Utc))
        .ok()
        .or_else(|| {
            NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
                .ok()
                .and_then(|n| Local.from_local_datetime(&n).single())
                .map(|d| d.with_timezone(&Utc))
        })?;
    Some(utc.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string())
}

/// Determinisztikus UUID (5-ös verzió-jelöléssel) egy szövegből, hogy az azonos rekord
/// mindig ugyanazt az azonosítót kapja.
pub fn stable_uuid(seed: &str) -> String {
    let digest = Sha256::digest(seed.as_bytes());
    let mut b = [0u8; 16];
    b.copy_from_slice(&digest[..16]);
    b[6] = (b[6] & 0x0f) | 0x50;
    b[8] = (b[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    )
}

pub fn to_stix(records: &[ExportRecord], metadata: &ExportMetadata) -> Result<String, String> {
    let now = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
    let identity_id = format!("identity--{}", stable_uuid("irbr-identity"));
    let mut objects: Vec<Value> = vec![json!({
        "type": "identity",
        "spec_version": "2.1",
        "id": identity_id,
        "created": now,
        "modified": now,
        "name": "IRBR",
        "identity_class": "system",
    })];

    for record in records {
        let url_value = if record.url.trim().is_empty() { &record.original_url } else { &record.url };
        if url_value.trim().is_empty() {
            continue;
        }
        let url_id = format!("url--{}", stable_uuid(&format!("url:{url_value}")));
        let collected = to_stix_time(&record.collected_at).unwrap_or_else(|| now.clone());
        let first = to_stix_time(&record.observed_at)
            .or_else(|| to_stix_time(&record.published_at))
            .unwrap_or_else(|| collected.clone());
        let (first, last) = if first <= collected { (first, collected) } else { (collected.clone(), first) };

        objects.push(json!({
            "type": "url",
            "spec_version": "2.1",
            "id": url_id,
            "value": url_value,
        }));

        let mut external = json!({
            "source_name": "IRBR",
            "description": record.title,
            "url": url_value,
        });
        if !record.hash.is_empty() {
            external["hashes"] = json!({ "SHA-256": record.hash });
        }

        objects.push(json!({
            "type": "observed-data",
            "spec_version": "2.1",
            "id": format!("observed-data--{}", stable_uuid(&format!("obs:{}:{}:{}", record.id, url_value, last))),
            "created_by_ref": identity_id,
            "created": now,
            "modified": now,
            "first_observed": first,
            "last_observed": last,
            "number_observed": 1,
            "object_refs": [url_id],
            "external_references": [external],
            "labels": [format!("tier-{}", record.tier), record.source_status],
        }));
    }

    serde_json::to_string_pretty(&json!({
        "type": "bundle",
        "id": format!("bundle--{}", stable_uuid(&format!("bundle:{}:{}", metadata.case_title, metadata.generated_at))),
        "objects": objects,
    }))
    .map_err(|e| e.to_string())
}

pub fn render(format: &str, records: &[ExportRecord], metadata: &ExportMetadata) -> Result<(String, &'static str), String> {
    match format {
        "json" => Ok((to_json(records, metadata)?, "json")),
        "csv" => Ok((to_csv(records, metadata), "csv")),
        "html" => Ok((to_html(records, metadata), "html")),
        "stix" => Ok((to_stix(records, metadata)?, "stix.json")),
        _ => Err("Ismeretlen exportformátum. Használható: json, csv, html, stix.".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta() -> ExportMetadata {
        ExportMetadata {
            generated_at: "2026-07-01 10:00:00".into(),
            case_title: "Teszt <ügy>".into(),
            total: 1,
            exported: 1,
            truncated: false,
            filters: "-".into(),
        }
    }

    fn record() -> ExportRecord {
        ExportRecord {
            id: 7,
            title: "=HYPERLINK(\"x\")".into(),
            url: "https://example.hu/a?b=1".into(),
            tier: 1,
            hash: "ab".repeat(32),
            collected_at: "2026-07-01 09:00:00".into(),
            published_at: "2026-06-01T00:00:00Z".into(),
            ..Default::default()
        }
    }

    #[test]
    fn csv_guards_formulas_and_quotes() {
        assert_eq!(csv_cell("=1+1"), "\"'=1+1\"");
        assert_eq!(csv_cell("  @x"), "\"'  @x\"");
        assert_eq!(csv_cell("a\"b"), "\"a\"\"b\"");
        assert_eq!(csv_cell("sima"), "\"sima\"");
        let csv = to_csv(&[record()], &meta());
        assert!(csv.starts_with('\u{feff}'));
        assert!(csv.contains("\"'=HYPERLINK("));
    }

    #[test]
    fn html_is_escaped() {
        let html = to_html(&[record()], &meta());
        assert!(html.contains("Teszt &lt;ügy&gt;"));
        assert!(!html.contains("<ügy>"));
        assert!(html.contains("=HYPERLINK(&quot;x&quot;)"));
    }

    #[test]
    fn stix_bundle_is_well_formed() {
        let text = to_stix(&[record()], &meta()).unwrap();
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["type"], "bundle");
        let objects = v["objects"].as_array().unwrap();
        assert_eq!(objects[0]["type"], "identity");
        let observed = objects.iter().find(|o| o["type"] == "observed-data").unwrap();
        let url = objects.iter().find(|o| o["type"] == "url").unwrap();
        assert_eq!(observed["object_refs"][0], url["id"]);
        assert!(observed["first_observed"].as_str().unwrap() <= observed["last_observed"].as_str().unwrap());
        assert!(observed["external_references"][0]["hashes"]["SHA-256"].is_string());
    }

    #[test]
    fn stable_uuid_is_deterministic_and_versioned() {
        let a = stable_uuid("x");
        assert_eq!(a, stable_uuid("x"));
        assert_ne!(a, stable_uuid("y"));
        assert_eq!(a.chars().nth(14), Some('5'));
    }

    #[test]
    fn render_rejects_unknown_format() {
        assert!(render("pdf", &[], &meta()).is_err());
        assert_eq!(render("stix", &[], &meta()).unwrap().1, "stix.json");
    }
}
