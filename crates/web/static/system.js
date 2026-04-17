(function () {
  const $ = (id) => document.getElementById(id);
  const errEl = $('error-bar');
  let timer = null;

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
      ? (s.journal_file_bytes != null ? 'present \u2014 ' + fmtBytes(s.journal_file_bytes) : 'present')
      : 'missing (no events yet)';

    $('tbody-rusthome').innerHTML =
      trKV('Service', esc(s.service)) +
      trKV('rusthome-web version', esc(s.rusthome_version)) +
      trKV('Listen address', esc(s.listen)) +
      trKV('Data directory', esc(s.data_dir)) +
      trKV('Journal file', esc(s.journal_path)) +
      trKV('Journal on disk', esc(journalMeta));

    $('tbody-host').innerHTML =
      trKV('Hostname', esc(s.hostname || '\u2014')) +
      trKV('OS', esc(os)) +
      trKV('Kernel', esc(s.kernel || '\u2014')) +
      trKV('CPU architecture', esc(s.cpu_arch)) +
      trKV('Uptime', esc(fmtDuration(s.uptime_secs))) +
      trKV('Load average', esc(
        s.load_avg_1.toFixed(2) + ' \u00B7 ' + s.load_avg_5.toFixed(2) + ' \u00B7 ' + s.load_avg_15.toFixed(2) + ' (1 / 5 / 15 min)'
      ));

    const memPct = s.memory_total_bytes > 0
      ? Math.min(100, (s.memory_used_bytes / s.memory_total_bytes) * 100)
      : 0;
    const memRow =
      '<tr><th>Memory</th><td>' +
      '<div class="meter-wrap" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow="' +
      Math.round(memPct) + '">' +
      '<div class="meter-fill" style="width:' + memPct.toFixed(1) + '%"></div></div>' +
      '<span class="meter-label">' + esc(fmtBytes(s.memory_used_bytes)) + ' / ' +
      esc(fmtBytes(s.memory_total_bytes)) + ' (' + Math.round(memPct) + '%)</span></td></tr>';

    const swap = s.swap_total_bytes > 0
      ? esc(fmtBytes(s.swap_used_bytes) + ' / ' + fmtBytes(s.swap_total_bytes))
      : '\u2014';

    let disk = '\u2014 (could not map mount)';
    if (s.disk_mount && s.disk_total_bytes != null && s.disk_available_bytes != null) {
      disk = esc(s.disk_mount) + ' \u2014 ' + esc(fmtBytes(s.disk_available_bytes)) + ' free of ' +
        esc(fmtBytes(s.disk_total_bytes)) + ' (data dir mount)';
    }

    const temp = s.cpu_temp_c_max != null ? s.cpu_temp_c_max.toFixed(1) + ' \u00B0C' : '\u2014';

    $('tbody-resources').innerHTML =
      memRow +
      trKV('Swap', swap) +
      trKV('CPUs (logical)', esc(String(s.cpu_count))) +
      trKV('CPU usage (global)', esc(s.cpu_usage_percent.toFixed(1) + '%')) +
      trKV('Temperature (sensors max)', esc(temp)) +
      trKV('Disk (data volume)', disk);
  }

  function triBool(v) {
    if (v === true) return 'yes';
    if (v === false) return 'no';
    return '\u2014';
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
      html += trKV('Address', esc(a.address || '\u2014'));
      html += trKV('Name', esc(a.name || '\u2014'));
      if (a.device_class) html += trKV('Device class', esc(a.device_class));
      if (typeof a.rfkill_soft_blocked === 'boolean') {
        html += trKV('RF-kill (soft)', esc(a.rfkill_soft_blocked ? 'blocked' : 'unblocked'));
      }
      html += trKV('Powered', esc(triBool(a.powered)));
      html += trKV('Discoverable', esc(triBool(a.discoverable)));
      html += trKV('Pairable', esc(triBool(a.pairable)));
    }
    if (devices.length) {
      html += '<tr><th colspan="2" class="subhdr">Known devices</th></tr>';
      for (const d of devices) {
        let td = d.name || '\u2014';
        if (typeof d.paired === 'boolean') td += d.paired ? ' \u00B7 paired: yes' : ' \u00B7 paired: no';
        if (typeof d.connected === 'boolean') {
          td += d.connected ? ' \u00B7 connected: yes' : ' \u00B7 connected: no';
        }
        html += '<tr><th class="mono">' + esc(d.address) + '</th><td>' + esc(td) + '</td></tr>';
      }
    }
    $('tbody-bluetooth').innerHTML = html;
  }

  async function refresh() {
    errEl.classList.remove('visible');
    errEl.textContent = '';
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
          renderBluetooth(JSON.parse(btText));
        } catch (_) {
          $('tbody-bluetooth').innerHTML = '<tr><td colspan="2" class="cell-empty">' +
            esc('Invalid JSON from /api/bluetooth') + '</td></tr>';
        }
      } else {
        $('tbody-bluetooth').innerHTML = '<tr><td colspan="2" class="cell-empty">' +
          esc('Bluetooth API: ' + btText) + '</td></tr>';
      }

      $('updated-at').textContent = 'Updated ' + new Date().toLocaleTimeString();
    } catch (e) {
      errEl.textContent = 'Refresh failed: ' + (e && e.message ? e.message : e);
      errEl.classList.add('visible');
    }
  }

  function schedule() {
    if (timer) clearInterval(timer);
    timer = null;
    if ($('auto-refresh').checked) timer = setInterval(refresh, 4000);
  }

  $('btn-refresh').addEventListener('click', () => refresh());
  $('auto-refresh').addEventListener('change', schedule);
  schedule();
  refresh();
})();
