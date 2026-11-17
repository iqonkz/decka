# BLOCKERS

Блокирующих задач нет. Ниже — замечание по дизайну, требующее решения владельца (не останавливает работу).

## D-1. Тема Glass: подписи на ярких участках обоев не достигают контраста 4.5:1 (T13.11, 2026-11-15)

**Что измерено** (`ui/test/contrast.test.ts`): белая подпись на стекле (слой `rgba(255,255,255,0.07)`) поверх самых ярких цветов обоев (худший случай — ровно этот цвет под клавишей, без размытия и затухания градиента):

| Обои | Самый яркий участок | Контраст белой подписи |
|---|---|---|
| aurora | `#0ea5e9` | 2.60 |
| aurora | `#10b981` | 2.39 |
| sunset | `#f97316` | 2.63 |
| ocean | `#0891b2` | 3.34 |
| graphite | `#3f3f46` | 8.52 |

Градиенты обоев затухают к `#05060a` вдали от центров, а фон клавиши размыт `blur(16px)`, поэтому на практике контраст выше; но гарантии ≥ 4.5 по токенам DESIGN.md §1.4 нет.

**Что сделано:** тест проверяет Glass как «> 2» и «graphite ≥ 4.5»; остальные темы (AMOLED, Studio, Material) и все 8 акцентов проходят ≥ 4.5 строго. Для цветных клавиш Studio/Glass исполнитель сделал адаптивную подпись (белая или почти чёрная) и затемнение средних тонов, пока белая подпись не достигнет 4.5 (`readableFace`).

**Варианты решения (выбор за владельцем/архитектором):** (а) подложка `rgba(0,0,0,0.25)` под стеклом; (б) усилить `text-shadow` подписи Glass до `0 1px 3px rgba(0,0,0,0.6)`; (в) затемнить яркие остановки обоев `aurora`/`sunset`. До решения DESIGN.md §1.4 не менялся.

**Решение архитектора (2026-11-15):** вариант (а), но слой лежит над обоями целиком, а не под каждой клавишей: `rgba(0,0,0,0.35)` (DESIGN.md §1.5, «Затемнение»). Подпись только белая, цвет клавиши затемняется до 4.5 (DESIGN.md §1.6). Тень подписи не усиливаем, обои не перекрашиваем. Реализация — T13.12.

Решено: T13.12

## D-2. Glass: ~37 к/с на A57 и в режиме `reduced` (T13.13, 2026-11-15)

**Замер** (`dumpsys gfxinfo app.decka.client`, 4 круга свайпов между страницами, обои `album`, раскладка владельца, APK T13.13): `Reduce effects` выключен — p50 кадра 27 мс (~37 к/с), 663 кадра, 0.30% janky; **включён — p50 26 мс (~38 к/с)**, 661 кадр, 0.30% janky. Остальные темы на этом же телефоне: AMOLED/Studio 10 мс (~100 к/с), Material 11 мс.

**Вывод:** `reduced` (без SVG-преломления, `blur(10px)`) почти не меняет частоту кадров, значит узкое место не слой 2. Подозрение — слой обоев (у `album` это `blur(48px)` на изображении ×1.3, которое ещё и дрейфует), общий для обоих режимов; не проверено на `aurora` в приложении. Спецификация не менялась (по T13.13 п. 3). Для решения: проверить обои без `filter` на A57 и при необходимости перенести размытие `album` на сторону агента или клиент в момент смены обложки.

**Решение архитектора (2026-11-15):** три изменения (DESIGN.md §1.4–§1.5): 1) обложка `album` размывается один раз на трек через `canvas` 24×24 вместо `blur(48px)`; 2) смещение обоев — ступенями раз в 30 с вместо непрерывного (непрерывное движение под `backdrop-filter` заставляло перерисовывать размытие под каждой клавишей каждый кадр); 3) `reduced` — без `backdrop-filter` совсем и без смещения. Цель на A57: обычный режим ≥ 45 к/с, `reduced` ≥ 55. Реализация — T14.0.

**Закрыто (2026-11-15, после T14.0):** `reduced` — ~83 к/с, обычный — ~38. Принимается как есть: автоматический замер при старте темы (DESIGN.md §1.4) сам включает `reduced` на A57 и подобных телефонах, полный Glass остаётся для более мощных. Дальнейшая оптимизация обычного режима не планируется.
**Результат T14.0 (A57, приложение, раскладка владельца, `gfxinfo`, 4 круга свайпов):**

| Режим | p50 кадра | к/с | janky | цель |
|---|---|---|---|---|
| Glass обычный, обои `album` (с обложкой) | 26 мс | ~38 | 7.5–9.5% | ≥ 45 — **не достигнута** |
| Glass `reduced`, обои `album` | 12 мс | ~83 | 3.9% | ≥ 55 — достигнута |
| (для сравнения до T14.0: обычный 27 мс, `reduced` 26 мс) | | ~37 / ~38 | | |

Обои `aurora` в приложении на раскладке владельца не мерились (правило T13.13 п. 4: конфигурацию владельца не менять). **Обычный режим** (`backdrop-filter` на 15+ клавишах и панелях, включая `url(#decka-glass)`) остаётся ниже 45 к/с; спецификацию не менял. Клиент сам переходит в `reduced` после 3 с замера (< 45), так что на A57 пользователь получает ~83 к/с, но без размытия фона под клавишами.

## T14.9 — `rotateLayout` and the `apps` of a new page (2026-11-16)

DATA_MODEL.md 3.7 rule 4 says the page `<name> 2` that receives the items that did not fit keeps the page's `apps`. Rule 9 of 3.2 lets an app belong to one page only, so a copy with the same `apps` makes the result invalid (`/pages/N/apps/0`), while 3.7 also demands a valid result. Not decided by the executor beyond this: `rotateLayout` gives the new page **no** `apps` (it is a normal page one swipe away); the original page keeps them. Owner: say if the other reading (move `apps` to the new page, or allow the duplicate) is wanted.

**Resolved (architect, 2026-11-16):** the executor's reading is correct. DATA_MODEL.md §3.7 rule 4 now says the new page has no `apps`. No code change.
## T14.10 — no Galaxy S9 for acceptance (2026-11-16)

`adb devices` lists no device. Steps 1–3 (T14.5a, remaining T14.9 checks, 0.2.0 version check on the phone) and therefore the release cannot proceed: v0.2.0 is not published without S9 acceptance. Owner: connect the S9 (USB debugging on, authorize the PC), then restart the task from step 1. Step 0 (architect docs commit 9fc77c4) is done.

**Update (2026-11-16):** S9 (`232512e0080c7ece`) is now visible to `adb`, but it is locked with a PIN (`mCurrentFocus=Bouncer`, `mDreamingLockscreen=true`; `KEYCODE_WAKEUP` and `wm dismiss-keyguard` do not help). `release:agent` and `release:apk` were built (0.1.0), the APK is installed on the S9, a test agent on `.tmp\cfg-s9` (127.0.0.1:9247, `adb reverse`) was started and stopped again; nothing could be run on the phone. Owner: unlock the S9, set Developer options → Stay awake (or remove the lock) for the time of the check, then restart the task from step 3.

## T14.10 — owner: keys "move chaotically" when the orientation is changed (2026-11-16)

Owner report after v0.2.0: switching the orientation rearranges the keys chaotically. `rotateLayout` does exactly what DATA_MODEL.md 3.7 says (items 1-5: transposed grid, `x' = y`, `y' = x`, widgets keep their size, items that do not fit go to a new page `<name> 2`), so this is not a code bug. For a 7x4 layout with a media widget and several widgets the transposition mixes reading order (rows become columns) and widgets collide, which looks chaotic. Not changed by the executor. Architect: decide another rule (for example keep reading order and re-flow row by row into the new grid, keep the widgets on top, or turn only the keys), and say if the preview in `Look → Orientation` should show it. Until then the owner can undo with `Ctrl+Z` right after `Rotate`.

**Решение архитектора (2026-11-16):** транспонирование отменено; поворот сохраняет порядок чтения (виджеты, затем кнопки, в первые свободные места), DATA_MODEL.md §3.7 п. 2–3. Реализация — T15.0. Решено: T15.0.

## Owner request: weather widget and streamer widgets (2026-11-16)

Owner wants a weather widget and widgets for streamers, taking the best from Stream Deck and similar tools. New widget kinds need decisions that are not in /docs, so the executor did not start: (1) weather: data source (a free keyless API such as Open-Meteo is the obvious candidate), who fetches it (the agent, not the phone; the agent already may call GitHub, see the update check), location (manual city or coordinates in settings; no automatic geolocation), refresh interval, units; (2) streamer widgets: which services (OBS over obs-websocket 5.x on localhost: recording/streaming state and timer, scene switch, mic mute; Twitch: viewers, followers, chat; YouTube), which of them need an account login/token and where the token is stored (the architect must settle secrets handling, ARCHITECTURE security section), and which are keys (actionIds like `obs-scene`, `obs-mute`) versus read-only widgets. Needed from the architect: DATA_MODEL widget schemas, PROTOCOL messages, collector design, a task list in TASKS.md. Competitor research (Stream Deck plugins, Touch Portal, Streamer.bot) is part of that work.

**Решение архитектора (2026-11-16):** погода — Open-Meteo через агент (DATA_MODEL.md §6.3); стримерам — OBS Studio локально через obs-websocket 5, пароль под DPAPI (§6.4, S17); Twitch/YouTube/Discord — отложены, вопрос владельцу Q-9 (D-47). Плюс каталог готовых кнопок и наборы (§9, D-46). Задачи — M15 в TASKS.md.


## T15.4 — shapes of the sets vs "fits 4x2" (2026-11-16)

DATA_MODEL.md 9 draws `media-remote` as 5 keys in a row and `sound` with a 1x2 widget on the right (5 wide), and TASKS T15.4 asks for a test that every shape fits 4x2 and the portrait 4x7. Both cannot hold: `stream` needs 3 rows, the two sets above need 5 columns. Executor's choice (not an architect decision, please confirm or change): the shapes are 4 columns wide (media-remote: 4 keys, then Repeat and the 3x1 Now playing; sound: Mic, Mute, the 1x2 slider over Vol-/Vol+), and the test checks "at most 4 wide and 7 high, whole onto an empty 7x4 and 4x7 page". Also added to `CatalogItem`: `confirm`, `popup`, and `ask: "sequence"`; colour `$accent` and bind key `$scene` are resolved by `insertSet` (DATA_MODEL.md 9 has no field for the confirm hold, the popup group, the sequence editor or the accent).

**Решение архитектора (2026-11-16):** оба выбора исполнителя приняты. Формы наборов — не шире 4 колонок и не выше 7 рядов (влезают и в 7×4, и в 4×7); поля `confirm`, `popup`, `ask: "sequence"`, цвет `$accent` и ключ `$scene` внесены в DATA_MODEL.md §9. Решено.

## Twitch сделан по прямому указанию владельца, документов под него нет (2026-11-17)

Q-9 (D-47) был открыт: Twitch/YouTube/Discord отложены до решения архитектора. Владелец 2026-11-17 прямо велел делать Twitch первым, поэтому он реализован, но **без записи в /docs** (исполнитель их не правит). Что сделано, чтобы архитектор внёс это в документы и подтвердил или изменил:
- **Вход:** device code grant публичного клиента Twitch (секрета нет). Владелец сам регистрирует приложение на dev.twitch.tv и вводит Client ID в окне; агент показывает код для twitch.tv/activate. Права: `moderator:read:followers`, `clips:edit`, `moderator:manage:announcements`, и только они.
- **Токены:** access и refresh вместе одним блобом DPAPI в `config.json` (`twitch.tokens`, `dpapi:…`), как пароль OBS (S17); `get_config` отдаёт `tokensSet`, `save_config` их сохраняет, смена Client ID сбрасывает вход. В протокол и в логи токены не попадают.
- **Данные:** раз в 30 с агент спрашивает Helix (`/users` один раз при входе, `/streams`, `/channels/followers`); секция `twitch` (`TwitchInfo`: connected, login, live, viewers, followers, title?, game?, startedAt?), переменные `twitch.live`, `twitch.viewers`, `twitch.followers` (префикс `twitch.` зарезервирован). Хосты зашиты в код.
- **Действия:** `type: "twitch"`, `op: clip | announce`, `message` только у `announce` (1..500). Телефон шлёт только `actionId`, текст объявления лежит в `actions.json`.
- **Виджет:** `twitch`, размеры 2×1..4×2; каталог: `twitch.clip`, `twitch.announce` (новый вид вопроса `ask: "text"`), `widget.twitch`, набор `twitch`; интеграция `requires: "twitch"`.
- **IPC:** `twitch_status`, `twitch_login_start`, `twitch_logout`.
- YouTube сделан так же по указанию владельца (2026-11-17): device flow Google для «TVs and Limited Input devices» (Client ID владельца и client secret, который Google для этого входа требует; секрет и токены — блобы DPAPI в `config.json`, `set_youtube_secret`), раз в минуту `channels`, `liveBroadcasts` (active), `videos` (зрители); раздел `youtube` (`YoutubeInfo`), переменные `youtube.live|viewers|subscribers` (префикс зарезервирован), действие `type: "youtube"`, `op: "chat"`, `message` 1..200, виджет `youtube` 2×1..4×2, каталог `youtube.chat`, `widget.youtube`, набор `youtube`; IPC `youtube_status|login_start|logout`, `set_youtube_secret`. Квота: ~4 300 единиц в сутки при непрерывном эфире, сообщение в чат — 50.
- Не сделано: события (новые фолловеры, рейды) через EventSub, чат Twitch, Discord (RPC Discord закрыт для обычных приложений), кнопки начала и конца трансляции YouTube.
Нужно от архитектора: D-49, DATA_MODEL (§2.3 действие, §3 виджет, §6 раздел), PROTOCOL (`TwitchInfo`, IPC), ответ на Q-9 по остальным сервисам.
