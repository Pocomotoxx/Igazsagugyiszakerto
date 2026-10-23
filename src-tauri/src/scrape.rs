//! Böngészős keresés: a beépített Chromium (WebView2) betölti a Google vagy a Bing
//! találati oldalát, és a találatokat a megjelenített oldalról olvassa ki. Nincs kulcs és
//! nincs külön HTTP-kliens: a keresőnek ugyanaz a böngészője látszik, amivel a felhasználó is keres.
//!
//! A találatok visszajuttatása IPC nélkül történik: az oldalba injektált szkript egy
//! `https://irbr-scrape.invalid/#<json>` címre navigál, a program ezt a navigációt elfogja
//! és megszakítja. Így a betöltött idegen oldal nem kap hozzáférést az alkalmazás parancsaihoz.

use crate::ddg::{percent_decode, DdgHit};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::Mutex;
use std::time::Duration;
use tauri::Manager;

const SENTINEL_HOST: &str = "irbr-scrape.invalid";
const QUERY_TIMEOUT: Duration = Duration::from_secs(30);
const GAP: Duration = Duration::from_millis(1200);
const MAX_HITS: usize = 15;

static COUNTER: AtomicU64 = AtomicU64::new(1);
static BATCH_LOCK: Mutex<()> = Mutex::new(());

fn sessions() -> &'static Mutex<HashMap<String, Sender<String>>> {
    static SESSIONS: std::sync::OnceLock<Mutex<HashMap<String, Sender<String>>>> = std::sync::OnceLock::new();
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// A találatokat kiolvasó szkript. A motort a hosztnévből ismeri fel, a hozzájárulási
/// oldalt elfogadja, a captchát jelzi, a találatokat pedig a sentinel-címre navigálva adja át.
fn script(session: &str) -> String {
    r##"(function () {
  if (window.top !== window) return;
  var SESSION = "__SESSION__";
  var started = Date.now();
  var sent = false;
  function text(el) { return ((el && el.innerText) || "").replace(/\s+/g, " ").trim(); }
  function send(obj) {
    if (sent) return;
    sent = true;
    obj.engine = location.hostname;
    obj.session = SESSION;
    location.href = "https://irbr-scrape.invalid/?s=" + SESSION + "#" + encodeURIComponent(JSON.stringify(obj));
  }
  function consent() {
    var buttons = Array.prototype.slice.call(document.querySelectorAll("button, input[type=submit], div[role=button]"));
    for (var i = 0; i < buttons.length; i++) {
      var label = (text(buttons[i]) || buttons[i].value || "").toLowerCase();
      if (/reject|elutas|mind(en)?t? elutas|customi|testre/.test(label)) continue;
      if (/accept all|agree|összes elfogad|mindet elfogad|elfogad|egyetért/.test(label)) { buttons[i].click(); return true; }
    }
    return false;
  }
  function google() {
    var out = [], seen = {};
    var root = document.querySelector("#rso") || document.querySelector("#search") || document.body;
    if (!root) return out;
    var anchors = root.querySelectorAll("a[href]");
    for (var i = 0; i < anchors.length; i++) {
      var a = anchors[i], h3 = a.querySelector("h3");
      if (!h3) continue;
      var href = a.href;
      if (!/^https?:/.test(href) || seen[href] || /(^|\.)google\./.test(new URL(href).hostname)) continue;
      seen[href] = 1;
      var block = a.closest("div.g") || a.closest("[data-hveid]") || a.parentElement.parentElement.parentElement;
      var snippet = "";
      if (block) {
        var cand = block.querySelectorAll("div[data-sncf], div.VwiC3b, span.aCOpRe, div[style*='line-clamp']");
        if (cand.length) snippet = text(cand[0]);
        if (!snippet) snippet = text(block).replace(text(h3), "").slice(0, 300);
      }
      out.push({ title: text(h3), url: href, snippet: snippet.slice(0, 400) });
    }
    return out;
  }
  function bing() {
    var out = [], items = document.querySelectorAll("li.b_algo");
    for (var i = 0; i < items.length; i++) {
      var a = items[i].querySelector("h2 a");
      if (!a || !a.href) continue;
      var p = items[i].querySelector(".b_caption p, p");
      out.push({ title: text(a), url: a.href, snippet: text(p).slice(0, 400) });
    }
    return out;
  }
  function tick() {
    if (sent || !document.body) return;
    var host = location.hostname, body = text(document.body);
    if (/consent\./.test(host) || document.querySelector("form[action*='consent']")) { consent(); return; }
    if (/\/sorry\//.test(location.pathname) || /unusual traffic|szokatlan forgalm/i.test(body) ||
        document.querySelector("#b_captcha, iframe[src*='captcha'], div.g-recaptcha")) {
      return send({ blocked: true, reason: "captcha" });
    }
    var hits = /google\./.test(host) ? google() : /bing\./.test(host) ? bing() : [];
    if (hits.length) return send({ hits: hits });
    if (Date.now() - started > 14000) send({ hits: [], empty: true, title: document.title });
  }
  setInterval(tick, 700);
})();"##
        .replace("__SESSION__", session)
}

pub fn google_url(query: &str) -> String {
    format!("https://www.google.com/search?q={}&hl=hu&gl=hu&num=20", crate::url_encode(query))
}

pub fn bing_url(query: &str) -> String {
    format!(
        "https://www.bing.com/search?q={}&setlang=hu&cc=HU&mkt=hu-HU&count=20",
        crate::url_encode(query)
    )
}

#[derive(Debug, PartialEq)]
pub enum Payload {
    Hits(Vec<DdgHit>),
    Blocked(String),
    Empty,
}

/// A szkript által küldött JSON feldolgozása; érvénytelen vagy nem http(s) találat kiesik.
pub fn parse_payload(json: &str) -> Result<Payload, String> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("Érvénytelen böngészős válasz: {e}"))?;
    if value["blocked"].as_bool().unwrap_or(false) {
        return Ok(Payload::Blocked(value["reason"].as_str().unwrap_or("ellenőrzés").to_string()));
    }
    let engine_is_bing = value["engine"].as_str().unwrap_or("").contains("bing.");
    let mut hits: Vec<DdgHit> = Vec::new();
    for item in value["hits"].as_array().cloned().unwrap_or_default() {
        let title = item["title"].as_str().unwrap_or("").trim().to_string();
        let raw = item["url"].as_str().unwrap_or("").trim().to_string();
        let url = if engine_is_bing || raw.contains("bing.com/ck/a") {
            crate::bing::resolve_href(&raw)
        } else if raw.starts_with("http://") || raw.starts_with("https://") {
            Some(raw)
        } else {
            None
        };
        let Some(url) = url else { continue };
        if title.is_empty() || hits.iter().any(|h| h.url == url) {
            continue;
        }
        hits.push(DdgHit {
            title: title.chars().take(300).collect(),
            url,
            snippet: item["snippet"].as_str().unwrap_or("").chars().take(500).collect(),
        });
        if hits.len() >= MAX_HITS {
            break;
        }
    }
    if hits.is_empty() {
        Ok(Payload::Empty)
    } else {
        Ok(Payload::Hits(hits))
    }
}

fn drain(rx: &std::sync::mpsc::Receiver<String>) {
    while rx.try_recv().is_ok() {}
}

/// Lefuttatja a kereséseket egyetlen, képernyőn kívüli böngészőablakban, egymás után
/// (a keresők kímélése miatt időközzel). Minden kereséshez a Google az első, captcha vagy üres
/// válasz esetén a Bing a második.
pub fn run_batch(
    app: &tauri::AppHandle,
    queries: Vec<(String, String)>,
) -> Vec<(String, Result<Vec<DdgHit>, String>)> {
    let _guard = BATCH_LOCK.lock();
    if queries.is_empty() {
        return Vec::new();
    }
    let session = format!("s{}", COUNTER.fetch_add(1, Ordering::SeqCst));
    let (tx, rx) = channel::<String>();
    if let Ok(mut map) = sessions().lock() {
        map.insert(session.clone(), tx);
    }

    let label = format!("scrape-{session}");
    let nav_session = session.clone();
    let first_url = match google_url(&queries[0].1).parse() {
        Ok(url) => url,
        Err(e) => {
            return queries
                .into_iter()
                .map(|(key, _)| (key, Err(format!("Érvénytelen keresési cím: {e}"))))
                .collect()
        }
    };

    let built = tauri::WebviewWindowBuilder::new(app, &label, tauri::WebviewUrl::External(first_url))
        .title("IRBR keresés")
        .inner_size(1200.0, 900.0)
        .position(-12000.0, -12000.0)
        .decorations(false)
        .skip_taskbar(true)
        .focused(false)
        .initialization_script(script(&session))
        .on_navigation(move |url| {
            if url.host_str() == Some(SENTINEL_HOST) {
                if let Some(fragment) = url.fragment() {
                    let payload = percent_decode(fragment);
                    if let Ok(map) = sessions().lock() {
                        if let Some(sender) = map.get(&nav_session) {
                            let _ = sender.send(payload);
                        }
                    }
                }
                return false;
            }
            true
        })
        .build();

    let window = match built {
        Ok(window) => window,
        Err(e) => {
            if let Ok(mut map) = sessions().lock() {
                map.remove(&session);
            }
            let message = format!("A böngészős keresőablak nem nyitható meg: {e}");
            return queries.into_iter().map(|(key, _)| (key, Err(message.clone()))).collect();
        }
    };

    let mut results = Vec::new();
    let mut use_bing_only = false;
    for (index, (key, query)) in queries.into_iter().enumerate() {
        let mut outcome: Result<Vec<DdgHit>, String> = Err("nem futott le".to_string());
        let attempts: Vec<&str> = if use_bing_only { vec!["bing"] } else { vec!["google", "bing"] };
        for (attempt, engine) in attempts.iter().enumerate() {
            // az első keresés első próbálkozása már a nyitott oldalon van
            if !(index == 0 && attempt == 0) {
                drain(&rx);
                let url = if *engine == "google" { google_url(&query) } else { bing_url(&query) };
                if let Ok(parsed) = url.parse() {
                    if window.navigate(parsed).is_err() {
                        outcome = Err("A böngészőablak nem navigálható.".to_string());
                        break;
                    }
                }
            }
            match rx.recv_timeout(QUERY_TIMEOUT) {
                Ok(json) => match parse_payload(&json) {
                    Ok(Payload::Hits(hits)) => {
                        outcome = Ok(hits);
                        break;
                    }
                    Ok(Payload::Blocked(reason)) => {
                        if *engine == "google" {
                            use_bing_only = true; // a Google ellenőrzést kért: a futam hátralévő részében Bing
                        }
                        outcome = Err(format!("A(z) {engine} ellenőrzést kért ({reason})."));
                    }
                    Ok(Payload::Empty) => outcome = Ok(Vec::new()),
                    Err(e) => outcome = Err(e),
                },
                Err(_) => outcome = Err(format!("A(z) {engine} nem válaszolt időben.")),
            }
        }
        results.push((key, outcome));
        std::thread::sleep(GAP);
    }

    let _ = window.destroy();
    if let Ok(mut map) = sessions().lock() {
        map.remove(&session);
    }
    // a főablak maradjon az előtérben
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.set_focus();
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_embeds_the_session_and_sentinel() {
        let s = script("s42");
        assert!(s.contains("\"s42\""));
        assert!(s.contains("irbr-scrape.invalid"));
        assert!(!s.contains("__SESSION__"));
    }

    #[test]
    fn parses_hits_and_filters_invalid() {
        let json = r#"{"engine":"www.google.com","hits":[
          {"title":"Lakás takarítás (Eger)","url":"https://qjob.hu/takaritas","snippet":"Elvárt órabér 1500 Ft"},
          {"title":"duplikátum","url":"https://qjob.hu/takaritas","snippet":""},
          {"title":"","url":"https://x.hu/","snippet":""},
          {"title":"js","url":"javascript:alert(1)","snippet":""}]}"#;
        match parse_payload(json).unwrap() {
            Payload::Hits(hits) => {
                assert_eq!(hits.len(), 1);
                assert_eq!(hits[0].url, "https://qjob.hu/takaritas");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn bing_redirects_are_resolved() {
        let json = r#"{"engine":"www.bing.com","hits":[{"title":"T","url":"https://www.bing.com/ck/a?!&&p=x&u=a1aHR0cHM6Ly9leGFtcGxlLmh1Lw&ntb=1","snippet":""}]}"#;
        match parse_payload(json).unwrap() {
            Payload::Hits(hits) => assert_eq!(hits[0].url, "https://example.hu/"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn blocked_and_empty_payloads() {
        assert_eq!(
            parse_payload(r#"{"blocked":true,"reason":"captcha"}"#).unwrap(),
            Payload::Blocked("captcha".into())
        );
        assert_eq!(parse_payload(r#"{"hits":[],"empty":true}"#).unwrap(), Payload::Empty);
        assert!(parse_payload("nem json").is_err());
    }

    #[test]
    fn search_urls_are_localised_and_encoded() {
        assert!(google_url("takarítás eger").contains("q=takar%C3%ADt%C3%A1s%20eger") || google_url("takarítás eger").contains("q=takar%C3%ADt%C3%A1s+eger"));
        assert!(google_url("x").contains("hl=hu"));
        assert!(bing_url("x").contains("setlang=hu"));
    }
}
