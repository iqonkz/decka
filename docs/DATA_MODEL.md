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
  | (ActionBase & { type: "system"; op: "lock" | "sleep" })
  | (ActionBase & { type: "audio"; op: "mic_toggle" | "out_mute_toggle" })                      // M15
  | (ActionBase & { type: "obs"; op: ObsOp; scene?: string; input?: string })                   // M15
  | (ActionBase & { type: "sequence"; steps: ({ actionId: Id } | { delayMs: number })[] });     // M15

export type MediaOp = "play_pause" | "next" | "prev" | "vol_up" | "vol_down" | "mute" | "shuffle" | "repeat"; // shuffle, repeat — M15
export type ObsOp = "record_toggle" | "stream_toggle" | "scene" | "input_mute_toggle" | "save_replay";
export type KeyName = string; // см. 2.2
```

### 2.2 Клавиши (`KeyName`)

Регистр — только нижний. Допустимый набор (всё остальное — ошибка валидации):

- модификаторы: `ctrl`, `alt`, `shift`, `win`
- буквы `a`..`z`, цифры `0`..`9`
- `f1`..`f24`
- `enter`, `esc`, `space`, `tab`, `backspace`, `delete`, `home`, `end`, `pageup`, `pagedown`, `up`, `down`, `left`, `right`
- M15: `printscreen`, `insert`, `period`, `comma`, `minus`, `equal`, `slash`, `semicolon` (нужны для `win+period` — панель эмодзи, `win+shift+s` уже покрыт буквами)

Исполнение `hotkey`: клавиши нажимаются в порядке массива (press), затем отпускаются в обратном порядке (release). Пример `["ctrl","shift","esc"]`.

### 2.3 Семантика и валидация

| type | Исполнение на Windows | Валидация |
|---|---|---|
| `hotkey` | crate `enigo`, см. 2.2 | `keys`: 1..5, каждая из набора, без повторов |
| `media` | (с T12.0) `play_pause`/`next`/`prev` — сначала через текущую медиасессию Windows (`GlobalSystemMediaTransportControlsSession::TryTogglePlayPauseAsync` / `TrySkipNextAsync` / `TrySkipPreviousAsync`); нет сессии или вызов вернул `false`/ошибку → запасной путь медиаклавишей ниже. Громкость — всегда клавишами. `enigo`: `play_pause`→MediaPlayPause, `next`→MediaNextTrack, `prev`→MediaPrevTrack, `vol_up`→VolumeUp, `vol_down`→VolumeDown, `mute`→VolumeMute (click) | `op` из списка |
| `launch` | `std::process::Command::new(program).args(args)`, **без shell**; `cwd` → `current_dir`; `hidden: true` → флаг создания `CREATE_NO_WINDOW` (0x08000000); процесс не ожидается (spawn и забыть) | `program` 1..500 символов; `args` ≤ 32 элементов по ≤ 500 символов; `cwd` ≤ 500 |
| `open_url` | `rundll32.exe url.dll,FileProtocolHandler <url>` через `Command` без shell | `url` ≤ 2000 символов, парсится crate `url`, схема строго `http` или `https` |
| `system` | `lock` → `rundll32.exe user32.dll,LockWorkStation`; `sleep` → `rundll32.exe powrprof.dll,SetSuspendState 0,1,0` | `op` из списка |
| `media` `shuffle`/`repeat` (M15) | только через GSMTC: `TryChangeShuffleActiveAsync(!IsShuffleActive)`; `TryChangeAutoRepeatModeAsync` по кругу `none → list → track → none`. Нет сессии или источник не поддерживает (`false`) → `ACTION_FAILED`, `message: "The player does not support this"`. Запасного пути клавишей нет | `op` из списка |
| `audio` (M15) | Core Audio (`IMMDeviceEnumerator::GetDefaultAudioEndpoint`, роль `eConsole`; `IAudioEndpointVolume::SetMute(!GetMute)`): `mic_toggle` — устройство записи (`eCapture`), `out_mute_toggle` — устройство вывода (`eRender`). Нет устройства → `ACTION_FAILED` `"No microphone"` / `"No audio output"` | `op` из списка |
| `obs` (M15) | запрос obs-websocket 5 (§6.4): `record_toggle` → `ToggleRecord`; `stream_toggle` → `ToggleStream`; `scene` → `SetCurrentProgramScene {sceneName}`; `input_mute_toggle` → `ToggleInputMute {inputName}`; `save_replay` → `SaveReplayBuffer`. Не подключено → `ACTION_FAILED` `"OBS is not connected"`; ошибка OBS → `ACTION_FAILED` с `comment` из ответа OBS | `scene` 1..100 обязателен при `op: "scene"`, `input` 1..100 — при `input_mute_toggle`, иначе запрещены |
| `sequence` (M15) | шаги по очереди: `actionId` — выполнить то действие (ждать его результата), `delayMs` — пауза; первая ошибка останавливает цепочку → `ACTION_FAILED`, `message: "Step <n>: <ошибка>"`. `toggleVar` — после всех шагов. Нажатие той же кнопки во время выполнения → `ACTION_FAILED` `"Already running"` | 1..10 шагов; `delayMs` 50..10000; сумма пауз ≤ 15000; `actionId` существует и не ссылается на `sequence` (без вложенности) |

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
  grid: { cols: number; rows: number }; // cols 3..8, rows 2..5; по умолчанию 7×4 (M10)
  display: DisplaySettings;
  pages: Page[];                  // 1..10 страниц, порядок = порядок свайпа
}
export interface Page {
  id: Id; name: string; items: LayoutItem[];
  apps?: string[];        // M11: контекстная страница — показывается сама, когда на ПК активно одно из этих приложений (имена exe, нижний регистр)
  popup?: boolean;        // M13: страница-группа — открывается поверх по нажатию кнопки с popup, не участвует в свайпе (DESIGN.md §4.1)
  stayOpen?: boolean;     // M13: только для popup-страниц: не закрывать панель после нажатия кнопки внутри
}

export interface DisplaySettings {
  dimAfterSec: number;    // 0 = не затемнять; иначе 10..3600. По умолчанию 60
  dimLevel: number;       // 0.05..1 — доля яркости в затемнённом режиме. По умолчанию 0.3
  sleepAfterSec: number;  // 0 = без заставки; иначе 30..86400 и > dimAfterSec. По умолчанию 600
  pixelShift: boolean;    // по умолчанию true
  night: { enabled: boolean; from: string; to: string }; // "HH:MM", локальное время телефона
  contextPages?: boolean; // M11: автопереключение на контекстные страницы; отсутствует = true
  theme?: "amoled" | "studio" | "material" | "glass";       // M13: DESIGN.md §1; отсутствует = "amoled"; новые установки — "studio"
  accent?: string;        // M13: "#rrggbb", акцент темы Material и окна настроек; отсутствует = "#7c5cff"
  wallpaper?: "aurora" | "sunset" | "ocean" | "graphite" | "album"; // M13: только тема glass; отсутствует = "aurora"
  keyFace?: "standard" | "full";   // M14: стиль кнопок по умолчанию, DESIGN.md §2.1; отсутствует = "standard"
  keyLabels?: boolean;             // M14: показывать подписи кнопок по умолчанию; отсутствует = true
  language?: "auto" | "en" | "ru"; // M14: язык интерфейса, DESIGN.md §6; отсутствует = "auto"
  orientation?: "landscape" | "portrait"; // M14: ориентация телефона, §3.7; отсутствует = "landscape"
}

interface ItemBase { id: Id; x: number; y: number; w: number; h: number; }

export interface ButtonLook { label?: string; icon?: string; color?: string; }
export interface ButtonItem extends ItemBase, ButtonLook {
  kind: "button";
  actionId?: Id;          // M13: допустимые сочетания — actionId | navigate | popup | actionId+popup
  navigate?: Id | "$back"; // id обычной (не popup) страницы; M15: "$back" — на предыдущую страницу (§3.8)
  popup?: Id;             // M13: id popup-страницы
  confirm?: boolean;      // true → нажатие удержанием 800 мс
  bind?: { var: VarName; states: Record<string, ButtonLook> }; // ключ = String(значение переменной)
  face?: "standard" | "full";   // M14: переопределяет display.keyFace для этой кнопки
  showLabel?: boolean;          // M14: переопределяет display.keyLabels; подпись видна, если label непустой и (showLabel ?? display.keyLabels ?? true)
}
export interface ClockItem    extends ItemBase { kind: "clock"; seconds?: boolean; }
export interface SysItem      extends ItemBase { kind: "sys"; graph?: boolean; }   // M15: graph — график последних 2 минут (§6.5)
export interface StatusesItem extends ItemBase { kind: "statuses"; group?: string; title?: string; }
export interface StatusItemTile extends ItemBase { kind: "status"; statusId: Id; }
export interface NotificationsItem extends ItemBase { kind: "notifications"; }

export interface MediaItem extends ItemBase {   // M11: «Сейчас играет»
  kind: "media";
  prevAction?: Id; playAction?: Id; nextAction?: Id;  // действия кнопок виджета; отсутствует → кнопка не рисуется
}

// M15 (§6.3–§6.7)
export interface VolumeItem  extends ItemBase { kind: "volume"; upAction?: Id; downAction?: Id; muteAction?: Id; } // по умолчанию vol-up / vol-down / vol-mute, если существуют
export interface TimerItem   extends ItemBase { kind: "timer"; presets?: number[]; }   // минуты, 1..180, 1..4 значения; по умолчанию [25, 5]
export interface WeatherItem extends ItemBase { kind: "weather"; }
export interface ObsItem     extends ItemBase { kind: "obs"; }

export type LayoutItem = ButtonItem | ClockItem | SysItem | StatusesItem | StatusItemTile | NotificationsItem | MediaItem
  | VolumeItem | TimerItem | WeatherItem | ObsItem;

// То, что уходит на телефон в секции "layout": LayoutFile без поля version.
export type ClientLayout = Omit<LayoutFile, "version">;
```

Телефон получает из раскладки `actionId`, но никогда не получает содержимое действия (тип, программу, клавиши).

### 3.2 Валидация раскладки

1. `grid.cols` — целое 3..8, `grid.rows` — целое 2..5. **M14:** при `display.orientation = "portrait"` — `cols` 2..5, `rows` 3..8 (JSON-схема допускает объединение 2..8 по обеим осям, точная проверка по ориентации — в валидаторах TS и Rust, `path: "/grid"`, `message: "Grid <c>×<r> does not fit portrait (2–5 columns, 3–8 rows)"` / `… landscape (3–8 columns, 2–5 rows)`).
2. `pages`: 1..10; `id` уникальны; `name` 1..30 символов.
3. Каждый элемент: `0 ≤ x`, `0 ≤ y`, `w ≥ 1`, `h ≥ 1`, `x + w ≤ grid.cols`, `y + h ≤ grid.rows`; элементы одной страницы не пересекаются; `id` уникальны в пределах всего файла.
4. Размеры по видам (w×h): `button` — только 1×1; `clock` — 1×1, 2×1; `sys` — 2×1, 2×2; `status` — 1×1; `statuses` — от 2×1 до `cols×rows`; `notifications` — от 2×1 до `cols×rows`.

Смена размера сетки не переставляет элементы: если после уменьшения сетки элементы не помещаются, это обычная ошибка валидации п. 3, и раскладку нельзя сохранить, пока пользователь их не переместит.
5. `button`: ровно одно из `actionId`/`navigate`; `actionId` должен существовать в `actions.json`; `navigate` — существующий `id` страницы; `confirm` допустим только с `actionId`.
6. `label` ≤ 24 символов; `color` — regex `^#[0-9a-fA-F]{6}$`; `icon` — §3.3.
7. `bind.states`: 1..8 ключей, каждый ключ ≤ 40 символов.
8. `display`: диапазоны из комментариев в §3.1; `night.from`/`to` — regex `^([01][0-9]|2[0-3]):[0-5][0-9]$`.
9. (M11) `apps`: 0..10 элементов, каждый — regex `^[a-z0-9._ ()-]{1,60}\.exe$`, без повторов внутри страницы; одно приложение может быть указано только на одной странице файла (иначе ошибка на второй странице: `/pages/<i>/apps/<j>`). Первая страница (`pages[0]`) не может иметь `apps` — она домашняя по умолчанию.
9a. (M13, правило 12) Кнопка: допустимы только сочетания `actionId`, `navigate`, `popup`, `actionId + popup` (иначе ошибка с текстом T11.9). `navigate` указывает на существующую страницу без `popup`; `popup` — на существующую страницу с `popup: true`. Страница с `popup: true` не может быть первой, не может иметь `apps`, её кнопки не могут иметь `popup` и `navigate` (из панели нельзя уйти на другую страницу). `stayOpen` допустим только при `popup: true`. `display.accent` — regex `^#[0-9a-fA-F]{6}$`. Правило 5 (ровно одно из `actionId`/`navigate`) заменяется этим правилом.
10. (M11) `media`: размер от 2×1 до `cols×2`; `prevAction`/`playAction`/`nextAction`, если заданы, должны существовать в `actions.json` (как `actionId` кнопки).
13. (M15) Размеры: `volume` — 1×2..1×4 (вертикальный) или 2×1..4×1 (горизонтальный); `timer` — 1×1, 2×1, 2×2; `weather` — 1×1, 2×1, 3×2, 4×2; `obs` — 2×1..4×2; `sys` с `graph: true` — 2×1..4×2. `upAction`/`downAction`/`muteAction` у `volume` — существующие действия. `navigate: "$back"` допустим только у кнопки обычной страницы. `timer.presets` — 1..4 целых 1..180.
11. (M12) см. §8.2.

Проверки 1–4, 6–8 выражены в JSON Schema там, где это возможно (§5); пересечения, уникальность, ссылки (п. 3, 5) проверяются кодом — одинаково в Rust (`decka-core::config::validate_layout`) и в TS (`shared/src/validate.ts`, используется редактором в окне настроек).

Если `actions.json` изменился так, что кнопка ссылается на удалённое действие: раскладка остаётся в силе, кнопка рисуется, нажатие вернёт `UNKNOWN_ACTION`; в окне настроек показывается предупреждение.

### 3.3 Иконки

Поле `icon` — строка одного из двух видов:
- `lucide:<name>` — имя иконки Lucide в kebab-case (`lucide:play`, `lucide:volume-2`). Regex `^lucide:[a-z0-9-]{1,40}$`. Клиент включает в сборку **фиксированный набор** из 60 иконок (список — §3.6); неизвестное имя → иконка `lucide:square`.
- `emoji:<символ>` — один emoji (`emoji:🚀`), ≤ 8 UTF-16 единиц после префикса.
- (M12) `img:<hash16>` — картинка из хранилища иконок агента (§8); regex `^img:[0-9a-f]{16}$`. Клиент берёт data URL из секции `icons`; нет в секции → иконка `lucide:square`.

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
  "grid": { "cols": 7, "rows": 4 },
  "display": { "dimAfterSec": 60, "dimLevel": 0.3, "sleepAfterSec": 600, "pixelShift": true, "night": { "enabled": false, "from": "23:00", "to": "07:00" } },
  "pages": [
    { "id": "main", "name": "Main", "items": [
      { "id": "w-clock", "kind": "clock", "x": 0, "y": 0, "w": 2, "h": 1 },
      { "id": "w-sys", "kind": "sys", "x": 2, "y": 0, "w": 2, "h": 2 },
      { "id": "w-notifs", "kind": "notifications", "x": 4, "y": 0, "w": 3, "h": 2 },
      { "id": "w-services", "kind": "statuses", "x": 0, "y": 1, "w": 2, "h": 1, "title": "Services" },
      { "id": "b-prev", "kind": "button", "x": 0, "y": 2, "w": 1, "h": 1, "icon": "lucide:skip-back", "actionId": "media-prev" },
      { "id": "b-play", "kind": "button", "x": 1, "y": 2, "w": 1, "h": 1, "icon": "lucide:play", "actionId": "media-play" },
      { "id": "b-next", "kind": "button", "x": 2, "y": 2, "w": 1, "h": 1, "icon": "lucide:skip-forward", "actionId": "media-next" },
      { "id": "b-mute", "kind": "button", "x": 3, "y": 2, "w": 1, "h": 1, "icon": "lucide:volume", "actionId": "vol-mute",
        "bind": { "var": "audio.muted", "states": { "true": { "icon": "lucide:volume-x", "color": "#b45309" } } } },
      { "id": "b-voldown", "kind": "button", "x": 4, "y": 2, "w": 1, "h": 1, "icon": "lucide:volume-1", "actionId": "vol-down" },
      { "id": "b-volup", "kind": "button", "x": 5, "y": 2, "w": 1, "h": 1, "icon": "lucide:volume-2", "actionId": "vol-up" },
      { "id": "b-lock", "kind": "button", "x": 6, "y": 2, "w": 1, "h": 1, "label": "Lock", "icon": "lucide:lock", "color": "#b91c1c", "actionId": "lock", "confirm": true },
      { "id": "b-m-tasks", "kind": "button", "x": 0, "y": 3, "w": 1, "h": 1, "label": "Tasks", "icon": "lucide:activity", "actionId": "task-manager" },
      { "id": "b-m-notepad", "kind": "button", "x": 1, "y": 3, "w": 1, "h": 1, "label": "Notepad", "icon": "lucide:file-text", "actionId": "notepad" },
      { "id": "b-m-github", "kind": "button", "x": 2, "y": 3, "w": 1, "h": 1, "label": "GitHub", "icon": "lucide:globe", "actionId": "github" },
      { "id": "b-to-dev", "kind": "button", "x": 6, "y": 3, "w": 1, "h": 1, "label": "Dev", "icon": "lucide:code", "navigate": "dev" }
    ] },
    { "id": "dev", "name": "Dev", "items": [
      { "id": "w-statuses", "kind": "statuses", "x": 0, "y": 0, "w": 4, "h": 4, "title": "Services" },
      { "id": "b-github", "kind": "button", "x": 4, "y": 0, "w": 1, "h": 1, "label": "GitHub", "icon": "lucide:globe", "actionId": "github" },
      { "id": "b-notepad", "kind": "button", "x": 5, "y": 0, "w": 1, "h": 1, "label": "Notepad", "icon": "lucide:file-text", "actionId": "notepad" },
      { "id": "b-taskmgr", "kind": "button", "x": 6, "y": 0, "w": 1, "h": 1, "label": "Tasks", "icon": "lucide:activity", "actionId": "task-manager" },
      { "id": "b-home", "kind": "button", "x": 6, "y": 3, "w": 1, "h": 1, "label": "Main", "icon": "lucide:home", "navigate": "main" }
    ] }
  ]
}
```

### 3.6 Набор иконок Lucide в сборке клиента (60)

`play, pause, skip-back, skip-forward, volume, volume-1, volume-2, volume-x, mic, mic-off, video, video-off, monitor, lock, power, moon, sun, home, settings, terminal, code, link, git-branch, globe, folder, file-text, activity, cpu, hard-drive, wifi, bell, bell-off, mail, message-square, music, headphones, camera, image, calendar, clock, timer, star, heart, zap, rocket, bug, check, x, plus, minus, refresh-cw, search, download, upload, copy, trash-2, square, circle, arrow-left, arrow-right`

Брендовых иконок (github и т. п.) в `lucide-react` 1.x нет — в набор они не входят. Если имя в установленной версии `lucide-react` отсутствует (переименовано), исполнитель заменяет его на `square` и перечисляет замены в описании коммита — список в этом файле не меняется.

### 3.7 Ориентация (M14, решение владельца 2026-11-15)

- `display.orientation` — одна на раскладку (телефон стоит на столе в одном положении; поворот по датчику не поддерживается). Телефон блокирует выбранную ориентацию; все страницы используют одну сетку `grid`.
- **Смена ориентации в окне** (`Look → Orientation`: `Landscape` / `Portrait`, карточки с силуэтом телефона) всегда идёт через `rotateLayout(layout, to)` — чистая функция в `shared`, результат проходит валидацию:
  1. Новая сетка — транспонированная: `cols' = rows`, `rows' = cols`, затем зажатая в пределы новой ориентации (§3.2 п. 1). Пример: 7×4 → 4×7; 8×5 → 5×8; 3×2 → 2×3.
  2. **Пересмотрено 2026-11-16 (жалоба владельца: «кнопки перемешиваются хаотично»; транспонирование отменено).** Раскладка перетекает в новую сетку **с сохранением порядка чтения**. Сначала виджеты — в порядке `(y, x)` исходной страницы, размер сохраняется (ширина зажимается в `cols'`, высота — в `rows'`, не меньше минимума вида; горизонтальный `volume` в книжной становится вертикальным: `w ↔ h`), каждый — в первую свободную позицию при обходе сверху вниз, слева направо.
  3. Затем кнопки — в порядке `(y, x)` исходной страницы, каждая в первую свободную клетку того же обхода. Соседние кнопки (например `prev`, `play`, `next`) остаются соседями и идут в том же порядке. Обратный поворот не обязан вернуть исходную раскладку — для этого `Ctrl+Z`.
  4. Элементы, которым не нашлось места, переносятся на новую страницу `<имя страницы> 2` (без `apps` — правило 9 §3.2 — и без `popup`; `apps` остаются у исходной страницы), сразу после исходной; popup-страница при переполнении получает ошибку «не помещается», и ориентация не меняется — окно показывает `Popup group "<name>" doesn't fit. Make it smaller first.`.
  5. (Отменено 2026-11-16 — порядок теперь задан пунктами 2–3.)
- Перед применением окно показывает превью результата и число перенесённых элементов: `3 items moved to new pages` с кнопками `Rotate` / `Cancel`. `Ctrl+Z` отменяет поворот целиком.
- Шаблоны контекстных страниц (§7): `minGrid` сравнивается с сеткой после транспонирования шаблона, если ориентация `portrait` (шаблон добавляется через `rotateLayout` его страницы).

### 3.8 Кнопка «Назад» (M15)

`navigate: "$back"` открывает страницу, с которой пользователь пришёл последним **ручным** переходом (свайп, кнопка `navigate`). Клиент хранит стек из ≤ 10 id страниц; автоматическая (контекстная) смена страницы в стек не пишется; пустой стек → первая страница. Иконка по умолчанию — `lucide:arrow-left`, подпись `Back` / `Назад`.

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
    focus?: { enabled: boolean };                     // M11; отсутствует = { true }
    media?: { enabled: boolean };                     // M11; отсутствует = { true }
    audio?: { enabled: boolean };                     // M15 (§6.6); отсутствует = { true }
  };
  updateCheck?: boolean;             // M14: проверка новых релизов на GitHub, ARCHITECTURE.md §8a; отсутствует = true
  weather?: { enabled: boolean; place: string; lat: number; lon: number; units: "metric" | "imperial" }; // M15 (§6.3); отсутствует = выключено
  obs?: { enabled: boolean; port: number; password?: string };  // M15 (§6.4); port по умолчанию 4455; password — только "dpapi:<base64>", пишет его лишь set_obs_password
}
export interface HttpCheck {
  id: Id; label: string;             // label 1..60
  url: string;                       // http/https, <= 2000
  intervalSec: number;               // 10..3600
  timeoutSec: number;                // 1..30
  expectStatus?: number;             // по умолчанию: любой 200..399 = ok
  group?: string;                    // <= 40
}

export interface DeviceInfo { id: Id; name: string; createdAt: number; lastSeenAt: number; connected: boolean; appVersion?: string; } // appVersion — M14: client.version из последнего pair/hello
// devices.json на диске:
// { "version": 1, "devices": [ { "id", "name", "tokenHash", "createdAt", "lastSeenAt", "appVersion"? } ] }
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
    "Icon": { "type": "string", "pattern": "^(lucide:[a-z0-9-]{1,40}|emoji:.{1,8}|img:[0-9a-f]{16})$" },
    "Color": { "type": "string", "pattern": "^#[0-9a-fA-F]{6}$" },
    "ButtonLook": { "type": "object", "properties": { "label": { "type": "string", "maxLength": 24 }, "icon": { "$ref": "#/$defs/Icon" }, "color": { "$ref": "#/$defs/Color" } } },
    "ItemBase": { "type": "object", "required": ["id","kind","x","y","w","h"],
      "properties": { "id": { "$ref": "#/$defs/Id" }, "kind": { "type": "string" },
        "x": { "type": "integer", "minimum": 0, "maximum": 7 }, "y": { "type": "integer", "minimum": 0, "maximum": 4 },
        "w": { "type": "integer", "minimum": 1, "maximum": 8 }, "h": { "type": "integer", "minimum": 1, "maximum": 5 } } },
    "ButtonItem": { "allOf": [ { "$ref": "#/$defs/ItemBase" }, { "$ref": "#/$defs/ButtonLook" }, {
      "properties": { "kind": { "const": "button" }, "w": { "const": 1 }, "h": { "const": 1 },
        "actionId": { "$ref": "#/$defs/Id" }, "navigate": { "$ref": "#/$defs/Id" }, "popup": { "$ref": "#/$defs/Id" }, "confirm": { "type": "boolean" },
        "bind": { "type": "object", "required": ["var","states"], "properties": {
          "var": { "type": "string", "pattern": "^[a-z0-9_.-]{1,64}$" },
          "states": { "type": "object", "minProperties": 1, "maxProperties": 8, "propertyNames": { "maxLength": 40 }, "additionalProperties": { "$ref": "#/$defs/ButtonLook" } } } } },
      "anyOf": [ { "required": ["actionId"] }, { "required": ["navigate"] }, { "required": ["popup"] } ] } ] },
    "ClockItem": { "allOf": [ { "$ref": "#/$defs/ItemBase" }, { "properties": { "kind": { "const": "clock" }, "w": { "maximum": 2 }, "h": { "const": 1 }, "seconds": { "type": "boolean" } } } ] },
    "SysItem": { "allOf": [ { "$ref": "#/$defs/ItemBase" }, { "properties": { "kind": { "const": "sys" }, "w": { "const": 2 }, "h": { "maximum": 2 } } } ] },
    "StatusesItem": { "allOf": [ { "$ref": "#/$defs/ItemBase" }, { "properties": { "kind": { "const": "statuses" }, "w": { "minimum": 2 }, "group": { "type": "string", "maxLength": 40 }, "title": { "type": "string", "maxLength": 30 } } } ] },
    "StatusTile": { "allOf": [ { "$ref": "#/$defs/ItemBase" }, { "required": ["statusId"], "properties": { "kind": { "const": "status" }, "w": { "const": 1 }, "h": { "const": 1 }, "statusId": { "$ref": "#/$defs/Id" } } } ] },
    "NotificationsItem": { "allOf": [ { "$ref": "#/$defs/ItemBase" }, { "properties": { "kind": { "const": "notifications" }, "w": { "minimum": 2 } } } ] },
    "MediaItem": { "allOf": [ { "$ref": "#/$defs/ItemBase" }, { "properties": { "kind": { "const": "media" }, "w": { "minimum": 2 }, "h": { "maximum": 2 },
      "prevAction": { "$ref": "#/$defs/Id" }, "playAction": { "$ref": "#/$defs/Id" }, "nextAction": { "$ref": "#/$defs/Id" } } } ] },
    "KnownItem": { "oneOf": [ { "$ref": "#/$defs/ButtonItem" }, { "$ref": "#/$defs/ClockItem" }, { "$ref": "#/$defs/SysItem" }, { "$ref": "#/$defs/StatusesItem" }, { "$ref": "#/$defs/StatusTile" }, { "$ref": "#/$defs/NotificationsItem" }, { "$ref": "#/$defs/MediaItem" } ] },
    "Page": { "type": "object", "required": ["id","name","items"],
      "properties": { "id": { "$ref": "#/$defs/Id" }, "name": { "type": "string", "minLength": 1, "maxLength": 30 }, "items": { "type": "array", "maxItems": 40, "items": { "$ref": "#/$defs/KnownItem" } },
        "apps": { "type": "array", "maxItems": 10, "uniqueItems": true, "items": { "type": "string", "pattern": "^[a-z0-9._ ()-]{1,60}\\.exe$" } },
        "popup": { "type": "boolean" }, "stayOpen": { "type": "boolean" } } },
    "DisplaySettings": { "type": "object", "required": ["dimAfterSec","dimLevel","sleepAfterSec","pixelShift","night"],
      "properties": {
        "dimAfterSec": { "type": "integer", "minimum": 0, "maximum": 3600 },
        "dimLevel": { "type": "number", "minimum": 0.05, "maximum": 1 },
        "sleepAfterSec": { "type": "integer", "minimum": 0, "maximum": 86400 },
        "pixelShift": { "type": "boolean" },
        "contextPages": { "type": "boolean" },
        "theme": { "enum": ["amoled","studio","material","glass"] },
        "accent": { "type": "string", "pattern": "^#[0-9a-fA-F]{6}$" },
        "wallpaper": { "enum": ["aurora","sunset","ocean","graphite","album"] },
        "night": { "type": "object", "required": ["enabled","from","to"], "properties": { "enabled": { "type": "boolean" },
          "from": { "type": "string", "pattern": "^([01][0-9]|2[0-3]):[0-5][0-9]$" }, "to": { "type": "string", "pattern": "^([01][0-9]|2[0-3]):[0-5][0-9]$" } } } } },
    "ClientLayout": { "type": "object", "required": ["grid","display","pages"],
      "properties": {
        "grid": { "type": "object", "required": ["cols","rows"], "properties": { "cols": { "type": "integer", "minimum": 2, "maximum": 8 }, "rows": { "type": "integer", "minimum": 2, "maximum": 8 } } },
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
| `media` (M11) | секция `media` | «Сейчас играет», см. §6.2 |

Цвета состояний/уровней (фиксированы): ok/success `#16a34a`, warn/warning `#d97706`, error `#dc2626`, running/info `#2563eb`, unknown `#6b7280`. Состояние `running` дополнительно пульсирует (opacity 1↔0.5, период 1.6 с).

### 6.1 Контекстные страницы (M11)

Клиент хранит: `homePageIndex` — страница, выбранная пользователем вручную (свайп, кнопка `navigate`, старт = 0); `autoPageId` — id страницы, открытой автоматически, или `null`; `lastTouch` — время последнего `pointerdown`.

Чистая функция `decideContextPage(input) → { pageIndex, autoPageId, hint: string | null }` вызывается при изменении `focus`, при изменении `layout` и раз в 1 с:
1. `display.contextPages === false` → ничего не менять.
2. `now − lastTouch < 5000` → ничего не менять (переключение отложено до следующего вызова).
3. `target` = индекс страницы, у которой `apps` содержит `focus.app` (нет такой или `focus` = `null` → `target = null`).
4. `target !== null` и `target !== pageIndex` → `pageIndex = target`, `autoPageId = pages[target].id`, `hint = pages[target].name`.
5. `target === null`, `autoPageId !== null` и текущая страница — это `autoPageId` → `pageIndex = homePageIndex`, `autoPageId = null`.
6. Иначе — без изменений.

Ручная навигация: `homePageIndex = новый индекс`, `autoPageId = null`. Подсказка `hint` — имя страницы внизу по центру, 2.4vmin, `#6b7280`, видна 2 с. В режимах `dim`/`sleep` переключение происходит, но экран не будит.

### 6.2 Виджет `media` (M11)

- Слева — обложка: квадрат со стороной `высота виджета − 2·отступ`, радиус 12%, непрозрачность 0.85; без `art` — клавиша с обводкой (как у кнопки) и иконкой `lucide:music`.
- Справа сверху: `title` (2.8vmin, жирный, многоточие), под ним `artist` (2.4vmin, `#9ca3af`, многоточие). При `h = 2` — до двух строк названия.
- Полоса прогресса (если есть `durationSec`): высота 0.5vmin, фон `#26262c`, заполнение `#e5e5e5` с непрозрачностью 0.6; позиция вычисляется на клиенте (PROTOCOL.md §4.5, «Секции M11»), обновление раз в 1 с только при `playing`.
- Кнопки управления — клавиши в стиле кнопок (T10.2, T10.5) со стороной `0.55 ×` высоты одной ячейки: при `h = 1` — `prev`, `play`, `next` только при `w ≥ 4`, при `w ≤ 3` — только `play` (решение 2026-11-15: при `w = 3` три клавиши оставляли названию 1–2 символа); при `h = 2` — три клавиши при `w ≥ 2`. Текстовому блоку всегда остаётся не меньше `1.5 × side` ширины; если не остаётся — клавиши убираются, начиная с `prev`/`next`. Иконки: `skip-back`, `pause` (если `playing`) или `play`, `skip-forward`. Нажатие → `press(prevAction|playAction|nextAction)` с теми же локальными состояниями, что у кнопки. Действие не задано → клавиша не рисуется. При `h = 1` клавиши справа от текста, при `h = 2` — под текстом.
- `media = null` → обложка-заглушка и текст `Nothing playing` (`#6b7280`), клавиши видны.

Всплывающий тост: каждое новое уведомление (id, которого не было в предыдущем значении секции; уведомления, полученные в `welcome`, тостов не порождают) показывается поверх любой страницы 8 с; уровни `warning` и `error` выводят экран из затемнения/заставки.

### 6.3 Погода (M15)

- **Источник:** Open-Meteo (без ключа). Запросы делает **только агент**, телефон в интернет не ходит. Прогноз: `GET https://api.open-meteo.com/v1/forecast?latitude=<lat>&longitude=<lon>&current=temperature_2m,weather_code,is_day,wind_speed_10m&daily=weather_code,temperature_2m_max,temperature_2m_min&forecast_days=3&timezone=auto` (+ `&temperature_unit=fahrenheit&wind_speed_unit=mph` для `imperial`). Координаты округляются до 2 знаков (≈ 1 км) — точнее не отправляем. Геокодинг (только из окна, `search_place`): `GET https://geocoding-api.open-meteo.com/v1/search?name=<query>&count=5&language=<en|ru>`. Хосты вшиты в код, таймаут 10 с, `User-Agent: decka-agent/<версия>`.
- **Расписание:** при старте, раз в 30 мин, при сохранении настроек погоды, по `refresh_weather`. Ошибка — секция не меняется; если данным больше 3 ч — `weather = null`.
- **Настройка:** `More → Integrations → Weather`: переключатель, поле поиска города (результаты списком: `Almaty, Kazakhstan`), единицы `°C` / `°F`. Подпись под блоком: `Weather data by Open-Meteo.com (CC BY 4.0)` — обязательна.
- **Виджет:** `1×1` — иконка и температура; `2×1` — иконка, температура, `min / max` сегодня, место; `3×2`/`4×2` — сверху «сейчас», снизу 3 дня (день недели по языку интерфейса, иконка, `max / min`). `weather = null` → иконка `cloud` и `—`. Иконки по коду WMO — таблица DESIGN.md §2.3.

### 6.4 OBS Studio (M15)

- **Подключение:** агент — клиент obs-websocket 5 (встроен в OBS 28+), адрес строго `ws://127.0.0.1:<port>` (другие хосты не поддерживаются), `tokio-tungstenite` 0.30.0 (уже в зависимостях). Рукопожатие: `Hello` (op 0) → если есть `authentication {challenge, salt}`: `secret = base64(sha256(password + salt))`, `auth = base64(sha256(secret + challenge))` → `Identify` (op 1) `{ rpcVersion: 1, authentication?, eventSubscriptions: 4 | 64 }` (Scenes | Outputs). После `Identified` — запросы `GetRecordStatus`, `GetStreamStatus`, `GetCurrentProgramScene`, `GetReplayBufferStatus`; события `RecordStateChanged`, `StreamStateChanged`, `CurrentProgramSceneChanged`, `ReplayBufferStateChanged` обновляют секцию `obs`. `recordStartedAt`/`streamStartedAt` = сейчас − `outputDuration` из статуса.
- **Переподключение:** OBS не запущен — повтор через 5, 10, 20, 40, 60, 60… с, без уведомлений; `connected: false`. Неверный пароль (закрытие с кодом 4009) — повторов нет до смены пароля, уведомление `OBS: wrong password` один раз.
- **Пароль:** хранится в `config.json` только как `dpapi:<base64>` (`CryptProtectData`, область текущего пользователя Windows). Пишет его только `set_obs_password`; `get_config` возвращает `obs` без поля `password` и с `passwordSet: boolean`; `save_config` поле `password` игнорирует и сохраняет прежнее. В логи, протокол и на телефон пароль не попадает (ARCHITECTURE.md S17).
- **Переменные** (секция `vars`, для `bind` кнопок): `obs.connected`, `obs.recording`, `obs.streaming` (bool), `obs.scene` (строка).
- **Настройка:** `More → Integrations → OBS Studio`: переключатель, порт, пароль (`Set password…`), статус `Connected` / `OBS is not running` / `Wrong password`, подсказка `In OBS: Tools → WebSocket Server Settings → Enable`.
- **Виджет `obs`:** точка подключения; `REC 00:12:34` (красная точка, мигает 1 раз в с; `reduced` — не мигает) или `Not recording`; `LIVE 01:02:03` или `Offline`; текущая сцена. `2×1` — запись и эфир в строку; `≥ 3×1` — плюс сцена; `h = 2` — каждая строка крупнее.

### 6.5 График нагрузки (M15)

`sys` с `graph: true`: клиент хранит последние 60 значений `cpu` и `ram` (≈ 2 мин при опросе 2 с), очищает при переподключении. Линия толщиной 1.5 px, под ней заливка цветом линии с непрозрачностью 0.15; CPU — `#3b82f6`, RAM — `#a855f7` (для Material — акцент и вторичный тон). Текущие значения цифрами справа сверху. Сеть и диск — как раньше, строкой снизу при `h = 2`.

### 6.6 Звук: сборщик и ползунок (M15)

- **Сборщик `audio`** (агент): раз в 500 мс читает Core Audio — громкость устройства вывода по умолчанию (`GetMasterVolumeLevelScalar`, 0..100 целым), mute вывода и mute микрофона по умолчанию. Публикует `vars`: `audio.volume`, `audio.muted`, `mic.muted`; изменения рассылаются, только если значение поменялось. Нет устройства → переменная удаляется.
- **Сборщик `media`** дополнительно публикует `media.playing`, `media.shuffle`, `media.repeat` (переменные для `bind`).
- **Ползунок `volume`** — телефон по-прежнему шлёт **только `press` с `actionId`** (правило безопасности): перетаскивание по дорожке переводится в число шагов по 2% (шаг клавиши громкости Windows) и отправляется нажатиями `upAction`/`downAction`, не чаще 15 в секунду; во время перетаскивания ползунок показывает ожидаемое значение, после отпускания через 600 мс — значение `audio.volume`. Тап по значку динамика → `muteAction`. Нет переменной `audio.volume` → ползунок серый, работают только `+` / `−` по тапу на концах.

### 6.7 Таймер (M15)

Целиком на телефоне, ПК не участвует. Тап — старт/пауза; удержание 0,8 с — сброс; кнопки пресетов (`25`, `5`) при `w ≥ 2` выбирают длительность. Окончание — виброотклик `confirm` ×3, тост `Timer done` / `Таймер закончился`, вспышка рамки виджета 3 раза, экран выходит из затемнения. Состояние живёт в памяти приложения (перезапуск сбрасывает).

## 7. Шаблоны контекстных страниц (M11)

Файлы `shared/templates/<id>.json`, встраиваются в агент через `include_str!`.

```ts
// /shared/src/templates.ts
export interface PageTemplate {
  id: Id;                              // он же предпочтительный id страницы
  name: string;                        // имя страницы
  apps: string[];                      // приложения контекста
  minGrid: { cols: number; rows: number };
  actions: Record<Id, Action>;         // id действий начинаются с "tpl-<id>-"
  items: LayoutItem[];                 // navigate: "$home" заменяется на pages[0].id
}
```

Алгоритм `add_template(templateId)` (агент, Rust):
1. Нет шаблона → ошибка `path: "/templates"`, `message: "unknown template"`.
2. `grid` меньше `minGrid` по любой оси → ошибка `path: "/grid"`, `message: "Template needs a <cols>×<rows> grid or larger"`.
3. Любое приложение из `apps` уже указано на странице → ошибка `path: "/pages/<i>/apps"`, `message: "<app> is already used by page <name>"`.
4. Страниц уже 10 → ошибка `path: "/pages"`, `message: "Page limit reached"`.
5. id страницы: `template.id`; занят → `template.id-2`, `-3`, …
6. Действия: добавить отсутствующие по id; существующие с тем же id не трогать.
7. Элементы: id каждого = `t-<pageId>-<исходный id>`; `navigate: "$home"` → `pages[0].id`.
8. Новая страница добавляется в конец. Полная валидация (`validate_actions`, `validate_layout`) → при замечаниях ничего не сохранять и вернуть их; иначе сохранить `actions.json`, затем `layout.json` (как `save_actions`/`save_layout`).

Содержимое шаблонов (раскладка под 7×4; клавиши из набора §2.2). Сочетания клавиш приложений взяты из их стандартных раскладок; помеченные `[unverified]` исполнитель не проверяет — владелец проверит вручную.

**`vscode`** — `name: "VS Code"`, `apps: ["code.exe"]`, `minGrid: 7×4`.

| Действие (id без префикса `tpl-vscode-`) | name | keys | Элемент: x,y | label | icon |
|---|---|---|---|---|---|
| `palette` | Command palette | ctrl shift p | 0,0 | Palette | terminal |
| `quickopen` | Quick open | ctrl p | 1,0 | Open | search |
| `findall` | Find in files | ctrl shift f | 2,0 | Find | folder |
| `git` | Source control | ctrl shift g | 3,0 | Git | git-branch |
| `sidebar` | Toggle sidebar | ctrl b | 4,0 | Sidebar | arrow-left |
| `save` | Save | ctrl s | 0,1 | Save | check |
| `format` | Format document | shift alt f | 1,1 | Format | code |
| `run` | Start debugging | f5 | 0,2 | Run | play |
| `stop` | Stop debugging | shift f5 | 1,2 | Stop | x |
| — | — | — | 6,3 | Home | home (`navigate: "$home"`) |

**`browser`** — `name: "Browser"`, `apps: ["chrome.exe", "msedge.exe"]`, `minGrid: 7×4`.

| id (`tpl-browser-`) | name | keys | x,y | label | icon |
|---|---|---|---|---|---|
| `newtab` | New tab | ctrl t | 0,0 | New tab | plus |
| `closetab` | Close tab | ctrl w | 1,0 | Close | x |
| `reopen` | Reopen closed tab | ctrl shift t | 2,0 | Reopen | refresh-cw |
| `back` | Back | alt left | 0,1 | Back | arrow-left |
| `forward` | Forward | alt right | 1,1 | Forward | arrow-right |
| `reload` | Reload | f5 | 2,1 | Reload | refresh-cw |
| `devtools` | DevTools | f12 | 4,0 | DevTools | bug |
| `private` | Private window | ctrl shift n | 5,0 | Private | moon |
| — | — | — | 6,3 | Home | home (`navigate: "$home"`) |

**`zoom`** — `name: "Zoom"`, `apps: ["zoom.exe"]`, `minGrid: 7×4`. Сочетания `[unverified]`.

| id (`tpl-zoom-`) | name | keys | x,y | label | icon | прочее |
|---|---|---|---|---|---|---|
| `mute` | Mute / unmute | alt a | 0,0 | Mic | mic-off | — |
| `video` | Start / stop video | alt v | 1,0 | Camera | video-off | — |
| `share` | Share screen | alt s | 2,0 | Share | monitor | — |
| `leave` | Leave meeting | alt q | 6,0 | Leave | power | `color: "#b91c1c"`, `confirm: true` |
| — | — | — | 6,3 | Home | home | `navigate: "$home"` |

**`spotify`** — `name: "Spotify"`, `apps: ["spotify.exe"]`, `minGrid: 7×4`.

| id (`tpl-spotify-`) | name | тип | x,y | label | icon |
|---|---|---|---|---|---|
| `prev` | Previous | media `prev` | — | — | — |
| `play` | Play / Pause | media `play_pause` | — | — | — |
| `next` | Next | media `next` | — | — | — |
| `voldown` | Volume down | media `vol_down` | 0,2 | Vol − | volume-1 |
| `volup` | Volume up | media `vol_up` | 1,2 | Vol + | volume-2 |
| `mute` | Mute | media `mute` | 2,2 | Mute | volume-x |
| — | — | — | 6,3 | Home | home (`navigate: "$home"`) |

Плюс элемент `{ "id": "now", "kind": "media", "x": 0, "y": 0, "w": 4, "h": 2, "prevAction": "tpl-spotify-prev", "playAction": "tpl-spotify-play", "nextAction": "tpl-spotify-next" }`.

Все элементы-кнопки шаблонов (см. ниже) — `kind: "button"`, `w: 1`, `h: 1`, `icon` с префиксом `lucide:`; id элемента = id действия без префикса (`palette`, `newtab`, …), у кнопки «Home» id `home`.

## 8. Иконки программ и установленные приложения (M12)

### 8.1 Хранилище иконок

- Каталог `<configDir>\icons\`, файлы `<hash16>.png`, где `hash16` — первые 16 hex-символов SHA-256 содержимого PNG.
- Каждая иконка — PNG 96×96 с прозрачностью; data URL не длиннее 32 768 символов (иначе иконка отклоняется с ошибкой `Icon is too large`).
- **M14 (режим «Большая иконка», DESIGN.md §2.1):** новые иконки из `icon_from_program` и `icon_from_image` — PNG **256×256**, предел data URL — **131 072** символа. Если PNG 256 больше предела — повтор с 192×192, затем 128×128; иначе ошибка `Icon is too large`. Существующие файлы 96×96 остаются валидными. Команда `refresh_program_icons` (PROTOCOL.md §9) для каждой кнопки с `img:`-иконкой и действием `launch` извлекает иконку заново, если текущий файл меньше 256 px, заменяет `icon` в раскладке и сохраняет её; старые файлы не удаляются (на них могут ссылаться другие кнопки). SVG-иконки (§8.4) не затрагиваются.
- **Маленькие иконки без версии 256 (уточнение 2026-11-15, пример — 7-Zip: крошечный знак в рамке посреди 256-px поля).** Для `icon_from_program` порядок: 1) `SHDefExtractIconW(path, 0, 0, &large, null, 256)` — самая большая иконка из ресурсов файла, без рамки Shell; ошибка → 2) `IShellItemImageFactory` как раньше. Затем для любого источника: обрезать прозрачные поля по рамке пикселей с альфой > 16 и вписать содержимое в 256×256 с полем 6% (качественное масштабирование `BitmapInterpolationMode::Fant`). Если в результате шага 2 по краю остаётся однотонная рамка толщиной 1–2 px (все пиксели внешнего контура одного цвета ±8 по каналам, альфа > 200) — это рамка Shell: срезать её и повторить обрезку.
- Источники: (а) `icon_from_program(path)` — системная иконка exe через Shell (`IShellItemImageFactory::GetImage` с размером 96×96, флаг «только иконка», не миниатюра); (б) `icon_from_image(path)` — файл png/jpg/ico/bmp, декодирование и масштабирование до 96×96 с сохранением пропорций и прозрачными полями (`Windows.Graphics.Imaging`, как в T11.4b). Результат кодируется в PNG через `BitmapEncoder` (`PngEncoderId`).
- Иконки никогда не скачиваются из интернета и не поставляются с Decka (DECISIONS.md D-35).
- Файлы, на которые не ссылается ни одна кнопка, не удаляются автоматически (в MVP; занимают килобайты).

### 8.2 Секция `icons`

- Значение = `{ hash16: dataURL }` для всех `img:`-иконок, встречающихся в текущей раскладке (`icon` кнопок и `bind.states[*].icon`). Пересчитывается при каждом изменении раскладки; рассылается только при изменении (обычное правило `Store::set`).
- Валидация раскладки (новое правило 11 к §3.2): `img:<hash>` без файла `<configDir>\icons\<hash>.png` → ошибка `icon file not found — choose the icon again`. Правило проверяется только агентом (TS-валидатор окна настроек его пропускает: сервер вернёт замечание при сохранении).

### 8.3 Установленные приложения

`list_installed_apps()`:
1. Рекурсивно обойти `.lnk` в `%ProgramData%\Microsoft\Windows\Start Menu\Programs` и `%APPDATA%\Microsoft\Windows\Start Menu\Programs`.
2. Каждый ярлык разобрать через `IShellLinkW` (`IPersistFile::Load`): путь цели, аргументы, рабочая папка. Аргументы разбить по правилам `CommandLineToArgvW`.
3. Оставить только цели, которые оканчиваются на `.exe` и существуют. Исключить ярлыки, у которых имя ярлыка содержит `uninstall` / `удал` (без учёта регистра) или имя файла цели начинается с `unins`.
4. Убрать дубликаты по паре (путь цели в нижнем регистре, аргументы); при дубликате оставить ярлык из `%APPDATA%`.
5. Иконка списка — та же Shell-иконка размером 32×32 PNG (data URL), без записи в хранилище; ошибка → без иконки.
6. Сортировка по `name` без учёта регистра. Ограничение — первые 500 результатов. Время выполнения на типовом ПК — несколько секунд: вызов асинхронный, окно показывает индикатор загрузки.
7. Приложения Microsoft Store (без `.exe`-ярлыка) в MVP не поддерживаются — для них остаётся `Browse…` или горячая клавиша.

### 8.4 SVG-иконки (M13)

- `icon_from_svg(svg: string) → { icon } | { error }`: принимает SVG-разметку из библиотеки Phosphor окна настроек.
- Проверка (иначе `{ error: "Unsupported SVG" }`): длина ≤ 12 000 символов; начинается с `<svg` (после пробелов); не содержит без учёта регистра `<script`, `<foreignObject`, `javascript:`, `href=` со значением не на `#`, атрибутов вида `on[a-z]+=`; корневой `viewBox` присутствует.
- Перед сохранением: заливки и обводки, отличные от `none`, заменяются на `#000` (маске важна только альфа), атрибуты `width`/`height` корня удаляются.
- Хранение: `<configDir>\icons\<hash16>.svg` (hash — от итоговой разметки); ссылка `img:<hash16>`; в секции `icons` — `data:image/svg+xml;base64,...` (≤ 16 384 символов).
- Клиент: data URL с `image/svg+xml` рисуется элементом с `mask-image: url(<dataURL>)`, `mask-size: contain`, `background-color: currentColor`; `image/png` — элементом `<img>`.

## 9. Каталог готовых кнопок и наборы (M15, решение 2026-11-16, D-46)

Каталог — данные, а не код: файлы `shared/catalog/catalog.json` и `shared/catalog/sets.json`, проверяются JSON-схемой и тестом. Окно строит из них библиотеку (DESIGN.md §4.8); быстрый выбор §4.5 берёт пункты оттуда же. Телефон каталог не получает.

```ts
// /shared/src/catalog.ts
export interface CatalogItem {
  id: string;                          // "media.shuffle", "audio.mic"; уникален
  label: { en: string; ru: string };   // подпись кнопки, ≤ 10 символов на обоих языках
  name: { en: string; ru: string };    // имя в библиотеке и в actions.json, ≤ 40
  icon: string;                        // "lucide:<имя>" или "ph:<имя>" (Phosphor; окно превращает в img: через icon_from_svg при создании)
  color?: string;                      // только сервисы и опасные действия, см. DESIGN.md §2.4
  action?: Action без поля name;       // кнопка-действие
  ask?: "url" | "keys" | "obs_scene" | "obs_input" | "page";   // чего не хватает — окно спрашивает при создании
  bind?: { var: VarName; states: Record<string, { label?: { en: string; ru: string }; icon?: string; color?: string }> };
  navigate?: "$back";
  widget?: { kind: LayoutItem["kind"]; w: number; h: number; options?: object };   // пункт-виджет
  requires?: "obs" | "weather";         // без настроенной интеграции пункт показывается с пометкой `Set up` и ведёт в её настройки
}
export interface CatalogCategory { id: string; name: { en: string; ru: string }; icon: string; items: string[] }  // items — id пунктов по порядку
export interface CatalogSet {
  id: string; name: { en: string; ru: string }; description: { en: string; ru: string }; icon: string;
  rows: (string | null)[][];           // форма набора: id пунктов по рядам, null — пустая клетка; виджеты занимают свои w×h, начиная с клетки, где указаны
  requires?: "obs" | "weather";
}
```

**Категории и пункты v1** (точные подписи — в `catalog.json`; ниже en / ru):

| Категория | Пункты |
|---|---|
| Windows / Windows | Lock / Блок (`system lock`, confirm); Sleep / Сон (`system sleep`, confirm); Screenshot / Снимок (`win+shift+s`); Task mgr / Задачи (`ctrl+shift+esc`); Desktop / Стол (`win+d`); Explorer / Проводник (`win+e`); Emoji / Эмодзи (`win+period`); Clipboard / Буфер (`win+v`); Settings / Параметры (`win+i`) |
| Media / Медиа | Play / Пуск (`play_pause`, bind `media.playing`: true → `pause`, «Пауза»); Next / Далее; Prev / Назад; Shuffle / Случайно (bind `media.shuffle`); Repeat / Повтор (bind `media.repeat`: `none`/`list`/`track` — три вида); виджет «Сейчас играет» 2×1 |
| Sound / Звук | Mic / Микрофон (`audio mic_toggle`, bind `mic.muted`: true → `mic-off`, «Mic off» / «Микр. выкл», красный); Mute / Без звука (`audio out_mute_toggle`, bind `audio.muted`); Vol + / Громче; Vol − / Тише; виджет «Громкость» 1×2 |
| Stream / Стрим | Record / Запись (`obs record_toggle`, bind `obs.recording`); Go live / Эфир (`obs stream_toggle`, bind `obs.streaming`, confirm); Replay / Повтор (`obs save_replay`); Scene / Сцена (`ask: obs_scene`, bind `obs.scene` = эта сцена → акцент); Mic OBS / Микр. OBS (`ask: obs_input`); виджет OBS 2×1. Все с `requires: "obs"` |
| Widgets / Виджеты | Часы, Система, Система с графиком, Погода (`requires: "weather"`), Таймер, Громкость, Сервисы, Уведомления, Сейчас играет, OBS |
| Navigation / Переходы | Back / Назад (`$back`); Open page / Страница (`ask: page`); Popup group / Группа |
| Web / Сайты | Website / Сайт (`ask: url`) |
| Keys / Клавиши | Shortcut / Сочетание (`ask: keys`); Sequence / Цепочка (открывает редактор цепочки, DESIGN.md §4.8) |
| Apps / Программы | установленные программы (M12), как в быстром выборе |

**Наборы v1** (`sets.json`):

| id | Имя | Форма |
|---|---|---|
| `media-remote` | Media remote / Медиапульт | `[prev, play, next, shuffle, repeat]` + ряд ниже: виджет «Сейчас играет» 3×1 |
| `sound` | Sound / Звук | `[mic, mute, vol-, vol+]`, справа виджет «Громкость» 1×2 |
| `windows-tools` | Windows tools / Инструменты Windows | `[lock, screenshot, taskmgr, desktop]`, `[explorer, emoji, clipboard, settings]` |
| `stream` | Streaming / Стрим (`requires: obs`) | `[record, live, replay, mic-obs]`, `[scene₁, scene₂, scene₃, null]` (первые три сцены OBS), виджет OBS 3×1 ниже |
| `focus` | Focus / Фокус | виджет «Таймер» 2×1, часы 2×1 |

**Вставка набора** — чистая функция `insertSet(layout, actions, pageId, set, ctx)` → `{ layout, actions, placedOn: pageId[] }` (`ctx` — язык, сцены/входы OBS):
1. Действия создаются как в §4.5 DESIGN (`findSameAction` — без дублей, `makeActionId`).
2. Форма целиком в первую позицию текущей страницы (обход сверху вниз, слева направо), где она помещается без пересечений.
3. Не помещается целиком, но свободных клеток хватает → элементы по одному в порядке формы в первые свободные клетки (виджеты — первая позиция, где помещаются; не поместился виджет — пропускается и указывается в результате).
4. Свободных клеток не хватает → новая страница с именем набора сразу после текущей, форма с левого верхнего угла.
5. Пункты с `ask` в наборе берут значение из `ctx` (сцены OBS по порядку); нет значения — пункт пропускается.
6. Повторная вставка того же набора на ту же страницу добавляет кнопки снова (без дедупликации кнопок; действия переиспользуются).