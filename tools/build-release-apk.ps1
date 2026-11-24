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
  # --no-daemon: a daemon left running keeps the output pipe of the caller open, and the script (with the permission check below) never ends.
  & .\gradlew.bat assembleRelease --no-daemon
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  # Guard (0.20.5 shipped without RECORD_AUDIO): the built APK must declare these permissions, else the build fails.
  $apk = Join-Path (Get-Location) "app\build\outputs\apk\release\app-release.apk"
  $sdk = if ($env:ANDROID_HOME) { $env:ANDROID_HOME } elseif ($env:ANDROID_SDK_ROOT) { $env:ANDROID_SDK_ROOT } else { Join-Path $env:USERPROFILE "Android\Sdk" }
  $aapt = Get-ChildItem (Join-Path $sdk "build-tools") -Recurse -Include aapt2.exe, aapt.exe -ErrorAction SilentlyContinue | Sort-Object FullName -Descending | Select-Object -First 1
  if (-not $aapt) { Write-Error "aapt/aapt2 not found in $sdk\build-tools; cannot verify APK permissions"; exit 1 }
  $dump = (& $aapt.FullName dump permissions $apk) -join "`n"
  foreach ($p in "RECORD_AUDIO", "CAMERA", "INTERNET") {
    if ($dump -notmatch "android\.permission\.$p") { Write-Error "APK is missing android.permission.$p"; exit 1 }
  }
  Write-Host "APK permissions OK (RECORD_AUDIO, CAMERA, INTERNET)"
} finally {
  Pop-Location
  Remove-Item Env:DECKA_KEYSTORE_PASS -ErrorAction SilentlyContinue
}
