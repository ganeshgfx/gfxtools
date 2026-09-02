<#
.SYNOPSIS
    Downloads yt-dlp, FFmpeg (essentials), and gallery-dl into the bin\ directory.

.DESCRIPTION
    Called automatically by the Inno Setup installer after placing gfx-tools.exe.
    Downloads run in parallel via background jobs. Uses curl.exe (built into
    Windows 10+) with resume support and retries. All downloads are best-effort --
    a single failure does not abort the installer.

    Speed improvements over previous version:
      - FFmpeg: switched from BtbN master-build (~163 MB zip) to Gyan.dev
        release-essentials (~8 MB zip). No GitHub API call needed -- stable URL.
      - gallery-dl: API query uses per_page=5 pagination and stops on first hit.
      - Progress heartbeat written to log every 3s showing KB received per tool.

    Tools land in:  <InstallDir>\bin\
      - yt-dlp.exe
      - ffmpeg.exe  + ffprobe.exe
      - gallery-dl.exe

    Pass -InstallDir when launching from Inno Setup (where $PSScriptRoot is empty).
#>
param(
    [string]$InstallDir = '',
    # When set, skip download of any tool whose exe already exists in bin\
    [switch]$SkipIfPresent
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Continue'

$Root    = if ($InstallDir -ne '') { $InstallDir } else { $PSScriptRoot }
$BinDir  = Join-Path $Root 'bin'
$LogFile = Join-Path $Root 'deps-download.log'

if (-not (Test-Path $BinDir)) {
    New-Item -ItemType Directory -Path $BinDir | Out-Null
}

$CurlExe = "$env:SystemRoot\System32\curl.exe"
$HasCurl  = Test-Path $CurlExe

$CurlBase = @('-fSL', '-C', '-', '--retry', '5', '--retry-delay', '3',
              '--retry-connrefused', '--connect-timeout', '30', '--no-progress-meter')

function Log {
    param([string]$Msg)
    $line = "[$(Get-Date -Format 'HH:mm:ss')] $Msg"
    Write-Host $line
    Add-Content $LogFile $line -ErrorAction SilentlyContinue
}

$ts = Get-Date -Format 'yyyy-MM-dd HH:mm:ss'
Add-Content $LogFile "[$ts] Starting parallel downloads..."

# --- yt-dlp ------------------------------------------------------------------
if ($SkipIfPresent -and (Test-Path (Join-Path $BinDir 'yt-dlp.exe'))) {
    Log "SKIP: yt-dlp already present"
    $jobYtDlp = $null
} else {
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
        $size = (Get-Item $dest -ErrorAction SilentlyContinue).Length
        "OK: yt-dlp -> $dest ($([math]::Round($size/1MB,1)) MB)"
    } catch { "WARN: yt-dlp failed -- $_" }
} -ArgumentList $BinDir, $CurlExe, $HasCurl, $CurlBase
} # end yt-dlp block

# --- FFmpeg (multi-source with fallback) ------------------------------------
if ($SkipIfPresent -and (Test-Path (Join-Path $BinDir 'ffmpeg.exe'))) {
    Log "SKIP: FFmpeg already present"
    $jobFfmpeg = $null
} else {
$jobFfmpeg = Start-Job -ScriptBlock {
    param($BinDir, $CurlExe, $HasCurl, $CurlBase)
    $zip = Join-Path $env:TEMP 'ffmpeg-dl.zip'

    # Sources tried in order (fastest/smallest first)
    $sources = @(
        'https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip',
        'https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-lgpl.zip',
        'https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip'
    )

    function TryDownload([string]$url, [string]$dest, [string]$curlExe, [bool]$hasCurl, [array]$curlBase) {
        # Try curl first
        if ($hasCurl) {
            & $curlExe @curlBase -o $dest $url 2>&1 | Out-Null
            if ($LASTEXITCODE -eq 0 -and (Test-Path $dest) -and (Get-Item $dest).Length -gt 1MB) { return $true }
            Remove-Item $dest -Force -ErrorAction SilentlyContinue
        }
        # Fallback: .NET WebClient (handles HTTPS redirects better on some networks)
        try {
            $wc = New-Object System.Net.WebClient
            $wc.Headers.Add('User-Agent', 'GFXTools-Installer/1.0 (curl-fallback)')
            $wc.DownloadFile($url, $dest)
            if ((Test-Path $dest) -and (Get-Item $dest).Length -gt 1MB) { return $true }
        } catch { }
        Remove-Item $dest -Force -ErrorAction SilentlyContinue
        return $false
    }

    $downloaded = $false
    $usedSource = ''
    foreach ($url in $sources) {
        "Trying FFmpeg source: $url"
        if (TryDownload $url $zip $CurlExe $HasCurl $CurlBase) {
            $downloaded = $true
            $usedSource = $url
            break
        }
        "Source failed, trying next..."
    }

    if (-not $downloaded) {
        "WARN: FFmpeg -- all sources failed. Place ffmpeg.exe manually in bin\"
        return
    }

    try {
        Add-Type -AssemblyName System.IO.Compression.FileSystem
        $archive   = [System.IO.Compression.ZipFile]::OpenRead($zip)
        $extracted = @()
        foreach ($entry in $archive.Entries) {
            if ($entry.Name -eq 'ffmpeg.exe' -or $entry.Name -eq 'ffprobe.exe') {
                $out = Join-Path $BinDir $entry.Name
                $src = $entry.Open()
                $dst = [System.IO.File]::Create($out)
                $src.CopyTo($dst)
                $dst.Close(); $src.Close()
                $extracted += $entry.Name
            }
        }
        $archive.Dispose()
        Remove-Item $zip -Force -ErrorAction SilentlyContinue
        "OK: $($extracted -join ' + ') -> $BinDir  [source: $([System.IO.Path]::GetFileName($usedSource))]"
    } catch {
        Remove-Item $zip -Force -ErrorAction SilentlyContinue
        "WARN: FFmpeg extraction failed -- $_"
    }
} -ArgumentList $BinDir, $CurlExe, $HasCurl, $CurlBase
} # end FFmpeg block

# --- gallery-dl (paginated search, stops at first .exe hit) ------------------
if ($SkipIfPresent -and (Test-Path (Join-Path $BinDir 'gallery-dl.exe'))) {
    Log "SKIP: gallery-dl already present"
    $jobGalleryDl = $null
} else {
$jobGalleryDl = Start-Job -ScriptBlock {
    param($BinDir, $CurlExe, $HasCurl, $CurlBase)
    $dest = Join-Path $BinDir 'gallery-dl.exe'
    $url  = $null
    try {
        $page = 1
        while ($null -eq $url -and $page -le 20) {
            $api      = "https://api.github.com/repos/mikf/gallery-dl/releases?per_page=5&page=$page"
            $releases = (& $CurlExe -fsSL $api 2>$null | ConvertFrom-Json)
            if (-not $releases -or $releases.Count -eq 0) { break }
            foreach ($rel in $releases) {
                $asset = $rel.assets | Where-Object { $_.name -eq 'gallery-dl.exe' } | Select-Object -First 1
                if ($asset) { $url = $asset.browser_download_url; break }
            }
            $page++
        }
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
        $size = (Get-Item $dest -ErrorAction SilentlyContinue).Length
        "OK: gallery-dl -> $dest ($([math]::Round($size/1MB,1)) MB)"
    } catch { "WARN: gallery-dl failed -- $_" }
} -ArgumentList $BinDir, $CurlExe, $HasCurl, $CurlBase
} # end gallery-dl block

# --- Progress poll loop ------------------------------------------------------
# Filter out nulls (skipped tools) so poll loop only waits for active jobs
$allJobs   = @($jobYtDlp, $jobFfmpeg, $jobGalleryDl)
$allNames  = @('yt-dlp', 'FFmpeg', 'gallery-dl')
$jobs  = @(); $names = @()
for ($i = 0; $i -lt $allJobs.Count; $i++) {
    if ($null -ne $allJobs[$i]) { $jobs += $allJobs[$i]; $names += $allNames[$i] }
}
$done    = @($jobs | ForEach-Object { $false })
$start   = [datetime]::UtcNow
$timeout = 600  # 10 min hard cap

if ($jobs.Count -eq 0) {
    Log "All tools already present -- nothing to download."
    $null | Set-Content -Path (Join-Path $Root 'deps-download-done.flag') -Encoding ASCII -ErrorAction SilentlyContinue
    exit 0
}

while ($true) {
    Start-Sleep -Seconds 3

    $allDone = $true
    for ($i = 0; $i -lt $jobs.Count; $i++) {
        if ($done[$i]) { continue }
        $j = $jobs[$i]
        if ($j.State -in 'Completed','Failed','Stopped') {
            $out = Receive-Job $j -ErrorAction SilentlyContinue
            if ($out) { Log $out }
            Remove-Job $j -Force -ErrorAction SilentlyContinue
            $done[$i] = $true
        } else {
            $allDone = $false
            $partial = switch ($names[$i]) {
                'yt-dlp'     { Join-Path $BinDir 'yt-dlp.exe' }
                'FFmpeg'     { Join-Path $env:TEMP 'ffmpeg-essentials.zip' }
                'gallery-dl' { Join-Path $BinDir 'gallery-dl.exe' }
            }
            if (Test-Path $partial) {
                $kb = [math]::Round((Get-Item $partial).Length / 1KB)
                Log "$($names[$i]): downloading... ${kb} KB received"
            } else {
                Log "$($names[$i]): resolving..."
            }
        }
    }

    if ($allDone) { break }

    $elapsed = ([datetime]::UtcNow - $start).TotalSeconds
    if ($elapsed -gt $timeout) {
        Log "WARN: timeout reached after ${timeout}s -- cancelling remaining jobs"
        $jobs | Stop-Job -ErrorAction SilentlyContinue
        $jobs | Remove-Job -Force -ErrorAction SilentlyContinue
        break
    }
}

Log "Done."
# Sentinel file — installer polls for this to know downloads finished
$null | Set-Content -Path (Join-Path $Root 'deps-download-done.flag') -Encoding ASCII -ErrorAction SilentlyContinue
exit 0
