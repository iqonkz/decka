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
  | (ActionBase & { type: "system"; op: "lock" | "sleep"
      | "shutdown" | "restart" | "signout" | "hibernate" | "display_off" | "task_manager" | "settings" | "explorer" | "empty_recycle_bin" }) // M17, §10.8
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

**Исправление 2026-11-20 (T17.7).** Буквы, цифры и знаки нажимаются **виртуальным кодом клавиши**, а не символом. Раньше был `Key::Unicode`: при русской раскладке у `b` нет клавиши, enigo отправлял символ (`KEYEVENTF_UNICODE`), Windows игнорировал модификаторы, и вместо `Ctrl+B` в документ печаталась буква.
- Коды: `a`..`z` → `0x41..0x5A`; `0`..`9` → `0x30..0x39`; `period` `0xBE`, `comma` `0xBC`, `minus` `0xBD`, `equal` `0xBB`, `slash` `0xBF`, `semicolon` `0xBA` (enigo 0.6.1: `Key::Other(vk)`).
- Пауза 10 мс между нажатиями клавиш сочетания. Удержание 30 мс до отпускания: Office и оболочка Windows (`Win+V`, `Win+.`) иначе пропускают быстрые сочетания.

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
  language?: "auto" | Language;    // M14: язык интерфейса, DESIGN.md §6; отсутствует = "auto". M19: Language = "en"|"zh"|"hi"|"es"|"ar"|"fr"|"pt"|"ru"|"de"|"ja"|"kk"; неизвестный код при чтении → "auto"
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
// M16 (§6.8)
export interface AiItem      extends ItemBase { kind: "ai"; show?: "both" | "claude" | "codex"; } // по умолчанию "both"; размеры 2×1, 2×2, 3×2, 4×2

export type LayoutItem = ButtonItem | ClockItem | SysItem | StatusesItem | StatusItemTile | NotificationsItem | MediaItem
  | VolumeItem | TimerItem | WeatherItem | ObsItem | AiItem;

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
13. (M15) Размеры: `volume` — 1×2..1×4 (вертикальный) или 2×1..4×1 (горизонтальный); `timer` — 1×1, 2×1, 2×2; `weather` — 1×1, 2×1, 3×2, 4×2; `obs` — 2×1..4×2; `sys` с `graph: true` — 2×1..4×2. `upAction`/`downAction`/`muteAction` у `volume` — существующие действия. `navigate: "$back"` допустим только у кнопки обычной страницы. `timer.presets` — 1..4 целых 1..180. (M16) `ai` — 2×1, 2×2, 3×2, 4×2 (`fitWidgetSize` — по тем же правилам).
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
  2. **Пересмотрено 2026-11-16 (жалоба владельца: «кнопки перемешиваются хаотично»; транспонирование отменено).** Раскладка перетекает в новую сетку **с сохранением порядка чтения**. Сначала виджеты — в порядке `(y, x)` исходной страницы, размер сохраняется (ширина зажимается в `cols'`, высота — в `rows'`, не меньше минимума вида; горизонтальный `volume` в книжной становится вертикальным: `w ↔ h`; после зажатия размер приводится к **допустимому для вида** по правилам 4 и 13 §3.2 — наибольший допустимый размер, не превышающий зажатый, иначе наименьший допустимый; например `weather` 4×2 в сетке шириной 3 → 2×1, а не 3×2 и не 3×1 — уточнение 2026-11-16), каждый — в первую свободную позицию при обходе сверху вниз, слева направо.
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
  ai?: {                                                         // M16 (§6.8); отсутствует = всё выключено
    claude: { enabled: boolean; approvals: boolean; timeoutSec: 60 | 120 | 300 };  // по умолчанию после cc_install: true, true, 120
    codex: { enabled: boolean };                                  // по умолчанию true, если Codex найден
    hookToken?: string;                                           // "dk_" + 43 base64url; создаёт cc_install; только для /api/cc/*; get_config его не отдаёт
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

**Исправление 2026-11-20 (T17.7).** Запись и сброс истории вызывались из хука `useDeckProps`, а он работает в каждой смонтированной странице, в полосе и во всплывающей группе. Монтирование соседней страницы при малейшем свайпе сбрасывало график, а одно значение записывалось несколько раз. Теперь история (и `ai`) пишется в одном месте — в store при приёме `welcome` / `patch sys` и сбрасывается только при новом соединении.

`sys` с `graph: true`: клиент хранит последние 60 значений `cpu` и `ram` (≈ 2 мин при опросе 2 с), очищает при переподключении. Линия толщиной 1.5 px, под ней заливка цветом линии с непрозрачностью 0.15; CPU — `#3b82f6`, RAM — `#a855f7` (для Material — акцент и вторичный тон). Текущие значения цифрами справа сверху. Сеть и диск — как раньше, строкой снизу при `h = 2`.

### 6.6 Звук: сборщик и ползунок (M15)

- **Сборщик `audio`** (агент): раз в 500 мс читает Core Audio — громкость устройства вывода по умолчанию (`GetMasterVolumeLevelScalar`, 0..100 целым), mute вывода и mute микрофона по умолчанию. Публикует `vars`: `audio.volume`, `audio.muted`, `mic.muted`; изменения рассылаются, только если значение поменялось. Нет устройства → переменная удаляется.
- **Сборщик `media`** дополнительно публикует `media.playing`, `media.shuffle`, `media.repeat` (переменные для `bind`).
- **Ползунок `volume`** — телефон по-прежнему шлёт **только `press` с `actionId`** (правило безопасности): перетаскивание по дорожке переводится в число шагов по 2% (шаг клавиши громкости Windows) и отправляется нажатиями `upAction`/`downAction`, не чаще 15 в секунду; во время перетаскивания ползунок показывает ожидаемое значение, после отпускания через 600 мс — значение `audio.volume`. Тап по значку динамика → `muteAction`. Нет переменной `audio.volume` → ползунок серый, работают только `+` / `−` по тапу на концах.

**Исправление 2026-11-20 (T17.7).** Ползунок зацикливался на ±2: при нечётной громкости (37 → цель 40) до цели оставался 1%, `Math.round(0.5) = 1` давал шаг, и ползунок перескакивал 39 ↔ 41. Теперь `volumeSteps` считает шаги через `Math.floor(|diff| / 2)`: разница меньше шага — это «на месте».

### 6.7 Таймер (M15)

Целиком на телефоне, ПК не участвует. Тап — старт/пауза; удержание 0,8 с — сброс; кнопки пресетов (`25`, `5`) при `w ≥ 2` выбирают длительность. Окончание — виброотклик `confirm` ×3, тост `Timer done` / `Таймер закончился`, вспышка рамки виджета 3 раза, экран выходит из затемнения. Состояние живёт в памяти приложения (перезапуск сбрасывает).

### 6.8 ИИ-помощники: лимиты Claude Code / Codex и разрешения с телефона (M16, D-48)

Всё локально: агент читает только то, что Claude Code и Codex сами пишут на этом ПК. В интернет ничего не уходит, OAuth-токены и `~/.claude/.credentials.json` NEVER не читаются.

**Установка в Claude Code (`cc_install`)** — правит `%USERPROFILE%\.claude\settings.json` (пользовательский уровень):
1. Перед первой записью — копия `settings.json.decka-bak` (если копии ещё нет). Нет файла — создать `{}`. Файл не JSON — ничего не менять, ошибка `settings.json is not valid JSON`.
2. Создать `ai.hookToken`, если нет. Порт — текущий из конфигурации.
3. `statusLine`: если отсутствует или уже наш (команда содержит `/api/cc/statusline`) → записать
   `{ "type": "command", "command": "curl.exe -s -m 2 --data-binary @- -H \"Authorization: Bearer <hookToken>\" http://127.0.0.1:<port>/api/cc/statusline" }`.
   Чужой `statusLine` не трогать → `statusLineConflict: true` (лимиты Claude не будут видны, разрешения работают).
4. `hooks.PermissionRequest`: удалить наши записи (url содержит `/api/cc/`), добавить
   `{ "hooks": [ { "type": "http", "url": "http://127.0.0.1:<port>/api/cc/permission", "timeout": <timeoutSec + 15>, "headers": { "Authorization": "Bearer <hookToken>" } } ] }` (без `matcher` — все инструменты). Остальные хуки и ключи сохраняются; порядок ключей может измениться.
5. Повторный вызов идемпотентен (после смены порта или `timeoutSec` переписывает наши записи). `cc_uninstall` удаляет только наши записи (statusLine — только если наш; пустые `hooks.PermissionRequest`/`hooks` удаляются), `hookToken` очищается.
6. Смена порта или `timeoutSec` в `save_config` при `installed` → `cc_install` автоматически.

**`POST /api/cc/statusline`** (вызывается Claude Code при каждом обновлении строки статуса, до нескольких раз в секунду):
- Из тела берутся только: `session_id`, `cwd`, `model.display_name`, `context_window.used_percentage`, `rate_limits.five_hour` и `rate_limits.seven_day` (`used_percentage`, `resets_at` — секунды). Остальное игнорируется; неразобранное тело → ответ пустой строкой.
- `rate_limits` есть → `ai.claude = { windows: [5h, 7d], updatedAt }`; нет поля → прежнее значение не трогать (у API-ключа и в начале сессии его нет).
- Сессия → `ai.sessions` (upsert по id, `lastSeen = now`; старше 10 мин удаляются раз в минуту; максимум 6).
- Рассылка `patch ai` — не чаще 1 раза в 2 с (изменения копятся).
- Ответ `text/plain; charset=utf-8` — строка статуса для терминала: `Decka · 5h 23% · 7d 41%` (окон нет → `Decka`). Ответ отдаётся сразу, до рассылки.

**`POST /api/cc/permission`** (хук `PermissionRequest`; Claude Code ждёт ответа, терминальный диалог не показывается, пока хук не ответит):
1. Выключено `claude.approvals`, нет ни одного подключённого телефона или уже 5 запросов в `pending` → сразу 200 с пустым телом. Claude Code покажет обычный вопрос в терминале.
2. Иначе — `PermRequest`: `reqId` 16 hex (`getrandom`/`rand`, уже в зависимостях), `tool` = `tool_name`, `project` = basename `cwd`, `sessionId`, `expiresAt = now + timeoutSec`, `summary`:
   - `Bash`/`PowerShell` → `tool_input.command` (одна строка, переводы строк → ` ⏎ `);
   - `Edit`/`Write`/`MultiEdit`/`NotebookEdit`/`Read` → `tool_input.file_path` (или `notebook_path`) относительно `cwd`, если внутри него;
   - `WebFetch` → `tool_input.url`; `WebSearch` → `tool_input.query`;
   - `mcp__<server>__<tool>` → `tool` = `<server>: <tool>`, summary — первое строковое поле `tool_input`;
   - прочие → первое строковое поле `tool_input`, иначе пусто.
   Обрезка до 300 символов (`…`). Дальше `PermRequest` идёт только на телефон: в логи агента пишутся `reqId`, `tool`, `project` и итог, без summary.
3. Добавить в `ai.pending`, сразу разослать `patch ai` (без задержки 2 с), ждать: ответа `perm_answer`, истечения `timeoutSec` или обрыва HTTP-соединения (пользователь ответил в терминале или прервал Claude — запрос удаляется, guard на Drop).
4. Ответ:
   - `allow` → `{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}`;
   - `deny` → `{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"deny","message":"Denied from the Decka phone panel"}}}`;
   - `pc` или таймаут → 200 с пустым телом (вопрос появится в терминале).
   Каждый исход удаляет запрос из `pending` и рассылает `patch ai`. Первый ответ побеждает; повторные `perm_answer` с тем же `reqId` игнорируются.

**Лимиты Claude без строки статуса (T17.7, 2026-11-20, пересмотр D-48).** Расширение Claude Code для VS Code не выполняет `statusLine`. У владельца так и есть: установка верная, но данных нет. Поэтому добавлен второй источник, **включается явно**: `config.ai.claude.accountLimits: boolean`, по умолчанию `false`.
- **Как получаем:** раз в 5 мин читаем `%USERPROFILE%\.claude\.credentials.json` → `claudeAiOauth.accessToken` и `expiresAt` (мс). Затем `GET https://api.anthropic.com/api/oauth/usage` с заголовками `Authorization: Bearer <accessToken>`, `anthropic-beta: oauth-2025-04-20`, `User-Agent: decka-agent/<версия>`, таймаут 10 с.
- **Ответ** (недокументированный, разбирать терпимо): `five_hour` и `seven_day` → `{ utilization: number 0..100, resets_at: ISO-строка | null }` → окна `5h` / `7d`. Неизвестные поля игнорируются; нет ни одного окна → данные не меняются.
- **Токен:** никуда не сохраняется и не логируется, читается из файла перед каждым запросом и уходит только на `api.anthropic.com` (вшит, S18). Токен истёк (`expiresAt` в прошлом) или ответ 401/403 → запроса нет до следующего цикла. Статус в окне: `Open Claude Code once to refresh the sign-in`. Файла нет → `Not signed in to Claude Code`.
- **Приоритет:** данные строки статуса новее 5 мин важнее. Иначе — данные аккаунта.
- **Окно** (`AI coding`): переключатель `Read limits from my Claude account (needed for the VS Code extension)`, под ним мелко `Decka reads the Claude Code sign-in on this PC and asks Anthropic for your usage. Nothing else is sent.`
- **Виджет:** при `claude = null` подсказка меняется на `Using VS Code? Turn on "Read limits from my Claude account" in Decka`.

**Codex** (`codex.enabled`): раз в 60 с найти самый новый `rollout-*.jsonl` в `%USERPROFILE%\.codex\sessions\YYYY\MM\DD\` — идти по папкам дат по убыванию, не сканировать всё дерево; смотреть не дальше 14 последних дней. Прочитать последние 256 КБ файла, найти последнюю строку с `"rate_limits"` и взять `payload.rate_limits`:
```json
{"limit_id":"codex","primary":{"used_percent":0.0,"window_minutes":43200,"resets_at":1796743008},"secondary":null,"plan_type":"free"}
```
`primary`/`secondary` (каждое может быть `null`) → окна; `resets_at` — секунды (старые версии Codex пишут `resets_in_seconds` — тогда `resetsAt = время строки (timestamp) + resets_in_seconds`). Метка окна по `window_minutes`: 300 → `5h`, 10080 → `7d`, 43200 → `30d`, иначе `<round(min/60)>h`. `plan_type` → `plan`. Нет папки или строк → `codex = null`. Файл не меняется — секцию не рассылать.

**Общее для обоих:** окно, у которого `resetsAt` уже прошёл, агент удаляет при проверке раз в минуту. Это значит, что окно сброшено и показывается как `0%`.

**Переменные** (для `bind` кнопок; префикс `ai.` зарезервирован): `ai.claude.5h`, `ai.claude.7d` (числа, целые проценты), `ai.codex.primary`, `ai.codex.secondary`, `ai.pending` (число запросов). Значения нет — переменная удаляется.

**Прогноз расхода (клиент, чистая функция `burnEta(samples, now, resetsAt)` в `shared`):** клиент хранит по каждому окну до 60 точек `(ts, pct)` за последний час (сброс при переподключении и при уменьшении pct). Нужно ≥ 3 точек на отрезке ≥ 10 мин и рост ≥ 1 п.п. Скорость — по первой и последней точке, `eta = now + (100 − pct) / скорость`. `eta < resetsAt` → прогноз `Runs out ~14:20`; иначе прогноза нет.

**Виджет `ai`:**
- **Строка окна:** имя провайдера (`Claude` / `Codex`, у Codex — с планом мелко), полоса заполнения, `23%`, `resets in 2h 14m` (меньше часа — `14m`; больше суток — `3d 4h`). Цвет полосы — акцент; ≥ 80% — цвет уровня `warning` темы; ≥ 95% — `error`. Прогноз — строкой под полосой, цветом `warning`.
- **Размеры:** `2×1` — только короткое окно каждого провайдера, без подписей сброса. `2×2` — оба окна. `3×2` — оба окна и прогноз. `4×2` — слева лимиты, справа список сессий: проект, модель, `ctx 34%`; сессия с ожидающим запросом — с точкой цвета `warning`.
- **Нет данных:** `claude = null` → `Claude: no data yet` (подпись мелко: `Install in Decka → Integrations`); у `codex = null` строка Codex скрыта. `show` ограничивает провайдеров.
- **Бейдж:** при `pending.length > 0` в углу виджета — число запросов, тап открывает лист разрешений.

**Лист разрешений (телефон):** появляется поверх любой страницы, когда в `pending` есть запрос. Сохраняется при переключении страниц и при затемнении экрана; экран выходит из затемнения. Виброотклик `confirm`.
- **Состав:** заголовок `Claude wants to use <tool>`; `project`; `summary` моноширинным шрифтом (3 строки, тап — развернуть); полоса обратного отсчёта до `expiresAt`. Кнопки: `Deny` (цвет `error`) и `Allow` (акцент) — обе в половину ширины, высота 56 px; под ними текстовая кнопка `Answer on PC`. Несколько запросов — показывается старейший, сверху `1 of 3`.
- **Ответ:** нажатие шлёт `perm_answer`, лист закрывается сразу, не дожидаясь `patch`. Запрос исчез из `pending`, потому что ответили на ПК или вышел таймаут, — лист закрывается тихо.
- **Повторы:** `Allow` и `Deny` не реагируют первые 400 мс после появления листа — защита от случайного касания.

**Окно настроек: `More → Integrations → AI coding`.**
- **Claude Code:** кнопка `Install` или `Remove`; статус `Connected · 2 min ago` / `Not installed` / `Waiting for a Claude Code session`; при конфликте — `Your status line is used by another tool — limits are not available, approvals work`. Переключатель `Approve on phone`; `Wait for the phone` 60 / 120 / 300 s. Подпись: `Restart running Claude Code sessions after installing`.
- **Codex:** переключатель `Show Codex limits`; статус `Found` / `Codex not found`. Разрешений для Codex нет (D-48).

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
  // M19: все { en: string; ru: string } каталога, наборов и шаблонов → LocalizedText = { en: string } & Partial<Record<Language, string>>;
  // выбор — pickText(text, lang): нужный язык, иначе en. Тест каталога требует все 11 языков у каждого текста.
  // Шаблоны: nameRu (T17.6) → names: LocalizedText без en (en — прежнее name).
  // Принято по итогам M19 (2026-11-21): тексты 9 новых языков лежат в shared/catalog/i18n/<код>.json и сливаются при загрузке;
  // у шаблонов в Rust — поля names и actionNames.
  label: { en: string; ru: string };   // подпись кнопки, ≤ 10 символов на обоих языках (M19: ≤ 12 на любом языке)
  name: { en: string; ru: string };    // имя в библиотеке и в actions.json, ≤ 40
  icon: string;                        // "lucide:<имя>" или "ph:<имя>" (Phosphor; окно превращает в img: через icon_from_svg при создании)
  color?: string;                      // только сервисы и опасные действия, см. DESIGN.md §2.4
  action?: Action без поля name;       // кнопка-действие
  ask?: "url" | "keys" | "obs_scene" | "obs_input" | "page";   // чего не хватает — окно спрашивает при создании
  bind?: { var: VarName; states: Record<string, { label?: { en: string; ru: string }; icon?: string; color?: string }> };
  navigate?: "$back";
  widget?: { kind: LayoutItem["kind"]; w: number; h: number; options?: object };   // пункт-виджет
  requires?: "obs" | "weather";         // без настроенной интеграции пункт показывается с пометкой `Set up` и ведёт в её настройки
  confirm?: boolean;                    // (принято 2026-11-16) кнопка с удержанием, как ButtonItem.confirm
  popup?: true;                         // (принято 2026-11-16) пункт «Группа»: создаёт popup-страницу и кнопку к ней
}
// ask дополнительно может быть "sequence" — открывает редактор цепочки. Специальные значения: color "$accent" → display.accent;
// ключ состояния "$scene" в bind.states → имя сцены, выбранной в ask: obs_scene. Оба разворачивает insertSet / быстрый выбор при создании.
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

**Наборы v1** (`sets.json`). Уточнение 2026-11-16: каждая форма **не шире 4 колонок и не выше 7 рядов**, чтобы целиком ложиться и на 7×4, и на книжную 4×7; таблица ниже — состав, точная раскладка по рядам — в `sets.json` (переложено на 4 колонки):

| id | Имя | Форма |
|---|---|---|
| `media-remote` | Media remote / Медиапульт | `[prev, play, next, shuffle, repeat]` + ряд ниже: виджет «Сейчас играет» 3×1 |
| `sound` | Sound / Звук | `[mic, mute, vol-, vol+]`, справа виджет «Громкость» 1×2 |
| `windows-tools` | Windows tools / Инструменты Windows | `[lock, screenshot, taskmgr, desktop]`, `[explorer, emoji, clipboard, settings]` |
| `stream` | Streaming / Стрим (`requires: obs`) | `[record, live, replay, mic-obs]`, `[scene₁, scene₂, scene₃, null]` (первые три сцены OBS), виджет OBS 3×1 ниже |
| `focus` | Focus / Фокус | виджет «Таймер» 2×1, часы 2×1 |
| `ai` | AI coding / ИИ-помощники (M16) | виджет `ai` 4×2 |

**Вставка набора** — чистая функция `insertSet(layout, actions, pageId, set, ctx)` → `{ layout, actions, placedOn: pageId[] }` (`ctx` — язык, сцены/входы OBS):
1. Действия создаются как в §4.5 DESIGN (`findSameAction` — без дублей, `makeActionId`).
2. Форма целиком в клетку сброса `ctx.at` (если задана и форма там помещается), иначе — в первую позицию текущей страницы (обход сверху вниз, слева направо), где она помещается без пересечений. Тень при перетаскивании — `previewSet` с той же логикой, без изменения раскладки (принято 2026-11-17, v0.3.0).
3. Не помещается целиком, но свободных клеток хватает → элементы по одному в порядке формы в первые свободные клетки (виджеты — первая позиция, где помещаются; не поместился виджет — пропускается и указывается в результате).
4. Свободных клеток не хватает → новая страница с именем набора сразу после текущей, форма с левого верхнего угла.
5. Пункты с `ask` в наборе берут значение из `ctx` (сцены OBS по порядку); нет значения — пункт пропускается.
6. Повторная вставка того же набора на ту же страницу добавляет кнопки снова (без дедупликации кнопок; действия переиспользуются).

## 12. Голосовое управление и локальный ИИ (M21+, решение владельца 2026-11-21, D-52)

Цель: Decka — универсальный инструмент управления ПК: кнопками на телефоне и голосом (с ПК по горячей клавише или с телефона плавающей кнопкой) на языке интерфейса (11 языков). Всё локально: модель и распознавание речи работают на ПК, в интернет уходят только загрузки файлов. Идеи — по образцу Nixi (heynixi), код не заимствуется (строка в NOTICE).

**Принцип простоты для пользователя:** одна кнопка `Turn on voice control` → одна полоса загрузки → готово. Ни Ollama, ни консоли, ни отдельной установки. Серверы моделей — внутренние дочерние процессы агента, пользователь их не видит. Размер установщика и APK не меняется: всё скачивается по кнопке.

### 12.1 Состав «пакета ИИ» (скачивается по кнопке)
Манифест `shared/ai/manifest.json`. В нём для каждого файла: `id`, `kind` (`runtime` | `llm` | `stt`), `url`, `sha256`, `size`, `license`. Версии закреплены архитектором; `sha256` и `size` исполнитель берёт из GitHub API (`assets[].digest`/`size`) и Hugging Face API (`siblings` → `lfs.sha256`). Сами файлы не скачивать для подсчёта.

| id | Что | Источник | Размер |
|---|---|---|---|
| `llama-runtime` | `llama-server.exe` с Vulkan (NVIDIA/AMD/Intel; без видеокарты — CPU-бэкенды из того же архива) | `github.com/ggml-org/llama.cpp/releases/download/b12602/llama-b12602-bin-win-vulkan-x64.zip`, MIT | 37 МБ |
| `whisper-runtime` | `whisper-server.exe` (CPU) | `github.com/ggml-org/whisper.cpp/releases/download/b5907/whisper-bin-x64.zip`, MIT | 9 МБ |
| `llm-standard` | **Qwen3.5 4B** Q4_K_M — основная | `huggingface.co/unsloth/Qwen3.5-4B-GGUF/Qwen3.5-4B-Q4_K_M.gguf`, Apache-2.0 | 2,55 ГБ |
| `llm-light` | Qwen3.5 2B Q4_K_M — для слабых ПК | `huggingface.co/unsloth/Qwen3.5-2B-GGUF/Qwen3.5-2B-Q4_K_M.gguf`, Apache-2.0 | 1,19 ГБ |
| `stt` | whisper large-v3-turbo q5_0 — одна на всех | `huggingface.co/ggerganov/whisper.cpp/ggml-large-v3-turbo-q5_0.bin`, MIT | ≈ 0,55 ГБ |

**Пересмотр 2026-11-21 (выбор моделей).** Скачивается и работает **одна модель ИИ и одна модель речи**, выбор делает агент по железу (§12.2), пользователь ничего не выбирает.
- **Почему 4B, а не 2B:** в семействе Qwen3.5 вызов инструментов резко лучше у 4B (по карточке модели BFCL-V4 50.3, TAU2 79.9; у 2B и 0.8B — заметно ниже).
- **0.8B и 9B не предлагаются:** 0.8B ненадёжна для выбора инструментов и для языков кроме английского; 9B слишком тяжела для фоновой работы.
- **Новее Qwen3.5 в малых размерах на 2026-11-21 нет:** Qwen3.6 и Qwen3.8 выпущены только от 27B.
- **Gemma 4 E4B** (Apache-2.0) понимает речь сама и могла бы заменить whisper. Но разбор звука в llama.cpp у неё пока нестабилен, а распознавание по сообщениям хуже whisper turbo. Пересмотреть позже (§12.10).
- **Казахские дообученные модели** (Qazwen-2B, ISSAI Qwen3.5-4B-Base-Kazakh) — исследовательские версии. По умолчанию не используются; пересмотреть по итогам `voice-eval` для `kk`.

- **Хосты загрузки** (S18, вшиты): `github.com`, `objects.githubusercontent.com`, `release-assets.githubusercontent.com`, `huggingface.co`, `cdn-lfs.hf.co`, `cas-bridge.xethub.hf.co`. Редиректы — только на эти хосты.
- **Если любого файла из таблицы нет** под таким именем — BLOCKERS, другой не подставлять. Файлы `mmproj-*.gguf` (зрение, 0,63 ГБ) сейчас не скачиваются — понадобятся для экранной помощи (§12.10).
- **Хранение:** `%LOCALAPPDATA%\Decka\ai\runtime\<id>-<версия>\`, `%LOCALAPPDATA%\Decka\ai\models\`.
- **Загрузка:**
  - с докачкой (`Range`) во временный файл `*.part`, затем проверка SHA-256; не совпало — файл удалить, ошибка `Download damaged — try again`;
  - архивы распаковываются встроенным `tar.exe -xf <zip> -C <dir>` (Windows 10 1803+), без новых библиотек, во временную папку → переименование.
- **Удаление:** кнопка `Remove` стирает всё из `ai\`.

### 12.2 Рекомендация по железу
Агент узнаёт: видеокарту и её память (DXGI `IDXGIFactory1::EnumAdapters1` → `DedicatedVideoMemory`, признак `Win32_Graphics_Dxgi`), ОЗУ (`GlobalMemoryStatusEx`).
- **Модель ИИ:** `llm-standard` (4B) при видеопамяти ≥ 6 ГБ, а без такой видеокарты — при ОЗУ ≥ 16 ГБ. Иначе `llm-light` (2B).
- **Распознавание речи:** всегда `stt`.
- **Подпись в окне:** `This PC: NVIDIA GeForce RTX 2060 SUPER (8 GB), 15.8 GB RAM — standard model`.
- **Смена модели:** только в `Advanced` — переключатель `Use the light model (faster, less accurate)`. Смена скачивает другую модель и удаляет прежнюю.

### 12.3 Надзор за серверами
- **Запуск:** агент запускает `llama-server` и `whisper-server` как дочерние процессы (`CREATE_NO_WINDOW`) и помещает их в Job Object с `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`: падение или закрытие агента убивает их. Адрес — только `127.0.0.1`, свободный случайный порт.
  - У `llama-server` — `--api-key <случайный на запуск>`.
  - У `whisper-server` ключа нет: защищают только loopback и случайный порт (S24).
- **Параметры `llama-server`:** `-m <gguf> -c 4096 -ngl 99 --jinja --no-webui --host 127.0.0.1 --port <p> --api-key <k>`.
- **Параметры `whisper-server`:** `-m <bin> --host 127.0.0.1 --port <p> -t <ядра-1>`.
- **Готовность:** `GET /health` (llama) / `GET /` (whisper) до 60 с.
- **Ленивый режим:** серверы стартуют при первом голосовом запросе. Пока модель грузится, плашка показывает `Loading the model…`. Через 10 мин без запросов серверы останавливаются, и видеопамять освобождается. Переключатель `Keep the model loaded` отключает остановку.
- **Падение:** перезапуск через 2, 5, 15 с. Больше 3 падений за 10 мин — статус `Voice control stopped: <последняя строка stderr>`, без новых попыток до ручного `Retry`.
- **Журнал:** stderr серверов пишется в лог агента не больше 50 строк на запуск, транскрипты в лог не попадают.

### 12.4 Конвейер одного запроса
1. **Запись:** 16 кГц, моно, PCM16. Конец — отпускание кнопки, повторное нажатие горячей клавиши или тишина 1,2 с (порог по энергии, после первых 300 мс речи). Максимум 30 с. `Esc` или `Cancel` — отмена.
2. **Распознавание:** `POST /inference` у `whisper-server`: multipart `file` (WAV), `language=<код языка интерфейса>` (принудительно, не `auto` — так точнее), `response_format=json`, `temperature=0`. Пустой текст → `Didn't catch that`.
3. **Быстрый путь (без ИИ):** таблица фраз языка `shared/voice/<lang>.json`, шаблоны со слотами `{app}`, `{n}`, `{query}`, `{name}`.
   - Нормализация: нижний регистр, без знаков препинания, числа словами → цифры через `Intl.NumberFormat` или таблицу.
   - Совпадение → сразу инструмент. Так выполняются частые команды: громкость, медиа, открыть/переключиться на программу, нажать кнопку по подписи, страница, таймер, поиск, блокировка.
   - **Формат файла `shared/voice/<lang>.json`:**
     ```json
     { "lang": "ru",
       "yes": ["да", "ага", "конечно"], "no": ["нет", "отмена", "не надо"],
       "intents": [
         { "tool": "volume", "args": { "op": "up" }, "patterns": ["громче", "сделай громче", "прибавь звук"] },
         { "tool": "open_app", "patterns": ["открой {app}", "запусти {app}"] },
         { "tool": "web_search", "args": { "site": "youtube" }, "patterns": ["найди на ютубе {query}"] },
         { "tool": "timer", "patterns": ["таймер на {n} минут", "поставь таймер {n} минут"] } ] }
     ```
     Слоты: `{app}`, `{name}`, `{label}`, `{query}`, `{domain}`, `{n}` (число цифрами или словами). Слот забирает остаток фразы до следующего литерала шаблона. Шаблоны сравниваются после нормализации; побеждает самый длинный литерал.
   - **Проверка `shared/voice/tests/<lang>.json`:** `[{ "say": "сделай погромче", "tool": "volume", "args": { "op": "up" } }]` — по 10 фраз, которые **не совпадают дословно** с шаблонами (проверка нормализации и нечёткого совпадения).
4. **ИИ-путь** (нет совпадения):
   - **Запрос:** `POST /v1/chat/completions` с `response_format: { type: "json_schema", json_schema: <схема инструментов> }`, `temperature: 0.1`, `max_tokens: 256`.
   - **Системный промпт** — по-английски. В нём список инструментов, контекст (имена до 200 установленных программ, подписи кнопок и действий пользователя, имена страниц, программа в фокусе) и строка `The user speaks <язык>. Reply text in <язык>.`
   - **Ответ:** `{ "tool": <enum>, "args": {…}, "say": "<короткий ответ>" }`. Он проверяется по схеме и правилам инструмента; не прошёл → `Didn't get that — try again` и никакого действия.
5. **Подтверждение** для рискованных инструментов (§12.5, пометка ⚠):
   - плашка или телефон показывают, что будет сделано, и кнопки `Yes` / `No`;
   - ответ голосом — короткие «да/нет» на языке пользователя быстрым путём;
   - ожидание 10 с, затем отмена.
6. **Исполнение** — только существующими исполнителями Decka. Результат: текст `say` или стандартный (`Done` / `Opened Calculator`) — на плашке ПК и в листе на телефоне.
   - Озвучка — по желанию, `Windows.Media.SpeechSynthesis`, если для языка установлен голос. По умолчанию выключена; голоса нет — пункт серый с подсказкой.

### 12.5 Инструменты (вся власть голоса — только здесь)
Аргументы — данные, а не команды. Каждый инструмент сам проверяет аргументы. Произвольного запуска, путей, shell и URL «как есть» нет.

| Инструмент | Аргументы | Исполнение и проверка |
|---|---|---|
| `press_button` | `label` | Кнопка раскладки по подписи или имени действия (нечёткое совпадение ≥ 0.8, триграммы) → её `actionId` |
| `run_action` | `id` | Только существующий id из `actions.json` |
| `open_app` / `switch_app` / `close_app` | `name` | Только программа из списка установленных (§8.3) или окно с таким exe. Переключение — `SetForegroundWindow` (с `AllowSetForegroundWindow`/`AttachThreadInput`). Закрытие — `WM_CLOSE`: программа сама спросит о несохранённом |
| `window_other_monitor` | — | Активное окно на следующий монитор (`MonitorFromWindow`, `SetWindowPos`), с сохранением относительного положения |
| `which_monitor` | — | Ответ `Monitor 2 of 2` |
| `volume` | `op: up|down|mute|set`, `value?` 0..100 | Core Audio |
| `media` | `op` | Как действие `media` |
| `go_page` | `name` | Телефоны получают `navigate` |
| `web_search` | `query`, `site: web|youtube|maps` | URL строится на ПК по шаблону (`https://www.google.com/search?q=`, `https://www.youtube.com/results?search_query=`, `https://www.google.com/maps/search/`), `query` кодируется. Браузер по умолчанию |
| `open_site` | `domain` | Только вид домена `^[a-z0-9-]+(\.[a-z0-9-]+)+$` → `https://<domain>` |
| `type_text` | `text` ≤ 2000 | Ввод в поле в фокусе — символами Unicode, это верно для текста, в отличие от сочетаний клавиш. Пустое поле фокуса не проверяется (Windows не даёт) — подсказка в справке |
| `dictate` | — | Режим «следующая фраза печатается как есть» |
| `timer` | `minutes` 1..180, `label?` | Таймер агента; по окончании — уведомление на ПК и тост на телефонах |
| `system` ⚠ | `op` | Существующие системные действия с ограничениями §11.4 |

Позже, отдельными этапами: `write` (черновик от ИИ в поле), заметки и списки, экранная помощь (UI Automation и `Windows.Media.Ocr`), поиск файлов (Windows Search), календарь (ICS), цепочки команд и сохранение как ярлык.

### 12.6 Точки входа
- **ПК, горячая клавиша:** по умолчанию `Ctrl+Alt+Space`, меняется в окне через `ChordField` (§10.3). Регистрация — `RegisterHotKey`. Первое нажатие — слушать, второе — закончить, `Esc` — отмена.
- **ПК, плашка (как «пилюля» Nixi):**
  - маленькое окно Tauri без рамки, поверх всех, сверху по центру экрана с курсором, 360 × 64 px, раскрывается до 360 × 160 для ответа или подтверждения;
  - **создаётся при старте запроса и уничтожается через 4 с после конца** (правило памяти §10.1);
  - в нём волна уровня микрофона, распознанный текст, результат и кнопки подтверждения;
  - вид — DESIGN.md §4.12.
- **Телефон, плавающая кнопка:**
  - круглая кнопка микрофона 56 px поверх страницы. Перетаскивается и прилипает к краю; положение хранится на телефоне (`decka.voicefab.v1`);
  - **(пересмотр 0.20.1, отзыв владельца)** тап — начать слушать, конец — сам по тишине (§12.6a), повторный тап — закончить сразу; удержание ≥ 250 мс без движения — «рация» (говорить, пока держишь; отпустить — закончить); движение > 8 px — перетаскивание. Настройки `Tap to start / tap to stop` больше нет;
  - у кнопки и у всего её слоя: `contextmenu` → `preventDefault`, `-webkit-touch-callout: none`, `user-select: none`, `touch-action: none`; касание кнопки не доходит до страницы (`stopPropagation` на pointer-событиях), чтобы не открывался лист долгого нажатия страницы (DESIGN §4.8) и выделение текста WebView;
  - видна, если голос включён на ПК (`vars` → `voice.ready = true`) и не скрыта в меню телефона (`Voice button`);
  - есть также вид кнопки раскладки `voice` (обычная клавиша 1×1 с микрофоном).
- **Телефон, звук:**
  - `getUserMedia({audio})` → `AudioContext` → понижение до 16 кГц → PCM16 кусками по 100 мс;
  - куски идут по зашифрованному каналу (§10 PROTOCOL): `voice_start`, `voice_chunk`, `voice_end`, `voice_cancel`;
  - Android: разрешение `RECORD_AUDIO` при первом использовании; отказ → подсказка `Allow the microphone for Decka in Android settings`.
- **Обратная связь:** секция `voice` (PROTOCOL.md §3) — состояние, распознанный текст, ответ, запрос подтверждения. Ответ на подтверждение — `voice_confirm{id, yes}`.
- **Источник звука ПК** — микрофон по умолчанию (`cpal` 0.18.2). Выбор устройства — в окне.

### 12.6a Конец фразы по тишине и уровень звука (0.20.1)
Одна логика для ПК (Rust, рядом с захватом `cpal`) и телефона (`client/src/voice/`), чистая функция с тестами: на вход — RMS кусков по 100 мс, на выход — `continue | end | no_speech`.
- **Порог речи:** первые 300 мс — замер шума `floor` (среднее RMS); речь — кусок с RMS > `max(floor × 3, 0.015)` (PCM, нормированный к 1.0).
- **Конец фразы:** после хотя бы 300 мс речи — **2 000 мс подряд ниже порога** → `end` (как `voice_end`/второе нажатие).
- **Ничего не сказали:** нет речи за 5 с от старта → `no_speech`, отмена, текст `Didn't hear anything` / `Ничего не услышал`, звук на распознавание не идёт.
- **Предел:** 20 с записи → `end`.
- **Ручное управление остаётся:** второе нажатие `Ctrl+Alt+Space` / повторный тап по кнопке телефона / **клик по плашке ПК (кнопка «стоп» или любое место пилюли)** — `end` сразу, сказанное идёт на распознавание и выполнение. Плашка принимает клик, не забирая фокус (`WS_EX_NOACTIVATE`, Tauri `focusable: false`) — иначе `type_text` уйдёт не в то окно; `Esc` / смахивание пузыря — отмена; в режиме «рация» на телефоне конец — только отпускание (тишина не обрывает).
- **Сторож:** плашка и кнопка не могут висеть в `Listening…`/`Thinking…`: если состояние не меняется 30 с, агент отменяет запрос и показывает `Something went wrong, try again`. Ошибка захвата микрофона (устройство не открылось, нет кусков 1 с) — сразу ошибка `Microphone is unavailable` / `Микрофон недоступен`.
- **Уровень для волны:** ПК шлёт в окно плашки `VoiceStatus.level` (0–1, RMS с усилением и сглаживанием: подъём мгновенно, спад 150 мс) не чаще 20 раз/с; на телефоне уровень считается локально из того же RMS. Пять столбиков волны — высота от уровня (средние выше крайних, коэффициенты 0.6/0.85/1/0.85/0.6, минимум 4 px); в тишине столбики лежат, а не стоят точками. Кольцо телефонной кнопки — радиус по тому же уровню.

### 12.7 Окно настроек: `More → Voice`
Одна карточка, без лишнего:
- **До включения:** заголовок `Voice control`, одна строка о том, что это, строка железа (§12.2), кнопка `Turn on voice control (≈ 3.2 GB)` (с лёгкой моделью — `≈ 1.8 GB`).
  - Размер считается по рекомендованным моделям.
  - Под кнопкой мелко: `Everything runs on this PC. Nothing you say leaves it.`
- **Во время загрузки:** одна общая полоса, `1.2 of 1.9 GB`, `Cancel`.
- **После включения:**
  - статус `Ready · Ctrl+Alt+Space`;
  - настройки одним списком, без вкладок: горячая клавиша, микрофон, `Keep the model loaded`, озвучка ответов, кнопка на телефоне;
  - свёрнутый блок `Advanced`: лёгкая модель (§12.2) и строка `Model: Qwen3.5 4B · Speech: Whisper large-v3-turbo`;
  - внизу `Remove voice control`.
- **Справка `What you can say`** — карточки по категориям с 2–3 примерами на языке интерфейса и строкой ограничений, как у Nixi, но короче. Примеры берутся из `shared/voice/<lang>.json`.

### 12.8 Языки и качество
- **Язык команд = язык интерфейса** (11 языков). Распознавание с принудительным языком; ИИ получает язык в промпте; ответы — на нём же.
- **Таблицы фраз** `shared/voice/<lang>.json` для 11 языков пишет модель по `GLOSSARY.md`. `ru` и `kk` проверяет владелец.
- **Набор проверки** `shared/voice/eval/<lang>.json`: 25 фраз → ожидаемые `tool` и `args`.
  - Скрипт `tools/voice-eval.mjs` гоняет текст (без звука) через быстрый путь и локальный `llama-server`, печатает точность по языкам.
  - Запускается **только при выпуске 0.x.0** (правило п. 7c).
  - Цель: en/ru/es/de/fr/pt ≥ 90%, остальные ≥ 80%. Ниже — строка в CHANGELOG `Voice commands in <язык> are experimental`.
- **Ограничения** честно показываются в справке: качество казахского и хинди ниже; длинные тексты для `type_text` — до 2000 символов.

### 12.9 Ресурсы
- **Пока модели загружены:** `llm-standard` ≈ 3 ГБ видеопамяти (`llm-light` ≈ 1,6 ГБ; без видеокарты — столько же ОЗУ), `stt` ≈ 1 ГБ ОЗУ.

### 12.10 Пересмотреть позже
- **Зрение для экранной помощи:** Qwen3.5 понимает изображения сама (файл `mmproj`, 0,63 ГБ). «Что за ошибка на экране» — снимок окна в модель вместо отдельного OCR.
- **Gemma 4 E4B вместо whisper + Qwen** — одна модель для речи и понимания. Когда её разбор звука в llama.cpp станет стабильным и `voice-eval` покажет не хуже.
- **Казахская дообученная модель** — если `kk` в `voice-eval` ниже 80%.
- **Когда не загружены:** 0 — серверы остановлены (§12.3).
- **Скорость:** цель — ответ на быструю команду ≤ 2,5 с от конца речи на ПК владельца; ИИ-путь ≤ 4 с.

## 13. M22 — помощник на локальном ИИ (решение владельца 2026-11-24: «берём всё, по порядку»; идеи — Nixi, Jarvis, D-52)
Общее: всё на тех же whisper и Qwen (§12), ничего не уходит в сеть; инструменты — только из белого списка §12.5 + новые ниже, проверка `validate` как раньше; тексты пользователя и снимки экрана не пишутся в лог и на диск. Ответы-тексты показываются в плашке ПК (раскрытой) и на телефоне в пузыре/листе голоса. Функции — в порядке задач T22.10…T22.15.

### 13.1 Умная диктовка (T22.10)
- **Запуск диктовки — двойное нажатие Ctrl** (решение владельца 2026-11-24: правого Ctrl на его клавиатуре нет, нравится как у Jarvis). Любой Ctrl, левый или правый: нажать-отпустить дважды за ≤ 400 мс, каждое нажатие короче 250 мс, **без других клавиш между ними** (поэтому `Ctrl+C`, `Ctrl+V` и т. п. не срабатывают). Отслеживание — `WH_KEYBOARD_LL`, клавиши не глотаются. Конец — тишина 2 с (§12.6a), одно нажатие Ctrl или клик по плашке; `Esc` — отмена. Настройка в `More → Voice`: `Double Ctrl` (по умолчанию) / любое сочетание через `ChordField` / выключено. Команды по-прежнему на `Ctrl+Alt+Space`.
- **Режимы** (`voice.dictation.mode`): `clean` (по умолчанию) и `raw`. `raw` — текст whisper как есть. `clean` — проход через Qwen со строгой инструкцией: знаки препинания и заглавные, убрать слова-паразиты («э-э», «ну», «как бы», um/uh), применить самоисправления («в четверг, нет, в пятницу» → «в пятницу»), списки голосом → строки списка; **ничего не добавлять и не переписывать по смыслу**. Защита: ответ длиннее 1,3× или короче 0,5× исходного, либо дольше 3 с → вставляется `raw`.
- **Личный словарь** (`voice.dictionary`, ≤ 500 слов/фраз, правится в `More → Voice → Dictionary`): передаётся whisper как подсказка (`prompt`) и в инструкцию `clean`.
- **Вставка:** до 300 символов — `type_text` (Unicode); длиннее — через буфер обмена (`Ctrl+V`) с восстановлением прежнего содержимого через 1 с.
- Плашка: `Dictating…` / `Диктую…` с волной; режим диктовки голосом («режим диктовки», §12.5 `dictate`) остаётся.

### 13.2 Работа с выделенным текстом (T22.11)
- Новый инструмент `text_transform{ op, lang? }`, `op`: `polite` · `shorter` · `fix` (ошибки и знаки) · `translate` (`lang` — один из 11 языков) · `formal` · `casual` · `summarize` · `explain`. Фразы: «сделай вежливее», «переведи на английский», «исправь ошибки», «перескажи коротко», «объясни»…
- **Взять выделение:** сохранить буфер обмена → `Ctrl+C` → ждать изменения буфера ≤ 400 мс → текст (≤ 6000 символов, длиннее — `The text is too long` / `Текст слишком длинный`); ничего не выделено → `Select text first` / `Сначала выделите текст`.
- **Результат:** `polite/shorter/fix/translate/formal/casual` — заменяют выделение (`Ctrl+V`), буфер восстанавливается; отмена — обычный `Ctrl+Z` программы. `summarize/explain` — ничего не вставляют: ответ в плашке и на телефоне (≤ 120 слов, язык пользователя).

### 13.3 Помощь по экрану (T22.12)
- Инструмент `screen_ask{ question }` — только по явной фразе («что на экране», «объясни эту ошибку», «перескажи эту переписку», «что здесь нажать»). Никогда автоматически.
- **Снимок:** только активное окно (Windows Graphics Capture; запасной — `PrintWindow`), уменьшение до 1280 px по длинной стороне, в памяти, удаляется после ответа; на диск и в лог не пишется. Пока снимок в работе — значок камеры в плашке (`Looking at the screen` / `Смотрю на экран`).
- **Модель:** Qwen3.5 со своим `mmproj` (§12.10, ≈ 0,63 ГБ) — отдельный необязательный пакет `Screen help` / `Помощь по экрану`, скачивается по согласию при первом запросе (как пакет ИИ, SHA-256 в манифесте). Лицензия модели — только Apache-2.0/MIT; если у выбранной версии нет готового `mmproj` в GGUF — BLOCKERS, другую модель не брать без решения.
- Ответ: ≤ 120 слов, в плашке и на телефоне.

### 13.4 Напоминания и списки (T22.13)
- Файлы `reminders.json` (`{id, text, at, done}`), `lists.json` (`{id, name, items[{id, text, done}]}`) в каталоге конфигурации; лимиты: 500 напоминаний, 50 списков × 200 пунктов, текст ≤ 200.
- Инструменты: `remind{ text, at? | inMinutes? }` (ИИ отдаёт локальное время ISO или минуты; проверка: в будущем, ≤ 1 год), `reminders_show`, `list_add{ list, item }`, `list_show{ list }`, `list_remove{ list, item }` (нечёткое совпадение как у `press_button`). Удаление/очистка всего списка — с подтверждением.
- **Срабатывание:** уведомление Windows + плашка + телефон (лента уведомлений). Пропущенные, пока ПК был выключен, — при старте одним уведомлением `Missed: …` / `Пропущено: …`.
- **Виджеты телефона:** `reminders` (ближайшие 3–5) и `list` (выбранный список; тап по пункту — отметить). Отметка с телефона — сообщение `list_toggle{ listId, itemId }` (PROTOCOL §3: данные, не команда; id выданы агентом).
- В окне ПК — `More → Reminders` / `Напоминания`: список, правка, удаление.

### 13.5 Сводка дня (T22.14)
- Фраза «что у меня сегодня», «сводка», «доброе утро» → инструмент `day_brief`. Данные собирает ПК из того, что уже есть: погода (виджет), непрочитанная почта (§10), напоминания на сегодня, непустые списки, лимиты Claude/Codex (§6.8); календарь — когда появится. Qwen пишет 3–5 предложений **только из этих данных** (JSON в запросе; без данных — фраза пропускается).
- Показ: лист на телефоне (карточки по разделам + текст) и плашка. Голосом — когда будет синтез речи.
- Настройка `Morning brief on the phone at the first unlock of the day` / `Сводка утром на телефоне`, по умолчанию выключена.

### 13.6 Память о пользователе (T22.15)
- `memory.json`: факты `{id, text, createdAt}`, ≤ 200 штук по ≤ 200 символов. Добавляются **только явно**: «запомни, что …» (инструмент `memory_add`), удаляются «забудь …» (`memory_remove`, подтверждение) и в `More → Voice → Memory` / `Память`.
- В запрос к Qwen идут до 20 фактов, подходящих к фразе (совпадение слов), — для подстановок вроде «напиши Айгерим» → имя из памяти, «открой мой проект» → путь из памяти только через существующее действие.
- Без автоматического «обучения» и без записи фраз.

## 11. M18 — безопасность (решение владельца 2026-11-20, D-50)

### 11.1 Шифрование
Протокол v2 — PROTOCOL.md §10.

### 11.2 Общественные сети
Правило брандмауэра остаётся `profile=private,public`, потому что владелец пользуется Decka и в общественных сетях. Политику соблюдает сам агент **для каждого соединения**:
- **Тип сети соединения:** локальный адрес принятого сокета → адаптер (`GetAdaptersAddresses`, имя адаптера = GUID) → категория сети (`INetworkListManager::GetNetworkConnections` → `GetAdapterId`, `GetNetwork().GetCategory()`). Признаки `windows` 0.62.2: `Win32_NetworkManagement_IpHelper`, `Win32_Networking_NetworkListManager`, `Win32_Networking_WinSock`. Loopback → `local`. Категория `DOMAIN`/`PRIVATE` → `private`. `PUBLIC` или не удалось определить → `public`. Кэш на 30 с.
- **`local`:** всё, включая v1.
- **`private`:** только v2. Веб-клиент отдаётся, ручной код разрешён, HTTP `/api/*` работает с `apiToken`.
- **`public`:**
  - только приложение (v2): статика веб-клиента → 404;
  - сопряжение только `pair_qr` (`pair_code` → `error BAD_MESSAGE "On a public network pair with the QR code or USB"`);
  - HTTP `/api/*` из сети → 404, с loopback — как обычно;
  - лимит рукопожатий 5 в минуту.
- **Настройка** `config.network.allowPublic: boolean`, по умолчанию `true`. `false` → соединения из общественной сети закрываются сразу после TCP-приёма.
- **Окно:** страница `Phone`, строка состояния: `Public network — encrypted, only the Decka app, pairing by QR` / `Private network` / `USB`. Переключатель `Allow phones on public networks`.

### 11.3 Разрешения Claude Code с телефона
- **Кто может отвечать:** `devices.json` → `canApprove: boolean`. При обновлении существующим телефонам ставится `true`, новым — `false`. Переключатель `Can answer Claude Code` на странице `Phone`. Нет подключённого телефона с `canApprove` → запроса на телефоне нет, вопрос сразу в терминале. `perm_answer` от телефона без права игнорируется и пишется в лог.
- **Опасные запросы:** агент помечает `PermRequest.danger: true` по уже существующему списку опасных команд (`ai.rs`). На телефоне такой `Allow` — удержание 1 с с круговым индикатором, как у меню (DESIGN.md §4.3). Тап только подсказывает `Hold to allow`. `Always allow` для опасных не показывается и не принимается.
- **`Always allow` с телефона:**
  - правило привязано к `session_id` Claude Code и живёт до 8 ч (`expiresAt`);
  - вид `tool` (весь инструмент) с телефона не принимается — сервер исполняет как разовый `allow`, а на телефоне этот вариант не предлагается;
  - в окне (`AI coding → Rules`) у правила видны источник (`Phone <name>` / `PC`), срок и кнопка `Remove`;
  - правила с ПК — как раньше.

### 11.4 Опасные системные действия
`shutdown`, `restart`, `signout`, `hibernate` (§10.8):
- отказ `ACTION_FAILED` `Too soon after connecting`, если соединение этого телефона моложе 60 с;
- не чаще 1 раза в 30 с на весь агент (`Wait a little before trying again`);
- каждое исполнение — строка лога `system <op> by <device name>`.
Подтверждение `confirm` остаётся.

### 11.5 Уведомления на виду
- **Настройка:** `config.windowsNotifications.privacy = { default: "full" | "app"; apps: Record<string, "full" | "app"> }`. По умолчанию `default: "full"`, но для программ, чьё имя содержит (без учёта регистра) `telegram`, `whatsapp`, `signal`, `viber`, `discord`, `slack`, `teams`, `outlook`, `mail`, `thunderbird`, `messenger`, `skype`, — `app`.
- **Режим `app`:** агент **не отправляет** текст на телефон: `title` = имя программы, `body` нет, `hidden: true`. Телефон показывает `New notification` / `Новое уведомление`.
- **Окно:** в списке программ раздела `Windows notifications` у каждой — выбор `Full text` / `App name only` / `Don't show`.

### 11.6 Секреты, журнал, релизы
- **Секреты:** все токены и пароли в `config.json` и `devices.json` — только `dpapi:<base64>`, включая токены Twitch и YouTube и `K_dev`. **Исключения (2026-11-21):** `apiToken` (ключ для скриптов пользователя, показывается в окне) и `ai.hookToken`, который и так открыт в `~/.claude/settings.json`. Открытое значение при загрузке шифруется и перезаписывается. Тест: конфиг с открытыми токенами → после загрузки и сохранения в файле нет исходных строк.
- **Журнал:** тест прогоняет сопряжение, вход, `perm_answer`, OBS и почту с известными секретными строками и проверяет, что их нет в выводе лога.
- **Релизы:** `release:agent` / `release:apk` пишут `release\SHA256SUMS.txt` (`Get-FileHash -Algorithm SHA256`) и прикладывают его к релизу. README, раздел `Verify the download`: `Get-FileHash .\Decka-Setup-<v>.exe` и сравнение с файлом.
- **Перед релизом:** `npm audit --omit=dev --audit-level=high` и `cargo audit`. Инструмент ставится один раз: `cargo install cargo-audit --locked`, в зависимости проекта не входит. Найденные `high`/`critical` → BLOCKERS. Обновлять зависимости самому нельзя.

## 10. M17 — список владельца 2026-11-19 (D-49)

### 10.1 Скорость и память
Замер на ПК владельца 2026-11-19: дерево процессов `decka-agent.exe` — 232 МБ private, из них агент 21 МБ, остальное — 6 процессов WebView2 скрытого окна настроек.
- **Окно настроек уничтожается при закрытии** (крестик или Esc; `WebviewWindow::destroy`, не `hide`). Открывается заново из трея, при повторном запуске exe или по `decka://`, создаётся с теми же параметрами. Вкладка и прокрутка восстанавливаются из `localStorage`. Агент не зависит от существования окна: события в отсутствующее окно не отправляются, ошибок нет.
- **Цель:** окно закрыто 10 с → у `decka-agent.exe` нет дочерних `msedgewebview2.exe`, дерево ≤ 60 МБ private. Открытие окна из трея ≤ 1,5 с.
- **Сборщики:** `sysinfo` обновляет только нужное: CPU и память — раз в 2 с; сеть — раз в 2 с; диски — раз в 30 с. Списки процессов не обновляются. Сборщики, чьих секций нет ни в одной раскладке и которые не нужны переменным `bind`, не запускаются: `media`, `audio`, `weather`, `obs`, `ai`, `battery`, `mail`. Секция `sys` нужна, только если в раскладке есть виджет `sys`.
- **Телефон:** раскладка не перерисовывается целиком на `patch` секции. Виджет подписан только на свою секцию, кнопки — только на `vars`, `statuses` и `icons`, и только в части своих `bind`. Изображения иконок декодируются один раз (кэш `Map<hash, HTMLImageElement>` или `img` с `decoding="async"`).

### 10.2 Перелистывание без рывков
Причина рывков (код `PageStrip.tsx`): `setOffset` на каждое движение пальца перерисовывает три страницы через React. Новые соседи монтируются в тот же кадр, что и конец доводки.
- Во время жеста позиция меняется **напрямую в DOM** через `ref` (`style.transform = translate3d(...)`), не чаще одного раза в кадр (`requestAnimationFrame`). Состояние React меняется только в начале жеста (видимость соседа) и после доводки.
- На `pointerdown` — `setPointerCapture`. На контейнере страниц — `touch-action: pan-y`. На слоях — `will-change: transform`, пока идёт жест или доводка.
- `DeckBridge` и клавиши — `React.memo` по `page`, `grid` и своим данным, поэтому смена `offset` их не перерисовывает.
- После доводки новые соседи монтируются в следующем кадре простоя (`requestIdleCallback`, запасной вариант — `setTimeout(…, 50)`), а не в кадре окончания анимации.
- Порог, скорость и кривая — без изменений (DESIGN.md §3).

### 10.3 Сочетания с системными клавишами
Windows перехватывает часть сочетаний (`Win+D`, `Win+L`, `Alt+Tab`, `Win+Shift+S`, `Ctrl+Alt+Del`…) до окна, и запись их не видит. Поэтому `ChordField` дополняется ручным выбором (DESIGN.md §4.9):
- **Модификаторы** — четыре кнопки-переключателя: `Ctrl`, `Alt`, `Shift`, `Win`.
- **Клавиша** — поле с поиском по всем `KeyName` (§2.2). Группы: буквы, цифры, F1–F24, навигация, знаки. Подписи: `Print Screen`, `Page Up`, `.` …
- Запись нажатием остаётся. Записанное сочетание сразу показывается в переключателях и поле — это одно и то же значение `keys`.
- Порядок в `keys`: `ctrl`, `alt`, `shift`, `win`, затем клавиша. Без основной клавиши сохранить нельзя: `Pick a key`. Исключение — одиночный `win`, он допустим (открывает «Пуск»).

### 10.4 Уведомления Windows на телефоне
- **Источник:** `Windows.UI.Notifications.Management.UserNotificationListener` (WinRT). Признаки окна `windows`: `UI_Notifications`, `UI_Notifications_Management`, `ApplicationModel`.
  - **Шаг 0 (проверка, ≤ 30 мин):** работает ли API в непакетном exe агента — `RequestAccessAsync` из потока окна, затем `GetNotificationsAsync(Toast)`.
  - Не работает (ошибка доступа или `Denied` без запроса пользователю) → запись в BLOCKERS.md, пункт пропускается целиком. Пакеты с внешним расположением (sparse package) и чтение базы `wpndatabase.db` — **не делать**: это решение архитектора.
- **Включение:** `More → Integrations → Windows notifications`: переключатель → `RequestAccessAsync` → Windows показывает свой запрос → статус `Allowed` / `Blocked in Windows Settings → Privacy → Notifications`. По умолчанию выключено. Список `Don't show from:` — приложения, уже присылавшие уведомления; отмечаются флажками.
- **Исправление 2026-11-20 (T17.7).** У владельца уведомления не включены: в `config.json` нет `windowsNotifications`, виджет пуст. Поэтому:
  1. Пока функция выключена, пустой виджет `notifications` показывает `Windows notifications are off` и подпись `Turn on in Decka → Integrations → Windows notifications`. Кнопки включения на телефоне нет: телефон не меняет настройки ПК. Состояние агент сообщает переменной `notif.windows`: при включённой функции — `true`, при выключенной переменной нет (принято 2026-11-20; префикс `notif.` зарезервирован).
  2. ~~Разовое предложение тостом Windows~~ — снято 2026-11-20: тост от агента — отдельная функция, а подписи в виджете достаточно.
  3. При включении уже лежащие в Центре уведомлений тосты (до 20, новые первыми) сразу попадают в секцию. Раньше первый проход их только запоминал, и виджет оставался пустым до нового уведомления.
- **Чтение:** событие `NotificationChanged` (Added), а если подписка недоступна для непакетного приложения — опрос `GetNotificationsAsync` раз в 3 с с запоминанием виденных `Id`. Из уведомления берутся: `AppInfo.DisplayInfo.DisplayName`, `AppInfo.AppUserModelId` и тексты привязки `ToastGeneric` (первый — title, остальные через перевод строки — body). Обрезка 120/500. Уведомления Decka не пересылаются.
- **Доставка:** в существующую секцию `notifications` (≤ 20, новые первыми) с полями `source: "windows"`, `app` (отображаемое имя), `openable: true` (AUMID известен). Иконка приложения — через хранилище иконок `img:<hash16>` (§8.1), если её удаётся получить из `AppInfo.DisplayInfo.GetLogo`, иначе без иконки. Снятие уведомления в Windows (Removed) убирает его и с телефона. `notif_dismiss` на телефоне скрывает только у Decka, в Windows не удаляет.
- **Открыть:** тап по уведомлению на телефоне → `notif_open{notifId}`. Агент по своему `notifId` находит AUMID и запускает `explorer.exe shell:AppsFolder\<AUMID>` (`Command`, без shell). Это открывает или активирует программу. Нет AUMID или уведомление удалено → `press_result`-подобного ответа нет, ошибка пишется в лог. Телефон ничего не передаёт, кроме `notifId` (S21).
- **Тост на телефоне:** новое уведомление Windows показывается тостом сверху 4 с (DESIGN.md §3), если на странице нет виджета `notifications`. Тап по тосту — как тап по уведомлению.

### 10.5 Полоса приложения (вместо подсказки «Добавить кнопки?»)
Сейчас при программе в фокусе без своей страницы телефон показывает подсказку `{app} is in front and has no page. Add keys for it?`. Её заменяет **полоса приложения** — выдвижная панель с кнопками встроенного шаблона, как Touch Bar на Mac. Раскладку она не меняет и новых страниц не создаёт.
- **Когда:** у активного приложения (`focus.app`) нет своей страницы (ни одна страница не содержит его в `apps`) и есть встроенный шаблон `shared/templates/*.json` с этим exe в `apps`. Есть своя страница → как раньше, контекстная страница (§6.1), полосы нет.
- **Секция `strip`** (агент):
  - формат `{ app: string; template: string; name: string; keys: { actionId: Id; label: string; icon?: string }[] } | null`;
  - `keys` — кнопки страницы шаблона в порядке чтения, максимум 16, с подписями на языке интерфейса;
  - при смене фокуса пересчитывается со стабилизацией 700 мс, как контекст.
- **Нажатие:** телефон шлёт обычный `press{actionId}` с id действия шаблона (`tpl-<template>-*`). Агент исполняет такое действие из встроенного шаблона, только если `strip.template` сейчас этот шаблон и в `actions.json` нет действия с таким id. В остальных случаях — `UNKNOWN_ACTION`. В `actions.json` ничего не пишется.
- **Поведение на телефоне** (DESIGN.md §4.10). Режим полосы для каждого приложения хранится только на этом телефоне: `localStorage` `decka.strip.v1` = `{ "<exe>": "auto" | "tab" | "never" }`, по умолчанию `auto`.
  - **`auto`:** полоса выезжает сама при появлении `strip` и остаётся, пока приложение в фокусе. Тап вне полосы сворачивает её в язычок, нажатие под ней не срабатывает.
  - **`tab`:** виден только язычок у края; тап или свайп от края открывает полосу.
  - **`never`:** ничего не показывается.
- **Выбор режима** — кнопка `⋯` в заголовке полосы: `Open automatically` / `Only the tab` / `Never for <name>`. Список приложений с `never` — в меню телефона (DESIGN.md §4.3), пункт `App strips`, у каждого — `Show again`.
- **Принято по итогам T17.3 (2026-11-19):**
  - секцию считает агент — функция `strip_for` на Rust;
  - иконка в заголовке пока `lucide:monitor`;
  - подписи кнопок — английские, у шаблонов нет перевода.
- **Исправление T17.6 (2026-11-20):**
  - **Подписи.** У действий шаблонов и в `name` шаблона — необязательное поле `nameRu`. Формат каталога `{en, ru}` не вводится: шаблоны уже используются в окне и тестах как строки. `strip_for` берёт `nameRu` при языке `ru`, иначе `name`. Перевести все 13 шаблонов по глоссарию DESIGN.md §6.
  - **Значок.** `StripInfo.icon?: "img:<hash16>"`. Агент находит exe в списке установленных приложений (§8.3) по имени файла без учёта регистра и извлекает иконку как для кнопок (§8.1); она попадает в секцию `icons`. Не найден — поля нет, на телефоне `lucide:app-window`.
- **Подсказка в окне ПК** (`TemplateHint`, вставка шаблона одним кликом) остаётся. Строка `pageapps.suggest` на телефоне больше не показывается. Виджет `context` без страницы показывает кнопки `strip`, если он есть.

### 10.6 Заряд подключённых устройств
- **Сборщик `battery`** (агент): раз в 5 мин и при старте перечисляет присутствующие устройства (`SetupDiGetClassDevsW(NULL, NULL, NULL, DIGCF_PRESENT | DIGCF_ALLCLASSES)`) и читает свойство `{104EA319-6EE2-4701-BD47-8DDBF425BBE5}, 2` (заряд Bluetooth-устройства, байт 0..100) через `SetupDiGetDevicePropertyW`. Имя — `DEVPKEY_Device_FriendlyName`. Дубликаты по имени (у одного устройства несколько узлов) — берётся максимум. Нужны признаки `Win32_Devices_DeviceAndDriverInstallation` и `Win32_Devices_Properties` того же `windows` 0.62.2.
- **Батарея ПК** (ноутбук): `GetSystemPowerStatus` (`Win32_System_Power`). Нет батареи (`BatteryFlag & 128`) — пункт отсутствует.
- **Секция `battery`:** `{ devices: { name: string; pct: number; kind: "pc" | "bt"; charging?: boolean }[]; updatedAt } | null`. ПК — первым, остальные по имени, ≤ 8.
- **Переменные:** `battery.pc`, `battery.low` (число устройств ниже 20%). Префикс `battery.` зарезервирован.
- **Виджет `battery`:** `1×1` — самое разряженное устройство; `2×1` / `2×2` / `3×1` — список строк: иконка по типу (`laptop`, `headphones`, `mouse`, `keyboard`, `gamepad`, иначе `bluetooth` — по словам в имени), имя, полоса, `%`. Заряд ниже 20% — цвет `warning`, ниже 10% — `error`. Нет устройств → `No devices report battery`.
- Устройства, которые не сообщают заряд Windows (многие мыши с собственным приёмником), не показываются — это ограничение Windows, а не ошибка.

Принято по итогам T17.4: к 60 иконкам Lucide (§3.6) добавлены `rotate-ccw`, `log-out`, `monitor-off`, `laptop`, `mouse`, `keyboard`, `gamepad-2`, `bluetooth`. Сборщик `ai` работает всегда, потому что принимает вопросы разрешений. `focus` тоже работает всегда.

### 10.7 Почта: непрочитанные
- **Протокол:** IMAP по TLS (порт 993) — только счёт `UNSEEN` во `INBOX`. Писем и заголовков не читаем.
  - Команды: приветствие → `a1 LOGIN "<user>" "<pass>"` (кавычки и `\` экранируются) → `a2 STATUS INBOX (UNSEEN)` → `a3 LOGOUT`.
  - Таймаут 15 с. Только `tokio-rustls` 0.26.6 и `rustls-platform-verifier` 0.7.1 — они уже в `Cargo.lock` через `reqwest`; добавляются прямыми зависимостями ровно этих версий. Без STARTTLS и без незашифрованного входа.
- **Учётные записи:** до 3, `config.mail.accounts[]`:
  - формат `{ id: Id; name: string; host: string; port: number; user: string; password?: "dpapi:<base64>"; openActionId?: Id }`;
  - пароль — как у OBS: `set_mail_password`, DPAPI, никогда в логах, протоколе, на телефоне;
  - пресеты в окне: Gmail (`imap.gmail.com`, `open_url https://mail.google.com`), Yandex (`imap.yandex.ru`), Mail.ru (`imap.mail.ru`), iCloud (`imap.mail.me.com`), Other;
  - подпись: `Use an app password (Gmail, Yandex, Mail.ru, iCloud). Outlook.com and Microsoft 365 are not supported — they allow only Microsoft sign-in`.
- **Расписание:** раз в 2 мин и по кнопке `Check now`. Ошибка входа (`NO`/`BAD` на LOGIN) → `error: "auth"`, повторов нет до смены пароля. Сетевая ошибка → `error: "network"`, обычный повтор.
- **Секция `mail`:** `{ accounts: { id; name; unread: number | null; error?: "auth" | "network" }[]; updatedAt } | null`. Переменные `mail.unread` (сумма) и `mail.<id>.unread`, префикс `mail.` зарезервирован.
- **Виджет `mail`:** `1×1` — конверт и общее число (0 — без числа, приглушённо); `2×1` / `2×2` — строки по ящикам. Тап по строке (или по `1×1`) → `press(openActionId)` этого ящика (первого, если `1×1`). Это обычное нажатие существующего действия. Нет `openActionId` — тап ничего не делает.
- **Безопасность (S21):** хост задаётся только в окне ПК. Исходящие соединения — только к хостам из `config.mail` (исключение из S18 для почты).

### 10.8 Системные действия
`system.op` дополняется (схема, валидаторы TS и Rust):
- `shutdown` → `shutdown.exe /s /t 0`;
- `restart` → `/r /t 0`;
- `signout` → `/l`;
- `hibernate` → `/h` (ненулевой код → `ACTION_FAILED` `Hibernation is turned off in Windows`);
- `display_off` → `PostMessageW(HWND_BROADCAST, WM_SYSCOMMAND, SC_MONITORPOWER, 2)`;
- `task_manager` → `taskmgr.exe`;
- `settings` → `explorer.exe ms-settings:`;
- `explorer` → `explorer.exe`;
- `empty_recycle_bin` → `SHEmptyRecycleBinW(None, None, SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND)`; пустая корзина — не ошибка.

Запуск — `Command` без shell, `CREATE_NO_WINDOW`. В `--dry-run` ничего не исполняется. Каталог, категория `System`: все новые действия. `shutdown`, `restart`, `signout`, `hibernate`, `empty_recycle_bin` — с `confirm: true`. Иконки Lucide: `power`, `rotate-ccw`, `log-out`, `moon`, `monitor-off`, `activity`, `settings`, `folder`, `trash-2`.

### 10.9 Переворот телефона
Ориентация раскладки (`display.orientation`, §3.7) не меняется. Сторону, которой стоит телефон, выбирает сам телефон и хранит у себя: `localStorage` `decka.turn.v1` = `"auto" | "normal" | "flipped"`, по умолчанию `auto`.
- `auto` → датчик сам выбирает одну из двух сторон, и телефон можно ставить зарядным разъёмом вверх или вниз.
  - **Исправление 2026-11-20 (T17.7):** `@capacitor/screen-orientation` 8.0.2 переводит `landscape` в `SCREEN_ORIENTATION_LANDSCAPE` — жёстко обычная сторона, датчик не работает. Поэтому `auto` идёт не через плагин, а через свой маленький Capacitor-плагин в `client/android` (`DeckaTurn.setSensor({ portrait: boolean })` → `activity.setRequestedOrientation(SCREEN_ORIENTATION_SENSOR_LANDSCAPE | SENSOR_PORTRAIT)` в UI-потоке).
  - `SENSOR_*` поворачивает экран даже при выключенном автоповороте в Android — так и нужно для подставки.
  - `normal` / `flipped` по-прежнему идут через плагин. В веб-режиме ничего не делается.
  - **Исправление 2026-11-21 (T20.1):** на телефоне владельца `SENSOR_LANDSCAPE` не переворачивает экран. Вероятно, это особенность прошивки, телефон для проверки не был подключён. Поэтому `auto` больше не полагается на системный датчик-режим:
    - `DeckaTurn` сам слушает `OrientationEventListener` (угол 0..359) и ставит **жёсткую** сторону:
      - для альбомной — угол 60..120 → `REVERSE_LANDSCAPE`, 240..300 → `LANDSCAPE`;
      - для книжной — 330..30 → `PORTRAIT`, 150..210 → `REVERSE_PORTRAIT`;
      - вне этих секторов сторона не меняется (гистерезис);
    - смена — только если новый сектор держится 600 мс;
    - **ориентацию раскладки (книжная/альбомная) датчик никогда не меняет** — её выбирают в окне ПК (`display.orientation`);
    - слушатель включается в `auto` и выключается в `normal` / `flipped`, а также при `onPause`; в `onResume` включается снова, если `auto`;
    - JS-API: `DeckaTurn.setAuto({ portrait })` и `DeckaTurn.stop()`. Метод `setSensor` удаляется.
- `normal` → `landscape-primary` / `portrait-primary`.
- `flipped` → `landscape-secondary` / `portrait-secondary`.
- Меню телефона (DESIGN.md §4.3): пункт `Turn` с тремя вариантами `Auto` / `Normal` / `Upside down`. Применяется сразу, без перезапуска; в веб-режиме пункт скрыт.