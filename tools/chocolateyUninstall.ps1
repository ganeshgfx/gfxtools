$ErrorActionPreference = 'Stop'

$appDir = Join-Path $env:LOCALAPPDATA 'GFXTools'
$exe    = Join-Path $appDir 'gfx-tools.exe'

# Ask the app to unregister its own context menu entries (HKCU registry)
if (Test-Path $exe) {
    Write-Host "Running gfx-tools uninstall to remove context menu entries..."
    Start-Process $exe -ArgumentList 'uninstall' -Wait -WindowStyle Hidden
}

# Remove application directory
if (Test-Path $appDir) {
    Write-Host "Removing $appDir ..."
    Remove-Item $appDir -Recurse -Force
}

Write-Host "GFX Tools uninstalled successfully."
