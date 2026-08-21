/**
 * downloader.js
 *
 * Bridges the panel UI to paste-link-downloader.exe via Node.js child_process.
 *
 * KEY INSIGHT: The Rust binary reads the URL from the Windows clipboard,
 * not from a CLI argument. So before spawning the process we must write
 * the URL to the clipboard ourselves.
 *
 * Interface:
 *   VideoYoinker.download(url, outputDir, format, callbacks)
 *
 * Callbacks object:
 *   { onProgress, onLog, onComplete, onError, onCancel }
 */

(function (global) {
  'use strict';

  // ── Node.js integration (available inside CEP panels) ────────────────────
  var path    = require('path');
  var fs      = require('fs');
  var spawn   = require('child_process').spawn;
  var exec    = require('child_process').exec;

  // ── Locate the Rust binary ───────────────────────────────────────────────
  /**
   * Resolve paste-link-downloader.exe.
   * Search order:
   *   1. plugin/bin/paste-link-downloader.exe  (bundled)
   *   2. LOCALAPPDATA\PasteLinkDownloader\paste-link-downloader.exe  (installed)
   */
  function resolveExe() {
    // __dirname is the plugin root when loaded via CEP
    var pluginRoot = path.dirname(path.dirname(__filename)); // js/ → plugin/
    var bundled = path.join(pluginRoot, 'bin', 'paste-link-downloader.exe');
    if (fs.existsSync(bundled)) return bundled;

    var localAppData = process.env.LOCALAPPDATA || '';
    var installed = path.join(localAppData, 'PasteLinkDownloader', 'paste-link-downloader.exe');
    if (fs.existsSync(installed)) return installed;

    return null;
  }

  // ── Write text to Windows clipboard ─────────────────────────────────────────
  function writeClipboard(text, callback) {
    // Build a small PS script that sets the clipboard.
    // Single-quote the URL (escape embedded single-quotes as '').
    var safeText = text.replace(/'/g, "''");
    var psScript = "Set-Clipboard -Value '" + safeText + "'";

    // Encode the script as UTF-16LE Base64 and pass via -EncodedCommand.
    // This completely avoids shell quoting problems — the encoded blob
    // contains no characters that need escaping on the cmd/pwsh command line.
    var encoded = Buffer.from(psScript, 'utf16le').toString('base64');

    exec(
      'powershell.exe -NoProfile -NonInteractive -EncodedCommand ' + encoded,
      function (err) { callback(err); }
    );
  }

  // ── Progress line parser ─────────────────────────────────────────────────
  /**
   * Parse a yt-dlp stdout line into { type, value }.
   * Mirrors the Rust progress.rs parser.
   */
  function parseLine(line) {
    var l = line.trim();

    // [download]  73.4% of ...
    var pctMatch = l.match(/\[download\]\s+([\d.]+)%/);
    if (pctMatch) return { type: 'percent', value: parseFloat(pctMatch[1]) };

    // [download]  100% of ...
    if (l.match(/\[download\]\s+100%/)) return { type: 'complete' };

    // Merging formats
    if (l.match(/\[Merger\]|Merging formats/i)) return { type: 'merging', value: l };

    // WARNING / ERROR
    if (l.startsWith('WARNING:')) return { type: 'warn', value: l };
    if (l.startsWith('ERROR:'))   return { type: 'error', value: l };

    return { type: 'other', value: l };
  }

  // ── Main download function ───────────────────────────────────────────────
  /**
   * @param {string}   url        - Video URL
   * @param {string}   outputDir  - Absolute path to output directory
   * @param {string}   format     - e.g. "mp4"
   * @param {object}   cb         - Callbacks
   */
  function download(url, outputDir, format, cb) {
    cb = cb || {};
    var onProgress = cb.onProgress || function () {};
    var onLog      = cb.onLog      || function () {};
    var onComplete = cb.onComplete || function () {};
    var onError    = cb.onError    || function () {};

    var exePath = resolveExe();
    if (!exePath) {
      onError('paste-link-downloader.exe not found.\n' +
              'Place it in plugin/bin/ or install it via --install.');
      return null;
    }

    onLog('info', 'Binary: ' + exePath);
    onLog('info', 'URL: ' + url);
    onLog('info', 'Output dir: ' + outputDir);

    // Step 1: Write URL to clipboard, then spawn
    onLog('dim', 'Writing URL to clipboard…');
    writeClipboard(url, function (err) {
      if (err) {
        onError('Failed to write clipboard: ' + err.message);
        return;
      }
      onLog('dim', 'Clipboard set. Spawning downloader…');
      spawnDownloader(exePath, outputDir, onProgress, onLog, onComplete, onError);
    });

    // Return a cancel handle (will be populated after spawn)
    var handle = { cancelled: false, process: null };
    return handle;
  }

  function spawnDownloader(exePath, outputDir, onProgress, onLog, onComplete, onError) {
    var child = spawn(exePath, [outputDir], {
      stdio: ['ignore', 'pipe', 'pipe'],
      windowsHide: true, // don't flash a console window
    });

    child.stdout.setEncoding('utf8');
    child.stderr.setEncoding('utf8');

    var stdoutBuf = '';
    child.stdout.on('data', function (chunk) {
      stdoutBuf += chunk;
      var lines = stdoutBuf.split('\n');
      stdoutBuf = lines.pop(); // keep incomplete last line
      lines.forEach(function (line) {
        if (!line.trim()) return;
        var ev = parseLine(line);
        onLog('dim', line);
        if (ev.type === 'percent')  onProgress(ev.value);
        if (ev.type === 'complete') onProgress(100);
        if (ev.type === 'merging')  onLog('info', ev.value);
        if (ev.type === 'warn')     onLog('warn', ev.value);
        if (ev.type === 'error')    onLog('error', ev.value);
      });
    });

    child.stderr.on('data', function (chunk) {
      chunk.split('\n').forEach(function (line) {
        if (line.trim()) onLog('dim', '[stderr] ' + line);
      });
    });

    child.on('close', function (code) {
      if (code === 0) {
        onProgress(100);
        onLog('ok', '✓ Download complete!');
        onComplete(outputDir);
      } else {
        onError('yt-dlp exited with code ' + code +
                '. Check the log for details.');
      }
    });

    child.on('error', function (err) {
      onError('Failed to spawn process: ' + err.message);
    });

    return child;
  }

  // ── Exposed API ──────────────────────────────────────────────────────────
  global.VideoYoinker = {
    download: download,
    resolveExe: resolveExe,
  };

})(window);
