# bin/ — Required Executables

Place the following executables in this directory before installing:

| File | Where to get |
|------|-------------|
| `yt-dlp.exe` | https://github.com/yt-dlp/yt-dlp/releases/latest |
| `ffmpeg.exe` | https://www.gyan.dev/ffmpeg/builds/ (ffmpeg-release-essentials) |
| `ffprobe.exe` | Included in the same FFmpeg archive as `ffmpeg.exe` |

## Quick Download Steps

### yt-dlp
1. Go to https://github.com/yt-dlp/yt-dlp/releases/latest
2. Download `yt-dlp.exe`
3. Place it in this `bin/` folder

### FFmpeg
1. Go to https://www.gyan.dev/ffmpeg/builds/
2. Download `ffmpeg-release-essentials.zip`
3. Extract and copy `ffmpeg.exe` and `ffprobe.exe` from the `bin/` folder inside the archive into this `bin/` folder

## After Placing Executables

Run:

```
paste-link-downloader.exe --install
```

This copies the application (including this `bin/` directory) to:
```
%LOCALAPPDATA%\PasteLinkDownloader\
```
and registers the Explorer context menu.

## Verification

```
paste-link-downloader.exe --diagnostics
```

Output should show `✓` for both yt-dlp and FFmpeg.
