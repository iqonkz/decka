//! What the user asked the assistant to remember (DATA_MODEL.md 13.6): `memory.json`, at most 200 facts of at most 200 characters.
//! A fact is added only by an explicit phrase ("remember that ...") and removed only on request and after a confirmation;
//! nothing is learned by itself and no phrase is recorded. Up to 20 facts that fit the phrase go into the request to the
//! model, as data.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

pub const MAX_FACTS: usize = 200;
pub const MAX_FACT_CHARS: usize = 200;
/// How many facts go with one request.
pub const MAX_SELECTED: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fact {
    pub id: String,
    pub text: String,
    /// Milliseconds since 1970 (UTC).
    pub created_at: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct File {
    facts: Vec<Fact>,
}

pub struct Memory {
    path: PathBuf,
    inner: Mutex<File>,
}

fn normalize(text: &str) -> Result<String, String> {
    let t = text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ");
    if t.is_empty() {
        return Err("There is nothing to remember".into());
    }
    if t.chars().count() > MAX_FACT_CHARS {
        return Err(format!("That is too long to remember (at most {MAX_FACT_CHARS} characters)"));
    }
    Ok(t)
}

/// The words of a text that can tell a fact from another: lower case, three letters or more.
fn words(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric()).filter(|w| w.chars().count() >= 3).map(str::to_lowercase).collect()
}

/// Two words are the same when they start alike (a case ending or a plural does not matter): the first five letters, or the
/// whole shorter word when it is shorter.
fn same_word(a: &str, b: &str) -> bool {
    let n = a.chars().count().min(b.chars().count()).min(5);
    n >= 3 && a.chars().take(n).eq(b.chars().take(n))
}

/// Up to `MAX_SELECTED` facts that share words with the phrase, the best match first (newer first among equals).
pub fn select(facts: &[Fact], phrase: &str) -> Vec<String> {
    let spoken = words(phrase);
    if spoken.is_empty() {
        return Vec::new();
    }
    let mut scored: Vec<(usize, u64, &Fact)> = facts
        .iter()
        .filter_map(|f| {
            let fw = words(&f.text);
            let score = spoken.iter().filter(|s| fw.iter().any(|w| same_word(s, w))).count();
            (score > 0).then_some((score, f.created_at, f))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    scored.into_iter().take(MAX_SELECTED).map(|(_, _, f)| f.text.clone()).collect()
}

/// The phrases that ask to remember something, in the 11 languages of the interface (word starts, lower case). The model may
/// only call `memory_add` after one of them was said.
const CUES: [&str; 22] = [
    "remember", "запомни", "запомн", "есте сақта", "есіңде сақта", "есіңде болсын", "记住", "记得", "याद रख", "recuerda", "recuérda", "retiens", "rappelle-toi",
    "souviens", "lembra", "lembre", "merk dir", "merke dir", "تذكر", "احفظ", "覚えて", "覚えておいて",
];

/// Whether the phrase asks to remember something.
pub fn asks_to_remember(phrase: &str) -> bool {
    let p = phrase.to_lowercase();
    CUES.iter().any(|c| p.contains(c))
}

impl Memory {
    pub fn load(dir: &Path) -> Memory {
        let path = dir.join("memory.json");
        let mut file: File = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        file.facts.truncate(MAX_FACTS);
        for f in &mut file.facts {
            f.text = f.text.chars().take(MAX_FACT_CHARS).collect();
        }
        Memory { path, inner: Mutex::new(file) }
    }

    fn save(&self, f: &File) -> Result<(), String> {
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(f).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &self.path).map_err(|e| e.to_string())
    }

    pub fn all(&self) -> Vec<Fact> {
        self.inner.lock().unwrap().facts.clone()
    }

    /// Adds a fact (the same fact twice is kept once). Only the explicit tool calls this.
    pub fn add(&self, text: &str, now_ms: u64) -> Result<Fact, String> {
        let text = normalize(text)?;
        let mut g = self.inner.lock().unwrap();
        if let Some(f) = g.facts.iter().find(|f| f.text.to_lowercase() == text.to_lowercase()) {
            return Ok(f.clone());
        }
        if g.facts.len() >= MAX_FACTS {
            return Err("The memory is full: forget something first".into());
        }
        let fact = Fact { id: format!("{:016x}", rand::random::<u64>()), text, created_at: now_ms };
        g.facts.push(fact.clone());
        self.save(&g)?;
        Ok(fact)
    }

    /// Forgets the fact that fits `query` best; `Err` when none fits.
    pub fn remove_matching(&self, query: &str) -> Result<Fact, String> {
        let mut g = self.inner.lock().unwrap();
        let i = crate::assistant::find_name(g.facts.iter().map(|f| f.text.as_str()), query).or_else(|| {
            // A word of the query that starts like a word of a fact ("Айгерим" - "Айгерим's number").
            let spoken = words(query);
            g.facts
                .iter()
                .enumerate()
                .map(|(i, f)| (spoken.iter().filter(|s| words(&f.text).iter().any(|w| same_word(s, w))).count(), i))
                .filter(|(n, _)| *n > 0)
                .max_by_key(|(n, _)| *n)
                .map(|(_, i)| i)
        });
        let Some(i) = i else { return Err("I do not remember that".into()) };
        let fact = g.facts.remove(i);
        self.save(&g)?;
        Ok(fact)
    }

    pub fn remove_id(&self, id: &str) -> bool {
        let mut g = self.inner.lock().unwrap();
        let before = g.facts.len();
        g.facts.retain(|f| f.id != id);
        let removed = g.facts.len() != before;
        if removed {
            let _ = self.save(&g);
        }
        removed
    }

    /// The facts for the request about `phrase`.
    pub fn for_phrase(&self, phrase: &str) -> Vec<String> {
        select(&self.inner.lock().unwrap().facts, phrase)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem() -> (tempfile::TempDir, Memory) {
        let d = tempfile::tempdir().unwrap();
        let m = Memory::load(d.path());
        (d, m)
    }

    #[test]
    fn a_fact_is_added_only_by_the_call_and_kept_once() {
        let (dir, m) = mem();
        assert!(m.all().is_empty(), "nothing is there by itself");
        let f = m.add("  Aigerim   is my wife ", 100).unwrap();
        assert_eq!((f.text.as_str(), f.created_at, f.id.len()), ("Aigerim is my wife", 100, 16));
        assert_eq!(m.add("aigerim is my wife", 200).unwrap().id, f.id, "the same fact twice is one");
        assert_eq!(m.all().len(), 1);
        assert_eq!(Memory::load(dir.path()).all(), m.all(), "saved");
        let json = std::fs::read_to_string(dir.path().join("memory.json")).unwrap();
        assert!(json.contains("createdAt") && json.contains("Aigerim is my wife"));
    }

    #[test]
    fn limits_are_kept() {
        let (_d, m) = mem();
        assert!(m.add("", 1).is_err() && m.add("   ", 1).is_err());
        assert!(m.add(&"x".repeat(201), 1).is_err());
        assert!(m.add(&"x".repeat(200), 1).is_ok());
        for i in 0..MAX_FACTS - 1 {
            m.add(&format!("fact number {i}"), i as u64).unwrap();
        }
        assert_eq!(m.add("one more", 1).unwrap_err(), "The memory is full: forget something first");
        assert_eq!(m.all().len(), MAX_FACTS);
    }

    #[test]
    fn at_most_twenty_facts_that_fit_the_phrase_are_chosen_best_first() {
        let facts: Vec<Fact> = (0..60)
            .map(|i| Fact { id: format!("{i:016x}"), text: if i % 2 == 0 { format!("my project folder number {i} is on drive D") } else { format!("unrelated thing {i}") }, created_at: i })
            .chain([Fact { id: "f".repeat(16), text: "Aigerim is my wife, her number is 555".into(), created_at: 1 }])
            .collect();
        let s = select(&facts, "write to Aigerim");
        assert_eq!(s, ["Aigerim is my wife, her number is 555"], "only a fact with a shared word");
        let many = select(&facts, "open my project folder on drive");
        assert_eq!(many.len(), MAX_SELECTED, "at most 20");
        assert!(many[0].contains("number 58"), "among equals the newest first: {}", many[0]);
        assert!(select(&facts, "xyz").is_empty() && select(&facts, "").is_empty() && select(&facts, "is of").is_empty(), "short words fit nothing");
    }

    #[test]
    fn a_case_ending_does_not_hide_a_fact() {
        let facts = vec![Fact { id: "1".repeat(16), text: "Айгерим — моя жена".into(), created_at: 1 }, Fact { id: "2".repeat(16), text: "Проект лежит на диске D".into(), created_at: 2 }];
        assert_eq!(select(&facts, "напиши Айгерим"), ["Айгерим — моя жена"]);
        assert_eq!(select(&facts, "напиши Айгерим"), select(&facts, "позвони Айгеримом"), "a case ending of the same name");
        assert_eq!(select(&facts, "открой мой проект"), ["Проект лежит на диске D"]);
    }

    #[test]
    fn forgetting_finds_the_fact_by_a_word_and_only_that_one() {
        let (dir, m) = mem();
        m.add("Aigerim is my wife", 1).unwrap();
        m.add("My project is on drive D", 2).unwrap();
        assert_eq!(m.remove_matching("Aigerim").unwrap().text, "Aigerim is my wife");
        assert_eq!(m.remove_matching("tractor").unwrap_err(), "I do not remember that");
        assert_eq!(Memory::load(dir.path()).all().len(), 1);
        let id = m.all()[0].id.clone();
        assert!(!m.remove_id("0000000000000000") && m.remove_id(&id));
        assert!(Memory::load(dir.path()).all().is_empty());
    }

    #[test]
    fn only_an_explicit_phrase_asks_to_remember() {
        for yes in [
            "remember that Aigerim is my wife", "Запомни, что Айгерим моя жена", "запомните мой номер", "есіңде сақта", "记住我的车牌", "Recuerda que mi coche es azul",
            "Retiens que je suis allergique", "Merk dir meine Adresse", "Lembre que eu moro aqui", "覚えておいて", "تذكر أن اسمي علي",
        ] {
            assert!(asks_to_remember(yes), "{yes}");
        }
        for no in ["open the calculator", "what is my project", "volume up", "напиши Айгерим", ""] {
            assert!(!asks_to_remember(no), "{no}");
        }
    }

    #[test]
    fn a_broken_file_is_an_empty_memory_and_an_oversized_one_is_cut() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("memory.json"), "{ nonsense").unwrap();
        assert!(Memory::load(d.path()).all().is_empty());
        let big = File { facts: (0..300).map(|i| Fact { id: format!("{i:016x}"), text: "y".repeat(500), created_at: i }).collect() };
        std::fs::write(d.path().join("memory.json"), serde_json::to_string(&big).unwrap()).unwrap();
        let m = Memory::load(d.path());
        assert_eq!((m.all().len(), m.all()[0].text.chars().count()), (MAX_FACTS, MAX_FACT_CHARS));
    }
}
