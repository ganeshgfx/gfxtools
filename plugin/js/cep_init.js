/**
 * cep_init.js
 * Loads the Adobe CSInterface library and exposes a global `cs` instance.
 *
 * CEP ships CSInterface.js inside the host application. We load it from
 * the standard location, then fall back to a bundled stub for development
 * outside of Adobe apps.
 */
(function () {
  'use strict';

  // Try to load the real CSInterface from the host app's CEP resources.
  // This path is injected by the CEP runtime.
  var csPath = window.__adobe_cep__
    ? JSON.parse(window.__adobe_cep__.getSystemPath('hostEnvironment'))
    : null;

  // We bundle CSInterface.js alongside the plugin for reliability.
  // (Copy from: https://github.com/Adobe-CEP/CEP-Resources/tree/master/CEP_11.x)
  // The script tag below is added dynamically so it blocks until loaded.
  var script = document.createElement('script');
  script.src = 'js/lib/CSInterface.js';
  script.onload = function () {
    window.cs = new CSInterface();
    // Fire a custom event so main.js knows CEP is ready
    document.dispatchEvent(new Event('cep-ready'));
  };
  script.onerror = function () {
    // Running in a normal browser (dev mode) — stub out cs
    console.warn('[GFXTools] CSInterface not available — running in stub mode');
    window.cs = {
      getHostEnvironment: function () {
        return JSON.stringify({ appName: 'DevBrowser', appVersion: '0.0' });
      },
      evalScript: function (script, cb) {
        console.log('[stub evalScript]', script);
        if (cb) cb('');
      },
      openURLInDefaultBrowser: function (url) { window.open(url, '_blank'); },
    };
    document.dispatchEvent(new Event('cep-ready'));
  };
  document.head.appendChild(script);
})();
