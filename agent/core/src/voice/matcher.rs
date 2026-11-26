//! The fast path of voice control (DATA_MODEL.md 12.4 item 3): a phrase is matched against the phrase table of the
//! language (`shared/voice/<lang>.json`) without any model. Pure functions only.
//!
//! * Normalisation: lower case, no punctuation, number words to digits (en, ru), CJK text split into characters.
//! * Exact: a template matches when its literals are in the phrase (up to 3 filler words before it) and its slots take the
//!   rest; the longest literal wins. Two different winners of the same length are ambiguous.
//! * Fuzzy: otherwise the literals of slot-less templates may be an ordered subsequence of the phrase, each word equal or
//!   with a trigram similarity of at least 0.8.

use std::collections::HashSet;

use serde::Deserialize;
use serde_json::{json, Map, Value};

pub const FUZZY_THRESHOLD: f64 = 0.8;
/// The longest answer to a question: "yes, do it" in any of the languages.
const MAX_ANSWER_WORDS: usize = 3;
/// Filler words allowed in front of an exact template ("can you open calculator").
const MAX_LEAD: usize = 3;
/// Extra words a fuzzy match tolerates around the template.
const MAX_EXTRA: usize = 4;
/// Best scores closer than this are ambiguous.
const TIE_MARGIN: f64 = 0.05;

#[derive(Debug, Clone, PartialEq)]
pub struct Intent {
    pub tool: String,
    pub args: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Match {
    Hit(Intent),
    /// Two or more different intents fit equally well: ask, do not act.
    Ambiguous(Vec<Intent>),
    None,
}

#[derive(Deserialize)]
struct RawTable {
    lang: String,
    #[serde(default)]
    yes: Vec<String>,
    #[serde(default)]
    no: Vec<String>,
    #[serde(default)]
    place_prepositions: Vec<String>,
    #[serde(default)]
    place_postpositions: Vec<String>,
    #[serde(default)]
    place_suffixes: Vec<String>,
    #[serde(default)]
    place_ignore: Vec<String>,
    intents: Vec<RawIntent>,
    /// M27: what is said during a guide - `next`, `back`, `stop`, `done`.
    #[serde(default)]
    guide_controls: std::collections::BTreeMap<String, Vec<String>>,
    /// M27: words of elements that are dangerous to press (they wait for a "yes").
    #[serde(default)]
    danger_words: Vec<String>,
}

#[derive(Deserialize)]
struct RawIntent {
    tool: String,
    #[serde(default)]
    args: Map<String, Value>,
    patterns: Vec<String>,
}

#[derive(Debug, Clone)]
enum Piece {
    Lit(Vec<String>),
    Slot(String),
}

#[derive(Debug, Clone)]
struct Compiled {
    intent: usize,
    pieces: Vec<Piece>,
    /// Number of characters in the literals: the longer, the more specific.
    weight: usize,
}

#[derive(Debug, Clone)]
pub struct Table {
    pub lang: String,
    yes: Vec<Vec<String>>,
    no: Vec<Vec<String>>,
    intents: Vec<(String, Map<String, Value>)>,
    patterns: Vec<Compiled>,
    /// Lead-ins of the `calc` templates ("calculate", "сколько будет", or empty for a bare expression).
    calc_leads: Vec<String>,
    /// Lead-ins of the `remind` templates ("напомни мне", "remind me to"), as words.
    remind_leads: Vec<Vec<String>>,
    guide_controls: Vec<(&'static str, Vec<Vec<String>>)>,
    danger_words: Vec<String>,
    place_prepositions: Vec<String>,
    place_postpositions: Vec<String>,
    place_suffixes: Vec<String>,
    place_ignore: Vec<String>,
}

// ---- Normalisation ----------------------------------------------------------------------------------------------

fn is_cjk(c: char) -> bool {
    matches!(c, '\u{3040}'..='\u{30ff}' | '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{ff66}'..='\u{ff9f}')
}

/// Combining marks (vowel signs, diacritics) belong to the word they sit in.
fn is_mark(c: char) -> bool {
    matches!(c, '\u{0300}'..='\u{036f}' | '\u{064b}'..='\u{065f}' | '\u{0670}' | '\u{0900}'..='\u{0903}' | '\u{093a}'..='\u{094f}' | '\u{0951}'..='\u{0957}' | '\u{0962}'..='\u{0963}' | '\u{3099}' | '\u{309a}')
}

/// Lower case, no punctuation, single spaces; apostrophes vanish ("don't" -> "dont").
pub fn normalize(text: &str) -> String {
    tokens(text).join(" ")
}

/// Normalised tokens; CJK characters are tokens of their own; number words become digits.
pub fn tokens(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut Vec<String>| {
        if !word.is_empty() {
            out.push(std::mem::take(word));
        }
    };
    for c in text.chars().flat_map(|c| c.to_lowercase()) {
        if matches!(c, '\'' | '\u{2019}' | '\u{02bc}') {
            continue;
        }
        if is_cjk(c) {
            flush(&mut word, &mut out);
            out.push(c.to_string());
        } else if c.is_alphanumeric() || is_mark(c) {
            word.push(c);
        } else {
            flush(&mut word, &mut out);
        }
    }
    flush(&mut word, &mut out);
    number_words(out)
}

fn unit(w: &str) -> Option<u32> {
    Some(match w {
        "zero" | "ноль" => 0,
        "one" | "один" | "одну" | "одна" => 1,
        "two" | "два" | "две" => 2,
        "three" | "три" => 3,
        "four" | "четыре" => 4,
        "five" | "пять" => 5,
        "six" | "шесть" => 6,
        "seven" | "семь" => 7,
        "eight" | "восемь" => 8,
        "nine" | "девять" => 9,
        "ten" | "десять" => 10,
        "eleven" | "одиннадцать" => 11,
        "twelve" | "двенадцать" => 12,
        "thirteen" | "тринадцать" => 13,
        "fourteen" | "четырнадцать" => 14,
        "fifteen" | "пятнадцать" => 15,
        "sixteen" | "шестнадцать" => 16,
        "seventeen" | "семнадцать" => 17,
        "eighteen" | "восемнадцать" => 18,
        "nineteen" | "девятнадцать" => 19,
        _ => return None,
    })
}

fn tens(w: &str) -> Option<u32> {
    Some(match w {
        "twenty" | "двадцать" => 20,
        "thirty" | "тридцать" => 30,
        "forty" | "сорок" => 40,
        "fifty" | "пятьдесят" => 50,
        "sixty" | "шестьдесят" => 60,
        "seventy" | "семьдесят" => 70,
        "eighty" | "восемьдесят" => 80,
        "ninety" | "девяносто" => 90,
        _ => return None,
    })
}

pub(super) fn number_words(toks: Vec<String>) -> Vec<String> {
    let mut out = Vec::with_capacity(toks.len());
    let mut i = 0;
    while i < toks.len() {
        if let Some(t) = tens(&toks[i]) {
            match toks.get(i + 1).and_then(|w| unit(w)).filter(|u| (1..10).contains(u)) {
                Some(u) => {
                    out.push((t + u).to_string());
                    i += 2;
                }
                None => {
                    out.push(t.to_string());
                    i += 1;
                }
            }
        } else if let Some(u) = unit(&toks[i]) {
            out.push(u.to_string());
            i += 1;
        } else {
            out.push(toks[i].clone());
            i += 1;
        }
    }
    out
}

// ---- Case suffixes of slot values -------------------------------------------------------------------------------

/// Suffix table per language: case endings that may sit on a spoken name ("калькуляторды" = calculator, accusative).
/// Longest first is enforced by `strip_suffix`, so the order here does not matter. Add other languages here.
fn suffixes(lang: &str) -> &'static [&'static str] {
    match lang {
        "kk" => &[
            "ды", "ді", "ты", "ті", "ны", "ні", "дан", "ден", "тан", "тен", "нан", "нен", "да", "де", "та", "те", "на", "не", "ға", "ге", "қа", "ке",
            "дың", "дің", "тың", "тің", "ның", "нің", "мен", "бен", "пен", "ын", "ін", "н",
        ],
        _ => &[],
    }
}

/// The stem of a word without its longest case suffix; the stem keeps at least 3 characters. `None` when nothing
/// can be cut (no suffix for the language, no match, stem too short).
pub fn strip_suffix(lang: &str, word: &str) -> Option<String> {
    let total = word.chars().count();
    suffixes(lang)
        .iter()
        .filter(|s| word.ends_with(**s) && total - s.chars().count() >= 3)
        .max_by_key(|s| s.chars().count())
        .map(|s| word[..word.len() - s.len()].to_string())
}

fn join(tokens: &[String]) -> String {
    let mut s = String::new();
    for (i, t) in tokens.iter().enumerate() {
        let cjk = |x: &str| x.chars().next().is_some_and(is_cjk);
        if i > 0 && !(cjk(t) && cjk(&tokens[i - 1])) {
            s.push(' ');
        }
        s.push_str(t);
    }
    s
}

// ---- Similarity -------------------------------------------------------------------------------------------------

fn trigrams(word: &str) -> HashSet<String> {
    let chars: Vec<char> = format!("  {word} ").chars().collect();
    chars.windows(3).map(|w| w.iter().collect()).collect()
}

/// Dice coefficient of the character trigrams of two strings (1.0 = same).
pub fn trigram_sim(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    let (x, y) = (trigrams(a), trigrams(b));
    let common = x.intersection(&y).count() as f64;
    2.0 * common / (x.len() + y.len()) as f64
}

fn word_sim(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    // Single characters (CJK) and digits must be equal.
    if a.chars().count() < 3 || b.chars().count() < 3 || a.chars().all(|c| c.is_ascii_digit()) {
        return 0.0;
    }
    // A stem inside a longer form (громче in погромче) counts as the same word.
    let (short, long) = if a.chars().count() <= b.chars().count() { (a, b) } else { (b, a) };
    if short.chars().count() >= 4 && long.contains(short) && short.chars().count() * 10 >= long.chars().count() * 6 {
        return 0.85;
    }
    trigram_sim(a, b)
}

// ---- Compile and match ------------------------------------------------------------------------------------------

fn compile(pattern: &str, intent: usize) -> Option<Compiled> {
    let mut pieces = Vec::new();
    let mut rest = pattern;
    while !rest.is_empty() {
        match rest.find('{') {
            Some(open) => {
                let lit = tokens(&rest[..open]);
                if !lit.is_empty() {
                    pieces.push(Piece::Lit(lit));
                }
                let close = rest[open..].find('}')? + open;
                pieces.push(Piece::Slot(rest[open + 1..close].trim().to_string()));
                rest = &rest[close + 1..];
            }
            None => {
                let lit = tokens(rest);
                if !lit.is_empty() {
                    pieces.push(Piece::Lit(lit));
                }
                break;
            }
        }
    }
    if pieces.is_empty() {
        return None;
    }
    let weight = pieces.iter().map(|p| if let Piece::Lit(l) = p { l.iter().map(|t| t.chars().count()).sum() } else { 0 }).sum();
    Some(Compiled { intent, pieces, weight })
}

/// Anchored match of `pieces` against `toks`: returns the slot values.
fn match_pieces(pieces: &[Piece], toks: &[String]) -> Option<Vec<(String, Vec<String>)>> {
    match pieces.split_first() {
        None => toks.is_empty().then(Vec::new),
        Some((Piece::Lit(lit), rest)) => {
            if toks.len() >= lit.len() && toks[..lit.len()] == lit[..] {
                match_pieces(rest, &toks[lit.len()..])
            } else {
                None
            }
        }
        Some((Piece::Slot(name), rest)) => {
            if rest.is_empty() {
                return (!toks.is_empty()).then(|| vec![(name.clone(), toks.to_vec())]);
            }
            // The slot takes words up to the next literal (the first position where the rest matches).
            for end in 1..=toks.len() {
                if let Some(mut tail) = match_pieces(rest, &toks[end..]) {
                    tail.insert(0, (name.clone(), toks[..end].to_vec()));
                    return Some(tail);
                }
            }
            None
        }
    }
}

/// Last words of a spoken web address.
const TLDS: &[&str] = &["com", "org", "net", "ru", "kz", "io", "dev", "app", "tv", "me", "info", "edu", "gov", "co", "uk", "de", "fr", "es", "ua", "by", "ai"];

/// Template slot -> argument name of the tool (DATA_MODEL.md 12.5).
fn arg_name(tool: &str, slot: &str) -> &'static str {
    match (slot, tool) {
        ("n", "volume") => "value",
        ("n", "remind") => "inMinutes",
        ("text", "screen_ask") => "question",
        ("text", _) => "text",
        ("item", _) => "item",
        ("list", _) => "list",
        ("lang", _) => "lang",
        ("time", _) => "time",
        ("n", _) => "minutes",
        ("app", _) | ("name", _) => "name",
        ("label", _) => "label",
        ("page", _) => "page",
        ("query", _) => "query",
        ("goal", _) => "goal",
        ("target", _) => "target",
        ("domain", _) => "domain",
        _ => "name",
    }
}

/// The code of a language said by its name (English, Russian or its own name).
fn lang_code(name: &str) -> Option<&'static str> {
    const NAMES: [(&str, &[&str]); 11] = [
        ("en", &["english", "английский", "англійски", "inglés", "anglais", "inglês", "englisch", "英语", "英語", "ағылшын"]),
        ("zh", &["chinese", "китайский", "chino", "chinois", "chinês", "chinesisch", "中文", "汉语", "中国語", "қытай"]),
        ("hi", &["hindi", "хинди", "हिंदी", "हिन्दी", "ヒンディー語", "印地语", "印地語"]),
        ("es", &["spanish", "испанский", "español", "espagnol", "espanhol", "spanisch", "西班牙语", "スペイン語", "испан"]),
        ("ar", &["arabic", "арабский", "árabe", "arabe", "árabe", "arabisch", "العربية", "阿拉伯语", "アラビア語", "араб"]),
        ("fr", &["french", "французский", "francés", "français", "francês", "französisch", "法语", "フランス語", "француз"]),
        ("pt", &["portuguese", "португальский", "portugués", "portugais", "português", "portugiesisch", "葡萄牙语", "ポルトガル語", "португал"]),
        ("ru", &["russian", "русский", "ruso", "russe", "russo", "russisch", "俄语", "ロシア語", "орыс"]),
        ("de", &["german", "немецкий", "alemán", "allemand", "alemão", "deutsch", "德语", "ドイツ語", "неміс"]),
        ("ja", &["japanese", "японский", "japonés", "japonais", "japonês", "japanisch", "日语", "日本語", "жапон"]),
        ("kk", &["kazakh", "казахский", "kazajo", "kazakh", "cazaque", "kasachisch", "哈萨克语", "カザフ語", "қазақ"]),
    ];
    let n = name.trim().to_lowercase();
    NAMES.iter().find(|(_, names)| names.iter().any(|x| *x == n)).map(|(c, _)| *c)
}

impl Table {
    pub fn parse(text: &str) -> Result<Table, String> {
        let raw: RawTable = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let mut patterns = Vec::new();
        let mut intents = Vec::new();
        let mut calc_leads = Vec::new();
        let mut remind_leads: Vec<Vec<String>> = Vec::new();
        for (i, def) in raw.intents.into_iter().enumerate() {
            if def.tool == "calc" {
                // Arithmetic keeps its symbols, which the tokens below would lose: its templates are only lead-ins before `{expr}`.
                for p in &def.patterns {
                    let lead = p.split("{expr}").next().unwrap_or_default();
                    calc_leads.push(lead.chars().filter(|c| !matches!(c, '\'' | '\u{2019}')).flat_map(char::to_lowercase).collect::<String>().trim().to_string());
                }
                intents.push((def.tool, def.args));
                continue;
            }
            for p in &def.patterns {
                let compiled = compile(p, i).ok_or_else(|| format!("bad pattern: {p}"))?;
                // "напомни мне {text} ...": the words in front of the text are the lead-in the time of a reminder is looked for after.
                if def.tool == "remind" {
                    if let [Piece::Lit(lead), Piece::Slot(slot), ..] = compiled.pieces.as_slice() {
                        if slot == "text" && !remind_leads.contains(lead) {
                            remind_leads.push(lead.clone());
                        }
                    }
                }
                patterns.push(compiled);
            }
            intents.push((def.tool, def.args));
        }
        let words = |v: Vec<String>| v.iter().map(|s| tokens(s)).filter(|t| !t.is_empty()).collect();
        let guide_controls = ["next", "back", "stop", "done"]
            .into_iter()
            .map(|k| (k, words(raw.guide_controls.get(k).cloned().unwrap_or_default())))
            .collect();
        let lower = |v: Vec<String>| v.into_iter().map(|s| s.to_lowercase()).collect();
        Ok(Table {
            lang: raw.lang,
            yes: words(raw.yes),
            no: words(raw.no),
            intents,
            patterns,
            calc_leads,
            remind_leads,
            guide_controls,
            danger_words: raw.danger_words,
            place_prepositions: lower(raw.place_prepositions),
            place_postpositions: lower(raw.place_postpositions),
            place_suffixes: lower(raw.place_suffixes),
            place_ignore: lower(raw.place_ignore),
        })
    }

    /// Whether a weather phrase names a place other than `configured` according to this language's place rules.
    pub fn names_other_place(&self, phrase: &str, configured: Option<&str>) -> bool {
        super::status::names_other_place_with(
            phrase,
            configured,
            &self.place_prepositions,
            &self.place_postpositions,
            &self.place_suffixes,
            &self.place_ignore,
        )
    }

    fn intent(&self, idx: usize, slots: &[(String, Vec<String>)]) -> Option<Intent> {
        let (tool, base) = &self.intents[idx];
        let mut args = base.clone();
        for (slot, toks) in slots {
            let name = arg_name(tool, slot);
            // A web address is spoken "github dot com" or written "github.com" (the tokens lose the dot): only an open_site takes it.
            let domain = toks.len() >= 2 && toks.last().is_some_and(|t| TLDS.contains(&t.as_str()));
            if matches!(tool.as_str(), "open_app" | "close_app" | "switch_app") && name == "name" && domain {
                return None;
            }
            if name == "domain" {
                if !domain {
                    return None;
                }
                args.insert(name.into(), json!(toks[toks.len() - 2..].join(".")));
                continue;
            }
            if name == "lang" {
                // A language said by name ("Russian", "русский") becomes its code; an unknown one is left to the model.
                args.insert(name.into(), json!(lang_code(&join(toks))?));
            } else if name == "time" {
                // "at 9:30" needs the clock to know the day: the model resolves it, not this table.
                return None;
            } else if name == "minutes" || name == "value" || name == "inMinutes" {
                args.insert(name.into(), json!(toks.first()?.parse::<u32>().ok()?));
            } else {
                args.insert(name.into(), json!(join(toks)));
            }
        }
        // Templates that cannot make a valid call alone are left to the model: a reminder without a time, a fact (its case matters).
        if (tool == "remind" && !args.contains_key("inMinutes")) || tool == "memory_add" {
            return None;
        }
        Some(Intent { tool: tool.clone(), args })
    }

    /// Matches a recognised phrase.
    pub fn matches(&self, phrase: &str) -> Match {
        self.matches_at(phrase, &crate::assistant::Local::now())
    }

    /// `matches` with the clock given: a time of the day without a day is the next such time after `now`.
    pub fn matches_at(&self, phrase: &str, now: &crate::assistant::Local) -> Match {
        let toks = tokens(phrase);
        if toks.is_empty() {
            return Match::None;
        }
        // A reminder with a time said in any order ("напомни в 2215 сделать X"): the time is worked out here, not guessed by the model.
        if let Some(intent) = super::timephrase::remind_intent(phrase, &self.remind_leads, now) {
            return Match::Hit(intent);
        }
        // Arithmetic first: its phrase has digits and signs, nothing else in the table looks like it.
        if let Some(expr) = super::calc::extract(phrase, &self.calc_leads) {
            return Match::Hit(Intent { tool: "calc".into(), args: Map::from_iter([("expr".to_string(), json!(expr))]) });
        }
        // Exact templates, skipping up to MAX_LEAD filler words in front.
        let mut hits: Vec<(usize, usize, Intent)> = Vec::new(); // (weight, lead, intent)
        for c in &self.patterns {
            for lead in 0..=MAX_LEAD.min(toks.len().saturating_sub(1)) {
                if let Some(slots) = match_pieces(&c.pieces, &toks[lead..]) {
                    if let Some(intent) = self.intent(c.intent, &slots) {
                        hits.push((c.weight, lead, intent));
                        break;
                    }
                }
            }
        }
        if let Some(best) = hits.iter().map(|h| (h.0, usize::MAX - h.1)).max() {
            let top: Vec<&Intent> = hits.iter().filter(|h| (h.0, usize::MAX - h.1) == best).map(|h| &h.2).collect();
            return unique(top);
        }
        self.fuzzy(&toks)
    }

    fn fuzzy(&self, toks: &[String]) -> Match {
        let phrase_chars: usize = toks.iter().map(|t| t.chars().count()).sum();
        let mut scored: Vec<(f64, Intent)> = Vec::new();
        for c in &self.patterns {
            let [Piece::Lit(lit)] = c.pieces.as_slice() else { continue };
            if toks.len() > lit.len() + MAX_EXTRA {
                continue;
            }
            let (mut at, mut matched, mut ok) = (0usize, 0.0f64, true);
            for word in lit {
                match (at..toks.len()).find(|&j| word_sim(word, &toks[j]) >= FUZZY_THRESHOLD) {
                    Some(j) => {
                        matched += word.chars().count() as f64 * word_sim(word, &toks[j]);
                        at = j + 1;
                    }
                    None => {
                        ok = false;
                        break;
                    }
                }
            }
            if ok {
                if let Some(intent) = self.intent(c.intent, &[]) {
                    scored.push((matched / phrase_chars.max(1) as f64, intent));
                }
            }
        }
        let Some(best) = scored.iter().map(|s| s.0).fold(None, |m: Option<f64>, x| Some(m.map_or(x, |y| y.max(x)))) else {
            return Match::None;
        };
        unique(scored.iter().filter(|s| best - s.0 < TIE_MARGIN).map(|s| &s.1).collect())
    }

    /// What is said during a guide: the whole phrase is "next", "back", "stop" or "done" (T27.5).
    pub fn guide_control(&self, phrase: &str) -> Option<&'static str> {
        let toks = tokens(phrase);
        self.guide_controls.iter().find(|(_, list)| list.contains(&toks)).map(|(k, _)| *k)
    }

    /// The words of this language that make an element dangerous to press.
    pub fn danger_words(&self) -> &[String] {
        &self.danger_words
    }

    /// A spoken answer to a confirmation: `Some(true)` yes, `Some(false)` no, `None` neither.
    pub fn answer(&self, phrase: &str) -> Option<bool> {
        let toks = tokens(phrase);
        // An answer is short. A sentence that merely begins with "yes" (a song, a talk in the room) confirms nothing.
        if toks.len() > MAX_ANSWER_WORDS {
            return None;
        }
        let first = |list: &Vec<Vec<String>>| list.iter().any(|w| toks.len() >= w.len() && toks[..w.len()] == w[..]);
        match (first(&self.yes), first(&self.no)) {
            (true, false) => Some(true),
            (false, true) => Some(false),
            _ => None,
        }
    }
}

/// One distinct intent is a hit, several are ambiguous.
fn unique(found: Vec<&Intent>) -> Match {
    let mut out: Vec<Intent> = Vec::new();
    for i in found {
        if !out.contains(i) {
            out.push(i.clone());
        }
    }
    match out.len() {
        0 => Match::None,
        1 => Match::Hit(out.remove(0)),
        _ => Match::Ambiguous(out),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_tables_carry_guide_controls_and_danger_words_and_the_m27_phrases_reach_their_tools() {
        use crate::voice::screenhelp::{all_danger_words, is_dangerous};
        let ru = Table::parse(crate::voice::host::table_text("ru")).unwrap();
        assert_eq!([ru.guide_control("стоп"), ru.guide_control("Дальше!"), ru.guide_control("назад"), ru.guide_control("готово"), ru.guide_control("привет")], [Some("stop"), Some("next"), Some("back"), Some("done"), None]);
        let words: Vec<&str> = all_danger_words().iter().map(String::as_str).collect();
        assert!(is_dangerous("Удалить всё", &words) && is_dangerous("削除する", &words) && is_dangerous("Delete", &words));
        assert!(!is_dangerous("Сохранить", &words) && !is_dangerous("保存", &words));
        let hit = |t: &Table, say: &str| match t.matches(say) {
            Match::Hit(i) => (i.tool, serde_json::Value::Object(i.args)),
            other => panic!("{say}: {other:?}"),
        };
        assert_eq!(hit(&ru, "где сохранить"), ("find_on_screen".to_string(), serde_json::json!({ "query": "сохранить" })));
        assert_eq!(hit(&ru, "не то"), ("find_on_screen".to_string(), serde_json::json!({ "next": true })));
        assert_eq!(hit(&ru, "покажи как сохранить в пдф"), ("guide".to_string(), serde_json::json!({ "goal": "сохранить в пдф" })));
        assert_eq!(hit(&ru, "нажми это"), ("ui_act".to_string(), serde_json::json!({ "action": "click", "target": "this" })));
        assert_eq!(hit(&ru, "прокрути вниз"), ("ui_act".to_string(), serde_json::json!({ "action": "scroll_down" })));
        assert_eq!(hit(&ru, "прочитай вслух").0, "read_aloud");
    }

    use super::*;

    fn en() -> Table {
        Table::parse(include_str!("../../../../shared/voice/en.json")).unwrap()
    }

    fn hit(t: &Table, say: &str) -> Intent {
        match t.matches(say) {
            Match::Hit(i) => i,
            other => panic!("{say:?} -> {other:?}"),
        }
    }

    #[test]
    fn status_phrases_reach_status_not_web_search_in_en_and_ru() {
        let ru = Table::parse(include_str!("../../../../shared/voice/ru.json")).unwrap();
        for (t, say, topic) in [
            (en(), "what is the weather", "weather"),
            (en(), "how much ram is free", "ram"),
            (en(), "next reminder", "next_reminder"),
            (ru.clone(), "какая погода", "weather"),
            (ru.clone(), "сколько места на диске", "disk"),
            (ru, "что сейчас играет", "media"),
        ] {
            let i = hit(&t, say);
            assert_eq!((i.tool.as_str(), i.args["topic"].as_str()), ("status", Some(topic)), "{say}");
        }
    }

    #[test]
    fn follow_up_phrases_in_en_and_ru() {
        let ru = Table::parse(include_str!("../../../../shared/voice/ru.json")).unwrap();
        for (t, say, kind) in [(en(), "copy it", "copy"), (en(), "insert it", "insert"), (en(), "speak it", "speak"), (en(), "another option", "again"), (ru.clone(), "вставь", "insert"), (ru.clone(), "скопируй", "copy"), (ru.clone(), "озвучь", "speak"), (ru.clone(), "скопируй", "copy"), (ru.clone(), "ещё вариант", "again")] {
            let i = hit(&t, say);
            assert_eq!((i.tool.as_str(), i.args["kind"].as_str()), ("reply_action", Some(kind)), "{say}");
        }
        // "and in German" is a translation; the agent continues the last answer when there is one.
        let i = hit(&ru, "а на немецкий");
        assert_eq!((i.tool.as_str(), i.args["op"].as_str(), i.args["lang"].as_str()), ("text_transform", Some("translate"), Some("de")));
        let i = hit(&en(), "and in german");
        assert_eq!((i.tool.as_str(), i.args["op"].as_str(), i.args["lang"].as_str()), ("text_transform", Some("translate"), Some("de")));
    }

    #[test]
    fn lyrics_window_phrases_hide_and_show_in_en_and_ru() {
        let ru = Table::parse(include_str!("../../../../shared/voice/ru.json")).unwrap();
        for (t, say, show) in [
            (en(), "hide lyrics", false),
            (en(), "show the lyrics", true),
            (ru.clone(), "скрой текст песни", false),
            (ru, "покажи текст песни", true),
        ] {
            let i = hit(&t, say);
            assert_eq!((i.tool.as_str(), i.args["show"].as_bool()), ("lyrics_window", Some(show)), "{say}");
        }
    }

    /// The eval phrases (`shared/voice/eval`) through the phrase tables, without a model: what the fast path answers right, what
    /// it answers wrong (a template that swallowed a phrase of another tool) and what it leaves to the model. A wrong answer of
    /// the fast path is a bug of the table (the model never gets a say). `cargo test -p decka-core eval_phrases -- --nocapture`.
    #[test]
    fn eval_phrases_through_the_tables() {
        use std::collections::BTreeMap;
        let sets: [(&str, &str, &str); 11] = [
            ("en", include_str!("../../../../shared/voice/en.json"), include_str!("../../../../shared/voice/eval/en.json")),
            ("zh", include_str!("../../../../shared/voice/zh.json"), include_str!("../../../../shared/voice/eval/zh.json")),
            ("hi", include_str!("../../../../shared/voice/hi.json"), include_str!("../../../../shared/voice/eval/hi.json")),
            ("es", include_str!("../../../../shared/voice/es.json"), include_str!("../../../../shared/voice/eval/es.json")),
            ("ar", include_str!("../../../../shared/voice/ar.json"), include_str!("../../../../shared/voice/eval/ar.json")),
            ("fr", include_str!("../../../../shared/voice/fr.json"), include_str!("../../../../shared/voice/eval/fr.json")),
            ("pt", include_str!("../../../../shared/voice/pt.json"), include_str!("../../../../shared/voice/eval/pt.json")),
            ("ru", include_str!("../../../../shared/voice/ru.json"), include_str!("../../../../shared/voice/eval/ru.json")),
            ("de", include_str!("../../../../shared/voice/de.json"), include_str!("../../../../shared/voice/eval/de.json")),
            ("ja", include_str!("../../../../shared/voice/ja.json"), include_str!("../../../../shared/voice/eval/ja.json")),
            ("kk", include_str!("../../../../shared/voice/kk.json"), include_str!("../../../../shared/voice/eval/kk.json")),
        ];
        let now = crate::assistant::Local::new(2026, 11, 25, 14, 30).unwrap();
        // Expected arguments must be among the answered ones (strings without case), as `tools/voice-eval.mjs` compares. Names
        // of programs, pages, buttons and the words to search come as spoken (the tools resolve them against the PC), so only
        // the fixed arguments are compared; arithmetic is compared by its value.
        let args_ok = |tool: &str, want: &Map<String, Value>, got: &Map<String, Value>| {
            if tool == "calc" {
                let (a, b) = (want.get("expr").and_then(|v| v.as_str()), got.get("expr").and_then(|v| v.as_str()));
                return matches!((a, b), (Some(a), Some(b)) if super::super::calc::eval(a).is_ok() && super::super::calc::eval(a) == super::super::calc::eval(b));
            }
            let spoken = |k: &str| {
                matches!(
                    (tool, k),
                    ("open_app" | "close_app" | "switch_app" | "go_page" | "press_button" | "open_site" | "type_text" | "list_add" | "list_show" | "list_remove" | "remind" | "memory_add" | "memory_remove" | "ask" | "screen_ask", _)
                        | ("web_search", "query")
                        | ("text_transform", "lang")
                )
            };
            want.iter().filter(|(k, _)| !spoken(k)).all(|(k, v)| got.get(k).is_some_and(|g| g == v || g.to_string().trim_matches('"').to_lowercase() == v.to_string().trim_matches('"').to_lowercase()))
        };
        let mut per_tool: BTreeMap<String, (u32, u32, u32)> = BTreeMap::new();
        let mut wrong: Vec<String> = Vec::new();
        for (lang, table, eval) in sets {
            let table = Table::parse(table).unwrap();
            let items: Vec<Value> = serde_json::from_str(eval).unwrap();
            let (mut ok, mut bad, mut miss) = (0, 0, 0);
            for item in &items {
                let (say, tool) = (item["say"].as_str().unwrap(), item["tool"].as_str().unwrap());
                let want = item.get("args").and_then(|a| a.as_object()).cloned().unwrap_or_default();
                let slot = per_tool.entry(tool.to_string()).or_default();
                match table.matches_at(say, &now) {
                    Match::Hit(i) if i.tool == "status" && i.args.get("topic").and_then(|t| t.as_str()) == Some("weather") && table.names_other_place(say, Some("Almaty")) => {
                        miss += 1;
                        slot.2 += 1;
                    }
                    Match::Hit(i) if i.tool == tool && args_ok(tool, &want, &i.args) => {
                        ok += 1;
                        slot.0 += 1;
                    }
                    // The fast path gave an answer that is not the one of the set (another tool, or other fixed arguments).
                    Match::Hit(i) => {
                        bad += 1;
                        slot.1 += 1;
                        wrong.push(format!("{lang}: {say:?} -> {} {:?}, wanted {tool} {want:?}", i.tool, i.args));
                    }
                    _ => {
                        miss += 1;
                        slot.2 += 1;
                    }
                }
            }
            println!("eval {lang}: {} phrases, fast path right {ok}, wrong {bad}, left to the model {miss}", items.len());
        }
        println!("per tool (right / wrong / model): {per_tool:?}");
        for w in &wrong {
            println!("WRONG {w}");
        }
        // Arithmetic is understood by the table, and no phrase of ru or en is answered wrongly.
        assert!(per_tool["calc"].0 >= 7, "calc fast path: {:?}", per_tool["calc"]);
        let bad_ru_en: Vec<&String> = wrong.iter().filter(|w| w.starts_with("ru:") || w.starts_with("en:")).collect();
        assert!(bad_ru_en.is_empty(), "{bad_ru_en:#?}");
    }

    /// T23.B: arithmetic is the fast path; a question is for the model (`ask`), never the web; a search stays a search.
    #[test]
    fn ten_phrases_in_two_languages_go_to_calc_ask_or_search() {
        let ru = Table::parse(include_str!("../../../../shared/voice/ru.json")).unwrap();
        let en = en();
        let calc = |t: &Table, say: &str, expr: &str| {
            let i = hit(t, say);
            assert_eq!((i.tool.as_str(), i.args["expr"].as_str()), ("calc", Some(expr)), "{say}");
        };
        calc(&ru, "54 умножить на 114", "54 умножить на 114");
        calc(&ru, "Сколько будет 15 процентов от 200?", "15 процентов от 200");
        calc(&ru, "посчитай корень из 144", "корень из 144");
        calc(&en, "calculate 54 times 114", "54 times 114");
        calc(&en, "What is 2 plus 2?", "2 plus 2");
        // Questions are not commands of the table: the model answers them with `ask`.
        for (t, say) in [(&ru, "сколько дней в високосном году"), (&ru, "почему небо голубое"), (&en, "how many days are in a leap year"), (&en, "what is the capital of France")] {
            assert_eq!(t.matches(say), Match::None, "{say}");
        }
        // A real search request still searches.
        let i = hit(&en, "search for cat videos");
        assert_eq!((i.tool.as_str(), i.args["query"].as_str()), ("web_search", Some("cat videos")));
    }

    #[test]
    fn weather_other_place_in_all_11_languages() {
        let cases: [(&str, &str, &str); 11] = [
            (include_str!("../../../../shared/voice/en.json"), "how is the weather", "what is the weather in Paris"),
            (include_str!("../../../../shared/voice/ru.json"), "какая сейчас погода", "погода в Париже"),
            (include_str!("../../../../shared/voice/de.json"), "wie wird das wetter", "wie ist das wetter in Paris"),
            (include_str!("../../../../shared/voice/fr.json"), "quel temps fait-il", "quel temps fait-il à Paris"),
            (include_str!("../../../../shared/voice/es.json"), "cómo está el clima", "qué tiempo hace en París"),
            (include_str!("../../../../shared/voice/pt.json"), "qual é o clima", "como está o tempo em Paris"),
            (include_str!("../../../../shared/voice/ar.json"), "ما حالة الطقس", "كيف الطقس في باريس"),
            (include_str!("../../../../shared/voice/hi.json"), "मौसम बताओ", "पेरिस में मौसम कैसा है"),
            (include_str!("../../../../shared/voice/ja.json"), "天気はどう", "パリの天気はどう"),
            (include_str!("../../../../shared/voice/zh.json"), "现在天气如何", "巴黎的天气怎么样"),
            (include_str!("../../../../shared/voice/kk.json"), "қазір ауа райы қалай", "Парижде ауа райы қандай"),
        ];
        for (raw, local, other) in cases {
            let t = Table::parse(raw).unwrap();
            let hit_local = hit(&t, local);
            assert_eq!((hit_local.tool.as_str(), hit_local.args["topic"].as_str()), ("status", Some("weather")), "{}: {local}", t.lang);
            assert!(!t.names_other_place(local, Some("Almaty")), "{}: {local}", t.lang);
            assert!(t.names_other_place(other, Some("Almaty")), "{}: {other}", t.lang);
        }
    }

    /// T23.B: the phrases of the owner report go through the real tables, 21:00 on a Tuesday.
    #[test]
    fn reminder_times_of_the_report_come_from_the_table_not_the_model() {
        let now = crate::assistant::Local::new(2026, 11, 24, 21, 0).unwrap();
        let ru = Table::parse(include_str!("../../../../shared/voice/ru.json")).unwrap();
        let get = |t: &Table, say: &str| match t.matches_at(say, &now) {
            Match::Hit(i) => (i.tool, i.args["text"].as_str().unwrap().to_string(), i.args.get("time").and_then(|v| v.as_str()).map(String::from), i.args.get("inMinutes").and_then(|v| v.as_u64())),
            other => panic!("{say:?} -> {other:?}"),
        };
        let rem = |text: &str, time: Option<&str>, m: Option<u64>| ("remind".to_string(), text.to_string(), time.map(String::from), m);
        assert_eq!(get(&ru, "Напомни мне в 2215 сделать дюзен"), rem("сделать дюзен", Some("22:15"), None));
        assert_eq!(get(&ru, "напомни в 22.14 сделать дизайн"), rem("сделать дизайн", Some("22:14"), None));
        assert_eq!(get(&ru, "напомни мне в 22:14 сделать дизайн"), rem("сделать дизайн", Some("22:14"), None));
        assert_eq!(get(&ru, "напомни мне в 10 вечера позвонить"), rem("позвонить", Some("22:00"), None));
        assert_eq!(get(&ru, "напомни мне через 5 минут проверить чайник"), rem("проверить чайник", None, Some(5)));
        assert_eq!(get(&en(), "remind me to call mom in 5 minutes"), rem("call mom", None, Some(5)));
        // Without a time the table leaves a reminder to the model, as before.
        assert_eq!(ru.matches_at("напомни мне купить хлеб", &now), Match::None);
    }

    #[test]
    fn normalisation_drops_punctuation_and_turns_number_words_into_digits() {
        assert_eq!(normalize("  Set a TIMER, for twenty-five minutes! "), "set a timer for 25 minutes");
        assert_eq!(normalize("Don't stop"), "dont stop");
        assert_eq!(normalize("таймер на пятнадцать минут"), "таймер на 15 минут");
        assert_eq!(tokens("打开计算器"), ["打", "开", "计", "算", "器"]);
    }

    #[test]
    fn exact_templates_fill_the_slots() {
        let t = en();
        assert_eq!(hit(&t, "Volume up").args["op"], "up");
        let i = hit(&t, "open Calculator");
        assert_eq!((i.tool.as_str(), i.args["name"].as_str()), ("open_app", Some("calculator")));
        let i = hit(&t, "set a timer for fifteen minutes");
        assert_eq!((i.tool.as_str(), i.args["minutes"].as_u64()), ("timer", Some(15)));
        let i = hit(&t, "search youtube for cat videos");
        assert_eq!((i.tool.as_str(), i.args["site"].as_str(), i.args["query"].as_str()), ("web_search", Some("youtube"), Some("cat videos")));
    }

    #[test]
    fn m22_templates_fill_the_arguments_the_tools_expect() {
        let t = en();
        let i = hit(&t, "remind me to call mom in 10 minutes");
        assert_eq!((i.tool.as_str(), i.args["text"].as_str(), i.args["inMinutes"].as_u64()), ("remind", Some("call mom"), Some(10)));
        let i = hit(&t, "translate to Russian");
        assert_eq!((i.tool.as_str(), i.args["op"].as_str(), i.args["lang"].as_str()), ("text_transform", Some("translate"), Some("ru")));
        let i = hit(&t, "add milk to shopping list");
        assert_eq!((i.tool.as_str(), i.args["item"].as_str(), i.args["list"].as_str()), ("list_add", Some("milk"), Some("shopping")));
        let i = hit(&t, "show shopping list");
        assert_eq!((i.tool.as_str(), i.args["list"].as_str()), ("list_show", Some("shopping")));
        // Left to the model: a day-less clock time, a reminder without a time, an unknown language, a fact (its case matters).
        for phrase in ["remind me to call mom at 9", "remind me to call mom", "translate to Klingon", "remember that Aigerim is my wife"] {
            assert!(!matches!(t.matches(phrase), Match::Hit(_)), "{phrase}");
        }
    }

    #[test]
    fn the_longest_literal_wins() {
        let t = en();
        // "go to {app}" also fits, but "go to page {name}" is longer.
        let i = hit(&t, "go to page media");
        assert_eq!((i.tool.as_str(), i.args["name"].as_str()), ("go_page", Some("media")));
        assert_eq!(hit(&t, "go to chrome").tool, "switch_app");
    }

    #[test]
    fn filler_words_before_a_template_are_skipped() {
        let t = en();
        let i = hit(&t, "can you open calculator");
        assert_eq!((i.tool.as_str(), i.args["name"].as_str()), ("open_app", Some("calculator")));
    }

    #[test]
    fn fuzzy_match_takes_extra_words_and_refuses_unrelated_text() {
        let t = en();
        assert_eq!(hit(&t, "can you make it louder").args["op"], "up");
        assert_eq!(hit(&t, "please lock my pc").args["op"], "lock");
        assert_eq!(t.matches("what is the capital of france today please"), Match::None);
        assert_eq!(t.matches(""), Match::None);
        assert_eq!(t.matches("..."), Match::None);
    }

    #[test]
    fn a_word_one_letter_off_is_not_enough_but_a_close_misspelling_is_not_needed_for_short_words() {
        assert!(trigram_sim("louder", "louder") == 1.0);
        assert!(trigram_sim("louder", "lauder") < FUZZY_THRESHOLD);
        assert!(trigram_sim("calculator", "calculators") >= FUZZY_THRESHOLD);
    }

    #[test]
    fn two_different_answers_of_equal_weight_are_ambiguous() {
        let t = Table::parse(
            r#"{"lang":"en","intents":[
              {"tool":"volume","args":{"op":"mute"},"patterns":["sound off"]},
              {"tool":"system","args":{"op":"sleep"},"patterns":["sound off"]},
              {"tool":"media","args":{"op":"next"},"patterns":["next"]} ]}"#,
        )
        .unwrap();
        match t.matches("sound off") {
            Match::Ambiguous(list) => assert_eq!(list.len(), 2),
            other => panic!("{other:?}"),
        }
        // The same template twice for the same intent is not ambiguous.
        let t = Table::parse(r#"{"lang":"en","intents":[{"tool":"media","args":{"op":"next"},"patterns":["next","next"]}]}"#).unwrap();
        assert!(matches!(t.matches("next"), Match::Hit(_)));
    }

    #[test]
    fn yes_and_no_are_recognised_at_the_start_of_the_answer() {
        let t = en();
        assert_eq!(t.answer("Yes, please"), Some(true));
        assert_eq!(t.answer("no"), Some(false));
        assert_eq!(t.answer("don't"), Some(false));
        assert_eq!(t.answer("maybe later"), None);
        assert_eq!(t.answer("yes I told her yesterday"), None, "talk in the room is not an answer");
    }

    #[test]
    fn a_broken_table_is_an_error() {
        assert!(Table::parse("{").is_err());
        assert!(Table::parse(r#"{"lang":"en","intents":[{"tool":"x","patterns":["{"]}]}"#).is_err());
    }

    /// Every `shared/voice/tests/<lang>.json` that exists runs through the matcher of its table.
    /// `en` must pass completely. The other tables were written for T21.3b against a matcher that did not exist yet; their
    /// misses (inflections, particles, number words) are listed in the output and held by a floor that may only rise.
    #[test]
    fn every_phrase_test_file_passes() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("shared").join("voice");
        let (mut en_failures, mut other_failures, mut other_total, mut en_total) = (Vec::new(), Vec::new(), 0, 0);
        let mut per_lang_misses: std::collections::BTreeMap<String, usize> = Default::default();
        let mut files = 0;
        for entry in std::fs::read_dir(root.join("tests")).unwrap().flatten() {
            let path = entry.path();
            let Some(lang) = path.file_stem().and_then(|s| s.to_str()).map(str::to_string) else { continue };
            files += 1;
            let table = Table::parse(&std::fs::read_to_string(root.join(format!("{lang}.json"))).unwrap()).unwrap();
            let cases: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            for case in cases {
                let say = case["say"].as_str().unwrap();
                let want_args = case["args"].as_object().cloned().unwrap_or_default();
                // A name slot may still carry a case suffix: tools.rs cuts it when the raw name matches nothing.
                let same = |got: Option<&Value>, want: &Value| {
                    got == Some(want)
                        || got.zip(want.as_str()).is_some_and(|(g, w)| g.as_str().and_then(|g| strip_suffix(&lang, g)).as_deref() == Some(w))
                };
                let bad = match table.matches(say) {
                    Match::Hit(i) if i.tool == case["tool"] && want_args.iter().all(|(k, v)| same(i.args.get(k), v)) => None,
                    other => Some(format!("{lang}: {say:?} -> {other:?}")),
                };
                if lang == "en" {
                    en_total += 1;
                } else {
                    other_total += 1;
                }
                match (bad, lang == "en") {
                    (Some(b), true) => en_failures.push(b),
                    (Some(b), false) => {
                        other_failures.push(b);
                        *per_lang_misses.entry(lang.clone()).or_insert(0) += 1;
                    }
                    (None, _) => {}
                }
            }
        }
        assert!(files >= 1);
        assert!(en_failures.is_empty(), "en must pass:\n{}", en_failures.join("\n"));
        let passed = other_total - other_failures.len();
        eprintln!("other languages: {passed} of {other_total} phrases matched; misses:\n{}", other_failures.join("\n"));
        assert!(passed + en_total >= 109, "the bar is 109 of 110: {} of {}", passed + en_total, other_total + en_total);
        assert_eq!(en_total, 10, "the bar is en 10/10");
        // Every other language misses at most one of its ten phrases.
        for (lang, misses) in &per_lang_misses {
            assert!(*misses <= 1, "{lang} missed {misses} of 10");
        }
    }

    #[test]
    fn screenshots_are_asked_for_without_the_model() {
        let ru = Table::parse(include_str!("../../../../shared/voice/ru.json")).unwrap();
        let en = en();
        let area = |t: &Table, say: &str| match t.matches(say) {
            Match::Hit(i) if i.tool == "screenshot" => i.args["area"].as_str().unwrap().to_string(),
            other => panic!("{say:?} -> {other:?}"),
        };
        for (say, want) in [("Сделай скриншот.", "screen"), ("скриншот окна", "window"), ("снимок области", "region")] {
            assert_eq!(area(&ru, say), want, "{say}");
        }
        for (say, want) in [("Take a screenshot", "screen"), ("window screenshot", "window"), ("screenshot a region", "region")] {
            assert_eq!(area(&en, say), want, "{say}");
        }
    }

    #[test]
    fn a_button_is_asked_for_by_its_name_and_page() {
        let ru = Table::parse(include_str!("../../../../shared/voice/ru.json")).unwrap();
        let hit = |say: &str| match ru.matches(say) {
            Match::Hit(i) => (i.tool, i.args.get("name").and_then(|v| v.as_str()).map(String::from), i.args.get("page").and_then(|v| v.as_str()).map(String::from)),
            other => panic!("{say:?} -> {other:?}"),
        };
        assert_eq!(hit("добавь кнопку блокировки"), ("add_button".into(), Some("блокировки".into()), None));
        assert_eq!(hit("добавь кнопку калькулятор на страницу медиа"), ("add_button".into(), Some("калькулятор".into()), Some("медиа".into())));
    }

    /// T29.2: "play music" starts it (no model needed); "pause the music" stays the toggle.
    #[test]
    fn play_music_is_a_start_and_pause_stays_a_toggle() {
        let ru = Table::parse(include_str!("../../../../shared/voice/ru.json")).unwrap();
        let en = en();
        let op = |t: &Table, say: &str| match t.matches(say) {
            Match::Hit(i) if i.tool == "media" => i.args["op"].as_str().unwrap().to_string(),
            other => panic!("{say:?} -> {other:?}"),
        };
        for say in ["Поставь музыку.", "включи музыку", "Продолжи музыку"] {
            assert_eq!(op(&ru, say), "play", "{say}");
        }
        for say in ["Play music", "play the music", "resume music"] {
            assert_eq!(op(&en, say), "play", "{say}");
        }
        for (t, say) in [(&ru, "поставь музыку на паузу"), (&ru, "пауза"), (&en, "pause the music"), (&en, "pause")] {
            assert_eq!(op(t, say), "play_pause", "{say}");
        }
    }

    #[test]
    fn kk_case_suffixes_are_cut_longest_first_and_keep_a_stem() {
        assert_eq!(strip_suffix("kk", "калькуляторды").as_deref(), Some("калькулятор"));
        assert_eq!(strip_suffix("kk", "бетін").as_deref(), Some("бет"));
        assert_eq!(strip_suffix("kk", "хромнан").as_deref(), Some("хром"));
        assert_eq!(strip_suffix("kk", "ады"), None); // stem would be 1 character
        assert_eq!(strip_suffix("kk", "калькулятор"), None);
        assert_eq!(strip_suffix("en", "calculators"), None);
    }
}
