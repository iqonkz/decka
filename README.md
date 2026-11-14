# Decka

## What is Decka

Decka turns an old Android phone into a desk panel for a Windows PC: a 5x3 grid of buttons that launch programs, send hotkeys, open URLs and control media, plus live widgets for PC state, service statuses and webhook notifications. A small agent runs in the Windows tray; the phone shows the panel and only ever tells the agent *which button* was pressed.

<!-- Screenshot: docs/img/hero.png -->

## Why

- **USB mode.** The phone can work over a cable with no Wi-Fi at all, through `adb reverse`. In "USB only" mode the agent listens on `127.0.0.1` and opens no port to your local network.
- **AMOLED burn-in protection.** Pure black background, slow pixel shift, dimming after inactivity, and a screen saver with a wandering clock. A night interval and a "PC offline" screen saver are built in too.
- **Webhook-first for developers.** `POST /api/notify` (even a plain-text one-liner with curl) and `POST /api/status` with a TTL give you deploy and AI-session statuses without plugins. Built-in HTTP checks watch your own services.

## Quick start

1. **Install the agent.** Run the NSIS installer from the releases page (or build it, see [Building from source](#building-from-source)). Decka starts in the tray; open the settings window from the tray icon.
2. **Install the app on the phone.** Copy `app-debug.apk` / the release APK to the phone and install it (or `adb install -r app-debug.apk`).
3. **Pair.** In the settings window open **Pairing** and press **Start pairing**; a QR code and an 8-character code appear for 120 seconds. In the app press **Scan QR code**, or type the address and code by hand.
4. Edit buttons in the **Actions** and **Layout** tabs. Changes appear on the phone within a second.

A browser also works for a quick try: open `http://<PC>:9247/` on the phone and paste the pairing details (no camera, no wake lock there).

## USB mode

1. Install Android platform-tools (`adb`) and put it in `PATH` (or set the path in **Settings**). Decka does not bundle or download adb.
2. On the phone: Settings → About phone → tap *Build number* 7 times; then Developer options → enable **USB debugging**.
3. Connect the cable and accept the "Allow USB debugging?" prompt on the phone.
4. In Decka **Settings** tick **USB mode**. Within about 10 seconds `adb reverse --list` shows `tcp:9247 tcp:9247`; the app connects through `127.0.0.1`.
5. For a cable-only setup choose **USB only (127.0.0.1)** and press **Restart now**.

Do not accept USB debugging prompts on computers you do not own.

## Sending notifications and statuses

Find your API token in the **Integrations** tab.

```powershell
curl.exe -s -X POST http://<PC>:9247/api/notify -H "Authorization: Bearer <TOKEN>" --data-binary "Build finished"
curl.exe -s -X POST http://<PC>:9247/api/status -H "Authorization: Bearer <TOKEN>" -H "Content-Type: application/json" --data-binary "@status.json"
```

More examples (PowerShell, deploy wrapper, Claude Code hooks, GitHub Actions, button state variables) are in [docs/RECIPES.md](docs/RECIPES.md).

## Security model

- The phone sends only an `actionId`. There is no message that carries a command, path, argument, key or URL, and extra fields are ignored.
- Actions live in `actions.json` on the PC; whoever can edit that file can already run code as you.
- Pairing uses a one-time 8-character code (40 bits), valid for 120 seconds, 5 wrong attempts destroy it, 10 attempts per minute per IP.
- Device tokens are stored on the PC only as SHA-256 hashes and can be revoked one by one; revoked devices are disconnected at once.
- The API token only allows adding notifications, statuses and variables, never running actions. It can be regenerated.
- WebSocket `Origin` is checked against a whitelist, the API needs an `Authorization` header (not a query string), and no CORS headers are sent.
- Rate limits and size limits apply to every endpoint; commands are started without a shell.
- **Traffic on Wi-Fi is not encrypted.** Someone on the same network can capture a token. Use USB mode or `127.0.0.1` binding on untrusted networks. TLS is planned after the MVP.

## Configuration files

Everything lives in `%APPDATA%\Decka`: `config.json`, `actions.json`, `layout.json`, `devices.json`. Formats and limits are described in [docs/DATA_MODEL.md](docs/DATA_MODEL.md); the wire protocol is in [docs/PROTOCOL.md](docs/PROTOCOL.md). Files are watched and reloaded while the agent runs.

## Building from source

Requirements: Node.js 22, stable Rust, Windows; for the APK also Android Studio (SDK + platform-tools), a JDK and `ANDROID_HOME`.

```powershell
npm ci
npm run typecheck; npm test; cargo test -p decka-core
npm run tauri dev        # agent with the settings window
npm run release:agent    # NSIS installer in target\release\bundle\nsis\
npm run release:apk      # client\android\app\build\outputs\apk\debug\app-debug.apk
```

## Troubleshooting

- **The phone cannot reach the PC.** Windows Firewall must allow `decka-agent.exe` on private networks; the phone and PC must be on the same network (guest Wi-Fi often isolates clients). Try USB mode to rule the network out.
- **`adb unauthorized`.** Unlock the phone and accept the USB debugging prompt; if it does not appear, replug the cable or revoke USB debugging authorizations in Developer options.
- **Port 9247 is busy.** Another program uses it. Change the port in **Settings** and press **Restart now**, or press **Retry** on the Status tab after closing the other program.
- **The phone does not find the PC after the PC slept.** Decka reconnects by itself within about 30 seconds; if the PC hibernates instead of sleeping, network and USB links are down until it wakes. Prefer sleep over hibernation.

## Battery and burn-in notes

- A phone that is always charging at 100% ages its battery and, in the worst case, can swell. Power it from a PC USB port (low current) rather than a fast charger, keep it out of direct sun, check the case now and then, and stop using it at once if it bulges. A smart plug on a schedule helps. Decka cannot limit charging without root.
- Pixel shift and dimming slow burn-in but do not remove it; bright static icons can still leave marks. Prefer dark button colors.

## License

MIT, see [LICENSE](LICENSE). Acknowledgements: [NOTICE.md](NOTICE.md).
