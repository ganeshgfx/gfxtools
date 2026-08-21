/**
 * host.jsx  —  ExtendScript host script
 *
 * Loaded by CEP as the ScriptPath entry. Contains helper functions
 * that main.js can call via cs.evalScript().
 *
 * Functions are top-level so evalScript("functionName(args)") works.
 *
 * Both Premiere Pro and After Effects load this same file;
 * functions check the host before running host-specific API calls.
 */

// ── Utility: detect host ─────────────────────────────────────────────────────
function getHostName() {
  if (typeof app !== 'undefined') {
    if (app.name !== undefined) return app.name; // Premiere: "Adobe Premiere Pro"
  }
  return 'Unknown';
}

// ── Premiere Pro: create bin and import file ─────────────────────────────────
/**
 * @param {string} filePath  Absolute OS path to the video file
 * @param {string} binName   Name of the bin to create/find
 * @returns {string}         "OK" | "NO_PROJECT" | error message
 */
function ppro_importFile(filePath, binName) {
  try {
    var proj = app.project;
    if (!proj) return 'NO_PROJECT';

    // Find or create bin
    var bin = null;
    var root = proj.rootItem;
    for (var i = 0; i < root.children.numItems; i++) {
      var child = root.children[i];
      if (child.name === binName && child.type === ProjectItemType.BIN) {
        bin = child;
        break;
      }
    }
    if (!bin) {
      bin = root.createBin(binName);
    }

    // Import into bin
    var result = proj.importFiles([filePath], true, bin, false);
    return result ? 'OK' : 'IMPORT_FAILED';

  } catch (e) {
    return 'ERROR: ' + e.toString();
  }
}

// ── After Effects: create folder item and import file ───────────────────────
/**
 * @param {string} filePath    Absolute OS path to the video file
 * @param {string} folderName  Name of the folder to create/find
 * @returns {string}           "OK" | "NO_PROJECT" | error message
 */
function aeft_importFile(filePath, folderName) {
  try {
    var proj = app.project;
    if (!proj) return 'NO_PROJECT';

    // Find or create folder item
    var folder = null;
    for (var i = 1; i <= proj.numItems; i++) {
      var item = proj.item(i);
      if ((item instanceof FolderItem) && item.name === folderName) {
        folder = item;
        break;
      }
    }
    if (!folder) {
      folder = proj.items.addFolder(folderName);
    }

    // Import file
    var opts = new ImportOptions(File(filePath));
    var imported = proj.importFile(opts);
    imported.parentFolder = folder;
    return 'OK';

  } catch (e) {
    return 'ERROR: ' + e.toString();
  }
}

// ── Get current project folder ───────────────────────────────────────────────
/**
 * Returns the OS folder path of the currently open project file.
 * Returns empty string if no project is open or file not yet saved.
 */
function getProjectFolder() {
  try {
    var proj = app.project;
    if (!proj) return '';

    // Premiere Pro
    if (typeof proj.path !== 'undefined' && proj.path !== '') {
      var f = new File(proj.path);
      return f.parent ? f.parent.fsName : '';
    }

    // After Effects: app.project.file
    if (proj.file) {
      return proj.file.parent ? proj.file.parent.fsName : '';
    }

    return '';
  } catch (e) {
    return '';
  }
}

// ── Folder picker (used by browse button) ────────────────────────────────────
function pickFolder() {
  var f = Folder.selectDialog('Select output folder for downloaded videos');
  return f ? f.fsName : '';
}
