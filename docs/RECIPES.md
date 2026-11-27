# Recipes

Replace `<PC>` with the address of your PC (or `127.0.0.1` on the PC itself) and `<TOKEN>` with the API token from the **Integrations** tab. Commands are for Windows PowerShell 5.1; JSON goes through a file because PowerShell mangles quotes in arguments to native programs.

## a) curl: JSON and plain text

```powershell
# Plain text: the body (cut to 120 characters) becomes the title, level = info
curl.exe -s -X POST http://<PC>:9247/api/notify -H "Authorization: Bearer <TOKEN>" --data-binary "Build finished"

# JSON
Set-Content notify.json '{"title":"Deploy finished","body":"web@1.4.2","level":"success","source":"ci"}' -Encoding ascii
curl.exe -s -X POST http://<PC>:9247/api/notify -H "Authorization: Bearer <TOKEN>" -H "Content-Type: application/json" --data-binary "@notify.json"
```

`level` is one of `info`, `success`, `warning`, `error`. Only `warning` and `error` wake a sleeping screen.

## b) PowerShell Invoke-RestMethod

```powershell
$headers = @{ Authorization = "Bearer <TOKEN>" }
$body = @{ title = "Backup done"; level = "success" } | ConvertTo-Json
Invoke-RestMethod -Method Post -Uri http://<PC>:9247/api/notify -Headers $headers -ContentType "application/json; charset=utf-8" -Body ([Text.Encoding]::UTF8.GetBytes($body))
```

## c) Deploy status: running, then ok or error

```powershell
param([string]$Command = ".\deploy.ps1")
$h = @{ Authorization = "Bearer <TOKEN>" }
function Set-Deploy($state, $text) {
  $json = @{ id = "deploy"; label = "Deploy"; state = $state; text = $text; ttlSec = 1800 } | ConvertTo-Json
  Invoke-RestMethod -Method Post -Uri http://<PC>:9247/api/status -Headers $h -ContentType "application/json; charset=utf-8" -Body ([Text.Encoding]::UTF8.GetBytes($json)) | Out-Null
}
Set-Deploy "running" "in progress"
& $Command
if ($LASTEXITCODE -eq 0) { Set-Deploy "ok" "done" } else { Set-Deploy "error" "exit $LASTEXITCODE" }
```

States: `ok`, `warn`, `error`, `running`, `unknown`. A status without a new update for `ttlSec` seconds turns `stale`.

## d) Claude Code hooks

Claude Code runs commands on events. `UserPromptSubmit` fires when you send a prompt, `Stop` when Claude finishes answering; neither takes a matcher. Hooks go into `~/.claude/settings.json` (all projects) or `.claude/settings.json` (one project). Format reference: <https://code.claude.com/docs/en/hooks>.

```json
{
  "hooks": {
    "UserPromptSubmit": [
      { "hooks": [ { "type": "command", "command": "powershell -NoProfile -File C:\\tools\\decka-claude.ps1 running" } ] }
    ],
    "Stop": [
      { "hooks": [ { "type": "command", "command": "powershell -NoProfile -File C:\\tools\\decka-claude.ps1 ok" } ] }
    ]
  }
}
```

`C:\tools\decka-claude.ps1`:

```powershell
param([string]$State)
$h = @{ Authorization = "Bearer <TOKEN>" }
function Post($path, $obj) {
  $json = $obj | ConvertTo-Json
  Invoke-RestMethod -Method Post -Uri "http://<PC>:9247$path" -Headers $h -ContentType "application/json; charset=utf-8" -Body ([Text.Encoding]::UTF8.GetBytes($json)) | Out-Null
}
Post "/api/status" @{ id = "claude"; label = "Claude"; state = $State; text = $(if ($State -eq "running") { "working" } else { "idle" }); group = "ai"; ttlSec = 900 }
if ($State -eq "ok") { Post "/api/notify" @{ title = "Claude finished"; level = "success"; source = "claude" } }
```

## e) GitHub Actions

```yaml
- name: Tell Decka
  if: always()
  run: |
    curl -s -X POST "http://<PC>:9247/api/notify" \
      -H "Authorization: Bearer ${{ secrets.DECKA_TOKEN }}" \
      --data-binary "CI ${{ job.status }}: ${{ github.repository }}"
```

The runner must be able to reach your PC: use a self-hosted runner on the same network, or a VPN. Never expose the agent port to the internet.

## f) Switching a button state with variables

A button with `bind` changes its look by the value of a variable. Set it from a script:

```powershell
$h = @{ Authorization = "Bearer <TOKEN>" }
$json = @{ value = $true } | ConvertTo-Json
Invoke-RestMethod -Method Put -Uri http://<PC>:9247/api/vars/build.failed -Headers $h -ContentType "application/json" -Body $json
```

The value is a boolean, a number or a string of up to 200 characters; the button picks the state whose key equals `String(value)`. Remove a variable with `DELETE /api/vars/build.failed`. The names starting with `audio.`, `mic.`, `media.` and `obs.` are set by Decka itself (volume, microphone, player, OBS): you can bind a button to them, but writing them gives `400 reserved variable`.

## g) Reaching Decka under your own name (VPN, domain)

Since 0.51.1 the agent answers only requests whose address is an IP address, `localhost`, the name of this PC or `<name>.local`; anything else gets `403` (protection from DNS rebinding). To use another name, for example a Tailscale MagicDNS name, add it to `config.json` and restart Decka:

```json
"server": { "allowedHosts": ["my-pc.tail1234.ts.net"] }
```

Names only, without `http://` and without a port; up to 20.
