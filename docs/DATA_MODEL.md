# DATA_MODEL.md — файлы и модель данных Decka

## 1. Каталог конфигурации

Путь: `%APPDATA%\Decka\` (в Rust: `dirs::config_dir()` + `Decka`). Переопределяется переменной окружения `DECKA_CONFIG_DIR` (используется тестами и headless-режимом).

| Файл | Кто пишет | Содержимое |
|---|---|---|
| `config.json` | агент (окно настроек), пользователь вручную | порт, bind, apiToken, USB, сборщики |
| `actions.json` | агент, пользователь вручную | определения действий (выполняются только на ПК) |
| `layout.json` | агент, пользователь вручную | страницы, кнопки, виджеты, настройки экрана |
| `devices.json` | только агент | сопряжённые устройства (хэши токенов) |

Правила для всех файлов:
- UTF-8 без BOM, отступ 2 пробела, поле `"version": 1` обязательно.
- Запись атомарная: запись в `<имя>.tmp` в том же каталоге, затем rename поверх.
- Отсутствующий файл при старте → создаётся из значений по умолчанию (§2.4, §3.5, §4).
- `actions.json` и `layout.json` отслеживаются (crate `notify`, debounce 300 мс). После изменения: файл читается и валидируется; валиден → применяется, секция `layout` рассылается клиентам; невалиден → **старая версия остаётся в силе**, в секцию `notifications` добавляется уведомление `level: "error"`, `source: "decka"`, `title: "<файл> invalid"`, `body` = первая ошибка валидации.
- Невалидный файл при старте → агент работает с пустыми значениями (`actions: {}`, одна пустая страница `main`), файл на диске не трогает, ошибку показывает в окне настроек и уведомлением.
- `config.json` не отслеживается: читается при старте и при `save_config`.
- Неизвестные поля при чтении игнорируются, при записи агентом — не сохраняются.

## 2. actions.json

### 2.1 Типы

```ts
// /shared/src/actions.ts
export interface ActionsFile { version: 1; actions: Record<Id, Action>; }

interface ActionBase {
  name: string;          // 1..60 символов, показывается в окне настроек
  toggleVar?: VarName;   // после УСПЕШНОГО выполнения агент инвертирует булеву переменную (нет переменной → true)
}
export type Action =
  | (ActionBase & { type: "hotkey"; keys: KeyName[] })                 // 1..5 клавиш
  | (ActionBase & { type: "media"; op: MediaOp })
  | (ActionBase & { type: "launch"; program: string; args?: string[]; cwd?: string; hidden?: boolean })
  | (ActionBase & { type: "open_url"; url: string })
  | (ActionBase & { type: "system"; op: "lock" | "sleep" });

export type MediaOp = "play_pause" | "next" | "prev" | "vol_up" | "vol_down" | "mute";
export type KeyName = string; // см. 2.2
```

### 2.2 Клавиши (`KeyName`)

Регистр — только нижний. Допустимый набор (всё остальное — ошибка валидации):

- модификаторы: `ctrl`, `alt`, `shift`, `win`
- буквы `a`..`z`, цифры `0`..`9`
- `f1`..`f24`
- `enter`, `esc`, `space`, `tab`, `backspace`, `delete`, `home`, `end`, `pageup`, `pagedown`, `up`, `down`, `left`, `right`

Исполнение `hotkey`: клавиши нажимаются в порядке массива (press), затем отпускаются в обратном порядке (release). Пример `["ctrl","shift","esc"]`.

### 2.3 Семантика и валидация

| type | Исполнение на Windows | Валидация |
|---|---|---|
| `hotkey` | crate `enigo`, см. 2.2 | `keys`: 1..5, каждая из набора, без повторов |
| `media` | `enigo`: `play_pause`→MediaPlayPause, `next`→MediaNextTrack, `prev`→MediaPrevTrack, `vol_up`→VolumeUp, `vol_down`→VolumeDown, `mute`→VolumeMute (click) | `op` из списка |
| `launch` | `std::process::Command::new(program).args(args)`, **без shell**; `cwd` → `current_dir`; `hidden: true` → флаг создания `CREATE_NO_WINDOW` (0x08000000); процесс не ожидается (spawn и забыть) | `program` 1..500 символов; `args` ≤ 32 элементов по ≤ 500 символов; `cwd` ≤ 500 |
| `open_url` | `rundll32.exe url.dll,FileProtocolHandler <url>` через `Command` без shell | `url` ≤ 2000 символов, парсится crate `url`, схема строго `http` или `https` |
| `system` | `lock` → `rundll32.exe user32.dll,LockWorkStation`; `sleep` → `rundll32.exe powrprof.dll,SetSuspendState 0,1,0` | `op` из списка |

- Ключ действия — `Id`. `name` обязателен.
- Успех действия = процесс успешно порождён / клавиши отправлены. Код возврата запущенной программы не отслеживается.
- Ошибка spawn → `press_result` с `ACTION_FAILED` и `message` = текст ошибки ОС.
- `launch` не раскрывает переменные окружения и не интерпретирует `%VAR%`; `program` без пути ищется в `PATH` стандартным поведением `Command`.
- Примечание: `sleep` при включённой гибернации Windows выполняет гибернацию — известное поведение `SetSuspendState`, описать в README.

### 2.4 Файл по умолчанию (он же `shared/fixtures/actions.default.json`)

```json
{
  "version": 1,
  "actions": {
    "media-play":  { "name": "Play / Pause", "type": "media", "op": "play_pause" },
    "media-next":  { "name": "Next track", "type": "media", "op": "next" },
    "media-prev":  { "name": "Previous track", "type": "media", "op": "prev" },
    "vol-up":      { "name": "Volume up", "type": "media", "op": "vol_up" },
    "vol-down":    { "name": "Volume down", "type": "media", "op": "vol_down" },
    "vol-mute":    { "name": "Mute", "type": "media", "op": "mute", "toggleVar": "audio.muted" },
    "lock":        { "name": "Lock PC", "type": "system", "op": "lock" },
    "task-manager":{ "name": "Task Manager", "type": "hotkey", "keys": ["ctrl", "shift", "esc"] },
    "notepad":     { "name": "Notepad", "type": "launch", "program": "notepad.exe" },
    "github":      { "name": "GitHub", "type": "open_url", "url": "https://github.com" }
  }
}
```

## 3. layout.json

### 3.1 Типы

```ts
// /shared/src/layout.ts
export interface LayoutFile {
  version: 1;
  grid: { cols: 5; rows: 3 };     // в v1 допустимо только 5×3
  display: DisplaySettings;
  pages: Page[];                  // 1..10 страниц, порядок = порядок свайпа
}
export interface Page { id: Id; name: string; items: LayoutItem[]; }

export interface DisplaySettings {
  dimAfterSec: number;    // 0 = не затемнять; иначе 10..3600. По умолчанию 60
  dimLevel: number;       // 0.05..1 — доля яркости в затемнённом режиме. По умолчанию 0.3
  sleepAfterSec: number;  // 0 = без заставки; иначе 30..86400 и > dimAfterSec. По умолчанию 600
  pixelShift: boolean;    // по умолчанию true
  night: { enabled: boolean; from: string; to: string }; // "HH:MM", локальное время телефона
}

interface ItemBase { id: Id; x: number; y: number; w: number; h: number; }

export interface ButtonLook { label?: string; icon?: string; color?: string; }
export interface ButtonItem extends ItemBase, ButtonLook {
  kind: "button";
  actionId?: Id;          // ровно одно из actionId / navigate
  navigate?: Id;          // id страницы
  confirm?: boolean;      // true → нажатие удержанием 800 мс
  bind?: { var: VarName; states: Record<string, ButtonLook> }; // ключ = String(значение переменной)
}
export interface ClockItem    extends ItemBase { kind: "clock"; seconds?: boolean; }
export interface SysItem      extends ItemBase { kind: "sys"; }
export interface StatusesItem extends ItemBase { kind: "statuses"; group?: string; title?: string; }
export interface StatusItemTile extends ItemBase { kind: "status"; statusId: Id; }
export interface NotificationsItem extends ItemBase { kind: "notifications"; }

export type LayoutItem = ButtonItem | ClockItem | SysItem | StatusesItem | StatusItemTile | NotificationsItem;

// То, что уходит на телефон в секции "layout": LayoutFile без поля version.
export type ClientLayout = Omit<LayoutFile, "version">;
```

Телефон получает из раскладки `actionId`, но никогда не получает содержимое действия (тип, программу, клавиши).

### 3.2 Валидация раскладки

1. `grid` строго `{cols: 5, rows: 3}`.
2. `pages`: 1..10; `id` уникальны; `name` 1..30 символов.
3. Каждый элемент: `0 ≤ x`, `0 ≤ y`, `w ≥ 1`, `h ≥ 1`, `x + w ≤ 5`, `y + h ≤ 3`; элементы одной страницы не пересекаются; `id` уникальны в пределах всего файла.
4. Размеры по видам (w×h): `button` — только 1×1; `clock` — 1×1, 2×1; `sys` — 2×1, 2×2; `status` — 1×1; `statuses` — от 2×1 до 5×3; `notifications` — от 2×1 до 5×3.
5. `button`: ровно одно из `actionId`/`navigate`; `actionId` должен существовать в `actions.json`; `navigate` — существующий `id` страницы; `confirm` допустим только с `actionId`.
6. `label` ≤ 24 символов; `color` — regex `^#[0-9a-fA-F]{6}$`; `icon` — §3.3.
7. `bind.states`: 1..8 ключей, каждый ключ ≤ 40 символов.
8. `display`: диапазоны из комментариев в §3.1; `night.from`/`to` — regex `^([01][0-9]|2[0-3]):[0-5][0-9]$`.

Проверки 1–4, 6–8 выражены в JSON Schema там, где это возможно (§5); пересечения, уникальность, ссылки (п. 3, 5) проверяются кодом — одинаково в Rust (`decka-core::config::validate_layout`) и в TS (`shared/src/validate.ts`, используется редактором в окне настроек).

Если `actions.json` изменился так, что кнопка ссылается на удалённое действие: раскладка остаётся в силе, кнопка рисуется, нажатие вернёт `UNKNOWN_ACTION`; в окне настроек показывается предупреждение.

### 3.3 Иконки

Поле `icon` — строка одного из двух видов:
- `lucide:<name>` — имя иконки Lucide в kebab-case (`lucide:play`, `lucide:volume-2`). Regex `^lucide:[a-z0-9-]{1,40}$`. Клиент включает в сборку **фиксированный набор** из 60 иконок (список — §3.6); неизвестное имя → иконка `lucide:square`.
- `emoji:<символ>` — один emoji (`emoji:🚀`), ≤ 8 UTF-16 единиц после префикса.

Пользовательские картинки — вне MVP.

### 3.4 Состояния кнопок

Вид кнопки = базовый `ButtonLook` (поля `label`, `icon`, `color` самой кнопки), поверх которого накладывается `bind.states[String(vars[bind.var])]`, если такой ключ есть. Переменная отсутствует → ищется ключ `"false"`; нет и его → базовый вид.

Переменные (`vars`) живут только в памяти агента (обнуляются при перезапуске). Источники изменения: `toggleVar` действия и HTTP `PUT /api/vars/{name}`. Локальные (не серверные) состояния кнопки на клиенте:

| Состояние | Когда | Вид |
|---|---|---|
| `idle` | по умолчанию | базовый/привязанный вид |
| `pressed` | палец на кнопке | масштаб 0.94 |
| `confirming` | удержание кнопки с `confirm` | круговой прогресс 800 мс |
| `pending` | `press` отправлен, ответа нет | пульсация рамки |
| `ok` | `press_result.ok = true` | зелёная рамка 300 мс |
| `error` | `ok = false` или таймаут 5 с | красная рамка 1500 мс + тост с текстом по коду |
| `disabled` | нет соединения | прозрачность 0.4, нажатия игнорируются |

### 3.5 Файл по умолчанию (он же `shared/fixtures/layout.default.json`)

```json
{
  "version": 1,
  "grid": { "cols": 5, "rows": 3 },
  "display": { "dimAfterSec": 60, "dimLevel": 0.3, "sleepAfterSec": 600, "pixelShift": true, "night": { "enabled": false, "from": "23:00", "to": "07:00" } },
  "pages": [
    { "id": "main", "name": "Main", "items": [
      { "id": "w-clock", "kind": "clock", "x": 0, "y": 0, "w": 2, "h": 1 },
      { "id": "w-sys", "kind": "sys", "x": 2, "y": 0, "w": 2, "h": 1 },
      { "id": "b-lock", "kind": "button", "x": 4, "y": 0, "w": 1, "h": 1, "label": "Lock", "icon": "lucide:lock", "color": "#b91c1c", "actionId": "lock", "confirm": true },
      { "id": "b-prev", "kind": "button", "x": 0, "y": 1, "w": 1, "h": 1, "icon": "lucide:skip-back", "actionId": "media-prev" },
      { "id": "b-play", "kind": "button", "x": 1, "y": 1, "w": 1, "h": 1, "icon": "lucide:play", "actionId": "media-play" },
      { "id": "b-next", "kind": "button", "x": 2, "y": 1, "w": 1, "h": 1, "icon": "lucide:skip-forward", "actionId": "media-next" },
      { "id": "w-notifs", "kind": "notifications", "x": 3, "y": 1, "w": 2, "h": 2 },
      { "id": "b-mute", "kind": "button", "x": 0, "y": 2, "w": 1, "h": 1, "icon": "lucide:volume-2", "actionId": "vol-mute",
        "bind": { "var": "audio.muted", "states": { "true": { "icon": "lucide:volume-x", "color": "#b45309" } } } },
      { "id": "b-voldown", "kind": "button", "x": 1, "y": 2, "w": 1, "h": 1, "icon": "lucide:volume-1", "actionId": "vol-down" },
      { "id": "b-volup", "kind": "button", "x": 2, "y": 2, "w": 1, "h": 1, "icon": "lucide:volume-2", "actionId": "vol-up" }
    ] },
    { "id": "dev", "name": "Dev", "items": [
      { "id": "w-statuses", "kind": "statuses", "x": 0, "y": 0, "w": 3, "h": 3, "title": "Services" },
      { "id": "b-github", "kind": "button", "x": 3, "y": 0, "w": 1, "h": 1, "label": "GitHub", "icon": "lucide:github", "actionId": "github" },
      { "id": "b-notepad", "kind": "button", "x": 4, "y": 0, "w": 1, "h": 1, "label": "Notepad", "icon": "lucide:file-text", "actionId": "notepad" },
      { "id": "b-taskmgr", "kind": "button", "x": 3, "y": 1, "w": 1, "h": 1, "label": "Tasks", "icon": "lucide:activity", "actionId": "task-manager" },
      { "id": "b-home", "kind": "button", "x": 4, "y": 2, "w": 1, "h": 1, "label": "Main", "icon": "lucide:home", "navigate": "main" }
    ] }
  ]
}
```

### 3.6 Набор иконок Lucide в сборке клиента (60)

`play, pause, skip-back, skip-forward, volume, volume-1, volume-2, volume-x, mic, mic-off, video, video-off, monitor, lock, power, moon, sun, home, settings, terminal, code, github, git-branch, globe, folder, file-text, activity, cpu, hard-drive, wifi, bell, bell-off, mail, message-square, music, headphones, camera, image, calendar, clock, timer, star, heart, zap, rocket, bug, check, x, plus, minus, refresh-cw, search, download, upload, copy, trash-2, square, circle, arrow-left, arrow-right`

Если имя в установленной версии `lucide-react` отсутствует (переименовано), исполнитель заменяет его на `square` и перечисляет замены в описании коммита — список в этом файле не меняется.

## 4. config.json и devices.json

```ts
// /shared/src/config.ts
export interface ConfigFile {
  version: 1;
  agentName: string;                 // 1..40; по умолчанию имя компьютера (env COMPUTERNAME)
  port: number;                      // 1024..65535; по умолчанию 9247
  bind: "0.0.0.0" | "127.0.0.1";     // по умолчанию "0.0.0.0"; "127.0.0.1" = режим «только USB»
  apiToken: string;                  // "dk_" + 43 символа base64url; генерируется при первом старте
  autostart: boolean;                // по умолчанию false
  usb: { enabled: boolean; adbPath: string };   // по умолчанию { false, "adb" }
  collectors: {
    sys: { enabled: boolean; intervalSec: number };   // по умолчанию { true, 2 }; 1..60
    checks: HttpCheck[];                              // по умолчанию []; максимум 20
  };
}
export interface HttpCheck {
  id: Id; label: string;             // label 1..60
  url: string;                       // http/https, <= 2000
  intervalSec: number;               // 10..3600
  timeoutSec: number;                // 1..30
  expectStatus?: number;             // по умолчанию: любой 200..399 = ok
  group?: string;                    // <= 40
}

export interface DeviceInfo { id: Id; name: string; createdAt: number; lastSeenAt: number; connected: boolean; }
// devices.json на диске:
// { "version": 1, "devices": [ { "id", "name", "tokenHash", "createdAt", "lastSeenAt" } ] }
// tokenHash = hex(SHA-256(token)); сам токен на диск NEVER не пишется.
```

Правила HTTP-проверок (`checks`): `GET url`, редиректы не следуются, тело не читается. Результат → `StatusItem` с `id = check.id`, `source: "check"`: ответ с ожидаемым статусом → `state: "ok"`, `text: "<код> · <мс> ms"`; иной код → `error`, `text: "<код>"`; таймаут/ошибка сети → `error`, `text: "timeout"` или `"unreachable"`. До первого результата — `unknown`. Проверка, удалённая из конфига, удаляет свой `StatusItem`.

Пример `config.json` (он же `shared/fixtures/config.example.json`):

```json
{
  "version": 1,
  "agentName": "DESKTOP-MAIN",
  "port": 9247,
  "bind": "0.0.0.0",
  "apiToken": "dk_pX3v8Zq1mB7kT0aYw2LrN5cE9uHs4JdG6fQoViC_-AA",
  "autostart": false,
  "usb": { "enabled": false, "adbPath": "adb" },
  "collectors": {
    "sys": { "enabled": true, "intervalSec": 2 },
    "checks": [ { "id": "api-prod", "label": "API prod", "url": "https://example.com/health", "intervalSec": 60, "timeoutSec": 5, "group": "services" } ]
  }
}
```

## 5. JSON Schema раскладки и действий

`/shared/schemas/layout.schema.json`:

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://decka.invalid/schemas/layout.schema.json",
  "$defs": {
    "Id": { "type": "string", "pattern": "^[a-z0-9_-]{1,64}$" },
    "Icon": { "type": "string", "pattern": "^(lucide:[a-z0-9-]{1,40}|emoji:.{1,8})$" },
    "Color": { "type": "string", "pattern": "^#[0-9a-fA-F]{6}$" },
    "ButtonLook": { "type": "object", "properties": { "label": { "type": "string", "maxLength": 24 }, "icon": { "$ref": "#/$defs/Icon" }, "color": { "$ref": "#/$defs/Color" } } },
    "ItemBase": { "type": "object", "required": ["id","kind","x","y","w","h"],
      "properties": { "id": { "$ref": "#/$defs/Id" }, "kind": { "type": "string" },
        "x": { "type": "integer", "minimum": 0, "maximum": 4 }, "y": { "type": "integer", "minimum": 0, "maximum": 2 },
        "w": { "type": "integer", "minimum": 1, "maximum": 5 }, "h": { "type": "integer", "minimum": 1, "maximum": 3 } } },
    "ButtonItem": { "allOf": [ { "$ref": "#/$defs/ItemBase" }, { "$ref": "#/$defs/ButtonLook" }, {
      "properties": { "kind": { "const": "button" }, "w": { "const": 1 }, "h": { "const": 1 },
        "actionId": { "$ref": "#/$defs/Id" }, "navigate": { "$ref": "#/$defs/Id" }, "confirm": { "type": "boolean" },
        "bind": { "type": "object", "required": ["var","states"], "properties": {
          "var": { "type": "string", "pattern": "^[a-z0-9_.-]{1,64}$" },
          "states": { "type": "object", "minProperties": 1, "maxProperties": 8, "propertyNames": { "maxLength": 40 }, "additionalProperties": { "$ref": "#/$defs/ButtonLook" } } } } },
      "oneOf": [ { "required": ["actionId"] }, { "required": ["navigate"] } ] } ] },
    "ClockItem": { "allOf": [ { "$ref": "#/$defs/ItemBase" }, { "properties": { "kind": { "const": "clock" }, "w": { "maximum": 2 }, "h": { "const": 1 }, "seconds": { "type": "boolean" } } } ] },
    "SysItem": { "allOf": [ { "$ref": "#/$defs/ItemBase" }, { "properties": { "kind": { "const": "sys" }, "w": { "const": 2 }, "h": { "maximum": 2 } } } ] },
    "StatusesItem": { "allOf": [ { "$ref": "#/$defs/ItemBase" }, { "properties": { "kind": { "const": "statuses" }, "w": { "minimum": 2 }, "group": { "type": "string", "maxLength": 40 }, "title": { "type": "string", "maxLength": 30 } } } ] },
    "StatusTile": { "allOf": [ { "$ref": "#/$defs/ItemBase" }, { "required": ["statusId"], "properties": { "kind": { "const": "status" }, "w": { "const": 1 }, "h": { "const": 1 }, "statusId": { "$ref": "#/$defs/Id" } } } ] },
    "NotificationsItem": { "allOf": [ { "$ref": "#/$defs/ItemBase" }, { "properties": { "kind": { "const": "notifications" }, "w": { "minimum": 2 } } } ] },
    "KnownItem": { "oneOf": [ { "$ref": "#/$defs/ButtonItem" }, { "$ref": "#/$defs/ClockItem" }, { "$ref": "#/$defs/SysItem" }, { "$ref": "#/$defs/StatusesItem" }, { "$ref": "#/$defs/StatusTile" }, { "$ref": "#/$defs/NotificationsItem" } ] },
    "Page": { "type": "object", "required": ["id","name","items"],
      "properties": { "id": { "$ref": "#/$defs/Id" }, "name": { "type": "string", "minLength": 1, "maxLength": 30 }, "items": { "type": "array", "maxItems": 15, "items": { "$ref": "#/$defs/KnownItem" } } } },
    "DisplaySettings": { "type": "object", "required": ["dimAfterSec","dimLevel","sleepAfterSec","pixelShift","night"],
      "properties": {
        "dimAfterSec": { "type": "integer", "minimum": 0, "maximum": 3600 },
        "dimLevel": { "type": "number", "minimum": 0.05, "maximum": 1 },
        "sleepAfterSec": { "type": "integer", "minimum": 0, "maximum": 86400 },
        "pixelShift": { "type": "boolean" },
        "night": { "type": "object", "required": ["enabled","from","to"], "properties": { "enabled": { "type": "boolean" },
          "from": { "type": "string", "pattern": "^([01][0-9]|2[0-3]):[0-5][0-9]$" }, "to": { "type": "string", "pattern": "^([01][0-9]|2[0-3]):[0-5][0-9]$" } } } } },
    "ClientLayout": { "type": "object", "required": ["grid","display","pages"],
      "properties": {
        "grid": { "type": "object", "required": ["cols","rows"], "properties": { "cols": { "const": 5 }, "rows": { "const": 3 } } },
        "display": { "$ref": "#/$defs/DisplaySettings" },
        "pages": { "type": "array", "minItems": 1, "maxItems": 10, "items": { "$ref": "#/$defs/Page" } } } },
    "LayoutFile": { "allOf": [ { "$ref": "#/$defs/ClientLayout" }, { "required": ["version"], "properties": { "version": { "const": 1 } } } ] }
  }
}
```

Схема `KnownItem` применяется при валидации **файла на ПК** (строго). Клиент при приёме секции `layout` схему не применяет и неизвестный `kind` рисует заглушкой (PROTOCOL.md §2).

`/shared/schemas/actions.schema.json`:

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://decka.invalid/schemas/actions.schema.json",
  "$defs": {
    "Key": { "type": "string", "pattern": "^(ctrl|alt|shift|win|[a-z]|[0-9]|f([1-9]|1[0-9]|2[0-4])|enter|esc|space|tab|backspace|delete|home|end|pageup|pagedown|up|down|left|right)$" },
    "Base": { "type": "object", "required": ["name","type"], "properties": { "name": { "type": "string", "minLength": 1, "maxLength": 60 }, "toggleVar": { "type": "string", "pattern": "^[a-z0-9_.-]{1,64}$" } } },
    "Hotkey": { "allOf": [ { "$ref": "#/$defs/Base" }, { "required": ["keys"], "properties": { "type": { "const": "hotkey" }, "keys": { "type": "array", "minItems": 1, "maxItems": 5, "uniqueItems": true, "items": { "$ref": "#/$defs/Key" } } } } ] },
    "Media": { "allOf": [ { "$ref": "#/$defs/Base" }, { "required": ["op"], "properties": { "type": { "const": "media" }, "op": { "enum": ["play_pause","next","prev","vol_up","vol_down","mute"] } } } ] },
    "Launch": { "allOf": [ { "$ref": "#/$defs/Base" }, { "required": ["program"], "properties": { "type": { "const": "launch" }, "program": { "type": "string", "minLength": 1, "maxLength": 500 },
      "args": { "type": "array", "maxItems": 32, "items": { "type": "string", "maxLength": 500 } }, "cwd": { "type": "string", "maxLength": 500 }, "hidden": { "type": "boolean" } } } ] },
    "OpenUrl": { "allOf": [ { "$ref": "#/$defs/Base" }, { "required": ["url"], "properties": { "type": { "const": "open_url" }, "url": { "type": "string", "pattern": "^https?://", "maxLength": 2000 } } } ] },
    "System": { "allOf": [ { "$ref": "#/$defs/Base" }, { "required": ["op"], "properties": { "type": { "const": "system" }, "op": { "enum": ["lock","sleep"] } } } ] },
    "Action": { "oneOf": [ { "$ref": "#/$defs/Hotkey" }, { "$ref": "#/$defs/Media" }, { "$ref": "#/$defs/Launch" }, { "$ref": "#/$defs/OpenUrl" }, { "$ref": "#/$defs/System" } ] },
    "ActionsFile": { "type": "object", "required": ["version","actions"],
      "properties": { "version": { "const": 1 }, "actions": { "type": "object", "maxProperties": 200, "propertyNames": { "pattern": "^[a-z0-9_-]{1,64}$" }, "additionalProperties": { "$ref": "#/$defs/Action" } } } }
  }
}
```

## 6. Виджеты (поведение на клиенте)

| kind | Данные | Отображение |
|---|---|---|
| `clock` | локальное время телефона | `HH:MM` (с `seconds: true` — `HH:MM:SS`), под ним дата `ср, 14 ноя` в локали устройства; при 1×1 — только время |
| `sys` | секция `sys` | 2×1: две полосы CPU % и RAM %; 2×2: дополнительно первый диск (% занято) и сеть `↓ rx ↑ tx` (Kbps, при ≥1000 — Mbps с одним знаком). `sys = null` → текст `—` |
| `statuses` | секция `statuses`, фильтр `group` (если задан) | заголовок `title` (если задан), список: цветная точка + `label` + `text` справа; сортировка: error, warn, running, unknown, ok, внутри — по `label`; не помещается → обрезка, последняя строка `+N` |
| `status` | один элемент по `statusId` | плитка: цвет фона по состоянию, `label`, `text`; элемента нет → состояние `unknown` и `label = statusId` |
| `notifications` | секция `notifications` | лента, новые сверху: цветная полоса уровня, `title`, `body` (1 строка), относительное время; тап по элементу → `notif_dismiss` |

Цвета состояний/уровней (фиксированы): ok/success `#16a34a`, warn/warning `#d97706`, error `#dc2626`, running/info `#2563eb`, unknown `#6b7280`. Состояние `running` дополнительно пульсирует (opacity 1↔0.5, период 1.6 с).

Всплывающий тост: каждое новое уведомление (id, которого не было в предыдущем значении секции; уведомления, полученные в `welcome`, тостов не порождают) показывается поверх любой страницы 8 с; уровни `warning` и `error` выводят экран из затемнения/заставки.
