//! The `calc` tool (DATA_MODEL.md 12.5): arithmetic is worked out here, by our own parser — never by the model and never by
//! `eval`. Input is the text of a phrase: digits, symbols (`+ - * / x × ÷ ^ % ( ) √`) and operator words of the 11 languages
//! ("54 умножить на 114", "20 percent of 150", "square root of 144"). Output is `54 × 114 = 6156`.
//!
//! Grammar (lowest binding first): `expr = term (("+" | "-") term)*`, `term = unary (("*" | "/" | "%of") unary)*`,
//! `unary = ("-" | "+") unary | "√" unary | power`, `power = postfix ("^" unary)?`, `postfix = atom "%"*`.
//! A percent on the right of `+`/`-` is a part of the left side (`100 + 10%` = 110), as on a pocket calculator.

use super::matcher::number_words;

/// Longest text of a phrase the tool takes.
pub const MAX_EXPR: usize = 200;
const MAX_DEPTH: usize = 40;

#[derive(Debug, Clone, PartialEq)]
pub enum CalcError {
    /// The text is not an expression.
    Syntax,
    DivZero,
    /// Square root of a negative number, an overflow.
    Domain,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    /// `20 percent of 150`: `l / 100 * r`.
    PctOf,
}

#[derive(Debug, Clone, PartialEq)]
enum Node {
    Num(f64),
    Neg(Box<Node>),
    Sqrt(Box<Node>),
    Pct(Box<Node>),
    Group(Box<Node>),
    Bin(Op, Box<Node>, Box<Node>),
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Sym(char),
    PctOf,
}

// ---- Operator words ---------------------------------------------------------------------------------------------

/// Phrases of the 11 languages and the symbol they stand for. Longer phrases are tried first.
const WORDS: &[(&str, &str)] = &[
    // multiply
    ("multiplied by", "*"), ("multiply by", "*"), ("times", "*"), ("умножить на", "*"), ("умножь на", "*"), ("умножить", "*"), ("умноженное на", "*"),
    ("multiplicado por", "*"), ("multiplicar por", "*"), ("por", "*"), ("multiplié par", "*"), ("multiplie par", "*"), ("fois", "*"), ("vezes", "*"),
    ("multipliziert mit", "*"), ("mal", "*"), ("乘以", "*"), ("乘", "*"), ("掛ける", "*"), ("かける", "*"), ("गुणा", "*"), ("ضرب", "*"), ("көбейту", "*"),
    ("көбейтілген", "*"),
    // divide
    ("divided by", "/"), ("divide by", "/"), ("разделить на", "/"), ("раздели на", "/"), ("поделить на", "/"), ("делить на", "/"), ("разделить", "/"),
    ("dividido por", "/"), ("dividido entre", "/"), ("entre", "/"), ("divisé par", "/"), ("divise par", "/"), ("geteilt durch", "/"), ("durch", "/"),
    ("除以", "/"), ("割る", "/"), ("割った", "/"), ("भाग", "/"), ("تقسيم", "/"), ("бөлу", "/"), ("бөлінген", "/"),
    // add
    ("plus", "+"), ("плюс", "+"), ("прибавить", "+"), ("прибавь", "+"), ("más", "+"), ("mais", "+"), ("加上", "+"), ("加", "+"), ("足す", "+"),
    ("プラス", "+"), ("प्लस", "+"), ("जमा", "+"), ("زائد", "+"), ("қосу", "+"),
    // subtract
    ("minus", "-"), ("минус", "-"), ("вычесть", "-"), ("вычти", "-"), ("отнять", "-"), ("menos", "-"), ("moins", "-"), ("减去", "-"), ("减", "-"),
    ("引く", "-"), ("マイナス", "-"), ("माइनस", "-"), ("ناقص", "-"),
    // power
    ("to the power of", "^"), ("в степени", "^"), ("hoch", "^"), ("elevado a", "^"), ("squared", "^ 2"), ("в квадрате", "^ 2"), ("cubed", "^ 3"),
    ("в кубе", "^ 3"),
    // square root
    ("square root of", "√"), ("square root", "√"), ("sqrt", "√"), ("квадратный корень из", "√"), ("квадратный корень", "√"), ("корень из", "√"),
    ("raíz cuadrada de", "√"), ("raiz cuadrada de", "√"), ("raíz cuadrada", "√"), ("racine carrée de", "√"), ("racine carrée", "√"),
    ("raiz quadrada de", "√"), ("raiz quadrada", "√"), ("quadratwurzel aus", "√"), ("quadratwurzel von", "√"), ("wurzel aus", "√"), ("wurzel", "√"),
    ("平方根", "√"), ("根号", "√"), ("ルート", "√"), ("वर्गमूल", "√"), ("الجذر التربيعي", "√"), ("квадрат түбір", "√"), ("түбір", "√"),
    // percent of (before the bare percent)
    ("percent of", "%of"), ("per cent of", "%of"), ("процентов от", "%of"), ("процента от", "%of"), ("процент от", "%of"), ("% of", "%of"), ("% от", "%of"),
    ("por ciento de", "%of"), ("pour cent de", "%of"), ("por cento de", "%of"), ("prozent von", "%of"), ("% von", "%of"),
    // percent
    ("percent", "%"), ("per cent", "%"), ("процентов", "%"), ("процента", "%"), ("процент", "%"), ("por ciento", "%"), ("pour cent", "%"),
    ("por cento", "%"), ("prozent", "%"), ("パーセント", "%"), ("प्रतिशत", "%"), ("بالمئة", "%"), ("пайыз", "%"),
];

fn is_cjk(c: char) -> bool {
    matches!(c, '\u{3040}'..='\u{30ff}' | '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}')
}

/// Replaces `phrase` in `s` where it stands as a whole word (CJK phrases anywhere).
fn replace_phrase(s: &str, phrase: &str, with: &str) -> String {
    let cjk = phrase.chars().any(is_cjk);
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if s[i..].starts_with(phrase) {
            let before = s[..i].chars().next_back();
            let after = s[i + phrase.len()..].chars().next();
            let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() && !is_cjk(c));
            let phrase_edge_alpha = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric());
            let boundary = cjk || (!word(before) || !phrase_edge_alpha(phrase.chars().next())) && (!word(after) || !phrase_edge_alpha(phrase.chars().next_back()));
            if boundary {
                out.push(' ');
                out.push_str(with);
                out.push(' ');
                i += phrase.len();
                continue;
            }
        }
        let c = s[i..].chars().next().unwrap();
        out.push(c);
        i += c.len_utf8();
    }
    out
}

// ---- Lexer ------------------------------------------------------------------------------------------------------

fn lex(text: &str) -> Result<Vec<Tok>, CalcError> {
    let mut s: String = text.chars().filter(|c| !matches!(c, '\'' | '\u{2019}')).flat_map(char::to_lowercase).collect();
    // Longest phrases first, so "умножить на" wins over "умножить".
    let mut words: Vec<&(&str, &str)> = WORDS.iter().collect();
    words.sort_by_key(|(p, _)| std::cmp::Reverse(p.chars().count()));
    for (phrase, with) in words {
        s = replace_phrase(&s, phrase, with);
    }
    let chars: Vec<char> = s.chars().collect();
    let mut toks = Vec::new();
    let mut word_run: Vec<String> = Vec::new();
    let flush = |run: &mut Vec<String>, toks: &mut Vec<Tok>| -> Result<(), CalcError> {
        if run.is_empty() {
            return Ok(());
        }
        // "fifty four" -> one number; anything else left over is not arithmetic.
        let parsed = number_words(std::mem::take(run));
        match parsed.as_slice() {
            // Digits only: "nan" and "inf" parse as numbers, they are not.
            [n] if n.chars().all(|c| c.is_ascii_digit()) => toks.push(Tok::Num(n.parse::<f64>().map_err(|_| CalcError::Syntax)?)),
            _ => return Err(CalcError::Syntax),
        }
        Ok(())
    };
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() {
            flush(&mut word_run, &mut toks)?;
            let start = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            // A decimal part: "2.5" or "2,5" (a separator that is followed by a digit).
            if i + 1 < chars.len() && matches!(chars[i], '.' | ',') && chars[i + 1].is_ascii_digit() {
                i += 1;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
            }
            let n: String = chars[start..i].iter().map(|c| if *c == ',' { '.' } else { *c }).collect();
            toks.push(Tok::Num(n.parse().map_err(|_| CalcError::Syntax)?));
        } else if c == '%' && chars.get(i + 1..i + 3) == Some(&['o', 'f']) {
            flush(&mut word_run, &mut toks)?;
            toks.push(Tok::PctOf);
            i += 3;
        } else if matches!(c, '+' | '-' | '−' | '–' | '*' | '×' | '·' | '/' | '÷' | '^' | '(' | ')' | '%' | '√') {
            flush(&mut word_run, &mut toks)?;
            toks.push(Tok::Sym(match c {
                '−' | '–' => '-',
                '×' | '·' => '*',
                '÷' => '/',
                other => other,
            }));
            i += 1;
        } else if c.is_alphabetic() && !is_cjk(c) {
            let start = i;
            while i < chars.len() && chars[i].is_alphabetic() && !is_cjk(chars[i]) {
                i += 1;
            }
            let w: String = chars[start..i].iter().collect();
            if w == "x" || w == "х" {
                flush(&mut word_run, &mut toks)?;
                toks.push(Tok::Sym('*'));
            } else if matches!(w.as_str(), "equals" | "is" | "будет" | "равно") {
                // Filler after the expression.
            } else {
                word_run.push(w);
            }
        } else if matches!(c, '=' | '?' | '.' | ',' | '!') {
            i += 1;
        } else {
            return Err(CalcError::Syntax);
        }
    }
    flush(&mut word_run, &mut toks)?;
    Ok(toks)
}

// ---- Parser -----------------------------------------------------------------------------------------------------

struct Parser {
    toks: Vec<Tok>,
    at: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.at)
    }

    fn eat(&mut self, sym: char) -> bool {
        if self.peek() == Some(&Tok::Sym(sym)) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn expr(&mut self, depth: usize) -> Result<Node, CalcError> {
        if depth > MAX_DEPTH {
            return Err(CalcError::Syntax);
        }
        let mut left = self.term(depth)?;
        loop {
            let op = if self.eat('+') {
                Op::Add
            } else if self.eat('-') {
                Op::Sub
            } else {
                return Ok(left);
            };
            let right = self.term(depth)?;
            left = Node::Bin(op, Box::new(left), Box::new(right));
        }
    }

    fn term(&mut self, depth: usize) -> Result<Node, CalcError> {
        let mut left = self.unary(depth)?;
        loop {
            let op = if self.eat('*') {
                Op::Mul
            } else if self.eat('/') {
                Op::Div
            } else if self.peek() == Some(&Tok::PctOf) {
                self.at += 1;
                Op::PctOf
            } else {
                return Ok(left);
            };
            let right = self.unary(depth)?;
            left = Node::Bin(op, Box::new(left), Box::new(right));
        }
    }

    fn unary(&mut self, depth: usize) -> Result<Node, CalcError> {
        if depth > MAX_DEPTH {
            return Err(CalcError::Syntax);
        }
        if self.eat('-') {
            return Ok(Node::Neg(Box::new(self.unary(depth + 1)?)));
        }
        if self.eat('+') {
            return self.unary(depth + 1);
        }
        if self.eat('√') {
            return Ok(Node::Sqrt(Box::new(self.unary(depth + 1)?)));
        }
        self.power(depth)
    }

    fn power(&mut self, depth: usize) -> Result<Node, CalcError> {
        let base = self.postfix(depth)?;
        if self.eat('^') {
            let exp = self.unary(depth + 1)?;
            return Ok(Node::Bin(Op::Pow, Box::new(base), Box::new(exp)));
        }
        Ok(base)
    }

    fn postfix(&mut self, depth: usize) -> Result<Node, CalcError> {
        let mut node = match self.peek().cloned() {
            Some(Tok::Num(n)) => {
                self.at += 1;
                Node::Num(n)
            }
            Some(Tok::Sym('(')) => {
                self.at += 1;
                let inner = self.expr(depth + 1)?;
                if !self.eat(')') {
                    return Err(CalcError::Syntax);
                }
                Node::Group(Box::new(inner))
            }
            _ => return Err(CalcError::Syntax),
        };
        // A percent sign right before a `%of` is the percent of that operator ("20% of 150").
        while self.peek() == Some(&Tok::Sym('%')) {
            self.at += 1;
            node = Node::Pct(Box::new(node));
        }
        Ok(node)
    }
}

fn parse_tokens(toks: Vec<Tok>) -> Result<Node, CalcError> {
    if toks.is_empty() {
        return Err(CalcError::Syntax);
    }
    let mut p = Parser { toks, at: 0 };
    let node = p.expr(0)?;
    if p.at != p.toks.len() {
        return Err(CalcError::Syntax);
    }
    Ok(node)
}

fn parse(text: &str) -> Result<Node, CalcError> {
    if text.chars().count() > MAX_EXPR {
        return Err(CalcError::Syntax);
    }
    parse_tokens(lex(text)?)
}

/// Whether the tree does arithmetic (a lone number or a negative number is not worth a calculator).
fn has_operator(n: &Node) -> bool {
    match n {
        Node::Num(_) => false,
        Node::Neg(x) | Node::Group(x) => has_operator(x),
        Node::Sqrt(_) | Node::Pct(_) | Node::Bin(..) => true,
    }
}

// ---- Evaluation and text ----------------------------------------------------------------------------------------

fn eval_node(n: &Node) -> Result<f64, CalcError> {
    let v = match n {
        Node::Num(x) => *x,
        Node::Neg(x) => -eval_node(x)?,
        Node::Group(x) => eval_node(x)?,
        Node::Pct(x) => eval_node(x)? / 100.0,
        Node::Sqrt(x) => {
            let v = eval_node(x)?;
            if v < 0.0 {
                return Err(CalcError::Domain);
            }
            v.sqrt()
        }
        Node::Bin(op, l, r) => {
            // `100 + 10%` is 110, `100 - 10%` is 90.
            if let (Op::Add | Op::Sub, Node::Pct(p)) = (op, r.as_ref()) {
                let (l, p) = (eval_node(l)?, eval_node(p)?);
                let part = l * p / 100.0;
                return finite(if *op == Op::Add { l + part } else { l - part });
            }
            let (a, b) = (eval_node(l)?, eval_node(r)?);
            match op {
                Op::Add => a + b,
                Op::Sub => a - b,
                Op::Mul => a * b,
                Op::Div => {
                    if b == 0.0 {
                        return Err(CalcError::DivZero);
                    }
                    a / b
                }
                Op::Pow => a.powf(b),
                // The left side was parsed as a percent when it had a sign: "20% of 150" is 0.2 * 150 too.
                Op::PctOf => match l.as_ref() {
                    Node::Pct(_) => a * b,
                    _ => a / 100.0 * b,
                },
            }
        }
    };
    finite(v)
}

fn finite(v: f64) -> Result<f64, CalcError> {
    if v.is_finite() { Ok(v) } else { Err(CalcError::Domain) }
}

/// `6156`, `2.5`, `0.6666666667`: whole numbers without a tail, the rest to 10 decimals without trailing zeros.
pub fn format_number(v: f64) -> String {
    let v = if v == 0.0 { 0.0 } else { v };
    if v.fract().abs() < 1e-9 && v.abs() < 1e15 {
        return format!("{}", v.round() as i64);
    }
    let s = format!("{v:.10}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn render(n: &Node) -> String {
    match n {
        Node::Num(x) => format_number(*x),
        Node::Neg(x) => format!("−{}", render(x)),
        Node::Sqrt(x) => format!("√{}", render(x)),
        Node::Pct(x) => format!("{}%", render(x)),
        Node::Group(x) => format!("({})", render(x)),
        Node::Bin(op, l, r) => {
            let sign = match op {
                Op::Add => "+",
                Op::Sub => "−",
                Op::Mul | Op::PctOf => "×",
                Op::Div => "÷",
                Op::Pow => "^",
            };
            let left = match (op, l.as_ref()) {
                (Op::PctOf, Node::Pct(_)) => render(l),
                (Op::PctOf, _) => format!("{}%", render(l)),
                _ => render(l),
            };
            if *op == Op::Pow { format!("{left}^{}", render(r)) } else { format!("{left} {sign} {}", render(r)) }
        }
    }
}

/// Whether the text is arithmetic that can be worked out (a division by zero is still an expression: the answer says so).
pub fn is_expression(text: &str) -> bool {
    parse(text).is_ok()
}

/// The value of an expression.
pub fn eval(text: &str) -> Result<f64, CalcError> {
    eval_node(&parse(text)?)
}

/// `54 × 114 = 6156`: the expression as it was understood and its value.
pub fn answer(text: &str) -> Result<String, CalcError> {
    let tree = parse(text)?;
    let v = eval_node(&tree)?;
    Ok(format!("{} = {}", render(&tree), format_number(v)))
}

/// The English texts of the failures (their i18n keys are in `pipeline::key`).
pub const DIV_ZERO: &str = "Can't divide by zero";
pub const NOT_NUMBER: &str = "Can't work that out";

/// The failure as a fixed text.
pub fn error_text(e: &CalcError) -> &'static str {
    match e {
        CalcError::DivZero => DIV_ZERO,
        CalcError::Syntax | CalcError::Domain => NOT_NUMBER,
    }
}

// ---- The fast path ----------------------------------------------------------------------------------------------

/// The expression of a phrase that starts with one of the lead-ins of the table (`"calculate"`, `"сколько будет"`, or an
/// empty one for a bare expression). `None` when the rest is not arithmetic: then the phrase goes on to the other templates.
pub fn extract(phrase: &str, leads: &[String]) -> Option<String> {
    let s: String = phrase.chars().filter(|c| !matches!(c, '\'' | '\u{2019}')).flat_map(char::to_lowercase).collect();
    let s = s.trim().trim_start_matches(['¿', '¡', ',', '.', ' ']).trim_end_matches(['?', '!', '=', ' ', '.', '。']).to_string();
    // Offsets of the words: filler words may stand in front of a lead-in ("please calculate ...").
    let mut starts = vec![0usize];
    let mut prev_space = false;
    for (i, c) in s.char_indices() {
        if c.is_whitespace() {
            prev_space = true;
        } else if prev_space {
            starts.push(i);
            prev_space = false;
        }
    }
    let mut order: Vec<&String> = leads.iter().collect();
    order.sort_by_key(|l| std::cmp::Reverse(l.chars().count()));
    for lead in order {
        let skips = if lead.is_empty() { 1 } else { starts.len().min(4) };
        for &at in &starts[..skips] {
            let Some(rest) = s[at..].strip_prefix(lead.as_str()) else { continue };
            let last = lead.chars().next_back();
            // The lead-in is a whole word unless it ends in a CJK character.
            if last.is_some_and(|c| c.is_alphanumeric() && !is_cjk(c)) && rest.chars().next().is_some_and(char::is_alphanumeric) {
                continue;
            }
            let expr = rest.trim();
            if let Ok(tree) = parse(expr) {
                if has_operator(&tree) {
                    return Some(expr.to_string());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(s: &str) -> String {
        answer(s).unwrap_or_else(|e| panic!("{s}: {e:?}"))
    }

    #[test]
    fn the_example_of_the_owner() {
        assert_eq!(ok("54 умножить на 114"), "54 × 114 = 6156");
        assert_eq!(ok("54 × 114"), "54 × 114 = 6156");
        assert_eq!(ok("54x114"), "54 × 114 = 6156");
    }

    #[test]
    fn precedence_and_parentheses() {
        assert_eq!(eval("2 + 3 * 4"), Ok(14.0));
        assert_eq!(eval("(2 + 3) * 4"), Ok(20.0));
        assert_eq!(eval("10 - 4 - 3"), Ok(3.0));
        assert_eq!(eval("100 / 5 / 2"), Ok(10.0));
        assert_eq!(eval("2 + 3 × 4 − 1"), Ok(13.0));
        assert_eq!(eval("-2 ^ 2"), Ok(-4.0));
        assert_eq!(eval("2 ^ 3 ^ 2"), Ok(512.0), "power is right-associative");
        assert_eq!(ok("(2 + 3) * 4"), "(2 + 3) × 4 = 20");
    }

    #[test]
    fn percent_power_and_roots() {
        assert_eq!(eval("50%"), Ok(0.5));
        assert_eq!(eval("100 + 10%"), Ok(110.0));
        assert_eq!(eval("100 - 10%"), Ok(90.0));
        assert_eq!(eval("20 percent of 150"), Ok(30.0));
        assert_eq!(eval("20% of 150"), Ok(30.0));
        assert_eq!(eval("20 процентов от 150"), Ok(30.0));
        assert_eq!(eval("2 ^ 10"), Ok(1024.0));
        assert_eq!(eval("5 в квадрате"), Ok(25.0));
        assert_eq!(eval("√144"), Ok(12.0));
        assert_eq!(eval("sqrt 144 + 1"), Ok(13.0));
        assert_eq!(eval("square root of 144"), Ok(12.0));
        assert_eq!(eval("квадратный корень из 144"), Ok(12.0));
        assert_eq!(ok("20 percent of 150"), "20% × 150 = 30");
        assert_eq!(ok("√144"), "√144 = 12");
    }

    #[test]
    fn division_by_zero_and_domain_errors() {
        assert_eq!(eval("5 / 0"), Err(CalcError::DivZero));
        assert_eq!(eval("5 разделить на 0"), Err(CalcError::DivZero));
        assert_eq!(eval("1 / (3 - 3)"), Err(CalcError::DivZero));
        assert_eq!(eval("sqrt(0 - 4)"), Err(CalcError::Domain));
        assert_eq!(eval("10 ^ 1000"), Err(CalcError::Domain));
        assert_eq!(error_text(&CalcError::DivZero), DIV_ZERO);
    }

    #[test]
    fn operator_and_number_words_of_several_languages() {
        assert_eq!(eval("fifty four times 114"), Ok(6156.0));
        assert_eq!(eval("двадцать пять плюс семь"), Ok(32.0));
        assert_eq!(eval("пять минус три"), Ok(2.0));
        assert_eq!(eval("10 divided by 4"), Ok(2.5));
        assert_eq!(eval("10 разделить на 4"), Ok(2.5));
        assert_eq!(eval("7 multiplied by 6"), Ok(42.0));
        assert_eq!(eval("6 por 7"), Ok(42.0));
        assert_eq!(eval("6 fois 7"), Ok(42.0));
        assert_eq!(eval("6 mal 7"), Ok(42.0));
        assert_eq!(eval("6乘以7"), Ok(42.0));
        assert_eq!(eval("2,5 + 2.5"), Ok(5.0));
    }

    #[test]
    fn what_is_not_arithmetic_is_refused() {
        for s in ["", "hello", "open calculator", "5 +", "(1 + 2", "1 + 2)", "2 3", "drop table", "1 + one two three"] {
            assert_eq!(eval(s), Err(CalcError::Syntax), "{s}");
        }
        assert_eq!(eval(&"(".repeat(150)), Err(CalcError::Syntax));
        assert_eq!(eval(&"1+".repeat(150)), Err(CalcError::Syntax), "longer than the limit");
    }

    #[test]
    fn numbers_are_written_short() {
        assert_eq!(format_number(6156.0), "6156");
        assert_eq!(format_number(2.5), "2.5");
        assert_eq!(format_number(2.0 / 3.0), "0.6666666667");
        assert_eq!(format_number(-0.0), "0");
        assert_eq!(ok("1 / 3"), "1 ÷ 3 = 0.3333333333");
    }

    #[test]
    fn the_fast_path_needs_arithmetic() {
        let leads: Vec<String> = ["calculate", "what is", "сколько будет", ""].iter().map(|s| s.to_string()).collect();
        assert_eq!(extract("Calculate 54 times 114.", &leads).as_deref(), Some("54 times 114"));
        assert_eq!(extract("what's 2 plus 2?", &["whats".to_string()]).as_deref(), Some("2 plus 2"));
        assert_eq!(extract("сколько будет 54 умножить на 114", &leads).as_deref(), Some("54 умножить на 114"));
        assert_eq!(extract("54 умножить на 114", &leads).as_deref(), Some("54 умножить на 114"));
        assert_eq!(extract("please calculate 5 + 5", &leads).as_deref(), Some("5 + 5"));
        assert_eq!(extract("what is the capital of France", &leads), None);
        assert_eq!(extract("calculate 5", &leads), None, "a lone number is not a sum");
        assert_eq!(extract("calculator", &leads), None, "the lead-in is a whole word");
        assert_eq!(extract("timer for 5 minutes", &leads), None);
    }
}
