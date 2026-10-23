//! Platformok és platformra igazított lekérdezések. Egy kérés minden bekapcsolt
//! platformon lefut, de mindenhol a platform saját szintaxisára alakítva: pl. a Google
//! operátorait kapja, az X dátumhatárokat (`since:`/`until:`), a Scholar évszűrő
//! paramétereket, a Facebook és a Reddit operátormentes keresőszöveget.

#[derive(Debug, Default, Clone, PartialEq)]
pub struct ParsedQuery {
    pub must: Vec<String>,
    pub phrases: Vec<String>,
    /// Minden csoportból legalább egy szónak szerepelnie kell ("(a OR b)").
    pub any: Vec<Vec<String>>,
    pub exclude: Vec<String>,
    pub site: Vec<String>,
    pub year_from: Option<i32>,
    pub year_to: Option<i32>,
}

fn is_year(token: &str) -> Option<i32> {
    if token.len() == 4 && token.chars().all(|c| c.is_ascii_digit()) {
        let year: i32 = token.parse().ok()?;
        if (1900..=2100).contains(&year) {
            return Some(year);
        }
    }
    None
}

/// Idézőjeleket és zárójeleket tiszteletben tartó darabolás: a zárójeles csoport egy token.
fn tokenize(query: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    let mut depth = 0;
    for c in query.chars() {
        match c {
            '"' => {
                in_quote = !in_quote;
                current.push(c);
            }
            '(' if !in_quote => {
                depth += 1;
                current.push(c);
            }
            ')' if !in_quote => {
                depth = (depth - 1).max(0);
                current.push(c);
            }
            c if c.is_whitespace() && !in_quote && depth == 0 => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn unquote(token: &str) -> String {
    token.trim_matches('"').trim().to_string()
}

fn update_years(parsed: &mut ParsedQuery, from: Option<i32>, to: Option<i32>) {
    if let Some(f) = from {
        parsed.year_from = Some(parsed.year_from.map_or(f, |old| old.min(f)));
    }
    if let Some(t) = to {
        parsed.year_to = Some(parsed.year_to.map_or(t, |old| old.max(t)));
    }
}

pub fn parse_query(query: &str) -> ParsedQuery {
    let mut parsed = ParsedQuery::default();
    for token in tokenize(query) {
        let lower = token.to_ascii_lowercase();

        if let Some(rest) = lower.strip_prefix("site:") {
            let domain = rest.trim_matches(|c| c == '(' || c == ')').to_string();
            if !domain.is_empty() {
                parsed.site.push(domain);
            }
            continue;
        }
        if let Some(year) = lower.strip_prefix("after:").and_then(is_year) {
            update_years(&mut parsed, Some(year), None);
            continue;
        }
        if let Some(year) = lower.strip_prefix("before:").and_then(is_year) {
            update_years(&mut parsed, None, Some(year));
            continue;
        }
        if let Some((a, b)) = token.split_once("..") {
            if let (Some(from), Some(to)) = (is_year(a), is_year(b)) {
                update_years(&mut parsed, Some(from), Some(to));
                continue;
            }
        }
        if token.starts_with('(') {
            let inner = token.trim_start_matches('(').trim_end_matches(')');
            let words: Vec<String> = tokenize(inner)
                .into_iter()
                .filter(|w| !w.eq_ignore_ascii_case("or"))
                .map(|w| unquote(&w))
                .filter(|w| !w.is_empty())
                .collect();
            if !words.is_empty() && words.iter().all(|w| is_year(w).is_some()) {
                let years: Vec<i32> = words.iter().filter_map(|w| is_year(w)).collect();
                update_years(&mut parsed, years.iter().min().copied(), years.iter().max().copied());
            } else if !words.is_empty() {
                parsed.any.push(words);
            }
            continue;
        }
        if token.eq_ignore_ascii_case("or") || token.eq_ignore_ascii_case("and") {
            continue;
        }
        if let Some(rest) = token.strip_prefix('-') {
            let word = unquote(rest);
            if !word.is_empty() {
                parsed.exclude.push(word);
            }
            continue;
        }
        if token.starts_with('"') {
            let phrase = unquote(&token);
            if !phrase.is_empty() {
                parsed.phrases.push(phrase);
            }
            continue;
        }
        if is_year(&token).is_some() && (parsed.year_from.is_some() || parsed.year_to.is_some()) {
            continue;
        }
        parsed.must.push(token);
    }
    parsed
}

impl ParsedQuery {
    /// Operátormentes keresőszöveg: kötelező szavak, idézett kifejezések, a csoportok szavai.
    pub fn plain(&self) -> String {
        let mut parts: Vec<String> = self.must.clone();
        parts.extend(self.phrases.iter().map(|p| format!("\"{p}\"")));
        for group in &self.any {
            parts.push(group.join(" "));
        }
        parts.join(" ").trim().to_string()
    }

    /// Google-szerű operátoros szöveg (site és évhatár nélkül).
    pub fn with_operators(&self) -> String {
        let mut parts: Vec<String> = self.must.clone();
        parts.extend(self.phrases.iter().map(|p| format!("\"{p}\"")));
        for group in &self.any {
            parts.push(format!("({})", group.join(" OR ")));
        }
        parts.extend(self.exclude.iter().map(|e| format!("-{e}")));
        parts.join(" ").trim().to_string()
    }

    pub fn has_terms(&self) -> bool {
        !(self.must.is_empty() && self.phrases.is_empty() && self.any.is_empty())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    /// Kereső vagy közösségi keresőoldal: a lekérdezés a saját keresőjében nyílik meg.
    Engine,
    /// Domainre szűkített valódi webes keresés (Tavily/Brave/CSE) + keresőkártya.
    Site,
    /// Valódi hírcsatorna-lekérés és szűrés (RSS).
    Rss,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Native {
    None,
    Google,
    Bing,
    Ddg,
    Scholar,
    Facebook,
    Reddit,
    X,
    YouTube,
}

pub struct Platform {
    pub id: &'static str,
    pub label: &'static str,
    pub tier: i32,
    pub source_type: &'static str,
    pub kind: Kind,
    pub domains: &'static [&'static str],
    pub native: Native,
    pub relevance: i32,
}

pub const PLATFORMS: &[Platform] = &[
    Platform { id: "google", label: "Google", tier: 3, source_type: "Keresőmotor", kind: Kind::Engine, domains: &[], native: Native::Google, relevance: 92 },
    Platform { id: "bing", label: "Bing", tier: 3, source_type: "Keresőmotor", kind: Kind::Engine, domains: &[], native: Native::Bing, relevance: 90 },
    Platform { id: "duckduckgo", label: "DuckDuckGo", tier: 3, source_type: "Keresőmotor", kind: Kind::Engine, domains: &[], native: Native::Ddg, relevance: 88 },
    Platform { id: "scholar", label: "Google Scholar", tier: 2, source_type: "Tudományos forrás", kind: Kind::Engine, domains: &[], native: Native::Scholar, relevance: 84 },
    Platform { id: "youtube", label: "YouTube", tier: 4, source_type: "Videó", kind: Kind::Engine, domains: &[], native: Native::YouTube, relevance: 70 },
    Platform { id: "x", label: "X (Twitter)", tier: 4, source_type: "Közösségi kontroll", kind: Kind::Engine, domains: &[], native: Native::X, relevance: 68 },
    Platform { id: "facebook", label: "Facebook", tier: 4, source_type: "Közösségi kontroll", kind: Kind::Site, domains: &["facebook.com"], native: Native::Facebook, relevance: 72 },
    Platform { id: "reddit", label: "Reddit", tier: 4, source_type: "Közösségi kontroll", kind: Kind::Site, domains: &["reddit.com"], native: Native::Reddit, relevance: 70 },
    Platform { id: "magyar_kozlony", label: "Magyar Közlöny", tier: 1, source_type: "Hivatalos forrás", kind: Kind::Site, domains: &["magyarkozlony.hu"], native: Native::None, relevance: 94 },
    Platform { id: "jogszabaly", label: "Nemzeti Jogszabálytár", tier: 1, source_type: "Hivatalos forrás", kind: Kind::Site, domains: &["njt.hu"], native: Native::None, relevance: 93 },
    Platform { id: "ksh", label: "KSH", tier: 1, source_type: "Statisztikai forrás", kind: Kind::Site, domains: &["ksh.hu"], native: Native::None, relevance: 90 },
    Platform { id: "birosag", label: "Bírósági határozatok", tier: 1, source_type: "Hivatalos forrás", kind: Kind::Site, domains: &["birosag.hu", "kuria-birosag.hu"], native: Native::None, relevance: 91 },
    Platform { id: "kozbeszerzes", label: "Közbeszerzési Értesítő", tier: 1, source_type: "Hivatalos beszerzési forrás", kind: Kind::Site, domains: &["kozbeszerzes.hu"], native: Native::None, relevance: 89 },
    Platform { id: "hu_rss", label: "Magyar hírcsatornák (RSS)", tier: 3, source_type: "Hírforrás (RSS)", kind: Kind::Rss, domains: &[], native: Native::None, relevance: 86 },
];

pub fn enabled(enabled_ids: &[String], id: &str) -> bool {
    enabled_ids.is_empty() || enabled_ids.iter().any(|e| e == id)
}

fn enc(value: &str) -> String {
    crate::url_encode(value)
}

fn years_as_dates(parsed: &ParsedQuery) -> String {
    let mut out = String::new();
    if let Some(from) = parsed.year_from {
        out.push_str(&format!(" since:{from}-01-01"));
    }
    if let Some(to) = parsed.year_to {
        out.push_str(&format!(" until:{to}-12-31"));
    }
    out
}

/// A platform saját keresőjének URL-je, a platformra igazított lekérdezéssel.
pub fn native_url(native: Native, original: &str, parsed: &ParsedQuery) -> Option<String> {
    let ops = parsed.with_operators();
    let plain = parsed.plain();
    Some(match native {
        Native::None => return None,
        Native::Google => format!("https://www.google.com/search?q={}", enc(original)),
        Native::Bing => format!("https://www.bing.com/search?q={}", enc(&with_site(&ops, parsed))),
        Native::Ddg => format!("https://duckduckgo.com/?q={}", enc(&with_site(&ops, parsed))),
        Native::Scholar => {
            let mut url = format!("https://scholar.google.com/scholar?hl=hu&q={}", enc(&ops));
            if let Some(from) = parsed.year_from {
                url.push_str(&format!("&as_ylo={from}"));
            }
            if let Some(to) = parsed.year_to {
                url.push_str(&format!("&as_yhi={to}"));
            }
            url
        }
        Native::Facebook => format!("https://www.facebook.com/search/posts/?q={}", enc(&plain)),
        Native::Reddit => format!("https://www.reddit.com/search/?q={}", enc(&plain)),
        Native::X => format!(
            "https://x.com/search?src=typed_query&f=live&q={}",
            enc(&format!("{ops}{}", years_as_dates(parsed)))
        ),
        Native::YouTube => format!("https://www.youtube.com/results?search_query={}", enc(&plain)),
    })
}

fn with_site(ops: &str, parsed: &ParsedQuery) -> String {
    match parsed.site.as_slice() {
        [] => ops.to_string(),
        sites => format!("{ops} {}", sites.iter().map(|s| format!("site:{s}")).collect::<Vec<_>>().join(" OR ")),
    }
}

/// Domainre szűkített Google-keresés URL-je (site:), operátorokkal és évhatárral.
pub fn google_site_url(platform: &Platform, parsed: &ParsedQuery) -> String {
    let sites = platform
        .domains
        .iter()
        .map(|d| format!("site:{d}"))
        .collect::<Vec<_>>()
        .join(" OR ");
    let mut query = format!("{} ({sites})", parsed.with_operators());
    match (parsed.year_from, parsed.year_to) {
        (Some(f), Some(t)) if f == t => query.push_str(&format!(" {f}")),
        (Some(f), Some(t)) => query.push_str(&format!(" {f}..{t}")),
        (Some(f), None) => query.push_str(&format!(" after:{f}")),
        (None, Some(t)) => query.push_str(&format!(" before:{t}")),
        (None, None) => {}
    }
    format!("https://www.google.com/search?q={}", enc(query.trim()))
}

/// A webes keresőszolgáltatásnak (Tavily/Brave/CSE) küldendő lekérdezés egy site-platformra.
/// A `site:` token ebből a szövegből kerül a szolgáltató domain-szűrőjébe.
pub fn site_api_query(platform: &Platform, parsed: &ParsedQuery) -> String {
    let sites = platform
        .domains
        .iter()
        .map(|d| format!("site:{d}"))
        .collect::<Vec<_>>();
    let group = if sites.len() > 1 {
        format!("({})", sites.join(" OR "))
    } else {
        sites.join(" ")
    };
    format!("{} {group}", parsed.with_operators()).trim().to_string()
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct PlatformLink {
    pub id: String,
    pub label: String,
    pub url: String,
    pub query: String,
}

/// Minden bekapcsolt platform nyitható linkje a platformra igazított lekérdezéssel.
pub fn links(original: &str, enabled_ids: &[String]) -> Vec<PlatformLink> {
    let parsed = parse_query(original);
    let mut out = Vec::new();
    for platform in PLATFORMS {
        if !enabled(enabled_ids, platform.id) {
            continue;
        }
        let url = match platform.kind {
            Kind::Engine => native_url(platform.native, original, &parsed),
            Kind::Site => native_url(platform.native, original, &parsed)
                .or_else(|| Some(google_site_url(platform, &parsed))),
            Kind::Rss => None,
        };
        if let Some(url) = url {
            let query = match platform.kind {
                Kind::Site => site_api_query(platform, &parsed),
                _ => parsed.with_operators(),
            };
            out.push(PlatformLink {
                id: platform.id.to_string(),
                label: platform.label.to_string(),
                url,
                query,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_operators_phrases_groups_and_years() {
        let p = parse_query("óradíj \"szakértői díj\" (házi OR otthoni) -hirdetés site:ksh.hu 2020..2024 háztartás");
        assert_eq!(p.must, vec!["óradíj", "háztartás"]);
        assert_eq!(p.phrases, vec!["szakértői díj"]);
        assert_eq!(p.any, vec![vec!["házi".to_string(), "otthoni".to_string()]]);
        assert_eq!(p.exclude, vec!["hirdetés"]);
        assert_eq!(p.site, vec!["ksh.hu"]);
        assert_eq!((p.year_from, p.year_to), (Some(2020), Some(2024)));
    }

    #[test]
    fn year_or_list_and_after_before() {
        let p = parse_query("bér (2020 OR 2021 OR 2022)");
        assert_eq!((p.year_from, p.year_to), (Some(2020), Some(2022)));
        assert!(p.any.is_empty());
        let p = parse_query("bér after:2019 before:2023");
        assert_eq!((p.year_from, p.year_to), (Some(2019), Some(2023)));
        assert_eq!(p.must, vec!["bér"]);
    }

    #[test]
    fn plain_and_operator_rendering() {
        let p = parse_query("óradíj \"szakértői díj\" (házi OR otthoni) -hirdetés");
        assert_eq!(p.plain(), "óradíj \"szakértői díj\" házi otthoni");
        assert_eq!(p.with_operators(), "óradíj \"szakértői díj\" (házi OR otthoni) -hirdetés");
        assert!(p.has_terms());
        assert!(!parse_query("site:ksh.hu 2020..2022").has_terms());
    }

    #[test]
    fn native_urls_are_adapted_per_platform() {
        let original = "óradíj \"szakértői díj\" -hirdetés 2020..2022";
        let p = parse_query(original);
        let x = native_url(Native::X, original, &p).unwrap();
        assert!(x.contains("since%3A2020-01-01"), "{x}");
        assert!(x.contains("until%3A2022-12-31"), "{x}");
        let scholar = native_url(Native::Scholar, original, &p).unwrap();
        assert!(scholar.contains("as_ylo=2020") && scholar.contains("as_yhi=2022"));
        let fb = native_url(Native::Facebook, original, &p).unwrap();
        assert!(!fb.contains("2020") && !fb.contains("hirdet"), "{fb}");
        let google = native_url(Native::Google, original, &p).unwrap();
        assert!(google.contains("2020..2022") || google.contains("2020%2E%2E2022"), "{google}");
        let bing = native_url(Native::Bing, original, &p).unwrap();
        assert!(!bing.contains("2020"), "{bing}");
        assert!(native_url(Native::None, original, &p).is_none());
    }

    #[test]
    fn site_queries_carry_domains_and_years() {
        let p = parse_query("bírság \"közlekedési\" 2021..2023");
        let birosag = PLATFORMS.iter().find(|x| x.id == "birosag").unwrap();
        let q = site_api_query(birosag, &p);
        assert!(q.contains("(site:birosag.hu OR site:kuria-birosag.hu)"), "{q}");
        assert!(!q.contains("2021"));
        let url = google_site_url(birosag, &p);
        assert!(url.contains("2021") && url.contains("birosag.hu"));
    }

    #[test]
    fn links_cover_every_platform_but_rss_and_respect_enabled() {
        let all = links("óradíj", &[]);
        let ids: Vec<&str> = all.iter().map(|l| l.id.as_str()).collect();
        assert!(ids.contains(&"facebook") && ids.contains(&"magyar_kozlony") && ids.contains(&"google"));
        assert!(!ids.contains(&"hu_rss"));
        let some = links("óradíj", &["facebook".to_string()]);
        assert_eq!(some.len(), 1);
    }
}
