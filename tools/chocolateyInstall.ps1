$ErrorActionPreference = 'Stop'

$packageArgs = @{
  packageName    = $env:ChocolateyPackageName
  fileType       = 'exe'
  url64bit       = 'https://github.com/ganeshgfx/gfxtools/releases/download/v1.1.0/GFXTools_Installer.exe'
  checksum64     = 'B2A174DA57F790AF6D6BF448A2496D1924C3E2BA8D36357ECC4F1204BD555986'
  checksumType64 = 'sha256'
  # Inno Setup silent flags
  silentArgs     = '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP-'
  validExitCodes = @(0)
}

Install-ChocolateyPackage @packageArgs
