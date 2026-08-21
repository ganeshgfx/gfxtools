/**
 * main.js
 *
 * Panel UI logic — wires DOM elements to VideoYoinker.download()
 * and calls ExtendScript via CSInterface to import the file.
 */

(function () {
  'use strict';

  var fs   = require('fs');

  // ── CEP eval bridge ──────────────────────────────────────────────────────
  // window.cs      = CSInterface — browser context only
  // window.__adobe_cep__ = native CEP bridge — available in BOTH contexts
  // Use evalCEP() everywhere so it works whether called from DOM handlers
  // OR from Node.js child_process callbacks.
  function evalCEP(script, callback) {
    if (window.cs && typeof window.cs.evalScript === 'function') {
      window.cs.evalScript(script, callback);
    } else if (window.__adobe_cep__ && typeof window.__adobe_cep__.evalScript === 'function') {
      window.__adobe_cep__.evalScript(script, callback);
    } else {
      // Dev stub: not inside Adobe
      console.log('[stub evalCEP]', script);
      if (callback) callback('');
    }
  }

  // ── State ─────────────────────────────────────────────────────────────────
  var isDownloading = false;
  var currentProcess = null;
  var hostApp = 'Unknown';

  // ── DOM refs ──────────────────────────────────────────────────────────────
  var urlInput      = document.getElementById('url-input');
  var pasteBtn      = document.getElementById('paste-btn');
  var outputDirEl   = document.getElementById('output-dir');
  var browseBtn     = document.getElementById('browse-btn');
  var formatSelect  = document.getElementById('format-select');
  var binNameInput  = document.getElementById('bin-name');
  var downloadBtn   = document.getElementById('download-btn');
  var downloadText  = document.getElementById('download-btn-text');
  var cancelBtn     = document.getElementById('cancel-btn');
  var urlValidation = document.getElementById('url-validation');
  var progressFill  = document.getElementById('progress-fill');
  var progressGlow  = document.getElementById('progress-glow');
  var progressPct   = document.getElementById('progress-pct');
  var progressLabel = document.getElementById('progress-label');
  var logOutput     = document.getElementById('log-output');
  var clearLogBtn   = document.getElementById('clear-log-btn');
  var statusBadge   = document.getElementById('status-badge');
  var hostNameEl    = document.getElementById('host-name');
  var diagLink      = document.getElementById('diag-link');

  // ── Default output dir (fallback only) ─────────────────────────────────
  // Avoid require('os') / require('path') which can fail in some CEP versions.
  var defaultOutputDir = (process.env.USERPROFILE || process.env.HOME || 'C:\\Users\\Public') +
                         '\\Videos\\VideoYoinker';

  function ensureOutputDir(dir) {
    try { fs.mkdirSync(dir, { recursive: true }); } catch (e) {}
  }

  function setOutputDir(dir) {
    ensureOutputDir(dir);
    outputDirEl.value = dir;
    outputDirEl.title = dir;
  }

  /**
   * Set fallback dir IMMEDIATELY (sync), then ask ExtendScript for the real
   * project folder and upgrade the value if a valid path comes back.
   */
  function resolveOutputDir(callback) {
    // Sync fallback — user always has a valid dir, even if evalScript fails
    setOutputDir(defaultOutputDir);

    // Try to get project folder from ExtendScript (async upgrade)
    evalCEP('getProjectFolder()', function (result) {
      var r = result ? result.trim() : '';
      // Reject empty, 'undefined', and ExtendScript error strings
      var isValid = r && r !== 'undefined' && r.indexOf('EvalScript') === -1 &&
                    r.indexOf('Error') === -1 && r.length > 2;
      if (isValid) {
        setOutputDir(r);
        log('info', 'Project folder: ' + r);
      } else {
        log('dim', 'No open project — saving to: ' + defaultOutputDir);
      }
      if (callback) callback(outputDirEl.value);
    });
  }

  // ── Logging ───────────────────────────────────────────────────────────────
  function log(level, msg) {
    var entry = document.createElement('span');
    entry.className = 'log-entry ' + (level || 'dim');
    entry.textContent = msg;
    logOutput.appendChild(entry);
    logOutput.appendChild(document.createTextNode('\n'));
    logOutput.scrollTop = logOutput.scrollHeight;
  }

  // ── URL validation ────────────────────────────────────────────────────────
  function validateUrl(val) {
    val = val.trim();
    if (!val) {
      setValidation('', '');
      return false;
    }
    try {
      var u = new URL(val);
      if (u.protocol !== 'http:' && u.protocol !== 'https:') {
        setValidation('error', '✗ Must be http:// or https://');
        return false;
      }
      setValidation('ok', '✓ Valid URL');
      return true;
    } catch (e) {
      setValidation('error', '✗ Not a valid URL');
      return false;
    }
  }

  function setValidation(cls, msg) {
    urlValidation.className = 'validation-msg ' + cls;
    urlValidation.textContent = msg;
  }

  // ── Progress ──────────────────────────────────────────────────────────────
  function setProgress(pct) {
    var p = Math.min(100, Math.max(0, pct));
    progressFill.style.width = p + '%';
    progressGlow.style.left  = p + '%';
    progressPct.textContent  = Math.round(p) + '%';
    if (p > 0 && p < 100) {
      progressGlow.classList.add('active');
    } else {
      progressGlow.classList.remove('active');
    }
  }

  // ── Status badge ──────────────────────────────────────────────────────────
  function setBadge(text, cls) {
    statusBadge.textContent = text;
    statusBadge.className = 'header-badge ' + (cls || '');
  }

  function importIntoProject(filePath, binName) {
    var escapedPath = filePath.replace(/\\/g, '\\\\');
    var escapedBin  = binName.replace(/'/g, "\\'");

    function runImport(appId) {
      var script;
      var id = (appId || '').toUpperCase();
      if (id === 'PPRO' || id.indexOf('PREMIERE') !== -1) {
        script = "ppro_importFile('" + escapedPath + "', '" + escapedBin + "')";
      } else if (id === 'AEFT' || id.indexOf('AFTER') !== -1) {
        script = "aeft_importFile('" + escapedPath + "', '" + escapedBin + "')";
      } else {
        // Unknown host — try Premiere first (most common), fall back to AE on error
        log('warn', 'Host "' + appId + '" unclear — trying Premiere Pro import…');
        script = "ppro_importFile('" + escapedPath + "', '" + escapedBin + "')";
      }

      log('info', 'Importing into "' + binName + '" bin/folder…');
      evalCEP(script, function (result) {
        if (result === 'OK') {
          log('ok', '✓ Imported into project bin "' + binName + '".');
        } else if (result === 'NO_PROJECT') {
          log('warn', '⚠ No open project — file saved to disk but not imported.');
        } else if (result && result.indexOf('Error') !== -1) {
          // Premiere failed — try AE
          log('warn', 'Premiere import failed, trying After Effects…');
          var aeftScript = "aeft_importFile('" + escapedPath + "', '" + escapedBin + "')";
          evalCEP(aeftScript, function (r2) {
            if (r2 === 'OK') {
              log('ok', '✓ Imported (AE) into folder "' + binName + '".');
            } else {
              log('warn', '⚠ Import result: ' + r2);
            }
          });
        } else {
          log('warn', '⚠ Import returned: ' + result);
        }
      });
    }

    // If host detection worked, use it directly
    if (!evalCEP) {
      log('warn', 'CEP not available — file saved to disk but not imported.');
      log('info', 'File: ' + filePath);
      return;
    }
    if (hostApp && hostApp !== 'Unknown') {
      runImport(hostApp);
    } else {
      // Last resort: ask ExtendScript for app.name at import time
      evalCEP('app.name', function (name) {
        hostApp = name || 'PPRO';
        hostNameEl.textContent = hostApp;
        runImport(hostApp);
      });
    }
  }


  /**
   * After download completes, find the most recently modified file
   * in the output directory to pass to importIntoProject.
   */
  function findLatestFile(dir) {
    try {
      var files = fs.readdirSync(dir)
        .map(function (f) {
          var full = path.join(dir, f);
          return { name: full, mtime: fs.statSync(full).mtimeMs };
        })
        .filter(function (f) { return !fs.statSync(f.name).isDirectory(); })
        .sort(function (a, b) { return b.mtime - a.mtime; });
      return files.length ? files[0].name : null;
    } catch (e) {
      return null;
    }
  }

  // ── Download flow ─────────────────────────────────────────────────────────
  function startDownload() {
    var url = urlInput.value.trim();
    if (!validateUrl(url)) {
      log('error', 'Invalid URL — please enter a valid https:// link.');
      return;
    }

    var outputDir = outputDirEl.value.trim();
    if (!outputDir) {
      log('error', 'No output directory set.');
      return;
    }

    var format  = formatSelect.value;
    var binName = binNameInput.value.trim() || 'Downloaded';

    ensureOutputDir(outputDir);

    isDownloading = true;
    setProgress(0);
    setBadge('Downloading…', 'downloading');
    downloadBtn.disabled = true;
    downloadText.textContent = 'Downloading…';
    cancelBtn.classList.remove('hidden');
    log('info', '─── Starting download ───');

    // Snapshot dir contents before download to detect new file
    var beforeFiles = new Set();
    try {
      fs.readdirSync(outputDir).forEach(function (f) { beforeFiles.add(f); });
    } catch (e) {}

    currentProcess = VideoYoinker.download(url, outputDir, format, {
      onProgress: function (pct) { setProgress(pct); },
      onLog:      function (level, msg) { log(level, msg); },

      onComplete: function (dir) {
        isDownloading = false;
        setProgress(100);
        setBadge('Done ✓', 'success');
        downloadBtn.disabled = false;
        downloadText.textContent = 'Download & Import';
        cancelBtn.classList.add('hidden');

        // Detect new file and import — deferred to browser event loop
        // (onComplete fires from Node.js child process callback; window.cs
        // is only accessible in the browser/DOM context, so we use setTimeout)
        var capturedDir = dir;
        setTimeout(function () {
          try {
            var afterFiles = fs.readdirSync(capturedDir);
            var newFile = afterFiles.find(function (f) { return !beforeFiles.has(f); });
            if (newFile) {
              var sep = capturedDir.slice(-1) === '\\' ? '' : '\\';
              var fullPath = capturedDir + sep + newFile;
              importIntoProject(fullPath, binName);
            } else {
              log('warn', 'Could not detect new file for import (already existed?).');
            }
          } catch (e) {
            log('warn', 'Post-download scan error: ' + e.message);
          }
        }, 500); // small delay to let yt-dlp fully release the file

        setTimeout(function () { setBadge('Ready'); }, 4000);
      },

      onError: function (msg) {
        isDownloading = false;
        setBadge('Error', 'error');
        downloadBtn.disabled = false;
        downloadText.textContent = 'Download & Import';
        cancelBtn.classList.add('hidden');
        log('error', '✗ ' + msg);
        setTimeout(function () { setBadge('Ready'); }, 6000);
      },
    });
  }

  // ── Event listeners ───────────────────────────────────────────────────────

  // Paste from clipboard button
  pasteBtn.addEventListener('click', function () {
    // Read clipboard via CEP / navigator API
    if (navigator.clipboard && navigator.clipboard.readText) {
      navigator.clipboard.readText().then(function (text) {
        urlInput.value = text.trim();
        validateUrl(text.trim());
      }).catch(function () {
        log('warn', 'Clipboard read failed — paste manually.');
      });
    }
  });

  // URL validation on input
  urlInput.addEventListener('input', function () {
    validateUrl(urlInput.value);
  });

  // Output dir: clicking field refreshes to project folder
  outputDirEl.addEventListener('click', function () {
    resolveOutputDir();
  });

  // Browse for output directory (override)
  browseBtn.addEventListener('click', function () {
    evalCEP('pickFolder()', function (result) {
      if (result && result.trim() && result !== 'undefined') {
        outputDirEl.value = result.trim();
        outputDirEl.title = result.trim();
        ensureOutputDir(result.trim());
        log('info', 'Output dir overridden: ' + result.trim());
      }
    });
  });

  // Download button
  downloadBtn.addEventListener('click', function () {
    if (!isDownloading) startDownload();
  });

  // Cancel button — kill the child process
  cancelBtn.addEventListener('click', function () {
    if (currentProcess && currentProcess.process) {
      try { currentProcess.process.kill(); } catch (e) {}
    }
    isDownloading = false;
    setBadge('Cancelled', 'error');
    downloadBtn.disabled = false;
    downloadText.textContent = 'Download & Import';
    cancelBtn.classList.add('hidden');
    log('warn', '⚠ Cancelled by user.');
    setTimeout(function () { setBadge('Ready'); }, 3000);
  });

  // Clear log
  clearLogBtn.addEventListener('click', function () {
    logOutput.innerHTML = '';
  });

  // Diagnostics link
  diagLink.addEventListener('click', function (e) {
    e.preventDefault();
    var exePath = VideoYoinker.resolveExe();
    if (!exePath) {
      log('error', 'Exe not found for diagnostics.');
      return;
    }
    var child = require('child_process').spawn(exePath, ['--diagnostics'], {
      stdio: ['ignore', 'pipe', 'pipe'],
      windowsHide: true,
    });
    child.stdout.setEncoding('utf8');
    child.stdout.on('data', function (d) {
      d.split('\n').forEach(function (l) { if (l.trim()) log('info', l); });
    });
    child.stderr.setEncoding('utf8');
    child.stderr.on('data', function (d) {
      d.split('\n').forEach(function (l) { if (l.trim()) log('warn', l); });
    });
  });

  // ── Init ──────────────────────────────────────────────────────────────────
  document.addEventListener('cep-ready', function () {
    // Method 1: getApplicationID via __adobe_cep__ (available in both contexts)
    try {
      var appId = window.__adobe_cep__
        ? window.__adobe_cep__.getApplicationID()
        : (window.cs && window.cs.getApplicationID ? window.cs.getApplicationID() : '');
      if (appId && appId !== 'undefined' && appId.trim()) {
        hostApp = appId.trim();
      }
    } catch (e) {}

    // Method 2: getHostEnvironment JSON fallback
    if (!hostApp || hostApp === 'Unknown') {
      try {
        var rawEnv = window.__adobe_cep__
          ? window.__adobe_cep__.getHostEnvironment()
          : (window.cs ? window.cs.getHostEnvironment() : null);
        if (rawEnv) {
          var env = JSON.parse(rawEnv);
          hostApp = env.appName || env.appId || 'Unknown';
        }
      } catch (e) {}
    }

    // Update subtitle
    var displayName = hostApp === 'PPRO' ? 'Premiere Pro' :
                      hostApp === 'AEFT' ? 'After Effects' : hostApp;
    hostNameEl.textContent = displayName || 'Adobe Host';
    hostNameEl.style.opacity = '1';

    resolveOutputDir(function () {
      log('info', 'Video Yoinker ready. Host: ' + hostApp);
      // Check binary is findable
      var exe = VideoYoinker.resolveExe();
      if (exe) {
        log('ok', '\u2713 Found: ' + exe);
      } else {
        log('error', '\u2717 paste-link-downloader.exe not found!');
        log('warn', 'Place it in plugin/bin/ or run --install first.');
      }
    });
  });

})();
