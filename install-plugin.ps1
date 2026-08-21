<#
.SYNOPSIS
    Installs / reinstalls the Video Yoinker CEP plugin for Adobe Premiere Pro and After Effects.

.DESCRIPTION
    1. Enables CEP debug mode (PlayerDebugMode) so unsigned plugins load.
    2. Copies the plugin folder to %APPDATA%\Adobe\CEP\extensions\VideoYoinker\.
    3. Optionally copies paste-link-downloader.exe from target\release\ if not already in plugin\bin\.
    4. Prints next-steps instructions.

.EXAMPLE
    .\install-plugin.ps1
    .\install-plugin.ps1 -Uninstall
#>

param(
    [switch]$Uninstall
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# ── Config ──────────────────────────────────────────────────────────────────
$BundleId   = "VideoYoinker"
$PluginSrc  = Join-Path $PSScriptRoot "plugin"
$ExtRoot    = Join-Path $env:APPDATA "Adobe\CEP\extensions"
$Dest       = Join-Path $ExtRoot $BundleId

# CEP version keys to enable debug mode (cover CC 2019 through CC 2024+)
$CsxsVersions = @(10, 11, 12)

# ── Uninstall ────────────────────────────────────────────────────────────────
if ($Uninstall) {
    Write-Host "`n[Video Yoinker] Uninstalling…" -ForegroundColor Yellow
    if (Test-Path $Dest) {
        Remove-Item $Dest -Recurse -Force
        Write-Host "  ✓ Removed: $Dest" -ForegroundColor Green
    } else {
        Write-Host "  Plugin not installed at: $Dest" -ForegroundColor DarkGray
    }
    Write-Host "`nDone. Restart Premiere Pro / After Effects." -ForegroundColor Cyan
    exit 0
}

# ── Pre-flight checks ────────────────────────────────────────────────────────
Write-Host "`n[Video Yoinker] Installing CEP plugin…" -ForegroundColor Cyan

# Verify plugin source exists
if (-not (Test-Path $PluginSrc)) {
    Write-Error "Plugin source not found: $PluginSrc`nRun this script from the repo root."
}

# Auto-copy exe from release build if missing from plugin/bin
$BinDir    = Join-Path $PluginSrc "bin"
$PluginExe = Join-Path $BinDir "paste-link-downloader.exe"
$ReleaseExe = Join-Path $PSScriptRoot "target\release\paste-link-downloader.exe"

if (-not (Test-Path $PluginExe)) {
    if (Test-Path $ReleaseExe) {
        Write-Host "  Copying exe from release build…"
        Copy-Item $ReleaseExe $BinDir -Force
        Write-Host "  ✓ paste-link-downloader.exe copied." -ForegroundColor Green
    } else {
        Write-Warning "  paste-link-downloader.exe not found in plugin\bin\ or target\release\."
        Write-Warning "  Run: cargo build --release"
        Write-Warning "  Then re-run this script."
    }
}

# Note: yt-dlp and ffmpeg are NOT required in plugin\bin\
# The Rust binary reads %APPDATA%\PasteLinkDownloader\config.toml (same as the
# Settings GUI) and auto-resolves yt-dlp / ffmpeg from there, or falls back to PATH.
Write-Host "  ✓ yt-dlp/ffmpeg resolved via app settings or PATH (no manual copy needed)." -ForegroundColor Green

# ── Enable CEP debug mode ────────────────────────────────────────────────────
Write-Host "`n  Enabling CEP PlayerDebugMode…"
foreach ($ver in $CsxsVersions) {
    $key = "HKCU:\Software\Adobe\CSXS.$ver"
    try {
        if (-not (Test-Path $key)) { New-Item $key -Force | Out-Null }
        Set-ItemProperty -Path $key -Name "PlayerDebugMode" -Value "1" -Type String
        Write-Host "    ✓ CSXS.$ver PlayerDebugMode = 1" -ForegroundColor Green
    } catch {
        Write-Host "    ⚠ Could not set CSXS.$ver : $_" -ForegroundColor Yellow
    }
}

# ── Copy plugin to CEP extensions dir ────────────────────────────────────────
Write-Host "`n  Installing to: $Dest"

New-Item -ItemType Directory -Force $ExtRoot | Out-Null

if (Test-Path $Dest) {
    Remove-Item $Dest -Recurse -Force
}

Copy-Item $PluginSrc $Dest -Recurse -Force
Write-Host "  ✓ Plugin installed." -ForegroundColor Green

# ── Summary ──────────────────────────────────────────────────────────────────
Write-Host @"

╔══════════════════════════════════════════════════════════════╗
║          Video Yoinker — Plugin Installed Successfully       ║
╚══════════════════════════════════════════════════════════════╝

  Location : $Dest

  Next steps:
    1. (Re)start Adobe Premiere Pro or After Effects.
    2. Window → Extensions → Video Yoinker

  yt-dlp / ffmpeg are auto-resolved from:
    • App settings  : $env:APPDATA\PasteLinkDownloader\config.toml
    • Fallback      : system PATH
  (Configure paths via: .\paste-link-downloader.exe --settings)

  To uninstall:
    .\install-plugin.ps1 -Uninstall

"@ -ForegroundColor Cyan
