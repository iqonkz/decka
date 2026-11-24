# PROTOCOL.md — протокол Decka v1

Единственный источник правды по сообщениям. Типы из этого файла переносятся в `/shared/src/protocol.ts` дословно, схемы — в `/shared/schemas/protocol.schema.json` дословно, примеры — в `/shared/fixtures/` (по одному файлу на пример, имя указано над примером).

## 1. Общие правила

| Параметр | Значение |
|---|---|
| Порт по умолчанию | `9247` (TCP) |
| WebSocket | `ws://<host>:<port>/ws`, текстовые кадры, один JSON-объект на кадр, UTF-8 |
| HTTP API | `http://<host>:<port>/api/...`, тело JSON UTF-8 |
| Статика клиента | `GET /` и всё, что не `/api/*` и не `/ws` |
| Версия протокола | целое `PROTOCOL_VERSION = 1` |
| Макс. размер кадра WS от клиента | 4096 байт (больше → закрытие с кодом 1009) |
| Макс. тело HTTP | 16384 байт (больше → 413 `PAYLOAD_TOO_LARGE`) |
| Таймаут рукопожатия WS | 5 с от открытия сокета до `pair` или `hello`, иначе закрытие 4401 |
| Heartbeat | клиент шлёт `ping` каждые 10 с; нет `pong` 25 с → клиент рвёт сокет и переподключается; сервер закрывает сокет, если 30 с не было ни одного кадра |
| Идентификаторы (`Id`) | regex `^[a-z0-9_-]{1,64}$` |
| Имена переменных (`VarName`) | regex `^[a-z0-9_.-]{1,64}$` |
| Время | `ts` — миллисекунды Unix epoch (number) |

Поле-дискриминатор всех WS-сообщений — `t`.

**Правило безопасности (MUST):** единственное сообщение клиента, вызывающее действие на ПК, — `press`, и оно содержит только `actionId`. В протоколе нет и NEVER не появится сообщение, несущее команду, путь, аргументы, клавиши или URL от клиента.

## 2. Версионирование

- Клиент передаёт `v` в `pair` и `hello`. Сервер поддерживает ровно список `SUPPORTED = [1]`.
- `v` не из списка → сервер шлёт `error` с `code: "PROTOCOL_VERSION"`, `fatal: true`, затем закрывает сокет кодом 4426. Клиент показывает экран «Обновите приложение Decka».
- Совместимые изменения (версию НЕ меняют): новое необязательное поле, новый тип сообщения сервера, новая секция состояния, новый `kind` виджета, новый `type` действия, новый код ошибки.
- Обязанности клиента ради совместимости: игнорировать неизвестное `t`; игнорировать неизвестную секцию в `patch`; неизвестный `kind` элемента раскладки рисовать как пустую плитку с текстом `?`; неизвестный код ошибки показывать через `message`.
- Обязанность сервера: неизвестные поля во входящих сообщениях игнорировать (не ошибка).
- Несовместимое изменение (удаление/переименование поля, смена смысла) → `PROTOCOL_VERSION` увеличивается на 1, сервер добавляет новую версию в `SUPPORTED`.
- `GET /api/health` возвращает `protocol` — по нему клиент может проверить версию до открытия сокета.

## 3. Типы TypeScript

```ts
// ===== Базовые =====
export const PROTOCOL_VERSION = 1;
export type Id = string;        // ^[a-z0-9_-]{1,64}$
export type VarName = string;   // ^[a-z0-9_.-]{1,64}$
export type VarValue = string | number | boolean;

export type ErrorCode =
  | "UNAUTHORIZED" | "PAIR_CODE_INVALID" | "DEVICE_REVOKED" | "PROTOCOL_VERSION"
  | "BAD_MESSAGE" | "UNKNOWN_ACTION" | "ACTION_FAILED" | "RATE_LIMITED"
  | "PAYLOAD_TOO_LARGE" | "NOT_FOUND" | "INTERNAL";

// ===== Клиент -> Сервер (WS) =====
export interface ClientInfo { name: string; app: "capacitor" | "web"; version: string; }

export interface PairMsg  { t: "pair";  v: number; code: string; client: ClientInfo; }
export interface HelloMsg { t: "hello"; v: number; token: string; client: ClientInfo; }
export interface PressMsg { t: "press"; id: number; actionId: Id; }
export interface NotifDismissMsg { t: "notif_dismiss"; notifId: string; }
export interface PingMsg  { t: "ping"; ts: number; }
// M16: ответ на запрос разрешения Claude Code (DATA_MODEL.md §6.8). Не команда: reqId выдан агентом,
// одноразовый, ничего не несёт, кроме выбора. Неизвестный/истёкший reqId — молча игнорируется.
export interface PermAnswerMsg { t: "perm_answer"; reqId: string; decision: "allow" | "deny" | "pc"; } // reqId ^[0-9a-f]{16}$
// M17: открыть программу уведомления Windows (DATA_MODEL.md §10.4). Только id, выданный агентом.
export interface NotifOpenMsg { t: "notif_open"; notifId: string; }
// M22 (DATA_MODEL.md §13.4): отметить пункт списка на телефоне. Не команда: оба id выданы агентом, неизвестные — молча игнорируются.
export interface ListToggleMsg { t: "list_toggle"; listId: string; itemId: string; } // id ^[0-9a-f]{16}$
// M21 (DATA_MODEL.md §12): голос с телефона. Только внутри канала v2. Звук — данные; что делать, решает ПК (S24).
export interface VoiceStartMsg  { t: "voice_start"; id: string; }                        // id — 16 hex от клиента, одна активная запись на устройство
export interface VoiceChunkMsg  { t: "voice_chunk"; id: string; seq: number; data: string; } // base64 PCM16 16 кГц моно, ≤ 3200 байт (100 мс)
export interface VoiceEndMsg    { t: "voice_end"; id: string; }
export interface VoiceCancelMsg { t: "voice_cancel"; id: string; }
export interface VoiceConfirmMsg { t: "voice_confirm"; id: string; yes: boolean; }
// Секция FullState.voice?: VoiceInfo | null:
// { state: "idle"|"listening"|"loading"|"thinking"|"confirm"|"done"|"error"; id?: string; source?: "pc"|"phone";
//   heard?: string (≤ 300); reply?: string (≤ 300); confirm?: { text: string; key?: string; args?: Record<string,string> };
//   replyKey?: string; replyArgs?: Record<string,string> }
// Тексты: постоянные фразы агента ("Didn't catch that", "Cancelled", вопросы подтверждения) идут ключом i18n
// (replyKey / confirm.key + args, ключи voice.reply.* и voice.confirm.* в shared/src/i18n), интерфейс показывает t(key, args).
// reply и confirm.text — только свободный текст (ответ модели, имена программ) и запасной вариант, если ключа нет в словаре.
// Лимиты: запись ≤ 30 с (больше — обрезается и завершается), seq начинается с 0 и растёт строго +1, ≤ 15 кусков в секунду на устройство.
// voice_end / voice_cancel / voice_confirm с неизвестным id молча игнорируются; voice_chunk с неизвестным id → BAD_MESSAGE.
// M20 (TASKS T20.4): загрузка нового APK с ПК внутри зашифрованного канала.
export interface ApkGetMsg { t: "apk_get"; offset: number; }                       // offset кратен 3000
export type ClientMsg = PairMsg | HelloMsg | PressMsg | NotifDismissMsg | PingMsg | PermAnswerMsg | NotifOpenMsg | ApkGetMsg;
// S→C: { t: "apk_chunk"; offset: number; data: string /* base64, ≤ 3000 байт */; total: number; sha256: string /* hex */ } — ApkChunkMsg;
// секция FullState.update?: UpdateInfo2 | null, где UpdateInfo2 = { app?: { version: string; size: number; sha256: string } }.

// ===== Сервер -> Клиент (WS) =====
export interface PairedMsg  { t: "paired"; deviceId: Id; token: string; }
export interface WelcomeMsg {
  t: "welcome"; v: number;
  agent: { name: string; version: string };
  deviceId: Id; rev: number; state: FullState;
}
export interface PatchMsg<K extends SectionName = SectionName> {
  t: "patch"; rev: number; section: K; value: FullState[K];
}
export interface PressResultMsg { t: "press_result"; id: number; ok: boolean; error?: { code: ErrorCode; message: string }; }
export interface PongMsg  { t: "pong"; ts: number; }
export interface ErrorMsg { t: "error"; code: ErrorCode; message: string; fatal: boolean; supported?: number[]; }
export type ServerMsg = PairedMsg | WelcomeMsg | PatchMsg | PressResultMsg | PongMsg | ErrorMsg;

// ===== Состояние (секции) =====
export type SectionName = "layout" | "sys" | "statuses" | "notifications" | "vars" | "focus" | "media" | "weather" | "obs" | "ai"; // weather, obs — M15; ai — M16
export interface FullState {
  layout: ClientLayout;            // см. DATA_MODEL.md §3
  sys: SysStats | null;            // null = сборщик выключен
  statuses: StatusItem[];          // отсортированы по (group, id)
  notifications: Notification[];   // новые первыми, максимум 20
  vars: Record<VarName, VarValue>;
  focus?: FocusInfo | null;        // M11; отсутствует у агентов до M11; null = сборщик выключен
  media?: MediaInfo | null;        // M11; null = ничего не играет или сборщик выключен
  icons?: Record<string, string>;  // M12: "<hash16>" → "data:image/png;base64,..."; только иконки, на которые ссылается раскладка
  weather?: WeatherInfo | null;    // M15; null = выключено или ещё нет данных
  obs?: ObsInfo | null;            // M15; null = интеграция выключена
  ai?: AiInfo | null;              // M16; null = обе интеграции выключены
  strip?: StripInfo | null;        // M17 §10.5; null = полосы нет
  battery?: BatteryInfo | null;    // M17 §10.6; null = сборщик не нужен или нет данных
  mail?: MailInfo | null;          // M17 §10.7; null = нет ящиков
}

// M17 (DATA_MODEL.md §10). Имена секций добавляются в SectionName: "strip" | "battery" | "mail".
// M22 (DATA_MODEL.md §13.7). Ещё две секции: "reminders" | "lists". M24 (§14): секция "lyrics" (LyricsInfo | null).
export interface RemindersSection { items: { id: string; text: string; at: string; repeat?: "daily" | "weekdays" | "weekly" }[] } // ближайшие 5 несделанных, at — локальное ISO
export interface ListsSection { lists: { id: string; name: string; items: { id: string; text: string; done: boolean }[] }[] } // ≤ 50 пунктов в списке
export interface StripInfo { app: string; template: string; name: string; icon?: string /* T17.6: "img:<hash16>" */; keys: { actionId: Id; label: string; icon?: string }[]; } // keys <= 16
export interface BatteryInfo { devices: { name: string; pct: number; kind: "pc" | "bt"; charging?: boolean }[]; updatedAt: number; } // <= 8
export interface MailInfo { accounts: { id: Id; name: string; unread: number | null; error?: "auth" | "network"; openActionId?: Id }[]; updatedAt: number; } // <= 3; openActionId — id действия для тапа (принято 2026-11-19)

// M16: лимиты Claude Code / Codex и запросы разрешений (DATA_MODEL.md §6.8).
export interface AiWindow {
  label: string;                   // "5h" | "7d" | "30d" | "<n>h" — из длины окна
  pct: number;                     // 0..100, одна цифра после запятой
  resetsAt: number;                // ts сброса окна (мс)
}
export interface AiProvider {
  windows: AiWindow[];             // 0..2, короткое окно первым; окна с resetsAt в прошлом агент удаляет
  plan?: string;                   // Codex plan_type ("plus", "pro", "free"); у Claude нет
  updatedAt: number;               // ts последних данных
}
export interface AiSession {
  id: string;                      // первые 8 символов session_id
  project: string;                 // basename(cwd), <= 60
  model?: string;                  // model.display_name, <= 40
  ctxPct?: number;                 // context_window.used_percentage, целое
  lastSeen: number;                // ts последнего statusline
}
export interface PermRequest {
  reqId: string;                   // 16 hex, случайный
  tool: string;                    // tool_name, <= 60
  summary: string;                 // что именно (команда, файл, URL), <= 300
  project: string;                 // basename(cwd), <= 60
  sessionId: string;               // первые 8 символов
  expiresAt: number;               // ts, после которого агент сам отпустит запрос в терминал
  danger?: boolean;                // M18: опасная команда — Allow удержанием 1 с, без Always allow (DATA_MODEL.md §11.3)
}
export interface AiInfo {
  claude?: AiProvider | null;      // null = нет данных (не установлено или ещё не было сессии)
  codex?: AiProvider | null;       // null = выключено или Codex не найден
  sessions: AiSession[];           // Claude Code, виденные за 10 мин, новые первыми, <= 6
  pending: PermRequest[];          // ожидают ответа, старые первыми, <= 5
}

// M15: погода (DATA_MODEL.md §6.3). Агент получает её сам; телефон в интернет не ходит.
export interface WeatherInfo {
  place: string;                   // <= 80, имя места из настроек
  units: "metric" | "imperial";
  updatedAt: number;               // ts получения данных
  current: { temp: number; code: number; isDay: boolean; wind: number };   // temp — целое °C/°F; code — WMO; wind — целое км/ч или mph
  daily: { date: string; min: number; max: number; code: number }[];      // ровно 3 дня, date "YYYY-MM-DD", начиная с сегодня
  hourly?: { time: string; temp: number; code: number; isDay: boolean }[]; // 0.23 (DESIGN §4.16): 6 часов начиная со следующего, time "HH:mm" локальное
}

// M24 (DATA_MODEL.md §14): текст текущей песни. Секция "lyrics"; null = выключено, ничего не играет или текста нет.
export interface LyricsInfo {
  trackKey: string;                // ^[0-9a-f]{16}$ — хеш (исполнитель|название|длительность), выдан ПК
  synced: boolean;                 // true — строки с временем; false — обычный текст без подсветки
  lines: { t?: number; text: string }[]; // t — мс от начала трека (только synced); ≤ 300 строк × ≤ 200 символов
  offsetMs: number;                // сдвиг пользователя для этого трека, −30000..30000, шаг 500
}
// Телефон → ПК. Не команда: trackKey выдан ПК, неизвестный — молча игнорируется; delta только ±500.
export interface LyricsOffsetMsg { t: "lyrics_offset"; trackKey: string; deltaMs: -500 | 500; }

// M15: OBS Studio (DATA_MODEL.md §6.4). Только состояние; пароль OBS никогда не попадает в протокол.
export interface ObsInfo {
  connected: boolean;
  recording: boolean;
  recordStartedAt?: number;        // ts начала записи (клиент сам считает таймер)
  streaming: boolean;
  streamStartedAt?: number;
  scene?: string;                  // текущая сцена, <= 100
  replayBuffer: boolean;           // буфер повтора включён (для кнопки Save replay)
}

// Активное окно ПК: только имя исполняемого файла, НИКОГДА не заголовок окна.
export interface FocusInfo {
  app: string | null;              // имя exe в нижнем регистре ("code.exe"); null = неизвестно (рабочий стол, экран блокировки)
}

// Текущая медиасессия Windows (Spotify, браузер, плеер).
export interface MediaInfo {
  source: string;                  // идентификатор приложения-источника от Windows (<= 120), только для показа
  title: string;                   // <= 200
  artist?: string;                 // <= 200
  album?: string;                  // <= 200
  playing: boolean;
  positionSec?: number;            // целое; позиция на момент positionAt
  durationSec?: number;            // целое
  positionAt: number;              // ts момента, к которому относится positionSec
  art?: string;                    // "data:image/png;base64,..." или "data:image/jpeg;base64,...", <= 65536 символов
  shuffle?: boolean;               // M15: из GSMTC PlaybackInfo.IsShuffleActive; нет у источника — поле отсутствует
  repeat?: "none" | "list" | "track"; // M15: из PlaybackInfo.AutoRepeatMode
}

export interface SysStats {
  cpu: number;                               // 0..100, целое
  mem: { usedMb: number; totalMb: number };  // целые
  disks: { mount: string; usedGb: number; totalGb: number }[]; // целые, только фиксированные диски
  net: { rxKbps: number; txKbps: number };   // целые, сумма по всем интерфейсам
  uptimeSec: number;                         // округлено вниз до 60
}

export type StatusState = "ok" | "warn" | "error" | "running" | "unknown";
export interface StatusItem {
  id: Id; label: string; state: StatusState;
  text?: string;                 // <= 80 символов
  group?: string;                // <= 40 символов
  source: "push" | "check";
  updatedAt: number;
}

export type NotifLevel = "info" | "success" | "warning" | "error";
export interface Notification {
  id: string;                    // десятичная строка счётчика: "1", "2", ...
  ts: number; title: string;     // title <= 120
  body?: string;                 // <= 500
  level: NotifLevel;
  source?: string;               // <= 40; M17: "windows" — уведомление Windows (DATA_MODEL.md §10.4)
  app?: string;                  // M17: отображаемое имя программы, <= 60
  icon?: string;                 // M17: "img:<hash16>"
  openable?: boolean;            // M17: тап шлёт notif_open
  hidden?: boolean;              // M18: текст скрыт настройкой приватности (DATA_MODEL.md §11.5)
}

// ===== HTTP =====
export interface HealthRes { name: "decka"; version: string; protocol: number; }
export interface NotifyReq { title: string; body?: string; level?: NotifLevel; source?: string; }
export interface NotifyRes { id: string; }
export interface StatusReq { id: Id; state: StatusState; label?: string; text?: string; group?: string; ttlSec?: number; }
export interface OkRes { ok: true; }
export interface VarReq { value: VarValue; }
export interface HttpError { error: { code: ErrorCode; message: string } }
```

`ClientLayout`, `LayoutItem`, `DisplaySettings` определены в DATA_MODEL.md §3 и лежат в `/shared/src/layout.ts`.

## 4. Сценарии WebSocket

### 4.1 Сопряжение (первое подключение)

1. Клиент открывает сокет, шлёт `pair`.
2. Сервер проверяет код (правила — ARCHITECTURE.md §6.2). Успех → `paired`, затем сразу `welcome` (сокет уже авторизован, `hello` не нужен).
3. Неуспех → `error` `PAIR_CODE_INVALID`, `fatal: true`, закрытие 4401. Причина (истёк / неверный / исчерпаны попытки) клиенту не сообщается.

`fixtures/c2s-pair.json`
```json
{ "t": "pair", "v": 1, "code": "K7M2QX9D", "client": { "name": "Galaxy S9", "app": "capacitor", "version": "0.1.0" } }
```
`fixtures/s2c-paired.json`
```json
{ "t": "paired", "deviceId": "d-3f9a1c", "token": "pX3v8Zq1mB7kT0aYw2LrN5cE9uHs4JdG6fQoViC_-AA" }
```

### 4.2 Обычное подключение

Клиент шлёт `hello` → сервер отвечает `welcome` с полным состоянием. Неверный токен → `error UNAUTHORIZED` fatal + закрытие 4401. При отзыве запись устройства удаляется, поэтому `hello` с токеном отозванного устройства неотличим от неверного токена: `UNAUTHORIZED` + 4401. `DEVICE_REVOKED` + 4403 шлётся только в уже открытые сокеты в момент отзыва. Клиент на оба кода реагирует одинаково: удаляет токен и показывает экран сопряжения.

`fixtures/c2s-hello.json`
```json
{ "t": "hello", "v": 1, "token": "pX3v8Zq1mB7kT0aYw2LrN5cE9uHs4JdG6fQoViC_-AA", "client": { "name": "Galaxy S9", "app": "capacitor", "version": "0.1.0" } }
```
`fixtures/s2c-welcome.json`
```json
{
  "t": "welcome", "v": 1,
  "agent": { "name": "DESKTOP-MAIN", "version": "0.1.0" },
  "deviceId": "d-3f9a1c", "rev": 42,
  "state": {
    "layout": {
      "grid": { "cols": 5, "rows": 3 },
      "display": { "dimAfterSec": 60, "dimLevel": 0.3, "sleepAfterSec": 600, "pixelShift": true, "night": { "enabled": false, "from": "23:00", "to": "07:00" } },
      "pages": [ { "id": "main", "name": "Main", "items": [
        { "id": "b-play", "kind": "button", "x": 1, "y": 1, "w": 1, "h": 1, "label": "Play", "icon": "lucide:play", "actionId": "media-play" },
        { "id": "w-clock", "kind": "clock", "x": 0, "y": 0, "w": 2, "h": 1 }
      ] } ]
    },
    "sys": { "cpu": 12, "mem": { "usedMb": 9120, "totalMb": 32000 }, "disks": [ { "mount": "C:", "usedGb": 210, "totalGb": 476 } ], "net": { "rxKbps": 80, "txKbps": 12 }, "uptimeSec": 86400 },
    "statuses": [ { "id": "api-prod", "label": "API prod", "state": "ok", "text": "200 · 84 ms", "group": "services", "source": "check", "updatedAt": 1794650000000 } ],
    "notifications": [ { "id": "7", "ts": 1794650001000, "title": "Deploy finished", "body": "web@1.4.2", "level": "success", "source": "ci" } ],
    "vars": { "mic.muted": false }
  }
}
```

### 4.3 Обновления состояния

Сервер хранит глобальный счётчик `rev` (u64, +1 на каждое фактическое изменение любой секции). `patch` всегда несёт **полное новое значение секции** (частичных патчей нет). Рассылка только если новое значение секции не равно старому (глубокое сравнение JSON). Клиент заменяет секцию целиком; `patch` с `rev` ≤ уже применённого игнорирует.

`fixtures/s2c-patch-vars.json`
```json
{ "t": "patch", "rev": 43, "section": "vars", "value": { "mic.muted": true } }
```

### 4.4 Нажатие кнопки

Клиент шлёт `press` с собственным возрастающим `id` (начиная с 1 на каждое соединение). Сервер отвечает `press_result` с тем же `id` после завершения запуска действия (не после завершения запущенной программы). Таймаут ожидания на клиенте — 5 с, после чего кнопка показывает ошибку.

`fixtures/c2s-press.json`
```json
{ "t": "press", "id": 17, "actionId": "media-play" }
```
`fixtures/s2c-press-result-ok.json`
```json
{ "t": "press_result", "id": 17, "ok": true }
```
`fixtures/s2c-press-result-err.json`
```json
{ "t": "press_result", "id": 18, "ok": false, "error": { "code": "UNKNOWN_ACTION", "message": "action 'foo' is not defined" } }
```

Лимит: не более 20 `press` в секунду на соединение; сверх лимита → `press_result` с `RATE_LIMITED` (сокет не закрывается).

### 4.5 Прочее

Секции M11. `focus` меняется при смене активного приложения; `media` — при смене трека, состояния воспроизведения, длительности или при перемотке (позиция отклонилась от расчётной больше чем на 3 с). Ход позиции во время воспроизведения клиент вычисляет сам: `positionSec + (now − positionAt)/1000`, не больше `durationSec`.

`fixtures/s2c-patch-focus.json`
```json
{ "t": "patch", "rev": 44, "section": "focus", "value": { "app": "code.exe" } }
```
`fixtures/s2c-patch-media.json`
```json
{ "t": "patch", "rev": 45, "section": "media", "value": { "source": "Spotify.exe", "title": "Midnight City", "artist": "M83", "album": "Hurry Up, We're Dreaming", "playing": true, "positionSec": 42, "durationSec": 244, "positionAt": 1794650003000 } }
```

`fixtures/c2s-notif-dismiss.json` — удаляет уведомление у всех клиентов (сервер меняет секцию `notifications`); неизвестный `notifId` молча игнорируется.
```json
{ "t": "notif_dismiss", "notifId": "7" }
```
`fixtures/c2s-ping.json` / `fixtures/s2c-pong.json` — сервер возвращает тот же `ts`.
```json
{ "t": "ping", "ts": 1794650002000 }
```
```json
{ "t": "pong", "ts": 1794650002000 }
```
`fixtures/s2c-error.json`
```json
{ "t": "error", "code": "PROTOCOL_VERSION", "message": "unsupported protocol version 2", "fatal": true, "supported": [1] }
```

До авторизации любое сообщение кроме `pair`/`hello` → `UNAUTHORIZED` fatal + 4401. Невалидный JSON или сообщение не по схеме после авторизации → `error BAD_MESSAGE`, `fatal: false`, сокет остаётся открытым.

## 5. HTTP API

Авторизация всех эндпоинтов кроме `/api/health`: заголовок `Authorization: Bearer <apiToken>` (`apiToken` из `config.json`, формат `dk_` + 43 символа base64url). Токен в query-строке не принимается. CORS-заголовки сервер NEVER не отдаёт (API предназначен для скриптов, не для браузеров). Ошибки — тело `HttpError`.

| Метод и путь | Тело | Успех | Ошибки |
|---|---|---|---|
| `GET /api/health` | — | 200 `HealthRes` | — |
| `POST /api/notify` | `NotifyReq` (JSON) **или** `text/plain` | 202 `NotifyRes` | 400 `BAD_MESSAGE`, 401 `UNAUTHORIZED`, 413, 429 `RATE_LIMITED` |
| `POST /api/status` | `StatusReq` | 200 `OkRes` | 400, 401, 413, 429 |
| `DELETE /api/status/{id}` | — | 200 `OkRes` | 401, 404 `NOT_FOUND` |
| `PUT /api/vars/{name}` | `VarReq` | 200 `OkRes` | 400, 401, 413, 429 |
| `DELETE /api/vars/{name}` | — | 200 `OkRes` | 401, 404 |

Правила:
- `POST /api/notify` с `Content-Type: text/plain` (или без Content-Type): всё тело, обрезанное до 120 символов, становится `title`; `level = "info"`.
- Начальный UTF-8 BOM (`EF BB BF`) в теле любого запроса отбрасывается до разбора (Windows PowerShell 5.1 пишет файлы с BOM).
- `level` по умолчанию `"info"`. Строки длиннее лимитов (title 120, body 500, source 40, text 80, group 40, label 60) → 400 `BAD_MESSAGE` (не обрезаются; исключение — text/plain выше).
- `POST /api/status`: upsert по `id`, `source` всегда `"push"`. Если элемента нет и `label` не передан → `label = id`. Если элемент есть и `label`/`group` не переданы → сохраняются прежние; `text` не передан → очищается. `ttlSec` (целое 1..86400): по истечении без нового upsert элемент получает `state: "unknown"` и `text: "stale"`. Элемент с `source: "check"` через API изменить нельзя → 400.
- `PUT /api/vars/{name}`: строковое значение ≤ 200 символов.
- M15: переменные сборщиков зарезервированы — имена с префиксами `audio.`, `mic.`, `media.`, `obs.` (M16: и `ai.`). `PUT`/`DELETE` для них → 400 `BAD_MESSAGE`, `message: "reserved variable"`. `toggleVar` с таким именем в `actions.json` — ошибка валидации `toggleVar: "<name>" is set by Decka itself — remove it`.
- M16, эндпоинты Claude Code (подробно — DATA_MODEL.md §6.8): `POST /api/cc/statusline` (тело — JSON statusline Claude Code, ответ — `text/plain` строка статуса) и `POST /api/cc/permission` (тело — JSON хука `PermissionRequest`, ответ держится до решения, затем JSON решения или пустое тело 200). Только с адреса 127.0.0.1/::1 (иначе 404), только `Authorization: Bearer <ai.hookToken>` (apiToken не подходит, иначе 401). Тело до 1 МБ (не 16 КБ), `Content-Type` не проверяется. В общий лимит 60/мин не входят; свой лимит — 20 запросов в секунду.
- Лимит: 60 запросов в минуту на IP-адрес отправителя суммарно по всем авторизованным эндпоинтам → 429.
- Неверный/отсутствующий токен → 401; после 10 ответов 401 за минуту с одного IP → 429 на 60 с.

Примеры:

`fixtures/http-notify-req.json`
```json
{ "title": "Deploy finished", "body": "web@1.4.2", "level": "success", "source": "ci" }
```
`fixtures/http-status-req.json`
```json
{ "id": "claude-decka", "label": "Claude: Decka", "state": "running", "text": "editing files", "group": "ai", "ttlSec": 900 }
```
`fixtures/http-var-req.json`
```json
{ "value": true }
```
`fixtures/http-error.json`
```json
{ "error": { "code": "UNAUTHORIZED", "message": "missing or invalid API token" } }
```

Проверка из PowerShell (`$t` — apiToken):
```
curl.exe -s -X POST http://127.0.0.1:9247/api/notify -H "Authorization: Bearer $t" -H "Content-Type: application/json" --data-binary "@shared/fixtures/http-notify-req.json"
```

## 6. Коды ошибок

| Код | HTTP | WS: fatal / код закрытия | Когда |
|---|---|---|---|
| `UNAUTHORIZED` | 401 | да / 4401 | нет токена, неверный токен, сообщение до авторизации, таймаут рукопожатия |
| `PAIR_CODE_INVALID` | — | да / 4401 | код неверный, истёк, использован или исчерпаны попытки |
| `DEVICE_REVOKED` | — | да / 4403 | только в открытый сокет в момент отзыва устройства (на `hello` с отозванным токеном — `UNAUTHORIZED`) |
| `PROTOCOL_VERSION` | — | да / 4426 | `v` не поддерживается |
| `BAD_MESSAGE` | 400 | нет | невалидный JSON, нарушение схемы или лимитов длины |
| `UNKNOWN_ACTION` | — | нет (в `press_result`) | `actionId` нет в actions.json |
| `ACTION_FAILED` | — | нет (в `press_result`) | исполнитель вернул ошибку (программа не найдена и т. п.) |
| `RATE_LIMITED` | 429 | нет; для `pair` — да / 4429 | превышен лимит |
| `PAYLOAD_TOO_LARGE` | 413 | закрытие 1009 без `error` | превышен размер |
| `NOT_FOUND` | 404 | — | нет такого status/var/пути `/api/*` |
| `INTERNAL` | 500 | нет | непредвиденная ошибка сервера |

`message` — английский текст для логов; клиент показывает пользователю собственные тексты по `code`.

## 7. JSON Schema (draft 2020-12)

Файл `/shared/schemas/protocol.schema.json`. Валидируется в тестах `ajv` (импорт `ajv/dist/2020`). `$defs.ClientLayout` — ссылка на `layout.schema.json#/$defs/ClientLayout` (DATA_MODEL.md §5).

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://decka.invalid/schemas/protocol.schema.json",
  "$defs": {
    "Id": { "type": "string", "pattern": "^[a-z0-9_-]{1,64}$" },
    "VarName": { "type": "string", "pattern": "^[a-z0-9_.-]{1,64}$" },
    "VarValue": { "oneOf": [ { "type": "string", "maxLength": 200 }, { "type": "number" }, { "type": "boolean" } ] },
    "ErrorCode": { "enum": ["UNAUTHORIZED","PAIR_CODE_INVALID","DEVICE_REVOKED","PROTOCOL_VERSION","BAD_MESSAGE","UNKNOWN_ACTION","ACTION_FAILED","RATE_LIMITED","PAYLOAD_TOO_LARGE","NOT_FOUND","INTERNAL"] },
    "ClientInfo": {
      "type": "object", "required": ["name","app","version"],
      "properties": { "name": { "type": "string", "minLength": 1, "maxLength": 60 }, "app": { "enum": ["capacitor","web"] }, "version": { "type": "string", "maxLength": 20 } }
    },
    "PairMsg": { "type": "object", "required": ["t","v","code","client"],
      "properties": { "t": { "const": "pair" }, "v": { "type": "integer" }, "code": { "type": "string", "pattern": "^[A-Z2-9]{8}$" }, "client": { "$ref": "#/$defs/ClientInfo" } } },
    "HelloMsg": { "type": "object", "required": ["t","v","token","client"],
      "properties": { "t": { "const": "hello" }, "v": { "type": "integer" }, "token": { "type": "string", "pattern": "^[A-Za-z0-9_-]{43}$" }, "client": { "$ref": "#/$defs/ClientInfo" } } },
    "PressMsg": { "type": "object", "required": ["t","id","actionId"],
      "properties": { "t": { "const": "press" }, "id": { "type": "integer", "minimum": 1 }, "actionId": { "$ref": "#/$defs/Id" } } },
    "NotifDismissMsg": { "type": "object", "required": ["t","notifId"],
      "properties": { "t": { "const": "notif_dismiss" }, "notifId": { "type": "string", "pattern": "^[0-9]{1,20}$" } } },
    "PingMsg": { "type": "object", "required": ["t","ts"], "properties": { "t": { "const": "ping" }, "ts": { "type": "number" } } },
    "ClientMsg": { "oneOf": [ { "$ref": "#/$defs/PairMsg" }, { "$ref": "#/$defs/HelloMsg" }, { "$ref": "#/$defs/PressMsg" }, { "$ref": "#/$defs/NotifDismissMsg" }, { "$ref": "#/$defs/PingMsg" } ] },

    "SysStats": { "type": "object", "required": ["cpu","mem","disks","net","uptimeSec"],
      "properties": {
        "cpu": { "type": "integer", "minimum": 0, "maximum": 100 },
        "mem": { "type": "object", "required": ["usedMb","totalMb"], "properties": { "usedMb": { "type": "integer", "minimum": 0 }, "totalMb": { "type": "integer", "minimum": 0 } } },
        "disks": { "type": "array", "items": { "type": "object", "required": ["mount","usedGb","totalGb"], "properties": { "mount": { "type": "string" }, "usedGb": { "type": "integer", "minimum": 0 }, "totalGb": { "type": "integer", "minimum": 0 } } } },
        "net": { "type": "object", "required": ["rxKbps","txKbps"], "properties": { "rxKbps": { "type": "integer", "minimum": 0 }, "txKbps": { "type": "integer", "minimum": 0 } } },
        "uptimeSec": { "type": "integer", "minimum": 0 } } },
    "StatusState": { "enum": ["ok","warn","error","running","unknown"] },
    "StatusItem": { "type": "object", "required": ["id","label","state","source","updatedAt"],
      "properties": { "id": { "$ref": "#/$defs/Id" }, "label": { "type": "string", "minLength": 1, "maxLength": 60 }, "state": { "$ref": "#/$defs/StatusState" },
        "text": { "type": "string", "maxLength": 80 }, "group": { "type": "string", "maxLength": 40 }, "source": { "enum": ["push","check"] }, "updatedAt": { "type": "number" } } },
    "NotifLevel": { "enum": ["info","success","warning","error"] },
    "Notification": { "type": "object", "required": ["id","ts","title","level"],
      "properties": { "id": { "type": "string", "pattern": "^[0-9]{1,20}$" }, "ts": { "type": "number" }, "title": { "type": "string", "minLength": 1, "maxLength": 120 },
        "body": { "type": "string", "maxLength": 500 }, "level": { "$ref": "#/$defs/NotifLevel" }, "source": { "type": "string", "maxLength": 40 } } },
    "ClientLayout": { "$ref": "layout.schema.json#/$defs/ClientLayout" },
    "FullState": { "type": "object", "required": ["layout","sys","statuses","notifications","vars"],
      "properties": {
        "layout": { "$ref": "#/$defs/ClientLayout" },
        "sys": { "oneOf": [ { "$ref": "#/$defs/SysStats" }, { "type": "null" } ] },
        "statuses": { "type": "array", "items": { "$ref": "#/$defs/StatusItem" } },
        "notifications": { "type": "array", "maxItems": 20, "items": { "$ref": "#/$defs/Notification" } },
        "vars": { "type": "object", "propertyNames": { "pattern": "^[a-z0-9_.-]{1,64}$" }, "additionalProperties": { "$ref": "#/$defs/VarValue" } },
        "icons": { "type": "object", "maxProperties": 200, "propertyNames": { "pattern": "^[0-9a-f]{16}$" },
          "additionalProperties": { "type": "string", "pattern": "^data:image/(png|svg\\+xml);base64,", "maxLength": 131072 } },
        "focus": { "oneOf": [ { "$ref": "#/$defs/FocusInfo" }, { "type": "null" } ] },
        "media": { "oneOf": [ { "$ref": "#/$defs/MediaInfo" }, { "type": "null" } ] } } },
    "FocusInfo": { "type": "object", "required": ["app"],
      "properties": { "app": { "oneOf": [ { "type": "string", "pattern": "^[a-z0-9._ ()-]{1,60}\\.exe$" }, { "type": "null" } ] } } },
    "MediaInfo": { "type": "object", "required": ["source","title","playing","positionAt"],
      "properties": { "source": { "type": "string", "maxLength": 120 }, "title": { "type": "string", "maxLength": 200 },
        "artist": { "type": "string", "maxLength": 200 }, "album": { "type": "string", "maxLength": 200 }, "playing": { "type": "boolean" },
        "positionSec": { "type": "integer", "minimum": 0 }, "durationSec": { "type": "integer", "minimum": 0 }, "positionAt": { "type": "number" },
        "art": { "type": "string", "pattern": "^data:image/(png|jpeg);base64,", "maxLength": 65536 } } },

    "PairedMsg": { "type": "object", "required": ["t","deviceId","token"],
      "properties": { "t": { "const": "paired" }, "deviceId": { "$ref": "#/$defs/Id" }, "token": { "type": "string", "pattern": "^[A-Za-z0-9_-]{43}$" } } },
    "WelcomeMsg": { "type": "object", "required": ["t","v","agent","deviceId","rev","state"],
      "properties": { "t": { "const": "welcome" }, "v": { "type": "integer" },
        "agent": { "type": "object", "required": ["name","version"], "properties": { "name": { "type": "string" }, "version": { "type": "string" } } },
        "deviceId": { "$ref": "#/$defs/Id" }, "rev": { "type": "integer", "minimum": 0 }, "state": { "$ref": "#/$defs/FullState" } } },
    "PatchMsg": { "type": "object", "required": ["t","rev","section","value"],
      "properties": { "t": { "const": "patch" }, "rev": { "type": "integer", "minimum": 0 }, "section": { "type": "string" }, "value": {} } },
    "ErrorBody": { "type": "object", "required": ["code","message"], "properties": { "code": { "$ref": "#/$defs/ErrorCode" }, "message": { "type": "string" } } },
    "PressResultMsg": { "type": "object", "required": ["t","id","ok"],
      "properties": { "t": { "const": "press_result" }, "id": { "type": "integer" }, "ok": { "type": "boolean" }, "error": { "$ref": "#/$defs/ErrorBody" } } },
    "PongMsg": { "type": "object", "required": ["t","ts"], "properties": { "t": { "const": "pong" }, "ts": { "type": "number" } } },
    "ErrorMsg": { "type": "object", "required": ["t","code","message","fatal"],
      "properties": { "t": { "const": "error" }, "code": { "$ref": "#/$defs/ErrorCode" }, "message": { "type": "string" }, "fatal": { "type": "boolean" }, "supported": { "type": "array", "items": { "type": "integer" } } } },
    "ServerMsg": { "oneOf": [ { "$ref": "#/$defs/PairedMsg" }, { "$ref": "#/$defs/WelcomeMsg" }, { "$ref": "#/$defs/PatchMsg" }, { "$ref": "#/$defs/PressResultMsg" }, { "$ref": "#/$defs/PongMsg" }, { "$ref": "#/$defs/ErrorMsg" } ] },

    "HealthRes": { "type": "object", "required": ["name","version","protocol"], "properties": { "name": { "const": "decka" }, "version": { "type": "string" }, "protocol": { "type": "integer" } } },
    "NotifyReq": { "type": "object", "required": ["title"],
      "properties": { "title": { "type": "string", "minLength": 1, "maxLength": 120 }, "body": { "type": "string", "maxLength": 500 }, "level": { "$ref": "#/$defs/NotifLevel" }, "source": { "type": "string", "maxLength": 40 } } },
    "StatusReq": { "type": "object", "required": ["id","state"],
      "properties": { "id": { "$ref": "#/$defs/Id" }, "state": { "$ref": "#/$defs/StatusState" }, "label": { "type": "string", "minLength": 1, "maxLength": 60 },
        "text": { "type": "string", "maxLength": 80 }, "group": { "type": "string", "maxLength": 40 }, "ttlSec": { "type": "integer", "minimum": 1, "maximum": 86400 } } },
    "VarReq": { "type": "object", "required": ["value"], "properties": { "value": { "$ref": "#/$defs/VarValue" } } },
    "HttpError": { "type": "object", "required": ["error"], "properties": { "error": { "$ref": "#/$defs/ErrorBody" } } }
  }
}
```

Схемы намеренно без `additionalProperties: false` (правило совместимости из §2). `PatchMsg.value` проверяется клиентом по секции: `layout`→`ClientLayout`, `sys`→`SysStats|null`, `statuses`→`StatusItem[]`, `notifications`→`Notification[]`, `vars`→map, `focus`→`FocusInfo|null`, `media`→`MediaInfo|null`. `focus` и `media` в `FullState` не обязательны (агент до M11 их не шлёт); клиент считает отсутствующую секцию равной `null`.

## 10. Защищённый канал, протокол v2 (M18, 2026-11-20, D-50)

Заменяет открытый WebSocket v1. Цель — чтобы в любой сети, включая общественные, перехват трафика не давал ни токена, ни содержимого, а подмена агента или телефона не проходила. Никаких сертификатов и ручных шагов: ключи выводятся из секретов, которые уже есть у обеих сторон или передаются вне сети (QR).

**Библиотеки (новых крейтов нет):** Rust — `ring` 0.17.14 (уже в `Cargo.lock` через rustls; прямой зависимостью ровно этой версии): `aead::AES_256_GCM`, `hmac` (HMAC-SHA256, сравнение `hmac::verify` — за постоянное время), `hkdf::HKDF_SHA256`, `pbkdf2::PBKDF2_HMAC_SHA256`, `rand::SystemRandom`. Клиент — `@noble/ciphers@2.4.0` (`gcm`) и `@noble/hashes@2.4.0` (`sha256`, `hmac`, `hkdf`, `pbkdf2`), случайные байты — `crypto.getRandomValues` (работает и в веб-режиме по http, где `crypto.subtle` недоступен). Своих криптопримитивов не писать.

**Секреты:**
- `K_dev` = SHA-256(токен устройства), 32 байта. Телефон считает его из своего токена; ПК уже хранит именно этот хеш. Поэтому сопряжённые телефоны переходят на v2 **без повторного сопряжения**. На ПК `K_dev` отныне хранится как `dpapi:<base64>` (`devices.json`, поле `keyDpapi`; старое поле хеша переносится при первом запуске и удаляется). Это нужно, потому что хеш стал ключом входа.
- `S_qr` — 32 случайных байта, создаются вместе с кодом сопряжения и живут столько же (120 с). Передаются только в QR, во фрагменте URL (`#…&s=<base64url>`): фрагмент не уходит по сети ни в HTTP-запросе, ни в WebSocket.
- `S_code` (ручной ввод кода) = PBKDF2-HMAC-SHA256(пароль = CODE в верхнем регистре, соль = `"decka-pair-v2" ‖ cn ‖ sn`, 200 000 итераций, 32 байта). Медленная функция нужна, чтобы записанное рукопожатие нельзя было быстро перебрать по 8 символам кода. Ручной ввод разрешён только в частной сети и по USB (§11.2 DATA_MODEL).

**Рукопожатие** (текстовые кадры JSON, общий таймаут 5 с):
1. C→S `{ "t":"hello2", "v":2, "mode":"device"|"pair_qr"|"pair_code", "deviceId"?:"…", "cn":"<16 байт base64url>", "client":ClientInfo }`. `deviceId` — только для `device`.
2. S→C `{ "t":"challenge", "sn":"<16 байт base64url>", "agent":{"name","version"} }`.
3. Обе стороны: `ikm` = `K_dev` | `S_qr` | `S_code`; `okm` = HKDF-SHA256(соль = `cn ‖ sn`, ikm, info = `"decka v2 keys"`, 96 байт) → `k_auth` (0..32), `k_c2s` (32..64), `k_s2c` (64..96).
4. C→S `{ "t":"auth", "mac": base64url(HMAC-SHA256(k_auth, "C" ‖ cn ‖ sn)) }`. Сервер сверяет за постоянное время. Не совпало → закрытие 4401. Попытка засчитывается: для `pair_*` — в лимит 5 попыток кода, для `device` — в счётчик ошибок IP.
5. S→C `{ "t":"auth_ok", "mac": base64url(HMAC-SHA256(k_auth, "S" ‖ cn ‖ sn)) }`. Клиент сверяет. Не совпало → закрытие и экран `This is not your PC — the answer did not match. Pair again if you reinstalled Decka.`: подмена агента, S14 закрыт. Повторов на этот адрес нет до ручного `Retry`.
6. Дальше **только бинарные кадры**: `counter` (8 байт, big-endian) ‖ AES-256-GCM(ключ направления, nonce = `00 00 00 00` ‖ counter, AAD = `"decka2"`, открытый текст = прежнее JSON-сообщение v1 в UTF-8). Счётчик в каждом направлении начинается с 0 и растёт на 1. Получатель требует ровно ожидаемое значение, иначе закрытие 4400 — так отсекаются повтор, перестановка и выпадение кадра. Ключи новые на каждое соединение (свежие `cn`/`sn`), поэтому nonce не повторяется.
7. Внутри — прежние сообщения v1 без изменений: `welcome`, `patch`, `press`, `perm_answer`, `ping`/`pong`… Лимит 4096 байт — на открытый текст кадра клиента. Для `pair_*` сервер после `auth_ok` шлёт зашифрованный `paired{deviceId, token}`, соединение закрывается, клиент переподключается в режиме `device`.

**Совместимость:** `PROTOCOL_VERSION = 2`. v1 (`hello`/`pair` открытым текстом) агент принимает только с loopback (USB через `adb reverse`, разработка, тесты); из сети — `error{code:"PROTOCOL_VERSION", supported:[2]}` и закрытие 4426. Старое приложение показывает `Update Decka on the phone`. Клиент v2 **никогда** не откатывается на v1, даже если сервер его предлагает.

**Ограничения:** рукопожатий — 10 в минуту на IP (в общественной сети — 5); 5 неудачных `auth` с IP → блок на 60 с.

**Тестовые векторы:** `shared/fixtures/crypto-v2.json` — фиксированные `token`, `cn`, `sn`, `CODE`, `S_qr` и ожидаемые `K_dev`, `S_code`, `okm`, оба `mac`, шифртекст кадра с `counter = 0` и `1`. Rust-тест создаёт и сверяет их, TS-тест сверяет те же значения. Так обе реализации гарантированно совместимы.

## 8. Формат QR и ручного сопряжения

**M18:** к фрагменту добавляется `&s=<S_qr base64url>` (§10). Веб-клиент по QR работает только в частной сети: в общественной агент его не отдаёт (DATA_MODEL.md §11.2).

QR содержит URL (одна строка):

```
http://<primaryIp>:<port>/#c=<CODE>&h=<ip1>,<ip2>&p=<port>&n=<agentName url-encoded>
```

- `CODE` — 8 символов алфавита `ABCDEFGHJKLMNPQRSTUVWXYZ23456789` (без `I`, `O`, `0`, `1`).
- `h` — все IPv4-адреса ПК из частных диапазонов, первым идёт `primaryIp`; максимум 4.
- Системная камера телефона открывает URL в браузере → веб-клиент, отданный агентом, читает `location.hash`, сопрягается и стирает hash (`history.replaceState`).
- Приложение Capacitor сканирует тот же QR встроенным сканером и разбирает тот же URL.
- Ручной ввод: поле «адрес ПК» (`host` или `host:port`) + поле кода (отображается как `XXXX-XXXX`, дефис и регистр при вводе игнорируются).

Код передаётся во фрагменте URL (`#`), а не в query, чтобы он не попадал в HTTP-запрос и логи.

## 9. IPC окна настроек (Tauri commands)

Только между React-окном настроек и Rust внутри агента; по сети недоступно. Имена команд — snake_case, вызываются через `invoke`. Типы лежат в `/shared/src/ipc.ts`. `ActionsFile`, `LayoutFile`, `ConfigFile`, `DeviceInfo` — DATA_MODEL.md.

```ts
export interface AgentStatus {
  version: string;
  focusApp: string | null;                            // M11: текущее активное приложение (для вкладки Status)
  listening: { bind: string; port: number } | null;   // null = порт занят / сервер не поднялся
  listenError?: string;
  lanIps: string[];
  clients: { deviceId: Id; name: string; remote: string; connectedAt: number }[];
  usb: { enabled: boolean; adbFound: boolean; devices: { serial: string; reversed: boolean }[]; lastError?: string };
  configDir: string;
}
export interface UpdateInfo {                         // M14
  current: string;                 // версия агента
  latest?: string;                 // из tag_name без "v"; нет — проверка не удалась или выключена
  url?: string;                    // html_url релиза (только https://github.com/iqonkz/decka/releases/...)
  available: boolean;              // latest новее current
  checkedAt?: number;              // unix ms
  error?: string;                  // для отображения в About мелким текстом
}
export interface InstalledApp {                       // M12
  name: string;          // имя ярлыка без .lnk
  program: string;       // полный путь к .exe
  args: string[];        // аргументы ярлыка, разбитые по правилам командной строки Windows
  cwd?: string;          // рабочая папка ярлыка
  icon?: string;         // data URL 32 px для списка (может отсутствовать)
}
export interface PairingInfo { code: string; url: string; qrSvg: string; expiresAt: number; }
export interface ValidationIssue { path: string; message: string; }   // path — JSON pointer
export type SaveResult = { ok: true } | { ok: false; issues: ValidationIssue[] };

export interface CcStatus {                           // M16
  installed: boolean;              // наши statusLine и хук есть в ~/.claude/settings.json
  statusLineConflict: boolean;     // statusLine занят чужой командой — ставим только хук, лимитов Claude не будет
  lastSeenAt?: number;             // последний statusline или хук
  codexFound: boolean;             // есть %USERPROFILE%\.codex\sessions
}

export interface IpcCommands {
  cc_status(): CcStatus;                                       // M16
  cc_install(): CcStatus;                                      // M16, DATA_MODEL.md §6.8
  cc_uninstall(): CcStatus;                                    // M16
  get_status(): AgentStatus;
  get_config(): ConfigFile;
  save_config(args: { config: ConfigFile }): SaveResult;       // port/bind применяются после restart_server
  restart_server(): AgentStatus;
  regenerate_api_token(): { apiToken: string };
  get_actions(): ActionsFile;
  save_actions(args: { actions: ActionsFile }): SaveResult;
  get_layout(): LayoutFile;
  save_layout(args: { layout: LayoutFile }): SaveResult;
  test_action(args: { actionId: Id }): { ok: boolean; error?: string };
  start_pairing(): PairingInfo;                                // отменяет предыдущий активный код
  cancel_pairing(): void;
  list_devices(): DeviceInfo[];
  revoke_device(args: { deviceId: Id }): void;
  send_test_notification(): void;                              // title "Decka test", level "info", source "decka"
  list_window_apps(): string[];                                // M11: имена exe (нижний регистр, без повторов, по алфавиту) процессов с видимыми окнами верхнего уровня, без decka-agent.exe
  get_templates(): PageTemplate[];                             // M11: DATA_MODEL.md §7
  add_template(args: { templateId: string }): SaveResult;      // M11: добавляет действия и страницу шаблона, сохраняет оба файла
  open_config_dir(): void;
  open_logs_dir(): void;                                       // T11.11: <configDir>\logs
  list_installed_apps(): InstalledApp[];                       // M12: ярлыки меню «Пуск», DATA_MODEL.md §8
  icon_from_program(args: { path: string }): { icon: string } | { error: string };  // M12: иконка exe → "img:<hash16>"
  icon_from_image(args: { path: string }): { icon: string } | { error: string };    // M12: png/jpg/ico/bmp → "img:<hash16>"
  get_icon_preview(args: { icon: string }): string | null;     // M12: data URL для окна настроек
  icon_from_svg(args: { svg: string }): { icon: string } | { error: string };       // M13: DATA_MODEL.md §8.4
  is_first_run(): boolean;                                     // M13: devices.json пуст и layout.json не менялся с создания (DESIGN.md §4.4)
  search_place(args: { query: string }): { name: string; country: string; region?: string; lat: number; lon: number }[] | { error: string };  // M15: геокодинг Open-Meteo, ≤ 5 результатов, query 2..80 символов
  refresh_weather(): WeatherInfo | { error: string };           // M15: немедленное обновление (кнопка в окне)
  set_obs_password(args: { password: string }): void;           // M15: шифрует DPAPI и сохраняет; "" — удалить пароль
  obs_status(): { connected: boolean; error?: string; scenes: string[]; inputs: string[] };  // M15: для окна (выбор сцены/источника звука при создании кнопок)
  check_updates(args: { force: boolean }): UpdateInfo;           // M14: ARCHITECTURE.md §8a; force=false → кеш последней проверки (≤ 24 ч), true → новый запрос
  refresh_program_icons(): { updated: number; failed: number };  // M14: DATA_MODEL.md §8.1; сохраняет раскладку, если updated > 0
  count_lowres_icons(): number;                                  // M14: число кнопок с img:-png < 256 px и действием launch (для строки «Sharpen» в Look)
  describe_program(args: { path: string }): InstalledApp | { error: string };   // M14: .exe → name = FileDescription версии файла (нет → имя файла без .exe), args [], icon 32 px; .lnk → разбор ярлыка как в list_installed_apps; иное расширение или файла нет → error. Только читает файл, ничего не запускает.
}
```

Событие Rust → окно: `decka://status-changed` (payload `AgentStatus`) — при подключении/отключении клиента, изменении USB-состояния, завершении сопряжения.
