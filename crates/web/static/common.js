/* Preferences: theme, density, refresh interval (localStorage, no auth). */
(function () {
  var THEME_KEY = 'rusthome-theme';
  var DENSITY_KEY = 'rusthome-density';
  var REFRESH_MS_KEY = 'rusthome-refresh-ms';
  var JOURNAL_LIMIT_KEY = 'rusthome-journal-limit';

  var ALLOWED_REFRESH = [0, 2000, 4000, 10000];
  var ALLOWED_JOURNAL = [20, 40, 80, 120];

  function applyDark() {
    document.documentElement.setAttribute('data-theme', 'dark');
    document.documentElement.style.colorScheme = 'dark';
  }
  function applyLight() {
    document.documentElement.setAttribute('data-theme', 'light');
    document.documentElement.style.colorScheme = 'light';
  }
  function applySystem() {
    document.documentElement.removeAttribute('data-theme');
    document.documentElement.style.colorScheme = '';
  }

  var savedTheme = localStorage.getItem(THEME_KEY);
  if (savedTheme === 'dark') applyDark();
  else if (savedTheme === 'light') applyLight();
  else applySystem();

  function applyDensity(mode) {
    if (mode === 'compact') {
      document.documentElement.setAttribute('data-density', 'compact');
    } else {
      document.documentElement.removeAttribute('data-density');
    }
  }

  var savedDensity = localStorage.getItem(DENSITY_KEY);
  if (savedDensity === 'compact') applyDensity('compact');

  window.rhSetTheme = function (mode) {
    if (mode === 'system') {
      localStorage.removeItem(THEME_KEY);
      applySystem();
    } else if (mode === 'dark') {
      localStorage.setItem(THEME_KEY, 'dark');
      applyDark();
    } else if (mode === 'light') {
      localStorage.setItem(THEME_KEY, 'light');
      applyLight();
    }
    syncThemeSelect();
  };

  window.rhSetDensity = function (mode) {
    if (mode === 'compact') {
      localStorage.setItem(DENSITY_KEY, 'compact');
      applyDensity('compact');
    } else {
      localStorage.removeItem(DENSITY_KEY);
      applyDensity('comfortable');
    }
    syncDensitySelect();
  };

  window.rhGetRefreshIntervalMs = function () {
    var v = localStorage.getItem(REFRESH_MS_KEY);
    if (v === null) return 4000;
    var n = parseInt(v, 10);
    if (isNaN(n)) return 4000;
    for (var i = 0; i < ALLOWED_REFRESH.length; i++) {
      if (ALLOWED_REFRESH[i] === n) return n;
    }
    return 4000;
  };

  window.rhGetJournalLimit = function (defaultLimit) {
    var v = localStorage.getItem(JOURNAL_LIMIT_KEY);
    if (v === null) return defaultLimit;
    var n = parseInt(v, 10);
    for (var i = 0; i < ALLOWED_JOURNAL.length; i++) {
      if (ALLOWED_JOURNAL[i] === n) return n;
    }
    return defaultLimit;
  };

  function syncThemeSelect() {
    var sel = document.getElementById('theme-select');
    if (!sel) return;
    var v = localStorage.getItem(THEME_KEY);
    sel.value = v === 'light' || v === 'dark' ? v : 'system';
  }

  function syncDensitySelect() {
    var sel = document.getElementById('density-select');
    if (!sel) return;
    sel.value = localStorage.getItem(DENSITY_KEY) === 'compact' ? 'compact' : 'comfortable';
  }

  function syncRefreshSelect() {
    var sel = document.getElementById('refresh-interval-select');
    if (!sel) return;
    sel.value = String(window.rhGetRefreshIntervalMs());
  }

  function syncJournalLimitSelect() {
    var sel = document.getElementById('journal-limit-select');
    if (!sel) return;
    var def = 40;
    var cfg = document.getElementById('rh-dashboard-config');
    if (cfg) {
      try {
        var o = JSON.parse(cfg.textContent);
        if (o && typeof o.journalLimit === 'number') def = o.journalLimit;
      } catch (e) { /* ignore */ }
    }
    var lim = window.rhGetJournalLimit(def);
    sel.value = String(lim);
  }

  function bindThemeSelect() {
    var sel = document.getElementById('theme-select');
    if (!sel) return;
    syncThemeSelect();
    sel.addEventListener('change', function () {
      window.rhSetTheme(sel.value);
    });
  }

  function bindDensitySelect() {
    var sel = document.getElementById('density-select');
    if (!sel) return;
    syncDensitySelect();
    sel.addEventListener('change', function () {
      window.rhSetDensity(sel.value === 'compact' ? 'compact' : 'comfortable');
    });
  }

  function bindRefreshSelect() {
    var sel = document.getElementById('refresh-interval-select');
    if (!sel) return;
    syncRefreshSelect();
    sel.addEventListener('change', function () {
      var ms = parseInt(sel.value, 10);
      if (isNaN(ms)) ms = 4000;
      localStorage.setItem(REFRESH_MS_KEY, String(ms));
      window.dispatchEvent(new CustomEvent('rh-refresh-interval-changed', { detail: { ms: ms } }));
    });
  }

  function bindJournalLimitSelect() {
    var sel = document.getElementById('journal-limit-select');
    if (!sel) return;
    syncJournalLimitSelect();
    sel.addEventListener('change', function () {
      var n = parseInt(sel.value, 10);
      localStorage.setItem(JOURNAL_LIMIT_KEY, String(n));
      window.dispatchEvent(new CustomEvent('rh-journal-limit-changed', { detail: { limit: n } }));
    });
  }

  function initPrefs() {
    bindThemeSelect();
    bindDensitySelect();
    bindRefreshSelect();
    bindJournalLimitSelect();
  }

  initPrefs();
})();
