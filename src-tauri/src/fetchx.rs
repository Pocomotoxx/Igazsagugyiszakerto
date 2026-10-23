//! Korlátos HTTP-lekérés: újrapróbálás átmeneti hibára, `Retry-After` tisztelete,
//! és szigorú méretkorlát, amely jelzi a csonkolást (nem csendben vág).

use std::io::Read;
use std::time::Duration;

/// Legfeljebb ennyit várunk egy újrapróbálás előtt, a szerver kérésétől függetlenül.
const MAX_RETRY_DELAY_MS: u64 = 20_000;
const BASE_RETRY_DELAY_MS: u64 = 1_500;

/// A `Retry-After` fejléc értéke ezredmásodpercben: másodpercszám vagy HTTP-dátum.
pub fn retry_after_ms(value: &str, now: chrono::DateTime<chrono::Utc>) -> Option<u64> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(seconds.saturating_mul(1000));
    }
    let when = chrono::DateTime::parse_from_rfc2822(value).ok()?;
    let delta = when.with_timezone(&chrono::Utc) - now;
    Some(delta.num_milliseconds().max(0) as u64)
}

/// Érdemes-e újra próbálni az adott státuszt?
pub fn is_retryable_status(status: u16) -> bool {
    status == 408 || status == 429 || (500..=599).contains(&status)
}

fn delay_for(attempt: u32, retry_after: Option<u64>) -> Duration {
    let backoff = BASE_RETRY_DELAY_MS * (attempt as u64 + 1);
    let ms = retry_after.unwrap_or(backoff).min(MAX_RETRY_DELAY_MS);
    Duration::from_millis(ms)
}

/// Elküldi a kérést; átmeneti hiba (hálózati hiba, időtúllépés, 408/429/5xx) esetén
/// legfeljebb `retries` újrapróbálás. A `build` minden próbához új kérést készít.
pub fn send_with_retry<F>(
    build: F,
    retries: u32,
) -> Result<reqwest::blocking::Response, reqwest::Error>
where
    F: Fn() -> reqwest::blocking::RequestBuilder,
{
    let mut attempt = 0;
    loop {
        match build().send() {
            Ok(response) => {
                let status = response.status().as_u16();
                if attempt < retries && is_retryable_status(status) {
                    let retry_after = response
                        .headers()
                        .get(reqwest::header::RETRY_AFTER)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| retry_after_ms(v, chrono::Utc::now()));
                    std::thread::sleep(delay_for(attempt, retry_after));
                    attempt += 1;
                    continue;
                }
                return Ok(response);
            }
            Err(error) => {
                let transient = error.is_timeout() || error.is_connect() || error.is_request();
                if attempt < retries && transient {
                    std::thread::sleep(delay_for(attempt, None));
                    attempt += 1;
                    continue;
                }
                return Err(error);
            }
        }
    }
}

/// Beolvassa a törzset legfeljebb `max` bájtig. A második érték igaz, ha a forrás ennél
/// hosszabb volt (a tartalom csonka).
pub fn read_capped_strict<R: Read>(reader: R, max: u64) -> std::io::Result<(Vec<u8>, bool)> {
    let mut buffer = Vec::new();
    reader.take(max + 1).read_to_end(&mut buffer)?;
    let truncated = buffer.len() as u64 > max;
    if truncated {
        buffer.truncate(max as usize);
    }
    Ok((buffer, truncated))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn retry_after_parses_seconds_and_dates() {
        let now = chrono::Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap();
        assert_eq!(retry_after_ms("7", now), Some(7000));
        assert_eq!(retry_after_ms("", now), None);
        assert_eq!(retry_after_ms("nonsense", now), None);
        assert_eq!(
            retry_after_ms("Thu, 01 Jan 2026 12:00:30 GMT", now),
            Some(30_000)
        );
        // múltbeli dátum → 0
        assert_eq!(retry_after_ms("Thu, 01 Jan 2026 11:00:00 GMT", now), Some(0));
    }

    #[test]
    fn retryable_statuses() {
        assert!(is_retryable_status(429));
        assert!(is_retryable_status(503));
        assert!(!is_retryable_status(404));
        assert!(!is_retryable_status(200));
    }

    #[test]
    fn delay_is_capped() {
        assert_eq!(delay_for(0, Some(999_999)), Duration::from_millis(MAX_RETRY_DELAY_MS));
        assert_eq!(delay_for(1, None), Duration::from_millis(BASE_RETRY_DELAY_MS * 2));
    }

    #[test]
    fn capped_read_flags_truncation() {
        let data = vec![1u8; 10];
        let (bytes, truncated) = read_capped_strict(&data[..], 10).unwrap();
        assert_eq!(bytes.len(), 10);
        assert!(!truncated);
        let (bytes, truncated) = read_capped_strict(&data[..], 4).unwrap();
        assert_eq!(bytes.len(), 4);
        assert!(truncated);
    }
}
