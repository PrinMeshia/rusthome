(function () {
  const cfgEl = document.getElementById('rh-dashboard-config');
  const cfg = cfgEl
    ? JSON.parse(cfgEl.textContent)
    : { journalLimit: 40, brokerAvailable: false, livePush: false };
  const journalLimitDefault = typeof cfg.journalLimit === 'number' ? cfg.journalLimit : 40;
  const brokerAvailable = !!cfg.brokerAvailable;
  const livePush = !!cfg.livePush;

  function currentJournalLimit() {
    return typeof window.rhGetJournalLimit === 'function'
      ? window.rhGetJournalLimit(journalLimitDefault)
      : journalLimitDefault;
  }

  const $ = (id) => document.getElementById(id);
  const errEl = $('error-bar');
  const toastEl = $('command-toast');
  const btnRefresh = $('btn-refresh');
  const brokerPill = document.getElementById('broker-pill');
  const updatedEl = $('updated-at');
  let timer = null;
  let toastHideTimer = null;
  let consecutiveFails = 0;

  if (brokerAvailable) {
    const hdr = $('action-hdr');
    if (hdr) hdr.textContent = 'Commande';
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
      em.textContent = 'Aucune pièce dans la projection';
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
      badge.textContent = on ? 'Allumée' : 'Éteinte';
      tdState.appendChild(badge);
      const tdProv = document.createElement('td');
      tdProv.className = 'col-prov';
      tdProv.textContent = provLabel(provMap[room]);
      tr.appendChild(tdRoom);
      tr.appendChild(tdState);
      tr.appendChild(tdProv);
      if (brokerAvailable) {
        const tdAction = document.createElement('td');
        const sw = document.createElement('button');
        sw.type = 'button';
        sw.className = 'light-switch';
        sw.setAttribute('role', 'switch');
        sw.setAttribute('aria-checked', on ? 'true' : 'false');
        sw.setAttribute('aria-label', 'Lumière ' + room + ', ' + (on ? 'allumée' : 'éteinte'));
        sw.dataset.room = room;
        sw.dataset.on = on ? 'true' : 'false';
        tdAction.appendChild(sw);
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
      em.textContent = 'Aucune donnée capteur';
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
      tdType.textContent = 'température';
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
      b.textContent = isOpen ? 'Ouvert' : 'Fermé';
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
      '<div class="summary-card"><span class="summary-icon">\uD83C\uDFE0</span><span class="summary-value">' + rooms.length + '</span><span class="summary-label">Pièces</span></div>' +
      '<div class="summary-card"><span class="summary-icon">\uD83D\uDCA1</span><span class="summary-value">' + lightsOn + '/' + rooms.length + '</span><span class="summary-label">Lampes</span></div>' +
      '<div class="summary-card"><span class="summary-icon">\uD83C\uDF21\uFE0F</span><span class="summary-value">' + sensorCount + '</span><span class="summary-label">Capteurs</span></div>' +
      '<div class="summary-card" id="summary-events"><span class="summary-icon">\uD83D\uDCCB</span><span class="summary-value">-</span><span class="summary-label">Événements</span></div>';
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
      em.textContent = 'Le journal est vide';
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
          ? 'Broker indisponible (utilisez rusthome serve pour les commandes). ' + msg
          : 'Échec de la commande : ' + msg;
        errEl.classList.add('visible');
        return;
      }
      showToast('Commande publiée sur MQTT');
      setTimeout(refresh, 300);
    } catch (e) {
      errEl.textContent = 'Échec de la commande : ' + (e && e.message ? e.message : e);
      errEl.classList.add('visible');
    }
  }

  document.getElementById('lights-body')?.addEventListener('click', function (e) {
    const sw = e.target.closest('.light-switch');
    if (!sw || !brokerAvailable) return;
    const room = sw.getAttribute('data-room');
    if (room == null) return;
    const on = sw.getAttribute('data-on') === 'true';
    toggleLight(room, on);
  });

  async function refresh() {
    errEl.classList.remove('visible');
    errEl.textContent = '';
    if (btnRefresh) btnRefresh.classList.add('is-loading');
    if (brokerPill) brokerPill.classList.add('is-syncing');
    try {
      const [stRes, jrRes] = await Promise.all([
        fetch('/api/state'),
        fetch('/api/journal?limit=' + currentJournalLimit())
      ]);
      if (!stRes.ok) throw new Error(await stRes.text());
      if (!jrRes.ok) throw new Error(await jrRes.text());
      renderState(await stRes.json());
      renderJournal(await jrRes.json());
      consecutiveFails = 0;
      if (updatedEl) {
        updatedEl.classList.remove('is-offline');
        updatedEl.textContent = 'Mis à jour : ' + new Date().toLocaleTimeString('fr-FR');
      }
    } catch (e) {
      consecutiveFails++;
      errEl.textContent = 'Actualisation impossible : ' + (e && e.message ? e.message : e);
      errEl.classList.add('visible');
      if (updatedEl) {
        if (consecutiveFails >= 2) updatedEl.classList.add('is-offline');
        updatedEl.textContent = consecutiveFails >= 2          ? 'Hors ligne ou erreur répétée'
          : 'Erreur : ' + new Date().toLocaleTimeString('fr-FR');
      }
    } finally {
      if (btnRefresh) btnRefresh.classList.remove('is-loading');
      if (brokerPill) brokerPill.classList.remove('is-syncing');
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
  window.addEventListener('rh-refresh-interval-changed', schedule);
  window.addEventListener('rh-journal-limit-changed', () => refresh());
  schedule();
  refresh();

  if (livePush && typeof EventSource !== 'undefined') {
    let liveDebounce = null;
    const es = new EventSource('/api/live');
    es.onmessage = function () {
      if (liveDebounce) clearTimeout(liveDebounce);
      liveDebounce = setTimeout(function () {
        refresh();
        liveDebounce = null;
      }, 80);
    };
    es.onerror = function () {
      /* navigateur reconnecte automatiquement ; polling reste actif */
    };
  }
})();
