# GFX Tools — Adobe CEP Plugin

Adds a **GFX Tools** panel to **Premiere Pro** and **After Effects**.  
Paste a URL → download video via yt-dlp → auto-import into a **"Downloaded"** bin.

---

## Prerequisites

| Requirement | Notes |
|---|---|
| Adobe Premiere Pro **CC 2019+** or After Effects **CC 2019+** | |
| `gfx-tools.exe` | Built from this repo |
| `yt-dlp.exe` | In `plugin/bin/` or on PATH |
| `ffmpeg.exe` | In `plugin/bin/` or on PATH |

---

## Step 1 — Build the Rust binary

```powershell
cd d:\tools\video_yoinker
cargo build --release
```

The exe is at `target\release\gfx-tools.exe`.

---

## Step 2 — Populate `plugin/bin/`

```
plugin\bin\
    gfx-tools.exe   ← copy from target\release\
    yt-dlp.exe                  ← download from https://github.com/yt-dlp/yt-dlp/releases
    ffmpeg.exe                  ← download from https://ffbinaries.com/downloads
    ffprobe.exe                 ← same source as ffmpeg
```

---

## Step 3 — Install the plugin

### Option A — Debug mode (personal use, no signing required)

1. Enable CEP player debug mode:

```powershell
reg add "HKCU\Software\Adobe\CSXS.11" /v PlayerDebugMode /t REG_SZ /d 1 /f
```

2. Copy the `plugin\` folder to the CEP extensions directory:

```powershell
$dest = "$env:APPDATA\Adobe\CEP\extensions\GFXTools"
Copy-Item -Recurse -Force "d:\tools\video_yoinker\plugin" $dest
```

3. Open Premiere Pro or After Effects.

4. Go to **Window → Extensions → GFX Tools**.

### Option B — ZXP installer (future)

Package with `ZXPSignCmd` and distribute a signed `.zxp`.  
Not required for personal use with debug mode enabled.

---

## Usage

1. Open a project in Premiere Pro or After Effects.
2. Open the **GFX Tools** panel (Window → Extensions → GFX Tools).
3. Paste a video URL (YouTube, Instagram, Twitter, etc.).
4. Choose output directory and format.
5. Click **Download & Import**.
6. The video downloads and appears in the **Downloaded** bin automatically.

> **Note:** The panel writes the URL to your clipboard before calling the downloader,  
> then restores clipboard state. This is a current limitation of the Rust binary's interface.

---

## Troubleshooting

| Problem | Fix |
|---|---|
| Panel doesn't appear | Verify debug mode reg key; restart Adobe app |
| `gfx-tools.exe not found` | Ensure `plugin\bin\gfx-tools.exe` exists |
| Import fails with "NO_PROJECT" | Open a project before downloading |
| yt-dlp errors | Run Diagnostics link in panel footer |

---

## File Layout

```
plugin\
├── CSXS\
│   └── manifest.xml          ← CEP bundle manifest
├── index.html                ← Panel UI
├── css\
│   └── panel.css             ← Dark theme styles
├── js\
│   ├── cep_init.js           ← Loads CSInterface
│   ├── downloader.js         ← Spawns Rust binary
│   ├── main.js               ← UI controller
│   └── lib\
│       └── CSInterface.js    ← Copy from Adobe-CEP GitHub (see below)
├── jsx\
│   └── host.jsx              ← ExtendScript: bin creation + file import
└── bin\
    ├── gfx-tools.exe
    ├── yt-dlp.exe
    ├── ffmpeg.exe
    └── ffprobe.exe
```

### Download CSInterface.js

```powershell
Invoke-WebRequest `
  "https://raw.githubusercontent.com/Adobe-CEP/CEP-Resources/master/CEP_11.x/CSInterface.js" `
  -OutFile "plugin\js\lib\CSInterface.js"
```
