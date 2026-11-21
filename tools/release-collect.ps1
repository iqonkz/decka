# `npm run release:collect`: gathers release\ (installer, APK, latest.json, SHA256SUMS.txt + .sig, notes.md); the key comes from DPAPI files.
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "updater-key.ps1")
Set-UpdaterEnv
Push-Location (Join-Path $PSScriptRoot "..")
try {
  node tools/collect-release.mjs
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
} finally {
  Pop-Location
  Clear-UpdaterEnv
}
