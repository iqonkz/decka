# Лендинг Decka на madiseit.com (решение архитектора 2026-11-22)

## Решения
- **Адрес (пересмотр владельца 2026-11-22: `/ru/decka` выглядит громоздко):** `decka.madiseit.com` — поддомен, который обслуживает **тот же Worker и тот же код** сайта (репозиторий `C:\DEV\madiseit.com`; в `wrangler.jsonc` — второй custom domain). Worker по заголовку `Host` отдаёт страницу Decka; общие стиль, компоненты и деплой (`pnpm deploy`) остаются. Шаблон на будущее: каждый продукт — `<продукт>.madiseit.com`.
  - Язык: `decka.madiseit.com/` — сам по правилам сайта (cookie → страна → `Accept-Language` → en), без редиректа, URL остаётся чистым; явные адреса `decka.madiseit.com/ru` и `/en` существуют для поиска (`hreflang`) и переключателя языка. Делиться — ссылкой без языка.
  - `madiseit.com/decka` и `madiseit.com/:lang/decka` → 301 на `decka.madiseit.com`. Если у Decka появится свой домен — 301 на него.
- **Технология:** React-страница сайта (не отдельная статика). Для поиска и превью в соцсетях Worker подставляет мета-теги в HTML (`HTMLRewriter`) для `/:lang/decka`.
- **Языки:** en и ru — как весь сайт. Другие языки — когда их получит сайт.
- **Стиль — madiseit.com:** токены `src/styles/tailwind.css` (primary — красный бренда `#c1272d`, neutral, шрифт Inter), компоненты сайта (`SectionHeading`, кнопки, карточки как `ProjectCard`), светлая и тёмная тема (`.dark`). Фиолетовый Decka `#7c5cff` — только в логотипе Decka и на снимках экрана, не в оформлении страницы.
- **Без новых зависимостей.** Иконки — `@hugeicons/react`, уже в сайте.
- **Факты** (требования, функции, языки) — только из этого репозитория: README, CHANGELOG, DATA_MODEL §12.2 (железо для голоса), `client/android` (minSdk). Не выдумывать.

## Структура страницы
1. **Hero.** Слева: H1 `Turn your old phone into a control deck for your PC` / `Старый телефон — пульт для вашего ПК`; подзаголовок `Buttons, live widgets and voice control with local AI. Free and open source. Works over your Wi-Fi — nothing goes to the cloud.` / `Кнопки, живые виджеты и голосовое управление с локальным ИИ. Бесплатно и с открытым кодом. Работает по вашему Wi-Fi — ничего не уходит в облако.`; кнопки `Download for Windows` (primary, под ней мелко версия и размер) и `Android app (APK)` (вторичная), ссылка `GitHub`. Справа: снимок телефона в альбомной ориентации в рамке-мокапе (CSS, без картинки рамки).
2. **Три шага:** установить на ПК → поставить приложение на телефон → отсканировать QR-код. Номера кружками primary.
3. **Возможности** — сетка 3×2 карточек (иконка, заголовок, 1–2 строки):
   - кнопки для всего: программы, сочетания клавиш, сайты, медиа, OBS;
   - живые виджеты: CPU и память, музыка, батарея, лимиты Claude и Codex;
   - голос с локальным ИИ (Qwen и Whisper работают на вашем ПК, 11 языков);
   - полоса программы: кнопки меняются под активное окно (Word, браузер);
   - уведомления Windows на телефоне;
   - безопасность: шифрование, только локальная сеть, телефон отправляет лишь номер кнопки.
4. **Голос** — отдельный блок: снимок плашки и 4–5 примеров фраз чипами (`Turn the volume down`, `Open Chrome`, `Set a timer for 10 minutes`, `Move this window to the other monitor`; ru — по-русски). Строка: `Runs on your PC. About 3 GB download, works without a graphics card.` — цифры сверить с DATA_MODEL §12.2/§12.9.
5. **Темы** — 4 снимка (AMOLED, Studio, Material, Glass), горизонтальная прокрутка на телефоне, сетка 2×2 на десктопе.
6. **Открытый код** — MIT, ссылка на GitHub, `No account, no ads, no tracking.` / `Без аккаунта, рекламы и слежки.`
7. **Требования:** Windows 10/11 x64; Android-телефон (версия из minSdk); одна сеть Wi-Fi; голос — по §12.2.
8. **FAQ** (раскрывающиеся, `@headlessui` Disclosure как на сайте): бесплатно ли; нужен ли интернет; какие телефоны (iPhone — нет); безопасно ли; уходит ли голос в облако (нет); как обновлять (в один клик из приложения).
9. Подвал — общий подвал сайта.

## Загрузка последней версии
- Worker: `GET /api/decka/latest` → `https://api.github.com/repos/iqonkz/decka/releases/latest`, кэш 1 ч (Cache API), ответ `{ version, date, exe: {url, size}, apk: {url, size} }` (ассеты `Decka-Setup-<v>.exe`, `Decka-<v>.apk`). При ошибке — кнопки ведут на `https://github.com/iqonkz/decka/releases/latest`.

## SEO и превью
- Для `decka.madiseit.com/`, `/en`, `/ru` Worker ставит (canonical: `/en` и `/ru`; `x-default` — корень) `<title>` (`Decka — your old phone as a PC control deck` / `Decka — старый телефон как пульт для ПК`), `description`, `canonical`, `hreflang` en/ru/x-default, `og:title`, `og:description`, `og:image` = `/decka/og.png` (1200×630: логотип, заголовок, снимок телефона; в стиле сайта).
- Если у сайта есть sitemap — добавить обе страницы.
- Decka — в список проектов сайта, карточка ведёт на `/:lang/decka`.

## Снимки
Берутся из `docs/img/` репозитория Decka (задача T22.1), копируются в `public/decka/` сайта в `webp`; у `<img>` — `width`/`height`, `loading="lazy"` везде, кроме hero.

## Проверка
`pnpm typecheck; pnpm build`; `pnpm dev` — посмотреть en и ru, светлую и тёмную тему, ширину 375 px и 1440 px. Деплой (`pnpm deploy`) делает владелец.
