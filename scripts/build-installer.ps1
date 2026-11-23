<#
.SYNOPSIS
    Build release binary, download bundled deps (yt-dlp / FFmpeg / gallery-dl),
    and package an Inno Setup installer.

.EXAMPLE
    .\build-installer.ps1                  # full build: cargo + deps + ISCC
    .\build-installer.ps1 -SkipBuild       # skip cargo build (reuse existing binary)
    .\build-installer.ps1 -SkipDeps        # skip dep download (deps already in bin\)
    .\build-installer.ps1 -SkipInstaller   # build binary + deps only, skip ISCC
#>

param(
    [switch]$SkipBuild,
    [switch]$SkipInstaller,
    # Skip downloading yt-dlp / FFmpeg / gallery-dl into bin\
    # (useful if you've pre-staged them manually or want to reuse cached copies)
    [switch]$SkipDeps
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$Root    = Split-Path $PSScriptRoot -Parent
$Exe     = Join-Path $Root 'target\release\gfx-tools.exe'
$IssFile = Join-Path $Root 'installer.iss'
$Iscc    = "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"

# Fallback ISCC search locations
if (-not (Test-Path $Iscc)) {
    $candidates = @(
        'C:\Program Files (x86)\Inno Setup 6\ISCC.exe',
        'C:\Program Files\Inno Setup 6\ISCC.exe'
    )
    foreach ($c in $candidates) {
        if (Test-Path $c) { $Iscc = $c; break }
    }
}

function Write-Step($msg) { Write-Host "`n>> $msg" -ForegroundColor Cyan }
function Write-Ok($msg)   { Write-Host "   OK: $msg" -ForegroundColor Green }
function Write-Skip($msg) { Write-Host "   SKIP: $msg" -ForegroundColor DarkGray }
function Write-Fail($msg) { Write-Host "   FAIL: $msg" -ForegroundColor Red }

# ── 1. Cargo build ────────────────────────────────────────────────────────────
if (-not $SkipBuild) {
    Write-Step "cargo build --release"
    Push-Location $Root
    try {
        cargo build --release
        if ($LASTEXITCODE -ne 0) { throw "cargo build failed (exit $LASTEXITCODE)" }
        Write-Ok "Binary: $Exe"
    } finally {
        Pop-Location
    }
} else {
    Write-Skip "cargo build (-SkipBuild)"
}

if (-not (Test-Path $Exe)) {
    throw "Binary not found: $Exe`nRun without -SkipBuild first."
}

# ── 2. Stage bundled deps into bin\ ───────────────────────────────────────────
# installer.iss requires bin\yt-dlp.exe, bin\ffmpeg.exe, bin\ffprobe.exe,
# bin\gallery-dl.exe to be present at compile time (not downloaded post-install).
$BinDir     = Join-Path $Root 'bin'
$DepScript  = Join-Path $Root 'scripts\download-deps.ps1'
$RequiredBinaries = @('yt-dlp.exe', 'ffmpeg.exe', 'ffprobe.exe', 'gallery-dl.exe')

if (-not $SkipDeps) {
    Write-Step "Staging bundled deps into bin\ (download-deps.ps1 -SkipIfPresent)"
    if (-not (Test-Path $DepScript)) {
        throw "download-deps.ps1 not found at: $DepScript"
    }
    & powershell.exe -ExecutionPolicy Bypass -NonInteractive `
        -File $DepScript -InstallDir $Root -SkipIfPresent
    Write-Ok "Dep download complete"
} else {
    Write-Skip "Dep download (-SkipDeps)"
}

# Verify all required binaries are present before handing off to ISCC
$missing = @($RequiredBinaries | Where-Object { -not (Test-Path (Join-Path $BinDir $_)) })
if ($missing.Count -gt 0) {
    Write-Fail "Missing in bin\: $($missing -join ', ')"
    Write-Host "  Run without -SkipDeps, or place the files manually in: $BinDir" -ForegroundColor Yellow
    exit 1
}
Write-Ok "All required binaries present in bin\"

# ── 3. Inno Setup ─────────────────────────────────────────────────────────────
if (-not $SkipInstaller) {
    Write-Step "Building installer with Inno Setup..."

    if (-not (Test-Path $Iscc)) {
        Write-Fail "ISCC.exe not found. Install Inno Setup 6 from https://jrsoftware.org/isinfo.php"
        exit 1
    }

    if (-not (Test-Path $IssFile)) {
        throw "installer.iss not found at: $IssFile"
    }

    & $Iscc $IssFile
    if ($LASTEXITCODE -ne 0) { throw "ISCC failed (exit $LASTEXITCODE)" }

    $Output = Join-Path $Root 'Output\GFXTools_Installer.exe'
    $Size   = [math]::Round((Get-Item $Output).Length / 1MB, 2)
    Write-Ok "Installer: $Output ($Size MB)"
} else {
    Write-Skip "Inno Setup (-SkipInstaller)"
}

Write-Host @"

==============================================================
  Build complete
==============================================================
  Binary    : $Exe
  Installer : $($Root)\Output\GFXTools_Installer.exe
"@ -ForegroundColor Green
