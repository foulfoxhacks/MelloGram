# Align, sign, verify and checksum the Android packages built on this machine.
#
#   powershell -ExecutionPolicy Bypass -File scripts/sign-android.ps1
#   powershell -ExecutionPolicy Bypass -File scripts/sign-android.ps1 -Keystore C:\keys\mellogram.jks -Alias mellogram
#
# The keystore password is read from MELLOGRAM_KEYSTORE_PASSWORD, or prompted for
# and kept only in this process. It is never passed on a command line, so it
# does not land in PowerShell history.
#
# Output: dist\MelloGram-android.apk, dist\MelloGram-android-unsigned.aab
# (when an AAB exists) and dist\SHA256SUMS.
[CmdletBinding()]
param(
    [string]$Keystore = (Join-Path $HOME "mellogram-release.jks"),
    [string]$Alias = "mellogram",
    [string]$BuildTools,
    [string]$Apk,
    [string]$Aab,
    [string]$OutDir = "dist"
)

$ErrorActionPreference = "Stop"

function Fail($message) { Write-Host "error: $message" -ForegroundColor Red; exit 1 }
function Step($message) { Write-Host "==> $message" -ForegroundColor Magenta }

# ---- locate the SDK tools --------------------------------------------------

$sdk = $env:ANDROID_SDK_ROOT
if (-not $sdk) { $sdk = $env:ANDROID_HOME }
if (-not $sdk) { $sdk = Join-Path $HOME "AppData\Local\Android\Sdk" }
if (-not (Test-Path $sdk)) { Fail "Android SDK not found; set ANDROID_SDK_ROOT or pass -BuildTools" }

if (-not $BuildTools) {
    $candidate = Get-ChildItem (Join-Path $sdk "build-tools") -Directory -ErrorAction SilentlyContinue |
        Sort-Object { [version]($_.Name -replace '[^0-9.].*$', '') } -Descending |
        Select-Object -First 1
    if (-not $candidate) { Fail "no build-tools under $sdk; install one with sdkmanager" }
    $BuildTools = $candidate.FullName
}

$zipalign = Join-Path $BuildTools "zipalign.exe"
$apksigner = Join-Path $BuildTools "apksigner.bat"
foreach ($tool in @($zipalign, $apksigner)) {
    if (-not (Test-Path $tool)) { Fail "$tool is missing; install build-tools 34 or newer" }
}

# ---- find what to sign -----------------------------------------------------

$outputs = Join-Path $PSScriptRoot "..\app\src-tauri\gen\android\app\build\outputs"
if (-not $Apk) {
    $found = Get-ChildItem $outputs -Recurse -Filter "*-unsigned.apk" -File -ErrorAction SilentlyContinue |
        Sort-Object @{ Expression = { if ($_.FullName -match '\\universal\\') { 0 } else { 1 } } }, LastWriteTime -Descending
    if (-not $found) { Fail "no unsigned APK under $outputs; run 'cargo tauri android build --apk' first" }
    $Apk = $found[0].FullName
}
if (-not (Test-Path $Apk)) { Fail "APK not found: $Apk" }

if (-not $Aab) {
    $bundle = Get-ChildItem $outputs -Recurse -Filter "*.aab" -File -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if ($bundle) { $Aab = $bundle.FullName }
}

if (-not (Test-Path $Keystore)) { Fail "keystore not found: $Keystore (create one with keytool -genkeypair)" }
Step "APK      $Apk"
Step "Keystore $Keystore (alias $Alias)"

# ---- password ---------------------------------------------------------------

$prompted = $false
if (-not $env:MELLOGRAM_KEYSTORE_PASSWORD) {
    $secure = Read-Host "Keystore password" -AsSecureString
    $env:MELLOGRAM_KEYSTORE_PASSWORD = [Runtime.InteropServices.Marshal]::PtrToStringBSTR(
        [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure))
    $prompted = $true
}
if (-not $env:MELLOGRAM_KEY_PASSWORD) { $env:MELLOGRAM_KEY_PASSWORD = $env:MELLOGRAM_KEYSTORE_PASSWORD }

$aligned = Join-Path $env:TEMP "mellogram-aligned.apk"
$signed = Join-Path $OutDir "MelloGram-android.apk"

try {
    New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

    Step "Aligning"
    & $zipalign -f -p 4 $Apk $aligned
    if ($LASTEXITCODE -ne 0) { Fail "zipalign failed ($LASTEXITCODE)" }

    Step "Signing"
    & $apksigner sign --ks $Keystore --ks-key-alias $Alias `
        --ks-pass "env:MELLOGRAM_KEYSTORE_PASSWORD" `
        --key-pass "env:MELLOGRAM_KEY_PASSWORD" `
        --out $signed $aligned
    if ($LASTEXITCODE -ne 0) { Fail "apksigner sign failed ($LASTEXITCODE)" }

    Step "Verifying"
    & $apksigner verify --print-certs $signed
    if ($LASTEXITCODE -ne 0) { Fail "apksigner verify failed ($LASTEXITCODE)" }

    $files = @($signed)
    if ($Aab -and (Test-Path $Aab)) {
        Step "Signing the App Bundle"
        $signedAab = Join-Path $OutDir "MelloGram-android-unsigned.aab"
        & jarsigner -keystore $Keystore -storepass:env MELLOGRAM_KEYSTORE_PASSWORD `
            -keypass:env MELLOGRAM_KEY_PASSWORD -sigalg SHA256withRSA -digestalg SHA-256 `
            -signedjar $signedAab $Aab $Alias
        if ($LASTEXITCODE -ne 0) { Fail "jarsigner failed ($LASTEXITCODE)" }
        & jarsigner -verify $signedAab
        if ($LASTEXITCODE -ne 0) { Fail "the signed bundle did not verify" }
        $files += $signedAab
    }

    Step "Writing checksums"
    $lines = foreach ($file in $files) {
        $hash = (Get-FileHash $file -Algorithm SHA256).Hash.ToLowerInvariant()
        "{0}  {1}" -f $hash, (Split-Path $file -Leaf)
    }
    $lines | Set-Content -Path (Join-Path $OutDir "SHA256SUMS") -Encoding ASCII
    Write-Host ($lines -join "`n")

    Write-Host ""
    Step "Signed packages in $OutDir"
    Get-ChildItem $OutDir -File | Format-Table Name, Length -AutoSize
    Write-Host "Keep a backup of ${Keystore}: without it you cannot update anyone who installed this APK."
}
finally {
    Remove-Item $aligned -ErrorAction SilentlyContinue
    if ($prompted) {
        Remove-Item Env:\MELLOGRAM_KEYSTORE_PASSWORD -ErrorAction SilentlyContinue
        Remove-Item Env:\MELLOGRAM_KEY_PASSWORD -ErrorAction SilentlyContinue
    }
}
