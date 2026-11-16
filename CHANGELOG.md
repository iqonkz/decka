# Changelog

All notable changes to Decka. The format follows [Keep a Changelog](https://keepachangelog.com/); versions follow [SemVer](https://semver.org/). One number covers the PC agent, the phone app and the APK.

## Unreleased

### Added
- Icon picker: a new main tab **Apps & brands** with about 3,400 logos (Discord, Steam, Spotify, OBS, Twitch, Chrome and more); picking one also sets the key to the brand colour if it has none.

### Changed
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
