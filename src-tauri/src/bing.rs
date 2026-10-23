//! Kulcs nélküli tartalék a Bing HTML-eredményoldalából (a DuckDuckGo mellett).

use crate::ddg::DdgHit;

const MAX_BYTES: u64 = 2 * 1024 * 1024;

fn decode_entities(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#0183;", "·")
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

fn base64url_decode(input: &str) -> Option<Vec<u8>> {
    let mut buffer: u32 = 0;
    let mut bits = 0;
    let mut out = Vec::new();
    for c in input.chars() {
        let value = match c {
            'A'..='Z' => c as u32 - 'A' as u32,
            'a'..='z' => c as u32 - 'a' as u32 + 26,
            '0'..='9' => c as u32 - '0' as u32 + 52,
            '-' | '+' => 62,
            '_' | '/' => 63,
            '=' => break,
            _ => return None,
        };
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buffer >> bits) & 0xff) as u8);
        }
    }
    Some(out)
}

/// A Bing átirányító linkjéből (`/ck/a?...&u=a1<base64url>`) a valódi cél; közvetlen linket változatlanul ad.
pub fn resolve_href(href: &str) -> Option<String> {
    let href = decode_entities(href);
    let href = href.trim();
    if href.contains("bing.com/ck/a") {
        let query = href.split_once('?')?.1;
        let u = query
            .split('&')
            .find_map(|pair| pair.strip_prefix("u="))?;
        let encoded = u.strip_prefix("a1")?;
        let bytes = base64url_decode(encoded)?;
        let url = String::from_utf8(bytes).ok()?;
        return (url.starts_with("http://") || url.starts_with("https://")).then_some(url);
    }
    if (href.starts_with("http://") || href.starts_with("https://")) && !href.contains("bing.com/") {
        Some(href.to_string())
    } else {
        None
    }
}

pub fn is_blocked_page(html: &str) -> bool {
    html.contains("id=\"b_captcha") || html.contains("/turing/captcha") || html.contains("Our systems have detected unusual traffic")
}

/// Az `b_algo` találatblokkok: a `<h2><a href>` cím + cél és a `b_caption` bekezdés.
pub fn parse_results(html: &str) -> Vec<DdgHit> {
    let mut hits = Vec::new();
    let mut cursor = 0;
    while let Some(pos) = html[cursor..].find("b_algo") {
        let block_start = cursor + pos;
        let block_end = html[block_start + 6..]
            .find("b_algo")
            .map(|p| block_start + 6 + p)
            .unwrap_or(html.len());
        let block = &html[block_start..block_end];
        cursor = block_end.max(block_start + 6);

        let Some(h2) = block.find("<h2") else { continue };
        let Some(a_rel) = block[h2..].find("<a ") else { continue };
        let a_start = h2 + a_rel;
        let Some(tag_end_rel) = block[a_start..].find('>') else { continue };
        let tag = &block[a_start..a_start + tag_end_rel];
        let Some(close_rel) = block[a_start..].find("</a>") else { continue };
        let title = strip_tags(&block[a_start + tag_end_rel + 1..a_start + close_rel]);

        let href = tag.find("href=").and_then(|h| {
            let rest = &tag[h + 5..];
            let quote = rest.chars().next().filter(|q| *q == '"' || *q == '\'')?;
            let end = rest[1..].find(quote)?;
            Some(rest[1..1 + end].to_string())
        });
        let Some(url) = href.as_deref().and_then(resolve_href) else { continue };

        let snippet = block
            .find("<p")
            .and_then(|p| {
                let from = block[p..].find('>')? + p + 1;
                let to = block[from..].find("</p>")?;
                Some(strip_tags(&block[from..from + to]))
            })
            .unwrap_or_default();

        if !title.is_empty() {
            hits.push(DdgHit { title, url, snippet });
        }
    }
    hits
}

pub fn search(client: &reqwest::blocking::Client, query: &str) -> Result<Vec<DdgHit>, String> {
    crate::ddg::throttle();
    let url = format!(
        "https://www.bing.com/search?q={}&setlang=hu&cc=HU&mkt=hu-HU&count=20",
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
    .map_err(|e| format!("Bing hívás sikertelen: {e}"))?;
    let status = response.status();
    let (bytes, _) = crate::fetchx::read_capped_strict(response, MAX_BYTES)
        .map_err(|e| format!("Bing válasz olvasási hiba: {e}"))?;
    let html = String::from_utf8_lossy(&bytes).into_owned();
    if is_blocked_page(&html) {
        return Err("A Bing ellenőrzést kért (captcha) a kulcs nélküli lekéréshez.".to_string());
    }
    if !status.is_success() {
        return Err(format!("Bing hiba: HTTP {status}"));
    }
    Ok(parse_results(&html))
}

#[cfg(test)]
mod tests {
    use super::*;

    // "https://www.example.hu/takaritas?a=1" base64url = aHR0cHM6Ly93d3cuZXhhbXBsZS5odS90YWthcml0YXM_YT0x
    const HTML: &str = r#"<ol id="b_results">
    <li class="b_algo" data-id><h2><a href="https://www.bing.com/ck/a?!&amp;&amp;p=abc&amp;u=a1aHR0cHM6Ly93d3cuZXhhbXBsZS5odS90YWthcml0YXM_YT0x&amp;ntb=1" h="ID=SERP,5000.1">Takarítás <strong>árak</strong> Eger</a></h2>
      <div class="b_caption"><p>Az <strong>óradíj</strong> 1500 Ft &amp; több.</p></div></li>
    <li class="b_algo"><h2><a href="https://njt.hu/jogszabaly/1">Jogszabály</a></h2><div class="b_caption"><p>Második.</p></div></li>
    <li class="b_ad"><h2><a href="https://www.bing.com/aclk?x">Hirdetés</a></h2></li>
    </ol>"#;

    #[test]
    fn decodes_redirect_links_and_direct_links() {
        let hits = parse_results(HTML);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].title, "Takarítás árak Eger");
        assert_eq!(hits[0].url, "https://www.example.hu/takaritas?a=1");
        assert_eq!(hits[0].snippet, "Az óradíj 1500 Ft & több.");
        assert_eq!(hits[1].url, "https://njt.hu/jogszabaly/1");
    }

    #[test]
    fn base64url_roundtrip_and_rejects_garbage() {
        assert_eq!(base64url_decode("aGk").unwrap(), b"hi");
        assert!(base64url_decode("a$b").is_none());
        assert!(resolve_href("https://www.bing.com/ck/a?u=a1!!!").is_none());
        assert!(resolve_href("javascript:alert(1)").is_none());
    }

    #[test]
    fn detects_captcha() {
        assert!(is_blocked_page("<div id=\"b_captcha\">"));
        assert!(!is_blocked_page(HTML));
    }
}
