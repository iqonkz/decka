# TASKS.md — план реализации Decka (M0..M9, 49 задач)

Исполнитель: Claude Sonnet 5.5. Каждая задача — 1–2 часа. Задачи выполняются строго по порядку. Архитектурных решений исполнитель не принимает: всё решено в `/docs`. Перед каждой задачей прочитай `docs/QUICKREF.md` и только те разделы, на которые задача ссылается.

## 0. Правила исполнителя

1. **Оболочка — Windows PowerShell 5.1.** `&&` и `||` не работают: команды разделяй `;`. Вместо `curl` пиши `curl.exe`. Все команды запускаются из корня репозитория, если не сказано иное. Блок «Проверка» запускается **одним вызовом** (переменные между вызовами не сохраняются).
2. **Версии.** Ставь ровно те версии, что указаны в задаче (`npm install pkg@x.y.z`, `cargo add crate@x.y.z`). Где версии нет и стоит пометка «последняя» — `npm install pkg` / `cargo add crate` без версии. Версию и набор библиотек не менять.
3. **Если API библиотеки отличается от имён в плане** — открой документацию ровно установленной версии (`https://docs.rs/<crate>/<version>`, README пакета) и используй эквивалент. Это не архитектурное решение. Поведение, описанное в `/docs`, менять нельзя.
4. **Если шаг невыполним или документы противоречат друг другу** — не выбирай сам. Запиши в `docs/BLOCKERS.md` (номер задачи, что именно, что пробовал) и остановись.
5. **Безопасность (MUST):** в коде клиента и в протоколе не должно появиться ни одного пути, по которому телефон передаёт команду, путь, аргументы, клавиши или URL для исполнения. Телефон шлёт только `actionId`.
6. Приоритет документов при расхождении: `PROTOCOL.md` > `DATA_MODEL.md` > `ARCHITECTURE.md` > `TASKS.md`. О расхождении — запись в `BLOCKERS.md`.
7. После каждой задачи: все команды «Проверки» прошли → `git add -A; git commit -m "T<номер>: <название>"`. Не коммить `.tmp/`, `target/`, `node_modules/`, `dist/`.
8. Тексты интерфейса — на английском. Комментарии в коде — на английском, только там, где логика неочевидна.
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

## M0 — Каркас репозитория

### T0.1 — Корень репозитория и пакет shared
**Цель:** git-репозиторий с npm workspaces и пустым пакетом `@decka/shared`.
**Файлы:** `.gitignore`, `.editorconfig`, `LICENSE`, `README.md`, `package.json`, `tsconfig.base.json`, `shared/package.json`, `shared/tsconfig.json`, `shared/src/index.ts`, `shared/test/smoke.test.ts`.
**Шаги:**
1. Проверь окружение: `node -v` ≥ 22.12, `npm -v`, `git --version`, `rustc -V`, `cargo -V`. Чего-то нет → `BLOCKERS.md`, стоп.
2. `git init -b main`.
3. `.gitignore`: `node_modules/`, `dist/`, `target/`, `.tmp/`, `*.log`, `client/android/app/build/`, `client/android/build/`, `client/android/.gradle/`, `client/android/local.properties`, `agent/src-tauri/gen/`.
4. `.editorconfig`: utf-8, lf, 2 пробела (для `*.rs` — 4), финальный перевод строки.
5. `LICENSE`: стандартный текст MIT, строка `Copyright (c) 2026 Madi Seit`.
6. `README.md`: заголовок `# Decka` и одна строка описания из ARCHITECTURE.md §1 (полный README — T9.2).
7. Корневой `package.json`: `"name": "decka"`, `"private": true`, `"workspaces": ["shared"]`, `"engines": {"node": ">=22.12"}`, scripts: `"typecheck": "npm run typecheck --workspaces --if-present"`, `"test": "npm run test --workspaces --if-present"`.
8. `tsconfig.base.json`: `strict: true`, `target: "ES2022"`, `module: "ESNext"`, `moduleResolution: "Bundler"`, `jsx: "react-jsx"`, `resolveJsonModule: true`, `isolatedModules: true`, `noEmit: true`, `skipLibCheck: true`, `noUncheckedIndexedAccess: true`.
9. `shared/package.json`: `"name": "@decka/shared"`, `"version": "0.1.0"`, `"private": true`, `"type": "module"`, `"main": "src/index.ts"`, `"types": "src/index.ts"`, `"exports": {".": "./src/index.ts", "./schemas/*": "./schemas/*", "./fixtures/*": "./fixtures/*"}`, scripts `"typecheck": "tsc --noEmit -p ."`, `"test": "vitest run"`.
10. `npm install -w shared -D typescript@7.0.2 vitest@5.1.0 ajv@8.20.0`. Если `tsc` 7.x не запускается или падает на корректном коде — запасной вариант (единственный разрешённый): `typescript@6` во всех пакетах, запись об этом в описании коммита.
11. `shared/tsconfig.json`: `extends ../tsconfig.base.json`, `include: ["src", "test"]`.
12. `shared/src/index.ts`: `export const PROTOCOL_VERSION = 1;`. `shared/test/smoke.test.ts`: тест, что `PROTOCOL_VERSION === 1`.
**Приёмка:** `npm install` завершается с кодом 0; typecheck и тест проходят; `git status` чист после коммита.
**Проверка:**
```
npm install; npm run typecheck; npm test
```

### T0.2 — Cargo workspace и скелет decka-core
**Цель:** Rust-workspace с библиотекой `decka-core` и бинарём `decka-headless`.
**Файлы:** `Cargo.toml`, `rust-toolchain.toml`, `agent/core/Cargo.toml`, `agent/core/src/lib.rs`, `agent/core/src/bin/decka-headless.rs`.
**Шаги:**
1. Корневой `Cargo.toml`: `[workspace]`, `members = ["agent/core"]`, `resolver = "2"`; `[workspace.package]` `version = "0.1.0"`, `edition = "2021"`, `license = "MIT"`.
2. `rust-toolchain.toml`: `channel = "stable"`.
3. `agent/core/Cargo.toml`: `name = "decka-core"`, версия/edition/license из workspace; `[lib]` и `[[bin]] name = "decka-headless"`, `path = "src/bin/decka-headless.rs"`.
4. `lib.rs`: `pub const VERSION: &str = env!("CARGO_PKG_VERSION");` и `pub const PROTOCOL_VERSION: u32 = 1;`.
5. `decka-headless.rs`: разбор аргументов вручную через `std::env::args` (без clap). Пока поддерживается только `--version` → печатает `decka-headless 0.1.0` и выходит с кодом 0; без аргументов — печатает `not implemented` и выходит с кодом 1.
**Приёмка:** сборка без предупреждений; вывод `--version` в точности `decka-headless 0.1.0`.
**Проверка:**
```
cargo build -p decka-core; cargo run -q -p decka-core --bin decka-headless -- --version
```

### T0.3 — Скелет клиента (Vite + React) и каталог tools
**Цель:** пакет `@decka/client`, который собирается и показывает чёрный экран с текстом `Decka`.
**Файлы:** `client/package.json`, `client/tsconfig.json`, `client/vite.config.ts`, `client/index.html`, `client/src/main.tsx`, `client/src/App.tsx`, `client/src/styles.css`, `client/src/vite-env.d.ts`, `tools/.gitkeep`; правка корневого `package.json`.
**Шаги:**
1. В корневом `package.json`: `"workspaces": ["shared", "client"]`.
2. `client/package.json`: `"name": "@decka/client"`, `"version": "0.1.0"`, `"private": true`, `"type": "module"`, scripts: `"dev": "vite"`, `"build": "tsc --noEmit -p . && vite build"`, `"typecheck": "tsc --noEmit -p ."`, `"test": "vitest run --passWithNoTests"`; `"dependencies": {"@decka/shared": "*"}`.
3. `npm install -w client react@19.3.1 react-dom@19.3.1 lucide-react@1.73.0`; `npm install -w client -D vite@8.3.8 @vitejs/plugin-react@6.3.0 typescript@7.0.2 vitest@5.1.0 @types/react @types/react-dom` (типы — последние).
4. `vite.config.ts`: плагин react; `server: { port: 5173, strictPort: true, host: true }`; `build: { target: "chrome90", outDir: "dist", emptyOutDir: true }`; `base: "./"`.
5. `index.html`: `<meta name="viewport" content="width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no, viewport-fit=cover">`, `<meta name="theme-color" content="#000000">`, `<title>Decka</title>`, `<div id="root">`.
6. `styles.css`: `html, body, #root { margin:0; height:100%; background:#000; color:#e5e5e5; font-family: system-ui, sans-serif; overflow:hidden; user-select:none; -webkit-tap-highlight-color: transparent; touch-action: none; }`.
7. `App.tsx`: по центру текст `Decka`. `main.tsx`: `createRoot` + `StrictMode`.
8. `client/tsconfig.json`: extends base, `include: ["src", "test"]`, `types: ["vite/client"]`.
**Приёмка:** `client/dist/index.html` существует после сборки; typecheck проходит.
**Проверка:**
```
npm install; npm run typecheck; npm run build -w client; Test-Path client\dist\index.html
```
(последняя команда печатает `True`)

---

## M1 — Общий контракт (/shared)

### T1.1 — Типы TypeScript
**Цель:** все типы из документов в `/shared/src`.
**Файлы:** `shared/src/protocol.ts`, `layout.ts`, `actions.ts`, `config.ts`, `ipc.ts`, `index.ts`.
**Шаги:**
1. `protocol.ts` — дословно блок кода из PROTOCOL.md §3 (импортируя `ClientLayout` из `./layout`).
2. `layout.ts` — дословно DATA_MODEL.md §3.1. `actions.ts` — §2.1. `config.ts` — §4 (включая `DeviceInfo`). `Id`, `VarName`, `VarValue` определены один раз в `protocol.ts` и импортируются остальными через `import type`.
3. `ipc.ts` — дословно PROTOCOL.md §9.
4. `index.ts` — `export * from` всех пяти файлов.
5. Добавь в `protocol.ts` константы: `DEFAULT_PORT = 9247`, `PAIR_ALPHABET = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789"`, `LIMITS = { title: 120, body: 500, source: 40, statusText: 80, group: 40, label: 60, notifications: 20 }`.
**Приёмка:** typecheck проходит; в `shared/src` нет `any`.
**Проверка:**
```
npm run typecheck -w shared; (Select-String -Path shared\src\*.ts -Pattern '\bany\b').Count
```
(вторая команда печатает `0`)

### T1.2 — JSON-схемы, фикстуры, тесты схем
**Цель:** схемы и примеры из документов лежат файлами и взаимно проверены.
**Файлы:** `shared/schemas/protocol.schema.json`, `layout.schema.json`, `actions.schema.json`; `shared/fixtures/*.json`; `shared/test/schemas.test.ts`.
**Шаги:**
1. Схемы — дословно: PROTOCOL.md §7, DATA_MODEL.md §5 (две схемы).
2. Фикстуры — дословно, по одному файлу на пример с именем, указанным над примером: из PROTOCOL.md §4–§5 (`c2s-pair`, `s2c-paired`, `c2s-hello`, `s2c-welcome`, `s2c-patch-vars`, `c2s-press`, `s2c-press-result-ok`, `s2c-press-result-err`, `c2s-notif-dismiss`, `c2s-ping`, `s2c-pong`, `s2c-error`, `http-notify-req`, `http-status-req`, `http-var-req`, `http-error`) и из DATA_MODEL.md (`actions.default`, `layout.default`, `config.example`). Всего 19 файлов.
3. `schemas.test.ts`: `Ajv2020` из `ajv/dist/2020`, опции `{ allErrors: true, strict: false }`; `addSchema` для всех трёх схем; таблица «файл фикстуры → `$id#/$defs/<Имя>`»: `c2s-*` → `ClientMsg`, `s2c-*` → `ServerMsg`, `http-notify-req` → `NotifyReq`, `http-status-req` → `StatusReq`, `http-var-req` → `VarReq`, `http-error` → `HttpError`, `actions.default` → `ActionsFile`, `layout.default` → `LayoutFile`. Каждая фикстура обязана быть валидной. `config.example` схемой не проверяется (схемы конфига нет).
4. Негативные тесты (инлайн-объекты, каждый обязан быть невалидным): `press` без `actionId`; `press` с `actionId: "Bad Id"`; `hello` с токеном длины 10; `NotifyReq` с `title` из 121 символа; действие `hotkey` с клавишей `"ctrl+c"`; действие `open_url` с `url: "file:///c:/x"`; кнопка одновременно с `actionId` и `navigate`; кнопка размером 2×1; `grid` 6×3.
**Приёмка:** 18 позитивных и 9 негативных проверок проходят.
**Проверка:**
```
npm test -w shared; (Get-ChildItem shared\fixtures\*.json).Count
```
(вторая команда печатает `19`)

### T1.3 — Семантическая валидация раскладки и разбор QR-URL
**Цель:** функции, общие для клиента и окна настроек.
**Файлы:** `shared/src/validate.ts`, `shared/src/pairUrl.ts`, `shared/src/index.ts`, `shared/fixtures/invalid/*.json`, `shared/test/validate.test.ts`, `shared/test/pairUrl.test.ts`.
**Шаги:**
1. `validate.ts`: `validateLayout(layout: LayoutFile, actions: ActionsFile): ValidationIssue[]` — проверки DATA_MODEL.md §3.2 п. 2, 3, 4, 5 (структурные проверки схемы не дублировать). `path` — JSON pointer на нарушающий элемент (`/pages/0/items/3`); при пересечении — на тот из двух, что позже в массиве; при дубликате `id` — на второй. Также `validateActions(actions: ActionsFile): ValidationIssue[]` — проверка, что `keys` без повторов и все ключи действий соответствуют `Id`.
2. Негативные фикстуры в `shared/fixtures/invalid/`, формат `{ "expectPath": "...", "layout": { ...LayoutFile } }`. Каждая — копия `layout.default.json` с одной правкой:
   - `layout-overlap.json`: у `b-prev` `x: 1` (совпадает с `b-play`) → `expectPath: "/pages/0/items/4"`;
   - `layout-out-of-bounds.json`: у `w-notifs` `x: 4` → `"/pages/0/items/6"`;
   - `layout-dup-id.json`: у `b-github` `id: "b-play"` → `"/pages/1/items/1"`;
   - `layout-unknown-action.json`: у `b-play` `actionId: "nope"` → `"/pages/0/items/4"`;
   - `layout-bad-navigate.json`: у `b-home` `navigate: "nope"` → `"/pages/1/items/4"`;
   - `layout-bad-size.json`: у `w-sys` `w: 1` → `"/pages/0/items/1"`.
3. Тест: `layout.default.json` + `actions.default.json` → 0 замечаний; каждая негативная фикстура → ≥1 замечание, первое имеет `path === expectPath`.
4. `pairUrl.ts`: `parsePairUrl(input: string): { code: string; hosts: string[]; port: number; agentName: string } | null` по PROTOCOL.md §8 (читает фрагмент после `#`; `h` пуст → `hosts = [hostname из URL]`; `p` нет → порт из URL, иначе 9247; код приводится к верхнему регистру, дефисы и пробелы удаляются; код не из 8 символов `PAIR_ALPHABET` → `null`). `normalizeCode(input: string): string | null` — то же для ручного ввода. `parseHostInput(input: string): { host: string; port: number } | null` — `host` или `host:port`, порт по умолчанию 9247.
5. Тесты `pairUrl`: корректный URL с двумя хостами; URL без `h`; код в нижнем регистре с дефисом; мусор → `null`; `parseHostInput("192.168.1.5")`, `("192.168.1.5:9000")`, `("")` → `null`.
**Приёмка:** все тесты проходят; 6 файлов в `fixtures/invalid`.
**Проверка:**
```
npm run typecheck -w shared; npm test -w shared; (Get-ChildItem shared\fixtures\invalid\*.json).Count
```

---

## M2 — Ядро агента: конфигурация, состояние, сервер

Структура модулей `agent/core/src` — ARCHITECTURE.md §4.2. Публичный API `Agent` — §4.3.

### T2.1 — Serde-типы и round-trip по фикстурам
**Цель:** Rust-типы, совпадающие с TS-типами байт-в-байт по JSON.
**Файлы:** `agent/core/src/proto.rs`, `model.rs`, `lib.rs`, `agent/core/tests/fixtures.rs`.
**Шаги:**
1. `cargo add -p decka-core serde --features derive`; `cargo add -p decka-core serde_json thiserror` (последние).
2. `model.rs`: `ActionsFile`, `Action` (`#[serde(tag = "type", rename_all = "snake_case")]`), `MediaOp`, `SystemOp`, `LayoutFile`, `ClientLayout`, `Page`, `DisplaySettings`, `Night`, `LayoutItem` (`#[serde(tag = "kind", rename_all = "snake_case")]`), `ButtonLook`, `Bind`, `ConfigFile`, `UsbConfig`, `Collectors`, `SysCollectorConfig`, `HttpCheck`, `DevicesFile`, `DeviceRecord`, `SysStats`, `StatusItem`, `StatusState`, `Notification`, `NotifLevel`, `VarValue` (`#[serde(untagged)]`: `Bool(bool)`, `Num(serde_json::Number)`, `Str(String)`), `ValidationIssue`. Имена полей в JSON — camelCase (`#[serde(rename_all = "camelCase")]`; для enum с полями — `rename_all_fields = "camelCase"`). Все `Option` — `#[serde(default, skip_serializing_if = "Option::is_none")]`. `deny_unknown_fields` не использовать.
3. `proto.rs`: `ClientMsg` и `ServerMsg` (`#[serde(tag = "t", rename_all = "snake_case")]`), `ClientInfo`, `ErrorCode` (`SCREAMING_SNAKE_CASE`), `ErrorBody`, HTTP-типы `HealthRes`, `NotifyReq`, `NotifyRes`, `StatusReq`, `VarReq`, `OkRes`, `HttpError`. В `ServerMsg::Patch` поле `value: serde_json::Value`; в `Welcome` поле `state: serde_json::Value`.
4. `tests/fixtures.rs`: путь к фикстурам `concat!(env!("CARGO_MANIFEST_DIR"), "/../../shared/fixtures")`. Для каждой пары «файл → тип» из T1.2 п. 3 (плюс `config.example` → `ConfigFile`, для `s2c-welcome` дополнительно `state.layout` → `ClientLayout`, `state.sys` → `SysStats`): прочитать как `Value`, десериализовать в тип, сериализовать обратно в `Value`, сравнить `assert_eq!` с исходным.
**Приёмка:** round-trip проходит для всех 19 фикстур.
**Проверка:**
```
cargo test -p decka-core --test fixtures
```

### T2.2 — Каталог конфигурации: загрузка, значения по умолчанию, валидация, атомарная запись
**Цель:** модуль `config` по DATA_MODEL.md §1–§4.
**Файлы:** `agent/core/src/config.rs`, `validate.rs`, `paths.rs`, `agent/core/tests/config.rs`.
**Шаги:**
1. `cargo add -p decka-core dirs url rand@0.10.3 base64 sha2` (без версии — последние); `cargo add -p decka-core --dev tempfile`.
2. `paths.rs`: `config_dir(override: Option<PathBuf>) -> PathBuf` — приоритет: аргумент, затем env `DECKA_CONFIG_DIR`, затем `dirs::config_dir()/Decka`.
3. `config.rs`: `load_or_init(dir) -> Loaded { config, actions, layout, devices, issues: Vec<(String /*file*/, ValidationIssue)> }`. Отсутствующий файл создаётся из значений по умолчанию; значения по умолчанию для `actions.json` и `layout.json` встроены через `include_str!` из `shared/fixtures/actions.default.json` и `layout.default.json`. `apiToken` при создании: `"dk_"` + base64url без паддинга от 32 байт из криптографического генератора ОС. `agentName` — env `COMPUTERNAME`, иначе `"PC"`. Поведение при невалидном файле — DATA_MODEL.md §1.
4. `save_json_atomic(path, &T)`: запись в `<path>.tmp`, `sync_all`, rename; формат — `serde_json::to_string_pretty` + `\n`.
5. `validate.rs`: `validate_actions(&ActionsFile) -> Vec<ValidationIssue>`, `validate_layout(&LayoutFile, &ActionsFile) -> Vec<ValidationIssue>`, `validate_config(&ConfigFile) -> Vec<ValidationIssue>` — **все** правила DATA_MODEL.md §2.2, §2.3 (колонка «Валидация»), §3.2 (п. 1–8), §3.3, §4 (диапазоны из комментариев). В Rust JSON Schema не используется — правила кодируются вручную. Формат `path` — как в T1.3.
6. Тесты: (а) пустой каталог → создаются 4 файла, 0 замечаний, `apiToken` соответствует `^dk_[A-Za-z0-9_-]{43}$`; (б) каждая из 6 фикстур `shared/fixtures/invalid/*` даёт первое замечание с `path == expectPath`; (в) `layout.json` с синтаксической ошибкой JSON → `load_or_init` не падает, раскладка — одна пустая страница `main`, файл на диске не изменён; (г) действие `open_url` с `ftp://x` → замечание; `hotkey` с клавишей `"f25"` → замечание; `port: 80` → замечание; `sleepAfterSec: 30, dimAfterSec: 60` → замечание; (д) `save_json_atomic` не оставляет `.tmp`.
**Приёмка:** все перечисленные тесты проходят.
**Проверка:**
```
cargo test -p decka-core --test config
```

### T2.3 — Хранилище состояния с рассылкой только при изменении
**Цель:** `Store` — секции, `rev`, broadcast (ARCHITECTURE.md §5.2).
**Файлы:** `agent/core/src/state.rs`, юнит-тесты в том же файле.
**Шаги:**
1. `cargo add -p decka-core tokio@1.54.1 --features rt-multi-thread,macros,sync,time,net,process,signal,fs`.
2. `Store` (внутри `Arc<Mutex<...>>`, `std::sync::Mutex`): пять секций как `serde_json::Value` (`layout`, `sys`, `statuses`, `notifications`, `vars`), `rev: u64` (старт 0), `tokio::sync::broadcast::Sender<Patch>` ёмкостью 64, где `Patch { rev: u64, section: &'static str, value: Arc<Value> }`.
3. Методы: `set(section, value: Value) -> bool` — если `value == текущее` → `false`, без рассылки, `rev` не меняется; иначе записать, `rev += 1`, разослать, вернуть `true`. `get(section) -> Value`. `snapshot() -> (u64, Value /* FullState */)`. `subscribe() -> Receiver<Patch>`. `receiver_count() -> usize`.
4. Типизированные помощники поверх `set`: `update_vars(|&mut BTreeMap<String, VarValue>|)`, `update_statuses(|&mut Vec<StatusItem>|)` (после замыкания сортирует по `(group, id)`), `update_notifications(|&mut Vec<Notification>|)` (после замыкания обрезает до 20).
5. Тесты: `set` тем же значением → `false` и подписчик ничего не получает; два разных `set` → `rev` 1 и 2 и два сообщения по порядку; `snapshot` содержит все пять ключей; `update_statuses` сортирует; `update_notifications` обрезает до 20.
**Приёмка:** тесты проходят.
**Проверка:**
```
cargo test -p decka-core state::
```

### T2.4 — HTTP-сервер: health, статика, жизненный цикл; headless-режим
**Цель:** `Agent::start` поднимает axum; headless-бинарь запускает агент.
**Файлы:** `agent/core/src/agent.rs`, `server/mod.rs`, `server/http.rs`, `server/static_files.rs`, `lib.rs`, `bin/decka-headless.rs`, `agent/core/tests/server.rs`.
**Шаги:**
1. `cargo add -p decka-core axum@0.8.9 --features ws`; `cargo add -p decka-core tower-http@0.7.1 --features fs,set-header`; `cargo add -p decka-core tracing tracing-subscriber local-ip-address@0.6.13`; `cargo add -p decka-core --dev reqwest@0.13.5`. Если `tower-http 0.7.1` не компилируется с `axum 0.8.9` — запасной вариант (единственный разрешённый): версия `tower-http`, указанная в зависимостях `axum 0.8.9` (`cargo tree -p axum`), запись в описании коммита.
2. `AgentOptions { config_dir: Option<PathBuf>, client_dist: Option<PathBuf>, port_override: Option<u16>, runner: Arc<dyn ActionRunner>, fixed_pair_code: Option<String> }`. Трейт `ActionRunner` объяви сейчас в `actions/mod.rs`: `fn run(&self, id: &str, action: &Action) -> Result<(), String>` (`Send + Sync`), и реализацию `DryRunner`, печатающую в stdout строку `DRY <id> <type>` и возвращающую `Ok(())`.
3. `Agent::start(opts).await -> Result<Agent, StartError>`: `load_or_init`; создать `Store` (секция `layout` = раскладка без `version`; `sys` = `null`; `statuses` = `[]`; `notifications` = `[]`; `vars` = `{}`); замечания валидации файлов превратить в уведомления по DATA_MODEL.md §1; привязать `TcpListener` к `config.bind:port` (или `port_override`; `0` = любой свободный). Порт занят → `Agent` всё равно создаётся, `status().listening = None`, `listen_error` заполнен.
4. Роутер: `GET /api/health` → `HealthRes`; любой другой `/api/*` → 404 `HttpError NOT_FOUND`; остальное — `ServeDir(client_dist)` с fallback на `index.html`; если `client_dist` не задан или каталога нет → 404 с текстом `client not bundled`. Ко всем ответам заголовок `X-Content-Type-Options: nosniff`; к `index.html` — `Cache-Control: no-cache`. CORS-слой не добавлять. Сервис поднимать через `into_make_service_with_connect_info::<SocketAddr>()`. Лимит тела запроса 16384 байт (`DefaultBodyLimit`).
5. `Agent::status() -> AgentStatus` (поля `usb` пока: `enabled` из конфига, остальное пустое/false), `Agent::shutdown().await` (graceful), `Agent::restart_server().await`.
6. `lan_ips()`: IPv4-адреса интерфейсов из частных диапазонов (10/8, 172.16/12, 192.168/16), первым — адрес из `local_ip()`, максимум 4.
7. `decka-headless`: аргументы `--port <n>`, `--config-dir <path>`, `--client-dist <path>`, `--pair-code <CODE>`, `--dry-run`, `--version`. Без `--dry-run` пока тоже используется `DryRunner` (реальный исполнитель — M3). Инициализирует `tracing_subscriber` (уровень из env `DECKA_LOG`, по умолчанию `info`), после успешной привязки печатает в stdout `LISTENING <bind>:<port>`, работает до Ctrl+C. Порт занят → печатает `LISTEN_ERROR <текст>` и выходит с кодом 2.
8. Тест `tests/server.rs`: старт с `port_override: Some(0)` и временным каталогом; `GET /api/health` → 200 и `{"name":"decka","version":"0.1.0","protocol":1}`; `GET /api/nope` → 404 с `code: "NOT_FOUND"`; ответ health не содержит заголовка `access-control-allow-origin`; второй агент на том же порту → `listening == None`.
**Приёмка:** тесты проходят; health отвечает из headless.
**Проверка:**
```
cargo test -p decka-core --test server
```
затем H-BUILD; H-START; затем
```
curl.exe -s http://127.0.0.1:19247/api/health; curl.exe -s -o NUL -w "%{http_code}" http://127.0.0.1:19247/
```
затем H-STOP. Ожидается JSON health и код `200` (если `client\dist` собран в T0.3).

### T2.5 — Сопряжение, токены устройств, ограничитель частоты
**Цель:** модуль `auth` по ARCHITECTURE.md §6.2–§6.3.
**Файлы:** `agent/core/src/auth.rs`, `ratelimit.rs`, `agent.rs`, юнит-тесты в этих файлах.
**Шаги:**
1. `ratelimit.rs`: `RateLimiter::new(max: u32, window: Duration)`, `check(key: &str) -> bool` — фиксированное окно на ключ, `Mutex<HashMap>`; записи старше окна удаляются при обращении. Время передаётся через параметр `now: Instant` во внутреннем методе, чтобы тесты не спали.
2. `auth.rs`:
   - `Pairing`: не более одного активного кода. `start() -> (code, expires_at)` — 8 символов `PAIR_ALPHABET` из криптографического генератора, TTL 120 с, заменяет предыдущий код. `cancel()`. `try_consume(code) -> bool`: сравнение за постоянное время (побайтовый XOR-аккумулятор по всей длине); успех → код уничтожается; неуспех → счётчик ошибок +1, на 5-й ошибке код уничтожается. Нет активного кода или истёк → `false`.
   - `fixed_pair_code` из `AgentOptions`: если задан — действует 3600 с, **не расходуется** при успехе и не имеет счётчика ошибок (только для headless/тестов; в сборке Tauri всегда `None`).
   - `Devices`: `create(name) -> (DeviceRecord, token)` — `id = "d-" + 6 hex-символов`, `token` = base64url без паддинга от 32 случайных байт (43 символа), в запись идёт `tokenHash = hex(sha256(token))`; `verify(token) -> Option<DeviceId>` (поиск по хэшу); `touch(id)` (обновляет `lastSeenAt`); `revoke(id) -> bool` (удаляет запись устройства); `list()`. Каждое изменение сохраняет `devices.json` атомарно.
3. `Agent`: методы `start_pairing() -> PairingInfo` (поле `qrSvg` пока пустая строка — T6.3), `cancel_pairing()`, `list_devices()`, `revoke_device(id)`.
4. Тесты: код из 8 символов алфавита; верный код работает один раз; 5 неверных попыток гасят код (6-я попытка с верным кодом → `false`); истёкший код → `false` (время через параметр); `verify` находит созданное устройство и не находит после `revoke`; в `devices.json` нет подстроки токена; лимитер: 3 из 3 проходят, 4-й нет, после окна снова проходит.
**Приёмка:** тесты проходят.
**Проверка:**
```
cargo test -p decka-core auth::; cargo test -p decka-core ratelimit::
```

### T2.6 — WebSocket: рукопожатие, welcome, patch, ping, отзыв
**Цель:** эндпоинт `/ws` по PROTOCOL.md §1, §4, §6.
**Файлы:** `agent/core/src/server/ws.rs`, `server/mod.rs`, `agent.rs`, `agent/core/tests/ws.rs`, `tools/ws-smoke.mjs`.
**Шаги:**
1. `cargo add -p decka-core futures-util`; `cargo add -p decka-core --dev tokio-tungstenite` (последние).
2. Проверка `Origin` до апгрейда: заголовка нет → разрешено; есть → разрешено, только если равен `http://<значение заголовка Host>`, `http://localhost`, `http://localhost:5173` или `http://127.0.0.1:5173`; иначе ответ 403 без апгрейда.
3. Максимальный размер сообщения 4096 байт (настройка апгрейда axum).
4. Рукопожатие: ждать первый кадр ≤ 5 с. `pair`: лимитер `pair` 10/мин на IP (превышение → `error RATE_LIMITED` fatal, закрытие 4429); проверка `v` (→ `PROTOCOL_VERSION`, 4426, поле `supported: [1]`); `try_consume` → неуспех: `PAIR_CODE_INVALID`, 4401; успех: создать устройство с `client.name`, отправить `paired`, затем `welcome`. `hello`: проверка `v`; `verify(token)` → неуспех: `UNAUTHORIZED`, 4401; успех: `touch`, `welcome`. Любое другое первое сообщение или таймаут → `UNAUTHORIZED`, 4401.
5. Порядок для `welcome`: сначала `store.subscribe()`, затем `store.snapshot()`, затем отправка `welcome` с `rev` снимка; далее пересылаются только патчи с `rev` > `rev` снимка.
6. Основной цикл (`tokio::select!`): патч из broadcast → `patch`; `RecvError::Lagged` → закрыть сокет кодом 1011; входящий `ping` → `pong` с тем же `ts`; `notif_dismiss` → удалить уведомление из секции; `press` → пока ответ `press_result` с `UNKNOWN_ACTION` (реализация — T3.1); невалидный JSON/схема → `error BAD_MESSAGE`, `fatal: false`; тишина 30 с → закрыть 1001.
7. Реестр живых соединений в `Agent`: `deviceId → Vec<канал отмены>` и список для `status().clients` (`deviceId`, `name`, `remote`, `connectedAt`). `revoke_device` шлёт в соединения устройства `error DEVICE_REVOKED` fatal и закрывает 4403.
8. `tools/ws-smoke.mjs` (Node ≥ 22, встроенный глобальный `WebSocket`, без зависимостей). Команды; каждая печатает одну строку JSON в stdout и завершает процесс; при `error` от сервера или таймауте 5 с — печатает ошибку в stderr и выходит с кодом 1:
   - `pair <host:port> <code>` → `{"deviceId","token"}`;
   - `state <host:port> <token>` → `welcome.state`;
   - `press <host:port> <token> <actionId>` → объект `press_result`;
   - `watch <host:port> <token> <seconds>` → каждая полученная за это время `patch` отдельной строкой, код выхода 0;
   - `raw <host:port> <файл.json> [token]` → если задан `token`, сначала выполняет `hello` и ждёт `welcome`; затем отправляет содержимое файла одним кадром и печатает первый ответ сервера, не являющийся `patch`. JSON передаётся файлом, потому что PowerShell 5.1 портит кавычки в аргументах нативных программ. При чтении файла срезается BOM.
   Имя клиента в `pair`/`hello`: `{"name":"ws-smoke","app":"web","version":"0.0.0"}`.
9. Тесты `tests/ws.rs` (клиент `tokio-tungstenite`, агент с `fixed_pair_code`, `port_override: Some(0)`): pair → `paired` + `welcome`; hello с полученным токеном → `welcome`, `state` содержит 5 секций; hello с мусорным токеном → `error UNAUTHORIZED` и код закрытия 4401; `v: 2` → `PROTOCOL_VERSION`, 4426, `supported == [1]`; первый кадр `press` → 4401; после welcome `store.set("vars", …)` → приходит `patch` с большим `rev`; повторный `set` тем же значением → патча нет в течение 300 мс; `ping` → `pong` с тем же `ts`; `revoke_device` → `DEVICE_REVOKED` и 4403; мусорная строка после авторизации → `BAD_MESSAGE` и сокет жив (последующий `ping` получает `pong`); запрос с `Origin: http://evil.example` → HTTP 403.
**Приёмка:** все тесты проходят; smoke-скрипт получает токен.
**Проверка:**
```
cargo test -p decka-core --test ws
```
затем H-BUILD; H-START; H-TOKEN; затем
```
$tok.Length; node tools/ws-smoke.mjs state 127.0.0.1:19247 $tok | ConvertFrom-Json | Select-Object -ExpandProperty layout | Select-Object -ExpandProperty pages | Measure-Object | Select-Object -ExpandProperty Count
```
затем H-STOP. Ожидается `43` и `2`.

### T2.7 — Отслеживание actions.json и layout.json
**Цель:** горячая перезагрузка по DATA_MODEL.md §1.
**Файлы:** `agent/core/src/watch.rs`, `agent.rs`, `agent/core/tests/watch.rs`.
**Шаги:**
1. `cargo add -p decka-core notify@8.2.2`.
2. Наблюдатель за каталогом конфигурации (не рекурсивно); события по `actions.json` и `layout.json` → debounce 300 мс → обработать только изменившийся файл (сравнение содержимого с последним применённым; не изменилось — ничего не делать, это же гасит события от собственной записи агента). Изменился `actions.json`: `validate_actions`; валидно → заменить в памяти. Изменился `layout.json`: `validate_layout` против текущих действий; валидно → заменить в памяти, `store.set("layout", …)`. Невалидно → оставить прежнее, `push_notification` по DATA_MODEL.md §1.
3. `Agent::push_notification(NotifyReq) -> String /* id */`: счётчик id с 1, `ts` = сейчас, вставка в начало, обрезка до 20.
4. `Agent::save_actions(ActionsFile) -> SaveResult`, `save_layout(LayoutFile) -> SaveResult`: валидация → при замечаниях вернуть их и ничего не писать; иначе атомарная запись, обновление памяти и секции `layout`. `save_actions` проверяет только сами действия (`validate_actions`): удаление действия, на которое ссылается кнопка, разрешено, ссылка остаётся висячей (DATA_MODEL.md §3.2, последний абзац). По той же причине наблюдатель при изменении `actions.json` применяет новые действия, даже если текущая раскладка ссылается на исчезнувшее действие; а `layout.json` с несуществующим `actionId` (изменён файл раскладки) — отклоняется. `Agent::actions()`, `Agent::layout()` — геттеры.
5. Тесты: запись валидного `layout.json` с переименованной страницей → в течение 2 с секция `layout` изменилась; запись невалидного → секция `layout` не изменилась, в `notifications` появился элемент с `level: "error"` и `title: "layout.json invalid"`; `save_layout` с пересечением → `ok: false`, файл на диске прежний.
**Приёмка:** тесты проходят.
**Проверка:**
```
cargo test -p decka-core --test watch
```

---

## M3 — Действия

### T3.1 — Обработка press, toggleVar, лимит
**Цель:** `press` исполняет действие через `ActionRunner`.
**Файлы:** `agent/core/src/actions/mod.rs`, `server/ws.rs`, `agent.rs`, `agent/core/tests/press.rs`.
**Шаги:**
1. Добавь публичный модуль `actions::testing` с `RecordingRunner` (без cfg-ограничений и без cargo-features — он нужен интеграционным тестам): хранит `Mutex<Vec<String>>` id выполненных действий (`calls() -> Vec<String>`), конструкторы `new()` и `failing(msg)` (режим «всегда ошибка»).
2. `Agent::execute(action_id) -> Result<(), (ErrorCode, String)>`: нет действия → `UNKNOWN_ACTION` с сообщением `action '<id>' is not defined`; `runner.run` выполняется в `tokio::task::spawn_blocking`; `Err(msg)` → `ACTION_FAILED`; успех и задан `toggleVar` → `store.update_vars`: булево значение инвертируется, отсутствующее или небулево → `true`.
3. WS `press`: лимитер 20 событий в секунду на соединение (свой экземпляр на сокет) → при превышении `press_result` c `RATE_LIMITED`; иначе `execute` и `press_result`. Обработка `press` не блокирует чтение сокета (отдельная задача tokio; ответы могут приходить не по порядку — клиент сопоставляет по `id`).
4. `Agent::test_action(id)` — то же `execute`, для окна настроек.
5. Тесты: `press` существующего действия → `ok: true` и id в `RecordingRunner`; несуществующего → `UNKNOWN_ACTION`; runner с ошибкой → `ACTION_FAILED`, переменная не изменилась; `vol-mute` дважды → `vars["audio.muted"]` `true`, затем `false`, и приходят два `patch` секции `vars`; 30 `press` подряд → хотя бы один `RATE_LIMITED`, сокет жив.
**Приёмка:** тесты проходят; smoke-нажатие в dry-run отвечает `ok`.
**Проверка:**
```
cargo test -p decka-core --test press
```
затем H-BUILD; H-START; H-TOKEN; затем
```
node tools/ws-smoke.mjs press 127.0.0.1:19247 $tok media-play; node tools/ws-smoke.mjs press 127.0.0.1:19247 $tok nope
```
затем H-STOP; `Select-String -Path .tmp\headless.log -Pattern 'DRY media-play media'`. Ожидается: первая строка содержит `"ok":true`, вторая — `"UNKNOWN_ACTION"`, в логе найдена строка `DRY`.

### T3.2 — WindowsRunner: launch, open_url, system
**Цель:** реальное исполнение трёх типов действий по DATA_MODEL.md §2.3.
**Файлы:** `agent/core/src/actions/process.rs`, `actions/windows.rs`, `bin/decka-headless.rs`.
**Шаги:**
1. `WindowsRunner` реализует `ActionRunner`; ветки `launch`, `open_url`, `system` — в `process.rs`, строго по таблице DATA_MODEL.md §2.3. Везде `std::process::Command` без `cmd /C` и без PowerShell. `stdin/stdout/stderr` → `Stdio::null()`. Для `open_url` и `system` всегда флаг `CREATE_NO_WINDOW`; для `launch` — только при `hidden: true`.
2. `open_url`: перед запуском URL повторно парсится crate `url`, схема не `http`/`https` → `Err("unsupported URL scheme")`.
3. Ветки `hotkey` и `media` пока возвращают `Err("not implemented")` (T3.3).
4. Код, зависящий от Windows, под `#[cfg(windows)]`; для прочих ОС `WindowsRunner::run` возвращает `Err("unsupported platform")` (чтобы крейт собирался в CI на любой ОС).
5. `decka-headless` без `--dry-run` использует `WindowsRunner`.
6. Юнит-тест: `launch` несуществующей программы `decka-no-such-program.exe` → `Err`; `open_url` с `file:///c:/x` → `Err`.
**Приёмка:** тесты проходят; запуск реальной программы из headless работает.
**Проверка:**
```
cargo test -p decka-core actions::
cargo build -p decka-core --bin decka-headless
New-Item -ItemType Directory -Force .tmp\cfg2 | Out-Null
Set-Content -Encoding utf8 .tmp\cfg2\actions.json '{"version":1,"actions":{"charmap":{"name":"Charmap","type":"launch","program":"charmap.exe"}}}'
Set-Content -Encoding utf8 .tmp\cfg2\layout.json '{"version":1,"grid":{"cols":5,"rows":3},"display":{"dimAfterSec":60,"dimLevel":0.3,"sleepAfterSec":600,"pixelShift":true,"night":{"enabled":false,"from":"23:00","to":"07:00"}},"pages":[{"id":"main","name":"Main","items":[]}]}'
$h = Start-Process -FilePath target\debug\decka-headless.exe -ArgumentList '--port','19247','--config-dir','.tmp\cfg2','--pair-code','TESTCDE2' -PassThru -NoNewWindow -RedirectStandardOutput .tmp\headless.log
Start-Sleep -Seconds 2
$tok = (node tools/ws-smoke.mjs pair 127.0.0.1:19247 TESTCDE2 | ConvertFrom-Json).token
node tools/ws-smoke.mjs press 127.0.0.1:19247 $tok charmap
Start-Sleep -Seconds 2
(Get-Process charmap -ErrorAction SilentlyContinue | Measure-Object).Count
Get-Process charmap -ErrorAction SilentlyContinue | Stop-Process -Force
Stop-Process -Id $h.Id -Force
```
Ожидается `"ok":true` и число ≥ 1. Примечание: Windows PowerShell 5.1 пишет `utf8` с BOM — загрузчик конфигурации обязан срезать BOM (`\u{FEFF}`) перед разбором JSON; добавь это в `config.rs` в рамках этой задачи вместе с юнит-тестом.

### T3.3 — WindowsRunner: hotkey и media через enigo
**Цель:** эмуляция клавиш по DATA_MODEL.md §2.2–§2.3.
**Файлы:** `agent/core/src/actions/keys.rs`, `actions/windows.rs`.
**Шаги:**
1. `cargo add -p decka-core enigo@0.6.1` (в секцию `[target.'cfg(windows)'.dependencies]`).
2. `keys.rs`: `fn map_key(name: &str) -> Option<enigo::Key>` для всего набора DATA_MODEL.md §2.2 (`ctrl`→Control, `alt`→Alt, `shift`→Shift, `win`→Meta, буквы и цифры → `Key::Unicode(char)`, `f1..f24`, `enter`→Return, `esc`→Escape, `space`, `tab`, `backspace`, `delete`, `home`, `end`, `pageup`, `pagedown`, стрелки) и `fn map_media(op: MediaOp) -> enigo::Key` по таблице §2.3. Точные имена вариантов сверить с docs.rs/enigo/0.6.1.
3. Экземпляр `Enigo` создаётся один раз в **выделенном потоке ОС** (`std::thread::spawn`), команды передаются через `std::sync::mpsc` вместе с каналом ответа; `WindowsRunner` хранит отправитель. Причина: `Enigo` не гарантирует `Send`.
4. `hotkey`: press всех клавиш по порядку, пауза 20 мс, release в обратном порядке. При ошибке на любом шаге — release уже нажатых, затем `Err`. `media`: один click.
5. Юнит-тесты (под `#[cfg(windows)]`): `map_key` возвращает `Some` для каждого имени из набора (перечисли набор массивом в тесте: 4 модификатора, 26 букв, 10 цифр, 24 F-клавиши, 14 именованных = 78 имён) и `None` для `"f25"`, `"ctrl+c"`, `"A"`.
**Приёмка:** тесты проходят. Ручная проверка: headless без `--dry-run` с конфигом по умолчанию; `press task-manager` открывает Диспетчер задач.
**Проверка:**
```
cargo test -p decka-core keys::
```
Ручная (один вызов): H-BUILD; запусти headless как в H-START, но **без** `'--dry-run'`; H-TOKEN; `node tools/ws-smoke.mjs press 127.0.0.1:19247 $tok task-manager; Start-Sleep 2; (Get-Process Taskmgr -ErrorAction SilentlyContinue | Measure-Object).Count; Get-Process Taskmgr -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue`; H-STOP. Ожидается число ≥ 1 (если Диспетчер задач не закрылся из-за прав — закрой вручную).

---

## M4 — Клиент: соединение, сопряжение, сетка

Структура `client/src` — ARCHITECTURE.md §4.4. Визуальные константы — ARCHITECTURE.md §7.

### T4.1 — Хранилище клиента и локальное хранение
**Цель:** единое состояние приложения без сторонних библиотек.
**Файлы:** `client/src/store/store.ts`, `store/storage.ts`, `client/test/store.test.ts`.
**Шаги:**
1. `storage.ts`: ключ `localStorage` `decka.conn.v1`, значение `SavedConn { token: string; deviceId: string; hosts: string[]; port: number; agentName: string }`; функции `loadConn(): SavedConn | null` (битый JSON → `null`), `saveConn(c)`, `clearConn()`. Доступ к `localStorage` в try/catch.
2. `store.ts`: тип `AppState { conn: "unpaired" | "connecting" | "online" | "offline" | "update_required"; via: "usb" | "lan" | null; agentName: string | null; rev: number; state: FullState | null; pageIndex: number; toasts: Toast[]; lastError: string | null }`, `Toast { id: number; level: NotifLevel; title: string; body?: string; until: number }`. Реализация: модульная переменная + `Set` подписчиков; `getState()`, `setState(partial | (s) => partial)`, `subscribe(fn)`, хук `useAppState(selector)` на `useSyncExternalStore`.
3. Редьюсеры-функции: `applyWelcome(msg: WelcomeMsg)` (ставит `state`, `rev`, `conn: "online"`, `agentName`; если `pageIndex` ≥ числа страниц → 0), `applyPatch(msg: PatchMsg)` (игнор при `rev <= state.rev` или неизвестной секции; иначе заменить секцию и `rev`; при секции `layout` — та же коррекция `pageIndex`), `pushToast(t)`, `expireToasts(now)`.
4. Тесты (среда node): `applyPatch` со старым `rev` не меняет состояние; с неизвестной секцией `"foo"` не падает и не меняет; `applyWelcome` сбрасывает `pageIndex`, если страниц стало меньше.
**Приёмка:** тесты и typecheck проходят.
**Проверка:**
```
npm run typecheck -w client; npm test -w client
```

### T4.2 — Машина соединения
**Цель:** надёжное подключение с перебором адресов, переподключением и heartbeat.
**Файлы:** `client/src/net/connection.ts`, `net/candidates.ts`, `client/test/connection.test.ts`.
**Шаги:**
1. `candidates.ts`: `buildCandidates(env: { native: boolean; devHost?: string; locationHost?: string }, saved: { hosts: string[]; port: number } | null): string[]` (строки `host:port`, без дубликатов, порядок важен): `native` → `["127.0.0.1:<port>", ...saved.hosts с портом]` (порт из `saved`, иначе 9247); не native и задан `devHost` → `[devHost]`; иначе → `[locationHost]`.
2. `connection.ts`: класс `Connection`, зависимости через конструктор: `{ WebSocketCtor, now, setTimeout, clearTimeout, candidates: () => string[], clientInfo: ClientInfo, onWelcome, onPatch, onPressResult, onStatus: (conn, via) => void, onFatal: (code: ErrorCode) => void }`.
3. Алгоритм попытки: по очереди для каждого кандидата открыть `ws://<кандидат>/ws`; после `open` отправить `hello` (или `pair` в режиме сопряжения); ждать `welcome` ≤ 1500 мс от начала попытки с этим кандидатом; не дождались/ошибка → закрыть, следующий кандидат. Успех → `via = "usb"`, если кандидат начинается с `127.0.0.1:` и `native`, иначе `"lan"`. Все кандидаты неуспешны → `conn: "offline"`, повтор через задержку из ряда 1, 2, 4, 8, 15, 15, … с (сброс ряда после успешного `welcome`).
4. Heartbeat: `ping` каждые 10 с; нет ни одного входящего кадра 25 с → закрыть сокет. Любая потеря установленного соединения (тишина или обрыв) → первая повторная попытка через 1 с с первого кандидата, далее по ряду из п. 3.
5. Фатальные ошибки: `UNAUTHORIZED`, `DEVICE_REVOKED` → `onFatal`, переподключение прекращается (приложение очищает сохранённое соединение и показывает сопряжение); `PROTOCOL_VERSION` → `onFatal`, переподключение прекращается (`update_required`); `PAIR_CODE_INVALID`, `RATE_LIMITED` в режиме сопряжения → промис сопряжения отклоняется с кодом.
6. API: `start()`, `stop()`, `pair(code): Promise<PairedMsg>` (после `paired` соединение остаётся открытым и ждёт `welcome`), `press(actionId): Promise<void>` — `id` с 1 по возрастанию на соединение, таймаут 5 с → reject `{ code: "TIMEOUT" }`; `ok: false` → reject с `error`; нет соединения → reject `{ code: "OFFLINE" }`; `dismiss(notifId)`.
7. Событие `visibilitychange` → `visible`: если сокет не открыт — немедленная попытка.
8. Тесты с поддельным `WebSocketCtor` и `vi.useFakeTimers()`: первый кандидат не отвечает → через 1500 мс пробуется второй; после успеха `via` корректен; обрыв → задержки 1, 2, 4, 8, 15, 15; после успеха ряд сброшен; `press` получает результат по `id` при ответах не по порядку; `press` без ответа → reject через 5 с; `UNAUTHORIZED` → `onFatal` и новых попыток нет; тишина 25 с → сокет закрыт клиентом.
**Приёмка:** тесты проходят.
**Проверка:**
```
npm run typecheck -w client; npm test -w client
```

### T4.3 — Экран сопряжения (ручной ввод и URL-фрагмент)
**Цель:** пользователь сопрягает клиент без сканера (сканер — T7.3).
**Файлы:** `client/src/screens/PairScreen.tsx`, `client/src/app/controller.ts`, `client/src/platform/native.ts`, `client/src/App.tsx`, `styles.css`.
**Шаги:**
1. `native.ts`: `isNative(): boolean` — пока всегда `false` (Capacitor — T7.2); `clientInfo(): ClientInfo` — `name`: `"Browser"`, `app: "web"`, `version` из `import.meta.env.VITE_APP_VERSION` (пропиши `define` в `vite.config.ts` из `package.json`).
2. `controller.ts` — единственное место, где создаётся `Connection` и связывается со `store`: `init()` (читает `loadConn`; есть → `start()`, нет → `conn: "unpaired"`), `pairWith(hosts: string[], port: number, code: string, agentName: string)` (кандидаты для сопряжения: в native сначала `127.0.0.1:<port>`, затем `hosts`; успех → `saveConn`), `unpair()` (`clearConn`, `stop`, `conn: "unpaired"`), `press(actionId)`, `dismiss(id)`. `onFatal`: `UNAUTHORIZED`/`DEVICE_REVOKED` → `unpair()`; `PROTOCOL_VERSION` → `conn: "update_required"`.
3. При старте: если `location.hash` содержит `c=` → `parsePairUrl(location.href)`; успех → стереть hash через `history.replaceState`, затем `pairWith(...)` (даже если сохранённое соединение уже есть — оно заменяется).
4. `PairScreen`: заголовок `Pair with your PC`; текст-подсказка `Open Decka on your PC → Pairing, then scan the QR code or type the code below.`; поле `PC address` (плейсхолдер `192.168.1.10`; в веб-режиме предзаполнено `location.host` и скрыто); поле `Code` (плейсхолдер `XXXX-XXXX`, автокапитализация, максимум 9 символов); кнопка `Pair`. Валидация через `parseHostInput` и `normalizeCode`; невалидно → текст под полем `Enter a valid address` / `Enter the 8-character code`. Во время запроса кнопка заблокирована с текстом `Pairing…`. Ошибки: `PAIR_CODE_INVALID` → `Code is wrong or expired. Generate a new one on the PC.`; `RATE_LIMITED` → `Too many attempts. Wait a minute.`; не удалось соединиться → `Can't reach the PC. Check Wi-Fi or the USB cable.`
5. `App.tsx`: `unpaired` → `PairScreen`; `update_required` → экран с текстом `Update the Decka app to continue.`; иначе — временная заглушка `Connected to <agentName>` (сетка — T4.4).
**Приёмка:** сборка проходит; сопряжение через URL-фрагмент работает против headless.
**Проверка:**
```
npm run typecheck -w client; npm test -w client; npm run build -w client
```
Ручная: H-BUILD; H-START; открыть в браузере ПК `http://127.0.0.1:19247/#c=TESTCDE2&h=127.0.0.1&p=19247&n=Test` → видна надпись `Connected to` и имя ПК; hash в адресной строке исчез; после перезагрузки страницы надпись остаётся (токен сохранён); H-STOP.

### T4.4 — Сетка, страницы, свайп, навигация
**Цель:** отрисовка раскладки 5×3 и переключение страниц.
**Файлы:** `client/src/screens/DeckScreen.tsx`, `client/src/deck/Grid.tsx`, `deck/Tile.tsx`, `deck/PageDots.tsx`, `styles.css`.
**Шаги:**
1. `DeckScreen`: корневой контейнер `.deck` на весь экран, внутренний `.deck-inner` с отступом 6 px от краёв (запас под pixel shift, T8.2).
2. `Grid`: CSS grid `repeat(5, 1fr)` × `repeat(3, 1fr)`, `gap: 1.6vmin`, `padding: 2vmin`; элемент размещается через `grid-column: x+1 / span w`, `grid-row: y+1 / span h`.
3. `Tile` — общий контейнер плитки: `border-radius: 2.5vmin`, фон `#16161a`, `overflow: hidden`. Диспетчер по `kind`: `button` → пока плитка с `label` (полная кнопка — T4.5); `clock`/`sys`/`statuses`/`status`/`notifications` → пока плитка с названием вида (виджеты — M5); неизвестный `kind` → плитка с текстом `?`.
4. Страницы: показывается страница `pageIndex`. Свайп: pointer-события на `.deck`; горизонтальное смещение ≥ 60 px при вертикальном < 40 px → следующая/предыдущая страница (без зацикливания). Свайп, начавшийся на кнопке, отменяет её нажатие (см. T4.5). Смена страницы — мгновенная, без анимации.
5. `PageDots`: при числе страниц > 1 — точки по центру снизу (диаметр 1.2vmin, активная `#e5e5e5`, остальные `#3a3a40`), `pointer-events: none`.
6. Кнопка с `navigate` переключает на страницу с этим `id` (локально, без обращения к серверу).
7. `App.tsx`: `connecting`/`online`/`offline` при наличии `state` → `DeckScreen`; `state` ещё нет → текст `Connecting…`.
**Приёмка:** сборка проходит. Ручная: с конфигом по умолчанию видны 10 плиток на первой странице в позициях из `layout.default.json`; свайп влево открывает страницу `Dev` с 5 плитками; плитка `Main` возвращает на первую; точек две.
**Проверка:**
```
npm run typecheck -w client; npm run build -w client
```
Ручная: как в T4.3 (после сопряжения).

### T4.5 — Кнопка: вид, иконки, состояния, нажатие, подтверждение
**Цель:** полноценная кнопка по DATA_MODEL.md §3.3–§3.4.
**Файлы:** `client/src/deck/Button.tsx`, `deck/Icon.tsx`, `deck/icons.ts`, `deck/look.ts`, `client/test/look.test.ts`, `styles.css`.
**Шаги:**
1. `icons.ts`: именованные импорты из `lucide-react` ровно для 60 имён DATA_MODEL.md §3.6 и карта `Record<string, LucideIcon>` (ключ — kebab-case имя). Импорт всего пакета (`import * as`) запрещён (размер сборки).
2. `Icon.tsx`: `lucide:<name>` → компонент из карты (нет → `square`), `strokeWidth 1.75`, размер 38% меньшей стороны плитки; `emoji:<x>` → `<span>` того же размера.
3. `look.ts`: `resolveLook(item: ButtonItem, vars): ButtonLook` по DATA_MODEL.md §3.4. Тесты: нет `bind` → базовый; переменная `true` и есть ключ `"true"` → наложение только заданных полей; переменная отсутствует и есть ключ `"false"` → он применяется; переменная `"x"` без ключа → базовый.
4. Вёрстка: фон плитки = `color` (нет → `#1e1e24`); иконка по центру; `label` снизу, 2.6vmin, одна строка с многоточием; нет иконки → `label` по центру, 3.4vmin, до двух строк.
5. Локальные состояния — таблица DATA_MODEL.md §3.4. Нажатие срабатывает на `pointerup`, если палец не ушёл дальше 20 px от точки касания (иначе это свайп — отмена). Кнопка с `confirm`: `press` отправляется после непрерывного удержания 800 мс (круговой индикатор), отпускание раньше — отмена без отправки. Пока кнопка в `pending`, повторные нажатия на неё игнорируются.
6. Результат: успех → `ok` 300 мс; ошибка → `error` 1500 мс и тост `level: "error"` на 4 с с заголовком по коду: `UNKNOWN_ACTION` → `Action is not defined on the PC`; `ACTION_FAILED` → `Action failed on the PC`; `RATE_LIMITED` → `Too fast`; `TIMEOUT` → `PC did not respond`; `OFFLINE` → `Not connected`; прочее → `message`.
7. `conn !== "online"` → все кнопки с `actionId` в состоянии `disabled`; кнопки `navigate` работают.
8. Проверка безопасности в коде: в сообщении `press` формируется объект ровно из трёх полей `t`, `id`, `actionId`.
**Приёмка:** тесты проходят; ручная: нажатие `Play` в dry-run даёт зелёную рамку и строку `DRY media-play media` в `.tmp\headless.log`; кнопка `Lock` требует удержания; `Mute` после нажатия меняет иконку на `volume-x` и цвет.
**Проверка:**
```
npm run typecheck -w client; npm test -w client; npm run build -w client
(Get-ChildItem client\src -Recurse -Include *.ts,*.tsx | Select-String -Pattern 'import \* as .* from .lucide-react.' | Measure-Object).Count
```
(последняя команда печатает `0`)

### T4.6 — Индикатор соединения и тосты
**Цель:** пользователь всегда видит, есть ли связь.
**Файлы:** `client/src/deck/ConnBadge.tsx`, `deck/Toasts.tsx`, `screens/DeckScreen.tsx`, `styles.css`.
**Шаги:**
1. `ConnBadge` в правом верхнем углу `.deck-inner`, `pointer-events: none`: `online` → ничего не показывается; `connecting` → серый текст `Connecting…`; `offline` → плашка `#7f1d1d` с текстом `PC offline — retrying`. Первые 3 с после потери связи плашка не показывается (защита от мигания при коротких обрывах).
2. `Toasts`: стек сверху по центру, максимум 3 одновременно (старые вытесняются), цветная левая полоса по уровню (цвета — DATA_MODEL.md §6), `title` жирный, `body` одной строкой. Тап по тосту закрывает его. Таймер `expireToasts` раз в 500 мс.
3. Выход из сопряжения: пятикратный тап за 3 с в левом верхнем углу экрана (невидимая зона 12vmin × 12vmin поверх сетки, не перехватывает одиночные касания — считает `pointerdown` в фазе capture на `window` по координатам) открывает диалог `Unpair this device?` с кнопками `Unpair` / `Cancel`. `Unpair` → `controller.unpair()`.
**Приёмка:** сборка проходит. Ручная: остановка headless → через ~3 с плашка `PC offline — retrying`, кнопки действий полупрозрачны; запуск headless снова → плашка исчезает ≤ 16 с без перезагрузки страницы; пятикратный тап → диалог, `Unpair` → экран сопряжения.
**Проверка:**
```
npm run typecheck -w client; npm run build -w client
```

---

## M5 — Данные и виджеты

### T5.1 — POST /api/notify
**Цель:** webhook уведомлений по PROTOCOL.md §5–§6.
**Файлы:** `agent/core/src/server/http.rs`, `server/api_auth.rs`, `agent/core/tests/http_api.rs`.
**Шаги:**
1. `api_auth.rs`: извлечение `Authorization: Bearer`, сравнение с `config.apiToken` за постоянное время; лимитеры по IP: 60 запросов/мин суммарно на авторизованные эндпоинты; 10 ответов 401/мин → далее 429 в течение 60 с. Все ошибки — тело `HttpError`.
2. `POST /api/notify`: `Content-Type` начинается с `application/json` → разбор `NotifyReq`, проверка лимитов длины (превышение → 400 `BAD_MESSAGE`, без обрезки), пустой `title` → 400; иначе (text/plain или без типа) — тело как UTF-8, `trim`, пусто → 400, обрезка до 120 символов → `title`. Успех → `push_notification`, ответ 202 `{"id": "<id>"}`.
3. Тело > 16384 байт → 413 с `HttpError PAYLOAD_TOO_LARGE`.
4. Тесты: без токена → 401; с неверным → 401; JSON-фикстура `http-notify-req.json` → 202 и уведомление первым в секции; text/plain `hello` → 202, `title == "hello"`, `level == "info"`; `title` из 121 символа → 400; 21 уведомление подряд → в секции 20, первое по времени вытеснено; 61-й запрос за минуту → 429; тело 20 КБ → 413; токен в query (`?token=`) без заголовка → 401.
**Приёмка:** тесты проходят; curl из PowerShell работает.
**Проверка:**
```
cargo test -p decka-core --test http_api
```
затем H-BUILD; H-START; H-API; H-TOKEN; затем
```
curl.exe -s -X POST http://127.0.0.1:19247/api/notify -H "Authorization: Bearer $api" -H "Content-Type: application/json" --data-binary "@shared/fixtures/http-notify-req.json"
curl.exe -s -o NUL -w "%{http_code}" -X POST http://127.0.0.1:19247/api/notify -d "x"
(node tools/ws-smoke.mjs state 127.0.0.1:19247 $tok | ConvertFrom-Json).notifications[0].title
```
затем H-STOP. Ожидается: `{"id":"1"}`, `401`, `Deploy finished`.

### T5.2 — API статусов и переменных, TTL
**Цель:** `POST /api/status`, `DELETE /api/status/{id}`, `PUT`/`DELETE /api/vars/{name}`.
**Файлы:** `agent/core/src/server/http.rs`, `statuses.rs`, тесты в `tests/http_api.rs`.
**Шаги:**
1. `statuses.rs`: менеджер статусов — карта `id → (StatusItem, Option<Instant> /* истекает */)`, правила upsert из PROTOCOL.md §5 (значения по умолчанию для `label`, сохранение `label`/`group`, очистка `text`, запрет изменения `source: "check"` → 400). После каждого изменения — `store.update_statuses`.
2. Фоновая задача раз в 1 с: элементы с истёкшим TTL → `state: "unknown"`, `text: "stale"`, TTL снимается.
3. `DELETE /api/status/{id}`: нет элемента или он `source: "check"` → 404.
4. `PUT /api/vars/{name}`: `name` по regex `VarName` (иначе 400), тело `VarReq`, строка > 200 символов → 400. `DELETE` — 404, если переменной нет.
5. Тесты: upsert по фикстуре `http-status-req.json` → элемент в секции с `source: "push"`; повторный upsert без `label` сохраняет `label`; без `text` — очищает; `ttlSec: 1` → через ≤ 3 с `state == "unknown"`, `text == "stale"`; `DELETE` → элемент исчез, повторный → 404; `PUT /api/vars/mic.muted` `{"value": true}` → в секции `vars`; `PUT /api/vars/Bad Name` → 400; значение-объект → 400; один и тот же upsert дважды подряд (без TTL) → `rev` вырос ровно на 1.

   Для последнего теста `updatedAt` не должен меняться, если остальные поля не изменились, — иначе рассылка «только при изменении» бессмысленна. Реализуй: `updatedAt` обновляется только когда изменилось хотя бы одно из `state`, `text`, `label`, `group`.
**Приёмка:** тесты проходят.
**Проверка:**
```
cargo test -p decka-core --test http_api
```
затем H-BUILD; H-START; H-API; H-TOKEN; затем
```
curl.exe -s -X POST http://127.0.0.1:19247/api/status -H "Authorization: Bearer $api" -H "Content-Type: application/json" --data-binary "@shared/fixtures/http-status-req.json"
curl.exe -s -X PUT http://127.0.0.1:19247/api/vars/mic.muted -H "Authorization: Bearer $api" -H "Content-Type: application/json" --data-binary "@shared/fixtures/http-var-req.json"
$s = node tools/ws-smoke.mjs state 127.0.0.1:19247 $tok | ConvertFrom-Json; $s.statuses[0].id; $s.vars.'mic.muted'
```
затем H-STOP. Ожидается: `{"ok":true}` дважды, `claude-decka`, `True`.

### T5.3 — Сборщик sys
**Цель:** секция `sys` по PROTOCOL.md §3 (`SysStats`).
**Файлы:** `agent/core/src/collectors/mod.rs`, `collectors/sys.rs`.
**Шаги:**
1. `cargo add -p decka-core sysinfo@0.40.3`.
2. Выделенный поток ОС (`std::thread`), цикл с периодом `collectors.sys.intervalSec`. Если `store.receiver_count() == 0` — итерация пропускается (нет клиентов — нет нагрузки). Флаг остановки — `AtomicBool`, проверяется каждую итерацию; сон дробится по 200 мс.
3. Значения: `cpu` — общая загрузка, округление до целого, зажатие 0..100; `mem` — в МБ (деление на 1 048 576, округление); `disks` — только несъёмные, `mount` без завершающего `\` (`C:`), в ГБ (округление), сортировка по `mount`; `net` — сумма по интерфейсам, (дельта байт × 8 / 1000 / интервал), округление до целого Kbps; `uptimeSec` — округление вниз до кратного 60.
4. Округления обязательны: они уменьшают число рассылок (сравнение в `Store::set`).
5. `collectors.sys.enabled == false` → поток не запускается, секция `sys` остаётся `null`.
6. Чистая функция `round_stats(raw) -> SysStats` с юнит-тестами: 12.6 → 13; 100.4 → 100; uptime 125 → 120; диск `C:\` → `C:`.
7. Сборщики перезапускаются при `save_config` (остановить старые, запустить с новым конфигом).
**Приёмка:** тесты проходят; при подключённом клиенте в секции `sys` появляются числа.
**Проверка:**
```
cargo test -p decka-core collectors::
```
затем H-BUILD; H-START; H-TOKEN; затем
```
node tools/ws-smoke.mjs watch 127.0.0.1:19247 $tok 6 | Select-String '"section":"sys"' | Measure-Object | Select-Object -ExpandProperty Count
```
затем H-STOP. Ожидается число ≥ 1.

### T5.4 — Сборщик HTTP-проверок
**Цель:** `collectors.checks` → элементы секции `statuses` по DATA_MODEL.md §4.
**Файлы:** `agent/core/src/collectors/checks.rs`, `agent/core/tests/checks.rs`, `agent/core/Cargo.toml`.
**Шаги:**
1. Перенеси `reqwest@0.13.5` из dev-зависимостей в обычные.
2. Один общий `reqwest::Client`: `redirect(Policy::none())`, `user_agent("decka/<version>")`. На каждую проверку — задача tokio с `tokio::time::interval(intervalSec)`; первая проверка сразу при старте; таймаут запроса `timeoutSec`.
3. При старте сборщика каждая проверка сразу публикуется как `state: "unknown"` (без `text`), `source: "check"`, `label`, `group` из конфига.
4. Результат → статус по правилам DATA_MODEL.md §4 (абзац «Правила HTTP-проверок»). Публикация через менеджер статусов из T5.2 (отдельный внутренний метод `set_check`, минующий запрет на `source: "check"`). `updatedAt` меняется только при изменении `state` или `text`; чтобы время ответа не вызывало рассылку каждую проверку, миллисекунды в `text` округляются до 10.
5. При `save_config`: остановить все задачи, удалить статусы проверок, которых больше нет в конфиге, запустить заново.
6. Тест: локальный axum-сервер в тесте с маршрутами `/ok` (200), `/fail` (500), `/slow` (ответ через 3 с); три проверки с `intervalSec: 10`, `timeoutSec: 1` → в течение 4 с статусы: `ok` с текстом, начинающимся на `200 · `; `error` с `text == "500"`; `error` с `text == "timeout"`. Проверка на закрытый порт → `text == "unreachable"`.
**Приёмка:** тесты проходят.
**Проверка:**
```
cargo test -p decka-core --test checks
```

### T5.5 — Виджеты clock и sys
**Цель:** отображение по DATA_MODEL.md §6.
**Файлы:** `client/src/deck/widgets/Clock.tsx`, `widgets/Sys.tsx`, `widgets/format.ts`, `deck/Tile.tsx`, `client/test/format.test.ts`, `styles.css`.
**Шаги:**
1. `format.ts`: `formatRate(kbps: number): string` (`< 1000` → `"<n> Kbps"`, иначе `"<n.n> Mbps"`), `percent(used, total): number` (целое, `total = 0` → 0), `formatClock(d: Date, seconds: boolean): string` (`HH:MM` / `HH:MM:SS`, 24 часа, ведущие нули), `formatDate(d: Date): string` (`toLocaleDateString(undefined, { weekday: "short", day: "numeric", month: "short" })`). Тесты на первые три.
2. `Clock`: обновление раз в 1 с при `seconds: true`, иначе раз в 10 с; время — 9vmin при 2×1 и 5.5vmin при 1×1, `font-variant-numeric: tabular-nums`, вес 300; дата — 2.6vmin, цвет `#9ca3af`, только при `w === 2`.
3. `Sys`: строки «подпись — полоса — значение». Полоса: высота 1.2vmin, фон `#26262c`, заполнение `#2563eb`, при значении ≥ 85% — `#d97706`, ≥ 95% — `#dc2626`. 2×1: `CPU`, `RAM`. 2×2: дополнительно `<mount>` первого диска и строка сети `↓ <rx>  ↑ <tx>`. `sys === null` → по центру `—`.
4. Подключи оба виджета в диспетчере `Tile`.
**Приёмка:** тесты и сборка проходят. Ручная: часы идут; полосы CPU/RAM меняются при живом агенте.
**Проверка:**
```
npm run typecheck -w client; npm test -w client; npm run build -w client
```

### T5.6 — Виджеты statuses и status
**Цель:** отображение статусов сервисов, деплоев и ИИ-сессий.
**Файлы:** `client/src/deck/widgets/Statuses.tsx`, `widgets/StatusTile.tsx`, `widgets/statusOrder.ts`, `client/test/statusOrder.test.ts`, `deck/Tile.tsx`, `styles.css`.
**Шаги:**
1. `statusOrder.ts`: `sortStatuses(items: StatusItem[], group?: string): StatusItem[]` — фильтр по `group` (если задан), порядок состояний `error, warn, running, unknown, ok`, внутри — по `label` (`localeCompare`). `STATE_COLOR` — цвета DATA_MODEL.md §6. Тест на порядок и фильтр.
2. `Statuses`: заголовок `title` (2.4vmin, `#9ca3af`, верхний регистр), строки высотой 5vmin: точка 1.6vmin цвета состояния, `label` (2.8vmin, многоточие), справа `text` (2.4vmin, `#9ca3af`, многоточие, максимум 45% ширины). Число помещающихся строк вычисляется по высоте контейнера (`ResizeObserver`); если элементов больше — последняя строка `+N more`. Пустой список → по центру `No statuses yet`.
3. Точка состояния `running` пульсирует (CSS-анимация opacity 1↔0.5, 1.6 с).
4. `StatusTile`: фон — цвет состояния с прозрачностью 0.25, рамка 0.4vmin цвета состояния, `label` по центру (3vmin), `text` снизу (2.2vmin). Нет элемента → `unknown`, `label = statusId`.
**Приёмка:** тесты и сборка проходят. Ручная: после команды `curl` из T5.2 на странице `Dev` в списке появляется `Claude: Decka` с синей пульсирующей точкой и текстом `editing files`.
**Проверка:**
```
npm run typecheck -w client; npm test -w client; npm run build -w client
```

### T5.7 — Виджет notifications, тосты уведомлений, dismiss
**Цель:** лента и всплывающие уведомления по DATA_MODEL.md §6.
**Файлы:** `client/src/deck/widgets/Notifications.tsx`, `widgets/relTime.ts`, `client/src/store/store.ts`, `client/test/notifs.test.ts`, `deck/Tile.tsx`.
**Шаги:**
1. `relTime.ts`: `relTime(ts, now): string` — `< 60 с` → `now`; `< 60 мин` → `<n>m`; `< 24 ч` → `<n>h`; иначе `<n>d`. Тест.
2. В `applyPatch` для секции `notifications`: вычислить id, которых не было в предыдущем значении секции; для каждого — `pushToast` на 8 с (`level`, `title`, `body`). Уведомления из `welcome` тостов не порождают. Тест: патч с одним новым id → один тост; тот же патч повторно → 0 новых; `welcome` с 3 уведомлениями → 0 тостов.
3. Экспортируй из `store.ts` событие для M8: `onAttention(fn: (level: NotifLevel) => void)` — вызывается при каждом новом тосте уведомления.
4. `Notifications`: строки — левая полоса 0.6vmin цвета уровня, `title` (2.8vmin, жирный, многоточие), `body` (2.4vmin, `#9ca3af`, одна строка), справа `source · relTime` (2vmin). Обновление относительного времени раз в 30 с. Список прокручивается вертикально (`touch-action: pan-y` только внутри виджета; свайп страниц внутри виджета не срабатывает). Тап по строке → `controller.dismiss(id)`. Пусто → `No notifications`.
**Приёмка:** тесты и сборка проходят. Ручная: команда `curl` из T5.1 → тост на 8 с и строка в ленте; тап по строке убирает её (в том числе во второй вкладке браузера).
**Проверка:**
```
npm run typecheck -w client; npm test -w client; npm run build -w client
```

---

## M6 — Tauri-оболочка и окно настроек

### T6.1 — Каркас Tauri-приложения
**Цель:** `Decka.exe` запускает ядро и открывает пустое окно настроек.
**Файлы:** `agent/package.json`, `agent/tsconfig.json`, `agent/vite.config.ts`, `agent/index.html`, `agent/src/main.tsx`, `agent/src/App.tsx`, `agent/src/styles.css`, `agent/src-tauri/Cargo.toml`, `build.rs`, `tauri.conf.json`, `capabilities/default.json`, `src/main.rs`, `src/lib.rs`, `icons/*`, `tools/make-icon.ps1`, `tools/icon-source.png`; правки корневых `package.json` и `Cargo.toml`.
**Шаги:**
1. Корневой `package.json`: `"workspaces": ["shared", "client", "agent"]`; scripts: `"build:ui": "npm run build -w client && npm run build -w agent"`, `"tauri": "npm run tauri -w agent --"`. Корневой `Cargo.toml`: добавить `"agent/src-tauri"` в `members`.
2. `agent/package.json`: `"name": "@decka/agent"`, private, `"type": "module"`, scripts: `"dev": "vite"`, `"build": "tsc --noEmit -p . && vite build"`, `"typecheck": "tsc --noEmit -p ."`, `"test": "vitest run --passWithNoTests"`, `"tauri": "tauri"`; зависимость `"@decka/shared": "*"`. Установить: `npm install -w agent react@19.3.1 react-dom@19.3.1 @tauri-apps/api@2.12.4 lucide-react@1.73.0`; `npm install -w agent -D @tauri-apps/cli@2.12.4 vite@8.3.8 @vitejs/plugin-react@6.3.0 typescript@7.0.2 vitest@5.1.0 @types/react @types/react-dom`.
3. `agent/vite.config.ts`: порт 5174, `strictPort: true`, `clearScreen: false`, `build.outDir: "dist"`, `build.target: "chrome105"`.
4. `tools/make-icon.ps1`: скрипт на `System.Drawing`, создающий `tools/icon-source.png` 1024×1024 — фон `#0b0b0f`, белая буква `D` шрифтом Segoe UI Bold 640 px по центру. Запусти его, затем из каталога `agent`: `npx tauri icon ..\tools\icon-source.png` (создаёт `src-tauri/icons`).
5. `agent/src-tauri/Cargo.toml`: `name = "decka-agent"`; зависимости: `tauri = { version = "2.12.5", features = ["tray-icon"] }`, `tauri-plugin-autostart = "2.7.0"`, `tauri-plugin-single-instance = "2.5.5"`, `decka-core = { path = "../core" }`, `serde`, `serde_json`, `tokio` (как в core); build-зависимость `tauri-build = "2.7.4"`. `build.rs` — стандартный `tauri_build::build()`.
6. `tauri.conf.json`: `productName: "Decka"`, `version: "0.1.0"`, `identifier: "app.decka.agent"`; `build: { devUrl: "http://localhost:5174", beforeDevCommand: "npm run dev", frontendDist: "../dist" }` (`beforeBuildCommand` не задавать — UI собирается явно командой `npm run build:ui`); `app.windows`: одно окно `label: "settings"`, `title: "Decka"`, `width: 980`, `height: 680`, `minWidth: 820`, `minHeight: 560`, `visible: false`; `app.security.csp`: `"default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; connect-src ipc: http://ipc.localhost"`; `bundle: { active: true, targets: ["nsis"], icon: [список файлов из icons], resources: { "../../client/dist": "client" } }`.
7. `capabilities/default.json`: `identifier: "default"`, `windows: ["settings"]`, `permissions: ["core:default"]`.
8. `src/lib.rs`: `pub fn run()` — `tauri::Builder::default()`, в `setup`: путь клиента `app.path().resolve("client", BaseDirectory::Resource)`; `AgentOptions { config_dir: None, client_dist: Some(путь), port_override: None, runner: Arc::new(WindowsRunner::new()), fixed_pair_code: None }`; `tauri::async_runtime::block_on(Agent::start(opts))`; результат положить в `app.manage(...)`; показать окно `settings` (`show` + `set_focus`). Ошибка старта агента не должна ронять приложение: окно открывается, ошибка видна в статусе (T6.4). `main.rs`: `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` и вызов `run()`.
9. `agent/src/App.tsx`: текст `Decka settings`. `agent/tsconfig.json`: extends base, `include: ["src", "test"]`, `types: ["vite/client"]`.
10. В начале `run()` инициализируй `tracing_subscriber` (уровень из env `DECKA_LOG`, по умолчанию `info`).
11. Если каталог ресурса `client` не найден (`resolve` вернул ошибку или пути нет) — передай `client_dist: None`, не падай.
**Важно для всех ручных проверок M6:** debug-сборка, запущенная напрямую из `target\debug`, загружает в окно `devUrl`, а не собранный интерфейс. Интерфейс окна проверяй командой `npm run tauri dev` (из корня), сетевую часть — как в блоке «Проверка». Перед `cargo build -p decka-agent` и `npm run tauri dev` каталог `client\dist` должен существовать (`npm run build -w client`), иначе сборка упадёт на отсутствующем ресурсе.
**Приёмка:** `cargo build -p decka-agent` проходит; при запуске собранного exe `/api/health` отвечает на порту 9247 и `/` отдаёт клиент.
**Проверка:**
```
npm install; npm run build:ui; cargo build -p decka-agent
$env:DECKA_CONFIG_DIR = "$PWD\.tmp\cfg-agent"; $a = Start-Process -FilePath target\debug\decka-agent.exe -PassThru; Start-Sleep 5; curl.exe -s http://127.0.0.1:9247/api/health; curl.exe -s -o NUL -w "%{http_code}" http://127.0.0.1:9247/; Stop-Process -Id $a.Id -Force
```
Ожидается JSON health и `200`. Переменная `DECKA_CONFIG_DIR` уводит конфигурацию в `.tmp`, реальный `%APPDATA%\Decka` не затрагивается. Если порт 9247 занят установленной копией Decka — закрой её на время проверки.

### T6.2 — Трей, сворачивание в трей, один экземпляр, автозапуск
**Цель:** поведение фонового приложения.
**Файлы:** `agent/src-tauri/src/lib.rs`, `src/tray.rs`.
**Шаги:**
1. Плагин single-instance (регистрируется первым): при запуске второго экземпляра — показать и сфокусировать окно `settings` первого.
2. Трей: иконка приложения, подсказка `Decka`, меню: `Open settings`, `Pair a device` (открывает окно и шлёт событие `decka://navigate` с payload `"pairing"`), разделитель, `Quit`. Левый клик по иконке — показать окно.
3. Закрытие окна (`CloseRequested`) → `prevent_close()` и `hide()`. Выход — только через `Quit` (вызывает `Agent::shutdown`, затем `app.exit(0)`).
4. Автозапуск: плагин `tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--minimized"]))`. При старте и после `save_config` состояние автозапуска синхронизируется с `config.autostart` (включить/выключить, если отличается).
5. Аргумент `--minimized` → окно при старте не показывается. Первый запуск (файла `config.json` не существовало до старта) → окно показывается всегда.
**Приёмка:** сборка проходит. Ручная: закрытие окна оставляет процесс и иконку в трее; второй запуск exe не создаёт второй процесс `decka-agent`; `Quit` завершает процесс.
**Проверка:**
```
cargo build -p decka-agent
$env:DECKA_CONFIG_DIR = "$PWD\.tmp\cfg-agent"; $a = Start-Process -FilePath target\debug\decka-agent.exe -ArgumentList '--minimized' -PassThru; Start-Sleep 4; Start-Process -FilePath target\debug\decka-agent.exe; Start-Sleep 3; (Get-Process decka-agent | Measure-Object).Count; Get-Process decka-agent | Stop-Process -Force
```
Ожидается `1`.

### T6.3 — IPC-команды и QR
**Цель:** все команды PROTOCOL.md §9.
**Файлы:** `agent/src-tauri/src/commands.rs`, `lib.rs`, `agent/core/src/qr.rs`, `agent/core/src/agent.rs`, `agent/src/ipc.ts`.
**Шаги:**
1. `cargo add -p decka-core qrcode@0.14.1 --no-default-features --features svg`. `qr.rs`: `pair_url(primary_ip, ips, port, code, agent_name) -> String` по PROTOCOL.md §8 (percent-encoding имени) и `qr_svg(text) -> String` (уровень коррекции M, тёмный `#000000`, светлый `#ffffff`, тихая зона включена). Юнит-тест: URL для `("192.168.1.5", ["192.168.1.5","10.0.0.2"], 9247, "K7M2QX9D", "My PC")` равен `http://192.168.1.5:9247/#c=K7M2QX9D&h=192.168.1.5,10.0.0.2&p=9247&n=My%20PC`; SVG начинается с `<?xml` или `<svg`.
2. `Agent::start_pairing` теперь заполняет `url` и `qrSvg`. Нет LAN-адресов (или `bind = 127.0.0.1`) → в URL используется `127.0.0.1`, `h=127.0.0.1`.
3. Допиши в `Agent` недостающие методы: `config()`, `save_config(ConfigFile) -> SaveResult` (валидация, атомарная запись, перезапуск сборщиков; порт/bind применяются только после `restart_server`), `regenerate_api_token()`, `subscribe_status() -> tokio::sync::watch::Receiver<AgentStatus>` (обновляется при подключении/отключении клиента, успешном сопряжении, изменении USB-состояния, рестарте сервера).
4. `commands.rs`: по одной `#[tauri::command]` на каждую запись `IpcCommands`; имена и формы аргументов/результатов — точно как в PROTOCOL.md §9. `open_config_dir` — `explorer.exe <configDir>` через `Command`. Все команды зарегистрированы в `invoke_handler`.
5. Задача в `setup`: слушает `subscribe_status()` и эмитит событие `decka://status-changed`.
6. `agent/src/ipc.ts`: типизированная обёртка `call<K extends keyof IpcCommands>(cmd: K, args?): Promise<ReturnType<IpcCommands[K]>>` поверх `invoke` и `onStatus(fn)` поверх `listen`.
7. Интеграционный тест `agent/core/tests/save_config.rs`: агент с одной HTTP-проверкой `a` → `save_config` с проверкой `b` вместо `a` → в течение 2 с в секции `statuses` нет `a` и есть `b` (`unknown` или результат); `save_config` с `collectors.sys.enabled: false` → секция `sys` равна `null` в течение 3 с; `save_config` с `port: 80` → `ok: false`, файл на диске не изменился.
**Приёмка:** тесты qr и save_config проходят; сборка агента проходит; число команд в `generate_handler!` равно 16.
**Проверка:**
```
cargo test -p decka-core qr::; cargo test -p decka-core --test save_config; cargo build -p decka-agent; npm run typecheck -w agent
(Select-String -Path agent\src-tauri\src\commands.rs -Pattern '#\[tauri::command\]').Count
```
(последняя команда печатает `16`)

### T6.4 — Оболочка окна настроек и вкладка Status
**Цель:** навигация по вкладкам и экран состояния.
**Файлы:** `agent/src/App.tsx`, `agent/src/tabs/StatusTab.tsx`, `agent/src/ui/*.tsx` (Button, Field, Tabs, Banner), `agent/src/styles.css`.
**Шаги:**
1. Вкладки слева вертикально, в порядке: `Status`, `Pairing`, `Layout`, `Actions`, `Integrations`, `Settings`. Событие `decka://navigate` переключает вкладку. Стили — обычный CSS, системный шрифт, светлая/тёмная тема через `prefers-color-scheme`.
2. `StatusTab`: данные из `get_status` + подписка `onStatus`. Блоки: (а) `Server` — `Listening on <bind>:<port>` зелёным или красный баннер с `listenError` и кнопкой `Retry` (`restart_server`); (б) `Addresses` — список `lanIps` в виде `http://<ip>:<port>`; (в) `Connected devices` — таблица (name, remote, connected since) или `No devices connected`; (г) `USB` — `Disabled` / `adb not found` / список устройств с отметкой `forwarded`; (д) `Config folder` — путь и кнопка `Open` (`open_config_dir`).
3. Если `bind = 0.0.0.0`: информационный блок `Windows Firewall` с текстом `If the phone can't connect over Wi-Fi, allow Decka in Windows Firewall (private networks) or run this command as administrator:` и полем с командой `netsh advfirewall firewall add rule name="Decka" dir=in action=allow protocol=TCP localport=<port> profile=private` и кнопкой `Copy`.
**Приёмка:** typecheck и сборка проходят. Ручная: вкладка показывает `Listening on 0.0.0.0:9247`; при открытии клиента в браузере устройство появляется в таблице без перезагрузки окна.
**Проверка:**
```
npm run typecheck -w agent; npm run build:ui; cargo build -p decka-agent
```

### T6.5 — Вкладка Pairing
**Цель:** QR, код, список устройств, отзыв.
**Файлы:** `agent/src/tabs/PairingTab.tsx`.
**Шаги:**
1. Кнопка `Pair a new device` → `start_pairing`. Показать: QR (вставка `qrSvg` через `dangerouslySetInnerHTML`, белая подложка с отступом 16 px, размер 260 px), код в виде `XXXX-XXXX` крупно, URL мелко, обратный отсчёт до `expiresAt`. По истечении — блок скрывается, текст `Code expired`. Кнопка `Cancel` → `cancel_pairing`.
2. Пояснение под QR: `Scan with the Decka app or the phone camera. Phone and PC must be on the same Wi-Fi, or connected by USB with USB mode enabled.`
3. При событии статуса, в котором появилось новое устройство, блок QR скрывается, показывается `Paired: <name>`.
4. Таблица `Paired devices` из `list_devices` (name, created, last seen, online-точка), кнопка `Revoke` с подтверждением `Revoke <name>? It will need to pair again.` → `revoke_device`, обновление списка.
5. Уход с вкладки при активном коде → `cancel_pairing`.
**Приёмка:** сборка проходит. Ручная: QR из окна сканируется камерой телефона и открывает клиент, который сопрягается; `Revoke` возвращает клиент на экран сопряжения в течение 2 с.
**Проверка:**
```
npm run typecheck -w agent; npm run build -w agent
```

### T6.6 — Вкладка Actions (редактор действий)
**Цель:** создание, изменение, удаление и проверка действий без ручной правки JSON.
**Файлы:** `agent/src/tabs/ActionsTab.tsx`, `agent/src/tabs/actionForm.tsx`, `agent/src/tabs/keys.ts`, `agent/test/keys.test.ts`.
**Шаги:**
1. Слева список действий (name, тип, id), сортировка по `name`; кнопка `New action`. Справа форма выбранного.
2. Поля формы: `ID` (только при создании; проверка regex `Id` и уникальности), `Name`, `Type` (select из 5 типов), далее по типу: `hotkey` — поле `Keys`, ввод строкой через `+` (`ctrl+shift+m`), разбор в `keys.ts`: `parseKeys(s): string[] | null` (trim, нижний регистр, разделитель `+`, каждая часть из набора DATA_MODEL.md §2.2, без повторов, 1..5) и `formatKeys`; `media` — select `op`; `launch` — `Program`, `Arguments` (по одному аргументу в строке textarea), `Working directory`, чекбокс `Hide window`; `open_url` — `URL`; `system` — select `op`. Общее необязательное поле `Toggle variable`.
3. Кнопки: `Save` (собирает полный `ActionsFile`, вызывает `save_actions`; `issues` показываются под формой списком `path: message`), `Test` (доступна только для сохранённого действия; `test_action`; результат `OK` или текст ошибки), `Delete` (подтверждение; если действие используется кнопками раскладки — в тексте подтверждения перечислить их `id`).
4. Для типа `launch` под формой предупреждение: `This program will run on your PC whenever the button is pressed on a paired phone.`
5. Тесты `keys.ts`: `"Ctrl + Shift + M"` → `["ctrl","shift","m"]`; `"ctrl+ctrl"` → `null`; `"ctrl+c+x+v+b+n"` → `null`; `"hyper+a"` → `null`.
**Приёмка:** тесты, typecheck и сборка проходят. Ручная: созданное действие `launch` → `charmap.exe`, кнопка `Test` открывает «Таблицу символов»; файл `%APPDATA%\Decka\actions.json` содержит новое действие.
**Проверка:**
```
npm run typecheck -w agent; npm test -w agent; npm run build -w agent
```

### T6.7 — Вкладка Layout (редактор раскладки)
**Цель:** редактирование страниц и сетки 5×3.
**Файлы:** `agent/src/tabs/LayoutTab.tsx`, `agent/src/tabs/layoutOps.ts`, `agent/src/tabs/itemForm.tsx`, `agent/test/layoutOps.test.ts`.
**Шаги:**
1. Сверху — вкладки страниц и кнопки `Add page`, `Rename`, `Delete page` (последнюю страницу удалить нельзя; при удалении страницы кнопки `navigate` на неё блокируют сохранение обычной валидацией), стрелки `←`/`→` для изменения порядка.
2. По центру — сетка 5×3 (ячейка 120×90 px): занятые ячейки показывают элемент (вид, label/иконка-имя), пустые — `+`. Клик по пустой ячейке → создать элемент в ней (форма с выбором `kind`, размер по умолчанию: button 1×1, clock 2×1, sys 2×1, status 1×1, statuses 2×2, notifications 2×2; если не помещается или пересекается — сообщение `Not enough free space here`). Клик по элементу → форма справа. Drag-and-drop не делать.
3. Форма элемента: общие `X`, `Y`, `W`, `H` (числовые select в допустимых диапазонах вида); `button`: `Label`, `Icon` (текстовое поле + выпадающий список 60 имён из DATA_MODEL.md §3.6 с префиксом `lucide:`), `Color` (`<input type="color">` + кнопка `Clear`), переключатель `Runs action` / `Opens page` → select действия (из `get_actions`, подпись `name (id)`) либо select страницы; чекбокс `Hold to confirm`; блок `State binding`: `Variable`, список состояний (значение + label/icon/color), кнопки `Add state`/`Remove`. `clock`: чекбокс `Show seconds`. `statuses`: `Title`, `Group`. `status`: `Status ID`. Кнопка `Delete item`.
4. `id` нового элемента генерируется: `<b|w>-<6 случайных символов [a-z0-9]>`.
5. Блок `Display` под сеткой: поля `DisplaySettings` (числа, чекбоксы, два поля времени).
6. `layoutOps.ts` — чистые функции: `findFreeSpot`, `fits(page, item)`, `addItem`, `updateItem`, `removeItem`, `movePage`. Перед сохранением вызывается `validateLayout` из `@decka/shared`; замечания показываются списком и подсвечивают элементы красной рамкой; кнопка `Save` при замечаниях заблокирована. `Save` → `save_layout`; серверные `issues` показываются так же. Кнопка `Revert` перечитывает `get_layout`.
7. Тесты `layoutOps`: `fits` ловит пересечение и выход за границы; `addItem` в занятую ячейку возвращает ошибку; `movePage` меняет порядок.
**Приёмка:** тесты, typecheck и сборка проходят. Ручная: добавленная кнопка появляется на подключённом клиенте в течение 1 с после `Save` без перезагрузки клиента.
**Проверка:**
```
npm run typecheck -w agent; npm test -w agent; npm run build -w agent
```

### T6.8 — Вкладки Integrations и Settings
**Цель:** API-токен, примеры webhook, HTTP-проверки, параметры сервера.
**Файлы:** `agent/src/tabs/IntegrationsTab.tsx`, `agent/src/tabs/SettingsTab.tsx`.
**Шаги:**
1. `Integrations`: (а) `API token` — скрытое поле с кнопками `Show`, `Copy`, `Regenerate` (подтверждение `Existing scripts will stop working.`); (б) `Send a notification` — готовые команды с подставленными адресом `http://127.0.0.1:<port>` и токеном, с кнопкой `Copy` у каждой: вариант `curl.exe` (JSON), вариант `curl.exe` (text/plain), вариант PowerShell `Invoke-RestMethod`; (в) `Update a status` — команда `curl.exe` для `POST /api/status` с примером тела; (г) кнопка `Send test notification` (`send_test_notification`); (д) `HTTP checks` — таблица `collectors.checks` с добавлением/редактированием/удалением (поля `HttpCheck`), сохранение через `save_config`.
2. `Settings`: `PC name` (`agentName`); `Port`; `Network access` — радио `Wi-Fi and USB (0.0.0.0)` / `USB only (127.0.0.1)`; чекбокс `Start with Windows`; чекбокс `System stats widget` и `Interval, s`; блок `USB mode`: чекбокс `Forward port over USB (adb)`, поле `adb path` (по умолчанию `adb`), пояснение `Requires Android platform-tools and USB debugging enabled on the phone.` Кнопка `Save` → `save_config`; `issues` — под формой. Если изменились `port` или `bind` — после успешного сохранения баннер `Restart the server to apply` с кнопкой `Restart now` (`restart_server`).
**Приёмка:** typecheck и сборка проходят. Ручная: `Send test notification` даёт тост на клиенте; скопированная команда `curl.exe`, вставленная в PowerShell, возвращает `{"id":"…"}`; смена порта на 9248 + `Restart now` → `curl.exe http://127.0.0.1:9248/api/health` отвечает.
**Проверка:**
```
npm run typecheck -w agent; npm run build:ui; cargo build -p decka-agent
```

---

## M7 — Android (Capacitor) и USB-режим

Требуется: Android Studio (SDK + platform-tools), JDK той версии, которую требует установленный Capacitor (см. его документацию по окружению), переменные `JAVA_HOME` и `ANDROID_HOME`. Нет окружения → запись в `BLOCKERS.md`, переход к M8 (M8 от M7 не зависит), возврат к M7 после устранения.

### T7.1 — Android-проект Capacitor и debug-APK
**Цель:** клиент собирается в APK и запускается на Android 10.
**Файлы:** `client/capacitor.config.ts`, `client/android/**` (генерируется и коммитится), `client/package.json`.
**Шаги:**
1. `npm install -w client @capacitor/core@8.6.2 @capacitor/android@8.6.2`; `npm install -w client -D @capacitor/cli@8.6.2`. Если `@capacitor/android`/`@capacitor/cli` версии 8.6.2 не существует — ставь последнюю 8.x, запись в коммите.
2. `capacitor.config.ts`: `appId: "app.decka.client"`, `appName: "Decka"`, `webDir: "dist"`, `server: { androidScheme: "http", cleartext: true }`. Причина `androidScheme: "http"`: со схемой `https` WebView блокирует `ws://` как смешанный контент (ARCHITECTURE.md §6.5).
3. Из каталога `client`: `npm run build; npx cap add android; npx cap sync android`.
4. `android/app/src/main/AndroidManifest.xml`: у `<application>` — `android:usesCleartextTraffic="true"`; у главной `<activity>` — `android:screenOrientation="sensorLandscape"`.
5. `minSdkVersion` не менять (значение шаблона Capacitor; оно ≤ 29).
6. Scripts в `client/package.json`: `"android:sync": "npm run build && cap sync android"`, `"android:apk": "cd android && gradlew.bat assembleDebug"`.
**Приёмка:** APK существует по пути `client\android\app\build\outputs\apk\debug\app-debug.apk`.
**Проверка:**
```
npm run android:sync -w client; npm run android:apk -w client; Test-Path client\android\app\build\outputs\apk\debug\app-debug.apk
```
Ручная (при подключённом телефоне): `adb install -r client\android\app\build\outputs\apk\debug\app-debug.apk` → приложение открывается в альбомной ориентации на экране сопряжения.

### T7.2 — Полноэкранный режим, keep-awake, определение платформы
**Цель:** панель занимает весь экран и не гаснет.
**Файлы:** `client/android/app/src/main/java/**/MainActivity.java`, `client/src/platform/native.ts`, `client/src/main.tsx`.
**Шаги:**
1. `npm install -w client @capacitor-community/keep-awake@8.0.1`.
2. `MainActivity.java` (наследник `BridgeActivity`): в `onWindowFocusChanged(hasFocus)` при `hasFocus` — `WindowInsetsControllerCompat` для окна: `hide(WindowInsetsCompat.Type.systemBars())` и `setSystemBarsBehavior(BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE)`. В `onCreate` после `super.onCreate` — `getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)` (страховка на случай сбоя плагина).
3. `native.ts`: `isNative()` → `Capacitor.isNativePlatform()`; `clientInfo()` в native: `app: "capacitor"`, `name` — `"Android"` (без дополнительных плагинов); `keepAwake()` — в native `KeepAwake.keepAwake()`, в вебе — `navigator.wakeLock?.request("screen")` в try/catch с повторным запросом на `visibilitychange` (в незащищённом контексте API отсутствует — молча ничего не делать).
4. `main.tsx`: вызвать `keepAwake()` при старте.
5. `buildCandidates` уже учитывает `native` (T4.2) — убедись, что `controller.ts` передаёт `isNative()`.
**Приёмка:** APK собирается. Ручная на S9: нет статус-бара и навигационной панели; экран не гаснет 5 минут без касаний при системном таймауте экрана 30 с.
**Проверка:**
```
npm run typecheck -w client; npm run android:sync -w client; npm run android:apk -w client
```

### T7.3 — Сканер QR в приложении
**Цель:** сопряжение сканированием QR из окна настроек.
**Файлы:** `client/src/platform/scan.ts`, `client/src/screens/PairScreen.tsx`, манифест Android (если требует README плагина).
**Шаги:**
1. `npm install -w client @capacitor/barcode-scanner@3.1.3`; выполнить шаги установки для Android из README плагина (разрешение камеры и т. п.).
2. `scan.ts`: `scanQr(): Promise<string | null>` — вызов сканера с ограничением на формат QR; отмена пользователем или ошибка → `null`.
3. `PairScreen` в native: главная кнопка `Scan QR code` над ручными полями. Результат → `parsePairUrl`; `null` → текст `This is not a Decka QR code`; иначе `controller.pairWith(hosts, port, code, agentName)`. В вебе кнопки нет.
**Приёмка:** APK собирается. Ручная на S9: сканирование QR из вкладки Pairing приводит к сетке кнопок менее чем за 5 с.
**Проверка:**
```
npm run typecheck -w client; npm run android:sync -w client; npm run android:apk -w client
```

### T7.4 — USB-режим в агенте (adb reverse)
**Цель:** агент сам пробрасывает порт на подключённые по USB устройства (ARCHITECTURE.md §5.5).
**Файлы:** `agent/core/src/usb.rs`, `agent.rs`, юнит-тесты в `usb.rs`.
**Шаги:**
1. `parse_devices(stdout: &str) -> Vec<(String /*serial*/, String /*state*/)>`: пропустить строку `List of devices attached`, пустые строки и строки, начинающиеся с `*`; остальные — два поля через пробельные символы.
2. Цикл (задача tokio), активен при `usb.enabled`: каждые 5 с — `<adbPath> devices` (`tokio::process::Command`, флаг `CREATE_NO_WINDOW`, таймаут 4 с). Ошибка запуска (файл не найден) → `status.usb.adbFound = false`, `lastError`, следующая попытка через 30 с. Для каждого устройства в состоянии `device`, которого нет в множестве «проброшенных»: `<adbPath> -s <serial> reverse tcp:<port> tcp:<port>`; успех → добавить в множество. Исчезнувшие устройства удаляются из множества. Раз в 60 с множество очищается (самовосстановление после сна ПК и перезапуска adb-сервера). Устройства в состоянии `unauthorized` попадают в статус с `reversed: false`, `lastError = "device <serial> is unauthorized: accept the USB debugging prompt on the phone"`.
3. `<port>` — фактический порт сервера. При `restart_server` и `save_config` цикл перезапускается с очисткой множества.
4. Каждое изменение списка/флагов обновляет `AgentStatus.usb` и канал статуса.
5. adb в состав Decka не входит и не скачивается автоматически.
6. Тесты `parse_devices`: вывод с двумя устройствами (`device` и `unauthorized`); пустой список; вывод со строками `* daemon not running…` и `* daemon started successfully`.
**Приёмка:** тесты проходят. Ручная (телефон с отладкой по USB, `adb` в PATH, `USB mode` включён): `adb reverse --list` показывает `tcp:9247 tcp:9247` в течение 10 с после подключения кабеля.
**Проверка:**
```
cargo test -p decka-core usb::; cargo build -p decka-agent
```

### T7.5 — USB в клиенте и сквозная проверка на устройстве
**Цель:** клиент работает по USB без Wi-Fi и показывает канал.
**Файлы:** `client/src/deck/ConnBadge.tsx`, `docs/MANUAL_CHECKS.md`.
**Шаги:**
1. `ConnBadge`: при `online` и `via === "usb"` — в течение 4 с после подключения показать серый текст `USB`, при `via === "lan"` — `Wi-Fi`; затем скрыть (защита от выгорания — постоянных надписей нет).
2. Ручные проверки на S9 (результаты `PASS`/`FAIL` — в `docs/MANUAL_CHECKS.md`):
   - (а) Wi-Fi выключен на телефоне, кабель подключён, USB-режим включён: приложение подключается ≤ 10 с, индикатор `USB`, кнопка `Play` работает.
   - (б) В настройках `USB only (127.0.0.1)` + `Restart now`: с другого устройства сети `http://<ip ПК>:9247/api/health` недоступен, телефон по USB работает.
   - (в) Кабель отключён, Wi-Fi включён, режим `Wi-Fi and USB`: переподключение ≤ 20 с, индикатор `Wi-Fi`.
   - (г) Сон ПК на 1 минуту и пробуждение: клиент восстанавливает связь ≤ 30 с после входа в Windows без действий пользователя.
**Приёмка:** сборка проходит; четыре пункта записаны в `MANUAL_CHECKS.md` с результатом либо с пометкой `PENDING (owner)`.
**Проверка:**
```
npm run typecheck -w client; npm run build -w client; Test-Path docs\MANUAL_CHECKS.md
```

---

## M8 — Защита AMOLED от выгорания

Спецификация поведения — ARCHITECTURE.md §5.6. Все параметры берутся из `state.layout.display`.

### T8.1 — Трекер бездействия и затемнение
**Цель:** режимы `active` → `dim` и «проглатывание» пробуждающего касания.
**Файлы:** `client/src/display/idle.ts`, `display/DimOverlay.tsx`, `screens/DeckScreen.tsx`, `client/test/idle.test.ts`.
**Шаги:**
1. `idle.ts`: чистая функция `computeMode(input: { now: number; lastActivity: number; display: DisplaySettings; offlineSince: number | null; nowMinutes: number }): "active" | "dim" | "sleep"` и `inNight(night, nowMinutes): boolean` (интервал через полночь поддерживается; `from === to` → всегда `false`). Правила, в порядке приоритета:
   1. `idle = now - lastActivity`; `idle < 10 с` → `active`;
   2. `offlineSince !== null` и `now - offlineSince ≥ 30 с` → `sleep`;
   3. `night.enabled` и `inNight` → `sleep`;
   4. `sleepAfterSec > 0` и `idle ≥ sleepAfterSec` → `sleep`;
   5. `dimAfterSec > 0` и `idle ≥ dimAfterSec` → `dim`;
   6. иначе `active`.
2. Хук `useDisplayMode()`: хранит `lastActivity`, пересчитывает режим раз в 1 с; `pointerdown` на `window` (фаза capture) обновляет `lastActivity`; подписка `onAttention` (T5.7): уровни `warning` и `error` обновляют `lastActivity` (экран просыпается).
3. `DimOverlay`: фиксированный слой поверх всего, чёрный, `opacity = 1 - dimLevel` в режиме `dim`, `0` в `active`; переход 600 мс. В режиме `dim` слой имеет `pointer-events: auto` и перехватывает первое касание (оно только будит экран, кнопка под пальцем не нажимается); в `active` — `pointer-events: none`.
4. Режим `sleep` в этой задаче отображается так же, как `dim` (заставка — T8.3).
5. Тесты `computeMode`: каждое из 6 правил; ночь `23:00–07:00` в 23:30 и в 06:59 → `sleep`, в 07:00 → не `sleep`; `dimAfterSec: 0` отключает затемнение; активность 5 с назад ночью → `active`.
**Приёмка:** тесты и сборка проходят. Ручная: с `dimAfterSec: 10` экран темнеет через 10 с; первое касание только возвращает яркость, второе нажимает кнопку.
**Проверка:**
```
npm run typecheck -w client; npm test -w client; npm run build -w client
```

### T8.2 — Pixel shift
**Цель:** медленное смещение всего интерфейса.
**Файлы:** `client/src/display/pixelShift.ts`, `screens/DeckScreen.tsx`, `client/test/pixelShift.test.ts`.
**Шаги:**
1. `pixelShift.ts`: `shiftOffset(step: number): { x: number; y: number }` — `x = round(4 · sin(2π · step / 16))`, `y = round(4 · sin(4π · step / 16))`; `step` увеличивается на 1 каждые 60 с (от момента запуска приложения).
2. При `display.pixelShift === true` к `.deck-inner` применяется `transform: translate(<x>px, <y>px)` с `transition: transform 2s ease`; при `false` — без смещения.
3. Отступ 6 px у `.deck-inner` (T4.4) гарантирует, что содержимое не обрезается: проверь, что максимум `|x|`, `|y|` равен 4.
4. Тесты: для `step` 0..15 `|x| ≤ 4` и `|y| ≤ 4`; `shiftOffset(0)` = `{0, 0}`; `shiftOffset(16)` = `shiftOffset(0)`; среди 16 шагов не менее 8 различных смещений.
**Приёмка:** тесты и сборка проходят.
**Проверка:**
```
npm run typecheck -w client; npm test -w client; npm run build -w client
```

### T8.3 — Заставка (sleep)
**Цель:** в режиме `sleep` экран почти полностью чёрный.
**Файлы:** `client/src/display/Screensaver.tsx`, `display/saverPosition.ts`, `screens/DeckScreen.tsx`, `client/test/saverPosition.test.ts`.
**Шаги:**
1. В режиме `sleep` сетка скрыта (`visibility: hidden`), поверх — `Screensaver` на чёрном фоне `#000`, перехватывает касания (первое касание только будит).
2. Содержимое: время `HH:MM` (12vmin, вес 200, цвет `#4b5563`); под ним — при отсутствии связи текст `PC offline` (3vmin, `#4b5563`); при наличии статусов в состоянии `error` — строка `● <N>` (3vmin, `#7f1d1d`).
3. Позиция блока меняется каждые 30 с: `saverPosition(seed: number, viewport: {w, h}, box: {w, h}): {x, y}` — детерминированная псевдослучайная функция от `seed` (номер 30-секундного интервала), результат гарантирует, что блок целиком в пределах экрана с отступом 16 px. Смена позиции — плавное исчезновение 1 с, перемещение, появление 1 с.
4. Новое уведомление уровня `info`/`success` в режиме `sleep` не будит экран (тост не показывается, запись остаётся в ленте); `warning`/`error` — будят (T8.1 п. 2) и показывают тост.
5. Тесты `saverPosition`: для 100 значений `seed` блок внутри границ; не менее 20 различных позиций.
**Приёмка:** тесты и сборка проходят. Ручная: с `sleepAfterSec: 30` через 30 с остаются только серые часы, меняющие место; остановка агента на минуту → заставка с `PC offline`; `curl` с `"level":"error"` будит экран.
**Проверка:**
```
npm run typecheck -w client; npm test -w client; npm run build -w client
```

---

## M9 — Выпуск

### T9.1 — Веб-манифест и иконки клиента
**Цель:** клиент добавляется на домашний экран из браузера.
**Файлы:** `client/public/manifest.webmanifest`, `client/public/icon-192.png`, `icon-512.png`, `client/index.html`, `tools/make-icon.ps1`.
**Шаги:**
1. Расширь `tools/make-icon.ps1`: дополнительно сохранять уменьшенные копии 192×192 и 512×512 в `client/public/`.
2. `manifest.webmanifest`: `name: "Decka"`, `short_name: "Decka"`, `start_url: "./"`, `display: "fullscreen"`, `orientation: "landscape"`, `background_color: "#000000"`, `theme_color: "#000000"`, две иконки.
3. `index.html`: `<link rel="manifest" href="./manifest.webmanifest">`, `<link rel="icon" href="./icon-192.png">`.
4. Service worker не добавлять (DECISIONS.md D-17).
5. Иконку Android-приложения в MVP не менять (остаётся иконка шаблона Capacitor). Добавь в `docs/MANUAL_CHECKS.md` строку `T9.1: Android launcher icon is the Capacitor template icon — replace before public release (owner)`.
**Приёмка:** после сборки `client/dist/manifest.webmanifest` существует и является валидным JSON.
**Проверка:**
```
npm run build -w client; Get-Content client\dist\manifest.webmanifest -Raw | ConvertFrom-Json | Select-Object -ExpandProperty display
```
(печатает `fullscreen`)

### T9.2 — README, рецепты, благодарности
**Цель:** документация для пользователей GitHub.
**Файлы:** `README.md`, `docs/RECIPES.md`, `NOTICE.md`.
**Шаги:**
1. `README.md` (английский), разделы в этом порядке: What is Decka (1 абзац + место под скриншот `docs/img/hero.png` — файл не создавать); Why (три отличия из ARCHITECTURE.md §2.3, по 2 предложения); Quick start (установка агента, сопряжение по QR, установка APK); USB mode (включение отладки по USB, platform-tools, чекбокс); Sending notifications and statuses (2 команды curl + ссылка на RECIPES); Security model (сжатый пересказ ARCHITECTURE.md §6: 6–8 пунктов, включая честное «трафик по Wi-Fi не шифруется»); Configuration files (ссылки на DATA_MODEL.md); Building from source (команды из T6.1, T7.1, T9.3); Troubleshooting (брандмауэр, `adb unauthorized`, порт занят, телефон не видит ПК, гибернация вместо сна); Battery and burn-in notes (RISKS.md R-05, R-06 кратко); License (MIT).
2. `docs/RECIPES.md`: готовые примеры с плейсхолдерами `<TOKEN>` и `<PC>`: (а) curl JSON и text/plain; (б) PowerShell `Invoke-RestMethod`; (в) статус деплоя: `running` перед деплоем, `ok`/`error` после (скрипт-обёртка PowerShell из 6–10 строк); (г) Claude Code hooks: статус `running` при начале работы и `ok` + уведомление при остановке — перед написанием сверить формат hooks с актуальной документацией Claude Code, в рецепте дать ссылку на неё; (д) GitHub Actions: шаг с `curl` (с оговоркой, что раннер должен иметь сетевой доступ к ПК — self-hosted раннер или VPN); (е) переключение состояния кнопки через `PUT /api/vars`.
3. `NOTICE.md`: раздел Acknowledgements — проекты, чьи идеи использованы (таблица из ARCHITECTURE.md §2.1 с лицензиями и формулировкой «ideas only, no code copied»), и лицензии основных зависимостей (Tauri — MIT/Apache-2.0, React — MIT, Capacitor — MIT, Lucide — ISC, axum — MIT, enigo — MIT).
**Приёмка:** три файла существуют; в README есть все 11 заголовков; в тексте нет слов `TODO`/`TBD`.
**Проверка:**
```
(Select-String -Path README.md -Pattern '^## ').Count; (Select-String -Path README.md,docs\RECIPES.md,NOTICE.md -Pattern 'TODO|TBD').Count
```
(печатает число ≥ 11 и `0`)

### T9.3 — CI и сборка релиза
**Цель:** автоматические проверки и воспроизводимые команды сборки.
**Файлы:** `.github/workflows/ci.yml`, `docs/TESTING.md`, корневой `package.json`.
**Шаги:**
1. `ci.yml`: триггеры `push` и `pull_request`; одна job на `windows-latest`: checkout, setup Node 22 с кэшем npm, установка stable Rust, `npm ci`, `npm run typecheck`, `npm test`, `npm run build -w client`, `cargo test -p decka-core`, `cargo build -p decka-agent` (перед ним `npm run build:ui`). Версии actions — последние мажорные на момент написания (проверь на github.com/actions).
2. Корневые scripts: `"release:agent": "npm run build:ui && npm run tauri -- build"`, `"release:apk": "npm run android:sync -w client && npm run android:apk -w client"`.
3. `docs/TESTING.md`: (а) как запускать все автоматические тесты; (б) как пользоваться headless и `ws-smoke.mjs` (макросы из этого файла); (в) полный ручной чек-лист выпуска — перенеси все пункты «Ручная» из задач T3.3–T8.3 в виде таблицы «№ / шаг / ожидаемый результат».
4. Собери установщик: `npm run release:agent`.
**Приёмка:** установщик существует в `target\release\bundle\nsis\`; все автоматические проверки зелёные локально.
**Проверка:**
```
npm run typecheck; npm test; cargo test -p decka-core; npm run release:agent; (Get-ChildItem target\release\bundle\nsis\*.exe | Measure-Object).Count
```
(последняя команда печатает число ≥ 1)

### T9.4 — Итоговая приёмка MVP
**Цель:** подтвердить критерии готовности MVP (ARCHITECTURE.md §9).
**Файлы:** `docs/MANUAL_CHECKS.md`.
**Шаги:**
1. Установи агент из установщика NSIS на чистый профиль конфигурации (переименуй `%APPDATA%\Decka` в `Decka.bak` на время проверки; после — верни).
2. Пройди чек-лист `docs/TESTING.md` целиком на Galaxy S9; результат каждого пункта — в `MANUAL_CHECKS.md`.
3. Проверка безопасности: создай `.tmp\evil.json` с содержимым `{"t":"press","id":1,"actionId":"x","program":"calc.exe","args":["/c","calc"]}`; `node tools/ws-smoke.mjs raw 127.0.0.1:9247 .tmp\evil.json` (без токена) → ошибка `UNAUTHORIZED`, код выхода 1. Добавь в `agent/core/tests/press.rs` тест `extra_fields_are_ignored`: после авторизации `press` с лишними полями `program`/`args` и существующим `actionId` → `RecordingRunner` получил ровно один вызов с этим `actionId`; с несуществующим `actionId` → `UNKNOWN_ACTION` и ноль вызовов.
4. Поиск по коду: в сетевом коде клиента (`client/src/net/`) нет строк `program`, `hotkey`, `open_url`, `eval(`, `new Function(`.
**Приёмка:** все автоматические проверки проходят; `MANUAL_CHECKS.md` не содержит `FAIL` (допустимо `PENDING (owner)`).
**Проверка:**
```
cargo test -p decka-core --test press; npm run typecheck; npm test
(Select-String -Path client\src\net\*.ts -Pattern 'program|hotkey|open_url|eval\(|new Function\(').Count
(Select-String -Path docs\MANUAL_CHECKS.md -Pattern 'FAIL').Count
```
(две последние команды печатают `0`)

---

## M10 — Доработки по отзыву владельца (2026-11-14)

Отзыв: иконки на телефоне не отображаются; серые плашки под кнопками и виджетами лишние и занимают место; кнопки слишком большие; нужно удобно выбирать программу для действия на ПК. Выполнять по порядку.

### T10.1 — Иконки не отображаются
**Цель:** все 60 иконок рисуются в браузере и в APK.
**Файлы:** `client/src/deck/Icon.tsx`, `client/src/deck/icons.ts`, `client/test/icons.test.tsx`.
**Шаги:**
1. Воспроизведи: открой клиент в настольном Chrome (headless + `client/dist`) и в APK на подключённом телефоне; для APK — `chrome://inspect` на ПК (debug-сборка Capacitor разрешает отладку WebView). Найди причину по DOM: есть ли `<svg>`, его `width`/`height`, `stroke`, вычисленный `color`.
2. Исправь причину. Требования к результату: размер иконки задаётся числом пикселей, вычисленным из размера клавиши (T10.2), а не процентной строкой; `stroke` = `currentColor`; цвет задаётся через CSS `color` на контейнере.
3. Тест `icons.test.tsx` (`react-dom/server` → `renderToString`): для каждого из 60 имён DATA_MODEL.md §3.6 `Icon` даёт строку с `<svg`; ни одно имя, кроме неизвестного, не падает на запасной `square` (сравни разметку с разметкой `square`); `emoji:🚀` даёт `<span`.
4. Причину опиши в описании коммита одной фразой.
**Приёмка:** тест проходит; на снимке экрана телефона (`adb exec-out screencap -p > .tmp\icons.png`) видны иконки кнопок первой страницы.
**Проверка:**
```
npm test -w client; npm run android:sync -w client; npm run android:apk -w client; adb install -r client\android\app\build\outputs\apk\debug\app-debug.apk
```

### T10.2 — Новый вид сетки: без плашек, компактные клавиши
**Цель:** лёгкий интерфейс на чёрном фоне, клавиши меньше ячейки.
**Файлы:** `client/src/deck/Grid.tsx`, `Tile.tsx`, `Button.tsx`, `deck/widgets/*.tsx`, `styles.css`.
**Шаги:**
1. Сетка: `gap: 2vmin`, `padding: 4vmin` (ARCHITECTURE.md §7). Ячейки и раскладка 5×3 не меняются.
2. `Tile`: фон и радиус убрать (прозрачный). Виджеты рисуются прямо на чёрном фоне; заголовки виджетов (`statuses.title`) — 2.2vmin, `#6b7280`, верхний регистр, `letter-spacing: 0.08em`.
3. Кнопка = ячейка-контейнер (вся ячейка — зона касания) + внутри по центру «клавиша»: квадрат со стороной `s = round(0.62 × min(ширина, высота ячейки))` (`ResizeObserver` на ячейке), радиус `0.22 × s`.
   - Без `color`: клавиша прозрачная, иконка `#e5e5e5`.
   - С `color`: фон клавиши — `color` с непрозрачностью 0.16, иконка — `color` (если `color` темнее `#404040` по яркости `0.2126R+0.7152G+0.0722B < 64` — иконка `#e5e5e5`).
   - Иконка: `round(0.5 × s)` px, `strokeWidth 1.75`.
   - Подпись: под клавишей, отступ `0.06 × s`, размер `max(11px, 0.16 × s)`, `#9ca3af`, одна строка с многоточием, ширина не больше ячейки. Кнопка без иконки: подпись внутри клавиши по центру, размер `0.2 × s`, `#e5e5e5`, до двух строк.
4. Состояния (DATA_MODEL.md §3.4) применяются к клавише, не к ячейке: `pressed` — фон клавиши `rgba(255,255,255,0.10)` (или `color` с 0.30) и масштаб 0.92; `pending` — рамка 2px `#2563eb` пульсирует; `ok` — рамка 2px `#16a34a` 300 мс; `error` — рамка 2px `#dc2626` 1500 мс; `confirming` — круговой прогресс по контуру клавиши; `disabled` — непрозрачность 0.35.
5. Виджет `status` (1×1) рисуется как клавиша того же размера: фон — цвет состояния с непрозрачностью 0.16, точка состояния и `label` под клавишей как подпись кнопки.
6. Ничего не светится постоянно ярче `#e5e5e5`; крупных светлых заливок нет (защита AMOLED).
**Приёмка:** typecheck, тесты и сборка проходят; снимок экрана телефона показывает первую страницу: чёрный фон без серых плашек, клавиши заметно меньше ячеек, иконки видны; кнопки `Lock` и `Mute` (с `color`) — с цветной полупрозрачной подложкой. Снимок сохрани в `.tmp\grid.png` и приложи путь в отчёте.
**Проверка:**
```
npm run typecheck -w client; npm test -w client; npm run android:sync -w client; npm run android:apk -w client; adb install -r client\android\app\build\outputs\apk\debug\app-debug.apk
```

### T10.3 — Выбор программы кнопкой «Browse…» в окне настроек
**Цель:** пользователь выбирает exe в проводнике, а не вписывает путь вручную.
**Файлы:** `agent/src-tauri/Cargo.toml`, `src/lib.rs`, `capabilities/default.json`, `agent/package.json`, `agent/src/tabs/actionForm.tsx`.
**Шаги:**
1. `cargo add -p decka-agent tauri-plugin-dialog`; `npm install -w agent @tauri-apps/plugin-dialog` (последние 2.x). Зарегистрируй плагин; в `capabilities/default.json` добавь `dialog:allow-open`.
2. В форме `launch` рядом с полем `Program` — кнопка `Browse…`: диалог открытия одного файла, фильтр `Programs` → `exe`. Выбранный путь → поле `Program`; если поле `Name` пустое — подставить имя файла без `.exe`; поле `Working directory` — папка файла, если оно пустое.
3. Ярлыки `.lnk` не поддерживаются (`Command` их не запускает) — фильтр только `exe`.
4. В редакторе раскладки (T6.7) в форме кнопки рядом с select действия — кнопка `New action…`: переключает на вкладку `Actions` с открытой формой нового действия (событие `decka://navigate` с payload `"actions:new"`).
**Приёмка:** typecheck и сборка проходят. Ручная (`npm run tauri dev`): `Browse…` → выбрать `C:\Windows\System32\charmap.exe` → поля заполнены (`Name` = `charmap`) → `Save` → `Test` открывает «Таблицу символов».
**Проверка:**
```
npm run typecheck -w agent; npm run build:ui; cargo build -p decka-agent
```

### T10.5 — Настраиваемая сетка (по умолчанию 7×4) и обводка клавиш
**Цель:** размер сетки задаётся в окне настроек на ПК; клавиши обведены тонкой линией.
**Файлы:** `shared/schemas/layout.schema.json`, `shared/src/layout.ts`, `shared/src/validate.ts`, `shared/fixtures/layout.default.json`, `agent/core/src/model.rs`, `agent/core/src/validate.rs`, `client/src/deck/Grid.tsx`, `client/src/deck/Button.tsx`, `client/src/styles.css`, `agent/src/tabs/LayoutTab.tsx`, `agent/src/tabs/layoutOps.ts`, тесты.
**Шаги:**
1. Перенеси дословно из DATA_MODEL.md: тип `grid` (§3.1), правила 1, 3, 4 и абзац о смене размера (§3.2), изменённые места схемы (§5: `ItemBase` x/y/w/h, `Page.items.maxItems: 40`, `ClientLayout.grid`), новый `layout.default.json` (§3.5). В Rust `grid.cols`/`grid.rows` — `u8`, проверки по тем же диапазонам. Все проверки границ используют `grid.cols`/`grid.rows`, ни одной константы 5 или 3 в коде проверки раскладки не остаётся.
2. Фикстуры `shared/fixtures/invalid/*` не менять: у них собственная сетка 5×3, она по-прежнему допустима, и ожидаемые ошибки остаются. Фикстура `s2c-welcome.json` тоже остаётся (5×3 допустима).
3. Добавь тесты (TS и Rust): `grid` 8×5 допустима; 9×4 и 7×1 — ошибка; элемент `x: 6` при `cols: 7` допустим, при `cols: 6` — ошибка с путём на элемент; дефолтная раскладка + дефолтные действия → 0 замечаний.
4. Клиент: `Grid` берёт `repeat(cols, 1fr)` × `repeat(rows, 1fr)` из `layout.grid`. Множитель клавиши — `0.7` (было 0.62 в T10.2).
5. Обводка клавиши (DATA_MODEL состояния не меняются): без `color` — `1px solid rgba(255,255,255,0.16)`; с `color` — `1px solid` цвет `color` с непрозрачностью 0.5. Рамки состояний `pending`/`ok`/`error` (2 px) заменяют обводку на время состояния. Виджет `status` — обводка цвета состояния с непрозрачностью 0.5. Остальные виджеты без обводки.
6. Окно настроек, вкладка `Layout`, блок над сеткой: `Grid` — два select: `Columns` 3..8 и `Rows` 2..5. Сетка редактора рисуется по этим значениям (ячейка по-прежнему 120×90 px; при 8 столбцах редактор прокручивается горизонтально). Элементы за пределами новой сетки подсвечиваются красным и перечислены в замечаниях, `Save` заблокирован (обычная валидация). `findFreeSpot`/`fits` учитывают текущий размер сетки; тест `layoutOps` на сетке 7×4.
7. Существующий у пользователя `layout.json` (5×3) не мигрируется — он валиден и показывается как есть. Новый размер применяется только после `Save`.
**Приёмка:** все тесты проходят; на снимке экрана телефона (`.tmp\grid74.png`) свежего агента с конфигурацией по умолчанию — 7 столбцов × 4 строки, у клавиш тонкая обводка; в `npm run tauri dev` смена `Columns` на 5 и `Save` перестраивает сетку на телефоне ≤ 1 с (если элементы помещаются).
**Проверка:**
```
npm run typecheck; npm test; cargo test -p decka-core; npm run android:sync -w client; npm run android:apk -w client; adb install -r client\android\app\build\outputs\apk\debug\app-debug.apk
```

### T10.4 — Установка агента для владельца
Выполняется последней в M10 (после T10.5).
**Цель:** у владельца работает настоящее приложение-агент, а не headless.
**Шаги:**
1. Останови все процессы `decka-headless` и `decka-agent`.
2. `npm run release:agent`; путь установщика — в отчёт.
3. Установку не запускай (это действие владельца). В отчёте дай владельцу три шага: запустить установщик → найти иконку Decka в трее → вкладка `Pairing` → `Pair a new device` и сканировать QR приложением на телефоне.
**Приёмка:** установщик собран; ни одного процесса `decka-headless` не запущено.
**Проверка:**
```
npm run release:agent; (Get-Process decka-headless -ErrorAction SilentlyContinue | Measure-Object).Count; Get-ChildItem target\release\bundle\nsis\*.exe | Select-Object -ExpandProperty FullName
```

---

## M11 — Контекстные страницы и «Сейчас играет» (решение владельца 2026-11-14)

Спецификация: PROTOCOL.md §3 (`FocusInfo`, `MediaInfo`), §4.5 («Секции M11»), §7, §9 (новые IPC и `AgentStatus.focusApp`); DATA_MODEL.md §3.1, §3.2 п. 9–10, §4 (`collectors.focus`, `collectors.media`), §5, §6.1, §6.2, §7; DECISIONS.md D-32..D-34; ARCHITECTURE.md §6.4 S16. Все новые поля необязательные: существующие `layout.json`, `config.json` и фикстуры остаются валидными без изменений.

### T11.1 — Общий контракт M11 и шаблоны
**Цель:** типы, схемы, фикстуры, валидация и файлы шаблонов в `/shared`.
**Файлы:** `shared/src/protocol.ts`, `layout.ts`, `config.ts`, `ipc.ts`, `templates.ts`, `validate.ts`, `index.ts`; `shared/schemas/*.json`; `shared/fixtures/s2c-patch-focus.json`, `s2c-patch-media.json`, `layout.context.json`; `shared/templates/{vscode,browser,zoom,spotify}.json`; тесты.
**Шаги:**
1. Типы и схемы — дословно из мест, перечисленных в шапке M11.
2. Фикстуры: две новые из PROTOCOL.md §4.5 (добавь их в явный список `schemas.test.ts` → `ServerMsg`); `layout.context.json` — копия `layout.default.json`, где у страницы `dev` добавлено `"apps": ["code.exe"]`, а на страницу `main` в свободные клетки `x: 3..5, y: 3` добавлен элемент `{ "id": "w-media", "kind": "media", "x": 3, "y": 3, "w": 3, "h": 1, "prevAction": "media-prev", "playAction": "media-play", "nextAction": "media-next" }` (схема → `LayoutFile`; `validateLayout` с действиями по умолчанию → 0 замечаний).
3. `validateLayout`: правила DATA_MODEL.md §3.2 п. 9–10. Тесты: одно приложение на двух страницах → путь `/pages/1/apps/0`; `apps` у первой страницы → ошибка; `media` 1×1 → ошибка; `media` 3×3 → ошибка; `media` с несуществующим `playAction` → ошибка.
4. Шаблоны — четыре файла строго по таблицам DATA_MODEL.md §7. Тест `templates.test.ts` для каждого шаблона: действия валидны по `actions.schema.json#/$defs/Action`; собрать временную раскладку `{ grid: minGrid, display: <из layout.default>, pages: [ { id: "main", name: "Main", items: [] }, { id: <template.id>, name, apps, items: <items, где "$home" → "main"> } ] }` → схема `LayoutFile` валидна и `validateLayout(раскладка, { version: 1, actions: template.actions })` → 0 замечаний; все id действий начинаются с `tpl-<id>-`.
**Приёмка:** все тесты `shared` проходят.
**Проверка:**
```
npm run typecheck -w shared; npm test -w shared
```

### T11.2 — Ядро: модель, секции, конфиг, шаблоны
**Цель:** Rust понимает новые поля; `Agent::add_template`.
**Файлы:** `agent/core/src/model.rs`, `proto.rs`, `validate.rs`, `state.rs`, `config.rs`, `agent.rs`, `templates.rs`, тесты `tests/fixtures.rs`, `tests/templates.rs`.
**Шаги:**
1. Serde-типы для `FocusInfo`, `MediaInfo`, `Page.apps`, `DisplaySettings.contextPages`, `LayoutItem::Media`, `Collectors.focus`/`media` (`#[serde(default)]`, отсутствие = `enabled: true`). Новые фикстуры — в round-trip тест.
2. `Store`: секции `focus` и `media`, начальное значение `null`; входят в `snapshot()` и `welcome`.
3. `validate_layout`: правила п. 9–10 (те же пути, что в TS).
4. `templates.rs`: шаблоны встроены через `include_str!` из `shared/templates/*.json`; `Agent::get_templates()`, `Agent::add_template(id) -> SaveResult` строго по алгоритму DATA_MODEL.md §7.
5. Тесты `tests/templates.rs`: на конфигурации по умолчанию (7×4) каждый из 4 шаблонов добавляется (`ok: true`), страница и действия появились в файлах и в секции `layout`; повторное добавление `vscode` → ошибка про `code.exe`; на сетке 5×3 → ошибка `/grid`; при 10 страницах → `Page limit reached`; существующее действие с id `tpl-vscode-save` и другим содержимым не перезаписывается; `$home` заменён на `pages[0].id`.
**Приёмка:** `cargo test -p decka-core` зелёный.
**Проверка:**
```
cargo test -p decka-core
```

### T11.3 — Сборщик активного приложения
**Цель:** секция `focus`, `AgentStatus.focusApp`, IPC `list_window_apps`.
**Файлы:** `agent/core/src/collectors/focus.rs`, `collectors/mod.rs`, `agent.rs`, `agent/core/Cargo.toml`, `agent/src-tauri/src/commands.rs`, `agent/src/tabs/StatusTab.tsx`.
**Шаги:**
1. `cargo add -p decka-core windows@0.62.2` в `[target.'cfg(windows)'.dependencies]` с features, нужными для: `GetForegroundWindow`, `GetWindowThreadProcessId`, `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)`, `QueryFullProcessImageNameW`, `CloseHandle`, `EnumWindows`, `IsWindowVisible`, а также (для T11.4) `Media_Control`, `Storage_Streams`, `Foundation`. Точные имена features — по docs.rs/windows/0.62.2.
2. Выделенный поток ОС (как `sys`), опрос каждые 250 мс: имя exe активного окна → только имя файла, нижний регистр; неудача → `null`. Значения `decka-agent.exe` и `decka-headless.exe` игнорируются (остаётся предыдущее). Публикация в `store.set("focus", …)` только когда новое значение держится непрерывно ≥ 700 мс. Заголовок окна NEVER не читается (S16). `collectors.focus.enabled = false` → поток не запускается, секция `null`. Перезапуск при `save_config` через сохранённый `Handle`/флаг остановки (как у других сборщиков).
3. Чистая функция `stable_value` (вход: последовательность (время, значение), выход: опубликованное значение) с юнит-тестами: короткий «всплеск» 300 мс не публикуется; значение ≥ 700 мс публикуется; игнорируемые имена не меняют результат.
4. `AgentStatus.focusApp` = текущее опубликованное значение; изменение обновляет канал статуса.
5. IPC `list_window_apps` (PROTOCOL.md §9): `EnumWindows` + `IsWindowVisible` + имя exe процесса; без повторов, по алфавиту, без `decka-agent.exe`. Зарегистрировать команду.
6. Вкладка Status: строка `Active app: <focusApp | —>`, обновляется по событию статуса.
7. Не-Windows: сборщик и команда возвращают `null` / `[]` (крейт собирается в CI на любой ОС).
**Приёмка:** тесты проходят. Проверка: headless + `ws-smoke watch` 8 с, в течение которых исполнитель переключает фокус между двумя окнами (`Start-Process notepad; Start-Sleep 3; Start-Process charmap`) — в выводе есть `patch` секции `focus` с `notepad.exe` и с `charmap.exe`.
**Проверка:**
```
cargo test -p decka-core focus::; cargo build -p decka-agent
```
затем H-BUILD; H-START; H-TOKEN; затем
```
$w = Start-Job { param($t) node tools/ws-smoke.mjs watch 127.0.0.1:19247 $t 8 } -ArgumentList $tok -InitializationScript ([scriptblock]::Create("Set-Location '$PWD'")); Start-Sleep 1; Start-Process notepad; Start-Sleep 3; Start-Process charmap; Start-Sleep 5; Receive-Job $w -Wait | Select-String '"section":"focus"'; Get-Process notepad,charmap -ErrorAction SilentlyContinue | Stop-Process -Force
```
затем H-STOP. Ожидаются строки с `notepad.exe` и `charmap.exe`. Если окно не получило фокус из-за ограничений Windows на перехват фокуса — пункт в `MANUAL_CHECKS.md` как PENDING (owner), автоматическая часть задачи (тесты, сборка) остаётся обязательной.

### T11.4 — Сборщик «Сейчас играет»
**Цель:** секция `media` из системной медиасессии Windows.
**Файлы:** `agent/core/src/collectors/media.rs`, `collectors/mod.rs`.
**Шаги:**
1. Выделенный поток ОС; в начале потока — инициализация WinRT в многопоточном режиме (`RoInitialize(RO_INIT_MULTITHREADED)` или эквивалент из `windows` 0.62). Асинхронные операции WinRT ожидаются блокирующе (`.get()` или эквивалент).
2. `GlobalSystemMediaTransportControlsSessionManager::RequestAsync()` один раз; каждую 1 с — `GetCurrentSession()`. Нет сессии → `media = null`.
3. Из сессии: `SourceAppUserModelId` → `source`; `TryGetMediaPropertiesAsync` → `Title`, `Artist`, `AlbumTitle`; `GetPlaybackInfo().PlaybackStatus == Playing` → `playing`; `GetTimelineProperties()` → `Position`, `EndTime`, `LastUpdatedTime` → `positionSec` (вниз до целого), `durationSec` (если `EndTime > 0`), `positionAt` (unix ms из `LastUpdatedTime`). Пустой `Title` → `media = null`. Строки обрезаются до лимитов PROTOCOL.md §3 по границе символа.
4. Обложка: `Thumbnail` читается только при смене (`title`, `artist`, `album`); тип по сигнатуре (`89 50 4E 47` → png, `FF D8 FF` → jpeg, иначе без обложки); `data:image/<тип>;base64,...`; длиннее 65 536 символов → без `art`.
5. Публикация (чистая функция `should_publish(prev, next, now)` с юнит-тестами): изменились `title`/`artist`/`album`/`playing`/`durationSec`/`art`/`source` → публиковать; иначе публиковать только если `|next.positionSec − расчётная позиция prev| > 3` (перемотка). Тесты: обычный ход воспроизведения 10 с → ни одной публикации; перемотка на 30 с → публикация; пауза → публикация.
6. Любая ошибка WinRT → лог уровня `warn` не чаще раза в минуту, `media = null`, повтор на следующем тике. `collectors.media.enabled = false` → поток не запускается.
7. Не-Windows: секция остаётся `null`.
**Приёмка:** тесты проходят; ручная: при играющей музыке (Spotify, YouTube в браузере) `ws-smoke state` показывает `media.title`.
**Проверка:**
```
cargo test -p decka-core media::; cargo build -p decka-agent
```
Ручная: H-BUILD; H-START; H-TOKEN; `(node tools/ws-smoke.mjs state 127.0.0.1:19247 $tok | ConvertFrom-Json).media`; H-STOP. Без играющей музыки ожидается пусто (null); с музыкой — объект с `title`. Если музыку включить нельзя — PENDING (owner).

### T11.4b — Уменьшение обложки средствами Windows
**Цель:** обложка Spotify и других источников укладывается в лимит `art` (65 536 символов) без новых библиотек.
**Файлы:** `agent/core/src/collectors/media.rs`, `agent/core/Cargo.toml` (только features крейта `windows`).
**Шаги:**
1. Добавь feature `Graphics_Imaging` (и зависимые, если требует docs.rs/windows/0.62.2) к уже подключённому `windows`. Новые крейты не добавлять.
2. После чтения байтов `Thumbnail` (T11.4 шаг 4): поток в памяти (`InMemoryRandomAccessStream`) → `BitmapDecoder::CreateAsync` → если обе стороны ≤ 192 и исходный data URL ≤ 65 536 символов — использовать исходник как есть; иначе `BitmapEncoder::CreateForTranscodingAsync` или `CreateAsync` с `JpegEncoderId` в новый поток в памяти, `BitmapTransform`: масштаб с сохранением пропорций так, чтобы большая сторона = 192, интерполяция `Fant`; качество JPEG 0.8 (свойство `ImageQuality`, если доступно в API; иначе значение по умолчанию). Результат → `data:image/jpeg;base64,...`.
3. Результат всё ещё > 65 536 символов или любая ошибка декодирования → без `art` (как раньше), лог `warn` не чаще раза в минуту.
4. Преобразование выполняется только при смене трека (тот же кэш, что в T11.4 шаг 4) — не каждую секунду.
5. Юнит-тест (под `#[cfg(windows)]`): PNG 600×600, сгенерированный в тесте через тот же `BitmapEncoder` из сплошного цвета, → функция `shrink_art(bytes)` возвращает JPEG (сигнатура `FF D8 FF`), длина data URL < 65 536, декодированный размер 192×192.
**Приёмка:** тест проходит; ручная: при играющем Spotify `ws-smoke state` показывает `media.art`, начинающийся с `data:image/jpeg;base64,`.
**Проверка:**
```
cargo test -p decka-core media::; cargo build -p decka-agent
```

### T11.5 — Клиент: контекстное переключение страниц
**Цель:** алгоритм DATA_MODEL.md §6.1.
**Файлы:** `client/src/deck/context.ts`, `client/src/store/store.ts`, `client/src/screens/DeckScreen.tsx`, `client/src/deck/PageHint.tsx`, `client/test/context.test.ts`.
**Шаги:**
1. `context.ts`: чистая `decideContextPage({ pages, display, focusApp, pageIndex, homePageIndex, autoPageId, now, lastTouch })` строго по шагам 1–6 §6.1.
2. Тесты: каждое из 6 правил; касание 3 с назад откладывает, через 5 с — переключает; ручная навигация на контекстной странице отменяет автовозврат; `contextPages: false` — ничего; отсутствующая секция `focus` = `null`; страница с `apps` удалена из раскладки → возврат домой.
3. `store`: поля `homePageIndex`, `autoPageId`, `lastTouch`, `hint`; свайп и кнопка `navigate` вызывают ручную навигацию; `pointerdown` (capture на `window`) обновляет `lastTouch`. Вызов `decideContextPage` при `applyPatch` секций `focus`/`layout`, при `applyWelcome` и по таймеру раз в 1 с.
4. `PageHint`: подсказка по §6.1, `pointer-events: none`.
5. Переключение страницы не обновляет таймер бездействия (не будит экран).
**Приёмка:** тесты, typecheck и сборка проходят.
**Проверка:**
```
npm run typecheck -w client; npm test -w client; npm run build -w client
```

### T11.6 — Клиент: виджет media
**Цель:** отображение по DATA_MODEL.md §6.2.
**Файлы:** `client/src/deck/widgets/Media.tsx`, `widgets/mediaPos.ts`, `client/src/deck/Tile.tsx`, `client/src/deck/Key.tsx` (выдели из `Button.tsx` общую клавишу с локальными состояниями, если её ещё нет; `Button` переиспользует её), `client/test/mediaPos.test.ts`.
**Шаги:**
1. `mediaPos.ts`: `currentPosition(media, now): number | null` (формула PROTOCOL.md §4.5; не `playing` → `positionSec`; без `positionSec` → `null`; не больше `durationSec`) и `formatTime(sec)` (`m:ss`, от часа — `h:mm:ss`). Тесты.
2. `Media.tsx` строго по §6.2; клавиши через общий `Key` (те же состояния, тосты ошибок, `disabled` без связи).
3. Подключи `media` в диспетчер `Tile`.
**Приёмка:** тесты и сборка проходят. Ручная (headless с `layout.context.json` вместо `layout.json` в `.tmp\cfg`, телефон или Chrome): при играющей музыке видны название, исполнитель, обложка, полоса идёт; `play` ставит на паузу.
**Проверка:**
```
npm run typecheck -w client; npm test -w client; npm run build -w client
```

### T11.7 — Окно настроек: приложения страницы, виджет media, шаблоны
**Цель:** всё настраивается без правки JSON.
**Файлы:** `agent/src/tabs/LayoutTab.tsx`, `agent/src/tabs/itemForm.tsx`, `agent/src/tabs/TemplatesDialog.tsx`, `agent/src-tauri/src/commands.rs`, `agent/src/ipc.ts`.
**Шаги:**
1. Команды `get_templates`, `add_template` (и `list_window_apps` из T11.3) зарегистрированы; итог — 19 команд `#[tauri::command]`.
2. Вкладка Layout, панель страницы (под вкладками страниц): блок `Show this page automatically when active` — список приложений-«чипов» с крестиком; поле ввода с подсказками из `list_window_apps` (кнопка `Refresh`); добавление по Enter; ввод приводится к нижнему регистру, без `.exe` — дописывается `.exe`. Для первой страницы блок неактивен с текстом `The first page is your home page.`
3. Блок `Display`: чекбокс `Switch to app pages automatically` (`contextPages`; отсутствие поля показывается как включённый).
4. Форма элемента `media`: три select `Previous`, `Play / Pause`, `Next` из действий + вариант `None`; при создании элемента `media` они по умолчанию `media-prev`/`media-play`/`media-next`, если такие действия существуют. Размер по умолчанию 3×1.
5. Кнопка `Add page from template` → диалог: карточки 4 шаблонов (name, apps, число кнопок) с кнопкой `Add`; результат `add_template`: успех → перечитать раскладку и действия, открыть новую страницу; ошибки — показать `message`.
6. Тексты-подсказки: под блоком приложений — `The phone switches to this page while one of these apps is in front on your PC, and switches back when you leave it.`
**Приёмка:** typecheck, тесты и сборка проходят; в `commands.rs` 19 команд. Ручная (`npm run tauri dev`): шаблон `VS Code` добавлен → при фокусе VS Code телефон сам открывает страницу `VS Code`, при переходе в другое окно возвращается.
**Проверка:**
```
npm run typecheck -w agent; npm test -w agent; npm run build:ui; cargo build -p decka-agent
(Select-String -Path agent\src-tauri\src\commands.rs -Pattern '#\[tauri::command\]').Count
```
(последняя команда печатает `19`)

### T11.9 — Понятные ошибки редактора раскладки (отзыв владельца)
**Цель:** вместо JSON-пути пользователь видит, какой элемент и что исправить.
**Файлы:** `agent/src/tabs/LayoutTab.tsx`, `agent/src/tabs/issueText.ts`, `agent/src/tabs/itemForm.tsx`, `agent/test/issueText.test.ts`.
**Шаги:**
1. `issueText.ts`: `describeIssue(issue: ValidationIssue, layout: LayoutFile): string`. Путь `/pages/<p>/items/<i>[/...]` → префикс `Page "<name>", <kind> at column <x+1>, row <y+1>: `; путь `/pages/<p>/apps/...` → `Page "<name>": `; `/grid` → `Grid: `; иначе путь как есть. Далее — `message`. Тексты `message` в `validate.ts` (TS) для кнопок заменить на пользовательские: «choose an action or a page for this button» (нет ни одного из `actionId`/`navigate`), «a button can either run an action or open a page, not both», «action "<id>" no longer exists — choose another», «page "<id>" no longer exists — choose another». Rust-валидацию не трогать (серверные замечания проходят через тот же `describeIssue`).
2. Список замечаний в редакторе показывает `describeIssue`; клик по замечанию выделяет элемент и открывает его форму.
3. Новая кнопка, созданная кликом по пустой ячейке, сразу открывает форму с режимом `Runs action` и **первым действием по алфавиту `name`** уже выбранным (если действий нет — режим `Opens page` и первая страница, кроме текущей; если и страниц других нет — замечание остаётся).
4. Тесты `describeIssue`: кнопка без действия на (0,3) страницы "Main" → `Page "Main", button at column 1, row 4: choose an action or a page for this button`; путь `/grid` и неизвестный путь.
**Приёмка:** тесты, typecheck и сборка проходят.
**Проверка:**
```
npm run typecheck -w agent; npm test -w agent; npm run build:ui; cargo build -p decka-agent
```

### T11.10 — Клавиши в стиле аппаратной деки (отзыв владельца)
**Выполнена (`0790bf3`). В M13 её стиль становится темой `studio` (T13.3 — перенос в движок тем, а не повторная реализация).**
**Цель:** крупные квадратные клавиши с «лицевой стороной», как у аппаратных панелей кнопок; зазор 3–5%. Стиль собственный: ресурсы, иконки и фирменные элементы Elgato NEVER не используются.

**Отменяет шаги 1–3 ниже (оставлены для истории) — выполняй блок «Итоговая спецификация».**

**Итоговая спецификация:**
1. `gridMetrics(viewport, cols, rows) → { side, gap, offsetX, offsetY }`: `pad = round(0.02 × min(w, h)) + 6`; `side0 = min((w − 2·pad)/cols, (h − 2·pad)/rows)`; `gap = max(4, round(0.04 × side0))`; `side = floor(min((w − 2·pad − (cols−1)·gap)/cols, (h − 2·pad − (rows−1)·gap)/rows))`. Клавиши **квадратные** (`side × side`), сетка центрирована: остаток места делится поровну на внешние поля (`offsetX`, `offsetY`). Элемент `w×h` занимает `w·side + (w−1)·gap` × `h·side + (h−1)·gap`. Тесты: 915×412 при 7×4 и 640×360 при 5×3 → `gap/side` в 0.03..0.05, сетка помещается в экран.
2. Лицевая сторона клавиши (`Key`), радиус `0.16 × side`:
   - без `color`: фон `linear-gradient(180deg, #1c1c22 0%, #0d0d10 100%)`; иконка `#f2f2f2`;
   - с `color`: фон `linear-gradient(180deg, <color светлее на 12%> 0%, <color темнее на 18%> 100%)` (смешивание с белым/чёрным в sRGB); иконка `#ffffff`;
   - обводка `1px solid rgba(255,255,255,0.10)` (с `color` — `rgba(255,255,255,0.18)`), внутренний блик сверху `inset 0 1px 0 rgba(255,255,255,0.08)`, тень `0 2px 6px rgba(0,0,0,0.7)`.
3. Иконка: `lucide:`/`emoji:` — `round(0.46 × side)`, по центру (со смещением вверх `0.07 × side`, если есть подпись); `img:` — `round(0.62 × side)`, `object-fit: contain`, скругление `0.10 × side`.
4. Подпись — поверх лицевой стороны внизу: отступ снизу `0.07 × side`, `max(11px, 0.14 × side)`, вес 600, `#ffffff`, `text-shadow: 0 1px 2px rgba(0,0,0,0.9)`, одна строка с многоточием, боковые поля `0.07 × side`. Кнопка без иконки — подпись по центру, `0.19 × side`, до двух строк.
5. Кнопка `navigate` — маленький шеврон `›` в правом верхнем углу (`0.14 × side`, `rgba(255,255,255,0.5)`).
6. Состояния: `pressed` — масштаб 0.94, тень `inset 0 2px 6px rgba(0,0,0,0.8)`, яркость `filter: brightness(1.25)`; `pending` — обводка 2px `#3b82f6` пульсирует; `ok` — обводка 2px `#22c55e` 300 мс; `error` — 2px `#ef4444` 1500 мс; `confirming` — круговой прогресс по контуру; `disabled` — `opacity: 0.35`, без тени. Переход трансформации 80 мс.
7. Виджеты (`clock`, `sys`, `statuses`, `notifications`, `media`, `status`) — «панели» с той же лицевой стороной без `color` (градиент, обводка, тень), внутренний отступ `0.08 × side`. Внутренние клавиши `media` — тот же `Key` со стороной `0.55 × side`.
8. Пустые ячейки — едва заметный контур `1px dashed rgba(255,255,255,0.05)` только в окне настроек; на телефоне пустые ячейки не рисуются.
9. Защита AMOLED: фон экрана `#000`; заливки клавиш тёмные; цветные клавиши — по решению пользователя; режимы `dim`/`sleep` (M8) не меняются.
**Приёмка:** тесты `gridMetrics`, typecheck и сборка проходят; снимок `.tmp\keys-deck.png` с телефона: квадратные клавиши с градиентом и тенью, зазор ~4%, подписи на лицевой стороне, шеврон на кнопке `Dev`.
**Проверка:**
```
npm run typecheck -w client; npm test -w client; npm run release:apk; adb install -r client\android\app\build\outputs\apk\debug\app-debug.apk; adb shell screencap -p /sdcard/keys.png; adb pull /sdcard/keys.png .tmp\keys-deck.png
```

**Прежняя версия (не выполнять):**
Клавиши занимают всю ячейку, зазор между ними 3–5% их размера.
**Файлы:** `client/src/deck/Grid.tsx`, `Key.tsx`, `Button.tsx`, `deck/widgets/StatusTile.tsx`, `deck/widgets/Media.tsx`, `client/src/styles.css`, `client/test/gridMetrics.test.ts`.
**Шаги:**
1. `gridMetrics.ts`: чистая `gridMetrics(viewport: {w, h}, cols, rows) → { gap, pad, cellW, cellH }`: `pad = round(0.02 × min(w, h))` + 6 (запас pixel shift); сначала ячейки без зазора `c0 = min((w − 2·pad)/cols, (h − 2·pad)/rows)`, `gap = max(4, round(0.04 × c0))`; затем `cellW = (w − 2·pad − (cols−1)·gap)/cols`, `cellH = (h − 2·pad − (rows−1)·gap)/rows`. Тесты: 915×412 при 7×4 → `gap/min(cellW, cellH)` в диапазоне 0.03..0.05; 640×360 при 5×3 — то же; `gap ≥ 4`.
2. `Grid`: размеры из `gridMetrics` (`ResizeObserver` на корне), CSS `gap`/`padding` в px; прежние `2vmin`/`4vmin` убрать.
3. `Key`: заполняет ячейку целиком (`width/height: 100%`), радиус `0.14 × min(cellW, cellH)`, обводка и цвета состояний — как в T10.5. Иконка `round(0.42 × min(cellW, cellH))` px по центру; если есть подпись — иконка смещена вверх на `0.08 × cellH`, подпись внутри клавиши снизу: отступ снизу `0.08 × cellH`, размер `max(11px, 0.15 × min(cellW, cellH))`, `#9ca3af` (на цветной клавише — `#e5e5e5`), одна строка с многоточием, поля по бокам `0.08 × cellW`. Кнопка без иконки — подпись по центру, `0.2 × min(cellW, cellH)`, до двух строк. Множитель `0.7` и подпись под клавишей из T10.2/T10.5 убрать.
4. Плитка `status` и клавиши виджета `media` используют тот же `Key` (у `media` клавиши `0.55 × cellH` остаются, они внутри виджета).
5. Виджеты (`clock`, `sys`, `statuses`, `notifications`, `media`) занимают свои ячейки целиком с внутренним отступом `0.06 × min(cellW, cellH)`, без фона и обводки.
**Приёмка:** тесты, typecheck и сборка проходят; снимок экрана телефона `.tmp\keys-big.png`: клавиши почти касаются друг друга, зазор заметно меньше клавиши, подписи внутри клавиш.
**Проверка:**
```
npm run typecheck -w client; npm test -w client; npm run release:apk; adb install -r client\android\app\build\outputs\apk\debug\app-debug.apk; adb shell screencap -p /sdcard/keys.png; adb pull /sdcard/keys.png .tmp\keys-big.png
```
(В Windows PowerShell 5.1 перенаправление `>` портит бинарный вывод, поэтому снимок — через `adb pull`.)

### T11.11 — Лог в файл и разбор: кнопки виджета media не управляют воспроизведением (отзыв владельца)
**Симптом:** у владельца на A57 виджет показывает трек, но `prev`/`play`/`next` не работают. Конфигурация верна: у виджета `prevAction: media-prev`, `playAction: media-play`, `nextAction: media-next`, действия существуют.
**Файлы:** `agent/src-tauri/src/lib.rs`, `agent/core/src/logging.rs`, `client/src/deck/widgets/Media.tsx`, `client/src/deck/Key.tsx`, тесты.
**Шаги:**
1. Лог в файл (без новых крейтов): `logging.rs` — `init_file_logging(dir)`: файл `<configDir>\logs\decka.log`; при старте, если файл > 5 МБ, переименовать в `decka.log.1` (старый `.1` удалить); `tracing_subscriber` с writer'ом поверх `Mutex<File>` (режим append), уровень из `DECKA_LOG`, по умолчанию `info`. Вызывается в Tauri-приложении; headless продолжает писать в stdout. Строки S15 (`press …`) попадают в файл.
2. Новая IPC-команда `open_logs_dir(): void` (открывает `<configDir>\logs` через `explorer.exe`, создав папку при отсутствии) и кнопка `Open logs` на вкладке Status рядом с `Config folder`. Итог — 20 команд.
3. Воспроизведи на A57 с headless (dry-run) и раскладкой `layout.context.json`: нажми каждую клавишу виджета (`adb shell input tap <x> <y>`; координаты — по снимку экрана). В логе headless должны быть `press media-prev`, `press media-play`, `press media-next`. Если строк нет — дефект клиента: найди причину (перехват касаний свайпом/`touch-action` контейнера виджета, `pointer-events`, состояние `disabled`, не тот `actionId`), исправь, добавь тест (рендер `Media` с поддельным `controller.press`, клик по каждой клавише → вызов с правильным id).
4. Если строки есть — нажатия доходят; проверь реальное исполнение: headless без `--dry-run`, при играющем Spotify `ws-smoke press … media-play` должен ставить на паузу. Не ставит — опиши в отчёте (это поведение enigo/плеера), не меняй тип действия без решения архитектора.
5. Причину одной фразой — в отчёт и в описание коммита.
**Приёмка:** тест клиента проходит; в отчёте — три строки `press media-*` из лога headless после касаний по виджету на A57.
**Проверка:**
```
npm run typecheck; npm test -w client; cargo build -p decka-agent
(Select-String -Path agent\src-tauri\src\commands.rs -Pattern '#\[tauri::command\]').Count
```
(последняя команда печатает `20`)

### T11.8 — Документация и итог M11
(выполнена; см. ниже. M12 идёт после T11.9–T11.11.)
**Цель:** пользователь знает о новых функциях.
**Файлы:** `README.md`, `docs/MANUAL_CHECKS.md`, `docs/TESTING.md`.
**Шаги:**
1. README: в раздел Why добавь пункт «Context pages» (2 предложения); новый раздел `## Context pages and Now Playing` после Quick start: как включить шаблон, как назначить приложения странице, как отключить сборщики (приватность: передаётся только имя exe, заголовки окон — никогда).
2. `TESTING.md`: ручные пункты T11.3–T11.7 в общий чек-лист.
3. Собери установщик (`npm run release:agent`; если запущен `decka-agent` — остановись и сообщи, не завершай его) и APK (`npm run release:apk`), установи APK на подключённый телефон.
**Приёмка:** число заголовков `## ` в README выросло на 1; установщик и APK собраны.
**Проверка:**
```
npm run typecheck; npm test; cargo test -p decka-core; npm run release:apk; adb install -r client\android\app\build\outputs\apk\debug\app-debug.apk
```

---

## M12 — Выбор установленной программы и её иконка (решение владельца 2026-11-14)

Спецификация: DATA_MODEL.md §3.3 (`img:`), §5 (`Icon`), §8; PROTOCOL.md §3 (`FullState.icons`), §7, §9 (`InstalledApp`, 4 новые команды); DECISIONS.md D-35. Иконки из интернета не скачивать и в репозиторий не добавлять.

### T12.0 — Управление воспроизведением через медиасессию Windows (срочно, по итогам T11.11)
**Выполнять сразу после текущей задачи M12** (номер 12.0 — для порядка в документе).
**Причина:** T11.11 показала: нажатия с телефона доходят до агента, но эмулированная медиаклавиша не меняет состояние Spotify.
**Файлы:** `agent/core/src/actions/windows.rs`, `agent/core/src/collectors/media.rs` (или новый `media_control.rs`), тесты.
**Шаги:**
1. Для `media` с `op` `play_pause`/`next`/`prev` — путь по DATA_MODEL.md §2.3 (обновлённая строка `media`): менеджер сессий получить один раз и переиспользовать (тот же поток WinRT, что у сборщика `media`, или свой поток с MTA; вызовы — через канал), `Try…Async` ожидать блокирующе с таймаутом 2 с.
2. Результат: сессия вернула `true` → `Ok`; иначе медиаклавиша (прежний путь); в лог `info`: `media <op> via session|key`.
3. После успешной команды через сессию — немедленно перечитать сессию и опубликовать `media` (кнопка паузы на телефоне меняется сразу, без ожидания тика 1 с).
4. Юнит-тест чистой функции выбора пути (`session ok` → session; `no session`/`false`/`error` → key).
**Приёмка:** тест проходит. Ручная: при играющем Spotify headless без `--dry-run` и `ws-smoke press … media-play` ставит на паузу, повтор — снимает; `media-next` переключает трек; в логе `via session`.
**Проверка:**
```
cargo test -p decka-core; cargo build -p decka-agent
```

### T12.1 — Контракт: `img:`-иконки, секция `icons`, новые IPC-типы
**Файлы:** `shared/src/*.ts`, `shared/schemas/*.json`, `shared/fixtures/s2c-patch-icons.json`, тесты; `agent/core/src/model.rs`, `proto.rs`, `state.rs`, `tests/fixtures.rs`.
**Шаги:**
1. Типы и схемы — дословно из мест, перечисленных в шапке M12. Секция `icons` в `Store` (начальное значение `{}`), в `snapshot()` и `welcome`; клиент считает отсутствующую секцию `{}`.
2. Фикстура `s2c-patch-icons.json`: `{ "t": "patch", "rev": 46, "section": "icons", "value": { "0123456789abcdef": "data:image/png;base64,iVBORw0KGgo=" } }` → `ServerMsg` (TS-схема) и round-trip (Rust).
3. Тесты схемы: иконка `img:0123456789abcdef` валидна; `img:XYZ` и `img:0123` — нет.
**Приёмка/Проверка:**
```
npm test -w shared; cargo test -p decka-core --test fixtures
```

### T12.2 — Агент: хранилище иконок и извлечение из exe/картинок
**Файлы:** `agent/core/src/icons.rs`, `agent.rs`, `validate.rs`, `agent/core/Cargo.toml` (только features `windows`), `agent/src-tauri/src/commands.rs`, тесты.
**Шаги:**
1. Features `windows`: всё нужное для `IShellItemImageFactory`, `SHCreateItemFromParsingName`, `GetDIBits`/`DeleteObject`, `CoInitializeEx` (имена — по docs.rs/windows/0.62.2); `Graphics_Imaging` уже подключён.
2. `icons.rs` строго по DATA_MODEL.md §8.1: `icon_from_program(path) -> Result<String /* "img:<hash>" */, String>`, `icon_from_image(path)`, `icon_data_url(hash) -> Option<String>`. Shell-вызовы — в выделенном потоке с COM в STA (`CoInitializeEx(COINIT_APARTMENTTHREADED)`), команды через канал (как поток enigo). Пиксели HBITMAP (BGRA, 32 бит) → `BitmapEncoder` PNG (`BitmapPixelFormat::Bgra8`, `BitmapAlphaMode::Premultiplied`). Повторное извлечение того же содержимого даёт тот же hash и не плодит файлы.
3. Секция `icons` пересчитывается из раскладки при каждом её изменении (`save_layout`, наблюдатель файлов, `add_template`, старт).
4. Правило валидации 11 (DATA_MODEL.md §8.2) — только в Rust.
5. IPC `icon_from_program`, `icon_from_image`, `get_icon_preview` (PROTOCOL.md §9).
6. Тесты (под `#[cfg(windows)]`): `icon_from_program("C:\\Windows\\System32\\notepad.exe")` → `img:` + 16 hex, файл существует, PNG 96×96 (проверка через `BitmapDecoder`), data URL ≤ 32 768; повторный вызов → тот же hash; несуществующий путь → `Err`; `icon_from_image` на PNG 600×600, сгенерированном в тесте, → 96×96; раскладка со ссылкой на отсутствующий `img:` → замечание правила 11; после `save_layout` с `img:`-кнопкой секция `icons` содержит этот hash.
**Проверка:**
```
cargo test -p decka-core icons::; cargo build -p decka-agent
```

### T12.3 — Агент: список установленных программ
**Файлы:** `agent/core/src/installed.rs`, `agent/src-tauri/src/commands.rs`, тесты.
**Шаги:**
1. Строго по DATA_MODEL.md §8.3 (`IShellLinkW` + `IPersistFile`, `CommandLineToArgvW`; COM-поток из T12.2). Вызов из IPC — через `spawn_blocking`/поток, окно не блокируется.
2. Чистые функции с юнит-тестами: `is_uninstaller(name, target) -> bool` (`Uninstall Foo` → да; `Удалить Foo` → да; цель `unins000.exe` → да; `Foo` → нет); `dedupe(apps)` (дубликат из ProgramData и APPDATA → остаётся APPDATA); сортировка без учёта регистра.
3. Тест под `#[cfg(windows)]`: `list_installed_apps()` не падает, возвращает ≤ 500 элементов, у каждого `program` оканчивается на `.exe` и существует.
4. IPC `list_installed_apps`; итог — 24 команды.
**Проверка:**
```
cargo test -p decka-core installed::; cargo build -p decka-agent
(Select-String -Path agent\src-tauri\src\commands.rs -Pattern '#\[tauri::command\]').Count
```
(последняя команда печатает `24`)

### T12.4 — Клиент: отображение `img:`-иконок
**Файлы:** `client/src/deck/Icon.tsx`, `client/src/store/store.ts`, `client/test/icons.test.tsx`.
**Шаги:**
1. Секция `icons` принимается стором (добавь её в список принимаемых секций — см. баг T11.5).
2. `Icon`: `img:<hash>` → `<img src={dataURL} draggable={false}>` размером как у lucide-иконки (T11.10), `object-fit: contain`, без фильтров цвета; нет в секции → `square`.
3. Тест: `img:` с data URL в секции → разметка содержит `<img`; без записи → разметка как у `square`.
**Проверка:**
```
npm run typecheck -w client; npm test -w client; npm run build -w client
```

### T12.5 — Окно настроек: выбор установленной программы и иконки
**Файлы:** `agent/src/tabs/AppPickerDialog.tsx`, `agent/src/tabs/actionForm.tsx`, `agent/src/tabs/itemForm.tsx`, `agent/src/tabs/IconField.tsx`.
**Шаги:**
1. `AppPickerDialog`: при открытии вызывает `list_installed_apps` (индикатор `Loading installed apps…`); поле поиска (фильтр по подстроке `name` без учёта регистра); список строк «иконка 32 px + name + путь мелко»; клик → выбор. Результат кэшируется на время жизни окна настроек, кнопка `Refresh`.
2. Форма действия `launch`: рядом с `Browse…` — кнопка `Choose installed app…` → диалог → заполнить `Program`, `Arguments` (по одному в строке), `Working directory`, `Name` (если пустое — имя ярлыка). В форме действия поле иконки не появляется (иконка — свойство кнопки).
3. `IconField` (форма кнопки, вместо прежнего поля `Icon`): превью текущей иконки (`get_icon_preview` для `img:`, компонент lucide для `lucide:`, текст для `emoji:`); кнопки `Lucide…` (прежний выпадающий список 60 имён), `From program` (активна, если у кнопки выбрано действие типа `launch`: `icon_from_program(action.program)`), `From image…` (диалог файла, фильтр `Images` → `png`, `jpg`, `jpeg`, `ico`, `bmp`; `icon_from_image`), `Clear`. Ошибка → текст под полем.
4. Автоподстановка: при выборе в форме кнопки действия типа `launch`, если `icon` пуст, — вызвать `icon_from_program` и подставить результат (без сохранения раскладки до `Save`).
5. То же поле `IconField` — в редакторе состояний `bind.states` (только кнопки `Lucide…`, `From image…`, `Clear`).
**Приёмка:** typecheck и сборка проходят. Ручная (`npm run tauri dev`): `Choose installed app…` показывает список с иконками; выбранная программа → действие сохраняется; кнопка с этим действием получает иконку программы автоматически, после `Save` иконка видна на телефоне ≤ 1 с.
**Проверка:**
```
npm run typecheck -w agent; npm test -w agent; npm run build:ui; cargo build -p decka-agent
```

### T12.6 — Итог M12
**Шаги:** README — в раздел Quick start абзац «Pick an installed app and its icon»; `TESTING.md` — ручные пункты T12.2–T12.5; собрать установщик (если запущен `decka-agent` — остановись и сообщи) и APK, установить APK на подключённый телефон.
**Проверка:**
```
npm run typecheck; npm test; cargo test -p decka-core; npm run release:agent; npm run release:apk; adb install -r client\android\app\build\outputs\apk\debug\app-debug.apk
```

---

## M13 — Визуальная система, всплывающие группы, простой редактор (решение владельца 2026-11-15)

Главный приоритет продукта. Спецификация — **DESIGN.md целиком**; данные — DATA_MODEL.md §3.1 (`theme`, `accent`, `wallpaper`, `popup`, `stayOpen`), §3.2 правило 12 («9a»), §5, §8.4; PROTOCOL.md §7 (`icons`), §9 (`icon_from_svg`, `is_first_run`); DECISIONS.md D-36..D-40. Выполняется после M12. Каждая визуальная задача сдаётся со снимками экрана телефона в `.tmp\design\` (через `adb shell screencap` + `adb pull`).

### T13.1 — Пакет `@decka/ui` и перенос компонентов
**Цель:** презентационные компоненты деки живут в общем пакете.
**Шаги:**
1. Workspace `ui/` (`@decka/ui`, исходники TS без сборки, как `shared`), добавить в корневые `workspaces`. Зависимости: `react@19.3.1`, `lucide-react@1.73.0` (peer — из корня), `@decka/shared`.
2. Перенести из `client/src/deck` в `ui/src`: `Grid`, `Key`, `Icon`, `icons.ts`, `look.ts`, `gridMetrics.ts` (новая, DESIGN.md §2), все виджеты и их чистые функции, `PageDots`. Компоненты получают данные и колбэки через props (`onPress(actionId)`, `onNavigate`, `onOpenPopup`), не импортируют `controller`, `store`, `WebSocket`.
3. `client` импортирует их из `@decka/ui`; поведение и тесты не меняются (тесты переезжают в `ui/test`).
**Приёмка/Проверка:** все тесты и сборки зелёные; `client/src/deck` содержит только связку с контроллером.
```
npm run typecheck; npm test; npm run build -w client; npm run build -w agent
(Get-ChildItem ui\src -Recurse -Include *.ts,*.tsx | Select-String -Pattern 'controller|WebSocket|localStorage' | Measure-Object).Count
```
(последняя команда печатает `0`)

### T13.2 — Движок тем и тема AMOLED
**Цель:** переключаемые темы через CSS-переменные; AMOLED по DESIGN.md §1.1.
**Шаги:**
1. Контракт: поля `theme`, `accent`, `wallpaper` в TS/Rust/схемах (DATA_MODEL.md §3.1, §5); тесты схемы.
2. `ui/src/theme/`: `applyTheme(root, display)` ставит `data-theme` и переменные; файлы `amoled.css`, `studio.css`, `material.css`, `glass.css` (пока заглушки, кроме amoled). Геометрия — только из `gridMetrics` (DESIGN.md §2).
3. Тема AMOLED строго по §1.1. Значение по умолчанию при отсутствии поля — `amoled`.
4. Новые установки: в `shared/fixtures/layout.default.json` → `"theme": "studio"`.
**Приёмка:** тесты; снимок `.tmp\design\amoled.png`.

### T13.3 — Тема Studio
Перенеси стиль из T11.10 (`0790bf3`) в движок тем как `studio.css`, сверив значения с DESIGN.md §1.2 (при расхождении — по DESIGN.md). Снимок `.tmp\design\studio.png` (первая страница, одна клавиша в состоянии `pressed` — зажать через `adb shell input swipe x y x y 2000` во время снимка).

### T13.4 — Тема Material
1. `npm install -w ui @material/material-color-utilities@0.4.0`.
2. `ui/src/theme/material.ts`: генерация схемы по DESIGN.md §1.3 (точное имя класса схемы — по README пакета этой версии), кэш схем по цвету, запись ролей в CSS-переменные.
3. Морфинг формы и слой состояния при нажатии.
4. Тест: для `#7c5cff` роли `surface`, `primaryContainer`, `onPrimaryContainer` — валидные `#rrggbb`, контраст `onPrimaryContainer`/`primaryContainer` ≥ 4.5.
**Приёмка:** снимки `.tmp\design\material.png` (акцент по умолчанию) и `material-green.png` (акцент `#16a34a`).

### T13.5 — Тема Glass: прототип, затем реализация
1. **Прототип (сначала):** страница-стенд `client/dev/glass-probe.html` (в сборку не входит) с одной клавишей на обоях `aurora` в двух вариантах — с `backdrop-filter: url(#decka-glass) …` и без. На A57 (`chrome://inspect`) проверить: поддерживается ли `url()` в `backdrop-filter` (вычисленный стиль и визуальное искажение), частота кадров при 28 клавишах. Результат — в отчёт и в `docs/BLOCKERS.md`, **если** `url()` не поддерживается: тогда слой 2 DESIGN.md §1.4 не делается, остальное — делается (это заранее разрешённое упрощение, не блокер для задачи).
2. Реализация DESIGN.md §1.4 и §1.5 (обои, включая `album` из `media.art`, медленный дрейф обоев).
3. Замер частоты кадров и режим `reduced` (`localStorage` в try/catch).
**Приёмка:** снимки `.tmp\design\glass-aurora.png`, `glass-album.png` (при играющей музыке; иначе PENDING owner), `glass-reduced.png`; в отчёте — средняя частота кадров на A57 в обоих режимах.

### T13.6 — Движение
Анимации DESIGN.md §3: свайп страниц за пальцем, переходы, нажатия по темам, тосты; `prefers-reduced-motion` и `Reduce effects`. Тест: чистая функция `swipeDecision(dx, vx, width) → "next" | "prev" | "stay"` (порог 25% или 0.5 px/мс).

### T13.6a — Виброотклик
1. `npm install -w client @capacitor/haptics@8.0.2`; `npm run android:sync -w client` (разрешение `VIBRATE` плагин добавляет сам — проверь манифест после sync).
2. `client/src/platform/haptics.ts` и `hapticFor` строго по DESIGN.md §3a; тесты `hapticFor` на каждую строку таблицы и каждое исключение (успех без вибрации, пробуждающее касание, контекстная смена страницы, выключенная настройка).
3. Подключить вызовы в клиенте (кнопки, ключи виджета `media`, `confirm`, ошибки `press`, смена страницы); для групп и меню — вызовы добавляются в T13.7 и T13.11 (оставь в них по одной строке `haptic("open")` / `haptic("menu")`).
**Приёмка:** тесты, сборка, APK установлен. Ручная на A57: лёгкий отклик на каждое касание, двойной импульс при ошибке (`press` на действие, которое падает), переключатель в меню (после T13.11) отключает отклик.
**Проверка:**
```
npm run typecheck -w client; npm test -w client; npm run release:apk; adb install -r client\android\app\build\outputs\apk\debug\app-debug.apk
```

### T13.7 — Всплывающие группы
1. Контракт: `popup`, `stayOpen` на странице, `popup` у кнопки, правило 12 (TS и Rust, тексты ошибок в стиле T11.9); фикстура `layout.popup.json` (копия `layout.default.json` + popup-страница `media-more` с кнопками `vol-mute`, `vol-down`, `vol-up` в ряд и кнопка на `main` в свободной клетке `x: 3, y: 3` с `popup: "media-more"`); негативные тесты на каждое ограничение правила 12.
2. Клиент: свайп и точки пропускают popup-страницы; панель по DESIGN.md §4.1 (позиционирование — чистая функция `placePopup(anchorRect, panelSize, viewport, pad)` с тестами: панель внутри экрана во всех углах); `actionId + popup` — панель после `ok`; закрытие по всем четырём правилам; автосмена страницы закрывает панель.
3. Окно настроек: страницы-группы с меткой `Popup` и кнопка `Add popup group`; в форме кнопки — `On tap`: `Run action` / `Open page` / `Open popup group` / `Run action, then open popup group`.
**Приёмка:** тесты; снимок `.tmp\design\popup.png` с открытой панелью.

### T13.8 — Иконки Phosphor
1. `npm install -w agent @phosphor-icons/core@2.1.1`; индекс иконок (имя, теги, начертания) строится из пакета при сборке окна настроек; SVG загружаются по требованию (динамический импорт или `?raw`).
2. Агент: `icon_from_svg` по DATA_MODEL.md §8.4 (проверка, нормализация, хранение, секция `icons`); тесты: допустимый SVG Phosphor → `img:`; SVG с `<script>`, с `onload=`, с внешним `href` → ошибка; повтор → тот же hash.
3. Клиент: рендер SVG-иконок маской (DATA_MODEL.md §8.4), цвет — из темы; тест разметки.
4. `IconField`: вкладки `Phosphor` / `Quick` / `Program` / `Image` / `Emoji` (DESIGN.md §5), поиск, переключатель начертания, виртуализированная сетка (рендерить только видимые строки).
**Приёмка:** тесты; снимок телефона с клавишами в иконках Phosphor во всех четырёх темах (`.tmp\design\phosphor-<theme>.png`).

### T13.9 — Редактор «что видишь — то и получишь»
По DESIGN.md §4.4 (пункт «Редактор раскладки»): превью из `@decka/ui` в выбранной теме внутри рамки телефона; перетаскивание (перемещение, обмен, создание из палитры `Apps`/`Actions`/`Widgets`, изменение размера виджета); отмена/повтор (50 шагов); `Ctrl+S`; метка `Unsaved changes`. Чистые функции `moveItem`, `swapItems`, `resizeItem`, `history` — с тестами в `agent/test`. Старые числовые поля `X/Y/W/H` остаются в панели свойств (свёрнуты в `Advanced`).
**Приёмка:** тесты; ручная (`npm run tauri dev`) — перетаскивание программы из палитры `Apps` на клетку создаёт кнопку с иконкой программы; видна на телефоне после `Ctrl+S`.

### T13.10 — Вкладка Appearance, мастер первого запуска, обновлённый вид окна
1. Вкладка `Appearance` (DESIGN.md §4.4): карточки 4 тем с живым превью (компоненты `@decka/ui`, мини-сетка 3×2), акцент (8 образцов: `#7c5cff`, `#3b82f6`, `#06b6d4`, `#16a34a`, `#eab308`, `#f97316`, `#ef4444`, `#ec4899` + произвольный), обои (для Glass), контекстные страницы, блок `Display` (переносится из Layout). Смена применяется на телефоне ≤ 1 с после `Save`.
2. Мастер первого запуска (`is_first_run`), три шага по DESIGN.md §4.4.
3. Вид окна: боковое меню с иконками Phosphor, отступы 8 px, системный шрифт, светлая/тёмная тема по системе, акцент = `display.accent`.
**Приёмка:** typecheck/сборка; ручная — мастер на чистом `DECKA_CONFIG_DIR` проходит до конца, выбранные программы появляются кнопками.

### T13.11 — Меню телефона и дизайн-приёмка
1. Меню телефона по DESIGN.md §4.3; удалить жест «5 тапов».
2. Дизайн-приёмка: для каждой темы — снимки первой страницы, страницы с виджетами (`media`, `statuses`, `notifications` с данными), открытой группы и заставки в `.tmp\design\final-<theme>-<n>.png`; автоматическая проверка контраста подписей ≥ 4.5 для всех тем и 8 акцентов (чистая функция `contrast(a, b)` + тест по токенам); частота кадров на A57 для каждой темы — в отчёт.
3. README: раздел `## Looks` со списком тем (без картинок, место под скриншоты `docs/img/theme-*.png`).
4. Собрать установщик и APK, установить APK.
**Приёмка:** все тесты; отчёт со ссылками на снимки и таблицей «тема → к/с».
```
npm run typecheck; npm test; cargo test -p decka-core; npm run release:agent; npm run release:apk; adb install -r client\android\app\build\outputs\apk\debug\app-debug.apk
```

### T13.12 — Контраст Glass и второстепенного текста (решение D-1)
1. Слой затемнения обоев по DESIGN.md §1.5 (абзац «Затемнение»). Удобнее всего — псевдоэлемент слоя обоев, чтобы он смещался вместе с ним.
2. `readableFace` привести к DESIGN.md §1.6: только белая подпись, затемнение цвета до 4.5; удалить ветку «почти чёрная подпись», если она есть. Окно настроек (превью редактора и карточки тем) использует ту же функцию из `@decka/ui`.
3. Тест контраста: Glass по всем обоям, кроме `album`, — строго ≥ 4.5, с учётом слоя затемнения и стекла `rgba(255,255,255,0.07)`. Добавить проверку второстепенного текста (токен приглушённого текста каждой темы на фоне панели виджета; для Glass — на худшем участке обоев). Если токен не проходит — поднять его яркость до ближайшего значения, которое проходит, и указать старое и новое значение в отчёте.
4. В `docs/BLOCKERS.md` у D-1 дописать строку «Решено: T13.12».
5. Раскладка по умолчанию: у `b-mute` иконка `lucide:volume` вместо `lucide:volume-2`, чтобы она не совпадала с `b-volup` (DATA_MODEL.md §3, пример). Поправить везде, где раскладка по умолчанию лежит в коде или фикстурах. Файл владельца не трогать.
6. Снимки `final-glass-1..3.png` переснять.
**Приёмка:** все тесты, контраст-тест без ослаблений, снимки.
```
npm run typecheck; npm test; npm run release:apk; adb install -r client\android\app\build\outputs\apk\debug\app-debug.apk
```

### T13.13 — Светлые клавиши, клавиши виджета media, замер Glass reduced
Выполнена (отчёт 2026-11-15). Отклонение принято: при `h = 1` правило минимальной ширины текста даёт 3×1 без клавиш, 4×1 — `play`, `w ≥ 5` — три клавиши.
1. `readableFace` по DESIGN.md §1.6, абзац «Исключение для светлых цветов»: тёмная подпись `#0b0b0f` при контрасте ≥ 7 с исходным цветом, иначе белая с затемнением. Тесты: `#ffffff` и `#facc15` не меняются, подпись тёмная; `#7c5cff` затемняется, подпись белая; все результаты ≥ 4.5.
2. Виджет `media` по DATA_MODEL.md §6.2 (новое правило клавиш при `h = 1` и минимальная ширина текста). Цвет `artist` — токен `--muted`. Тест на выбор набора клавиш по `w`, `h`, `side`.
3. A57, тема Glass: замерить к/с в режиме `reduced` (переключатель `Reduce effects`) тем же способом, что в T13.11. Если < 45 и в `reduced`, записать в BLOCKERS.md и не трогать спецификацию.
4. Проверки на телефоне — только на копии конфигурации: агент с `DECKA_CONFIG_DIR=.tmp\cfg-phone` (копия `%APPDATA%\Decka`); файлы владельца в `%APPDATA%\Decka` не менять даже временно. Перед съёмкой — `adb shell input keyevent KEYCODE_WAKEUP` и проверка, что на переднем плане `app.decka.client` (`dumpsys activity activities | findstr mResumed`); кнопку «назад» не нажимать.
5. Снимки `a57-glass-1.png` (раскладка владельца с виджетом media 3×1) и `a57-studio-keys.png` (страница с клавишами белого, жёлтого, `#7c5cff`, красного цвета на тестовой конфигурации).
**Приёмка:** все тесты, к/с reduced в отчёте, снимки.
```
npm run typecheck; npm test; npm run release:apk; adb install -r client\android\app\build\outputs\apk\debug\app-debug.apk
```

## M14 — Простое создание кнопок и онбординг (отзыв владельца 2026-11-15)

Источник: DESIGN.md §4.5–§4.7, DECISIONS.md D-38 (дополнение), D-41, BLOCKERS.md D-2. Общее правило M14: всё, что касается окна настроек, проверяется в **настоящем окне** (`npm run tauri dev` или установленный агент), не в Chrome с подменой IPC. Снимки окна — `.tmp\design\m14-*.png` (снимок окна через `PrintWindow`/`CopyFromScreen` в PowerShell-скрипте из scratch-папки, не в репозитории).

### T14.0 — Скорость Glass (D-2)
1. По DESIGN.md §1.4–§1.5: обложка `album` через `canvas` 24×24 один раз на смену `media.art`; ступенчатое смещение обоев (30 с, переход 2 с); `reduced` без `backdrop-filter` и без смещения.
2. Замер на A57 в приложении (раскладка владельца, тот же метод `gfxinfo`): Glass обычный и `reduced`, обои `album` и `aurora` (переключать на копии конфигурации — правило T13.13 п. 4; для раскладки владельца замер только с теми обоями, что уже выбраны).
**Приёмка:** тесты; таблица к/с. Цель: обычный ≥ 45, `reduced` ≥ 55. Не достигнуто → BLOCKERS.md, спецификацию не менять.

### T14.1 — Перетаскивание на pointer-событиях
1. Убрать HTML5 drag-and-drop (`draggable`, `onDragStart`, `onDrop`) из `EditorCanvas` и `Palette`. Перетаскивание клавиш и пунктов палитры — pointer-события с `setPointerCapture`, порог 4 px, призрак клавиши, подсветка клетки, анимация обмена (DESIGN.md §4.6). Чистая функция-автомат `dragReducer(state, event)` (idle → pressed → dragging → drop/cancel) с тестами; `Esc` отменяет перетаскивание.
2. `dragDropEnabled` в `tauri.conf.json` не трогать (остаётся по умолчанию `true`).
3. Бросок `.exe`/`.lnk` из Проводника: `getCurrentWebview().onDragDropEvent` → позиция → клетка превью → `describe_program` → создание кнопки по DESIGN.md §4.5. Агент: команда `describe_program` (PROTOCOL.md §9) с тестами в `decka-core` для разбора .lnk и .exe (фикстуры — ярлык на `C:\Windows\System32\notepad.exe`, создаваемый в тесте; FileDescription `notepad.exe`).
**Приёмка:** тесты; в настоящем окне: перемещение клавиши, обмен, перенос программы из палитры, бросок `notepad.exe` из Проводника — каждое видно на телефоне. Что исполнитель не может сделать мышью сам — пункт в MANUAL_CHECKS.md как PENDING (owner), но запуск окна и отсутствие ошибок в консоли проверить обязательно.

### T14.2 — Быстрый выбор: кнопка за два клика
1. `+` на пустой клетке и окно быстрого выбора (DESIGN.md §4.5): поиск, группы `Apps` / `Media & system` / `Your actions` / `Widgets`, нижняя строка `Browse for a program…` (диалог `tauri-plugin-dialog`, фильтр `exe`, `lnk`) / `Website…` / `Keyboard shortcut…`.
2. Чистые функции в `agent/src/editor/`: `presetActions()` (таблица §4.5), `actionFromApp(app)`, `actionFromUrl(url)`, `actionFromKeys(keys)`, `findSameAction(actions, action)`, `makeActionId(name, existing)`, `addButtonAt(layout, actions, page, cell, choice)` → новые `layout` и `actions`. Тесты: переиспользование одинакового действия; суффиксы id; подпись обрезается до 24; URL → домен без `www.`; результат проходит `validateLayout`/`validateActions`.
3. Запись сочетания клавиш: `keydown` в поле записи → имена клавиш по DATA_MODEL.md §2 (функция `keyNameFromEvent(e)` с тестами на `Ctrl`, `Shift`, `Alt`, `Win`, буквы, цифры, `F1..F24`, стрелки); неподдерживаемая клавиша — подсказка `This key is not supported`.
**Приёмка:** тесты; в настоящем окне: клик по пустой клетке → выбрать Spotify (или любую программу из списка) → кнопка с иконкой программы на телефоне, всего два клика; снимки `m14-picker.png`, `m14-created.png`.

### T14.3 — Автосохранение, контекстное меню, панель свойств
1. Автосохранение по DESIGN.md §4.6 (600 мс, `Saved`, невалидное не сохраняется); убрать `Unsaved changes` и обязательность `Ctrl+S` (сочетание оставить как немедленное сохранение). Отмена/повтор тоже сохраняются. Порядок записи: сначала `save_actions`, затем `save_layout`; ошибка — показ у поля и без потери истории.
2. Контекстное меню клавиши и клавиша `Delete` (§4.6).
3. Панель свойств кнопки в новом порядке (§4.6), строка `Does: …` (функция `describeAction(action)` с тестами: `Launch Spotify`, `Open github.com`, `Press Ctrl+Shift+M`, `Play / Pause`, `Lock PC`).
**Приёмка:** тесты; в настоящем окне: изменение подписи появляется на телефоне без `Ctrl+S`; `Ctrl+Z` возвращает; снимок `m14-props.png`.

### T14.4 — Онбординг
1. Мастер: строка `Media & system` в шаге 3 (§4.7); `Run setup again` во вкладке `About` и в пустом состоянии; мастер только добавляет в свободные клетки (используй `addButtonAt`).
2. Подсказки редактора (три шага, `localStorage` `decka.editor.tips.v1`), пустое состояние страницы, полоса `Phone not connected`.
3. README: раздел «Add your first button» (3 строки).
**Приёмка:** тесты; в настоящем окне на чистом `DECKA_CONFIG_DIR`: мастер до конца → кнопки программ и медиа на телефоне; подсказки показываются один раз; снимки `m14-wizard.png`, `m14-tips.png`.
```
npm run typecheck; npm test; cargo test -p decka-core; npm run release:agent; npm run release:apk; adb install -r client\android\app\build\outputs\apk\debug\app-debug.apk
```