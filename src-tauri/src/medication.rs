//! Gyógyszer-ellenőrzés: a gyógyszernév alapján internetes keresést indító linkek.
//! Ez NEM gyógyszerazonosítás: a modul csak keresőoldalakat nyit meg, a találatokat nem
//! értelmezi, és semmit nem tárol a keresett névről (egészségügyi adatnak tekintjük).

use crate::platforms::PlatformLink;

const MAX_NAME_CHARS: usize = 100;
const MIN_NAME_CHARS: usize = 2;

/// Megtisztított gyógyszernév: vezérlőkarakterek, idézőjelek és keresőoperátorok nélkül,
/// összevont szóközökkel, legfeljebb 100 karakter. Túl rövid vagy üres bemenetre `None`.
pub fn sanitize_name(input: &str) -> Option<String> {
    let cleaned: String = input
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '"' | '“' | '”' | '\'' | '(' | ')' | '<' | '>' | '\\'))
        .collect();
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let truncated: String = collapsed.chars().take(MAX_NAME_CHARS).collect();
    let name = truncated.trim().to_string();
    if name.chars().count() < MIN_NAME_CHARS {
        return None;
    }
    // keresőoperátorok kiszűrése, hogy a név ne módosíthassa a lekérdezést
    let safe = name
        .split_whitespace()
        .filter(|word| {
            let w = word.to_ascii_lowercase();
            !(w.contains(':') && (w.starts_with("site:") || w.starts_with("inurl:") || w.starts_with("filetype:")))
        })
        .collect::<Vec<_>>()
        .join(" ");
    if safe.chars().count() < MIN_NAME_CHARS {
        None
    } else {
        Some(safe)
    }
}

fn enc(value: &str) -> String {
    crate::url_encode(value)
}

/// A névre induló keresések: betegtájékoztató, a hivatalos OGYÉI-oldalak és a képes találatok
/// (a doboz és a tabletta kinézetének összevetéséhez).
pub fn links(name: &str) -> Result<Vec<PlatformLink>, String> {
    let name = sanitize_name(name).ok_or_else(|| {
        "Írd be a gyógyszer nevét (legalább 2 karakter), keresőoperátorok és idézőjelek nélkül.".to_string()
    })?;
    let google = |q: String, extra: &str| format!("https://www.google.com/search?q={}&hl=hu&gl=hu{extra}", enc(&q));
    Ok(vec![
        PlatformLink {
            id: "med_betegtajekoztato".into(),
            label: "Betegtájékoztató".into(),
            url: google(format!("\"{name}\" gyógyszer betegtájékoztató"), ""),
            query: name.clone(),
        },
        PlatformLink {
            id: "med_ogyei".into(),
            label: "OGYÉI (hivatalos)".into(),
            url: google(format!("{name} site:ogyei.gov.hu"), ""),
            query: name.clone(),
        },
        PlatformLink {
            id: "med_kepek".into(),
            label: "Képek (doboz, tabletta)".into(),
            url: google(format!("{name} gyógyszer doboz tabletta"), "&tbm=isch"),
            query: name,
        },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_control_chars_quotes_and_operators() {
        assert_eq!(sanitize_name("  Algopyrin \n 500 mg  ").as_deref(), Some("Algopyrin 500 mg"));
        assert_eq!(sanitize_name("\"Ibuprofen\" (400)").as_deref(), Some("Ibuprofen 400"));
        assert_eq!(sanitize_name("Aspirin site:evil.com").as_deref(), Some("Aspirin"));
        assert_eq!(sanitize_name("a"), None);
        assert_eq!(sanitize_name("   "), None);
        assert_eq!(sanitize_name("site:x.hu"), None);
        let long = "x".repeat(500);
        assert_eq!(sanitize_name(&long).unwrap().chars().count(), MAX_NAME_CHARS);
    }

    #[test]
    fn builds_leaflet_official_and_image_searches() {
        let tabs = links("Algopyrin 500 mg").unwrap();
        let by = |id: &str| tabs.iter().find(|t| t.id == id).map(|t| t.url.clone()).unwrap_or_default();
        assert!(by("med_betegtajekoztato").contains("betegt%C3%A1j%C3%A9koztat%C3%B3"));
        assert!(by("med_ogyei").contains("site%3Aogyei.gov.hu"));
        assert!(by("med_kepek").contains("tbm=isch"));
        assert!(tabs.iter().all(|t| t.url.starts_with("https://www.google.com/search?")));
    }

    #[test]
    fn rejects_empty_input_with_a_helpful_message() {
        let error = links("  ").unwrap_err();
        assert!(error.contains("gyógyszer nevét"));
    }
}
