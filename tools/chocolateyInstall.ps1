$ErrorActionPreference = 'Stop'

$packageArgs = @{
  packageName    = $env:ChocolateyPackageName
  fileType       = 'exe'
  url64bit       = 'https://github.com/ganeshgfx/gfxtools/releases/download/v1.0.0/GFXTools_Installer.exe'
  checksum64     = '71059DA1CE83B2FEAE04ECCD222FA7B9BA1706C95F05D92D1A3D4DA84D423C17'
  checksumType64 = 'sha256'
  # Inno Setup silent flags
  silentArgs     = '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP-'
  validExitCodes = @(0)
}

Install-ChocolateyPackage @packageArgs
