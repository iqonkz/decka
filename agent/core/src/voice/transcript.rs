//! What the recognition heard, made fit for use (T29.2): whisper loops on short sound followed by silence and returns the
//! same phrase several times. Every phrase is kept once.

/// A block of up to this many words is looked for as a repeat.
const MAX_BLOCK: usize = 30;

/// A block of three words or more said twice in a row is one; a word or a pair only from three times ("no no" and
/// "very very good" are how people talk).
fn needed(block: usize) -> usize {
    if block >= 3 {
        2
    } else {
        3
    }
}

/// A word as it is compared: no case, no punctuation. CJK text has no spaces, so its characters are words of their own.
fn pieces(text: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut open = false;
    for c in text.chars() {
        let cjk = matches!(c as u32, 0x3040..=0x30FF | 0x3400..=0x9FFF | 0xF900..=0xFAFF);
        if c.is_whitespace() {
            if let Some(last) = out.last_mut() {
                last.0.push(c);
            }
            open = false;
        } else if cjk {
            out.push((c.to_string(), c.to_string()));
            open = false;
        } else if c.is_alphanumeric() {
            if !open {
                out.push((String::new(), String::new()));
                open = true;
            }
            let last = out.last_mut().expect("a word was just opened");
            last.0.push(c);
            last.1.extend(c.to_lowercase());
        } else {
            // Punctuation stays with the word before it (or opens the text).
            match out.last_mut() {
                Some(last) => last.0.push(c),
                None => out.push((c.to_string(), String::new())),
            }
        }
    }
    // Marks without letters (a leading quote) join the following word.
    let mut merged: Vec<(String, String)> = Vec::new();
    let mut lead = String::new();
    for (shown, key) in out {
        if key.is_empty() {
            lead.push_str(&shown);
        } else {
            merged.push((std::mem::take(&mut lead) + &shown, key));
        }
    }
    if let Some(last) = merged.last_mut() {
        last.0.push_str(&lead);
    }
    merged
}

/// How many times the `block` words at `at` follow each other.
fn times(words: &[(String, String)], at: usize, block: usize) -> usize {
    let mut n = 1;
    while at + (n + 1) * block <= words.len() && (0..block).all(|i| words[at + i].1 == words[at + n * block + i].1) {
        n += 1;
    }
    n
}

pub fn collapse_repeats(text: &str) -> String {
    let words = pieces(text);
    let mut kept: Vec<&str> = Vec::with_capacity(words.len());
    let (mut at, mut collapsed) = (0, false);
    while at < words.len() {
        // The shortest repeating block first: "a b a b a b" is "a b", not "a b a b".
        let found = (1..=MAX_BLOCK.min((words.len() - at) / 2)).find_map(|block| {
            let n = times(&words, at, block);
            (n >= needed(block)).then_some((block, n))
        });
        match found {
            Some((block, n)) => {
                // The last copy ends the way the text ended it.
                kept.extend(words[at + (n - 1) * block..at + n * block].iter().map(|w| w.0.as_str()));
                at += n * block;
                collapsed = true;
            }
            None => {
                kept.push(words[at].0.as_str());
                at += 1;
            }
        }
    }
    if !collapsed {
        return text.trim().to_string();
    }
    let joined = kept.concat().trim().to_string();
    // A second pass: collapsing the inside may have brought two copies together.
    if joined.len() < text.trim().len() {
        collapse_repeats(&joined)
    } else {
        joined
    }
}

/// T32.H.2: a recognition that was given words to expect may answer noise with those very words. That is so when the text
/// is a run of three or more entries of the prompt, from the start of one to the end of another.
pub fn echoes_prompt(text: &str, prompt: &str) -> bool {
    let heard = super::matcher::tokens(text);
    let entries: Vec<Vec<String>> = prompt.split(". ").map(super::matcher::tokens).collect();
    (0..entries.len()).any(|from| {
        let mut run: Vec<&String> = Vec::new();
        entries[from..].iter().enumerate().any(|(n, entry)| {
            run.extend(entry);
            n >= 2 && run.len() == heard.len() && run.iter().zip(&heard).all(|(a, b)| *a == b)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_phrase_that_whisper_repeats_is_kept_once() {
        assert_eq!(collapse_repeats("Что на экране? Что на экране? Что на экране? Что на экране?"), "Что на экране?");
        assert_eq!(collapse_repeats("что на экране Что на экране"), "Что на экране");
        assert_eq!(collapse_repeats("Объясни это. Объясни это. Объясни это."), "Объясни это.");
        assert_eq!(collapse_repeats("Второй раз - текст в блокноте. Второй раз - текст в блокноте."), "Второй раз - текст в блокноте.");
        assert_eq!(collapse_repeats("Привет. Что на экране? Что на экране?"), "Привет. Что на экране?");
        assert_eq!(collapse_repeats("Да. Да. Да. Да."), "Да.");
        assert_eq!(collapse_repeats("屏幕上有什么屏幕上有什么"), "屏幕上有什么");
    }

    #[test]
    fn ordinary_speech_is_left_alone() {
        for text in [
            "Да, да, конечно",
            "это очень очень хорошо",
            "раз, два, три, проверка диктовки.",
            "no no",
            "Я пришёл, увидел и победил.",
            "the more the merrier, the more the better",
            "",
        ] {
            assert_eq!(collapse_repeats(text), text);
        }
    }

    #[test]
    fn the_prompt_said_back_is_noise_and_a_command_is_not() {
        use super::echoes_prompt;
        let prompt = "Декка. Открой. Закрой. Переключись на. Выключи звук.";
        assert!(echoes_prompt("открой, закрой, переключись на", prompt));
        assert!(echoes_prompt("Декка открой закрой", prompt));
        for said in ["выключи звук", "переключись на хром", "открой закрой", "закрой", ""] {
            assert!(!echoes_prompt(said, prompt), "{said}");
        }
    }
}
