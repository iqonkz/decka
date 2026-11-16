# Changelog

All notable changes to Decka. The format follows [Keep a Changelog](https://keepachangelog.com/); versions follow [SemVer](https://semver.org/). One number covers the PC agent, the phone app and the APK.

## Unreleased

### Added
- OBS Studio: turn it on in More → Integrations → OBS Studio (enable the WebSocket server in OBS, set the password once; it is stored encrypted for your Windows user). Keys for record, go live, replay, scenes and audio input mute, and the OBS widget with `REC` / `LIVE` timers, follow OBS live.
- Weather: turn it on in More → Integrations → Weather, search for your city, pick °C or °F. The PC asks Open-Meteo (the phone never goes online; coordinates are rounded to about 1 km) and the weather widget shows now and three days ahead.
- A library of ready-made keys and sets in the editor (Windows tools, Media, Sound, Stream, Widgets, Navigation, Web, Keys) with search, in English and Russian. Add a key by dragging it onto a cell or double-clicking it; add a whole set (Media remote, Sound, Windows tools, Streaming, Focus) with one click. Keys that need an address, shortcut, OBS scene or page ask for it first. A sequence editor builds a chain of actions with pauses. The first-run guide offers the sets as cards. The cell picker uses the same categories.
- New widgets on the phone: a volume slider, a timer, weather, OBS status, and a CPU / RAM graph for the System widget. A new Back key returns to the page you came from. Keys that follow Decka's own state show it clearly: red for a muted microphone or a recording, the accent colour for shuffle, a blinking dot while recording or on air, and a dimmed key when there is no data (no OBS, no microphone). The widgets get their place in the settings window with the key library.
- New kinds of key actions: mute the microphone, mute the sound, shuffle and repeat of the current player, and a sequence of up to 10 steps with pauses (the buttons for them arrive with the new key library). Decka now follows the volume, the mutes and the player state, so keys can change their look with them.
- Icon picker: a new main tab **Apps & brands** with about 3,400 logos (Discord, Steam, Spotify, OBS, Twitch, Chrome and more); picking one also sets the key to the brand colour if it has none.

- A new Decka logo: in the Windows installer, the app and tray icon, the Android adaptive (and themed) icon, the launch screen, the pairing screen, the settings window and the README.
- Dragging a set over the deck shows a shadow of where it will land, and a hint when it will go to a new page. Steps of a sequence can be reordered by dragging.

### Changed
- Turning the deck to portrait or back keeps every widget at a size it supports (for example a wide weather widget becomes a smaller one instead of an invalid size). Keys dim only when the Decka state they follow is missing, never for your own variables or in the editor preview.
- Variables starting with `audio.`, `mic.`, `media.` and `obs.` now belong to Decka (volume, microphone, player, OBS state): scripts can no longer write them through the HTTP API (`400 reserved variable`), but keys can follow them. The default Mute key no longer needs its own toggle variable.
- Changing the orientation keeps the reading order: widgets first, then keys, each in the order they were on the page, so neighbours like Prev / Play / Next stay together instead of being shuffled.
- Key settings look tidier: two columns in a wide window (icon on the left, name, action and colour on the right), the icon picker opens on the quick set, its grid fills the width, the icon shows its name instead of a file id, rounder controls.

## 0.2.2 — 2026-11-16

### Changed
- Smoother page swipe on the phone: the page you swipe to is no longer rebuilt at the end of the swipe.
- A nicer Windows installer in the Glass style, with a choice of English or Russian.

## 0.2.1 — 2026-11-16

### Added
- **Cut** (right-click menu or `Ctrl+X`) on a key or widget: cut it, open another page and paste it there.

### Fixed
- The icon picker in the installed app failed with "Failed to fetch phosphor icons".

## 0.2.0 — 2026-11-16

### Added
- Russian interface (window, phone, tray menu); **More → Settings → Language** or the switch in the first-run guide. **Auto** follows the system language.
- One version number for the agent and the phone, shown in the phone menu (`App 0.1.0 · PC 0.1.0`) and in **More → About**.
- The agent can check GitHub once a day for a newer release and tells you in the window and the tray; it never downloads or installs anything. Switch it off in **More → About**.
- **Phone** page shows the app version of each paired phone and warns when it differs from the PC version.

- Portrait orientation: **Look → Orientation** turns the whole deck (the grid becomes 4×7 instead of 7×4, buttons keep their places relative to the phone, what does not fit goes to a new page); the phone opens in the chosen orientation at once. The grid picker, page templates and the first-run guide know about it.

### Changed
- Glass look: the wallpaper is blurred once in advance, so it looks smooth and runs faster; **Reduce effects** now shows frosted glass instead of a flat tint.
- Big-icon labels are easier to read on bright icons; icons of programs with small pictures (for example 7-Zip) fill the key.

## 0.1.0

- Turns an old Android phone into a desk panel for a Windows PC: a grid of buttons (launch a program, hotkey, website, media keys, lock and sleep) and live widgets (clock, CPU and memory, statuses, notifications, now playing).
- Phone only ever sends which button was pressed; the PC decides what runs.
- Wi-Fi or USB (`adb reverse`) connection, pairing by QR code or 8-character code.
- Four looks: AMOLED, Studio, Material and Glass, big-icon keys, popup groups, context pages that follow the program in front.
- Screen saving for the phone: dimming, pixel shift, a screen saver clock and a night mode.
- Webhooks: `POST /api/notify` and `POST /api/status`, plus built-in website checks.
- Editor in the PC window with drag and drop, quick picker, undo and redo, autosave.
