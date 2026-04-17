(function () {
  const cfgEl = document.getElementById('rh-dashboard-config');
  const cfg = cfgEl ? JSON.parse(cfgEl.textContent) : { journalLimit: 40, brokerAvailable: false };
  const journalLimit = cfg.journalLimit;
  const brokerAvailable = !!cfg.brokerAvailable;

  const $ = (id) => document.getElementById(id);
  const errEl = $('error-bar');
  const toastEl = $('command-toast');
  let timer = null;
  let toastHideTimer = null;

  if (brokerAvailable) {
    const hdr = $('action-hdr');
    if (hdr) hdr.textContent = 'Action';
  }

  function showToast(msg) {
    if (!toastEl) return;
    toastEl.textContent = msg;
    toastEl.classList.add('visible');
    if (toastHideTimer) clearTimeout(toastHideTimer);
    toastHideTimer = setTimeout(() => {
      toastEl.classList.remove('visible');
      toastEl.textContent = '';
    }, 3500);
  }

  function provLabel(v) {
    if (!v) return '\u2014';
    if (typeof v === 'string') return v.charAt(0).toUpperCase() + v.slice(1);
    return String(v);
  }

  function fmtCelsius(millideg) {
    return (millideg / 1000).toFixed(1) + '\u00B0C';
  }

  function renderState(st) {
    const tbody = $('lights-body');
    tbody.replaceChildren();
    const lights = st.lights || {};
    const provMap = st.light_last_provenance || {};
    const rooms = Object.keys(lights).sort();
    const cols = brokerAvailable ? 4 : 3;
    if (rooms.length === 0) {
      const tr = document.createElement('tr');
      const td = document.createElement('td');
      td.colSpan = cols;
      td.className = 'cell-empty';
      const em = document.createElement('em');
      em.textContent = 'No rooms in projection yet';
      td.appendChild(em);
      tr.appendChild(td);
      tbody.appendChild(tr);
    } else for (const room of rooms) {
      const tr = document.createElement('tr');
      const tdRoom = document.createElement('td');
      tdRoom.className = 'col-room';
      tdRoom.textContent = room;
      const tdState = document.createElement('td');
      const badge = document.createElement('span');
      const on = !!lights[room];
      badge.className = 'badge ' + (on ? 'on' : 'off');
      badge.textContent = on ? 'On' : 'Off';
      tdState.appendChild(badge);
      const tdProv = document.createElement('td');
      tdProv.className = 'col-prov';
      tdProv.textContent = provLabel(provMap[room]);
      tr.appendChild(tdRoom);
      tr.appendChild(tdState);
      tr.appendChild(tdProv);
      if (brokerAvailable) {
        const tdAction = document.createElement('td');
        const btn = document.createElement('button');
        btn.type = 'button';
        btn.className = 'btn-toggle';
        btn.textContent = on ? 'Turn Off' : 'Turn On';
        btn.addEventListener('click', () => toggleLight(room, on));
        tdAction.appendChild(btn);
        tr.appendChild(tdAction);
      }
      tbody.appendChild(tr);
    }

    renderSensors(st);
    renderSummary(st);
  }

  function renderSensors(st) {
    const tbody = $('sensors-body');
    tbody.replaceChildren();
    const temps = st.temperatures || {};
    const contacts = st.contacts || {};
    const tempKeys = Object.keys(temps).sort();
    const contactKeys = Object.keys(contacts).sort();

    if (tempKeys.length === 0 && contactKeys.length === 0) {
      const tr = document.createElement('tr');
      const td = document.createElement('td');
      td.colSpan = 3;
      td.className = 'cell-empty';
      const em = document.createElement('em');
      em.textContent = 'No sensor data yet';
      td.appendChild(em);
      tr.appendChild(td);
      tbody.appendChild(tr);
      return;
    }

    for (const id of tempKeys) {
      const tr = document.createElement('tr');
      const tdName = document.createElement('td');
      tdName.className = 'col-room';
      tdName.textContent = '\uD83C\uDF21\uFE0F ' + id;
      const tdVal = document.createElement('td');
      const b = document.createElement('span');
      b.className = 'badge badge-fact';
      b.textContent = fmtCelsius(temps[id]);
      tdVal.appendChild(b);
      const tdType = document.createElement('td');
      tdType.className = 'col-prov';
      tdType.textContent = 'temperature';
      tr.appendChild(tdName);
      tr.appendChild(tdVal);
      tr.appendChild(tdType);
      tbody.appendChild(tr);
    }

    for (const id of contactKeys) {
      const tr = document.createElement('tr');
      const tdName = document.createElement('td');
      tdName.className = 'col-room';
      tdName.textContent = '\uD83D\uDEAA ' + id;
      const tdVal = document.createElement('td');
      const b = document.createElement('span');
      const isOpen = !!contacts[id];
      b.className = 'badge ' + (isOpen ? 'badge-obs' : 'badge-fact');
      b.textContent = isOpen ? 'Open' : 'Closed';
      tdVal.appendChild(b);
      const tdType = document.createElement('td');
      tdType.className = 'col-prov';
      tdType.textContent = 'contact';
      tr.appendChild(tdName);
      tr.appendChild(tdVal);
      tr.appendChild(tdType);
      tbody.appendChild(tr);
    }
  }

  function renderSummary(st) {
    const lights = st.lights || {};
    const rooms = Object.keys(lights);
    const lightsOn = rooms.filter(r => !!lights[r]).length;
    const temps = st.temperatures || {};
    const contacts = st.contacts || {};
    const sensorCount = Object.keys(temps).length + Object.keys(contacts).length;
    $('summary-bar').innerHTML =
      '<div class="summary-card"><span class="summary-icon">\uD83C\uDFE0</span><span class="summary-value">' + rooms.length + '</span><span class="summary-label">Rooms</span></div>' +
      '<div class="summary-card"><span class="summary-icon">\uD83D\uDCA1</span><span class="summary-value">' + lightsOn + '/' + rooms.length + '</span><span class="summary-label">Lights On</span></div>' +
      '<div class="summary-card"><span class="summary-icon">\uD83C\uDF21\uFE0F</span><span class="summary-value">' + sensorCount + '</span><span class="summary-label">Sensors</span></div>' +
      '<div class="summary-card" id="summary-events"><span class="summary-icon">\uD83D\uDCCB</span><span class="summary-value">-</span><span class="summary-label">Events</span></div>';
  }

  function renderJournal(lines) {
    const tbody = $('journal-body');
    tbody.replaceChildren();
    if (!lines.length) {
      const tr = document.createElement('tr');
      const td = document.createElement('td');
      td.colSpan = 3;
      td.className = 'cell-empty';
      const em = document.createElement('em');
      em.textContent = 'Journal is empty';
      td.appendChild(em);
      tr.appendChild(td);
      tbody.appendChild(tr);
      return;
    }
    const eventsCard = document.querySelector('#summary-events .summary-value');
    if (eventsCard) eventsCard.textContent = String(lines[lines.length - 1].sequence + 1);
    for (let i = lines.length - 1; i >= 0; i--) {
      const row = lines[i];
      const tr = document.createElement('tr');
      const tdSeq = document.createElement('td');
      tdSeq.className = 'mono';
      tdSeq.textContent = String(row.sequence);
      const tdTs = document.createElement('td');
      tdTs.className = 'mono';
      tdTs.textContent = String(row.timestamp);
      const tdDetail = document.createElement('td');
      const b = document.createElement('span');
      b.className = 'badge badge-' + (row.family || 'fact');
      b.textContent = row.detail || row.kind;
      tdDetail.appendChild(b);
      tr.appendChild(tdSeq);
      tr.appendChild(tdTs);
      tr.appendChild(tdDetail);
      tbody.appendChild(tr);
    }
  }

  async function toggleLight(room, currentlyOn) {
    const action = currentlyOn ? 'turn_off' : 'turn_on';
    try {
      const res = await fetch('/api/command', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ action, room })
      });
      if (!res.ok) {
        const msg = await res.text();
        errEl.textContent = res.status === 503
          ? 'Broker unavailable (start with rusthome serve for commands). ' + msg
          : 'Command failed: ' + msg;
        errEl.classList.add('visible');
        return;
      }
      showToast('Command published to MQTT');
      setTimeout(refresh, 300);
    } catch (e) {
      errEl.textContent = 'Command failed: ' + (e && e.message ? e.message : e);
      errEl.classList.add('visible');
    }
  }

  document.addEventListener('click', function (e) {
    const btn = e.target.closest('#lights-body .btn-toggle[data-room]');
    if (!btn || !brokerAvailable) return;
    const room = btn.getAttribute('data-room');
    if (room == null) return;
    const on = btn.getAttribute('data-on') === 'true';
    toggleLight(room, on);
  });

  async function refresh() {
    errEl.classList.remove('visible');
    errEl.textContent = '';
    try {
      const [stRes, jrRes] = await Promise.all([
        fetch('/api/state'),
        fetch('/api/journal?limit=' + journalLimit)
      ]);
      if (!stRes.ok) throw new Error(await stRes.text());
      if (!jrRes.ok) throw new Error(await jrRes.text());
      renderState(await stRes.json());
      renderJournal(await jrRes.json());
      $('updated-at').textContent = 'Updated ' + new Date().toLocaleTimeString();
    } catch (e) {
      errEl.textContent = 'Refresh failed: ' + (e && e.message ? e.message : e);
      errEl.classList.add('visible');
    }
  }

  function schedule() {
    if (timer) clearInterval(timer);
    timer = null;
    if ($('auto-refresh').checked)
      timer = setInterval(refresh, 4000);
  }

  $('btn-refresh').addEventListener('click', () => refresh());
  $('auto-refresh').addEventListener('change', schedule);
  schedule();
  refresh();
})();
