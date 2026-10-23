//! Azonos történet összevonása: ugyanarról szóló találatok egy szálba kerülnek.
//! Az egyes elemek profilja a címének és kivonatának ritkasággal (IDF) súlyozott,
//! tőcsonkolt szavaiból áll; két csoport akkor egyesül, ha a teljes profiljuk
//! koszinusz-hasonlósága eléggé magas (centroid-kapcsolás, nincs láncolódás).

use std::collections::{HashMap, HashSet};

/// Ennél hasonlóbb profilú csoportok egyesülnek.
const TAU: f64 = 0.40;
/// Egy szál legfeljebb ennyi elemből állhat.
const MAX_MEMBERS: usize = 60;
/// Legalább ennyi súlyozott szó kell egy elem profiljába.
const MIN_WORDS: usize = 3;
/// Két elemnek legalább ennyi közös szava legyen.
const MIN_COMMON: usize = 2;
/// A teljesítmény védelme: ennél több elemet nem fürtözünk.
const MAX_ITEMS: usize = 300;

const STOP_WORDS: &str = "a az egy es is hogy nem de ha meg mar van volt lesz csak mint vagy ez azt ott itt \
    amit aki ami mar nagy uj ujabb szerint utan elott kozott alatt felett ellen miatt \
    the a an and are as at be but by for from has have he her his in is it its of on or that this to was were will with you your \
    hir hirek cikk video frissites olvas tovabb tobb";

/// Ékezetek és kis/nagybetű elhagyása, hogy az ékezet nélküli és ékezetes alak egyezzen.
pub fn fold(input: &str) -> String {
    input
        .chars()
        .flat_map(|c| c.to_lowercase())
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' | 'ő' => 'o',
            'ú' | 'ù' | 'û' | 'ü' | 'ű' => 'u',
            other => other,
        })
        .collect()
}

/// Durva tőképzés: az ékezet nélküli szó első hat betűje. Magyar ragozásnál ez a
/// legegyszerűbb használható közelítés (a "Telex-nek", "Telexen" -> "telex").
pub fn stem(word: &str) -> String {
    word.chars().take(6).collect()
}

fn stop_set() -> HashSet<String> {
    STOP_WORDS.split_whitespace().map(|w| stem(&fold(w))).collect()
}

fn tokens(text: &str, stops: &HashSet<String>) -> Vec<String> {
    let folded = fold(text);
    folded
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() > 2)
        .map(stem)
        .filter(|w| !stops.contains(w))
        .collect()
}

type Profile = HashMap<String, f64>;

fn norm(profile: &Profile) -> f64 {
    profile.values().map(|v| v * v).sum::<f64>().sqrt()
}

fn cosine(a: &Profile, b: &Profile) -> f64 {
    let (small, large) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    let dot: f64 = small.iter().filter_map(|(k, v)| large.get(k).map(|w| v * w)).sum();
    let denom = norm(a) * norm(b);
    if denom == 0.0 {
        0.0
    } else {
        dot / denom
    }
}

fn common_words(a: &Profile, b: &Profile) -> usize {
    a.keys().filter(|k| b.contains_key(*k)).count()
}

/// Minden elemhez egy szál-azonosítót ad (0-tól). Az egyedül álló, vagy túl kevés szóból
/// álló elem külön szál marad. `texts[i]` az i. elem címe és kivonata együtt.
pub fn cluster(texts: &[String]) -> Vec<usize> {
    let n = texts.len().min(MAX_ITEMS);
    let stops = stop_set();
    let docs: Vec<Vec<String>> = texts.iter().take(n).map(|t| tokens(t, &stops)).collect();

    // dokumentumgyakoriság
    let mut df: HashMap<&str, usize> = HashMap::new();
    for doc in &docs {
        let unique: HashSet<&str> = doc.iter().map(String::as_str).collect();
        for word in unique {
            *df.entry(word).or_insert(0) += 1;
        }
    }
    // kis mintán a "túl gyakori" határ legyen nagyvonalú; nagyobb mintán szigorúbb
    let max_df = if n >= 8 { (n as f64 * 0.5).ceil() as usize } else { n };

    let profiles: Vec<Profile> = docs
        .iter()
        .map(|doc| {
            let mut profile = Profile::new();
            let unique: HashSet<&str> = doc.iter().map(String::as_str).collect();
            for word in unique {
                let d = df[word];
                if d > max_df && n >= 8 {
                    continue;
                }
                let idf = ((n as f64 + 1.0) / (d as f64 + 1.0)).ln() + 1.0;
                profile.insert(word.to_string(), idf);
            }
            profile
        })
        .collect();

    // csoportok: tagok + összegzett profil (centroid)
    struct Group {
        members: Vec<usize>,
        centroid: Profile,
    }
    let mut groups: Vec<Option<Group>> = (0..n)
        .map(|i| {
            Some(Group {
                members: vec![i],
                centroid: profiles[i].clone(),
            })
        })
        .collect();

    loop {
        let mut best: Option<(usize, usize, f64)> = None;
        for i in 0..n {
            let Some(gi) = &groups[i] else { continue };
            for j in (i + 1)..n {
                let Some(gj) = &groups[j] else { continue };
                if gi.members.len() + gj.members.len() > MAX_MEMBERS {
                    continue;
                }
                // két egyedül álló elemnél kell a szigorú feltétel
                let lone_pair = gi.members.len() == 1 && gj.members.len() == 1;
                if lone_pair
                    && (profiles[i].len() < MIN_WORDS
                        || profiles[j].len() < MIN_WORDS
                        || common_words(&profiles[i], &profiles[j]) < MIN_COMMON)
                {
                    continue;
                }
                if !lone_pair && common_words(&gi.centroid, &gj.centroid) < MIN_COMMON {
                    continue;
                }
                let sim = cosine(&gi.centroid, &gj.centroid);
                if sim >= TAU && best.map_or(true, |(_, _, b)| sim > b) {
                    best = Some((i, j, sim));
                }
            }
        }
        let Some((i, j, _)) = best else { break };
        let gj = groups[j].take().expect("létező csoport");
        let gi = groups[i].as_mut().expect("létező csoport");
        gi.members.extend(gj.members);
        for (word, weight) in gj.centroid {
            *gi.centroid.entry(word).or_insert(0.0) += weight;
        }
    }

    let mut ids = vec![0usize; texts.len()];
    let mut next = 0;
    for group in groups.into_iter().flatten() {
        for member in group.members {
            ids[member] = next;
        }
        next += 1;
    }
    // a MAX_ITEMS fölötti elemek külön szálak
    for slot in ids.iter_mut().skip(n) {
        *slot = next;
        next += 1;
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_accents_and_stems() {
        assert_eq!(fold("Őrizetbe Vétel"), "orizetbe vetel");
        assert_eq!(stem("szakértői"), "szakér");
    }

    #[test]
    fn groups_the_same_story_and_keeps_others_apart() {
        let texts: Vec<String> = [
            "Szakértői óradíj emelkedés az igazságügyi szakértőknél: új rendelet lép életbe",
            "Új rendelet: emelkedik az igazságügyi szakértői óradíj",
            "Az igazságügyi szakértői óradíj emelése: mit jelent az új rendelet",
            "Időjárás: hidegfront érkezik hétvégén, esőre számíthatunk országszerte",
            "Hidegfront és eső érkezik hétvégén: időjárás országszerte",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let ids = cluster(&texts);
        assert_eq!(ids[0], ids[1]);
        assert_eq!(ids[1], ids[2]);
        assert_eq!(ids[3], ids[4]);
        assert_ne!(ids[0], ids[3]);
    }

    #[test]
    fn lone_and_short_items_stay_single() {
        let texts: Vec<String> = ["ok", "ok", "teljesen más téma kutatás jegyzőkönyv"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let ids = cluster(&texts);
        assert_ne!(ids[0], ids[1]);
        assert_ne!(ids[1], ids[2]);
    }

    #[test]
    fn empty_input_is_fine() {
        assert!(cluster(&[]).is_empty());
    }
}
