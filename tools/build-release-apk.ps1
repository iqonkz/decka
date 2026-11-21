# Builds the signed release APK. The keystore password is read from a DPAPI-protected file and lives only in this process' environment.
$ErrorActionPreference = "Stop"
$dir = Join-Path $env:USERPROFILE ".decka-signing"
$jks = Join-Path $dir "decka-release.jks"
$passFile = Join-Path $dir "decka-release.pass"
if (-not (Test-Path $jks) -or -not (Test-Path $passFile)) {
  Write-Error "Release signing key not found in $dir (need decka-release.jks and decka-release.pass). Create it as described in docs/TASKS.md T19.6; refusing to build an unsigned or debug APK."
  exit 1
}
$secure = Get-Content $passFile | ConvertTo-SecureString
$bstr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure)
try { $env:DECKA_KEYSTORE_PASS = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($bstr) } finally { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($bstr) }
$env:DECKA_KEYSTORE = $jks
Push-Location (Join-Path $PSScriptRoot "..\client\android")
try {
  & .\gradlew.bat assembleRelease
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
} finally {
  Pop-Location
  Remove-Item Env:DECKA_KEYSTORE_PASS -ErrorAction SilentlyContinue
}
