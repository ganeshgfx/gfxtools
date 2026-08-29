<#
.SYNOPSIS
    Master install script: build, uninstall old, install new.

.DESCRIPTION
    1. cargo build --release
    2. Uninstall old context menu (gfx-tools.exe uninstall)
    3. Uninstall old CEP plugin (install-plugin.ps1 -Uninstall)
    4. Install new context menu (gfx-tools.exe install)
    5. Install new CEP plugin (install-plugin.ps1)

.EXAMPLE
    .\install.ps1
    .\install.ps1 -SkipBuild        # skip cargo build
    .\install.ps1 -SkipPlugin       # skip CEP plugin install
    .\install.ps1 -SkipContextMenu  # skip context menu install
#>

param(
    [switch]$SkipBuild,
    [switch]$SkipPlugin,
    [switch]$SkipContextMenu
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$RepoRoot   = $PSScriptRoot
$ReleaseBin = Join-Path $RepoRoot "target\release\gfx-tools.exe"
$PluginScript = Join-Path $RepoRoot "install-plugin.ps1"

function Write-Step($msg) {
    Write-Host "`n>> $msg" -ForegroundColor Cyan
}

function Write-Ok($msg) {
    Write-Host "   OK: $msg" -ForegroundColor Green
}

function Write-Skip($msg) {
    Write-Host "   SKIP: $msg" -ForegroundColor DarkGray
}

# ── 1. Build ─────────────────────────────────────────────────────────────────
if (-not $SkipBuild) {
    Write-Step "Building release binary..."
    Push-Location $RepoRoot
    try {
        cargo build --release
        if ($LASTEXITCODE -ne 0) { throw "cargo build failed (exit code $LASTEXITCODE)" }
        Write-Ok "Build complete."
    } finally {
        Pop-Location
    }
} else {
    Write-Skip "Build (--SkipBuild)"
}

# Verify binary exists
if (-not (Test-Path $ReleaseBin)) {
    throw "Release binary not found: $ReleaseBin`nRun without -SkipBuild first."
}

# ── 2. Uninstall old context menu ────────────────────────────────────────────
if (-not $SkipContextMenu) {
    Write-Step "Uninstalling old context menu..."
    & $ReleaseBin uninstall 2>$null
    Write-Ok "Old context menu removed."
} else {
    Write-Skip "Context menu uninstall (--SkipContextMenu)"
}

# ── 3. Uninstall old CEP plugin ─────────────────────────────────────────────
if (-not $SkipPlugin) {
    Write-Step "Uninstalling old CEP plugin..."
    if (Test-Path $PluginScript) {
        & powershell -ExecutionPolicy Bypass -File $PluginScript -Uninstall
        Write-Ok "Old plugin removed."
    } else {
        Write-Skip "Plugin script not found: $PluginScript"
    }
} else {
    Write-Skip "Plugin uninstall (--SkipPlugin)"
}

# ── 4. Install new context menu ─────────────────────────────────────────────
if (-not $SkipContextMenu) {
    Write-Step "Installing context menu..."
    & $ReleaseBin install
    if ($LASTEXITCODE -ne 0) { throw "Context menu install failed (exit code $LASTEXITCODE)" }
    Write-Ok "Context menu installed."
} else {
    Write-Skip "Context menu install (--SkipContextMenu)"
}

# ── 5. Install new CEP plugin ───────────────────────────────────────────────
if (-not $SkipPlugin) {
    Write-Step "Installing CEP plugin..."
    if (Test-Path $PluginScript) {
        & powershell -ExecutionPolicy Bypass -File $PluginScript
        if ($LASTEXITCODE -ne 0) { throw "Plugin install failed (exit code $LASTEXITCODE)" }
        Write-Ok "Plugin installed."
    } else {
        Write-Skip "Plugin script not found: $PluginScript"
    }
} else {
    Write-Skip "Plugin install (--SkipPlugin)"
}

# ── Done ─────────────────────────────────────────────────────────────────────
Write-Host @"

==============================================================
  GFX Tools - Full Install Complete
==============================================================

  Binary  : $ReleaseBin
  Menu    : $( if ($SkipContextMenu) { "skipped" } else { "installed" } )
  Plugin  : $( if ($SkipPlugin) { "skipped" } else { "installed" } )

  Restart Explorer / Adobe apps to pick up changes.
"@ -ForegroundColor Green
