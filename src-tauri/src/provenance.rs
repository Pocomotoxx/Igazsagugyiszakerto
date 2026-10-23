//! Szolgáltatói idő és frissesség. Alapelv: a forrás saját időbélyege számít, a begyűjtés
//! ideje sosem helyettesíti a közzététel idejét. Ha nincs szolgáltatói idő, a frissesség
//! "ismeretlen".

use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};

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
    fn freshness_uses_provider_time_only() {
        assert_eq!(freshness_label(3, "", now()), "ismeretlen");
        assert_eq!(freshness_label(3, "2026-06-01T00:00:00Z", now()), "friss");
        assert_eq!(freshness_label(3, "2025-01-01T00:00:00Z", now()), "elavult");
        // hivatalos forrásnál tágabb a küszöb
        assert_eq!(freshness_label(1, "2025-01-01T00:00:00Z", now()), "friss");
    }
}
