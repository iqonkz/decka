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


## T14.6a — leftovers (2026-11-15)

- PENDING (owner): in the real window (not automated — no way to click it from the session): `Choose…` in the action form (a program from the list and a `.lnk` through `Browse…`), `Forget` of a paired phone (on a test device in a copy of the config), `Add check` with `https://example.com` (the status must show on the phone / in `ws-smoke state`).
- PENDING (owner): `Sharpen` once in `Look` on the real layout: icons made before processing version 2 (trim, 6% margin, Shell frame cut) are redone; 7-Zip in the `Big icon` mode should now fill the key (checked on a stored PNG in `.tmp\icons-check`, not on the phone).
## T14.7 — Russian (2026-11-15)

- PENDING (owner): the tray menu in Russian (`Открыть настройки`, `Подключить телефон`, `Выход`) and its switch after `More → Settings → Language` (unit-tested, not seen: the tray of a test copy was not inspected).
- PENDING (owner): the pairing screen of a real unpaired phone in Russian (the shot `phone-ru-pairing.png` is headless Chrome at the S9 size with a Russian `Accept-Language`; the S9 is paired with the owner's agent and was not unpaired).
- Left in English on purpose: names of the four looks (AMOLED, Studio, Material, Glass), `Phosphor`, key names in shortcuts (`Ctrl`, `Alt`, `Shift`, `Win`), `USB`, `Wi-Fi`, `QR`, `CPU`, `RAM`; text that comes from the PC — validation messages (`ValidationIssue.message`), `press_result.error`, built-in page template names and their buttons (Rust, DATA_MODEL.md 7); the default page `Main` of a new layout; the weights of Phosphor icons in the picker.
## T14.9 — portrait orientation (2026-11-16)

- PENDING (owner / S9 later): the S9 was not reachable over `adb` any more (`adb devices` empty, no `192.168.10.9:5555`) when the phone part was due. Not done on a phone: `phone-portrait-<theme>.png` (4 looks), `phone-portrait-popup.png`, `phone-portrait-menu.png`, `mCurrentRotation` right after a restart in portrait, the safe-area bottom padding on a gesture-bar phone. Done and seen in the real window: `Look → Orientation` (cards, the turn dialog with the preview and the counter, `Rotate`, `Ctrl+Z` handler), `m14-orientation.png`, `m14-orientation-deck.png`. Recipe for the phone: a test copy `DECKA_CONFIG_DIR` with `display.orientation: "portrait"` listening on `127.0.0.1:9247` (the second agent does not take part in the single-instance lock), `adb reverse tcp:9247 tcp:9247`, the phone takes the USB candidate with the token copied in `devices.json`.
- PENDING (owner): `Ctrl+Z` after `Rotate` in the real window (the handler is on `Look`; checked by reading, not pressed).
## T14.10 — S9 acceptance before v0.2.0 (2026-11-16)

- DONE (S9 `232512e0080c7ece`, SM-G960F, 1080x2220; APK built from this tree, test agent on a copy of `.tmp\cfg-ru` -> `.tmp\cfg-s9`, `127.0.0.1:9247`, `adb reverse`; the owner's configuration was not touched): the phone connects over USB, live widgets (media with cover, clock, CPU/RAM) work.
- DONE portrait (3x8): `.tmp\phone-portrait-glass.png`, `-amoled.png`, `-studio.png`, `-material.png`, `phone-portrait-popup.png`, `phone-portrait-menu.png` (menu shows "App 0.1.0 · PC 0.1.0"); `mCurrentRotation=ROTATION_0` right after an app restart in portrait. The popup group of the copy has no children (the owner's layout has none), so it shows an empty popup that fits the screen.
- DONE landscape (7x4 copy `cfg-blur`, glass): the phone turns to `ROTATION_90`, `.tmp\phone-landscape-glass.png`: grid, widgets and keys as before. Note: the first landscape try with the 3x8 layout showed `layout.json invalid: Grid 3x8 does not fit landscape` (expected validation, my test edit, not a bug).
- fps on S9 (`gfxinfo`, 4 laps of swipes, p50 frame): portrait glass 34 ms (~29 fps, janky 97%), amoled 14 ms (~60+), studio 19 ms (~53), material 11 ms (~60+); landscape glass with the default aurora wallpaper 133 ms (janky 100%). Glass on the S9 stays below the 45 fps target in normal mode; the mode picked automatically was not read from the app.
- PENDING (owner): T14.5a items not covered here (pairing by QR, T7.1/T7.2/T7.4/T7.5 on the S9, labels at ~80 px, `Ctrl+Z` after `Rotate`, safe-area padding on a gesture-bar phone).
- DONE (S9): `release\Decka.apk` 0.2.0 installed, phone menu shows "App 0.2.0 · PC 0.2.0" (agent 0.2.0 on the `cfg-s9` copy). Release https://github.com/iqonkz/decka/releases/tag/v0.2.0 published; both README links answer 200.
- PENDING (owner): install from release v0.2.0 on a clean PC and phone; More -> About in the installed agent 0.2.0 (the update check against the now public repo was not looked at in the window).

## T15.2 — sound, player, sequences

- DONE (headless without `--dry-run`, copy of the config in `.tmp\cfg-t152`): `ws-smoke press mic-toggle` twice gave `mic.muted` true, then false in `ws-smoke watch` (the microphone is back as it was); the first `vars` patch carried `audio.volume`, `audio.muted`, `mic.muted`, `media.playing`.
- PENDING (owner): `shuffle` / `repeat` on a real player (Spotify, a browser tab): the key changes the player and `media.shuffle` / `media.repeat` follow; with a player that does not support it the key shows the error `The player does not support this`. Not pressed here, to leave the owner's player as it was.
- PENDING (owner): `out_mute_toggle` (`out-mute`): the sound goes off and `audio.muted` becomes true; `audio.volume` follows the system volume slider within about a second.


## T15.4 — library, sets, sequence editor

- DONE (real Tauri window, `npm run tauri dev` with `DECKA_CONFIG_DIR=.tmp\cfg-t154`, WebView2 driven over CDP): Library tab with categories, Sets tab with cards; `Add` on Media remote, Sound and Windows tools put 20 keys on an empty page and saved `actions.json` / `layout.json` of the copy; the sequence editor made a sequence of 2 actions and a pause and a key for it. Snapshots `.tmp\design\m15-library.png`, `m15-sets.png`, `m15-sets-added.png`, `m15-sequence.png`.
- PENDING (owner): drag a library item / set card onto a cell with a real mouse (only `Add` and double-click were driven); the ask dialogs (website, shortcut, page, group); the first-run guide cards (`Media remote` / `Sound` ticked, `Streaming` only with OBS found); the cell picker categories.
- PENDING (owner): the OBS items (`Set up OBS` until T15.6) and `Set up weather` (until T15.5) lead to More -> Integrations.

## T15.5 — weather

- DONE (real Tauri window with `DECKA_CONFIG_DIR=.tmp\cfg-t154`, real Open-Meteo): searched "Almaty", picked it, switched weather on, `Refresh now` answered `Updated.`; the weather widget placed from the library showed -5° / 1° and the place on the phone page (web client, 740x360). Snapshots `.tmp\design\m15-weather-settings.png`, `m15-weather.png`.
- PENDING (owner): `°F` units on the phone, and the widget in 3x2 / 4x2 with three days (not placed here).

## T15.6 — OBS Studio

- Not checked against a real OBS (not installed on the executor's PC); the protocol is covered by a stand-in server in `agent/core/tests/obs.rs`.
- PENDING (owner): OBS -> Tools -> WebSocket Server Settings -> Enable; Decka -> More -> Integrations -> OBS Studio -> switch on, `Set password…`; the status line shows `Connected`; Library -> Sets -> Streaming -> `Add`; from the phone: Record starts a recording (the key turns red, `REC` runs in the widget), a scene key switches the scene, with a wrong password the status says `Wrong password` and a notification appears once.

## T15.7 — M15 acceptance

- DONE (web client in headless Chrome 740x360 against a stand-in agent with the M15 state; not the phone): the Streaming set keys, volume, timer, weather, OBS and the graph on one page in 4 themes `.tmp\design\m15-final-<theme>.png`, portrait 4x7 `m15-portrait.png` (the volume slider turns vertical), Russian library `m15-ru-library.png` (the settings window runs in Russian here).
- PENDING (owner): fps on the A57 for a page with volume + sys graph + obs (not lower than the same theme without them minus 10 %): no A57 was connected, and the S9 stayed locked (PIN), so no `gfxinfo` table was made.
- PENDING (owner): `release:agent`, `release:apk` and `adb install` were not run; the same check on a real phone (T14.5a items stay open for the same reason: S9 locked).

## T15.8 / T15.10 / T15.9 (final run, 0.3.0)
- DONE: T15.8 — `fitWidgetSize`, dimmed keys only for reserved variables (never in the editor preview), `previewSet` and the set drag shadow, drag of sequence steps: all unit-tested; the set also lands where it is dropped (`InsertContext.at`).
- DONE: T15.10 — `tools/render-logo.mjs` renders every PNG from `docs/assets/*.svg` in headless Chrome (alpha checked: the tile corners are transparent); Tauri icon set, Android adaptive icon (gradient background, glyph foreground + monochrome), legacy icons, splash, favicon, manifest icons, UI, README, NOTICE; the placeholder (`make-icon.ps1`, `icon-source.png`) removed by `git rm`.
- PENDING (owner): the logo in the real window and the tray at 16 px (not clicked or photographed from the session); the set drag shadow and the sequence step drag with a real mouse in the Tauri window (`m15-set-drag.png`).
- PENDING (owner): the launcher icon on the phone (`phone-launcher.png`), the themed (monochrome) icon on Android 13+, the splash, the pairing screen with the logo on the phone (72 px logo plus the form on a 360 px high screen: check that nothing is pushed off).
- PENDING (owner): T15.9 was skipped: the S9 (`232512e0080c7ece`) is connected but locked (`mWakefulness=Dozing`, lock screen), no A57. Everything listed above for M15 and T14.5a stays open: the volume slider and `Mic` by finger, `phone-m15-*.png`, the fps table per theme.

## Twitch (owner request, 2026-11-17)
- DONE (tests against a local stand-in of Twitch, no account): sign-in by code, polling, refresh of an expired token, clip, announcement, sign-out; no token in the section, the window or `config.json` in plain text; action and widget rules; the library item that asks for the text.
- PENDING (owner): register an application on dev.twitch.tv/console (Client Type: Public), paste the Client ID in More → Integrations → Twitch, `Sign in`, type the code on twitch.tv/activate: the channel name shows as signed in. Needs the real Twitch (not run from the session).
- PENDING (owner): the Twitch widget on the phone while live and offline (viewers, followers, title, game, time on air), `Clip` while live (and the message when offline), `Chat post` with a text, the red dot, the four themes.
- PENDING (owner): that Twitch accepts `scopes` in the device flow for a public client as built (`moderator:read:followers clips:edit moderator:manage:announcements`).

## YouTube (owner request, 2026-11-17)
- DONE (tests against a local stand-in of Google and YouTube, no account): the client secret (DPAPI), sign-in by code, polling (channel, live broadcast, viewers), refresh of an expired token, a chat message, sign-out; neither the secret nor a token in the section, the window or `config.json` in plain text; action and widget rules; the library item that asks for the text.
- PENDING (owner): make the Google project and the OAuth client of the type "TVs and Limited Input devices", paste the Client ID, `Set client secret…`, `Sign in`, type the code on google.com/device: the channel title shows as signed in. Needs the real Google (not run from the session). Google may show the application as unverified; add yourself as a test user in the consent screen.
- PENDING (owner): the YouTube widget on the phone live and offline, `YT chat` while live (and the message when not live), the quota (one look a minute is about 4 300 units in a day of continuous live stream).

## M16 — AI limits and approvals
- DONE (tests, no real Claude Code): statusline parsing, summaries per tool, permission allow / deny / pc / timeout / no phone / hang-up / second answer, loopback and hook-token checks, Codex fixtures (new and old format), install and remove on a temporary home (foreign keys, hooks and a foreign status line stay, one `.decka-bak`), `burnEta`, the permission sheet in the store.
- PENDING (owner): `Install` in More → Integrations → AI coding, restart Claude Code: the line `Decka · 5h …` shows in the terminal and the limits appear in the AI limits widget on the phone.
- PENDING (owner): a `Bash` request in Claude Code: the sheet opens on the phone (also with the screen dimmed); `Allow`, `Deny`, `Answer on PC` and the wait running out (60 s is easiest to try) all behave; `Allow` and `Deny` do not react in the first 0.4 s.
- PENDING (owner): with no phone connected the question shows in the terminal at once.
- PENDING (owner): Codex: the limits show in the widget (needs a Codex session with a `rate_limits` line).
- PENDING (owner): `Remove` brings back `%USERPROFILE%\.claude\settings.json` (compare with `settings.json.decka-bak`); a foreign status line gives the warning line.
- PENDING (owner): the widget at 2×1, 2×2, 3×2 and 4×2 in the four looks on the phone, and the forecast line after about 10 minutes of use.
- DONE (tests): the Codex hook in `.codex\hooks.json` (foreign hooks stay, one `.decka-bak`, a file made only for Decka goes away with it); the format of the hook is from the official Codex docs (command hook, `timeout` in seconds, same decision JSON as Claude Code).
- PENDING (owner): Codex approvals: after `Install` open Codex, trust the hook with `/hooks`, ask for a command that needs approval: the sheet shows `Codex wants to use …` on the phone; `Allow`/`Deny` reach Codex; `Approve Codex on phone` off removes the hook.
- PENDING (owner): the installer's firewall rule (`installer/hooks.nsh`): the UAC prompt appears once at the end of the installation, `netsh advfirewall firewall show rule name=Decka` then lists the rule for private and public; a refused prompt does not stop the installation; the uninstaller removes the rule. Then pairing over a PC hotspot and over a phone hotspot on a PC that was blocked before.
- PENDING (owner): `curl.exe` as the hook command under Codex on Windows (quotes, stdin) — not run against a real Codex.
## Since 0.6.0 (not released yet)
- DONE (tests): the connection check (statuses, firewall verdicts for Allow / Block / missing profile, the reach test against a real local port); the real PowerShell probe was run once on this PC (`cargo test -p decka-core --lib diag -- --ignored`).
- PENDING (owner): Phone tab → Check the connection on the second PC: the lines are right; with the installer's rule removed the check says which network is not covered and `Fix the firewall` (UAC) repairs it.
- DONE (tests): the rules of Always allow (what is offered, what a rule covers, the history order and its limit, a rule answers with no phone, removing a rule brings the question back).
- PENDING (owner): with a real Claude Code / Codex: `Always allow: Bash: git status` appears for a plain command and not for `git status && …`; after it the same command is not asked again; the rule shows in AI coding and `Remove` brings the question back; tapping the AI limits widget opens the list of recent questions (the colours of ✓ ✗ ↩ ⚡ on the phone).
- DONE (tests): the app keys widget helpers, the eight new page templates are valid layouts, follower alerts (first look only sets the mark), YouTube go live / end against the local fake.
- PENDING (owner): the shortcuts of the new templates (Outlook, Word, Excel, PowerPoint, Canva, Telegram, WhatsApp, Discord) were written from memory of each program's documentation and not tried in the programs; the exe names (`olk.exe`, `whatsapp.root.exe`, `canva.exe`) depend on how the program was installed.
- PENDING (owner): the App keys widget on the phone: bring a program that has a page to the front and the keys of that page fill the widget; a program without a page shows the hint and its exe name.
- PENDING (owner): the installer pictures at 125% / 150% scaling look sharp; a new follower on Twitch rings on the phone (needs a live channel); YT go live / YT end with a stream scheduled in YouTube Studio.- PENDING (owner): `Deny and say` buttons under a permission question (three built-in texts, editable in AI coding): the chosen text appears as Claude Code's reason for the denial; Codex likewise.
- PENDING (owner): a page with hours of the day (Pages → the page's card → Show at certain hours): the phone opens it inside the hours when no program page applies, and goes home after; hours across midnight.
- PENDING (owner): the banner in Pages (`X is in front and has no page`) appears for a program with a template and adds the page in one click.
- PENDING (owner): the phone's `Cannot reach the PC` screen with the three hints after about 8 seconds (stop the agent to see it).
- PENDING (owner): Teams and Chrome/Edge keys (Teams `ms-teams.exe`; shortcuts from Microsoft's own table).

## M17 — owner's list 2026-11-19
- DONE (tests): the collectors a layout needs (`needed_collectors`), `patch sys` keeps the references of the data of the keys, the shortcut state functions, the system operations and their programs, the app strip (`strip_for`, a strip key runs only for the strip shown), `orientationLock` for the six combinations, the battery list (dedupe by name, PC first, ≤ 8, variables, device icon by name), IMAP (quoting, `STATUS`, `NO`/`BAD`, a dialog on text), the Windows toast texts from a `ToastGeneric` fixture, the exception list, `notif_open` with an unknown id launches nothing.
- DONE (measured): the tree of `decka-agent.exe` with the settings window closed: 235.8 MB private before (agent 21 MB + 6 WebView2 processes), 13.8 MB after (debug build, own copy on port 19247, window not created because of `--minimized`).
- DONE (probe, 2026-11-19): `UserNotificationListener` works in an unpackaged exe (`Current()` ok, access `Allowed`, `GetNotificationsAsync(Toast)` answered; only the count was read, no text).
- PENDING (owner): the settings window opens from the tray in ≤ 1.5 s after being closed, the tab and scroll come back, `decka://` and a second launch of the exe also open it; ten seconds after closing, Task Manager shows no `msedgewebview2.exe` under Decka.
- PENDING (owner): swiping between pages on the phone is smooth (no jerk at the end of the settle), also with the Glass theme.
- PENDING (owner): the shortcut field: `Win+D`, `Alt+Tab`, `Win+Shift+S`, `Ctrl+Alt+Delete` chosen with the switches and the key list; a lone `Win`; without a key `Pick a key` and nothing is saved.
- PENDING (owner): the new system keys on a PC you can afford to restart: lock/sleep (as before), hibernate (with hibernation off: `Hibernation is turned off in Windows`), display off, empty the recycle bin (also when it is empty), Task Manager, Settings, Explorer; shut down, restart and sign out only when ready for it.
- PENDING (owner): the app strip on the phone: bring Word (no page of its own) to the front: the strip slides out at the right edge (bottom in portrait) in 220 ms; a tap outside folds it to the tab and does not press the key under the finger; `⋯` → Only the tab / Never; the phone menu lists the programs with `Never` and `Show again`.
- PENDING (owner): `Turn` in the phone menu (Auto / Normal / Upside down) on a real phone, in landscape and portrait.
- PENDING (owner): the Battery widget at 1×1, 2×1, 2×2, 3×1 with a Bluetooth headset or keyboard that reports its charge (a laptop for the PC line); a device without a charge is not shown.
- PENDING (owner): Mail: a mailbox with an app password (Gmail, Yandex, Mail.ru, iCloud) shows its unread count in the widget within two minutes and at once after `Check now`; a wrong password shows `Sign-in failed` and is not tried again until a new password is set; a tap on the row opens the page chosen as the open action. Not run against any real mailbox from the session.
- PENDING (owner): Windows notifications: switch on in More → Integrations → Windows notifications (Windows asks its own question the first time), a new toast shows on the phone as a 4 s toast (or in the Notifications widget), the exception list hides an app, a tap opens the program, a toast read in Windows goes from the phone.
## M18 — security (2026-11-20)
- DONE (tests): the vectors of `shared/fixtures/crypto-v2.json` (`K_dev`, `S_code`, the three `okm`, both MACs, frames with counter 0 and 1) in Rust and TS; a wrong MAC → 4401; a forged `auth_ok` ends the client; a replayed, skipped or changed frame → 4400 (both sides); plain text after the handshake → 4400; `hello` v1 from another address → 4426, from loopback works; `pair_qr` once, `pair_code`, then device login; an old `devices.json` with `tokenHash` logs in with the old token; `policy(category, request)` for all combinations and the end-to-end run with a forced category; `allowPublic: false` closes at once; 5 handshakes a minute on a public network; `perm_answer` of a phone without the right is ignored; `allow_always` for a whole tool is a single allow; a phone rule belongs to its session and ends after 8 h; shutdown 10 s after connecting and a second one 5 s later are refused; a messenger notification goes out without its text; plain secrets in `config.json` are encrypted at load; no secret in the log after pairing, login, `perm_answer`, OBS and mail.
- DONE (live, headless): `ws-smoke` pairs by code, reads the state and presses over v2; `devices.json` holds no token.
- PENDING (owner): update the phone and the PC to this version together. A phone with 0.8.x shows `Update the Decka app to continue.` and does not connect; after updating, the paired phone connects **without** pairing again.
- PENDING (owner): pairing by the QR code of the Decka window on the phone, and by typing the code (More options is not needed: the code is under the QR). Both work on the home Wi-Fi.
- PENDING (owner): the screen `This is not your PC`: delete `devices.json` on the PC (or reinstall), restart the agent: the old phone shows the screen with `Retry` and `Pair again` and does not try again by itself.
- PENDING (owner): a public network. Set the hotspot of the phone (or any network) as `Public` in Windows. The Phone page says `Public network — encrypted, only the Decka app, pairing by QR`; the address in a browser gives a blank 404; the app connects (already paired) and pairs by QR; a typed code says `On a public network pair with the QR code or USB`. `Allow phones on public networks` off: the app does not connect on this network at all (and does on a private one).
- PENDING (owner): a new phone cannot answer Claude Code until `Can answer Claude Code` is on for it (the Phone page, the list of phones): with it off the question stays in the terminal. A phone paired before this version has it on.
- PENDING (owner): a dangerous Claude Code command (for example `rm -rf build`): `Allow` on the phone is a 1 s hold with a ring, a tap says `Hold to allow`, no `Always allow`. `Always allow` of a safe command from the phone: the rule shows in More → Integrations → AI coding → Rules as `from Phone <name>, until <time>`; it does not apply in another Claude Code session; it is gone after 8 hours.
- PENDING (owner, a PC you can afford to restart): a `shutdown` key pressed in the first minute after the phone connected answers `Too soon after connecting`; two presses in a row answer `Wait a little before trying again`; the log line is `system shutdown by <phone>`.
- PENDING (owner): a message in Telegram/Outlook/Teams shows on the phone as `New notification` + the program name, without text; More → Integrations → Windows notifications: `Full text` for that program brings the text back; `Don't show` hides it.
- PENDING (owner): the release has `SHA256SUMS.txt`; `Get-FileHash` of the installer and the APK equal their lines.

## T17.8 — library panel (2026-11-21)
- DONE (screenshots, Chrome with a mock IPC, `node tools/ui-shots.mjs all`): `ui-library-ru.png`, `ui-sets-ru.png`, `ui-apps-ru.png` at 1000 px and `ui-library-ru-min.png` at 760 px: nothing cut off, the head does not scroll, the segments fit, rows show `+` on hover.
- PENDING (owner): in the real Tauri window (WebView2): the thin scrollbar, `+` on a row adds the key to the first free cell (`No free cell on this page` when the page is full), `Esc` clears the search, `↓` goes from the field to the list, `Enter` on a row adds it.

## M19 — 11 languages (2026-11-21)
- DONE (tests): every dictionary has every key of `en.ts`, no empty text, the same `{placeholders}`, at most 20% of the texts equal to English (names excepted); plural forms for every language by `Intl.PluralRules` (Arabic has all six); `auto`: `zh-TW`→`zh`, `pt-PT`→`pt`, `kk-KZ`→`kk`, `sw`→`en`; every text of the catalog, the sets and the template captions in all 11 languages (labels ≤ 12 characters); tray texts and `GetUserDefaultUILanguage` ids; `display.language` accepts the 11 codes.
- DONE (screenshots, Chrome with a mock IPC, `node tools/ui-shots.mjs all`): `.tmp\shots\i18n-de.png`, `i18n-ar.png`, `i18n-kk.png`, `i18n-ja.png` (and `i18n-de-more.png`, `i18n-ar-more.png`): Arabic mirrors the window but not the grid; no tofu in Chinese/Japanese/Kazakh; long German names are cut with an ellipsis.
- PENDING (owner): Kazakh and Russian read by a native speaker; the other nine are machine translations.
- PENDING (owner): on the phone: switch the language in the window, the phone follows within a second; Arabic: the menu and the sheets mirror, the grid and the page swipe do not; Hindi, Chinese and Japanese show real letters on the Galaxy (system fonts of Android).
- PENDING (owner): the tray menu in a Windows with another UI language (`auto`), for example German.

## M20 — flip, limits widget, one-click updates (2026-11-21)
- PENDING (owner): T20.1 on the phone: with `Turn → Auto`, put it on its other side (charging port up, then down): the screen flips within a second and stays; the layout orientation (portrait/landscape) never changes. With a phone on USB: `adb shell dumpsys window | findstr mCurrentAppOrientation` changes between `0` and `8` (landscape). No phone was connected when this was written.
- DONE (screenshots, headless Chrome, `node tools/ai-shots.mjs`): `.tmp\shots\ai-2x1.png`, `ai-2x2.png`, `ai-4x2.png` (AMOLED) and `ai-2x2-material.png` on an 800x360 phone-sized deck: no cut text, nothing below 11 px; long session names and the `ctx` part are left out when they do not fit.
- PENDING (owner): the limits widget on the phone at 2x1, 2x2, 3x2 and 4x2 (ring colours at 70% / 90%, the `↻` and `⚠` lines, the details sheet on a tap).
- PENDING (owner): one-click update of the agent, on the NEXT release after 0.11.0 (0.11.1): More → About → `Update to 0.11.1` downloads, installs passively and restarts the agent. The update key is in `%USERPROFILE%\.decka-signing\updater.key` (password under DPAPI in `updater.pass`); back both up: without them installed agents cannot take later updates.
- PENDING (owner): update of the phone app from the PC, also on 0.11.1: the phone menu shows `Update Decka to 0.11.1`, the progress, then Android's installer (the first time: allow installing from Decka). The pairing stays because the APK has the same signature.

## M21 — voice control, PC side (C2, 2026-11-22)
Everything below needs a PC with a microphone and the voice models installed (`More → Voice → Turn on`); nothing of it could run without them.
- PENDING (owner): hotkey from the PC. With voice control `Ready`, press `Ctrl+Alt+Space` in any program: the pill appears at the top centre of the screen with the mouse cursor and says `Listening…`. Press the hotkey again: the recording ends. Change the hotkey in the window (for example `Ctrl+F9`): the old one stops working at once, the new one works without a restart. Turn voice control off: the hotkey does nothing.
- PENDING (owner): busy hotkey. Set a chord another program uses (for example one of a screenshot tool): the window shows an error under the hotkey (`The hotkey … is used by another program`), voice control still works from the phone; after choosing a free chord the error goes away.
- PENDING (owner): recording. Say `volume up` (the volume rises), `open calculator` (Calculator opens), `lock the computer` (see the confirmation below). Stay silent after speaking: the recording ends by itself after about a second. Speak for more than 30 s: it is cut and recognised. Press `Esc` while listening: the pill goes away, nothing is run. The first request after a pause shows `Loading…` while the models start; a later one is quick. The wave of the pill follows the voice.
- PENDING (owner): the pill does not steal the focus. Type in Notepad and start voice control with the hotkey: the cursor stays in Notepad, the typed text is not interrupted, the title bar of Notepad does not turn grey. Click `Yes` / `No` on the pill with the mouse: the focus stays where it was. The pill is not in the taskbar and not in `Alt+Tab`. It is gone about 4 s after the result (look in Task Manager: no extra `msedgewebview2` processes after that).
- PENDING (owner): confirmation of `lock`. Say `lock the computer`: the pill shows `Lock the computer` with `Yes (Enter)` and `No (Esc)`. `Enter` or a spoken `yes` locks Windows; `Esc`, `No` or waiting 10 s cancels (`Cancelled`) and the computer stays unlocked. After the answer `Enter` and `Esc` work normally again in other programs (they are caught only while the pill is asking).
- PENDING (owner): voice from the phone. With the phone paired, hold the microphone button and say `volume down`: the PC volume falls, the phone shows the result (in the language of the phone). A confirmation (`lock`) appears as a sheet on the phone; `Yes` on the phone locks, and the pill does NOT appear on the PC (the request is the phone's). Pressing the hotkey on the PC while the phone records is refused without breaking the phone's recording.
- PENDING (owner): no microphone. Disable the microphone in Windows (`Settings → Privacy → Microphone` off, or unplug the headset and choose it in the window first): the hotkey shows the pill with `No microphone found`, then it goes away; the hotkey keeps working. Choose a microphone in the window that is later unplugged: the default one is used.
- PENDING (owner): language of the texts. Switch the language of the window to Russian: `Didn't catch that` on the pill becomes `Не расслышал`, the lock question is `Заблокировать компьютер`; the phone shows the same words in its own language.
- PENDING (owner): tools without an action of their own (C3, 2026-11-22). Say each with the PC hotkey: `set volume to 30 percent` (the volume of Windows becomes 30 and an unmuted state); `switch to Chrome` with Chrome minimised (it comes to the front); `close Notepad` with an unsaved Notepad (Notepad asks to save, nothing is lost); `move this window to the other monitor` on two monitors (the window keeps its relative place; a maximised window stays maximised) and on one monitor (`There is only one monitor`); `which monitor is this` (`Monitor 1 of 2`); `set a timer for 1 minute` (a notification `Timer · 1 min` on the PC and a toast on the phone after a minute); `start dictation`, then click into Notepad and say a phrase that is also a command, e.g. `volume up` (the text `volume up` is typed, the volume does not change); `go to page <name of a page>` with a phone connected (the phone swipes to that page).
- PENDING (owner): AI path (C4). With the models installed, say a phrase that is not in the table, e.g. `make it a bit quieter, I'm on a call` (volume down) or `pull up the weather in Almaty` (a web search). The first request after a pause shows `Loading the model…`; a nonsense phrase ends with `Didn't get that — try again` and nothing happens.
- PENDING (owner): phone voice (C5). On the phone hold the microphone button, say `turn the volume up` (the same result as from the PC); first use asks for the microphone permission (refuse it once: the hint `Allow the microphone for Decka in Android settings`); say `lock the computer` and answer the sheet with Yes / No; turn Wi-Fi off in the middle of a phrase (nothing happens on the PC, the button returns to idle).
- PENDING (owner): 0.20.1 voice, tap and silence (T21.8). With voice control `Ready`: (1) press `Ctrl+Alt+Space`, say `volume up`, stay silent: the wave moves while you speak (the bars lie low in silence), about 2 s after the last word the pill goes to `Thinking…` and the command runs by itself; (2) press the hotkey and stay silent: after 5 s `Didn't hear anything`; (3) press the hotkey, say `write hello` / `type hello` with Notepad in focus, then click the stop button (or anywhere on the pill) at once: the recording ends, `hello` is typed into Notepad (the focus stays in Notepad); (4) on the phone TAP the microphone button (no hold): it listens, the icon becomes `stop`, the ring follows the voice, silence for 2 s ends it by itself, a second tap ends it at once; (5) hold the button 250 ms or more: walkie-talkie, release ends it, silence does not; (6) drag the button more than 8 px: it moves and snaps to the edge, no voice starts; (7) hold the button for a long time: no context menu, no text selection, no long-press sheet of the page; (8) unplug or mute the microphone: `Microphone is unavailable`; (9) the pill never stays in `Listening…` more than 30 s (it ends with `Something went wrong, try again`).- PENDING (owner): 0.20.3 voice on the PC and the phone (T21.11). (1) `Ctrl+Alt+Space`, say `open calculator`: the pill stays, the wave moves, the calculator opens (with the slow recognition of M21-4 it can take about 20 s); (2) on the phone hold a key: no context menu, no text selection; (3) say a phrase on the phone and check `%LOCALAPPDATA%\Decka\logs\voice.log`: `chunk ...` lines and `event AudioEnd` appear (the voice_chunk of the phone reach the PC).

- PENDING (owner): T22.10 smart dictation. Voice control Ready; Notepad in focus: (1) press and release Ctrl twice quickly: the pill listens, say "ну привет как бы я завтра приду э-э позже": about 2 s after the last word the cleaned text ("Привет, я завтра приду позже.") is typed; (2) Ctrl+C and Ctrl+V do NOT start it; (3) one tap of Ctrl while it listens ends the recording, Esc cancels; (4) dictate more than 300 characters: the text arrives through the clipboard and your earlier clipboard text is back after a second. The trigger (double Ctrl / a chord / off), the mode (clean / raw) and the dictionary have no controls in the window yet: they are read from dictation.json in the configuration folder (UI follow-up, see BLOCKERS).
- PENDING (owner): T22.11 selected text. In Notepad type "i has a apple", select it, press the voice hotkey and say "fix the errors": the selection becomes "I have an apple." and your earlier clipboard text is back; "translate to German" replaces with a German text; "summarize" / "explain" leave the text and show the answer on the pill and the phone; with nothing selected: "Select text first"; Ctrl+Z in Notepad undoes a replacement. The model chooses the tool from the phrase; phrases for the fast path are the job of T22.11b (second agent).
- PENDING (owner): T22.12 help with the screen. Checked without running a model (2026-11-25): the Qwen3.5 repositories (unsloth/Qwen3.5-4B-GGUF and -2B-GGUF) have mmproj-F16.gguf under Apache-2.0 (672 MB / 668 MB, SHA-256 in shared/ai/manifest.json), and llama-server b12602 has --mmproj. Live: open a window with an error message, press the voice hotkey, say "what does this error mean" (the model chooses screen_ask): the first time the pill asks "Download the Screen help package (about 0.7 GB)?"; Yes downloads it (no progress bar yet: the pill stays in Thinking for a few minutes), restarts the model server and answers in at most 120 words (cut to 300 characters on the pill). Nothing of the screen is saved: check that %LOCALAPPDATA%\Decka has no new picture and voice.log has no text of the window. The icon of a camera in the pill ("Looking at the screen") is not drawn yet.
- PENDING (owner): T22.13 reminders and lists. Say "remind me in 2 minutes to check the oven": a toast, a line in the notification strip of the phone and the pill after 2 minutes (the reminder fires within 20 s of its time); "remind me tomorrow at nine to pay rent" then "what are my reminders" lists both; "add milk to the shopping list" (the list is made on the first use), "what is on the shopping list", "remove milk from the shopping list"; "delete the shopping list" asks first. Close Decka, wait past a reminder, start Decka: one notification Missed: .... Files: reminders.json and lists.json in the configuration folder. The widgets of the phone (reminders, list) and the window More -> Reminders are not built yet: see BLOCKERS 2026-11-25.
- PENDING (owner): T22.14 brief of the day. With weather and mail on and a reminder for today, say "good morning" / "what do I have today" (the model chooses day_brief): the pill shows 3-5 sentences with only your own numbers and names; if the model writes anything that is not in the data, the plain facts are shown instead; with nothing to tell: "Nothing to report". The sheet on the phone (cards by section) and the setting Morning brief on the phone at the first unlock of the day are not built yet: see BLOCKERS 2026-11-25 (T22.14).
- PENDING (owner): T22.15 memory. Say "remember that Aigerim is my wife" ("Remembered: ..."), then "write to Aigerim" with a field in focus: the model uses the name from memory; "forget Aigerim" asks first and then "Forgot: ..."; a phrase without "remember" never adds anything. The file is memory.json in the configuration folder (at most 200 facts, no phrases are recorded). The list More -> Voice -> Memory is not built yet: see BLOCKERS 2026-11-25 (T22.15).
- PENDING (owner): T22.16 phone widgets (2026-11-25). In the editor add `Reminders` and `List` (Library, 2x2 by default, 2x1 smallest; the list has a drop-down of the names of your lists). Say "add milk to the shopping list" and "remind me in 10 minutes ...": both appear on the phone within a second. Tap a row of the list: the tick shows at once, the item goes to the bottom, struck through at half opacity; tap again: it comes back; with the agent stopped the tick rolls back on the next patch. A deleted list shows `List not found`, an empty one `List is empty`, no reminders `No reminders`. Rows are cut to one line with an ellipsis; the time is `HH:mm` today, otherwise `Wed 09:00` (the weekday in the language of the phone).
- PENDING (owner): T22.17 PC windows and pill (2026-11-25). `More -> Reminders`: add a reminder with a date-time and text (a past time is refused), edit the text in place (click, Enter), change the time, delete with the bin; `Show done` shows finished ones; `Lists`: new list, tick, add and delete items, rename, delete a list (asks first). `More -> Voice`: `Dictation` (Double Ctrl / Shortcut with the key field / Off, Clean / Raw, the dictionary one phrase per line) takes effect at once (the trigger is re-registered; check that Off stops the double Ctrl); `Memory` lists the facts with `N / 200`, edit in the row, bin, `Forget all` asks first. The pill says `Dictating...` with a microphone while a dictation listens, and `Looking at the screen` with an eye while the screen help works; a due reminder shows its text on the pill for 5 s (not while a voice request runs). The harness pictures (`node tools/voice-shots.mjs`, scenes `m22-*`) show blank bins because the harness does not serve the Phosphor files; the app does.

- PENDING (owner): 0.23 phone widgets and screens (T23.5b / T23.R3). Pair the phone and look at the same page in each of the four themes (AMOLED, Studio, Material, Glass), landscape and portrait: widgets share one look (a small header, the same corners and padding), the empty and waiting states are quiet (`WidgetEmpty`, skeletons), numbers move smoothly, the cover colour of `Now playing` tints its tile and never makes the text unreadable. The `Keys` editor of the window shows the same widgets in its preview.
- PENDING (owner): 0.23 hourly weather (T24.1). With weather on, the `weather` widget in a size with room for it shows the next six hours (`HH:mm`, icon, temperature) starting with the next full hour; around midnight the list continues into the next day; with the PC offline for 3 hours the widget goes empty as before. Say `good morning` when rain or snow is forecast within six hours: the brief says `Rain expected around HH:mm` (only a time that is in the data).
- PENDING (owner): 0.23 lyrics (T24.2, off by default). (1) With `Integrations -> Lyrics` OFF play a song: nothing is sent (check the Windows firewall log or Resource Monitor: no connection to lrclib.net). (2) Switch it ON (the card says that the artist and the title go to lrclib.net): the first request goes out when the next song starts, not before; the text appears on the large `Now playing` widget (4x2 and up) with the current line bright and the neighbours dim; a song from YouTube in the browser (`Artist - Title (Official Video)`) is found too. (3) Long press the text: the pills `-0.5 s` / `+0.5 s` appear for 4 s, tap them: the lines move, the shift is kept for this song (play it again tomorrow); a video that is longer than the record by more than 3 s shows the text without a highlight. (4) Switch it OFF: the text disappears from the phone at once. Files: `%LOCALAPPDATA%\Decka\lyrics\` (cache, at most 500 files), `lyrics_offsets.json`.
- PENDING (owner): 0.23 recurring reminders (T24.4). Say `remind me every day at 9 am to take pills`, `напомни по будням в 8:30 вставать`, `remind me every Monday at 10 am to plan the week`: each is one line in `More -> Reminders` with its next time; when it fires the line stays and moves to the next time (a daily one tomorrow, weekdays skip Saturday and Sunday, a weekly one a week later). Delete the line: the whole series goes. Close Decka for a day or more, start it: one `Missed` notification per series, not one per day. The window's time panel (`Once · Daily · Weekdays · Weekly`) and the repeat icon in the row are the follow-up of the second agent if they are not there yet.
- PENDING (owner): 0.23 keyboard in the settings window (T24.3b). Press Tab from the first element through every page (Keys, Look, Phone, Voice, Reminders, Integrations, Actions, Settings, About): the focus is always visible, never lost in a hidden element, Enter / Space press buttons and switches, Esc closes dialogs and pickers, the search box opens with its shortcut and a result is chosen with the arrows and Enter (the page scrolls to the setting and highlights it for about a second).
- PENDING (owner): 0.23 onboarding (T24.3b). Run `About -> Run setup again`: the new steps look like the rest of the window (light and dark), the phone is paired from the QR step, the apps step adds buttons, the voice step offers the download without starting it by itself.
- PENDING (owner): 0.23 calc and short answers (T23.B). With the AI pack: `54 умножить на 114` shows `54 × 114 = 6156`; `how many days in a leap year` is answered in a few words; `what is the bitcoin price today` asks `I can't know that offline, search the web?` and opens the search only after Yes; `напомни в 22.14 сделать дизайн` is saved for 22:14.
- PENDING (owner): 0.50 guide on Word "save as PDF" (T27.5, 4B and then 9B). Open a document, say `как сохранить как PDF` / `how do I save as PDF`: the island shows a card (goal, "Step 1", the hint, Back, Stop), a frame with a bubble lights the element, and after you press it the next step appears. Say `дальше` / `назад` / `стоп`. After a successful guide save it (`remember how to ...`); the second time it must go without the model (no "~N s").
- PENDING (owner): 0.50 guide in Windows Settings "dark theme" (T27.5, 4B and 9B). Same card; the steps lead Personalization -> Colors -> Choose your mode -> Dark. Partly checked by the agent (2026-11-26, 4B, `cargo run -p decka-core --example guide_demo`): the window was read through UI Automation (82 elements in 0.3 s) and the model chose a real element of the list as step one ("Personalization"); the loop itself (highlight window, waiting for the click, next steps) was not driven by hand.
- PENDING (owner): 0.50 guide in Chrome "clear browsing history" (T27.5, 4B and 9B). Chrome exposes its page to UI Automation only with accessibility on: if the list is empty the guide says it cannot see the buttons (the vision fallback is not built, see BLOCKERS).
- PENDING (owner): 0.50 find and press (T27.3/T27.4). `где сохранить` highlights the element; `не то` goes to the next one; `нажми это` presses it; `нажми на экране удалить` (a dangerous name) asks for a `да` first and presses nothing before it; `прокрути вниз`, `закрой вкладку` work in a browser.
- PENDING (owner): 0.50 highlight window (T27.2). The frame follows the element on a second monitor too, is click-through, and goes away on a click into it, on another window, on `стоп` and after 30 s.
- PENDING (owner): 0.50 read aloud (T27.9): select a paragraph, `прочитай вслух` reads it with the Windows voice; `стоп` cuts it; with nothing selected the text under the mouse is read.
- PENDING (owner): 0.50 work modes (T27.10): `запомни расклад как работа` with a few programs open, close them, `рабочий режим` starts and places them; the settings list shows, edits and deletes modes. The focus timer of a mode is not started yet (see BLOCKERS).
- PENDING (owner): 0.50 notices (T27.8): fill the memory/CPU or lower a Bluetooth device charge; a short note appears on the island, not twice within 30 minutes, not over a full-screen program; the switch in Settings -> Voice turns it off.
- PENDING (owner): 0.50 extended model (T27.7): on a PC with 16 GB RAM the settings offer "Extended"; the download size is shown first; after it the server restarts and the 4B file is removed.
