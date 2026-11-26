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

## Сделано по указанию владельца без записи в /docs: подсказки, диагностика, «Всегда разрешать», кнопки программы (2026-11-18)

Исполнитель по прямому указанию владельца сделал следующее; в /docs не вносилось, нужны архитекторские документы:
- **Виджет `context` («Кнопки программы»):** кнопки страницы, у которой `apps` содержит программу в фокусе (раскладываются по порядку чтения в клетки виджета, переходные кнопки пропускаются); размеры 2×1..вся сетка; без страницы под программу — подсказка. Новый вид элемента макета, схема `ContextItem`, запись каталога `widget.context`. Новые шаблоны страниц: Outlook, Word, Excel, PowerPoint, Canva, Telegram, WhatsApp, Discord (комбинации клавиш записаны по памяти, на живых программах не проверены).
- **Лимиты ИИ отдельно:** каталог `widget.ai.claude` и `widget.ai.codex` (2×1, `show`); у виджета одного провайдера в 2×1 видны оба окна лимита.
- **Диагностика подключения** (IPC `diagnose_connection`, `fix_firewall`; `agent/core/src/diag.rs`): сервер, адрес привязки, адреса ПК, проверка порта по каждому адресу, правила брандмауэра для этой программы (Allow/Block, профили) с учётом сети, в которой сейчас ПК; кнопка исправления правила (UAC).
- **История разрешений:** `AiInfo.history` (до 30, только в памяти агента, на диск не пишется: там команды и пути), виджет `ai` по нажатию открывает список на телефоне.
- **«Всегда разрешать»:** `PermAnswer.decision = "allow_always"`, `PermRequest.always`, `AiConfig.rules` (`AiRule`), IPC `ai_remove_rule`. Правило запоминается только для узких видов: простая команда оболочки из 1–3 слов (без `; & | > < $ ( ) { }`, без опасных программ вроде rm, curl, powershell, npx), правка/чтение внутри одной папки проекта (без `..` и `~`), один сайт для WebFetch, один MCP-инструмент. Правила видны и удаляются в окне (More → Integrations → AI coding) и на телефоне не расширяются. **Это решение о безопасности: архитектору подтвердить или ужесточить.**
- **Twitch:** уведомление о новом подписчике (опрос `/channels/followers` раз в 30 с, не EventSub: у `tokio-tungstenite` нет TLS-функции для `wss://`, а добавлять её без решения архитектора исполнитель не стал). Рейды и подписки через EventSub не сделаны.
- **YouTube:** действия `start` (запланированная трансляция → `live`) и `end` (идущая → `complete`) через `liveBroadcasts.transition` (по 50 единиц квоты); `message` только у `chat`.
- **Не сделано (нужны решения):** автообновление агента — в ARCHITECTURE.md 8a и Q-8 агент только сообщает о новой версии и ничего не скачивает; без решения о ключах подписи и о скачивании исполнитель это не меняет. Шифрование Wi-Fi-трафика (TLS) меняет протокол (PROTOCOL.md) и требует, чтобы Android-клиент доверял сертификату агента (закрепление отпечатка из QR); без решения архитектора не начато. Подпись установщика требует платного сертификата.
## Контекст по заголовку окна не сделан: запрещён ARCHITECTURE.md 6.4 / S16 (2026-11-18)

**Решение архитектора 2026-11-21: пока запрет остаётся.** Сначала идут M19 (языки) и ручные проверки. Если владелец попросит снова, возможный вариант: заголовок читается только на ПК, сопоставляется с правилом страницы «подстрока заголовка», правило включается вручную для каждой страницы, на телефон уходит только id страницы, в логи и протокол заголовок не попадает. Для этого нужна отдельная задача с пересмотром S16.

Владелец попросил разные кнопки для вкладок браузера (YouTube, Gmail, Figma). Для этого агенту пришлось бы читать заголовок окна, а ARCHITECTURE.md 6.4 (S16) и заголовок `collectors/focus.rs` прямо запрещают: заголовки содержат темы писем, имена файлов и чатов. Исполнитель это правило не менял. Нужно решение архитектора: отменить S16 для сопоставления «подстрока заголовка → страница» (заголовок остаётся на ПК, на телефон уходит только id страницы) или оставить запрет.

Сделано рядом, без чтения заголовков: страницы по времени суток (`Page.time`, часы телефона), подсказка в окне «для программы в фокусе есть шаблон — добавить», готовые ответы «Отклонить и сказать» (`AiConfig.replies`, `PermAnswer.reply`: телефон шлёт номер готового текста, сам текст хранится на ПК), шаблон Teams и расширенный шаблон браузера. Быстрые ответы в окно терминала Claude Code/Codex (ввод текста нажатиями клавиш) не делались: это эмуляция ввода в чужое окно, ненадёжная и небезопасная.
## M18 — решения исполнителя в местах, где спецификация допускала два прочтения (2026-11-20)

**Решение архитектора 2026-11-21: все пункты ниже приняты.**
- `apiToken` и `hookToken` остаются открытыми: `hookToken` и так лежит открытым в `~/.claude/settings.json` (его читает Claude Code), а `apiToken` — ключ для скриптов пользователя. Оба дают доступ только к `/api`: в общественной сети из LAN `/api` закрыт, `/api/cc/*` работает только с loopback. DATA_MODEL.md §11.6 уточнён.
- Декодированные байты `cn`/`sn`, `K_dev` под DPAPI в hex, формат `ws-smoke`, синхронный PBKDF2 и кнопка `Pair again` — приняты как есть.

Ничего не остановлено; каждое решение ниже можно поменять отдельной правкой. Архитектору: подтвердить или поправить.

- **`apiToken` и `ai.hookToken` остались открытыми в `config.json`** (DATA_MODEL.md 11.6 говорит «все токены и пароли»). Причина: макрос `H-API` в TASKS.md и хуки Claude Code читают их из файла; пользователи берут `apiToken` из окна и подставляют в скрипты. Под DPAPI лежат пароль OBS, пароли почты, секрет и токены YouTube, токены Twitch и `K_dev` (то, что агент использует сам). Открытое значение этих полей, вписанное вручную, шифруется при загрузке (до проверки схемы) и файл переписывается. Если нужно шифровать и `apiToken`/`hookToken` — нужно решение, как тогда живут `H-API` и хуки.
- **Байты в рукопожатии (PROTOCOL.md 10).** В `HKDF`-соли, соли `PBKDF2` и входе MAC используются **декодированные** 16 байт `cn` и `sn` (не их base64url-текст). `K_dev` = SHA-256 текста токена (43 символа), как и был хеш. Это зафиксировано векторами `shared/fixtures/crypto-v2.json`, которые создал независимый `node:crypto` и проверяют Rust (`ring`) и TS (`@noble`).
- **`K_dev` на диске** = `dpapi:<base64>` от hex-строки из 64 символов; старое поле `tokenHash` (тот же хеш в hex) переносится в `keyDpapi` при первом старте и удаляется из файла.
- **`tools/ws-smoke.mjs`:** `pair` печатает `{deviceId, token}`, где `token` = `<deviceId>.<token>`: вход по v2 требует `deviceId`, а макросы H-* передают одно значение. `(… pair …).token` работает как раньше.
- **`PBKDF2` на телефоне** считается синхронно в потоке страницы (≈0,3 с на ПК, на слабом телефоне заметно дольше), только в режиме `pair_code`. Таймер «кандидат ждёт 1,5 с» на это время снимается (после `challenge`).
- **Веб-клиент и приложение** при `SERVER_MISMATCH` показывают экран «This is not your PC» с кнопками `Retry` и `Pair again` (вторая добавлена исполнителем: иначе после переустановки ПК телефон не выйти без очистки данных).


## M21-1. `go_page` без поля протокола (C3, 2026-11-22) — подтверждено архитектором 2026-11-22 (переменная `voice.goto` — окончательное решение, отдельное сообщение протокола не нужно)

**Противоречие.** DATA_MODEL §12.5: «`go_page` — телефоны получают `navigate`», но в PROTOCOL.md нет сообщения «перейти на страницу» (ни в `voice`, ни отдельного).
**Что сделано (без изменения протокола).** Агент публикует переменную `voice.goto` = `<pageId>#<счётчик>` (`Agent` → `vars`); телефон (`client/src/voice/goto.ts`) следит за ней и вызывает `navigateManually`: первое значение после подключения только запоминается, дальше любое изменение — переход. Телефон ничего не присылает. Префикс `voice.` добавлен к зарезервированным именам (`validate::is_reserved_var`), чтобы `PUT /api/vars/voice.goto` не мог двигать телефоны.
**Как поменять.** Если нужен отдельный тип сообщения — заменить запись в `host.rs` (`ToolCall::GoPage`) и `goto.ts`; остальное не затронуто.

## M21-2. Совпадение шаблона таблицы, которое отвергают инструменты, теперь идёт к ИИ (C4) — подтверждено архитектором 2026-11-22

Шаблон `open {app}` забирал «open main page», «open github.com», «start dictation» (программы нет → `Can't do that`). Правило конвейера: попадание таблицы, которое `validate` отклонил, отправляется к модели; без ответа модели пользователь видит прежнее `Can't do that: …`. Исполнение по-прежнему только после `validate`.

## M21-3. Загрузка с Hugging Face упала на редиректе (T21.6, 2026-11-22) — исправлено

Hugging Face перенаправляет на `us.aws.cdn.hf.co`, которого не было в `ALLOWED_HOSTS` (`error following redirect`). Добавлен шаблон `*.hf.co` (поддомены, не сам `hf.co`); целостность по-прежнему решает SHA-256 манифеста. Тест — в `ai/download.rs`.

## M21-4. Распознавание речи на этом ПК дольше лимита (T21.11, 2026-11-22) — решение архитектора

`whisper-server` (large-v3-turbo q5_0, CPU-сборка) на i3-12100 тратит 16–20 с на одну фразу (замер curl: 18–22 с на 2 с звука); лимит `TRANSCRIBE_TIMEOUT` — 15 с, поэтому на этом ПК без `DECKA_VOICE_STT_TIMEOUT_S=60` распознавание обрывается. Видеокарта RTX 2060 SUPER есть, а whisper-рантайм без GPU. Нужно решение: модель полегче (small/base) или GPU-рантайм whisper (Vulkan, как у llama), или другой лимит. Код не менялся, кроме переменной окружения для проверки.

**Решение архитектора (2026-11-22), задача T21.12, входит в 0.20.3:**
1. **GPU-рантайм whisper.** Если видеокарта подходит под llm-standard (VRAM ≥ 6 ГБ, §12.2) или вообще есть Vulkan-устройство с ≥ 2 ГБ — ставить GPU-сборку whisper-server той же версии whisper.cpp: Vulkan, если в официальном релизе есть готовая Windows-сборка; иначе для NVIDIA — cuBLAS-сборка (с её cudart-файлами), для прочих — CPU. Выбор и SHA-256 — в `shared/ai/manifest.json`, загрузка как у llama. Цель на этом ПК: ≤ 2 с на фразу.
2. **Быстрые параметры для коротких фраз (и на CPU):** жадное декодирование (beam 1, best_of 1), без таймстемпов, язык из настроек вместо автоопределения, `audio_ctx` по длине записи (не всё 30-секундное окно), потоки = физические ядра. Замер до/после в отчёт.
3. **Лимит распознавания — от железа:** GPU — 15 с; CPU — 10 с + 3 × длина записи, но не больше 60 с. Плашка во время распознавания показывает `Recognizing…`, чтобы долгое ожидание не выглядело зависанием.
4. Без видеокарты модель не менять (large-v3-turbo нужна для kk); если после п. 2 CPU всё ещё > 8 с на фразу — записать замер, решение позже.


**Закрыто (T21.12, 2026-11-22):** GPU-сборка whisper-server — cuBLAS из того же релиза b5907 (`whisper-bin-win-cuda-12.4.0-x64.zip`, 735 МБ, SHA-256 в manifest; готовой Vulkan-сборки у whisper.cpp нет ни в одном релизе). Замер на этом ПК (i3-12100 / RTX 2060 SUPER, фраза 2,3 с): CPU 12,4–14,3 с → 4,5 с с быстрыми параметрами; GPU 0,21–0,24 с → 0,08 с. Лимит: GPU 15 с, CPU 10 с + 3 × длина (≤ 60 с); плашка «Recognizing…».

## 2026-11-25 · T22.13: данные и виджеты телефона для напоминаний и списков — нужно решение архитектора
Сделано (core, с тестами): eminders.json / lists.json, лимиты, разбор времени (emind{text, inMinutes | day+time | at} — дату считает агент, а не ИИ), пропущенные при старте, уведомления, инструменты emind, eminders_show, list_add, list_show, list_remove (без item — удаление списка с подтверждением), сообщение list_toggle (proto, ws, shared/protocol.ts, схема; чужие id игнорируются).
Не сделано, потому что в /docs не описано (архитектурное решение, п. 4 правил):
1. **Как данные попадают на телефон.** Секции состояния фиксированы (SECTIONS, PROTOCOL §3); для eminders и lists секции нет. Нужен выбор: новые секции (меняют FullState и фикстуры) или переменные ars с зарезервированным префиксом. Пока list_toggle принимается, но телефон не видит списков.
2. **Виджеты eminders и list** — типы элементов раскладки (DATA_MODEL §3), схемы, редактор, вид на телефоне — нет в DATA_MODEL/DESIGN.
3. **Окно ПК More → Reminders** — нет макета в DESIGN.
4. «Плашка» при срабатывании напоминания — отдельного вида плашки нет; срабатывание идёт через обычное уведомление (push_notification: лента телефона и уведомление Windows).
5. «Удаление/очистка списка» (§13.4): сделано как «удалить весь список»; очистка с сохранением списка — отдельной фразы нет.
## 2026-11-25 · T22.14: лист сводки на телефоне и настройка «сводка утром» — нет макета
Сделано (core, с тестами): сбор фактов из погоды, почты, напоминаний на сегодня, списков и лимитов Claude/Codex (только непустое), запрос к Qwen с JSON фактов, проверка ответа (числа и имена ⊂ данных; иначе показывается обычный текст из фактов), инструмент day_brief, ответ в плашке и на телефоне как обычный результат (≤ 300 символов — предел eply в PROTOCOL).
Не сделано (нет в DESIGN/PROTOCOL): лист с карточками по разделам на телефоне (нужны структурированные данные — у oice их нет); настройка Morning brief on the phone at the first unlock of the day (нужен признак первой разблокировки телефона — сообщения для этого в PROTOCOL нет).
## 2026-11-25 · T22.15: список «Память» в окне ПК — нет макета
Сделано (core, с тестами): memory.json (≤ 200 фактов × 200 символов), только явное добавление (memory_add разрешён модели лишь после фразы «запомни/remember…» на 11 языках — проверка в pipeline), удаление memory_remove с подтверждением, подбор до 20 фактов по словам фразы (с учётом падежных окончаний) в запрос к Qwen как данные.
Не сделано (нет в DESIGN): More → Voice → Memory / Память — просмотр, правка и удаление фактов в окне ПК; IPC-команды для него. Memory::all и emove_id готовы.

## 2026-11-25 · Окно настроек для T22.10 (диктовка)
Сделано (core, с тестами): dictation.json — триггер (doubleCtrl по умолчанию / аккорд / выкл), режим clean/aw, личный словарь; двойное нажатие Ctrl (WH_KEYBOARD_LL), clean через Qwen с защитой, подсказка словаря для whisper, вставка > 300 символов через буфер. Host умеет set_dictation_settings.
Не сделано: элементы More → Voice (переключатель триггера, ChordField, режим, словарь) и IPC-команды oice_dictation_get/set в shared/src/ipc.ts + commands.rs; надпись плашки Dictating… / Диктую… (отдельного вида плашки для диктовки в DESIGN нет); значок камеры Looking at the screen для T22.12. Пока настройки читаются из dictation.json в каталоге конфигурации.
**Решение архитектора 2026-11-25 (блокеры T22.13–T22.15 и диктовка):** DATA_MODEL §13.7, PROTOCOL §3 «M22», задачи T22.16–T22.17. Лист сводки и «сводка утром» отложены.

## 2026-11-26 - T24.S: ARCHITECTURE S18 (outbound hosts) does not list lrclib.net
Not a blocker, a docs gap for the architect. T24.2 asks lrclib.net (HTTPS, 5 s, no redirects, no cookies) only while config.media.lyrics.enabled is true; the request carries artist, title and duration only (DATA_MODEL 14). S18 lists api.github.com and open-meteo hosts only - please add lrclib.net. Also new in this pass: list_toggle, notif_open, notif_dismiss, perm_answer and lyrics_offset now share the 20 per second limit of a socket (RATE_LIMITED, not fatal) and ignore ids that do not match the issued formats; S9 could mention it.

**Решение архитектора 2026-11-25:** lrclib.net добавлен в S18. Общий лимит 20/с и проверка форматов id для data-сообщений принимаются как есть.

## 2026-11-25 - T25.3: the PC does not know which page the phone shows
Not a stop, a gap for the architect. DESIGN 4.17 (`Auto`) hides the lyrics window while the phone shows a `media` widget of size `L` "on the current page". No message carries the current page of a phone (PROTOCOL has none), so the PC cannot tell. Implemented the safe approximation in `lyrics_overlay::phone_shows_big_media`: a phone is connected and the layout has a `media` widget with `w >= 4 && h >= 2` on any normal (non-popup) page. Effect: with such a widget anywhere in the layout, `Auto` stays hidden while a phone is connected, even when the phone is on another page; `Always` is not affected. An exact rule needs a small data message `page_view { pageId }` from the phone (data only, no command) - architect to decide; the function is the only place to change.

**Решение архитектора 2026-11-25:** условие про телефон из `Auto` убрано (DESIGN §4.17): `Auto` = играет + синхронный текст, скрыто при паузе 3 с и полноэкранной программе. `page_view` не вводим; `phone_shows_big_media` удалить.

**Решение архитектора 2026-11-25 (T25.3):** `Auto` не зависит от телефона: показ, пока играет песня с синхронным текстом; скрытие через 3 с после паузы и при полноэкранной программе. `phone_shows_big_media` и его тесты удалены, сообщения `page_view` не будет. `Always` — пока песня играет, в том числе поверх полного экрана.

## 2026-11-25 - T26.1: two gaps in `status` (not stops)
1. `phone_battery`: the `battery` section holds only PC and Bluetooth devices (`kind: pc|bt`); the phone's own battery is not sent to the PC (PROTOCOL has no such data message). Implemented: the answer lists `bt` devices; empty -> `voice.status.none.phone_battery`. The phone's own charge needs a small data message (`battery_report`) - architect to decide.
2. The fast path takes `what is the weather in paris today` (up to 4 extra words) as `status weather` and answers with the configured place. A place named in the phrase is not read. Option: patterns with a `{place}` slot that leaves such phrases to the model, or a `status` argument `place` that is refused.

**Решение архитектора 2026-11-25 (вопросы T26.R):** `phone_battery` → `battery` по `BatteryInfo` (T26.F.5); погода с чужим городом — мимо `status` (T26.F.6); `voice.context.enabled` в `config.json` — верно; `always` поверх полноэкранной программы — верно (решение T25.3), `auto` скрывается — верно. Акрил — попытка в T26.F.3.

## M27 (2026-11-26, night run)

1. **Vision fallback not built.** `find_on_screen` with no match and a guide on a window with an empty UI Automation tree (some Chrome/Electron pages) should fall back to the screen help model returning a `bbox` (TASKS T27.3/T27.5). Not built: the guide answers "Cannot see any buttons in this window" and `find_on_screen` "Did not find it on the screen". Needs the `bbox` schema and the screenshot path of `screen_ask` shared with the guide.
2. **Waiting for the person is polling, not UIA events.** The guide looks at the window every 0.4 s (changed names of elements, a click into the highlight, another window) instead of subscribing to UIA events (Invoke, focus, structure changed). Same effect for the steps; subscribing would save the polling.
3. **Notices are shown as an island answer for 6 s, not as a `compact` state.** `Host::plate` is reused (silent). The sources: memory, CPU, disk, AI limit and any battery in the `battery` section; "phone battery" has no data (see 26.R item 1) and "end of the focus timer" has no source yet (`Sample.focus_ended` is always false).
4. **Work mode `focusMinutes` is stored but not started** by `mode_run` (the focus timer is an action of the deck, not a call of the host).
5. **T27.0 target (90 %) not reached in Russian.** End to end (table, then model) on Qwen3.5 4B: ru 70 % -> 86 %, en 74 % -> 92 %; model only: ru 63.5 % -> 66.7 %, en 64.9 % -> 72.6 % (sets grew from 74 to 84 phrases with the M27 and place phrases). What is left in ru: `list_add`/`list_remove` with a different word for the list, `day_brief` phrased as a question, `memory_remove` "сотри запись", the live-data questions the set wants as `ask` (news, exchange rate: the model rightly searches the web), `remind` with a series.
6. **Phrase tables of the screen help: "стоп" collides with the general "stop"** only while no guide runs (no collision then); while a guide runs the guide controls win (tested).

## 2026-11-26 · T28.4 «кнопка голосом»: функции нет ни в коде, ни в спецификации — нужно решение архитектора

**Что не так.** T28.4 проверяет «обычная команда → черновик действия» и «запись в `actions.json` — только после „да“ и проверки схемы». Голосового создания кнопки нет: в белом списке инструментов (DATA_MODEL §12.5) нет инструмента, который создаёт или меняет действие, фраз для этого в `shared/voice/*.json` нет, ни один путь голоса не пишет `actions.json` и `layout.json`. Правило «писать только после „да“» выполняется потому, что голос не пишет вовсе.

**Что сделано.** Только тест того, что есть (`tools.rs`, `a_button_by_voice_asks_confirms_and_never_writes_actions`): неоднозначное имя → вопрос «какое?»; нажатие на «удалить / отправить / оплатить» → подтверждение; вызовы вида `create_button` отклоняются. Новый инструмент не заводился: это новая власть голоса (запуск программ, клавиши), это архитектурное решение.

**Что решить, если функция нужна:** имя инструмента и его аргументы; какие типы действий голос вправе создавать (`open_url`? `launch` только из списка программ? `hotkey`?); как показать черновик на острове и подтвердить «да»; куда ставить кнопку в раскладке; ключи i18n. Схема проверки уже есть: `Agent::save_actions` отклоняет неверный файл целиком и возвращает причины.
