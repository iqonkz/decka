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

## 2026-11-27 · T29.A: что требует решения (аудит M29)

Найдено при аудите; исправления без новых решений сделаны коммитами `T29.A: …`. Ниже — то, где нужно решение (цена: S — часы, M — день-два, L — больше).

1. **Страница телефона неизвестна ПК** (нет такого сообщения в PROTOCOL.md) → «добавь кнопку» кладёт на страницу, куда голос переходил последним, иначе на первую. Решение: номер страницы в начале голосового запроса телефона. S.
2. **Телефон не видит код ответа вебхука** (в протоколе нет поля) → показывается только «не сработало». S.
3. **Секреты в заголовках вебхука лежат открытым текстом** в `actions.json`. Решение: DPAPI, как пароли почты (нужна схема и перенос). M.
4. **Нет одной иконки — отклоняется вся раскладка** (`save_layout`) → голосовое «добавь кнопку» отвечает «icon file not found». Решение: заменять отсутствующую иконку значком по умолчанию. S.
5. **Проверка `Origin` в WebSocket пропускает `Origin` == `http://{Host}`** → при DNS rebinding проверка источника проходит (без токена или кода всё равно не войти). Решение: принимать только IP, `localhost` и локальные имена; сломает доступ по доменному имени (например, Tailscale MagicDNS). S.
6. **Буфер обмена восстанавливается только текстом**: картинка или файлы, лежавшие в буфере, пропадают после «переведи выделенное». M.
7. **Временный текст попадает в историю буфера Windows** (Win+V, облачный буфер). Решение: помечать свою временную запись форматом, исключающим её из истории. S.
8. **Окна острова и подсветки после первого показа остаются в памяти**: до первой команды или музыки агент занимает около 70 МБ, после — около 260 МБ (WebView2), после первого «где кнопка» ещё около 40 МБ. Решение: низкий уровень памяти WebView2, пока окно скрыто, или закрытие окна после долгого простоя (следующее появление станет медленнее). S/M.
9. **`whisper-server` занимает около 1,7 ГБ** выделенной памяти при модели large-v3-turbo. Решение: меньшая модель распознавания как вариант для слабых ПК (хуже качество). M.
10. **`cargo audit`: предупреждения без исправлений** (неподдерживаемые крейты из дерева Tauri и `chrono`; `glib` — только Linux). Ждать обновлений Tauri; `chrono` — решить о замене. M.
11. **Поиск на экране ограничен 300 мс** (T27.1): пока грузится модель, снимок окна может обрезаться и «где кнопка …» отвечает «Не нашёл» (один раз воспроизведено). Решение: лимит 1 с или повторный снимок, если первый обрезан. S.

## 2026-11-27 · T29.P (0.51.1): что сделано иначе и что осталось

Пункты 1–9 и 11 списка выше закрыты коммитами `T29.P.…`; 10 только отслеживается (те же 6 предупреждений).

- **Документы отстают** (DATA_MODEL.md не редактируется исполнителем): правило 11 (`icon file not found`) заменено значком `lucide:square`; значения заголовков вебхука в `actions.json` — `dpapi:<base64>` (до 4096 символов, открытые — по-прежнему до 500); новый ключ `config.json` `server.allowedHosts`; `voice.json` `lightStt`; модель `stt-light` (`ggml-small-q5_1.bin`) в манифесте.
- **Win+V:** выделенный фрагмент, который программа сама копирует по `Ctrl+C`, в историю попадает — пометить чужую запись нельзя. Текст, который кладёт Decka, и восстановленное содержимое туда не попадают.
- **Лёгкое распознавание** занимает на процессоре 0,86 ГБ выделенной памяти (0,37 ГБ рабочего набора) против 1,28 (0,78) у большой, а не 0,5 ГБ выделенной. На процессоре этого ПК большая модель распознаёт фразу около 20 с, малая — около 3,5 с: возможно, на ПК без видеокарты лёгкую стоит включать по умолчанию. Решение владельца.
- **Заголовки вебхука** под DPAPI не переносятся на другой ПК или пользователя Windows: там их надо ввести заново.

## 2026-11-27 · M30 (0.60.0): что сделано иначе и что отстало в документах

Тупиков не было. Решения, принятые по ходу (проверить архитектору):

1. **T29.P2.3** — пересобрать черновик 0.51.1 было нельзя: `v0.51.1` уже опубликован. Изменения T29.P2 вошли в 0.60.0.
2. **T29.P2.1** — лёгкое распознавание по умолчанию не навязывается там, где на диске уже лежит только большая модель (само ничего не скачивается и не меняется).
3. **T29.P2.2** — при выключенной «Помощи по экрану» вопрос об экране предлагает включить её (`voice.confirm.screen_on`); «да» на скачивание пакета тоже включает её.
4. **T30.1** — на ПК без видеокарты распознавание идёт после остановки (не во время встречи); во время диктовки (шарик у курсора) от индикатора записи видна только красная точка; добавлен переключатель «Хранить звук» на странице «Заметки».
5. **T30.2** — «закрой <имя>» с вопросом «да?» действует 2 минуты после списка «что ест память» (и по кнопке в острове); вне этого окна `close_app` работает как раньше. Программы называются по описанию файла, затем по заголовку окна.
6. **T30.3** — телефон убирается из секции `battery` через 3 минуты молчания или при отключении.
7. **T30.5** — без событий UIA окно читается раз в 1 с, при событиях — по событию и раз в 5 с. Найдена и исправлена старая ошибка: снимок окна, не уложившийся в 300 мс, получался пустым.
8. **T30.6** — в окне, прочитанном распознаванием, гид не переходит дальше сам (текст в играх меняется каждый кадр): шаг заканчивают щелчок по рамке или «дальше». В `decka-core` добавлены зависимость `windows-core` 0.62.2 (уже в Cargo.lock) и фича `Media_Ocr`.
9. **T30.7** — правила поверх ответа модели (`voice::llm::adjust`); `reply_action`, `guide_control` и `ha` предлагаются модели только когда им есть с чем работать; `MAX_CONTEXT_CHARS` 6000 → 5200 (инструкции выросли, окно 4096 токенов). `voice_eval` повторяет это поведение агента; наборы eval под модель не менялись.
10. **T30.R** — в `ag-m30` фразы «стоп заметка» запускали запись, а «выключи свет» переключал: таблицы 11 языков разделены; в 11 eval-наборах ожидаемый сервис для «включи свет» исправлен с `toggle` на `turn_on`; тест острова ждёт «МБ» в ru.
11. **Тестовая копия агента** (`DECKA_CONFIG_DIR`) могла изменить автозапуск установленной Decka — теперь автозапуск она не трогает.

**Отстают документы** (по разрешению владельца дописаны только `phone_status`, `notes_record`, `ha`): в DATA_MODEL §12.5 нет инструментов `memory_top` и `ha` в общем списке и правил закрытия программ; нет OCR-снимка (§13.3), событий UIA гида, таймера фокуса режима, события `island_state.memory`; в ARCHITECTURE S18 не названы исходящие соединения с Home Assistant.

**Не проверено вживую:** Home Assistant — только на моках (настоящего сервера нет); батарея — без настоящего телефона; OCR — на картинке, не в игре.

## 2026-11-27 · T31.R (0.61.0): отличия от спецификации и отставшие документы

Тупиков не было.

**Найдено и исправлено: почта и Home Assistant по HTTPS не работали в приложении** (0.60.0 и раньше). В сборке всего приложения есть два криптопровайдера rustls (aws-lc-rs от `reqwest`, ring от `tauri-plugin-updater`), и `ClientConfig::with_platform_verifier()` паниковал; задача почты молча умирала. Тесты одного `decka-core` этого не видят — ловит только `cargo test` всего workspace. Провайдер теперь назван явно (`core/src/tls.rs`). Предложение: в T*.R всегда гонять `cargo test` без `-p` (так и сделано).

Проверить архитектору:

1. **Подписи на телефоне** (T31.7b): переводится только подпись, совпадающая с подписью пункта каталога или его состояния, у действия, чей id похож на пункт каталога. Свою подпись, дословно равную каталожной, отличить нельзя — она переведётся. Подписи, которые телефон не может сопоставить (свои действия, часть демо-раскладки), остаются как есть. `layout.json` не менялся.
2. **T31.3b не по значениям** (ветка дизайна): иконка клавиши — 0.46 × меньшей стороны (спецификация: 40 % высоты), подпись `max(11px, 0.14 × side)` весом темы (спецификация: 13/500). S.
3. **Мастер первого запуска** больше не добавляет программы и наборы (T31.4b): `editor/wizardPlan.ts` и его тест не используются. Убрать или вернуть шаг. S.
4. **Отчёт о проблеме** берёт по 500 строк из каждого лога (`decka.log`, `voice.log`); обычные строки серверов моделей выброшены целиком (в них может быть сказанное), остаются строки об ошибках и загрузке.
5. **Восстановление из копии** никогда не берёт из файла: API-токен, порт и адрес, `server.allowedHosts`, политику публичных сетей, правила «Always allow», путь adb, имя ПК, модель и микрофон голоса. Режимы и гиды добавляются или заменяются по имени, чужие не удаляются.
6. **Снимки-эталоны** сняты на этом ПК; задача `visual` в CI необязательная и может расходиться из-за шрифтов раннера.

**Отстают документы** (не редактируются исполнителем): PROTOCOL.md (IPC) — `mail_test`, `mail_status`, `window_first_frame`, `save_problem_report`, `backup_export`, `backup_preview`, `backup_apply`; DATA_MODEL.md — формат `.decka` и состав отчёта; ARCHITECTURE — разрешение окна `dialog:allow-save`; комментарий «The phone never sees it» у каталога устарел (телефон берёт из него подписи).

## 2026-11-27 · T31.5: цель «первый кадр окна настроек ≤ 400 мс» не достигнута

Замер (релизная сборка, тестовая копия с отдельной папкой настроек, медиана 6 открытий после прогрева): было 925 мс, стало 822 мс (ленивые страницы, только используемые значки lucide, отдельный код острова и подсветки). Разбивка из лога и из самой страницы:

- создание окна и WebView2 (`settings window: created in … ms`) — 335–400 мс: оно одно занимает весь бюджет;
- от начала загрузки страницы до первого кадра — около 470 мс, из них скрипты загружены за ~190 мс, долгих задач нет: остальное WebView2 ждёт первый кадр нового прозрачного окна.

Кодом страницы цель не достигается. Решения за архитектором (без них окно оставлено как есть):

1. Держать скрытое окно настроек заранее — открытие за десятки миллисекунд, но +≈100 МБ памяти; противоречит T29.A («окно при закрытии уничтожается»). M.
2. Не уничтожать окно N минут после закрытия — быстрое повторное открытие, память освобождается позже. S.
3. Проверить эффект `transparent` + Mica и `visible: false` на задержку первого кадра (замер на каждый вариант). S.

Если бы словари i18n грузились по языку, из первого кадра ушло бы ~600 КБ скриптов (по замеру — десятки миллисекунд, `t()` стал бы асинхронным при старте). M.

### 2026-11-27 · T31.R: окружение WebView2 заранее — не помогает, откачено

Проверено: `ICoreWebView2Environment` (без окна) создаётся при старте агента и отдаётся окну настроек (`with_environment`, параметры те же, что у `wry` 0.57). Релизная сборка, тестовая копия со своими папками настроек и WebView2, 8 открытий, медиана:

| Состояние | Окно создано | Первый кадр | Память в покое (private / working set) |
|---|---|---|---|
| без окружения, браузер WebView2 холодный | 293 мс | 661 мс | 73 / 47 МБ |
| с окружением, браузер холодный | 290 мс | 655 мс | 74 / 49 МБ |
| без окружения, остров жив (играла музыка) | 80 мс | 450 мс | 224 / 320 МБ |
| с окружением, остров жив | 84 мс | 455 мс | 225 / 322 МБ |

Память не растёт, но и выигрыша нет: окружение само не запускает процесс браузера, а время уходит на него. Код убран (он повторял внутренние параметры `wry`; при расхождении перестали бы создаваться остальные окна).

Что даёт эффект — уже запущенный браузер: ~450 мс вместо ~660 мс ценой ≈150 МБ памяти. Это вариант 1 из списка выше (скрытое окно заранее); решение за архитектором. Цель ≤ 400 мс не достигается и так.

## 2026-11-27 · T32.1: «Декка» написана, но не замерена и ни разу не запускалась с моделью

Классификатор разрешений Claude Code отклонил команду, в которой был запуск скачанного `sherpa-onnx.exe` (причина: «Undermining Oversight»). Запуск скачанной библиотеки другим путём (тест, пример, агент) был бы обходом отказа, поэтому не делался. Скачивание и SHA-256 проверены (совпадают с GitHub и Hugging Face); раскладка структур C API перенесена из `c-api.h` 2.0.3 и проверена тестом на размеры и смещения.

**Не проверено:** загрузка библиотеки и модели, написания слова (`wake::keywords`, подобраны по словарю модели без прогона), пороги чувствительности, ЦП, память, ложные срабатывания, пропуски. Цели T32.1 (ЦП ≤ 3 %, ОЗУ ≤ 80 МБ, ≤ 1 ложного в час, пропуски ≤ 10 %) не подтверждены. По умолчанию выключено; сбой библиотеки завершает только рабочий процесс.

**Чтобы замерить (после разрешения владельца):** включить слово в тестовой копии (файлы скачаются), затем
`powershell -File tools\wake-eval-synth.ps1 -Out <папка>` и
`cargo run --release -p decka-core --example wake_eval -- <dll> <папка модели> medium <папка>\positives <папка>\negatives`.

**Решения за архитектором:** готовые KWS-модели sherpa-onnx не взяты — лицензия на их страницах не указана; взята русская потоковая модель Vosk (Apache-2.0), поэтому английское «Decka» распознаётся как русское «дека».

## 2026-11-27 · T32.R: что проверено вживую в 0.70.0 и что нет

| Проверка | Результат |
|---|---|
| «Декка» 10 раз подряд; ложные срабатывания 30 мин с музыкой | не выполнено (запись T32.1 выше) |
| Мини-панель, релизный exe, тестовая копия (`DECKA_OPEN_TRAY=1`) | открылась над треем 420×525 при 125 %, без активации, поверх окон; плеер, громкость, микрофон, избранное, событие из ICS; закрылась по клику мимо; окно уничтожено не позже 33 с после закрытия |
| Мини-панель: клик по значку, Esc, повторный клик | не проверено (синтетический ввод в сеансе владельца не делался) |
| Память тестовой копии (отдельная папка WebView2), private | без панели 288 МБ, с панелью 365 МБ, через 40 с 315 МБ |
| Календарь по настоящей ICS (публичные праздники Google), `decka-headless` + `ws-smoke` | 3 события; ссылки нет в состоянии, `config.json` и логах |
| «Подключиться», голос про календарь, заметка за 5 мин, несколько ПК на телефоне | только тесты |

**Замечено, не из M32:** пока играет музыка, копия Decka занимает 30–34 % одного ядра (отрисовка острова: `gpu-process` ≈ 16 %, `renderer` ≈ 11 %). То же у установленной 0.51.0. Решение за архитектором.

## 2026-11-28 · T32.F: нагрузка острова снижена в 2–6 раз, цели не достигнуты

Замер: релизная тестовая копия с «тестовой песней» (`DECKA_FAKE_MUSIC=tab|compact|expanded`, только при `DECKA_CONFIG_DIR`), всё дерево процессов, % одного ядра по времени процессора, 20 с. Разброс из-за скорости процессора (73–123 % номинала) и работы владельца за этим ПК.

| Состояние | Цель | До (1 прогон) | После (3–5 прогонов, типично) |
|---|---|---|---|
| полоса | ≤ 5 | 47,9 | 6,6–13,2 (≈ 7) |
| compact | ≤ 5 | 21,7 | 11,8–13,5 (≈ 13) |
| expanded (слушает) | ≤ 10 | 22,1 | 11,1–18,9 (≈ 12) |

**Сделано:** уровень отдельным событием `island_level` 20 раз/с без перерисовки React; состояние 10 раз/с и только при изменении; в полосе только `transform` без перехода; орб по таймеру 15 кадров/с в compact и expanded; строки песни по таймеру следующей строки.

**Почему не ниже (compact):** с выключенным орбом та же копия — ≈ 5,6 %; каждый кадр прозрачного окна стоит GPU-процессу ≈ 3–4 мс, поэтому орб 15 раз/с даёт ещё 6–7 %. Требования «15 кадров» и «≤ 5 %» вместе не выполняются. Проверено и отброшено: программный холст (хуже), собственный слой холста (в пределах шума), без тени панели (без эффекта).

**Решения за архитектором:** (1) орб в compact 5 кадров/с или неподвижный между строками (≈ 6–7 %); (2) орб на CSS вместо холста (оценить отдельно); (3) в полосе уровень на экране 10 раз/с (ожидается ≈ 4–5 %).

## 2026-11-28 · T32.F: «Декка» замерена — пропуски и память не проходят цели

Рантайм и модель скачаны заново, SHA-256 совпали с манифестом. `wake_eval` (тот же порог громкости и та же модель, что у рабочего процесса, звук из файлов, микрофон не использовался). Положительные — голоса Windows (ru Irina, en Zira) разными скоростями и громкостью; отрицательные — 41 мин: 31 мин речи по текстам документов с синтетической музыкой и 10 мин тонов.

| Уровень | «Декка» ru, пропуски из 10 | то же под музыку | «Эй, Декка» / «Хей Декка» | английское «Decka» (из 30) | ложные за 41 мин | ЦП | память |
|---|---|---|---|---|---|---|---|
| низкая | 5 | 8 | 9 / 10 | 30 | 0 | 3,0 % | 107 МБ |
| средняя | 2 (было 5) | 4 | 5 / 9 | 26 | 0 | 3,0 % | 108 МБ |
| высокая | 0 | 2 | 6 / 7 | 24 | 3 (4,3 в час) | 3,0 % | 107 МБ |

Цели: ЦП ≤ 3 % — на границе; память ≤ 80 МБ — **нет** (модель и ONNX Runtime ≈ 100 МБ, настроек памяти в C API нет); ложные ≤ 1 в час — да на низкой и средней; пропуски ≤ 10 % — **только «Декка» на высокой без музыки**.

**Изменено по замеру:** написания и пороги по уровням (`wake::Sensitivity::spellings`), `num_trailing_blanks` 2 → 1, на высокой 8 путей поиска. DATA_MODEL §17 с прежними порогами отстаёт.

**Ограничения замера:** синтетический голос, не живой микрофон. Английское произношение русская модель почти не слышит, а «Эй/Хей, Декка» хуже одного слова.

**Решения за архитектором:** (1) в интерфейсе и справке называть только «Декка»; (2) для «Hey Decka» и английского — отдельная английская модель (нужна с явной лицензией Apache-2.0/MIT) или обучение своей; (3) смириться с ≈ 108 МБ.

## 2026-11-28 · T32.G: подпись «EC» у глобуса не воспроизведена

В коде и шаблонах нет строки «EC»; свою раскладку владельца исполнитель не читал. Вероятное объяснение: пунктирные клетки редактора, сдвинутые на полряда, перекрывали нижнюю часть плиток и оставляли от подписи обрывок (после исправления геометрии подписи видны). Если «EC» осталась в 0.70.1 — нужен снимок этой кнопки и её свойств.

Дописать архитектору: DATA_MODEL / PROTOCOL не обновлялись для 0.70.1 (событие `island_level` { music, voice }, `turnedMisfits`, подпись кнопки каталога без `label`).

## 2026-11-28 · T32.H.3: порция звука телефона не помещалась в кадр (решено на телефоне)

PROTOCOL.md одновременно требует кадр клиента ≤ 4096 байт и разрешает `voice_chunk` до 3200 байт звука; такое сообщение занимает около 4,35 КБ, и ПК рвал соединение на первой порции. Лимиты ПК не менялись: телефон теперь шлёт порции по 2880 байт (11 в секунду). Приложение на телефоне нужно обновить до 0.70.1 — старое по-прежнему теряет связь.

Дописать архитектору: PROTOCOL — `voice_chunk` ≤ 2880 байт с телефона; DATA_MODEL §12 — ответы `voice.reply.no_audio`, `off_on_pc`, `busy`, `pc_refused`, `offline_pc`; словарная подсказка распознаванию и сравнение глагола по звучанию (T32.H.2).

## 2026-11-28 · T33.1: голоса Piper на sherpa-onnx 2.0.3 не работают без фонемизатора под GPL — пункт не сделан

**Факт.** В `sherpa-onnx` 2.0.x убрали встроенный espeak-ng. Проверено на пакете `sherpa-onnx-v2.0.3-win-x64-shared-MT-Release` (SHA-256 `1d2414ac…`, совпал) и голосе `vits-piper-ru_RU-denis-medium` (SHA-256 `efa4c18e…`, совпал): `--vits-data-dir is deprecated and ignored in sherpa-onnx >= v2.0.0 … Please use GenerationConfig.phoneme_codepoints with an external phonemizer (e.g. piper_phonemize)`, затем `no lexicon, and no frontend available`. То же поле помечено в `c-api.h`. `piper_phonemize` работает через espeak-ng (GPL-3.0). Это противоречит условиям T33.1 (sherpa-onnx 2.0.3, Apache-2.0, программу под GPL не брать). Замену движка или набора моделей исполнитель не выбирает.

**Голоса, прошедшие проверку лицензии** (модель и данные: CC0 или CC-BY 4.0; дообучены от баз CC0 `rdh` или CC-BY 4.0 `hifi_tts_92`, казахские обучены с нуля). Размер — архив из релиза `tts-models`, источник — карточки `rhasspy/piper-voices`. Всем, кроме китайского (`phoneme_type: pinyin`, не проверялся), нужен espeak-ng.

| Голос | Язык | Лицензия | Архив, МБ |
|---|---|---|---|
| ru_RU-denis-medium, ru_RU-dmitri-medium | ru | CC0 | 67 каждый |
| kk_KZ-raya-x_low, kk_KZ-iseke-x_low | kk | CC-BY 4.0 (ISSAI) | 27 каждый |
| en_US-joe-medium | en | CC0 | 67 |
| en_GB-alba-medium | en | CC-BY 4.0 | 67 |
| de_DE-thorsten-medium | de | CC0 | 67 |
| fr_FR-siwis-medium | fr | CC-BY 4.0 | 67 |
| es_ES-davefx-medium | es | CC0 | 67 |
| pt_BR-faber-medium, pt_BR-jeff-medium | pt | CC0 | 67 каждый |
| zh_CN-chaowen-medium | zh | CC0 | 60 |
| — | ar | `clartts` (CC-BY 4.0) нет в сборке для sherpa; `kareem` — «See URL» | — |
| — | hi | только CC BY-NC-SA и собственная лицензия IIT Madras | — |
| — | ja | голосов нет | — |

**Решения за архитектором:** (1) разрешить espeak-ng (GPL-3.0) отдельной программой-фонемизатором: скачивается по требованию, не связывается с кодом Decka, указывается в NOTICE; (2) только модели со словарём без espeak (китайский, английский) — без русского и казахского; (3) остаться на голосах Windows.

**Что сделано в 0.71.0 без Piper:** T33.2 целиком (для голоса Windows), скорость 0,8–1,3 и фразы «быстрее / медленнее / смени голос». Из `ag-m33` убраны элементы Piper: «скачивание» и «Послушать» были имитацией, настройки лежали в `localStorage` (ключ удаляется при старте). Тесты T33.1 (лицензии манифеста, выбор движка) не писались: движка и голосов в манифесте нет.

Дописать архитектору: PROTOCOL / DATA_MODEL — `speechPercent` в `voice_set` и статусе, `speech_voice`, `read_aloud {op}`, ответы `voice.reply.speed*`, `voice_changed`, `one_voice`; правила текста для речи (`voice/spoken`).

## 2026-11-28 · Решения архитектора по отчёту 0.70.1 / 0.71.0

- **Остров (T32.F):** варианты (1) и (3): орб в compact 5 кадров/с, в полосе уровень на экране 10 раз/с. Новые цели: полоса ≤ 5 %, compact ≤ 7 %, expanded ≤ 12 %. Орб на CSS — не делать.
- **«Декка»:** (1) в интерфейсе и справке только «Декка», без «Эй/Хей» и «Hey Decka»; уровень по умолчанию — средняя; (3) память ≈ 110 МБ принимается. Английская модель — в бэклог.
- **EC у глобуса:** закрыто до снимка от владельца.
- **Piper (T33.1):** рекомендован вариант (1) — espeak-ng отдельной программой, скачивается по требованию, не связывается с кодом Decka. Ждёт «да» владельца.
- Долг документации (island_level, voice_chunk ≤ 2880, ответы voice.reply.*, speechPercent, read_aloud {op}) архитектор допишет в DATA_MODEL / PROTOCOL вместе со следующей очередью.

## 2026-11-28 · T34.1: причина молчания «Декка» — тишина гарнитуры принималась за сломанный микрофон

**Причина.** Гарнитура владельца (MCHOSE V9 PRO) в тишине передаёт чистые нули. Рабочий процесс «Декка» открывал микрофон через `mic::start`, где секунда нулей означает «поток мёртв»: переоткрыть, попробовать другие микрофоны, иначе `NO_SOUND` и выход. После четвёртого выхода за 10 минут (`on_crash`) агент выключал слово до смены настроек.

**Доказательство** (только служебные строки логов этого ПК): `voice.log` 15:26:17–15:26:55 — «digital silence: reopening the stream»; `decka.log` — четыре `wake word worker: Microphone gives no sound` (15:26:18, :25, :34, :55), после них перезапусков нет. Установка 0.71.0 — 15:13.

**Это не регрессия кода 0.70.1:** `mic::start` и политика перезапусков те же, что в 0.70.0; в `decka.log` те же четыре отказа есть утром при 0.70.0 (10:27), а после переключения настройки слово сработало (10:33, 10:48). Процесс после установки поверх запускался; уровень в `wake.json` — `medium`, формат не менялся; микрофон по умолчанию тот же.

**Пороги проверены и оставлены** (`wake_eval`, одни и те же файлы: 60 положительных, 41,4 мин отрицательных):

| Средняя | пропуски из 60 | «Декка» ru, пропуски из 10 | ложные |
|---|---|---|---|
| настройки 0.70.0 (1,5 / 0,22 / 0,37, `num_trailing_blanks` 2) | 54 | 5 | 0 |
| сейчас | 42 | 2 | 0 |

**Сделано:** слово пережидает тишину (поток переоткрывается раз в 30 с); другой микрофон берётся только вместо виртуального, который не дал ни звука; после отказов микрофона запуск повторяется раз в минуту; `voice.log` — строка раз в 5 с (`wake alive: chunks peak silent model hits` или `wake not listening: state reason`). Оценку ключевого слова C API 2.0.3 не отдаёт — вместо неё число срабатываний и порций, дошедших до модели.

**Не проверено:** живой микрофон (исполнитель его не открывал).

**За архитектором:** запись заметок и голосовые команды по клавише используют то же правило «секунда нулей — микрофон мёртв»; на этой гарнитуре команда в тишине кончается «no sound from any microphone» (в `voice.log` такие случаи есть), а заметка может оборваться на паузе. В M34 это не входило и не менялось.

## 2026-11-28 · T34.2: «стоп» голосом работает, цель 300 мс не достигнута

Замер `wake_eval` (синтетические голоса Windows, файлы; живой микрофон не использовался), средняя:

| | результат |
|---|---|
| «стоп» / «хватит» / «stop» услышаны | 23 из 30 (пропуски в основном английское «Stop») |
| от конца звука до срабатывания | медиана 500 мс, 90 % — 800 мс; плюс ≤ 100 мс порция микрофона и ≤ 80 мс до обрыва речи |
| ложные «стоп» на своей речи без этих слов | 0 за 42,5 мин русской и 0 за 11,1 мин английской |
| влияние на «Декка» | нет: те же пропуски (42 из 60), ложных 0 |

`num_trailing_blanks` 0 задержку не изменил: она задана размером порции потоковой модели.
 **За архитектором:** принять ≈ 0,6 с или менять модель.

Если «Декка» выключена, но файлы скачаны, процесс запускается на время чтения (загрузка модели в начале чтения — слово в первую секунду можно пропустить; время загрузки не замерено). Значок микрофона в трее в этом случае не меняется. Голос из колонок попадает в микрофон: текст, в котором есть «стоп», на колонках может остановить сам себя (на наушниках — нет). Живой проверки с речью не было.

## 2026-11-28 · T34.3: почему перевод не вставляется — причина не найдена

Теперь при неудаче перевод показан на острове (6 строк с прокруткой, 20 с) с «Скопировано — вставь Ctrl+V»; если проверить вставку нельзя — «Если текст не изменился — вставь Ctrl+V».

**Что видно в `voice.log` 0.71.0** (три попытки перевода): (1) выделение прочитано, поле определено как поле, `Ctrl+V` отправлен, за 2,5 с текст элемента не изменился (`paste Ok(No)`); (2) и (3) элемент в конце запроса определён как «не поле» (`field=No`) — текст ушёл в буфер без попытки вставки. Программа в этих строках не записывалась. Для диктовки в том же логе Блокнот и Chrome определяются как поле; встречается и `field=No` в Chrome.

**Не воспроизведено:** для этого нужно управлять окнами и клавиатурой в сеансе владельца. Простой причины по коду не видно (клавиши отправляются тем же путём, что при диктовке).

**Добавлено для диагностики:** строка `transform …: route= field= was= same= class=` (тип места и класс окна, без текста). После проверки на 0.71.1 по этим строкам будет видно программу.
