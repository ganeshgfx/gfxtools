<#
.SYNOPSIS
    Build release binary and package Inno Setup installer.

.EXAMPLE
    .\build-installer.ps1
    .\build-installer.ps1 -SkipBuild      # skip cargo build (use existing binary)
    .\build-installer.ps1 -SkipInstaller  # build only the binary
#>

param(
    [switch]$SkipBuild,
    [switch]$SkipInstaller
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

# ── 2. Inno Setup ─────────────────────────────────────────────────────────────
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
