//! Szolgáltatói idő és frissesség. Alapelv: a forrás saját időbélyege számít, a begyűjtés
//! ideje sosem helyettesíti a közzététel idejét. Ha nincs szolgáltatói idő, a frissesség
//! "ismeretlen".

use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use serde::Serialize;

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct PageTimes {
    pub published_at: String,
    pub modified_at: String,
    /// Honnan származik: "meta", "header", "provider" vagy üres.
    pub time_source: String,
}

/// Különféle időformátumokat egységes, UTC RFC 3339 szöveggé alakít. Érvénytelen vagy
/// jövőbeli (több mint egy napnál későbbi) érték esetén `None`.
pub fn normalize_time(raw: &str, now: DateTime<Utc>) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() || raw.len() > 64 {
        return None;
    }
    let parsed: Option<DateTime<Utc>> = DateTime::parse_from_rfc3339(raw)
        .map(|d| d.with_timezone(&Utc))
        .ok()
        .or_else(|| DateTime::parse_from_rfc2822(raw).map(|d| d.with_timezone(&Utc)).ok())
        .or_else(|| {
            NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S")
                .or_else(|_| NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S"))
                .ok()
                .map(|n| Utc.from_utc_datetime(&n))
        })
        .or_else(|| {
            NaiveDate::parse_from_str(raw, "%Y-%m-%d")
                .ok()
                .and_then(|d| d.and_hms_opt(0, 0, 0))
                .map(|n| Utc.from_utc_datetime(&n))
        });
    let parsed = parsed?;
    if parsed > now + chrono::Duration::days(1) {
        return None;
    }
    Some(parsed.format("%Y-%m-%dT%H:%M:%SZ").to_string())
}

/// Egy HTML-tag adott attribútumának értéke (kisbetűs vizsgálat, idézőjeles érték).
fn attr_value(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let needle = format!("{name}=");
    let start = lower.find(&needle)? + needle.len();
    let rest = &tag[start..];
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let inner = &rest[1..];
    let end = inner.find(quote)?;
    Some(inner[..end].to_string())
}

const PUBLISHED_KEYS: &[&str] = &[
    "article:published_time",
    "og:published_time",
    "datepublished",
    "date",
    "dc.date",
    "dc.date.issued",
    "pubdate",
    "publishdate",
    "parsely-pub-date",
];
const MODIFIED_KEYS: &[&str] = &[
    "article:modified_time",
    "og:updated_time",
    "datemodified",
    "last-modified",
    "dc.date.modified",
];

/// Közzétételi és módosítási idő a `<meta>` címkékből, a `<time datetime>` elemből
/// és a `Last-Modified` fejlécből.
pub fn extract_page_times(html: &str, last_modified_header: &str, now: DateTime<Utc>) -> PageTimes {
    let lower = html.to_ascii_lowercase();
    let mut published = String::new();
    let mut modified = String::new();

    let mut cursor = 0;
    while let Some(pos) = lower[cursor..].find("<meta") {
        let start = cursor + pos;
        let Some(end_rel) = lower[start..].find('>') else { break };
        let tag = &html[start..start + end_rel + 1];
        cursor = start + end_rel + 1;

        let key = attr_value(tag, "property")
            .or_else(|| attr_value(tag, "name"))
            .or_else(|| attr_value(tag, "itemprop"))
            .unwrap_or_default()
            .to_ascii_lowercase();
        let Some(content) = attr_value(tag, "content") else { continue };
        let Some(normalized) = normalize_time(&content, now) else { continue };
        if published.is_empty() && PUBLISHED_KEYS.contains(&key.as_str()) {
            published = normalized;
        } else if modified.is_empty() && MODIFIED_KEYS.contains(&key.as_str()) {
            modified = normalized;
        }
    }

    if published.is_empty() {
        if let Some(pos) = lower.find("<time") {
            if let Some(end_rel) = lower[pos..].find('>') {
                let tag = &html[pos..pos + end_rel + 1];
                if let Some(value) = attr_value(tag, "datetime").and_then(|v| normalize_time(&v, now)) {
                    published = value;
                }
            }
        }
    }

    let header_time = normalize_time(last_modified_header, now).unwrap_or_default();
    let mut source = String::new();
    if !published.is_empty() || !modified.is_empty() {
        source = "meta".to_string();
    }
    if modified.is_empty() && !header_time.is_empty() {
        modified = header_time;
        if source.is_empty() {
            source = "header".to_string();
        }
    }

    PageTimes {
        published_at: published,
        modified_at: modified,
        time_source: source,
    }
}

/// Elavulási küszöb napokban a találat tier-je szerint.
pub fn max_age_days(tier: i32) -> i64 {
    match tier {
        1 => 730,
        2 => 365,
        3 => 180,
        _ => 90,
    }
}

/// "friss", "elavult" vagy "ismeretlen". Csak a szolgáltatói idő számít.
pub fn freshness_label(tier: i32, published_at: &str, now: DateTime<Utc>) -> &'static str {
    let Some(time) = DateTime::parse_from_rfc3339(published_at)
        .ok()
        .map(|d| d.with_timezone(&Utc))
    else {
        return "ismeretlen";
    };
    if now - time > chrono::Duration::days(max_age_days(tier)) {
        "elavult"
    } else {
        "friss"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 1, 0, 0, 0).unwrap()
    }

    #[test]
    fn normalizes_common_formats_and_rejects_future() {
        assert_eq!(
            normalize_time("2026-03-04T10:20:30+01:00", now()).as_deref(),
            Some("2026-03-04T09:20:30Z")
        );
        assert_eq!(normalize_time("2026-03-04", now()).as_deref(), Some("2026-03-04T00:00:00Z"));
        assert_eq!(
            normalize_time("Wed, 04 Mar 2026 10:20:30 GMT", now()).as_deref(),
            Some("2026-03-04T10:20:30Z")
        );
        assert_eq!(normalize_time("2030-01-01", now()), None);
        assert_eq!(normalize_time("holnap", now()), None);
    }

    #[test]
    fn extracts_meta_times() {
        let html = r#"<html><head>
            <meta property="article:published_time" content="2026-02-01T08:00:00Z">
            <meta property="article:modified_time" content="2026-02-03T09:00:00Z">
            </head><body></body></html>"#;
        let t = extract_page_times(html, "", now());
        assert_eq!(t.published_at, "2026-02-01T08:00:00Z");
        assert_eq!(t.modified_at, "2026-02-03T09:00:00Z");
        assert_eq!(t.time_source, "meta");
    }

    #[test]
    fn falls_back_to_time_element_and_header() {
        let html = r#"<body><time datetime="2026-01-05">jan 5</time></body>"#;
        let t = extract_page_times(html, "Wed, 04 Mar 2026 10:20:30 GMT", now());
        assert_eq!(t.published_at, "2026-01-05T00:00:00Z");
        assert_eq!(t.modified_at, "2026-03-04T10:20:30Z");

        let none = extract_page_times("<p>semmi</p>", "", now());
        assert_eq!(none, PageTimes::default());
    }

    #[test]
    fn freshness_uses_provider_time_only() {
        assert_eq!(freshness_label(3, "", now()), "ismeretlen");
        assert_eq!(freshness_label(3, "2026-06-01T00:00:00Z", now()), "friss");
        assert_eq!(freshness_label(3, "2025-01-01T00:00:00Z", now()), "elavult");
        // hivatalos forrásnál tágabb a küszöb
        assert_eq!(freshness_label(1, "2025-01-01T00:00:00Z", now()), "friss");
    }
}
