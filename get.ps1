$url  = "https://github.com/ganeshgfx/gfxtools/releases/download/v1.0.0/GFXTools_Installer.exe"
$path = "$env:TEMP\GFXTools_Installer.exe"

Write-Host "Downloading GFX Tools installer..."
Invoke-WebRequest $url -OutFile $path -UseBasicParsing

Write-Host "Installing silently..."
$proc = Start-Process $path -ArgumentList "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART" -Wait -PassThru

Remove-Item $path -Force

if ($proc.ExitCode -ne 0) {
    Write-Error "Installer exited with code $($proc.ExitCode)"
    exit $proc.ExitCode
}

Write-Host "GFX Tools installed successfully."