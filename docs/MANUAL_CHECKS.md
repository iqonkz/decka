# MANUAL_CHECKS.md — ручные проверки для владельца

Формат: задача, текст пункта, статус. `PENDING (owner)` — нужен человек с экраном или телефоном.
`DONE (автоматически)` — исполнитель проверил в headless Chrome (мышь вместо пальца, события через Chrome DevTools Protocol). Физический экран телефона и касания пальцем это не заменяет.

## T4.3 — экран сопряжения

- DONE (автоматически): открытие `http://127.0.0.1:19247/#c=TESTCDE2&h=127.0.0.1&p=19247&n=Test` сопрягает клиент, `#c=...` исчезает из адресной строки; после перезагрузки с тем же профилем клиент остаётся сопряжённым (в `devices.json` одно устройство); на чистом профиле виден экран `Pair with your PC`.

## T4.4 — сетка, страницы, свайп

- DONE (автоматически): на первой странице 10 плиток, точек две; свайп влево открывает `Dev` с 5 плитками, вторая точка активна; свайп вправо возвращает; за первую страницу не зацикливается; плитка `Main` возвращает на первую.
- PENDING (owner): то же пальцем на телефоне (Galaxy S9, альбомная ориентация): плитки на своих местах из `layout.default.json`, свайп срабатывает уверенно.

## T4.5 — кнопки

- DONE (автоматически): нажатие `Play` даёт состояния pressed → pending → ok, в `.tmp\headless.log` строка `DRY media-play media`; короткое касание `Lock` ничего не отправляет, удержание 800 мс с круговым индикатором отправляет `DRY lock system`; `Mute` после нажатия меняет иконку на `volume-x` и цвет на `#b45309`.
- PENDING (owner): зелёная рамка и пульсация видны на реальном экране; размер иконок и подписей на телефоне читаем.

## T4.6 — индикатор соединения, тосты, выход из сопряжения

- DONE (автоматически): после остановки агента первые секунды плашки нет, примерно через 3 с появляется `PC offline — retrying`, кнопки с действиями полупрозрачны; после запуска агента плашка исчезает (≤ 16 с) без перезагрузки страницы; четыре тапа в левом верхнем углу диалог не открывают, пятый открывает `Unpair this device?`; `Cancel` закрывает; `Unpair` ведёт на экран сопряжения и стирает сохранённое соединение.
- PENDING (owner): тост с ошибкой действия (например, удалить действие из `actions.json` и нажать его кнопку: `Action is not defined on the PC`, красная полоса слева, закрывается тапом, не более трёх сразу). В автоматическом сценарии тосты не проверялись.

- T4.5: error toast when an action errors — PENDING (owner)

## T5.5 — виджеты clock и sys

- DONE (автоматически): часы показывают `HH:MM` и дату (при ширине 2), у `sys` 2×1 две полосы CPU и RAM, значения меняются при живом агенте (CPU 54% → 30%).
- PENDING (owner): на телефоне часы идут по секундам с `seconds: true`; полоса меняет цвет при ≥ 85% и ≥ 95%; вид 2×2 (диск и сеть `↓ ↑`) на экране читаем.

## T5.6 — виджеты statuses и status

- DONE (автоматически): после `POST /api/status` из `http-status-req.json` на странице `Dev` в списке `Claude: Decka` с синей пульсирующей точкой и текстом `editing files`; пустой список показывает `No statuses yet`; при переполнении последняя строка `+N more`, порядок error → … → ok.
- PENDING (owner): плитка `status` (один сервис) на реальной раскладке; вид и читаемость на телефоне.

## T5.7 — уведомления

- DONE (автоматически): `POST /api/notify` из `http-notify-req.json` даёт тост (исчезает через ~8 с) и строку в ленте `ci · now` с зелёной полосой; свайп внутри виджета страницу не меняет; тап по строке убирает её, и второй клиент видит пустой список.
- PENDING (owner): прокрутка длинной ленты пальцем на телефоне (`touch-action: pan-y`); читаемость тоста и ленты.

## T6.1–T6.2 — оболочка Tauri, трей

- DONE (автоматически, с `DECKA_CONFIG_DIR=.tmp\cfg-agent`): `decka-agent.exe` поднимает сервер, `/api/health` отвечает, `/` отдаёт клиент (200); второй запуск exe не создаёт второй процесс (счётчик `1`); после `CloseMainWindow` процесс и сервер продолжают работать.
- PENDING (owner): визуально — иконка `D` в трее с подсказкой `Decka`; левый клик по иконке показывает окно; пункты меню `Open settings`, `Pair a device`, `Quit` работают, `Quit` завершает процесс; закрытие окна крестиком прячет его (а не только оставляет процесс); при `--minimized` окно при старте не видно, при первом запуске (без `config.json`) видно всегда.

## T6.3–T6.4 — IPC и вкладка Status

- DONE (автоматически, `npm run tauri dev` с `DECKA_CONFIG_DIR=.tmp\cfg-agent`, окно управлялось через WebView2 remote debugging): шесть вкладок в нужном порядке; `Listening on 0.0.0.0:9247`; блок Firewall с командой на нужный порт; `start_pairing` возвращает код, URL с `#c=` и QR-SVG; после сопряжения клиента в браузере устройство появляется в таблице без перезагрузки окна и исчезает после отключения.
- PENDING (owner): внешний вид окна (светлая и тёмная темы Windows), кнопки `Retry` при занятом порте, `Open` (открывает Проводник), `Copy` в блоке Firewall.

## T6.5 — вкладка Pairing

- DONE (автоматически, настоящее окно через `tauri dev`): кнопка `Pair a new device` показывает QR (белая подложка 260 px + 16 px), код `XXXX-XXXX`, обратный отсчёт и пояснение; `Cancel` и уход с вкладки гасят код на агенте (повторное сопряжение по нему даёт `PAIR_CODE_INVALID`); после сопряжения клиента QR скрыт, показано `Paired: Browser`, в таблице строка с зелёной точкой; `Revoke` с подтверждением `Revoke Browser? It will need to pair again.` убирает строку, а клиент возвращается на экран сопряжения.
- PENDING (owner): QR из окна сканируется настоящей камерой телефона и открывает клиент; по истечении 120 с блок скрывается и показывается `Code expired`.

## T6.6 — вкладка Actions

- DONE (автоматически, настоящее окно): список действий по алфавиту; неверный ID, дубликат ID и неверные клавиши показываются ошибками; созданное действие `launch` → `charmap.exe` попадает в `actions.json` (в каталоге `DECKA_CONFIG_DIR`), `Test` возвращает `OK` и открывает «Таблицу символов»; подтверждение удаления `media-play` перечисляет кнопку `b-play`; удаление работает.
- PENDING (owner): внешний вид формы и удобство ввода (ручной ввод в настоящие поля, а не программная подстановка значений).

## T6.7 — вкладка Layout

- DONE (автоматически, настоящее окно + подключённый браузерный клиент): страницы `Main` и `Dev`; на `Main` пустых ячеек нет (все 15 заняты), на `Dev` две; для виджета 2×2 в ячейке (4,1) показывается `Not enough free space here`; добавленная кнопка (`Label: Hi`, действие `notepad`, id вида `b-xxxxxx`) после `Save` попадает в `layout.json` и появляется у подключённого клиента ≤ 1 с без перезагрузки; пересечение даёт замечание, красную рамку и блокирует `Save`; `Revert` перечитывает файл; `Add page` / `Delete page` работают.
- PENDING (owner): внешний вид сетки и форм при ручной работе мышью; редактор привязки состояния (`State binding`) и выбор цвета/иконки руками; блок `Display` (время вида `23:00`).

## T6.8 — вкладки Integrations и Settings

- DONE (автоматически, настоящее окно, `DECKA_CONFIG_DIR=.tmp\cfg-agent`): токен скрыт, `Show` открывает его, совпадает с `config.json`; скопированные команды выполнены в Windows PowerShell 5.1 как есть: `curl.exe` (текст), `curl.exe` (JSON через файл), `Invoke-RestMethod` возвращают `{"id":"…"}`, команда статуса — `{"ok":true}`; `Send test notification` даёт тост `Decka test` на клиенте; `Regenerate` просит подтверждение, после него старый токен даёт `UNAUTHORIZED`; HTTP-проверка добавляется, видна на клиенте статусом `200 · N ms` и исчезает после удаления; порт 80 отклоняется с замечанием, порт 9248 + `Restart now` — `/api/health` отвечает на 9248 (и не отвечает на 9247), затем порт возвращён на 9247.
- Найдено и исправлено при этой проверке: `save_config` из окна (поток без контекста tokio) вызывал панику при запуске HTTP-проверок; теперь агент запускает задачи через сохранённый `Handle` рантайма (регрессионный тест `save_config_works_from_a_thread_without_a_runtime`).
- PENDING (owner): внешний вид вкладок и работа руками (набор в полях, кнопки `Copy` с реальным буфером обмена); `Start with Windows` (запись автозапуска в реестр) не проверялась намеренно, чтобы не менять настройки системы; режим `USB only` и поля `USB mode` без телефона (`adb` — M7).

## T7.4 — USB-режим в агенте (adb reverse)

- DONE (автоматически): `parse_devices` (4 теста) и цикл целиком против поддельного `adb` (.cmd): устройство `device` получает `reverse tcp:<port> tcp:<port>`, `unauthorized` попадает в статус с `reversed: false` и текстом ошибки, отсутствующий `adb` даёт `adbFound: false` (`agent/core/tests/usb.rs`).
- PENDING (owner): телефон с отладкой по USB, `adb` в PATH, `USB mode` включён: `adb reverse --list` показывает `tcp:9247 tcp:9247` в течение 10 с после подключения кабеля.

## T7.1–T7.3, T7.5 — Android (устройство проверки: Galaxy A57, R5GL364260L; целевой S9 не подключён)

- T7.1: DONE (A57 only — S9 PENDING (owner)): `adb install -r app-debug.apk` → Success; приложение открывается в альбомной ориентации (`ROTATION_90`) на экране сопряжения.
- T7.2: DONE (A57 only — S9 PENDING (owner)): нет статус-бара и навигационной панели (снимок экрана); при системном таймауте 15 с экран остался включённым через 40 с (`mWakefulness=Awake`). Полные 5 минут при 30 с на S9 — PENDING (owner).
- T7.3: A57: кнопка `Scan QR code` есть, нажатие запрашивает разрешение камеры, после выдачи открывается сканер (`OSBARCScannerActivity`). Само сканирование QR с экрана ПК (< 5 с до сетки) — PENDING (owner): нужны руки. `minSdkVersion` поднят до 26 (требует README плагина `@capacitor/barcode-scanner`).
- T7.4 (на A57): DONE (A57 only — S9 PENDING (owner)): с `usb.enabled` `adb reverse --list` показал `tcp:9247 tcp:9247` через 9 с после запуска агента.
- T7.5 (а): DONE (A57 only — S9 PENDING (owner)): Wi-Fi телефона выключен (`svc wifi disable`), кабель подключён, USB-режим включён; приложение сопрягается по `127.0.0.1` и показывает сетку, индикатор `USB` (4 с), кнопка `Play` → агент получил `media-play` (dry-run).
- T7.5 (б): DONE (A57 only — S9 PENDING (owner)): `bind = 127.0.0.1` после перезапуска: `http://192.168.10.3:9247/api/health` с ПК не отвечает, loopback отвечает, телефон по USB работает (`media-play` получен).
- T7.5 (в): PENDING (owner): отключить кабель при включённом Wi-Fi и режиме `Wi-Fi and USB`, переподключение ≤ 20 с, индикатор `Wi-Fi` (требует физического отключения кабеля).
- T7.5 (г): PENDING (owner): сон ПК на 1 минуту и пробуждение, восстановление связи ≤ 30 с.
- Наблюдение (не воспроизведено): после перезапуска headless в двух прогонах в логе появились лишние `DRY vol-down`/`task-manager` до моего касания; повторные перезапуски без касаний (2 раза по 14 с и 20 с) лишних нажатий не дали. Причина не найдена; связано, вероятно, с моими командами `adb input`.

## T10.3 — Browse… и New action…

- DONE (автоматически): `npm run typecheck -w agent`, тесты `applyPickedProgram` (заполнение `Program`/`Name`/`Working directory`, непустые поля сохраняются), `npm run build:ui`, `cargo build -p decka-agent` (плагин `tauri-plugin-dialog` зарегистрирован, `dialog:allow-open` в capabilities).
- PENDING (owner): `npm run tauri dev` или установленный агент → Actions → тип `Launch a program` → `Browse…` → `C:\Windows\System32\charmap.exe` → `Name` = `charmap`, `Working directory` = `C:\Windows\System32` → `Save` → `Test` открывает «Таблицу символов». Проверить `New action…` в форме кнопки на вкладке Layout (переключает на Actions с формой нового действия; несохранённые правки формы кнопки теряются). Я не смог проверить окно: во время работы уже был запущен `target\release\decka-agent.exe` (запущен не мной в 22:27), а single-instance передаёт запуск dev-версии ему; системный диалог выбора файла CDP не управляется.

## T11.3–T11.4 — сборщики `focus` и `media`

- DONE (автоматически, этот ПК): headless + `ws-smoke watch` 9 с, пока запускались `notepad` и затем `charmap` → `patch` секции `focus` с `notepad.exe` и с `charmap.exe`. `ws-smoke state` при играющем Spotify → `media` с `title`, `artist`, `album`, `durationSec`, `positionSec`, `source`.
- Наблюдение: обложка Spotify (PNG ≈ 176 КБ) длиннее лимита 65 536 символов data URI. Решено в T11.4b: уменьшение до 192 px JPEG через Windows.Graphics.Imaging.
- PENDING (owner): обложка у источников с маленькой картинкой (браузер, другие плееры); вкладка Status показывает `Active app: …` при переключении окон.

## T11.4b–T11.7 — обложка, контекстные страницы, виджет, окно настроек

- DONE (автоматически): тесты `media::` (уменьшение PNG 600×600 → JPEG 192×192, data URL < 65 536; малая обложка остаётся как есть), клиент 95 тестов (`decideContextPage`, `currentPosition`, `formatTime`, store), агент 27 тестов (`normalizeApp`, `withApps`, `withMediaDefaults`), `npm run build -w client`, `npm run build:ui`, `cargo build -p decka-agent`, в `commands.rs` 19 команд.
- PENDING (owner): T11.4b — при играющем Spotify `ws-smoke state` показывает `media.art`, начинающийся с `data:image/jpeg;base64,`.
- PENDING (owner): T11.5 — на телефоне: страница `VS Code` открывается при фокусе VS Code, подсказка с именем 2 с, возврат при уходе; касание откладывает на 5 с; затемнённый экран не просыпается от переключения.
- PENDING (owner): T11.6 — вид виджета `media` на телефоне (обложка, полоса, клавиши при `h = 1` и `h = 2`), `play` ставит на паузу.
- PENDING (owner): T11.7 — окно настроек (`npm run tauri dev` или установленный агент): блок `Show this page automatically when active`, `Refresh`, диалог `Add page from template`, форма `media`. Окно я не проверял.

## T11.9–T12.5 — понятные ошибки, лог, стиль деки, иконки программ

- DONE (автоматически): тесты `describeIssue`; `logging::` (ротация, папка); `icons::` на этом ПК (иконка `notepad.exe` → PNG 96×96, повтор → тот же hash и один файл; картинка 600×300 → 96×96); `installed::` (реальное меню «Пуск»: ≤ 500 программ, все `.exe` существуют, без деинсталляторов, по алфавиту); `tests/icons.rs` (правило 11, секция `icons` при старте и после `save_layout`); клиент 99 тестов (`img:` → `<img>` / квадрат); агент 36 тестов; в `commands.rs` 24 команды.
- PENDING (owner): T11.9 — клик по замечанию в редакторе выделяет элемент; новая кнопка получает первое действие.
- PENDING (owner): T11.10 — вид клавиш на телефоне (снимок `.tmp\keys-deck.png` сделан, шеврон `›` на кнопке `navigate` на нём не виден: на видимой странице таких кнопок нет).
- PENDING (owner): T11.11 — лог: `Open logs` на вкладке Status открывает `%APPDATA%\Decka\logs\decka.log`, строки `press …` попадают в файл. Нажатие `media-play` доходит до агента и возвращает ok, но Spotify на этом ПК состояние не меняет (ни через enigo, ни через сырой `keybd_event`) — проверить на ПК владельца по логу.
- PENDING (owner): T12.5 — окно настроек (`npm run tauri dev` или установленный агент): диалог `Choose installed app…`, поле иконки кнопки (`From program`, `From image…`, `Clear`, список Lucide), иконка на телефоне ≤ 1 с после `Save`. Окно я не проверял.

## T12.0, T13.1–T13.5 — сессия Windows, @decka/ui, темы

- DONE (автоматически): T12.0 — `via session` для play/pause/next/prev на реальной медиасессии (Chrome, собственный PID); `via key` без сессии. Тесты: shared 80, ui 43, client 71, agent 36, `cargo test -p decka-core` зелёный.
- PENDING (owner): T12.0 — то же на живом Spotify (во время проверки Spotify не был запущен).
- PENDING (owner): T13.5 — телефон был отключён (`adb devices` пуст). Нужно: открыть `client/dev/glass-probe.html?mode=url` и `?mode=plain` на A57 (`adb reverse` + Chrome, `adb logcat -s chromium | Select-String GLASSPROBE`) и записать `supportsUrl`, видимое искажение (`?bg=stripes`) и `fps`; затем тема Glass на телефоне: средняя частота кадров, режим `reduced`, обои `album` при играющей музыке (`glass-album.png`).
- Результат на ПК (Chromium 157, headless): `backdrop-filter: url(#decka-glass)` поддерживается — полосы в `glass-probe-stripes-url.png` искажены; 28 клавиш ≈ 130–145 к/с. Слой преломления оставлен; на медленном устройстве клиент сам переходит в `reduced` (< 45 к/с, ключ `decka.glass.reduced.v1`).
- Снимки тем (рендер в Chrome 915×412 @2x на тестовом агенте, не на телефоне): `.tmp\design\amoled.png`, `studio.png` (нажата первая клавиша), `material.png`, `material-green.png`, `glass-aurora.png`, `glass-reduced.png`.

## T13.6–T13.11 — движение, вибрация, группы, Phosphor, редактор, вид окна, меню

- DONE (автоматически): `swipeDecision`, `hapticFor` (каждая строка таблицы §3a и исключения), `placePopup`, правило 12 в TS и Rust (негативные тесты), `icon_from_svg` (допустимый SVG → `img:`; `<script>`, `onload=`, внешний `href` → `Unsupported SVG`; повтор → тот же hash), `visibleRows`, `moveItem`/`swapItems`/`resizeItem`/история, `planAppButtons`, `statusLine`, контраст (AMOLED, Studio, Material × 8 акцентов; Glass — см. BLOCKERS.md D-1); `is_first_run`. Итог: shared 93, ui 53, client 103, agent 69, `cargo test -p decka-core` зелёный; в `commands.rs` 26 команд.
- Снимки в `.tmp\design\`: `final-<тема>-1..3.png` для четырёх тем (первая страница с данными, страница Dev, открытая группа), `final-glass-menu.png` (меню), `popup.png`, `swipe-mid*.png`, `editor.png`, `appearance.png`, `wizard-look.png`, `phosphor-<тема>.png`. Это рендеры в Chrome 915×412 @2x на тестовом агенте, не экран телефона.
- PENDING (owner): всё это на телефоне A57 (отключён во время работы): вид и частота кадров четырёх тем, вибрация (APK установить не удалось — нет `adb`-устройства), жест удержания 1,2 с пальцем, свайп «за пальцем», группы, `album` при играющей музыке (`glass-album.png`), заставка (`final-saver.png`).
- PENDING (owner): окно настроек в `npm run tauri dev` или из установщика: перетаскивание в редакторе (проверено в Chrome с подменой IPC: превью, наложение ячеек, выбор, ручка размера; сам drag мышью — не проверен), диалог мастера на чистом `DECKA_CONFIG_DIR`, вкладка Phosphor.

## T10.5 — настраиваемая сетка и обводка клавиш

- DONE (A57 only — S9 PENDING (owner)): свежий агент с конфигурацией по умолчанию → снимок `.tmp\grid74.png`: 7 столбцов × 4 строки, у клавиш тонкая обводка, `Lock` с цветной подложкой и обводкой.
- PENDING (owner): в окне настроек (установленный агент или `npm run tauri dev`) вкладка Layout → `Columns` = 5 → `Save` → сетка на телефоне перестраивается ≤ 1 с (если элементы помещаются); при уменьшении сетки лишние элементы подсвечиваются красным и `Save` заблокирован. Окно я не проверял: запущен `target\release\decka-agent.exe` владельца.

## T9.4 — итоговая приёмка MVP

- DONE (автоматически): тест `extra_fields_are_ignored` в `agent/core/tests/press.rs` (лишние `program`/`args` игнорируются: один вызов `media-play`; неизвестный `actionId` → `UNKNOWN_ACTION`, ноль новых вызовов); `node tools/ws-smoke.mjs raw 127.0.0.1:19247 .tmp\evil.json` без токена → `UNAUTHORIZED`, код выхода 1 (для этого `raw` теперь завершается кодом 1, когда ответ сервера — `error`; ответ по-прежнему печатается); в `client/src/net/` нет строк `program`, `hotkey`, `open_url`, `eval(`, `new Function(`.
- PENDING (owner): установка агента из `Decka_0.1.0_x64-setup.exe` на чистый профиль (переименование `%APPDATA%\Decka` в `Decka.bak`) — установка в систему и перенос реальной папки конфигурации не выполнялись.
- PENDING (owner): прохождение чек-листа `docs/TESTING.md` §3 целиком на Galaxy S9 (на A57 пройдены только пункты, отмеченные выше как `A57 only`).

## T9.1 — веб-манифест и иконки клиента

- T9.1: Android launcher icon is the Capacitor template icon — replace before public release (owner)

## T8.1–T8.3 — затемнение, pixel shift, заставка

- DONE (автоматически, headless Chrome через CDP, агент `decka-headless --dry-run` с `dimAfterSec: 10`, `sleepAfterSec: 30`): через 10 с слой затемнения (непрозрачность 0.7, перехватывает касания); первое касание возвращает яркость и не нажимает кнопку; через 30 с остаются только серые часы `HH:MM`, сетка скрыта; уведомление `info` не будит экран и не даёт тост; `error` будит и показывает тост; касание будит заставку.
- PENDING (owner): на телефоне — внешний вид (яркость затемнения на AMOLED, серый цвет часов), смена места часов каждые 30 с с плавным исчезновением, `PC offline` на заставке после остановки агента на минуту, видимое смещение pixel shift (раз в минуту на 1–4 px).

## T13.12 — Glass contrast, readableFace, A57 (2026-11-15)

- DONE (A57, USB, APK from T13.12): screenshots of the phone screen, `adb pull`: `.tmp\design\a57-<amoled|studio|material|glass>-1.png` (page 1) and `-2.png` (page 2), `a57-glass-menu.png` (phone menu).
- DONE: hold 1.2 s on an empty spot opens the phone menu (Reduce effects, Haptics, Reload, Unpair). A hold that lands on a widget does not open it (expected).
- DONE: haptics - `dumpsys vibrator_manager` lists ~100 ms TOUCH effects from `app.decka.client` on taps and swipes.
- DONE: Spotify playing - `media next`, `media play_pause` (twice) all logged `via session`.
- Frame rate on A57 (`dumpsys gfxinfo`, 3 swipe round trips per theme; p50 frame time -> fps): AMOLED 10 ms (~100), Studio 10 ms (~100), Material 11 ms (~90), Glass 27 ms (~37, 536 frames, 0.56% janky). Glass is below the 45 fps threshold, so the client's own probe puts it in `reduced` mode.
- Method note: the theme was switched by temporarily editing `display.theme` in the owner's `%APPDATA%\Decka\layout.json` (file watcher applied it live); the original file was restored byte-for-byte (`fc /b` identical).
- PENDING (owner): popup groups on the phone (the owner's layout has none), `album` wallpaper with music, screensaver, perception of the vibration strength/pattern.
## T13.13 — светлые клавиши, клавиши media, к/с reduced (2026-11-15)

- DONE (A57): `.tmp\design\a57-glass-1.png` (Glass, виджет media 3×1: видны название и исполнитель, клавиш нет), `.tmp\design\a57-studio-keys.png` (Studio: белая и жёлтая — тёмная подпись, `#7c5cff` затемнена под белую, красная — белая).
- Method: files in `%APPDATA%\Decka` were not touched. A second agent (`decka-headless`, port 19247, `--config-dir .tmp\cfg-phone`, a copy of the owner's config) was opened in Chrome on the phone (the installed app stays paired with the owner's agent). Chrome's bars are cropped out of the screenshots. Landscape was forced with `settings put system user_rotation` and restored (`accelerometer_rotation 1`).
- DONE: к/с Glass `reduced` и обычный режим — см. BLOCKERS.md D-2 (оба ~37 к/с). Переключатель `Reduce effects` в приложении возвращён в выключенное состояние.
- PENDING (owner): виджет media на экране приложения с твоей раскладки (3×1 — без клавиш; 4×1 — одна `pause`; клавиши `prev/next` появляются с `w ≥ 5`).
## T14.0–T14.4 — Glass perf, drag, quick picker, autosave, onboarding (2026-11-15)

- Method: the settings window was checked in the real Tauri window (`npm run tauri -- dev --config` with a different `identifier`, `DECKA_CONFIG_DIR=.tmp\cfg-dev` / `.tmp\cfg-clean`, copies of the owner's files; `%APPDATA%\Decka` was only read). Mouse and keys were sent with `mouse_event` / `SendKeys`; an Explorer drag was made with a real OLE file drag from a small helper window. Screenshots: `.tmp\design\m14-*.png`.
- DONE (real window): move, swap, palette drag, `notepad.exe` dropped from a file drag (`m14-drag-*`, `m14-swap-after`, `m14-palette-after`, `m14-explorer-*`); quick picker in two clicks (`m14-picker`, `m14-created`); key context menu (`m14-menu`); F2 rename on the key; autosave of a colour change to `layout.json` and its error banner for a missing icon file; properties panel (`m14-props`); wizard on a clean config dir with the new `Media & system` row, three tips once only, `Phone not connected` strip (`m14-wizard`, `m14-tips`, `m14-tips3`, `m14-tips-once`).
- DONE: Glass perf table is in BLOCKERS.md D-2 (normal ~38 fps, `reduced` ~83 fps on A57).
- PENDING (owner): right-click menus of a widget, an empty cell and a page tab (built and unit-tested, only the key menu was seen in the window); `Esc` cancel of a drag; `Ctrl+C`/`Ctrl+V`/`Ctrl+D`; the shortcut recorder and the `Website…` / `Browse…` rows of the picker; `Change action…`; `Run setup again` (About card in Settings) and the empty-page buttons; the result of all this on the phone (the phone is paired with the owner's agent; the new buttons were saved only in the copies); the Esc key hid the window once during automation, not reproduced by hand.