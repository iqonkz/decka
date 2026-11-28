//! Numbers, times, dates and units of Russian, English and Kazakh in words (T33.2).

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Lang {
    Ru,
    En,
    Kk,
}

/// The gender of the Russian noun a number stands in front of.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Gender {
    Masculine,
    Feminine,
    Neuter,
}

/// The forms of a Russian ordinal that dates and years ask for.
#[derive(Clone, Copy)]
enum Form {
    Nominative,
    Neuter,
    Genitive,
    Prepositional,
}

/// A unit of measure: how it is written and how each language says it.
pub struct Unit {
    spelled: &'static [&'static str],
    /// Russian after 1, after 2 to 4, after 5; what follows the noun ("в час").
    ru: [&'static str; 3],
    ru_after: &'static str,
    gender: Gender,
    /// English singular and plural.
    en: [&'static str; 2],
    /// Kazakh: the word in front of the number ("сағатына") and the noun.
    kk: [&'static str; 2],
}

macro_rules! unit {
    ([$($s:literal),+], $ru:expr, $after:literal, $gender:ident, $en:expr, $kk:expr) => {
        Unit { spelled: &[$($s),+], ru: $ru, ru_after: $after, gender: Gender::$gender, en: $en, kk: $kk }
    };
}

const UNITS: &[Unit] = &[
    unit!(["%"], ["процент", "процента", "процентов"], "", Masculine, ["percent", "percent"], ["", "пайыз"]),
    unit!(["°C", "°С", "℃", "°"], ["градус", "градуса", "градусов"], "", Masculine, ["degree", "degrees"], ["", "градус"]),
    unit!(["°F", "℉"], ["градус", "градуса", "градусов"], " по Фаренгейту", Masculine, ["degree Fahrenheit", "degrees Fahrenheit"], ["", "градус Фаренгейт"]),
    unit!(["ТБ", "Тб", "TB"], ["терабайт", "терабайта", "терабайт"], "", Masculine, ["terabyte", "terabytes"], ["", "терабайт"]),
    unit!(["ГБ", "Гб", "GB"], ["гигабайт", "гигабайта", "гигабайт"], "", Masculine, ["gigabyte", "gigabytes"], ["", "гигабайт"]),
    unit!(["МБ", "Мб", "MB"], ["мегабайт", "мегабайта", "мегабайт"], "", Masculine, ["megabyte", "megabytes"], ["", "мегабайт"]),
    unit!(["КБ", "Кб", "KB", "kB"], ["килобайт", "килобайта", "килобайт"], "", Masculine, ["kilobyte", "kilobytes"], ["", "килобайт"]),
    unit!(["Гбит/с", "Gbps", "Gbit/s"], ["гигабит", "гигабита", "гигабит"], " в секунду", Masculine, ["gigabit per second", "gigabits per second"], ["секундына", "гигабит"]),
    unit!(["Мбит/с", "Mbps", "Mbit/s"], ["мегабит", "мегабита", "мегабит"], " в секунду", Masculine, ["megabit per second", "megabits per second"], ["секундына", "мегабит"]),
    unit!(["Кбит/с", "кбит/с", "Kbps", "kbps"], ["килобит", "килобита", "килобит"], " в секунду", Masculine, ["kilobit per second", "kilobits per second"], ["секундына", "килобит"]),
    unit!(["км/ч", "км/сағ", "km/h"], ["километр", "километра", "километров"], " в час", Masculine, ["kilometer per hour", "kilometers per hour"], ["сағатына", "километр"]),
    unit!(["mph"], ["миля", "мили", "миль"], " в час", Feminine, ["mile per hour", "miles per hour"], ["сағатына", "миля"]),
    unit!(["м/с", "m/s"], ["метр", "метра", "метров"], " в секунду", Masculine, ["meter per second", "meters per second"], ["секундына", "метр"]),
    unit!(["мм рт. ст.", "мм рт.ст."], ["миллиметр", "миллиметра", "миллиметров"], " ртутного столба", Masculine, ["millimeter of mercury", "millimeters of mercury"], ["", "миллиметр сынап бағанасы"]),
    unit!(["ГГц", "GHz"], ["гигагерц", "гигагерца", "гигагерц"], "", Masculine, ["gigahertz", "gigahertz"], ["", "гигагерц"]),
    unit!(["МГц", "MHz"], ["мегагерц", "мегагерца", "мегагерц"], "", Masculine, ["megahertz", "megahertz"], ["", "мегагерц"]),
    unit!(["км", "km"], ["километр", "километра", "километров"], "", Masculine, ["kilometer", "kilometers"], ["", "километр"]),
    unit!(["см", "cm"], ["сантиметр", "сантиметра", "сантиметров"], "", Masculine, ["centimeter", "centimeters"], ["", "сантиметр"]),
    unit!(["мм", "mm"], ["миллиметр", "миллиметра", "миллиметров"], "", Masculine, ["millimeter", "millimeters"], ["", "миллиметр"]),
    unit!(["кг", "kg"], ["килограмм", "килограмма", "килограммов"], "", Masculine, ["kilogram", "kilograms"], ["", "килограмм"]),
    unit!(["мс", "ms"], ["миллисекунда", "миллисекунды", "миллисекунд"], "", Feminine, ["millisecond", "milliseconds"], ["", "миллисекунд"]),
    unit!(["мин.", "мин", "min"], ["минута", "минуты", "минут"], "", Feminine, ["minute", "minutes"], ["", "минут"]),
    unit!(["сек.", "сек", "sec"], ["секунда", "секунды", "секунд"], "", Feminine, ["second", "seconds"], ["", "секунд"]),
    unit!(["сағ"], ["час", "часа", "часов"], "", Masculine, ["hour", "hours"], ["", "сағат"]),
    unit!(["ч.", "ч", "h"], ["час", "часа", "часов"], "", Masculine, ["hour", "hours"], ["", "сағат"]),
    unit!(["м"], ["метр", "метра", "метров"], "", Masculine, ["meter", "meters"], ["", "метр"]),
    unit!(["₽", "руб.", "руб"], ["рубль", "рубля", "рублей"], "", Masculine, ["ruble", "rubles"], ["", "рубль"]),
    unit!(["₸", "тг"], ["тенге", "тенге", "тенге"], "", Masculine, ["tenge", "tenge"], ["", "теңге"]),
    unit!(["$"], ["доллар", "доллара", "долларов"], "", Masculine, ["dollar", "dollars"], ["", "доллар"]),
    unit!(["€"], ["евро", "евро", "евро"], "", Masculine, ["euro", "euros"], ["", "еуро"]),
];

const RU_MONTHS: [&str; 12] = ["января", "февраля", "марта", "апреля", "мая", "июня", "июля", "августа", "сентября", "октября", "ноября", "декабря"];
const EN_MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const KK_MONTHS: [&str; 12] = ["қаңтар", "ақпан", "наурыз", "сәуір", "мамыр", "маусым", "шілде", "тамыз", "қыркүйек", "қазан", "қараша", "желтоқсан"];

// ---- Russian -----------------------------------------------------------------------------------------------------

const RU_ONES: [&str; 20] = ["ноль", "один", "два", "три", "четыре", "пять", "шесть", "семь", "восемь", "девять", "десять", "одиннадцать", "двенадцать", "тринадцать", "четырнадцать", "пятнадцать", "шестнадцать", "семнадцать", "восемнадцать", "девятнадцать"];
const RU_TENS: [&str; 10] = ["", "", "двадцать", "тридцать", "сорок", "пятьдесят", "шестьдесят", "семьдесят", "восемьдесят", "девяносто"];
const RU_HUNDREDS: [&str; 10] = ["", "сто", "двести", "триста", "четыреста", "пятьсот", "шестьсот", "семьсот", "восемьсот", "девятьсот"];
const RU_ORD: [&str; 20] = ["нулевой", "первый", "второй", "третий", "четвёртый", "пятый", "шестой", "седьмой", "восьмой", "девятый", "десятый", "одиннадцатый", "двенадцатый", "тринадцатый", "четырнадцатый", "пятнадцатый", "шестнадцатый", "семнадцатый", "восемнадцатый", "девятнадцатый"];
const RU_ORD_TENS: [&str; 10] = ["", "", "двадцатый", "тридцатый", "сороковой", "пятидесятый", "шестидесятый", "семидесятый", "восьмидесятый", "девяностый"];
const RU_ORD_HUNDREDS: [&str; 10] = ["", "сотый", "двухсотый", "трёхсотый", "четырёхсотый", "пятисотый", "шестисотый", "семисотый", "восьмисотый", "девятисотый"];

/// Which of the three forms of a Russian noun follows `n`.
fn ru_form(n: u64) -> usize {
    match (n % 100, n % 10) {
        (11..=14, _) => 2,
        (_, 1) => 0,
        (_, 2..=4) => 1,
        _ => 2,
    }
}

fn ru_below_thousand(n: u64, gender: Gender, out: &mut Vec<&'static str>) {
    if n >= 100 {
        out.push(RU_HUNDREDS[(n / 100) as usize]);
    }
    let mut rest = n % 100;
    if rest >= 20 {
        out.push(RU_TENS[(rest / 10) as usize]);
        rest %= 10;
    }
    if rest > 0 {
        out.push(match (rest, gender) {
            (1, Gender::Feminine) => "одна",
            (1, Gender::Neuter) => "одно",
            (2, Gender::Feminine) => "две",
            _ => RU_ONES[rest as usize],
        });
    }
}

fn ru_cardinal(n: u64, gender: Gender) -> String {
    if n == 0 {
        return RU_ONES[0].into();
    }
    let mut out = Vec::new();
    let scales: [(u64, [&str; 3], Gender); 3] = [
        (1_000_000_000, ["миллиард", "миллиарда", "миллиардов"], Gender::Masculine),
        (1_000_000, ["миллион", "миллиона", "миллионов"], Gender::Masculine),
        (1_000, ["тысяча", "тысячи", "тысяч"], Gender::Feminine),
    ];
    let mut rest = n;
    for (size, forms, of) in scales {
        let many = rest / size;
        if many > 0 {
            ru_below_thousand(many % 1000, of, &mut out);
            out.push(forms[ru_form(many)]);
            rest %= size;
        }
    }
    ru_below_thousand(rest, gender, &mut out);
    out.join(" ")
}

const RU_ONES_GEN: [&str; 20] = ["нуля", "одного", "двух", "трёх", "четырёх", "пяти", "шести", "семи", "восьми", "девяти", "десяти", "одиннадцати", "двенадцати", "тринадцати", "четырнадцати", "пятнадцати", "шестнадцати", "семнадцати", "восемнадцати", "девятнадцати"];
const RU_TENS_GEN: [&str; 10] = ["", "", "двадцати", "тридцати", "сорока", "пятидесяти", "шестидесяти", "семидесяти", "восьмидесяти", "девяноста"];
const RU_HUNDREDS_GEN: [&str; 10] = ["", "ста", "двухсот", "трёхсот", "четырёхсот", "пятисот", "шестисот", "семисот", "восьмисот", "девятисот"];

fn ru_below_thousand_genitive(n: u64, gender: Gender, out: &mut Vec<&'static str>) {
    if n >= 100 {
        out.push(RU_HUNDREDS_GEN[(n / 100) as usize]);
    }
    let mut rest = n % 100;
    if rest >= 20 {
        out.push(RU_TENS_GEN[(rest / 10) as usize]);
        rest %= 10;
    }
    if rest > 0 {
        out.push(if rest == 1 && gender == Gender::Feminine { "одной" } else { RU_ONES_GEN[rest as usize] });
    }
}

fn ru_genitive(n: u64, gender: Gender) -> String {
    if n == 0 {
        return RU_ONES_GEN[0].into();
    }
    let mut out = Vec::new();
    // The noun after 1, 21, 31... and after every other number.
    let scales: [(u64, [&str; 2], Gender); 3] = [
        (1_000_000_000, ["миллиарда", "миллиардов"], Gender::Masculine),
        (1_000_000, ["миллиона", "миллионов"], Gender::Masculine),
        (1_000, ["тысячи", "тысяч"], Gender::Feminine),
    ];
    let mut rest = n;
    for (size, forms, of) in scales {
        let many = rest / size;
        if many > 0 {
            ru_below_thousand_genitive(many % 1000, of, &mut out);
            out.push(forms[usize::from(ru_form(many) != 0)]);
            rest %= size;
        }
    }
    ru_below_thousand_genitive(rest, gender, &mut out);
    out.join(" ")
}

/// An ordinal up to 9999: the last part is declined, what is in front of it stays a number.
fn ru_ordinal(n: u64, form: Form) -> String {
    let (front, last): (u64, &str) = if (1..20).contains(&(n % 100)) {
        (n - n % 100, RU_ORD[(n % 100) as usize])
    } else if !n.is_multiple_of(10) {
        (n - n % 10, RU_ORD[(n % 10) as usize])
    } else if !n.is_multiple_of(100) {
        (n - n % 100, RU_ORD_TENS[(n % 100 / 10) as usize])
    } else if !n.is_multiple_of(1000) {
        (n - n % 1000, RU_ORD_HUNDREDS[(n % 1000 / 100) as usize])
    } else {
        (0, if n == 1000 { "тысячный" } else if n == 2000 { "двухтысячный" } else { RU_ORD[0] })
    };
    let chars: Vec<char> = last.chars().collect();
    let stem: String = chars[..chars.len() - 2].iter().collect();
    let word = match (form, last == "третий") {
        (Form::Nominative, _) => last.to_string(),
        (Form::Neuter, true) => "третье".into(),
        (Form::Genitive, true) => "третьего".into(),
        (Form::Prepositional, true) => "третьем".into(),
        (Form::Neuter, false) => format!("{stem}ое"),
        (Form::Genitive, false) => format!("{stem}ого"),
        (Form::Prepositional, false) => format!("{stem}ом"),
    };
    if front == 0 {
        word
    } else {
        // A year is "тысяча девятьсот...", not "одна тысяча".
        let front = ru_cardinal(front, Gender::Masculine);
        format!("{} {word}", front.strip_prefix("одна ").unwrap_or(&front))
    }
}

// ---- English ------------------------------------------------------------------------------------------------------

const EN_ONES: [&str; 20] = ["zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve", "thirteen", "fourteen", "fifteen", "sixteen", "seventeen", "eighteen", "nineteen"];
const EN_TENS: [&str; 10] = ["", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety"];

fn en_below_thousand(n: u64, out: &mut Vec<String>) {
    if n >= 100 {
        out.push(format!("{} hundred", EN_ONES[(n / 100) as usize]));
    }
    let rest = n % 100;
    if rest >= 20 {
        let ten = EN_TENS[(rest / 10) as usize];
        out.push(if rest.is_multiple_of(10) { ten.into() } else { format!("{ten}-{}", EN_ONES[(rest % 10) as usize]) });
    } else if rest > 0 {
        out.push(EN_ONES[rest as usize].into());
    }
}

fn en_cardinal(n: u64) -> String {
    if n == 0 {
        return EN_ONES[0].into();
    }
    let mut out = Vec::new();
    let mut rest = n;
    for (size, name) in [(1_000_000_000, "billion"), (1_000_000, "million"), (1_000, "thousand")] {
        if rest >= size {
            en_below_thousand(rest / size, &mut out);
            out.push(name.into());
            rest %= size;
        }
    }
    en_below_thousand(rest, &mut out);
    out.join(" ")
}

fn en_ordinal(n: u64) -> String {
    let words = en_cardinal(n);
    let cut = words.rfind([' ', '-']).map_or(0, |i| i + 1);
    let (front, last) = words.split_at(cut);
    let last = match last {
        "one" => "first".into(),
        "two" => "second".into(),
        "three" => "third".into(),
        "five" => "fifth".into(),
        "eight" => "eighth".into(),
        "nine" => "ninth".into(),
        "twelve" => "twelfth".into(),
        w => match w.strip_suffix('y') {
            Some(stem) => format!("{stem}ieth"),
            None => format!("{w}th"),
        },
    };
    format!("{front}{last}")
}

fn en_year(y: u64) -> String {
    match (y / 100, y % 100) {
        _ if (2000..2010).contains(&y) || y < 1100 => en_cardinal(y),
        (century, 0) => format!("{} hundred", en_cardinal(century)),
        (century, rest @ 1..=9) => format!("{} oh {}", en_cardinal(century), en_cardinal(rest)),
        (century, rest) => format!("{} {}", en_cardinal(century), en_cardinal(rest)),
    }
}

// ---- Kazakh -------------------------------------------------------------------------------------------------------

const KK_ONES: [&str; 10] = ["нөл", "бір", "екі", "үш", "төрт", "бес", "алты", "жеті", "сегіз", "тоғыз"];
const KK_TENS: [&str; 10] = ["", "он", "жиырма", "отыз", "қырық", "елу", "алпыс", "жетпіс", "сексен", "тоқсан"];

fn kk_below_thousand(n: u64, out: &mut Vec<&'static str>) {
    if n >= 100 {
        if n / 100 > 1 {
            out.push(KK_ONES[(n / 100) as usize]);
        }
        out.push("жүз");
    }
    if n % 100 >= 10 {
        out.push(KK_TENS[(n % 100 / 10) as usize]);
    }
    if !n.is_multiple_of(10) {
        out.push(KK_ONES[(n % 10) as usize]);
    }
}

fn kk_cardinal(n: u64) -> String {
    if n == 0 {
        return KK_ONES[0].into();
    }
    let mut out = Vec::new();
    let mut rest = n;
    for (size, name) in [(1_000_000_000, "миллиард"), (1_000_000, "миллион"), (1_000, "мың")] {
        let many = rest / size;
        if many > 0 {
            // "мың" alone is a thousand; "бір миллион" keeps its one.
            if many > 1 || size > 1_000 {
                kk_below_thousand(many, &mut out);
            }
            out.push(name);
            rest %= size;
        }
    }
    kk_below_thousand(rest, &mut out);
    out.join(" ")
}

fn kk_ordinal(n: u64) -> String {
    let words = kk_cardinal(n);
    let cut = words.rfind(' ').map_or(0, |i| i + 1);
    let (front, last) = words.split_at(cut);
    let ordinal = match last {
        "бір" => "бірінші",
        "екі" => "екінші",
        "үш" => "үшінші",
        "төрт" => "төртінші",
        "бес" => "бесінші",
        "алты" => "алтыншы",
        "жеті" => "жетінші",
        "сегіз" => "сегізінші",
        "тоғыз" => "тоғызыншы",
        "он" => "оныншы",
        "жиырма" => "жиырмасыншы",
        "отыз" => "отызыншы",
        "қырық" => "қырқыншы",
        "елу" => "елуінші",
        "алпыс" => "алпысыншы",
        "жетпіс" => "жетпісінші",
        "сексен" => "сексенінші",
        "тоқсан" => "тоқсаныншы",
        "жүз" => "жүзінші",
        "мың" => "мыңыншы",
        "миллион" => "миллионыншы",
        "миллиард" => "миллиардыншы",
        _ => "нөлінші",
    };
    format!("{front}{ordinal}")
}

// ---- The three together -------------------------------------------------------------------------------------------

/// `word` stands at the start of `text` and ends there.
fn begins(text: &[char], word: &str) -> Option<usize> {
    let mut n = 0;
    for c in word.chars() {
        if text.get(n) != Some(&c) {
            return None;
        }
        n += 1;
    }
    let open = word.ends_with(char::is_alphabetic) && text.get(n).is_some_and(|c| c.is_alphanumeric());
    (!open).then_some(n)
}

impl Lang {
    pub fn of(lang: &str) -> Option<Lang> {
        match lang {
            "ru" => Some(Lang::Ru),
            "en" => Some(Lang::En),
            "kk" => Some(Lang::Kk),
            _ => None,
        }
    }

    pub fn sign(self, plus: bool) -> &'static str {
        match (self, plus) {
            (Lang::En, true) => "plus",
            (Lang::En, false) => "minus",
            (_, true) => "плюс",
            (_, false) => "минус",
        }
    }

    pub fn dot(self) -> &'static str {
        match self {
            Lang::Ru => "точка",
            Lang::En => "point",
            Lang::Kk => "нүкте",
        }
    }

    pub fn cardinal(self, n: u64, gender: Gender) -> String {
        match self {
            Lang::Ru => ru_cardinal(n, gender),
            Lang::En => en_cardinal(n),
            Lang::Kk => kk_cardinal(n),
        }
    }

    /// Whether the word in front of a number asks for its genitive (Russian only; "с" asks for two cases and is left).
    pub fn asks_genitive(self, before: &str) -> bool {
        self == Lang::Ru && matches!(before, "из" | "от" | "до" | "без" | "около" | "более" | "менее")
    }

    /// A number in the genitive.
    pub fn genitive(self, n: u64, gender: Gender) -> String {
        match self {
            Lang::Ru => ru_genitive(n, gender),
            _ => self.cardinal(n, gender),
        }
    }

    /// A number with its unit in the genitive: "двадцати одного градуса", "пяти градусов".
    pub fn measure_genitive(self, n: u64, unit: &Unit) -> String {
        match self {
            Lang::Ru => format!("{} {}{}", ru_genitive(n, unit.gender), unit.ru[if ru_form(n) == 0 { 1 } else { 2 }], unit.ru_after),
            _ => self.measure(n, "", unit),
        }
    }

    pub fn each_digit(self, digits: &str) -> String {
        digits.chars().filter_map(|c| c.to_digit(10)).map(|d| self.cardinal(u64::from(d), Gender::Masculine)).collect::<Vec<_>>().join(" ")
    }

    pub fn digits_or_number(self, digits: &str, gender: Gender) -> String {
        match digits.parse::<u64>() {
            Ok(n) if digits.len() <= 12 && !(digits.len() > 1 && digits.starts_with('0')) => self.cardinal(n, gender),
            _ => self.each_digit(digits),
        }
    }

    /// The gender of the noun that follows a number: only Russian asks, and only for the nouns an answer is likely to use.
    pub fn gender_before(self, noun: &str) -> Gender {
        const FEMININE: &[&str] = &["минут", "секунд", "тысяч", "недел", "штук", "строк", "кнопк", "страниц", "задач", "встреч", "копе", "ссылк", "папк", "программ", "песн", "вкладк", "заметк", "миль", "миля", "мили", "ошибк"];
        const NEUTER: &[&str] = &["сообщени", "напоминани", "уведомлени", "письм", "окн", "событи", "слов", "приложени"];
        let noun = noun.to_lowercase();
        if self == Lang::Ru && FEMININE.iter().any(|s| noun.starts_with(s)) {
            Gender::Feminine
        } else if self == Lang::Ru && NEUTER.iter().any(|s| noun.starts_with(s)) {
            Gender::Neuter
        } else {
            Gender::Masculine
        }
    }

    /// "3,5": the whole part and the digits after the separator.
    pub fn decimal(self, whole: u64, fraction: &str) -> String {
        let part: u64 = fraction.parse().unwrap_or(0);
        let short = fraction.len() <= 3;
        match self {
            Lang::En => format!("{} point {}", en_cardinal(whole), self.each_digit(fraction)),
            Lang::Ru if short => {
                let one = |n: u64| ru_form(n) == 0;
                let wholes = if one(whole) { "целая" } else { "целых" };
                let names = [["десятая", "десятых"], ["сотая", "сотых"], ["тысячная", "тысячных"]][fraction.len() - 1];
                format!("{} {wholes} {} {}", ru_cardinal(whole, Gender::Feminine), ru_cardinal(part, Gender::Feminine), names[usize::from(!one(part))])
            }
            Lang::Ru => format!("{} запятая {}", ru_cardinal(whole, Gender::Masculine), self.each_digit(fraction)),
            Lang::Kk if short => format!("{} бүтін {} {}", kk_cardinal(whole), ["оннан", "жүзден", "мыңнан"][fraction.len() - 1], kk_cardinal(part)),
            Lang::Kk => format!("{} үтір {}", kk_cardinal(whole), self.each_digit(fraction)),
        }
    }

    /// The unit written at the start of `text`, and how many characters it takes.
    pub fn unit(self, text: &[char]) -> Option<(&'static Unit, usize)> {
        UNITS.iter().flat_map(|u| u.spelled.iter().filter_map(move |s| begins(text, s).map(|n| (u, n)))).max_by_key(|(_, n)| *n)
    }

    pub fn measure(self, whole: u64, fraction: &str, unit: &Unit) -> String {
        match self {
            Lang::Ru if fraction.is_empty() => format!("{} {}{}", ru_cardinal(whole, unit.gender), unit.ru[ru_form(whole)], unit.ru_after),
            // After a fraction the noun takes the form it has after "two".
            Lang::Ru => format!("{} {}{}", self.decimal(whole, fraction), unit.ru[1], unit.ru_after),
            Lang::En if fraction.is_empty() => format!("{} {}", en_cardinal(whole), unit.en[usize::from(whole != 1)]),
            Lang::En => format!("{} {}", self.decimal(whole, fraction), unit.en[1]),
            Lang::Kk => {
                let number = if fraction.is_empty() { kk_cardinal(whole) } else { self.decimal(whole, fraction) };
                format!("{} {number} {}", unit.kk[0], unit.kk[1]).trim_start().to_string()
            }
        }
    }

    pub fn time(self, h: u64, m: u64) -> String {
        match self {
            Lang::Ru if m == 0 => format!("{} {}", ru_cardinal(h, Gender::Masculine), ["час", "часа", "часов"][ru_form(h)]),
            Lang::En if m == 0 && h == 0 => "midnight".into(),
            Lang::En if m == 0 && h <= 12 => format!("{} o'clock", en_cardinal(h)),
            Lang::En if m == 0 => format!("{} hundred", en_cardinal(h)),
            _ => {
                let zero = self.cardinal(0, Gender::Masculine);
                let minutes = match m {
                    0 => format!("{zero} {zero}"),
                    1..=9 if self == Lang::En => format!("oh {}", en_cardinal(m)),
                    1..=9 => format!("{zero} {}", self.cardinal(m, Gender::Masculine)),
                    _ => self.cardinal(m, Gender::Masculine),
                };
                format!("{} {minutes}", self.cardinal(h, Gender::Masculine))
            }
        }
    }

    /// A full date; `before` is the word in front of it.
    pub fn date(self, day: u64, month: usize, year: u64, before: &str) -> String {
        match self {
            Lang::Ru => format!("{} {} {} года", ru_ordinal(day, ru_day_form(before)), RU_MONTHS[month - 1], ru_ordinal(year, Form::Genitive)),
            Lang::En => format!("{} {}, {}", EN_MONTHS[month - 1], en_ordinal(day), en_year(year)),
            Lang::Kk => format!("{} жылғы {} {}", kk_ordinal(year), kk_ordinal(day), KK_MONTHS[month - 1]),
        }
    }

    /// The day in front of or after the name of a month.
    pub fn day_of_month(self, day: u64, before: &str, after: &str) -> Option<String> {
        let word: String = after.chars().take_while(|c| c.is_alphabetic()).collect();
        match self {
            Lang::Ru => RU_MONTHS.contains(&word.as_str()).then(|| ru_ordinal(day, ru_day_form(before))),
            Lang::En => (EN_MONTHS.contains(&word.as_str()) || EN_MONTHS.iter().any(|m| m.eq_ignore_ascii_case(before) && before != "may")).then(|| en_ordinal(day)),
            Lang::Kk => KK_MONTHS.iter().any(|m| word.starts_with(m)).then(|| kk_ordinal(day)),
        }
    }

    /// "2026 года": the year with the word that names it; returns the words and the characters they take from `after`.
    /// `before` is the word in front of the year.
    pub fn year_before(self, year: u64, before: &str, after: &[char]) -> Option<(String, usize)> {
        let gap = after.iter().take_while(|c| c.is_whitespace()).count();
        if gap == 0 {
            return None;
        }
        let rest = &after[gap..];
        let found = |word: &str| begins(rest, word).map(|n| gap + n);
        // A short form with a dot at the end of a sentence keeps the pause of that dot.
        let closes = |taken: usize| after[taken..].iter().find(|c| !c.is_whitespace()).is_none_or(|c| c.is_uppercase());
        match self {
            Lang::Ru => {
                for (word, form) in [("года", Form::Genitive), ("году", Form::Prepositional), ("год", Form::Nominative)] {
                    if let Some(taken) = found(word) {
                        return Some((format!("{} {word}", ru_ordinal(year, form)), taken));
                    }
                }
                // "в 2026 г." is said "году", "за 2026 г." is said "год".
                let (word, form) = match before {
                    "в" | "во" => ("году", Form::Prepositional),
                    "за" | "на" => ("год", Form::Nominative),
                    _ => ("года", Form::Genitive),
                };
                found("г.").map(|taken| (format!("{} {word}{}", ru_ordinal(year, form), if closes(taken) { "." } else { "" }), taken))
            }
            Lang::Kk => {
                for word in ["жылдың", "жылғы", "жылы", "жылдан", "жылға", "жылда", "жыл"] {
                    if let Some(taken) = found(word) {
                        return Some((format!("{} {word}", kk_ordinal(year)), taken));
                    }
                }
                found("ж.").map(|taken| (format!("{} жыл{}", kk_ordinal(year), if closes(taken) { "." } else { "" }), taken))
            }
            Lang::En => None,
        }
    }
}

/// "Сегодня 28 ноября" names the day; everywhere else the date says when ("двадцать восьмого").
fn ru_day_form(before: &str) -> Form {
    if matches!(before, "сегодня" | "завтра" | "вчера" | "это") {
        Form::Neuter
    } else {
        Form::Genitive
    }
}
