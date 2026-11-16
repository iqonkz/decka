# TASKS.md — открытые задачи Decka

Исполнитель: Claude Sonnet 5.5. Каждая задача — 1–2 часа. Задачи выполняются строго по порядку. Архитектурных решений исполнитель не принимает: всё решено в `/docs`. Перед каждой задачей прочитай `docs/QUICKREF.md` и только те разделы, на которые задача ссылается.

## 0. Правила исполнителя

1. **Оболочка — Windows PowerShell 5.1.** `&&` и `||` не работают: команды разделяй `;`. Вместо `curl` пиши `curl.exe`. Все команды запускаются из корня репозитория, если не сказано иное. Блок «Проверка» запускается **одним вызовом** (переменные между вызовами не сохраняются).
2. **Версии.** Ставь ровно те версии, что указаны в задаче (`npm install pkg@x.y.z`, `cargo add crate@x.y.z`). Где версии нет и стоит пометка «последняя» — `npm install pkg` / `cargo add crate` без версии. Версию и набор библиотек не менять.
3. **Если API библиотеки отличается от имён в плане** — открой документацию ровно установленной версии (`https://docs.rs/<crate>/<version>`, README пакета) и используй эквивалент. Это не архитектурное решение. Поведение, описанное в `/docs`, менять нельзя.
4. **Если шаг невыполним или документы противоречат друг другу** — не выбирай сам. Запиши в `docs/BLOCKERS.md` (номер задачи, что именно, что пробовал) и остановись.
5. **Безопасность (MUST):** в коде клиента и в протоколе не должно появиться ни одного пути, по которому телефон передаёт команду, путь, аргументы, клавиши или URL для исполнения. Телефон шлёт только `actionId`.
6. Приоритет документов при расхождении: `PROTOCOL.md` > `DATA_MODEL.md` > `ARCHITECTURE.md` > `TASKS.md`. О расхождении — запись в `BLOCKERS.md`.
7. После каждой задачи: все команды «Проверки» прошли → `git add <свои пути>; git commit -m "T<номер>: <название>"` (никогда `git add -A` / `git add .` — в дереве могут быть незакоммиченные правки архитектора в `docs/`). Не коммить `.tmp/`, `target/`, `node_modules/`, `dist/`.
7a. **Экономия токенов.** Вывод сборки и тестов не показывай целиком: `| Select-Object -Last 20` или только строки с ошибками. Снимки экрана открывай только когда их нужно оценить. Архив `docs/archive/` целиком не читай — только поиск по номеру задачи. Новая сессия (`/clear`) после каждой одной-двух задач: состояние восстанавливается из `CLAUDE.md`, `QUICKREF.md`, `git log`.
8. Тексты интерфейса — на английском (с T14.7 — через словари `en`/`ru`, DESIGN.md §6). Комментарии в коде — на английском, только там, где логика неочевидна.
9. Тесты, не требующие железа, обязательны там, где они перечислены. «Ручная проверка» выполняется исполнителем, если у него есть доступ к экрану/устройству; иначе пункт переносится в `docs/MANUAL_CHECKS.md` (номер задачи + текст пункта) для владельца, задача считается выполненной при прохождении автоматических проверок.
10. Каталог `/docs` не редактировать, кроме файлов, явно названных в задачах (`BLOCKERS.md`, `MANUAL_CHECKS.md`, `RECIPES.md`, `TESTING.md`).

### Макросы для блоков «Проверка»

**H-BUILD**
```
cargo build -p decka-core --bin decka-headless
```
**H-START** (после H-BUILD; запускает headless-агент на порту 19247 с чистым конфигом, кодом сопряжения `TESTCDE2`, без реального исполнения действий)
```
New-Item -ItemType Directory -Force .tmp | Out-Null
Remove-Item -Recurse -Force .tmp\cfg -ErrorAction SilentlyContinue
$h = Start-Process -FilePath target\debug\decka-headless.exe -ArgumentList '--port','19247','--config-dir','.tmp\cfg','--pair-code','TESTCDE2','--dry-run','--client-dist','client\dist' -PassThru -NoNewWindow -RedirectStandardOutput .tmp\headless.log
Start-Sleep -Seconds 2
```
**H-STOP**
```
Stop-Process -Id $h.Id -Force
```
**H-TOKEN** (сопряжение и получение токена устройства; после H-START)
```
$tok = (node tools/ws-smoke.mjs pair 127.0.0.1:19247 TESTCDE2 | ConvertFrom-Json).token
```
**H-API** (чтение apiToken; после H-START)
```
$api = (Get-Content .tmp\cfg\config.json -Raw | ConvertFrom-Json).apiToken
```

---

> **Выполненные задачи M0–M15b перенесены в [archive/TASKS-M0-M13.md](archive/TASKS-M0-M13.md).** Ссылки вида «T11.10», «T15.9» ведут туда. Нужный раздел открывай через поиск, файл целиком не читай.

## M16 — ИИ-помощники: лимиты и разрешения (2026-11-17, D-48)

Вся спецификация — DATA_MODEL.md §6.8. Типы — PROTOCOL.md §3 (`AiInfo`, `PermAnswerMsg`), §5, §9 (`CcStatus`). Безопасность — ARCHITECTURE.md S20.

**Быстрый режим (решение владельца).**
- Новых зависимостей нет.
- Тесты — только перечисленные в задаче.
- Без снимков, телефона, `tauri dev`-обходов и сборки релиза между задачами.
- Ручные проверки → `MANUAL_CHECKS.md` (`PENDING (owner)`).
- Документы, кроме указанных разделов, не читать.

**Безопасность.**
- Настоящий `%USERPROFILE%\.claude\settings.json` NEVER не трогать в тестах: путь к домашней папке передаётся параметром, тесты используют временную папку.
- `~/.codex` тестами не читать: нужна фикстура.

### T16.1 — Агент: секция `ai`, эндпоинты `/api/cc/*`, Codex, установка в Claude Code
1. Конфигурация `ai` и `hookToken`. `get_config` не отдаёт `hookToken`, `save_config` сохраняет прежний.
2. `POST /api/cc/statusline` и `POST /api/cc/permission`. Проверки: только loopback, свой лимит, тело до 1 МБ, ожидание с guard на Drop.
3. Сообщение WS `perm_answer`, секция `ai` (рассылка ≤ 1 раз в 2 с, кроме pending), переменные `ai.*` и их резерв.
4. Сборщик Codex. `cc_install`, `cc_uninstall`, `cc_status` (IPC).
5. Тесты (Rust, без сети и настоящих файлов):
   - разбор statusline;
   - `summary` для каждого вида инструмента и обрезка;
   - permission: allow, deny, pc, таймаут (`tokio::time::pause`), нет телефона → пустое тело, обрыв соединения → запрос удалён, повторный ответ игнорируется;
   - не-loopback → 404, apiToken вместо hookToken → 401;
   - Codex: фикстура со строкой из §6.8 (`resets_at`) и старая (`resets_in_seconds`), метки окон;
   - install/uninstall на временной папке: чужие ключи и хуки целы, чужой statusLine не тронут (`statusLineConflict`), повтор идемпотентен, `.decka-bak` создаётся один раз.
```
npm run typecheck; npm test; cargo test -p decka-core
```

### T16.2 — Клиент: виджет `ai`, лист разрешений, набор `ai`
1. `shared`:
   - типы `AiInfo`, `AiItem`, `PermAnswerMsg`;
   - схемы и валидация размеров `ai`, `fitWidgetSize` для `ai`;
   - `burnEta` с тестами: рост, нет роста, мало точек, eta после сброса → нет прогноза;
   - набор `ai` в `sets.json`, виджет в библиотеке редактора;
   - строки en/ru по DESIGN.md §6.
2. Виджет `ai` по DATA_MODEL §6.8 во всех темах. Стили — существующие токены тем, новых цветов не вводить.
3. Лист разрешений поверх deck: защита 400 мс, `perm_answer`, закрытие без ожидания `patch`. Тест store: ответ убирает лист; исчезновение из `pending` закрывает лист.
4. `NOTICE.md`: строка `Idea of AI usage limits and phone approvals inspired by PhoneBox (github.com/ersinkoc/phonebox, MIT); no code copied.`
```
npm run typecheck; npm test; cargo test -p decka-core
```

### T16.3 — Окно настроек и сборка
1. `More → Integrations → AI coding` по §6.8: кнопки `Install`/`Remove`, статусы, переключатели, таймаут. Строки — через `t()`.
2. CHANGELOG `Unreleased`: две-три строки для пользователя.
3. Один прогон `npm run release:agent; npm run release:apk` — проверка, что всё собирается. Версию не поднимать, тег не ставить, релиз не публиковать.
4. В `MANUAL_CHECKS.md`, раздел `M16`, — `PENDING (owner)`:
   - `Install` → перезапуск Claude Code → строка `Decka · 5h …` в терминале и лимиты на телефоне;
   - запрос `Bash` → лист на телефоне → `Allow`/`Deny`/`Answer on PC`/таймаут;
   - телефон отключён → вопрос сразу в терминале;
   - Codex — лимиты видны;
   - `Remove` возвращает `settings.json`.
```
npm run typecheck; npm test; cargo test -p decka-core; npm run release:agent; npm run release:apk
```

## Что ещё открыто
- **Ручные проверки владельца:** `docs/MANUAL_CHECKS.md`, пункты `PENDING (owner)`. Приёмка на телефоне T15.9 описана в архиве.
- **Вопрос Q-9** (Twitch, YouTube, Discord) — `docs/OPEN_QUESTIONS.md`.
