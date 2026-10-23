//! Kulcs nélküli webes keresés a DuckDuckGo HTML-oldalán keresztül. Tartalék, ha nincs
//! beállított keresőszolgáltatás (Tavily, Brave, Google CSE): a lekérések szándékosan
//! lassúak (időközzel), és a korlátozást a program hibaként jelzi, nem titkolja.

use std::sync::Mutex;
use std::time::{Duration, Instant};

const MIN_GAP: Duration = Duration::from_millis(700);
const MAX_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, PartialEq, Clone)]
pub struct DdgHit {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

pub fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = &input[i + 1..i + 3];
                if let Ok(v) = u8::from_str_radix(hex, 16) {
                    out.push(v);
                    i += 3;
                    continue;
                }
                out.push(b'%');
                i += 1;
            }
            b'+' => {
                out.push(b' ');
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

fn decode_entities(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

fn strip_tags(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    decode_entities(&out).split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A DuckDuckGo átirányító linkjéből (`//duckduckgo.com/l/?uddg=...`) a valódi cél.
pub fn resolve_href(href: &str) -> Option<String> {
    let href = decode_entities(href);
    let href = href.trim();
    if let Some(pos) = href.find("uddg=") {
        let rest = &href[pos + 5..];
        let encoded = rest.split('&').next().unwrap_or(rest);
        let decoded = percent_decode(encoded);
        if decoded.starts_with("http://") || decoded.starts_with("https://") {
            return Some(decoded);
        }
        return None;
    }
    let full = if let Some(rest) = href.strip_prefix("//") {
        format!("https://{rest}")
    } else {
        href.to_string()
    };
    if (full.starts_with("http://") || full.starts_with("https://"))
        && !full.contains("duckduckgo.com/y.js")
        && !full.contains("duckduckgo.com/l/")
    {
        Some(full)
    } else {
        None
    }
}

pub fn is_blocked_page(html: &str) -> bool {
    html.contains("anomaly-modal")
        || html.contains("bots use DuckDuckGo too")
        || html.contains("anomaly.js")
}

/// A találatok kinyerése: `result__a` hivatkozás (cím + cél) és a hozzá tartozó `result__snippet`.
pub fn parse_results(html: &str) -> Vec<DdgHit> {
    let mut hits = Vec::new();
    let mut cursor = 0;
    while let Some(pos) = html[cursor..].find("result__a") {
        let class_pos = cursor + pos;
        // a hivatkozás nyitó címkéje
        let Some(tag_start) = html[..class_pos].rfind("<a") else {
            cursor = class_pos + 9;
            continue;
        };
        let Some(tag_end_rel) = html[tag_start..].find('>') else { break };
        let tag_end = tag_start + tag_end_rel;
        let tag = &html[tag_start..=tag_end];
        let Some(close_rel) = html[tag_end..].find("</a>") else { break };
        let title = strip_tags(&html[tag_end + 1..tag_end + close_rel]);
        let next_result = html[tag_end + close_rel..]
            .find("result__a")
            .map(|p| tag_end + close_rel + p)
            .unwrap_or(html.len());
        let after = &html[tag_end + close_rel..next_result];

        let snippet = after
            .find("result__snippet")
            .and_then(|sp| {
                let from = after[sp..].find('>')? + sp + 1;
                let to = after[from..].find("</a>").or_else(|| after[from..].find("</div>"))?;
                Some(strip_tags(&after[from..from + to]))
            })
            .unwrap_or_default();

        let href = tag.find("href=").and_then(|h| {
            let rest = &tag[h + 5..];
            let quote = rest.chars().next().filter(|q| *q == '"' || *q == '\'')?;
            let end = rest[1..].find(quote)?;
            Some(rest[1..1 + end].to_string())
        });
        if let Some(url) = href.as_deref().and_then(resolve_href) {
            if !title.is_empty() {
                hits.push(DdgHit { title, url, snippet });
            }
        }
        cursor = next_result.max(class_pos + 9);
        if next_result == html.len() {
            break;
        }
    }
    hits
}

pub fn throttle() {
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    if let Ok(mut last) = LAST.lock() {
        if let Some(at) = *last {
            let elapsed = at.elapsed();
            if elapsed < MIN_GAP {
                std::thread::sleep(MIN_GAP - elapsed);
            }
        }
        *last = Some(Instant::now());
    }
}

const BLOCK_COOLDOWN: Duration = Duration::from_secs(600);
const BLOCKED_MESSAGE: &str = "A DuckDuckGo ideiglenesen korlátozta a kulcs nélküli lekéréseket. Állíts be egy ingyenes Tavily-kulcsot a megbízható működéshez.";

fn blocked_until() -> &'static Mutex<Option<Instant>> {
    static BLOCKED: Mutex<Option<Instant>> = Mutex::new(None);
    &BLOCKED
}

/// Egy kulcs nélküli keresés. A lekérdezés tartalmazhat `site:` operátort.
/// Ha a DuckDuckGo korlátoz, tíz percig nem próbálkozunk újra (nem terheljük feleslegesen).
pub fn search(client: &reqwest::blocking::Client, query: &str) -> Result<Vec<DdgHit>, String> {
    if let Ok(guard) = blocked_until().lock() {
        if guard.map_or(false, |at| at.elapsed() < BLOCK_COOLDOWN) {
            return Err(BLOCKED_MESSAGE.to_string());
        }
    }
    throttle();
    let url = format!(
        "https://html.duckduckgo.com/html/?q={}&kl=hu-hu",
        crate::url_encode(query)
    );
    let response = crate::fetchx::send_with_retry(
        || {
            client
                .get(&url)
                .header("Accept-Language", "hu-HU,hu;q=0.9,en;q=0.5")
                .header("Accept", "text/html,application/xhtml+xml")
        },
        1,
    )
    .map_err(|e| format!("DuckDuckGo hívás sikertelen: {e}"))?;
    let status = response.status();
    let (bytes, _) = crate::fetchx::read_capped_strict(response, MAX_BYTES)
        .map_err(|e| format!("DuckDuckGo válasz olvasási hiba: {e}"))?;
    let html = String::from_utf8_lossy(&bytes).into_owned();
    if is_blocked_page(&html) || status.as_u16() == 202 {
        if let Ok(mut guard) = blocked_until().lock() {
            *guard = Some(Instant::now());
        }
        return Err(BLOCKED_MESSAGE.to_string());
    }
    if !status.is_success() {
        return Err(format!("DuckDuckGo hiba: HTTP {status}"));
    }
    Ok(parse_results(&html))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HTML: &str = r#"<html><body>
    <div class="result results_links results_links_deep web-result">
      <div class="links_main links_deep result__body">
        <h2 class="result__title"><a rel="nofollow" class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fwww.example.hu%2Ftakaritas%3Fa%3D1%26b%3D2&amp;rut=abc">Takarítás <b>árak</b> 2026</a></h2>
        <a class="result__snippet" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fwww.example.hu%2F">Az <b>óradíjak</b> &amp; árak áttekintése.</a>
      </div>
    </div>
    <div class="result result--ad">
      <h2 class="result__title"><a class="result__a" href="https://duckduckgo.com/y.js?ad_provider=x">Hirdetés</a></h2>
    </div>
    <div class="result results_links web-result">
      <h2 class="result__title"><a class="result__a" href="https://njt.hu/jogszabaly/2000-1-00-00">Jogszabály</a></h2>
      <a class="result__snippet" href="https://njt.hu/">Második <b>találat</b></a>
    </div></body></html>"#;

    #[test]
    fn parses_titles_urls_and_snippets_and_skips_ads() {
        let hits = parse_results(HTML);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].title, "Takarítás árak 2026");
        assert_eq!(hits[0].url, "https://www.example.hu/takaritas?a=1&b=2");
        assert_eq!(hits[0].snippet, "Az óradíjak & árak áttekintése.");
        assert_eq!(hits[1].url, "https://njt.hu/jogszabaly/2000-1-00-00");
        assert_eq!(hits[1].snippet, "Második találat");
    }

    #[test]
    fn detects_the_bot_wall() {
        assert!(is_blocked_page("<div class=\"anomaly-modal__title\">Unfortunately, bots use DuckDuckGo too.</div>"));
        assert!(!is_blocked_page(HTML));
    }

    #[test]
    fn percent_decode_handles_utf8_and_bad_input() {
        assert_eq!(percent_decode("takar%C3%ADt%C3%A1s+ar"), "takarítás ar");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%zz"), "%zz");
    }

    #[test]
    fn resolve_href_rejects_non_http_and_ad_redirects() {
        assert_eq!(resolve_href("//duckduckgo.com/l/?uddg=javascript%3Aalert(1)"), None);
        assert_eq!(resolve_href("https://duckduckgo.com/y.js?x=1"), None);
        assert_eq!(resolve_href("//example.hu/a").as_deref(), Some("https://example.hu/a"));
    }
}
