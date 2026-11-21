# `npm run release:agent`: builds the window and the NSIS installer with its update signature (updater.key from DPAPI files).
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "updater-key.ps1")
Set-UpdaterEnv
Push-Location (Join-Path $PSScriptRoot "..")
try {
  npm run build:ui
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  npm run tauri -- build
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
} finally {
  Pop-Location
  Clear-UpdaterEnv
}
