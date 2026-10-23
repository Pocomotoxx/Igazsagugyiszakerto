//! Magyar hírcsatornák (RSS/Atom) valódi lekérése és szűrése a keresőkifejezésre.
//! A találat időpontja a csatorna saját közzétételi ideje.

use crate::platforms::ParsedQuery;
use crate::{provenance, story};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct Feed {
    pub label: &'static str,
    pub url: &'static str,
}

pub const HU_FEEDS: &[Feed] = &[
    Feed { label: "Telex", url: "https://telex.hu/rss" },
    Feed { label: "444.hu", url: "https://444.hu/feed" },
    Feed { label: "HVG", url: "https://hvg.hu/rss" },
    Feed { label: "Index", url: "https://index.hu/24ora/rss" },
    Feed { label: "24.hu", url: "https://24.hu/feed/" },
    Feed { label: "Portfolio", url: "https://portfolio.hu/rss/all.xml" },
    Feed { label: "Népszava", url: "https://nepszava.hu/rss" },
    Feed { label: "Világgazdaság", url: "https://www.vg.hu/feed" },
    Feed { label: "ATV", url: "https://www.atv.hu/rss" },
    Feed { label: "Origo", url: "https://api.origo.hu/publicapi/hu/rss/origo/itthon/articles" },
];

const MAX_FEED_BYTES: u64 = 3 * 1024 * 1024;
const MAX_ITEMS_PER_FEED: usize = 200;
const CACHE_TTL: Duration = Duration::from_secs(600);
pub const MAX_RESULTS: usize = 25;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FeedItem {
    pub feed: String,
    pub title: String,
    pub link: String,
    pub summary: String,
    /// UTC RFC 3339, ha a csatorna érvényes időt adott.
    pub published_at: String,
}

fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let tail = &rest[start..];
        if let Some(end) = tail.find(';').filter(|e| *e <= 10) {
            let entity = &tail[1..end];
            let decoded = match entity {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some(' '),
                _ => entity
                    .strip_prefix("#x")
                    .and_then(|h| u32::from_str_radix(h, 16).ok())
                    .or_else(|| entity.strip_prefix('#').and_then(|d| d.parse().ok()))
                    .and_then(char::from_u32),
            };
            if let Some(c) = decoded {
                out.push(c);
                rest = &tail[end + 1..];
                continue;
            }
        }
        out.push('&');
        rest = &tail[1..];
    }
    out.push_str(rest);
    out
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
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Az adott címke (`<tag ...>...</tag>`) tartalmai.
fn blocks<'a>(xml: &'a str, tag: &str) -> Vec<&'a str> {
    let lower = xml.to_ascii_lowercase();
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut out = Vec::new();
    let mut cursor = 0;
    while let Some(pos) = lower[cursor..].find(&open) {
        let start = cursor + pos;
        // pontos címkenév: a nyitás után szóköz, `>` vagy `/` kell
        let after = lower.as_bytes().get(start + open.len()).copied().unwrap_or(b'>');
        if !(after == b'>' || after == b' ' || after == b'/' || after == b'\n' || after == b'\r' || after == b'\t') {
            cursor = start + open.len();
            continue;
        }
        let Some(gt) = lower[start..].find('>') else { break };
        let content_start = start + gt + 1;
        if lower[start..start + gt].ends_with('/') {
            cursor = content_start;
            continue;
        }
        let Some(end_rel) = lower[content_start..].find(&close) else { break };
        out.push(&xml[content_start..content_start + end_rel]);
        cursor = content_start + end_rel + close.len();
    }
    out
}

fn tag_text(block: &str, tag: &str) -> String {
    let Some(inner) = blocks(block, tag).into_iter().next() else {
        return String::new();
    };
    let inner = inner.trim();
    let inner = inner
        .strip_prefix("<![CDATA[")
        .and_then(|v| v.strip_suffix("]]>"))
        .map(str::to_string)
        .unwrap_or_else(|| decode_entities(inner));
    inner.trim().to_string()
}

fn atom_link(block: &str) -> String {
    let lower = block.to_ascii_lowercase();
    let mut cursor = 0;
    while let Some(pos) = lower[cursor..].find("<link") {
        let start = cursor + pos;
        let Some(end_rel) = lower[start..].find('>') else { break };
        let tag = &block[start..start + end_rel + 1];
        cursor = start + end_rel + 1;
        let tag_lower = tag.to_ascii_lowercase();
        if tag_lower.contains("rel=\"self\"") || tag_lower.contains("rel='self'") {
            continue;
        }
        if let Some(h) = tag_lower.find("href=") {
            let rest = &tag[h + 5..];
            if let Some(quote) = rest.chars().next().filter(|q| *q == '"' || *q == '\'') {
                if let Some(end) = rest[1..].find(quote) {
                    return decode_entities(&rest[1..1 + end]);
                }
            }
        }
    }
    String::new()
}

pub fn parse_feed(xml: &str, feed_label: &str, now: chrono::DateTime<chrono::Utc>) -> Vec<FeedItem> {
    let mut entries = blocks(xml, "item");
    let atom = entries.is_empty();
    if atom {
        entries = blocks(xml, "entry");
    }
    entries
        .into_iter()
        .take(MAX_ITEMS_PER_FEED)
        .filter_map(|block| {
            let title = strip_tags(&tag_text(block, "title"));
            let link = if atom {
                atom_link(block)
            } else {
                let l = tag_text(block, "link");
                if l.is_empty() { tag_text(block, "guid") } else { l }
            };
            if title.is_empty() || !(link.starts_with("http://") || link.starts_with("https://")) {
                return None;
            }
            let summary_raw = [
                tag_text(block, "description"),
                tag_text(block, "summary"),
                tag_text(block, "content:encoded"),
                tag_text(block, "content"),
            ]
            .into_iter()
            .find(|v| !v.is_empty())
            .unwrap_or_default();
            let summary: String = strip_tags(&summary_raw).chars().take(500).collect();
            let date_raw = [
                tag_text(block, "pubdate"),
                tag_text(block, "published"),
                tag_text(block, "dc:date"),
                tag_text(block, "updated"),
            ]
            .into_iter()
            .find(|v| !v.is_empty())
            .unwrap_or_default();
            Some(FeedItem {
                feed: feed_label.to_string(),
                title,
                link,
                summary,
                published_at: provenance::normalize_time(&date_raw, now).unwrap_or_default(),
            })
        })
        .collect()
}

/// Egy hír megfelel-e a kifejezésnek (kis/nagybetű- és ékezetfüggetlen, részszó-egyezés).
/// Minden kötelező szó és kifejezés kell, minden csoportból legalább egy szó, kizárt szó nem.
/// A kifejezésben megadott évtartományon kívüli, ismert dátumú hír nem felel meg.
pub fn matches(item: &FeedItem, parsed: &ParsedQuery) -> bool {
    let haystack = story::fold(&format!("{} {}", item.title, item.summary));
    let contains = |needle: &str| {
        let n = story::fold(needle);
        !n.is_empty() && haystack.contains(&n)
    };
    if !parsed.must.iter().all(|w| contains(w)) || !parsed.phrases.iter().all(|p| contains(p)) {
        return false;
    }
    if !parsed.any.iter().all(|group| group.iter().any(|w| contains(w))) {
        return false;
    }
    if parsed.exclude.iter().any(|w| contains(w)) {
        return false;
    }
    if let Some(year) = item.published_at.get(0..4).and_then(|y| y.parse::<i32>().ok()) {
        if parsed.year_from.map_or(false, |f| year < f) || parsed.year_to.map_or(false, |t| year > t) {
            return false;
        }
    }
    true
}

type Cache = Mutex<HashMap<&'static str, (Instant, Vec<FeedItem>)>>;

fn cache() -> &'static Cache {
    static CACHE: std::sync::OnceLock<Cache> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn fetch_feed(client: &reqwest::blocking::Client, feed: &'static Feed) -> Result<Vec<FeedItem>, String> {
    if let Ok(guard) = cache().lock() {
        if let Some((at, items)) = guard.get(feed.url) {
            if at.elapsed() < CACHE_TTL {
                return Ok(items.clone());
            }
        }
    }
    let response = crate::fetchx::send_with_retry(|| client.get(feed.url), 1)
        .map_err(|e| format!("{}: {e}", feed.label))?;
    if !response.status().is_success() {
        return Err(format!("{}: HTTP {}", feed.label, response.status()));
    }
    let (bytes, _) = crate::fetchx::read_capped_strict(response, MAX_FEED_BYTES)
        .map_err(|e| format!("{}: {e}", feed.label))?;
    let xml = String::from_utf8_lossy(&bytes).into_owned();
    let items = parse_feed(&xml, feed.label, chrono::Utc::now());
    if items.is_empty() {
        return Err(format!("{}: üres vagy értelmezhetetlen csatorna", feed.label));
    }
    if let Ok(mut guard) = cache().lock() {
        guard.insert(feed.url, (Instant::now(), items.clone()));
    }
    Ok(items)
}

pub struct RssOutcome {
    pub items: Vec<FeedItem>,
    pub errors: Vec<String>,
}

/// Lekéri az összes csatornát párhuzamosan, és a kifejezésre szűr; legfeljebb `MAX_RESULTS`
/// hír, a legújabbak elöl.
pub fn search(parsed: &ParsedQuery) -> Result<RssOutcome, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(12))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) IRBR-rss/0.1")
        .build()
        .map_err(|e| format!("HTTP kliens inicializálási hiba: {e}"))?;

    let outcomes: Vec<Result<Vec<FeedItem>, String>> = std::thread::scope(|scope| {
        let handles: Vec<_> = HU_FEEDS
            .iter()
            .map(|feed| {
                let client = &client;
                scope.spawn(move || fetch_feed(client, feed))
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or_else(|_| Err("belső hiba".to_string())))
            .collect()
    });

    let mut items = Vec::new();
    let mut errors = Vec::new();
    let mut feeds_ok = 0;
    for outcome in outcomes {
        match outcome {
            Ok(list) => {
                feeds_ok += 1;
                items.extend(list.into_iter().filter(|item| matches(item, parsed)));
            }
            Err(error) => errors.push(error),
        }
    }
    if feeds_ok == 0 {
        return Err(format!(
            "Egyik hírcsatorna sem érhető el. {}",
            errors.first().cloned().unwrap_or_default()
        ));
    }
    items.sort_by(|a, b| b.published_at.cmp(&a.published_at));
    items.dedup_by(|a, b| a.link == b.link);
    items.truncate(MAX_RESULTS);
    Ok(RssOutcome { items, errors })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platforms::parse_query;
    use chrono::TimeZone;

    fn now() -> chrono::DateTime<chrono::Utc> {
        chrono::Utc.with_ymd_and_hms(2026, 7, 1, 0, 0, 0).unwrap()
    }

    const RSS: &str = r#"<?xml version="1.0"?><rss><channel><title>Teszt</title>
        <item><title><![CDATA[Emelkedik a szakértői óradíj]]></title><link>https://x.hu/a</link>
          <description>&lt;p&gt;Új rendelet az &amp;igazságügyi szakértők díjáról&lt;/p&gt;</description>
          <pubDate>Wed, 04 Mar 2026 10:20:30 GMT</pubDate></item>
        <item><title>Időjárás: hidegfront</title><link>https://x.hu/b</link><description>eső</description>
          <pubDate>Tue, 03 Mar 2026 08:00:00 GMT</pubDate></item>
        <item><title>Régi hír</title><link>https://x.hu/c</link><description>szakértői óradíj 2019-ben</description>
          <pubDate>Mon, 01 Jan 2018 08:00:00 GMT</pubDate></item></channel></rss>"#;

    const ATOM: &str = r#"<feed xmlns="http://www.w3.org/2005/Atom"><entry><title>Atom cím</title>
        <link rel="self" href="https://x.hu/self"/><link rel="alternate" href="https://x.hu/atom?a=1&amp;b=2"/>
        <summary>összefoglaló</summary><updated>2026-02-01T08:00:00Z</updated></entry></feed>"#;

    #[test]
    fn parses_rss_with_cdata_entities_and_dates() {
        let items = parse_feed(RSS, "Teszt", now());
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].title, "Emelkedik a szakértői óradíj");
        assert_eq!(items[0].link, "https://x.hu/a");
        assert!(items[0].summary.contains("Új rendelet az") && !items[0].summary.contains('<'));
        assert_eq!(items[0].published_at, "2026-03-04T10:20:30Z");
    }

    #[test]
    fn parses_atom_links_and_skips_self() {
        let items = parse_feed(ATOM, "Atom", now());
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].link, "https://x.hu/atom?a=1&b=2");
        assert_eq!(items[0].published_at, "2026-02-01T08:00:00Z");
    }

    #[test]
    fn matching_is_accent_insensitive_and_honours_years_and_excludes() {
        let items = parse_feed(RSS, "Teszt", now());
        let p = parse_query("szakertoi oradij");
        let hits: Vec<_> = items.iter().filter(|i| matches(i, &p)).collect();
        assert_eq!(hits.len(), 2);

        let p = parse_query("szakértői óradíj 2025..2026");
        let hits: Vec<_> = items.iter().filter(|i| matches(i, &p)).collect();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].link, "https://x.hu/a");

        let p = parse_query("óradíj -rendelet");
        assert!(items.iter().filter(|i| matches(i, &p)).all(|i| i.link != "https://x.hu/a"));

        let p = parse_query("\"igazságügyi szakértők\"");
        assert_eq!(items.iter().filter(|i| matches(i, &p)).count(), 1);
    }

    #[test]
    fn entity_decoding_handles_numeric_and_unknown() {
        assert_eq!(decode_entities("a &#8217; b &#x41; &unknown; &amp;"), "a ’ b A &unknown; &");
    }
}
