$ErrorActionPreference = 'Stop'

$packageArgs = @{
  packageName    = $env:ChocolateyPackageName
  fileType       = 'exe'
  url64bit       = 'https://github.com/ganeshgfx/gfxtools/releases/download/v1.1.1/GFXTools_Installer.exe'
  checksum64     = 'E335FB90BCF528A0645402B10C4F25C3918793718B783862E29463A4CC9B47B1'
  checksumType64 = 'sha256'
  # Inno Setup silent flags
  silentArgs     = '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP-'
  validExitCodes = @(0)
}

Install-ChocolateyPackage @packageArgs
