<#
.SYNOPSIS
    Downloads yt-dlp, FFmpeg, and gallery-dl into the bin\ directory.

.DESCRIPTION
    Called automatically by the Inno Setup installer after placing gfx-tools.exe.
    Downloads run in parallel via background jobs. Uses curl.exe (built into
    Windows 10+) with resume support and retries. All downloads are best-effort --
    a single failure does not abort the installer.

    Tools land in:  <InstallDir>\bin\
      - yt-dlp.exe
      - ffmpeg.exe  + ffprobe.exe
      - gallery-dl.exe

    Pass -InstallDir when launching from Inno Setup (where $PSScriptRoot is empty).
#>
param(
    [string]$InstallDir = ''
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Continue'

# $PSScriptRoot is empty when launched hidden by Inno Setup -- use -InstallDir instead.
$Root    = if ($InstallDir -ne '') { $InstallDir } else { $PSScriptRoot }
$BinDir  = Join-Path $Root 'bin'
$LogFile = Join-Path $Root 'deps-download.log'

if (-not (Test-Path $BinDir)) {
    New-Item -ItemType Directory -Path $BinDir | Out-Null
}

# curl flags: fail-fast, silent, resume on drop, 5 retries with backoff, 30s connect timeout
$CurlBase = @('-fSL', '-C', '-', '--retry', '5', '--retry-delay', '3',
              '--retry-connrefused', '--connect-timeout', '30')

$CurlExe = "$env:SystemRoot\System32\curl.exe"
$HasCurl  = Test-Path $CurlExe

# --- Parallel jobs -----------------------------------------------------------

$jobYtDlp = Start-Job -ScriptBlock {
    param($BinDir, $CurlExe, $HasCurl, $CurlBase)
    $dest = Join-Path $BinDir 'yt-dlp.exe'
    $url  = 'https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe'
    try {
        if ($HasCurl) {
            & $CurlExe @CurlBase -o $dest $url 2>&1 | Out-Null
            if ($LASTEXITCODE -ne 0) { throw "curl exit $LASTEXITCODE" }
        } else {
            Invoke-WebRequest -Uri $url -OutFile $dest -UseBasicParsing -TimeoutSec 180
        }
        "OK: yt-dlp -> $dest"
    } catch { "WARN: yt-dlp failed -- $_" }
} -ArgumentList $BinDir, $CurlExe, $HasCurl, $CurlBase

$jobFfmpeg = Start-Job -ScriptBlock {
    param($BinDir, $CurlExe, $HasCurl, $CurlBase)
    $zip = Join-Path $env:TEMP 'ffmpeg-win64.zip'
    # Resolve latest win64 GPL build URL via GitHub API (avoids hardcoded filenames that change)
    try {
        $api  = 'https://api.github.com/repos/BtbN/FFmpeg-Builds/releases/latest'
        $json = & $CurlExe -fsSL $api 2>$null
        $url  = ($json | ConvertFrom-Json).assets |
                Where-Object { $_.name -like '*win64-gpl.zip' -and $_.name -like '*master*' } |
                Select-Object -First 1 -ExpandProperty browser_download_url
        if (-not $url) { throw 'Could not resolve FFmpeg download URL from GitHub API' }
    } catch {
        "WARN: FFmpeg failed -- $_"
        return
    }
    try {
        if ($HasCurl) {
            & $CurlExe @CurlBase -o $zip $url 2>&1 | Out-Null
            if ($LASTEXITCODE -ne 0) { throw "curl exit $LASTEXITCODE" }
        } else {
            Invoke-WebRequest -Uri $url -OutFile $zip -UseBasicParsing -TimeoutSec 300
        }
        # Extract only ffmpeg.exe + ffprobe.exe
        Add-Type -AssemblyName System.IO.Compression.FileSystem
        $archive = [System.IO.Compression.ZipFile]::OpenRead($zip)
        foreach ($entry in $archive.Entries) {
            if ($entry.Name -eq 'ffmpeg.exe' -or $entry.Name -eq 'ffprobe.exe') {
                $out = Join-Path $BinDir $entry.Name
                $src = $entry.Open()
                $dst = [System.IO.File]::Create($out)
                $src.CopyTo($dst)
                $dst.Close(); $src.Close()
            }
        }
        $archive.Dispose()
        Remove-Item $zip -Force -ErrorAction SilentlyContinue
        "OK: ffmpeg.exe + ffprobe.exe -> $BinDir"
    } catch {
        Remove-Item $zip -Force -ErrorAction SilentlyContinue
        "WARN: FFmpeg failed -- $_"
    }
} -ArgumentList $BinDir, $CurlExe, $HasCurl, $CurlBase

$jobGalleryDl = Start-Job -ScriptBlock {
    param($BinDir, $CurlExe, $HasCurl, $CurlBase)
    $dest = Join-Path $BinDir 'gallery-dl.exe'
    # gallery-dl stopped shipping exe assets from v1.32+; find latest release that has one
    try {
        $api      = 'https://api.github.com/repos/mikf/gallery-dl/releases'
        $releases = (& $CurlExe -fsSL $api 2>$null | ConvertFrom-Json)
        $asset    = $releases | ForEach-Object { $_.assets } |
                    Where-Object { $_.name -eq 'gallery-dl.exe' } |
                    Select-Object -First 1
        $url = $asset.browser_download_url
        if (-not $url) { throw 'No gallery-dl.exe asset found in any release' }
    } catch {
        "WARN: gallery-dl failed -- $_"
        return
    }
    try {
        if ($HasCurl) {
            & $CurlExe @CurlBase -o $dest $url 2>&1 | Out-Null
            if ($LASTEXITCODE -ne 0) { throw "curl exit $LASTEXITCODE" }
        } else {
            Invoke-WebRequest -Uri $url -OutFile $dest -UseBasicParsing -TimeoutSec 180
        }
        "OK: gallery-dl -> $dest"
    } catch { "WARN: gallery-dl failed -- $_" }
} -ArgumentList $BinDir, $CurlExe, $HasCurl, $CurlBase

# --- Wait + log --------------------------------------------------------------
$ts = Get-Date -Format 'yyyy-MM-dd HH:mm:ss'
Add-Content $LogFile "[$ts] Starting parallel downloads..."

$jobs = @($jobYtDlp, $jobFfmpeg, $jobGalleryDl)
$jobs | Wait-Job -Timeout 600 | Out-Null   # 10 min max

foreach ($job in $jobs) {
    $out = Receive-Job $job -ErrorAction SilentlyContinue
    if ($out) {
        $line = "[$(Get-Date -Format 'HH:mm:ss')] $out"
        Write-Host $line
        Add-Content $LogFile $line -ErrorAction SilentlyContinue
    }
    Remove-Job $job -Force -ErrorAction SilentlyContinue
}

Add-Content $LogFile "[$(Get-Date -Format 'HH:mm:ss')] Done."
exit 0

