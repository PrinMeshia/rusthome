(function () {
  const $ = (id) => document.getElementById(id);
  const errEl = $('error-bar');
  const btnRefresh = $('btn-refresh');
  const updatedEl = $('updated-at');
  let timer = null;
  let consecutiveFails = 0;
  let lastBtSnapshot = null;

  function fmtBytes(n) {
    const GB = 1024 * 1024 * 1024, MB = 1024 * 1024;
    if (n >= GB) return (n / GB).toFixed(2) + ' GiB';
    if (n >= MB) return (n / MB).toFixed(1) + ' MiB';
    if (n >= 1024) return (n / 1024).toFixed(1) + ' KiB';
    return n + ' B';
  }

  function fmtDuration(secs) {
    const d = Math.floor(secs / 86400);
    const h = Math.floor((secs % 86400) / 3600);
    const m = Math.floor((secs % 3600) / 60);
    const s = secs % 60;
    if (d > 0) return d + 'd ' + h + 'h ' + m + 'm';
    if (h > 0) return h + 'h ' + m + 'm ' + s + 's';
    if (m > 0) return m + 'm ' + s + 's';
    return s + 's';
  }

  function esc(s) {
    return String(s)
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/"/g, '&quot;');
  }

  function trKV(label, valueHtml) {
    return '<tr><th>' + esc(label) + '</th><td>' + valueHtml + '</td></tr>';
  }

  function render(s) {
    const os = s.os_long || s.os_name || '\u2014';
    const journalMeta = s.journal_file_present
      ? (s.journal_file_bytes != null ? 'pr\u00E9sent \u2014 ' + fmtBytes(s.journal_file_bytes) : 'pr\u00E9sent')
      : 'absent (pas encore d\u2019\u00E9v\u00E9nements)';

    $('tbody-rusthome').innerHTML =
      trKV('Service', esc(s.service)) +
      trKV('Version rusthome-web', esc(s.rusthome_version)) +
      trKV('Adresse d\u2019\u00E9coute', esc(s.listen)) +
      trKV('R\u00E9pertoire de donn\u00E9es', esc(s.data_dir)) +
      trKV('Fichier journal', esc(s.journal_path)) +
      trKV('Journal sur disque', esc(journalMeta));

    $('tbody-host').innerHTML =
      trKV('Nom d\u2019h\u00F4te', esc(s.hostname || '\u2014')) +
      trKV('OS', esc(os)) +
      trKV('Noyau', esc(s.kernel || '\u2014')) +
      trKV('Architecture CPU', esc(s.cpu_arch)) +
      trKV('Dur\u00E9e de fonctionnement', esc(fmtDuration(s.uptime_secs))) +
      trKV('Charge moyenne', esc(
        s.load_avg_1.toFixed(2) + ' \u00B7 ' + s.load_avg_5.toFixed(2) + ' \u00B7 ' + s.load_avg_15.toFixed(2) + ' (1 / 5 / 15 min)'
      ));

    const memPct = s.memory_total_bytes > 0
      ? Math.min(100, (s.memory_used_bytes / s.memory_total_bytes) * 100)
      : 0;
    const memRow =
      '<tr><th>M\u00E9moire</th><td>' +
      '<div class="meter-wrap" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow="' +
      Math.round(memPct) + '">' +
      '<div class="meter-fill" style="width:' + memPct.toFixed(1) + '%"></div></div>' +
      '<span class="meter-label">' + esc(fmtBytes(s.memory_used_bytes)) + ' / ' +
      esc(fmtBytes(s.memory_total_bytes)) + ' (' + Math.round(memPct) + '%)</span></td></tr>';

    const swap = s.swap_total_bytes > 0
      ? esc(fmtBytes(s.swap_used_bytes) + ' / ' + fmtBytes(s.swap_total_bytes))
      : '\u2014';

    let disk = '\u2014 (montage introuvable)';
    if (s.disk_mount && s.disk_total_bytes != null && s.disk_available_bytes != null) {
      disk = esc(s.disk_mount) + ' \u2014 ' + esc(fmtBytes(s.disk_available_bytes)) + ' libres sur ' +
        esc(fmtBytes(s.disk_total_bytes)) + ' (volume donn\u00E9es)';
    }

    const temp = s.cpu_temp_c_max != null ? s.cpu_temp_c_max.toFixed(1) + ' \u00B0C' : '\u2014';

    $('tbody-resources').innerHTML =
      memRow +
      trKV('Swap', swap) +
      trKV('CPU (logiques)', esc(String(s.cpu_count))) +
      trKV('Utilisation CPU', esc(s.cpu_usage_percent.toFixed(1) + ' %')) +
      trKV('Temp\u00E9rature (max capteurs)', esc(temp)) +
      trKV('Disque (donn\u00E9es)', disk);
  }

  function triBool(v) {
    if (v === true) return 'oui';
    if (v === false) return 'non';
    return '\u2014';
  }

  function normalizeMacClient(raw) {
    if (!raw || typeof raw !== 'string') return null;
    var s = raw.trim().replace(/-/g, ':').replace(/\s+/g, '');
    if (s.indexOf(':') < 0 && /^[0-9A-Fa-f]{12}$/.test(s)) {
      s = s.match(/.{1,2}/g).join(':').toUpperCase();
    } else {
      s = s.toUpperCase();
    }
    if (!/^([0-9A-F]{2}:){5}[0-9A-F]{2}$/.test(s)) return null;
    return s;
  }

  function syncBtPresenceFromSnapshot(bt) {
    var input = $('bt-mac-input');
    var el = $('bt-mac-result');
    if (!input || !el) return;
    var raw = input.value.trim();
    if (!raw) {
      el.innerHTML = '';
      return;
    }
    var norm = normalizeMacClient(raw);
    if (!norm) {
      el.innerHTML = '<span class="badge badge-error">MAC invalide</span>';
      return;
    }
    if (!bt || !bt.devices) return;
    var dev = null;
    for (var i = 0; i < bt.devices.length; i++) {
      if (bt.devices[i].address && bt.devices[i].address.toUpperCase() === norm) {
        dev = bt.devices[i];
        break;
      }
    }
    if (dev) {
      var name = dev.name ? esc(dev.name) : '\u2014';
      var p = typeof dev.paired === 'boolean' ? (dev.paired ? 'oui' : 'non') : '\u2014';
      var c = typeof dev.connected === 'boolean' ? (dev.connected ? 'oui' : 'non') : '\u2014';
      el.innerHTML = '<span class="badge on">Vu dans la liste BlueZ</span> <strong class="mono">' + esc(norm) + '</strong> \u00B7 ' + name +
        ' \u00B7 appair\u00E9 : ' + p + ' \u00B7 connect\u00E9 : ' + c;
    } else {
      el.innerHTML = '<span class="badge off">Absent de la liste</span> <span class="mono">' + esc(norm) +
        '</span> <span class="cell-muted">\u2014 appairez l\u2019appareil ou attendez qu\u2019il apparaisse dans BlueZ.</span>';
    }
  }

  function renderLookupApi(lookup) {
    var el = $('bt-mac-result');
    if (!el) return;
    var scanLine = '';
    if (lookup.scan_performed && lookup.scan_seconds) {
      scanLine = '<span class="cell-muted">D\u00E9couverte BLE \u2248' + lookup.scan_seconds + ' s. </span>';
    }
    if (!lookup.platform_supported) {
      el.innerHTML = scanLine + '<span class="cell-muted">' + esc(lookup.note || 'Plateforme non prise en charge.') + '</span>';
      return;
    }
    if (!lookup.valid_format) {
      el.innerHTML = scanLine + '<span class="badge badge-error">MAC invalide</span> ' + esc(lookup.note || '');
      return;
    }
    if (lookup.in_known_devices) {
      var name = lookup.name ? esc(lookup.name) : '\u2014';
      var p = lookup.paired === true ? 'oui' : lookup.paired === false ? 'non' : '\u2014';
      var c = lookup.connected === true ? 'oui' : lookup.connected === false ? 'non' : '\u2014';
      el.innerHTML = scanLine + '<span class="badge on">Vu dans la liste BlueZ</span> <strong class="mono">' + esc(lookup.mac_normalized) + '</strong> \u00B7 ' + name +
        ' \u00B7 appair\u00E9 : ' + p + ' \u00B7 connect\u00E9 : ' + c;
    } else {
      el.innerHTML = scanLine + '<span class="badge off">Absent de la liste</span> <span class="mono">' + esc(lookup.mac_normalized) + '</span> \u00B7 ' +
        (lookup.note ? '<span class="cell-muted">' + esc(lookup.note) + '</span>' : '');
    }
  }

  async function checkBtMacApi() {
    var input = $('bt-mac-input');
    var el = $('bt-mac-result');
    var scanSel = $('bt-scan-sec');
    if (!input || !el) return;
    var raw = input.value.trim();
    if (!raw) {
      el.textContent = 'Saisissez une adresse MAC.';
      return;
    }
    var scanSec = scanSel ? parseInt(scanSel.value, 10) : 0;
    if (isNaN(scanSec) || scanSec < 0) scanSec = 0;
    var waitMsg = scanSec >= 5
      ? 'D\u00E9couverte BLE + v\u00E9rification (\u2248' + scanSec + ' s)\u2026'
      : 'V\u00E9rification\u2026';
    el.innerHTML = '<span class="cell-muted">' + waitMsg + '</span>';
    try {
      var qs = '/api/bluetooth/device?addr=' + encodeURIComponent(raw);
      if (scanSec >= 5) qs += '&scan=' + scanSec;
      var res = await fetch(qs);
      var text = await res.text();
      if (!res.ok) {
        el.textContent = text;
        return;
      }
      renderLookupApi(JSON.parse(text));
    } catch (e) {
      el.textContent = 'Erreur : ' + (e && e.message ? e.message : e);
    }
  }

  function renderBluetooth(bt) {
    let html = '';
    if (bt.notes && bt.notes.length) {
      html += '<tr><td colspan="2" class="cell-muted">' + esc(bt.notes.join(' \u00B7 ')) + '</td></tr>';
    }
    const adapters = bt.adapters || [];
    const devices = bt.devices || [];
    if (!adapters.length && !devices.length) {
      $('tbody-bluetooth').innerHTML = html;
      return;
    }
    for (const a of adapters) {
      html += '<tr><th colspan="2" class="subhdr">' + esc(a.hci_device) + '</th></tr>';
      html += trKV('Adresse', esc(a.address || '\u2014'));
      html += trKV('Nom', esc(a.name || '\u2014'));
      if (a.device_class) html += trKV('Classe d\u2019appareil', esc(a.device_class));
      if (typeof a.rfkill_soft_blocked === 'boolean') {
        html += trKV('RF-kill (logiciel)', esc(a.rfkill_soft_blocked ? 'bloqu\u00E9' : 'd\u00E9bloqu\u00E9'));
      }
      html += trKV('Aliment\u00E9', esc(triBool(a.powered)));
      html += trKV('Visible', esc(triBool(a.discoverable)));
      html += trKV('Appairable', esc(triBool(a.pairable)));
    }
    if (devices.length) {
      html += '<tr><th colspan="2" class="subhdr">Appareils connus</th></tr>';
      for (const d of devices) {
        let td = d.name || '\u2014';
        if (typeof d.paired === 'boolean') td += d.paired ? ' \u00B7 appair\u00E9 : oui' : ' \u00B7 appair\u00E9 : non';
        if (typeof d.connected === 'boolean') {
          td += d.connected ? ' \u00B7 connect\u00E9 : oui' : ' \u00B7 connect\u00E9 : non';
        }
        var rawAddr = d.address || '';
        var addrAttr = String(rawAddr).replace(/&/g, '&amp;').replace(/"/g, '&quot;');
        html += '<tr><th class="mono">' + esc(rawAddr) + '</th><td><span class="bt-device-summary">' + esc(td) +
          '</span> <button type="button" class="bt-device-info" data-address="' + addrAttr +
          '" aria-label="D\u00E9tails Bluetooth ' + esc(rawAddr) + '">D\u00E9tails</button></td></tr>';
      }
    }
    $('tbody-bluetooth').innerHTML = html;
  }

  function renderBtInfoPanel(j) {
    var panel = $('bt-detail-panel');
    if (!panel) return;
    panel.hidden = false;
    if (!j.platform_supported) {
      panel.innerHTML = '<p class="cell-muted">' + esc(j.error || 'Non disponible sur cette plateforme.') + '</p>';
      return;
    }
    if (j.error) {
      panel.innerHTML = '<p class="cell-muted">' + esc(j.error) + '</p><p class="bt-detail-foot cell-muted">Souvent : l\u2019appareil n\u2019est pas encore dans le cache BlueZ ou la MAC est incorrecte.</p>';
      return;
    }
    function row(label, val) {
      if (val === undefined || val === null || val === '') return '';
      return '<tr><th>' + esc(label) + '</th><td>' + esc(String(val)) + '</td></tr>';
    }
    var body = '';
    body += row('Nom', j.name);
    body += row('Alias', j.alias);
    body += row('Classe', j.device_class);
    body += row('Ic\u00F4ne', j.icon);
    body += row('Appair\u00E9', j.paired === true ? 'oui' : j.paired === false ? 'non' : null);
    body += row('Li\u00E9 (bonded)', j.bonded === true ? 'oui' : j.bonded === false ? 'non' : null);
    body += row('Fiable (trusted)', j.trusted === true ? 'oui' : j.trusted === false ? 'non' : null);
    body += row('Bloqu\u00E9', j.blocked === true ? 'oui' : j.blocked === false ? 'non' : null);
    body += row('Connect\u00E9', j.connected === true ? 'oui' : j.connected === false ? 'non' : null);
    body += row('RSSI', j.rssi != null ? j.rssi + ' dBm' : null);
    body += row('Batterie %', j.battery_percentage != null ? j.battery_percentage : null);
    body += row('ManufacturerData', j.manufacturer_data);
    body += row('Modalias', j.modalias);
    if (j.uuids && j.uuids.length) {
      body += '<tr><th>UUID</th><td class="mono uuid-cell">' + j.uuids.map(function (u) { return esc(u); }).join('<br>') + '</td></tr>';
    }
    var title = j.mac_normalized ? esc(j.mac_normalized) : '';
    panel.innerHTML = '<h3 class="bt-detail-title">D\u00E9tails BlueZ <span class="mono">' + title + '</span></h3>' +
      '<p class="bt-detail-src cell-muted">Source : <code>bluetoothctl info</code></p>' +
      '<div class="table-scroll-wrap"><table class="data bt-detail-inner"><tbody>' + body + '</tbody></table></div>';
    panel.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
  }

  async function loadBtDeviceInfo(addr) {
    var panel = $('bt-detail-panel');
    if (!panel) return;
    panel.hidden = false;
    panel.innerHTML = '<p class="cell-muted">Chargement des d\u00E9tails\u2026</p>';
    try {
      var res = await fetch('/api/bluetooth/info?addr=' + encodeURIComponent(addr));
      var text = await res.text();
      if (!res.ok) {
        panel.innerHTML = '<p class="cell-muted">' + esc(text) + '</p>';
        return;
      }
      renderBtInfoPanel(JSON.parse(text));
    } catch (e) {
      panel.innerHTML = '<p class="cell-muted">Erreur : ' + esc(e && e.message ? e.message : String(e)) + '</p>';
    }
  }

  async function refresh() {
    errEl.classList.remove('visible');
    errEl.textContent = '';
    if (btnRefresh) btnRefresh.classList.add('is-loading');
    try {
      const [sysRes, btRes] = await Promise.all([
        fetch('/api/system'),
        fetch('/api/bluetooth'),
      ]);
      if (!sysRes.ok) throw new Error(await sysRes.text());
      render(await sysRes.json());

      const btText = await btRes.text();
      if (btRes.ok) {
        try {
          var bt = JSON.parse(btText);
          lastBtSnapshot = bt;
          renderBluetooth(bt);
          syncBtPresenceFromSnapshot(bt);
        } catch (_) {
          lastBtSnapshot = null;
          $('tbody-bluetooth').innerHTML = '<tr><td colspan="2" class="cell-empty">' +
            esc('JSON invalide depuis /api/bluetooth') + '</td></tr>';
        }
      } else {
        lastBtSnapshot = null;
        $('tbody-bluetooth').innerHTML = '<tr><td colspan="2" class="cell-empty">' +
          esc('API Bluetooth : ' + btText) + '</td></tr>';
      }

      consecutiveFails = 0;
      if (updatedEl) {
        updatedEl.classList.remove('is-offline');
        updatedEl.textContent = 'Mis \u00E0 jour : ' + new Date().toLocaleTimeString('fr-FR');
      }
    } catch (e) {
      consecutiveFails++;
      errEl.textContent = 'Actualisation impossible : ' + (e && e.message ? e.message : e);
      errEl.classList.add('visible');
      if (updatedEl) {
        if (consecutiveFails >= 2) updatedEl.classList.add('is-offline');
        updatedEl.textContent = consecutiveFails >= 2
          ? 'Hors ligne ou erreur r\u00E9p\u00E9t\u00E9e'
          : 'Erreur : ' + new Date().toLocaleTimeString('fr-FR');
      }
    } finally {
      if (btnRefresh) btnRefresh.classList.remove('is-loading');
    }
  }

  function schedule() {
    if (timer) clearInterval(timer);
    timer = null;
    const ms = typeof window.rhGetRefreshIntervalMs === 'function'
      ? window.rhGetRefreshIntervalMs()
      : 4000;
    if (ms > 0) timer = setInterval(refresh, ms);
  }

  $('btn-refresh').addEventListener('click', () => refresh());
  var btnBt = $('bt-mac-check');
  if (btnBt) btnBt.addEventListener('click', () => checkBtMacApi());
  var inpBt = $('bt-mac-input');
  if (inpBt) {
    inpBt.addEventListener('keydown', function (e) {
      if (e.key === 'Enter') {
        e.preventDefault();
        checkBtMacApi();
      }
    });
  }
  window.addEventListener('rh-refresh-interval-changed', schedule);
  var tbodyBt = $('tbody-bluetooth');
  if (tbodyBt) {
    tbodyBt.addEventListener('click', function (e) {
      var btn = e.target.closest('.bt-device-info');
      if (!btn) return;
      var addr = btn.getAttribute('data-address');
      if (addr) loadBtDeviceInfo(addr);
    });
  }
  schedule();
  refresh();
})();
