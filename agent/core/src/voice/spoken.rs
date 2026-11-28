//! T33.2: a text as it is to be read aloud, for any voice: without markup, code, emoji and addresses, with a pause at the
//! end of every line, and - for Russian, English and Kazakh - numbers, times, dates and units in words. Other languages
//! keep their numbers for the voice to read. The words are the nominative ones; of the cases a sentence asks for only
//! those that would sound plainly wrong are worked out: the date of a month, the number of a year, and the Russian
//! genitive after "из", "от", "до", "без", "около", "более", "менее".

mod numbers;

use numbers::{Gender, Lang};

/// The word that stands for an address of a page.
fn link_word(lang: &str) -> &'static str {
    match lang {
        "ru" => "ссылка",
        "kk" => "сілтеме",
        "es" => "enlace",
        "de" | "pt" => "link",
        "fr" => "lien",
        "ar" => "رابط",
        "zh" => "链接",
        "ja" => "リンク",
        "hi" => "लिंक",
        _ => "link",
    }
}

/// The language the numbers of a text are to be said in: Cyrillic text is Kazakh or Russian whatever the interface says;
/// Latin text is English only when the interface is. `hint` is the language of the interface or of the answer.
pub fn language_of<'a>(text: &str, hint: &'a str) -> &'a str {
    let (mut cyrillic, mut letters, mut kazakh) = (0usize, 0usize, false);
    for c in text.chars().flat_map(char::to_lowercase) {
        if c.is_alphabetic() {
            letters += 1;
            if matches!(c, '\u{0400}'..='\u{04ff}') {
                cyrillic += 1;
                kazakh |= matches!(c, 'ә' | 'ғ' | 'қ' | 'ң' | 'ө' | 'ұ' | 'ү' | 'һ');
            }
        }
    }
    if cyrillic * 2 > letters {
        if kazakh {
            "kk"
        } else {
            "ru"
        }
    } else if letters > 0 && matches!(hint, "ru" | "kk") {
        // Latin text under a Russian or Kazakh interface: the default voice reads it as it is.
        "en"
    } else {
        hint
    }
}

fn is_pictograph(c: char) -> bool {
    matches!(c, '\u{1f000}'..='\u{1faff}' | '\u{2600}'..='\u{27bf}' | '\u{2b00}'..='\u{2bff}' | '\u{2190}'..='\u{21ff}' | '\u{25a0}'..='\u{25ff}' | '\u{fe00}'..='\u{fe0f}' | '\u{200d}' | '\u{20e3}' | '\u{e0020}'..='\u{e007f}')
}

/// `[label](address)` and `![label](address)` become the label.
fn unlink(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(open) = rest.find('[') {
        let found = rest[open..].find("](").and_then(|mid| {
            let mid = open + mid;
            rest[mid..].find(')').map(|end| (mid, mid + end))
        });
        let Some((mid, end)) = found else { break };
        out.push_str(rest[..open].strip_suffix('!').unwrap_or(&rest[..open]));
        out.push_str(&rest[open + 1..mid]);
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Removes `<tags>`; a `<` that no letter follows stays (a comparison).
fn untag(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(open) = rest.find('<') {
        let tag = rest[open + 1..].starts_with(|c: char| c.is_ascii_alphabetic() || c == '/');
        match rest[open..].find('>').filter(|_| tag) {
            Some(end) => {
                out.push_str(&rest[..open]);
                rest = &rest[open + end + 1..];
            }
            None => {
                out.push_str(&rest[..=open]);
                rest = &rest[open + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn unaddress(word: &str, lang: &str) -> String {
    let start = ["http://", "https://", "www."].iter().filter_map(|p| word.to_ascii_lowercase().find(p)).min();
    match start {
        Some(at) => {
            let tail: String = word.chars().rev().take_while(|c| matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | ')' | '"' | '»')).collect::<Vec<_>>().into_iter().rev().collect();
            format!("{}{}{tail}", &word[..at], link_word(lang))
        }
        None => word.to_string(),
    }
}

/// One line of a text without its markup; empty when nothing of it is to be read.
fn clean_line(line: &str, lang: &str) -> String {
    let mut line = line.trim();
    // A rule and the line under the head of a table.
    if !line.is_empty() && line.chars().all(|c| matches!(c, '-' | '=' | '*' | '_' | '|' | ':' | ' ')) {
        return String::new();
    }
    loop {
        let next = line.trim_start_matches(['#', '>']).trim_start();
        let next = ["- ", "* ", "+ ", "• "].iter().find_map(|b| next.strip_prefix(b)).unwrap_or(next).trim_start();
        if next == line {
            break;
        }
        line = next;
    }
    let mut text = untag(&unlink(line));
    if text.starts_with('|') {
        text = text.split('|').map(str::trim).filter(|c| !c.is_empty()).collect::<Vec<_>>().join(", ");
    }
    for mark in ["**", "__", "~~", "`"] {
        text = text.replace(mark, "");
    }
    text.chars()
        .filter(|c| !is_pictograph(*c))
        .collect::<String>()
        .split_whitespace()
        // A star or an underscore at the edge of a word is emphasis; alone between spaces a star is a sign of a sum.
        .map(|w| if w.chars().count() > 1 { w.trim_matches(['*', '_']) } else { w })
        .map(|w| unaddress(w, lang))
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The text without markup and code, every line closed so that the voice pauses after it.
fn clean(text: &str, lang: &str) -> String {
    let mut code = false;
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            code = !code;
            continue;
        }
        if code {
            continue;
        }
        let mut line = clean_line(line, lang);
        if line.is_empty() {
            continue;
        }
        if !line.ends_with(['.', '!', '?', ':', ';', ',', '…', '。', '！', '？', '؟', '।']) {
            line.push('.');
        }
        out.push(line);
    }
    out.join(" ")
}

/// The text as it is to be read in `lang`.
pub fn for_speech(text: &str, lang: &str) -> String {
    let cleaned = clean(text, lang);
    match Lang::of(lang) {
        Some(lang) => expand(&cleaned, lang),
        None => cleaned,
    }
}

// ---- Numbers in their places -------------------------------------------------------------------------------------

struct Reader<'a> {
    chars: &'a [char],
    at: usize,
}

impl Reader<'_> {
    fn peek(&self, ahead: usize) -> Option<char> {
        self.chars.get(self.at + ahead).copied()
    }

    /// Digits from the place `from` ahead; returns them and how far they reach.
    fn digits(&self, from: usize) -> (String, usize) {
        let mut end = from;
        let mut found = String::new();
        while let Some(c) = self.peek(end).filter(char::is_ascii_digit) {
            found.push(c);
            end += 1;
        }
        (found, end)
    }
}

/// The word before the place where `out` ends, in lower case.
fn last_word(out: &str) -> String {
    out.split_whitespace().next_back().unwrap_or_default().trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase()
}

fn expand(text: &str, lang: Lang) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut r = Reader { chars: &chars, at: 0 };
    let mut out = String::new();
    while let Some(c) = r.peek(0) {
        let before = r.at.checked_sub(1).map(|i| chars[i]);
        let glued = before.is_some_and(|b| b.is_alphanumeric());
        // A sign belongs to a number only where no word or number is in front of it.
        if matches!(c, '+' | '-' | '−') && !glued && r.peek(1).is_some_and(|d| d.is_ascii_digit()) {
            out.push_str(lang.sign(c == '+'));
            out.push(' ');
            r.at += 1;
            continue;
        }
        if !c.is_ascii_digit() || glued {
            // Digits inside a word ("mp3", "A4") are a name: the voice reads them.
            out.push(c);
            r.at += 1;
            continue;
        }
        number(&mut r, &mut out, lang);
    }
    out
}

/// Reads the number at the reader and writes its words.
fn number(r: &mut Reader, out: &mut String, lang: Lang) {
    let (whole, mut end) = r.digits(0);

    // 28.11.2026 and 2026-11-28.
    for (separator, year_first) in [('.', false), ('-', true)] {
        let (middle, e1) = r.digits(end + 1);
        let (last, e2) = r.digits(e1 + 1);
        if r.peek(end) == Some(separator) && r.peek(e1) == Some(separator) && !last.is_empty() {
            let (d, m, y) = if year_first { (&last, &middle, &whole) } else { (&whole, &middle, &last) };
            if y.len() == 4 && d.len() <= 2 && m.len() <= 2 {
                if let (Ok(d @ 1..=31), Ok(m @ 1..=12), Ok(y)) = (d.parse::<u64>(), m.parse::<usize>(), y.parse::<u64>()) {
                    let word = last_word(out);
                    out.push_str(&lang.date(d, m, y, &word));
                    r.at += e2;
                    return;
                }
            }
        }
    }
    // 14:30 and 14:30:05.
    if r.peek(end) == Some(':') && whole.len() <= 2 {
        let (minutes, e1) = r.digits(end + 1);
        if minutes.len() == 2 {
            if let (Ok(h @ 0..=23), Ok(m @ 0..=59)) = (whole.parse::<u64>(), minutes.parse::<u64>()) {
                out.push_str(&lang.time(h, m));
                r.at += e1;
                // Seconds are not said.
                if r.peek(0) == Some(':') && r.digits(1).0.len() == 2 {
                    r.at += 3;
                }
                return;
            }
        }
    }
    // 0.70.1, 192.168.0.1: said part by part.
    let mut parts = vec![whole.clone()];
    let mut e = end;
    while r.peek(e) == Some('.') && !r.digits(e + 1).0.is_empty() {
        let (next, after) = r.digits(e + 1);
        parts.push(next);
        e = after;
    }
    if parts.len() > 2 {
        let said: Vec<String> = parts.iter().map(|p| lang.digits_or_number(p, Gender::Masculine)).collect();
        out.push_str(&said.join(&format!(" {} ", lang.dot())));
        r.at += e;
        return;
    }
    // Groups of thousands: "12 345" in Russian and Kazakh, "12,345" in English.
    let mut digits = whole.clone();
    if digits.len() <= 3 && digits != "0" {
        loop {
            let separator = r.peek(end);
            let grouped = match lang {
                Lang::En => separator == Some(','),
                _ => matches!(separator, Some(' ' | '\u{a0}' | '\u{202f}')),
            };
            let (group, after) = r.digits(end + 1);
            if !grouped || group.len() != 3 {
                break;
            }
            digits.push_str(&group);
            end = after;
        }
    }
    // A fraction: "3,5" and "3.5" ("3,5" of an English text is a list).
    let mut fraction = String::new();
    if r.peek(end) == Some('.') || (lang != Lang::En && r.peek(end) == Some(',')) {
        let (part, after) = r.digits(end + 1);
        if !part.is_empty() {
            fraction = part;
            end = after;
        }
    }
    r.at += end;

    let gap = usize::from(matches!(r.peek(0), Some(' ' | '\u{a0}' | '\u{202f}')));
    let value: Option<u64> = digits.parse().ok().filter(|_| digits.len() <= 12 && !(digits.len() > 1 && digits.starts_with('0')));
    let Some(value) = value else {
        // A code, a phone number: digit by digit.
        out.push_str(&lang.each_digit(&digits));
        return;
    };
    // A year: "2026 г.", "2026 года".
    if fraction.is_empty() && digits.len() == 4 && (1000..=2100).contains(&value) {
        let word = last_word(out);
        if let Some((said, taken)) = lang.year_before(value, &word, &r.chars[r.at..]) {
            out.push_str(&said);
            r.at += taken;
            return;
        }
    }
    // The day of a month: "28 ноября", "November 28".
    if fraction.is_empty() && (1..=31).contains(&value) {
        let rest: String = r.chars[r.at..].iter().take(14).collect();
        let word = last_word(out);
        if let Some(said) = lang.day_of_month(value, &word, rest.trim_start()) {
            out.push_str(&said);
            return;
        }
    }
    let genitive = fraction.is_empty() && lang.asks_genitive(&last_word(out));
    if let Some((unit, taken)) = lang.unit(&r.chars[r.at + gap..]) {
        out.push_str(&if genitive { lang.measure_genitive(value, unit) } else { lang.measure(value, &fraction, unit) });
        r.at += gap + taken;
        // A short form with a dot at the end of a sentence keeps the pause of that dot.
        if r.chars[r.at - 1] == '.' && r.chars[r.at..].iter().find(|c| !c.is_whitespace()).is_none_or(|c| c.is_uppercase()) {
            out.push('.');
        }
        return;
    }
    // "3D", "5G": a name.
    if r.peek(0).is_some_and(char::is_alphabetic) {
        out.push_str(&digits);
        if !fraction.is_empty() {
            out.push('.');
            out.push_str(&fraction);
        }
        return;
    }
    let next: String = r.chars[r.at + gap..].iter().take_while(|c| c.is_alphabetic()).collect();
    let gender = lang.gender_before(&next);
    out.push_str(&if genitive {
        lang.genitive(value, gender)
    } else if fraction.is_empty() {
        lang.cardinal(value, gender)
    } else {
        lang.decimal(value, &fraction)
    });
}

#[cfg(test)]
mod tests;
