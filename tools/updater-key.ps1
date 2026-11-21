# Dot-source this file: `. tools\updater-key.ps1; Set-UpdaterEnv`.
# Puts the update-signing key (path) and its password into THIS process' environment only. The password is read from a
# DPAPI-protected file next to the key and is never printed or written anywhere else.
function Set-UpdaterEnv {
  $dir = Join-Path $env:USERPROFILE ".decka-signing"
  $key = Join-Path $dir "updater.key"
  $passFile = Join-Path $dir "updater.pass"
  if (-not (Test-Path $key) -or -not (Test-Path $passFile)) {
    throw "Update signing key not found in $dir (need updater.key and updater.pass); see docs/TASKS.md T20.3. Refusing to build an unsigned update."
  }
  $secure = Get-Content $passFile | ConvertTo-SecureString
  $bstr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure)
  try { $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($bstr) } finally { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($bstr) }
  # The Tauri tools take the key text (not a path) from this variable; the key is encrypted with the password above.
  $env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content $key -Raw).Trim()
}

function Clear-UpdaterEnv {
  Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue
  Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY -ErrorAction SilentlyContinue
}
