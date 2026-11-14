# Testing

## 1. Automatic tests

```powershell
npm run typecheck
npm test
cargo test -p decka-core
```

`npm test` runs vitest in `shared`, `client` and `agent`; `cargo test -p decka-core` runs unit and integration tests of the agent core (WebSocket, HTTP API, actions with `RecordingRunner`, USB loop against a fake adb). CI (`.github/workflows/ci.yml`) runs the same commands plus `npm run build -w client`, `npm run build:ui` and `cargo build -p decka-agent`.

## 2. The headless agent and `ws-smoke.mjs`

`decka-headless` is the agent without a window. Arguments: `--port <n>`, `--config-dir <path>`, `--client-dist <path>`, `--pair-code <CODE>`, `--dry-run` (no real actions), `--version`.

**H-BUILD**
```powershell
cargo build -p decka-core --bin decka-headless
```
**H-START** (port 19247, clean config, pairing code `TESTCDE2`, no real actions)
```powershell
New-Item -ItemType Directory -Force .tmp | Out-Null
Remove-Item -Recurse -Force .tmp\cfg -ErrorAction SilentlyContinue
$h = Start-Process -FilePath target\debug\decka-headless.exe -ArgumentList '--port','19247','--config-dir','.tmp\cfg','--pair-code','TESTCDE2','--dry-run','--client-dist','client\dist' -PassThru -NoNewWindow -RedirectStandardOutput .tmp\headless.log
Start-Sleep -Seconds 2
```
**H-STOP**
```powershell
Stop-Process -Id $h.Id -Force
```
**H-TOKEN** (pair and get a device token; after H-START)
```powershell
$tok = (node tools/ws-smoke.mjs pair 127.0.0.1:19247 TESTCDE2 | ConvertFrom-Json).token
```
**H-API** (read the API token; after H-START)
```powershell
$api = (Get-Content .tmp\cfg\config.json -Raw | ConvertFrom-Json).apiToken
```

`tools/ws-smoke.mjs` (Node 22+, no dependencies) prints one JSON line and exits; on a server `error` or after 5 s it prints to stderr and exits with 1:

| Command | Result |
|---|---|
| `pair <host:port> <code>` | `{"deviceId","token"}` |
| `state <host:port> <token>` | `welcome.state` |
| `press <host:port> <token> <actionId>` | the `press_result` object |
| `watch <host:port> <token> <seconds>` | every `patch` received in that time, one per line |
| `raw <host:port> <file.json> [token]` | with a token: `hello` first; then sends the file as one frame and prints the first non-`patch` answer |

Pass JSON through files: Windows PowerShell 5.1 mangles quotes in arguments of native programs.

## 3. Release checklist (manual)

Run on a Galaxy S9 (or another Android 10+ phone) with the installed agent. Results go to `docs/MANUAL_CHECKS.md` as `PASS`, `FAIL` or `PENDING (owner)`.

| № | Step | Expected result |
|---|---|---|
| T3.3 | Headless without `--dry-run`, default config; `press task-manager` | Task Manager opens |
| T4.3 | Open `http://127.0.0.1:19247/#c=TESTCDE2&h=127.0.0.1&p=19247&n=Test` in a browser on the PC | `Connected to` and the PC name are shown; the hash disappears from the address bar; after a page reload the label stays (token saved) |
| T4.4 | Pair, look at the first page | 10 tiles in the positions from `layout.default.json`; swipe left opens page `Dev` with 5 tiles; tile `Main` returns to the first page; two page dots |
| T4.5 | As T4.3 (after pairing); press a failing action | Error toast on a failed action |
| T4.6 | Stop headless; later start it again; five taps in the top-left corner | After ~3 s the banner `PC offline — retrying`, action buttons semi-transparent; after restart the banner disappears within 16 s without reloading; five taps open the dialog, `Unpair` leads to the pairing screen |
| T5.5 | Clock widget and `sys` widget with a live agent | The clock runs; the CPU/RAM bars change |
| T5.6 | `curl` status command from the Integrations tab (T5.2) | On page `Dev`, `Claude: Decka` appears with a blue pulsing dot and the text `editing files` |
| T5.7 | `curl` notify command (T5.1) | A toast for 8 s and a row in the feed; tapping the row removes it (also in a second browser tab) |
| T6.3 | Close the settings window; start the exe a second time; tray `Quit` | The process and the tray icon stay; no second `decka-agent` process; `Quit` ends the process |
| T6.4 | Status tab; open the client in a browser | `Listening on 0.0.0.0:9247`; the device appears in the table without reloading the window |
| T6.5 | Pairing tab; scan the QR with the phone camera; press `Revoke` | The QR opens the client and it pairs; `Revoke` returns the client to the pairing screen within 2 s |
| T6.6 | Actions tab: create a `launch` action for `charmap.exe`, press `Test` | Character Map opens; `%APPDATA%\Decka\actions.json` contains the new action |
| T6.7 | Layout tab: add a button, `Save` | The button appears on the connected client within 1 s without reloading |
| T6.8 | Integrations: `Send test notification`; paste a copied `curl.exe` command into PowerShell; Settings: port 9248 + `Restart now` | Toast on the client; the command returns `{"id":"…"}`; `curl.exe http://127.0.0.1:9248/api/health` answers |
| T7.1 | `adb install -r client\android\app\build\outputs\apk\debug\app-debug.apk` | The app opens in landscape on the pairing screen |
| T7.2 | Open the app on the S9; leave it for 5 minutes with a 30 s system screen timeout | No status bar and no navigation bar; the screen stays on |
| T7.3 | Scan the QR from the Pairing tab in the app | The button grid shows in under 5 s |
| T7.4 | Phone with USB debugging, `adb` in PATH, `USB mode` on | `adb reverse --list` shows `tcp:9247 tcp:9247` within 10 s of plugging in the cable |
| T7.5a | Wi-Fi off on the phone, cable in, USB mode on | The app connects within 10 s, indicator `USB`, `Play` works |
| T7.5b | Settings: `USB only (127.0.0.1)` + `Restart now` | `http://<PC IP>:9247/api/health` from another device is unreachable; the phone works over USB |
| T7.5c | Cable out, Wi-Fi on, mode `Wi-Fi and USB` | Reconnects within 20 s, indicator `Wi-Fi` |
| T7.5d | PC sleeps for 1 minute, then wakes | The client reconnects within 30 s of the Windows sign-in, with no user action |
| T8.1 | `dimAfterSec: 10` | The screen dims after 10 s; the first touch only restores brightness, the second presses the button |
| T8.3 | `sleepAfterSec: 30`; stop the agent for a minute; `curl` with `"level":"error"` | After 30 s only a grey clock remains and it moves; after the agent stops the screen saver shows `PC offline`; the error notification wakes the screen |
| T9.1 | Look at the launcher icon of the APK | It is the Capacitor template icon — replace before a public release (owner) |
