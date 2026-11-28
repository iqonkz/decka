//! T33.2: "text -> speech", at least thirty lines of Russian and of English.

use super::*;

const RU: &[(&str, &str)] = &[
    ("Открываю калькулятор", "Открываю калькулятор."),
    ("## Итог\n\n**Готово**: всё `проверено`.", "Итог. Готово: всё проверено."),
    ("- первое\n- второе", "первое. второе."),
    ("Смотри https://example.com/a?b=1, там всё.", "Смотри ссылка, там всё."),
    ("[Инструкция](https://example.com) готова!", "Инструкция готова!"),
    ("Запусти:\n```\nnpm test\n```\nи подожди", "Запусти: и подожди."),
    ("Отлично 🎉👍 → дальше", "Отлично дальше."),
    ("Осталось 5 минут", "Осталось пять минут."),
    ("Осталась 1 минута", "Осталась одна минута."),
    ("Пришло 1 сообщение и 2 задачи", "Пришло одно сообщение и две задачи."),
    ("Сегодня +12°, ночью -3 °C", "Сегодня плюс двенадцать градусов, ночью минус три градуса."),
    ("Заряд 1%", "Заряд один процент."),
    ("Память занята на 43 %", "Память занята на сорок три процента."),
    ("Загрузка 3,5%", "Загрузка три целых пять десятых процента."),
    ("Свободно 12 ГБ, всего 32 ГБ", "Свободно двенадцать гигабайт, всего тридцать два гигабайта."),
    ("Скачано 1 МБ", "Скачано один мегабайт."),
    ("Диск 2 TB", "Диск два терабайта."),
    ("Ветер 5 м/с", "Ветер пять метров в секунду."),
    ("Скорость 101 км/ч", "Скорость сто один километр в час."),
    ("Сеть 100 Мбит/с", "Сеть сто мегабит в секунду."),
    ("Давление 745 мм рт. ст.", "Давление семьсот сорок пять миллиметров ртутного столба."),
    ("Встреча в 14:30", "Встреча в четырнадцать тридцать."),
    ("Будильник на 7:05", "Будильник на семь ноль пять."),
    ("Конец в 21:00", "Конец в двадцать один час."),
    ("Выпуск 28.11.2026", "Выпуск двадцать восьмого ноября две тысячи двадцать шестого года."),
    ("Сегодня 3 декабря", "Сегодня третье декабря."),
    ("Отпуск с 1 мая", "Отпуск с первого мая."),
    ("Это было в 1999 году", "Это было в тысяча девятьсот девяносто девятом году."),
    ("План на 2000 год", "План на двухтысячный год."),
    ("Отчёт за 2026 г.", "Отчёт за две тысячи двадцать шестой год."),
    ("Открыт в 2024 г. Работает", "Открыт в две тысячи двадцать четвёртом году. Работает."),
    ("Всего 12 345 строк", "Всего двенадцать тысяч триста сорок пять строк."),
    ("Цена 1500 ₸", "Цена одна тысяча пятьсот тенге."),
    ("Версия 0.70.1", "Версия ноль точка семьдесят точка один."),
    ("Код 007", "Код ноль ноль семь."),
    ("Файл mp3 и модель 3D", "Файл mp3 и модель 3D."),
    ("Это 2 * 2 = 4", "Это два * два = четыре."),
    ("Свободно 43 ГБ из 512 ГБ", "Свободно сорок три гигабайта из пятисот двенадцати гигабайт."),
    ("От 5 до 21 градуса", "От пяти до двадцати одного градуса."),
    ("Около 40% и не более 1 минуты", "Около сорока процентов и не более одной минуты."),
    ("Без 2 тысяч", "Без двух тысяч."),
    ("Меньше не бывает: до 1000 ₸", "Меньше не бывает: до одной тысячи тенге."),
    ("| Диск | Свободно |\n|---|---|\n| C | 50 ГБ |", "Диск, Свободно. C, пятьдесят гигабайт."),
];

const EN: &[(&str, &str)] = &[
    ("Opening the calculator", "Opening the calculator."),
    ("# Summary\n> All *good* now", "Summary. All good now."),
    ("* one\n* two", "one. two."),
    ("See www.example.com/page.", "See link."),
    ("Read the [guide](https://example.com/g) first", "Read the guide first."),
    ("Run `cargo test` <br> now", "Run cargo test now."),
    ("```js\nlet a = 1;\n```\nDone", "Done."),
    ("Great ✅ job 🚀", "Great job."),
    ("5 files were copied", "five files were copied."),
    ("It is 21 degrees", "It is twenty-one degrees."),
    ("Today +12°, tonight -1 °C", "Today plus twelve degrees, tonight minus one degree."),
    ("Water boils at 212°F", "Water boils at two hundred twelve degrees Fahrenheit."),
    ("Battery 1%", "Battery one percent."),
    ("CPU load is 3.5%", "CPU load is three point five percent."),
    ("12 GB free of 32 GB", "twelve gigabytes free of thirty-two gigabytes."),
    ("Only 1 MB left", "Only one megabyte left."),
    ("A drive of 2 TB", "A drive of two terabytes."),
    ("Wind 5 m/s", "Wind five meters per second."),
    ("Limit 60 mph or 100 km/h", "Limit sixty miles per hour or one hundred kilometers per hour."),
    ("Line speed 100 Mbps", "Line speed one hundred megabits per second."),
    ("Ping 35 ms", "Ping thirty-five milliseconds."),
    ("The meeting is at 14:30", "The meeting is at fourteen thirty."),
    ("Wake me at 7:05", "Wake me at seven oh five."),
    ("It ends at 9:00", "It ends at nine o'clock."),
    ("Closed from 18:00", "Closed from eighteen hundred."),
    ("Released on 2026-11-28", "Released on November twenty-eighth, twenty twenty-six."),
    ("Born 05.03.1905", "Born March fifth, nineteen oh five."),
    ("See you on November 21", "See you on November twenty-first."),
    ("The 3 December report", "The third December report."),
    ("It has 1,234,567 rows", "It has one million two hundred thirty-four thousand five hundred sixty-seven rows."),
    ("Pi is 3.14", "Pi is three point one four."),
    ("Version 2.0.3 is out", "Version two point zero point three is out."),
    ("Call 87011234567890", "Call eight seven zero one one two three four five six seven eight nine zero."),
    ("Pick 2, 5 or 9", "Pick two, five or nine."),
    ("An A4 page for the 5G test", "An A4 page for the 5G test."),
];

const KK: &[(&str, &str)] = &[
    ("Бүгін +12°", "Бүгін плюс он екі градус."),
    ("Заряд 45%", "Заряд қырық бес пайыз."),
    ("Бос орын 100 ГБ", "Бос орын жүз гигабайт."),
    ("Жел 5 м/с", "Жел секундына бес метр."),
    ("Кездесу 14:30", "Кездесу он төрт отыз."),
    ("Күні 28.11.2026", "Күні екі мың жиырма алтыншы жылғы жиырма сегізінші қараша."),
    ("2026 жылы 1 қаңтарда", "екі мың жиырма алтыншы жылы бірінші қаңтарда."),
    ("Бағасы 1000 ₸, сілтеме: https://example.kz", "Бағасы мың теңге, сілтеме: сілтеме."),
];

fn table(lang: &str, rows: &[(&str, &str)]) {
    let wrong: Vec<String> = rows.iter().filter_map(|(text, want)| {
        let said = for_speech(text, lang);
        (said != *want).then(|| format!("{text:?}\n  said {said:?}\n  want {want:?}"))
    }).collect();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn russian_text_as_it_is_said() {
    assert!(RU.len() >= 30);
    table("ru", RU);
}

#[test]
fn english_text_as_it_is_said() {
    assert!(EN.len() >= 30);
    table("en", EN);
}

#[test]
fn kazakh_text_as_it_is_said() {
    table("kk", KK);
}

#[test]
fn other_languages_are_cleaned_and_keep_their_numbers() {
    assert_eq!(for_speech("**Es** 12 % https://example.com", "de"), "Es 12 % link.");
    assert_eq!(for_speech("温度 5°。\n看 www.example.com", "zh"), "温度 5°。 看 链接.");
    assert_eq!(for_speech("```\nx\n```\n🎉", "ru"), "");
    assert_eq!(for_speech("a < b and b > c", "en"), "a < b and b > c.");
}

#[test]
fn the_language_of_the_numbers_follows_the_letters_of_the_text() {
    assert_eq!(language_of("Свободно 12 GB", "en"), "ru");
    assert_eq!(language_of("Бос орын 12 ГБ қалды", "ru"), "kk");
    assert_eq!(language_of("12 GB free", "en"), "en");
    assert_eq!(language_of("12 GB free", "ru"), "en");
    assert_eq!(language_of("12 GB frei", "de"), "de");
    assert_eq!(language_of("12:30", "ru"), "ru");
}

#[test]
fn no_text_makes_it_fail() {
    for odd in ["", "-", "+", "1", "1.", ".5", "00:99", "31.13.2026", "[x](", "<a", "**", "| |", "9999999999999999999999", "5 %%%", "2026 г", "г.", "-5-5-5", "٥ %", "１２"] {
        for lang in ["ru", "en", "kk", "ar"] {
            for_speech(odd, lang);
        }
    }
}
