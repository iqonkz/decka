# MANUAL_CHECKS.md — ручные проверки для владельца

Формат: задача, текст пункта, статус. `PENDING (owner)` — нужен человек с экраном или телефоном.
`DONE (автоматически)` — исполнитель проверил в headless Chrome (мышь вместо пальца, события через Chrome DevTools Protocol). Физический экран телефона и касания пальцем это не заменяет.

Здесь только открытые пункты. Закрытые (DONE) — в [archive/MANUAL_CHECKS-2026-11-15.md](archive/MANUAL_CHECKS-2026-11-15.md). Закрытый пункт удаляется отсюда.

## T4.4 — сетка, страницы, свайп

- PENDING (owner): то же пальцем на телефоне (Galaxy S9, альбомная ориентация): плитки на своих местах из `layout.default.json`, свайп срабатывает уверенно.

## T4.5 — кнопки

- DONE (автоматически): нажатие `Play` даёт состояния pressed → pending → ok, в `.tmp\headless.log` строка `DRY media-play media`; короткое касание `Lock` ничего не отправляет, удержание 800 мс с круговым индикатором отправляет `DRY lock system`; `Mute` после нажатия меняет иконку на `volume-x` и цвет на `#b45309`.
- PENDING (owner): зелёная рамка и пульсация видны на реальном экране; размер иконок и подписей на телефоне читаем.

## T4.6 — индикатор соединения, тосты, выход из сопряжения

- PENDING (owner): тост с ошибкой действия (например, удалить действие из `actions.json` и нажать его кнопку: `Action is not defined on the PC`, красная полоса слева, закрывается тапом, не более трёх сразу). В автоматическом сценарии тосты не проверялись.
- T4.5: error toast when an action errors — PENDING (owner)

## T5.5 — виджеты clock и sys

- PENDING (owner): на телефоне часы идут по секундам с `seconds: true`; полоса меняет цвет при ≥ 85% и ≥ 95%; вид 2×2 (диск и сеть `↓ ↑`) на экране читаем.

## T5.6 — виджеты statuses и status

- PENDING (owner): плитка `status` (один сервис) на реальной раскладке; вид и читаемость на телефоне.

## T5.7 — уведомления

- PENDING (owner): прокрутка длинной ленты пальцем на телефоне (`touch-action: pan-y`); читаемость тоста и ленты.

## T6.1–T6.2 — оболочка Tauri, трей

- PENDING (owner): визуально — иконка `D` в трее с подсказкой `Decka`; левый клик по иконке показывает окно; пункты меню `Open settings`, `Pair a device`, `Quit` работают, `Quit` завершает процесс; закрытие окна крестиком прячет его (а не только оставляет процесс); при `--minimized` окно при старте не видно, при первом запуске (без `config.json`) видно всегда.

## T6.3–T6.4 — IPC и вкладка Status

- PENDING (owner): внешний вид окна (светлая и тёмная темы Windows), кнопки `Retry` при занятом порте, `Open` (открывает Проводник), `Copy` в блоке Firewall.

## T6.5 — вкладка Pairing

- PENDING (owner): QR из окна сканируется настоящей камерой телефона и открывает клиент; по истечении 120 с блок скрывается и показывается `Code expired`.

## T6.6 — вкладка Actions

- PENDING (owner): внешний вид формы и удобство ввода (ручной ввод в настоящие поля, а не программная подстановка значений).

## T6.7 — вкладка Layout

- PENDING (owner): внешний вид сетки и форм при ручной работе мышью; редактор привязки состояния (`State binding`) и выбор цвета/иконки руками; блок `Display` (время вида `23:00`).

## T6.8 — вкладки Integrations и Settings

- PENDING (owner): внешний вид вкладок и работа руками (набор в полях, кнопки `Copy` с реальным буфером обмена); `Start with Windows` (запись автозапуска в реестр) не проверялась намеренно, чтобы не менять настройки системы; режим `USB only` и поля `USB mode` без телефона (`adb` — M7).

## T7.4 — USB-режим в агенте (adb reverse)

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

## T10.3 — Browse… и New action…

- PENDING (owner): `npm run tauri dev` или установленный агент → Actions → тип `Launch a program` → `Browse…` → `C:\Windows\System32\charmap.exe` → `Name` = `charmap`, `Working directory` = `C:\Windows\System32` → `Save` → `Test` открывает «Таблицу символов». Проверить `New action…` в форме кнопки на вкладке Layout (переключает на Actions с формой нового действия; несохранённые правки формы кнопки теряются). Я не смог проверить окно: во время работы уже был запущен `target\release\decka-agent.exe` (запущен не мной в 22:27), а single-instance передаёт запуск dev-версии ему; системный диалог выбора файла CDP не управляется.

## T11.3–T11.4 — сборщики `focus` и `media`

- PENDING (owner): обложка у источников с маленькой картинкой (браузер, другие плееры); вкладка Status показывает `Active app: …` при переключении окон.

## T11.4b–T11.7 — обложка, контекстные страницы, виджет, окно настроек

- PENDING (owner): T11.4b — при играющем Spotify `ws-smoke state` показывает `media.art`, начинающийся с `data:image/jpeg;base64,`.
- PENDING (owner): T11.5 — на телефоне: страница `VS Code` открывается при фокусе VS Code, подсказка с именем 2 с, возврат при уходе; касание откладывает на 5 с; затемнённый экран не просыпается от переключения.
- PENDING (owner): T11.6 — вид виджета `media` на телефоне (обложка, полоса, клавиши при `h = 1` и `h = 2`), `play` ставит на паузу.
- PENDING (owner): T11.7 — окно настроек (`npm run tauri dev` или установленный агент): блок `Show this page automatically when active`, `Refresh`, диалог `Add page from template`, форма `media`. Окно я не проверял.

## T11.9–T12.5 — понятные ошибки, лог, стиль деки, иконки программ

- PENDING (owner): T11.9 — клик по замечанию в редакторе выделяет элемент; новая кнопка получает первое действие.
- PENDING (owner): T11.10 — вид клавиш на телефоне (снимок `.tmp\keys-deck.png` сделан, шеврон `›` на кнопке `navigate` на нём не виден: на видимой странице таких кнопок нет).
- PENDING (owner): T11.11 — лог: `Open logs` на вкладке Status открывает `%APPDATA%\Decka\logs\decka.log`, строки `press …` попадают в файл. Нажатие `media-play` доходит до агента и возвращает ok, но Spotify на этом ПК состояние не меняет (ни через enigo, ни через сырой `keybd_event`) — проверить на ПК владельца по логу.
- PENDING (owner): T12.5 — окно настроек (`npm run tauri dev` или установленный агент): диалог `Choose installed app…`, поле иконки кнопки (`From program`, `From image…`, `Clear`, список Lucide), иконка на телефоне ≤ 1 с после `Save`. Окно я не проверял.

## T12.0, T13.1–T13.5 — сессия Windows, @decka/ui, темы

- PENDING (owner): T12.0 — то же на живом Spotify (во время проверки Spotify не был запущен).
- PENDING (owner): T13.5 — телефон был отключён (`adb devices` пуст). Нужно: открыть `client/dev/glass-probe.html?mode=url` и `?mode=plain` на A57 (`adb reverse` + Chrome, `adb logcat -s chromium | Select-String GLASSPROBE`) и записать `supportsUrl`, видимое искажение (`?bg=stripes`) и `fps`; затем тема Glass на телефоне: средняя частота кадров, режим `reduced`, обои `album` при играющей музыке (`glass-album.png`).

## T13.6–T13.11 — движение, вибрация, группы, Phosphor, редактор, вид окна, меню

- PENDING (owner): всё это на телефоне A57 (отключён во время работы): вид и частота кадров четырёх тем, вибрация (APK установить не удалось — нет `adb`-устройства), жест удержания 1,2 с пальцем, свайп «за пальцем», группы, `album` при играющей музыке (`glass-album.png`), заставка (`final-saver.png`).
- PENDING (owner): окно настроек в `npm run tauri dev` или из установщика: перетаскивание в редакторе (проверено в Chrome с подменой IPC: превью, наложение ячеек, выбор, ручка размера; сам drag мышью — не проверен), диалог мастера на чистом `DECKA_CONFIG_DIR`, вкладка Phosphor.

## T10.5 — настраиваемая сетка и обводка клавиш

- DONE (A57 only — S9 PENDING (owner)): свежий агент с конфигурацией по умолчанию → снимок `.tmp\grid74.png`: 7 столбцов × 4 строки, у клавиш тонкая обводка, `Lock` с цветной подложкой и обводкой.
- PENDING (owner): в окне настроек (установленный агент или `npm run tauri dev`) вкладка Layout → `Columns` = 5 → `Save` → сетка на телефоне перестраивается ≤ 1 с (если элементы помещаются); при уменьшении сетки лишние элементы подсвечиваются красным и `Save` заблокирован. Окно я не проверял: запущен `target\release\decka-agent.exe` владельца.

## T9.4 — итоговая приёмка MVP

- PENDING (owner): установка агента из `Decka_0.1.0_x64-setup.exe` на чистый профиль (переименование `%APPDATA%\Decka` в `Decka.bak`) — установка в систему и перенос реальной папки конфигурации не выполнялись.
- PENDING (owner): прохождение чек-листа `docs/TESTING.md` §3 целиком на Galaxy S9 (на A57 пройдены только пункты, отмеченные выше как `A57 only`).

## T8.1–T8.3 — затемнение, pixel shift, заставка

- PENDING (owner): на телефоне — внешний вид (яркость затемнения на AMOLED, серый цвет часов), смена места часов каждые 30 с с плавным исчезновением, `PC offline` на заставке после остановки агента на минуту, видимое смещение pixel shift (раз в минуту на 1–4 px).

## T13.12 — Glass contrast, readableFace, A57 (2026-11-15)

- PENDING (owner): popup groups on the phone (the owner's layout has none), `album` wallpaper with music, screensaver, perception of the vibration strength/pattern.

## T13.13 — светлые клавиши, клавиши media, к/с reduced (2026-11-15)

- PENDING (owner): виджет media на экране приложения с твоей раскладки (3×1 — без клавиш; 4×1 — одна `pause`; клавиши `prev/next` появляются с `w ≥ 5`).

## T14.0–T14.4 — Glass perf, drag, quick picker, autosave, onboarding (2026-11-15)

- PENDING (owner): right-click menus of a widget, an empty cell and a page tab (built and unit-tested, only the key menu was seen in the window); `Esc` cancel of a drag; `Ctrl+C`/`Ctrl+V`/`Ctrl+D`; the shortcut recorder and the `Website…` / `Browse…` rows of the picker; `Change action…`; `Run setup again` (About card in Settings) and the empty-page buttons; the result of all this on the phone (the phone is paired with the owner's agent; the new buttons were saved only in the copies); the Esc key hid the window once during automation, not reproduced by hand.

## T14.4a–T14.6 — key preview, big icons, S9, simpler window (2026-11-15)

- PENDING (owner): **S9 (`232512e0080c7ece`) is locked** (the screen is off and the PIN screen `Bouncer` is up), so the APK was installed but nothing could be run on it. Unlock it and turn on "stay awake" (Developer options) or remove the lock for the time of the check; then T14.5a can be done: pairing, the `A57 only - S9 PENDING` items (T7.1, T7.2, T7.4, T7.5, 7×4 grid), `s9-<theme>.png`, `s9-full.png`, fps per theme with `gfxinfo`, labels at a key side of ~80 px, the media widget title.
- PENDING (owner): A57 is not plugged in: `a57-full-<theme>.png` of T14.5 (use the headless snapshots above meanwhile).
- PENDING (owner): `Sharpen` on a real layout (the owner's old 96 px icons), the quick check of the grid choice with a widget too big for the new grid, `Forget` of a paired phone, the firewall hint (shows 2 minutes after a pairing without any connection), `Add check` dialog with a real address, `Choose…` for an app in the action form.

