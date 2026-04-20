(function () {
  const cfgEl = document.getElementById('rh-sensors-config');
  const cfg = cfgEl
    ? JSON.parse(cfgEl.textContent)
    : { brokerAvailable: false, livePush: false };
  const brokerAvailable = !!cfg.brokerAvailable;
  const livePush = !!cfg.livePush;

  const $ = (id) => document.getElementById(id);
  const errEl = $('error-bar');
  const toastObs = $('observation-toast');
  const brokerPill = $('broker-pill');
  const btnRefresh = $('btn-refresh');
  const btnSync = $('btn-sensor-sync');
  const btnSave = $('btn-sensor-save');
  const updatedEl = $('updated-at');
  const formObs = $('form-observation');
  const obsKind = $('obs-kind');
  const obsSubmit = $('obs-submit');
  let obsToastTimer = null;
  let timer = null;
  let consecutiveFails = 0;
  let lastSensorState = null;
  let lastDisplayDoc = null;

  const FAM_TEMP = 'temperature';
  const FAM_HUM = 'humidity';
  const FAM_CONTACT = 'contact';

  const STORAGE_Q = 'rusthome-sensors-query';
  const STORAGE_KIND = 'rusthome-sensors-kind';

  function fmtCelsius(millideg) {
    return (millideg / 1000).toFixed(1) + '\u00B0C';
  }

  function fmtHumidityPermille(permille) {
    return (permille / 10).toFixed(1) + ' %';
  }

  function getSearchQuery() {
    const el = $('sensors-search');
    return el ? el.value.trim().toLowerCase() : '';
  }

  function getKindFilter() {
    const el = $('sensors-kind-filter');
    const v = el ? el.value : 'all';
    if (v === 'temperature' || v === 'contact' || v === 'humidity') return v;
    return 'all';
  }

  function entriesMap(doc, family) {
    if (!doc || !doc.entries || !doc.entries[family]) return {};
    return doc.entries[family];
  }

  function unionSortedKeys(stateKeys, displayKeys) {
    const s = new Set();
    stateKeys.forEach(function (k) {
      s.add(k);
    });
    displayKeys.forEach(function (k) {
      s.add(k);
    });
    return Array.from(s).sort();
  }

  function getMeta(doc, family, id) {
    const m = entriesMap(doc, family)[id];
    return m && typeof m === 'object'
      ? { label: m.label || '', room: m.room || '' }
      : { label: '', room: '' };
  }

  function rowMatches(id, meta, q) {
    if (!q) return true;
    if (String(id).toLowerCase().indexOf(q) >= 0) return true;
    if (meta.label && String(meta.label).toLowerCase().indexOf(q) >= 0) return true;
    if (meta.room && String(meta.room).toLowerCase().indexOf(q) >= 0) return true;
    return false;
  }

  function setSectionHidden(sectionEl, hidden) {
    if (!sectionEl) return;
    if (hidden) sectionEl.classList.add('is-filter-hidden');
    else sectionEl.classList.remove('is-filter-hidden');
  }

  function buildPayloadFromInputs() {
    const entries = {};
    const families = [FAM_TEMP, FAM_HUM, FAM_CONTACT];
    for (let i = 0; i < families.length; i++) {
      entries[families[i]] = {};
    }
    const inputs = document.querySelectorAll('input.sensor-meta-input');
    for (let j = 0; j < inputs.length; j++) {
      const el = inputs[j];
      const fam = el.getAttribute('data-family');
      const sid = el.getAttribute('data-sensor-id');
      const field = el.getAttribute('data-field');
      if (!fam || !sid || !field) continue;
      if (!entries[fam]) entries[fam] = {};
      if (!entries[fam][sid]) entries[fam][sid] = {};
      entries[fam][sid][field] = el.value.trim();
    }
    return {
      schema_version: 1,
      entries: entries,
    };
  }

  function renderSensors(st, displayDoc) {
    lastSensorState = st;
    lastDisplayDoc = displayDoc;
    const q = getSearchQuery();
    const kind = getKindFilter();

    const temps = (st && st.temperatures) || {};
    const humidities = (st && st.humidities) || {};
    const contacts = (st && st.contacts) || {};

    const tempIds = unionSortedKeys(Object.keys(temps), Object.keys(entriesMap(displayDoc, FAM_TEMP)));
    const humidityIds = unionSortedKeys(
      Object.keys(humidities),
      Object.keys(entriesMap(displayDoc, FAM_HUM)),
    );
    const contactIds = unionSortedKeys(Object.keys(contacts), Object.keys(entriesMap(displayDoc, FAM_CONTACT)));

    const tempIdsFiltered = tempIds.filter(function (id) {
      return rowMatches(id, getMeta(displayDoc, FAM_TEMP, id), q);
    });
    const humidityIdsFiltered = humidityIds.filter(function (id) {
      return rowMatches(id, getMeta(displayDoc, FAM_HUM, id), q);
    });
    const contactIdsFiltered = contactIds.filter(function (id) {
      return rowMatches(id, getMeta(displayDoc, FAM_CONTACT, id), q);
    });

    const showTemp = kind === 'all' || kind === 'temperature';
    const showHumidity = kind === 'all' || kind === 'humidity';
    const showContact = kind === 'all' || kind === 'contact';

    setSectionHidden($('sensors-section-temp'), !showTemp);
    setSectionHidden($('sensors-section-humidity'), !showHumidity);
    setSectionHidden($('sensors-section-contact'), !showContact);

    function fillTable(bodyId, family, idsFiltered, idsAll, stateMap, renderValueTd) {
      const tbody = $(bodyId);
      if (!tbody) return;
      tbody.replaceChildren();

      if (idsFiltered.length === 0) {
        const tr = document.createElement('tr');
        const td = document.createElement('td');
        td.colSpan = 4;
        td.className = 'cell-empty';
        const em = document.createElement('em');
        em.textContent =
          idsAll.length === 0
            ? family === FAM_TEMP
              ? 'Aucun capteur de température'
              : family === FAM_HUM
                ? 'Aucun capteur d\u2019humidit\u00E9'
                : 'Aucun contact'
            : 'Aucun r\u00E9sultat pour ce filtre';
        td.appendChild(em);
        tr.appendChild(td);
        tbody.appendChild(tr);
        return;
      }

      for (let i = 0; i < idsFiltered.length; i++) {
        const id = idsFiltered[i];
        const meta = getMeta(displayDoc, family, id);
        const hasVal = Object.prototype.hasOwnProperty.call(stateMap, id);
        const tr = document.createElement('tr');

        const tdLabel = document.createElement('td');
        tdLabel.className = 'sensor-cell-meta';
        const inLabel = document.createElement('input');
        inLabel.type = 'text';
        inLabel.className = 'sensor-meta-input';
        inLabel.setAttribute('data-family', family);
        inLabel.setAttribute('data-sensor-id', id);
        inLabel.setAttribute('data-field', 'label');
        inLabel.setAttribute('aria-label', 'Libellé ' + id);
        inLabel.value = meta.label;
        tdLabel.appendChild(inLabel);

        const tdRoom = document.createElement('td');
        tdRoom.className = 'sensor-cell-meta';
        const inRoom = document.createElement('input');
        inRoom.type = 'text';
        inRoom.className = 'sensor-meta-input';
        inRoom.setAttribute('data-family', family);
        inRoom.setAttribute('data-sensor-id', id);
        inRoom.setAttribute('data-field', 'room');
        inRoom.setAttribute('aria-label', 'Pièce ' + id);
        inRoom.value = meta.room;
        tdRoom.appendChild(inRoom);

        const tdId = document.createElement('td');
        tdId.className = 'sensor-id-cell mono';
        tdId.textContent = id;

        const tdVal = document.createElement('td');
        tdVal.appendChild(renderValueTd(id, hasVal));

        tr.appendChild(tdLabel);
        tr.appendChild(tdRoom);
        tr.appendChild(tdId);
        tr.appendChild(tdVal);
        tbody.appendChild(tr);
      }
    }

    fillTable('temp-body', FAM_TEMP, tempIdsFiltered, tempIds, temps, function (id, hasVal) {
      const frag = document.createDocumentFragment();
      if (!hasVal) {
        const em = document.createElement('em');
        em.className = 'cell-muted';
        em.textContent = 'Pas de mesure r\u00E9cente';
        frag.appendChild(em);
        return frag;
      }
      const b = document.createElement('span');
      b.className = 'badge badge-fact';
      b.textContent = fmtCelsius(temps[id]);
      frag.appendChild(b);
      return frag;
    });

    fillTable('humidity-body', FAM_HUM, humidityIdsFiltered, humidityIds, humidities, function (id, hasVal) {
      const frag = document.createDocumentFragment();
      if (!hasVal) {
        const em = document.createElement('em');
        em.className = 'cell-muted';
        em.textContent = 'Pas de mesure r\u00E9cente';
        frag.appendChild(em);
        return frag;
      }
      const b = document.createElement('span');
      b.className = 'badge badge-fact';
      b.textContent = fmtHumidityPermille(humidities[id]);
      frag.appendChild(b);
      return frag;
    });

    fillTable('contact-body', FAM_CONTACT, contactIdsFiltered, contactIds, contacts, function (id, hasVal) {
      const frag = document.createDocumentFragment();
      if (!hasVal) {
        const em = document.createElement('em');
        em.className = 'cell-muted';
        em.textContent = 'Pas de mesure r\u00E9cente';
        frag.appendChild(em);
        return frag;
      }
      const b = document.createElement('span');
      const isOpen = !!contacts[id];
      b.className = 'badge ' + (isOpen ? 'badge-obs' : 'badge-fact');
      b.textContent = isOpen ? 'Ouvert' : 'Ferm\u00E9';
      frag.appendChild(b);
      return frag;
    });

    const countEl = $('sensors-filter-count');
    if (countEl) {
      const nTemp = tempIdsFiltered.length;
      const nHumidity = humidityIdsFiltered.length;
      const nContact = contactIdsFiltered.length;
      const totalT = tempIds.length;
      const totalH = humidityIds.length;
      const totalC = contactIds.length;
      const shown =
        (showTemp ? nTemp : 0) + (showHumidity ? nHumidity : 0) + (showContact ? nContact : 0);
      const totalAll = totalT + totalH + totalC;
      const hasFilter = !!q || kind !== 'all';
      if (hasFilter) {
        countEl.textContent =
          shown +
          ' affich\u00E9' +
          (shown !== 1 ? 's' : '') +
          ' sur ' +
          totalAll +
          ' au total';
      } else {
        countEl.textContent =
          totalAll + ' capteur' + (totalAll !== 1 ? 's' : '') + ' au total';
      }
    }
  }

  function reapplyFiltersOnly() {
    if (lastSensorState) renderSensors(lastSensorState, lastDisplayDoc);
  }

  async function refresh() {
    errEl.classList.remove('visible');
    errEl.textContent = '';
    if (btnRefresh) btnRefresh.classList.add('is-loading');
    if (brokerPill) brokerPill.classList.add('is-syncing');
    try {
      const [resState, resDisp] = await Promise.all([
        fetch('/api/state'),
        fetch('/api/sensor-display'),
      ]);
      if (!resState.ok) throw new Error(await resState.text());
      if (!resDisp.ok) throw new Error(await resDisp.text());
      const st = await resState.json();
      const disp = await resDisp.json();
      renderSensors(st, disp);
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
        updatedEl.textContent =
          consecutiveFails >= 2
            ? 'Hors ligne ou erreur répétée'
            : 'Erreur : ' + new Date().toLocaleTimeString('fr-FR');
      }
    } finally {
      if (btnRefresh) btnRefresh.classList.remove('is-loading');
      if (brokerPill) brokerPill.classList.remove('is-syncing');
    }
  }

  async function syncFromState() {
    errEl.classList.remove('visible');
    errEl.textContent = '';
    if (btnSync) btnSync.disabled = true;
    try {
      const res = await fetch('/api/sensor-display/sync-from-state', { method: 'POST' });
      if (!res.ok) throw new Error(await res.text());
      const disp = await res.json();
      if (lastSensorState) renderSensors(lastSensorState, disp);
      else await refresh();
    } catch (e) {
      errEl.textContent = 'Synchro impossible : ' + (e && e.message ? e.message : e);
      errEl.classList.add('visible');
    } finally {
      if (btnSync) btnSync.disabled = false;
    }
  }

  async function saveLabels() {
    errEl.classList.remove('visible');
    errEl.textContent = '';
    if (btnSave) btnSave.disabled = true;
    try {
      const payload = buildPayloadFromInputs();
      const res = await fetch('/api/sensor-display', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
      if (!res.ok) throw new Error(await res.text());
      const disp = await res.json();
      lastDisplayDoc = disp;
      if (lastSensorState) renderSensors(lastSensorState, disp);
    } catch (e) {
      errEl.textContent = 'Enregistrement impossible : ' + (e && e.message ? e.message : e);
      errEl.classList.add('visible');
    } finally {
      if (btnSave) btnSave.disabled = false;
    }
  }

  function schedule() {
    if (timer) clearInterval(timer);
    timer = null;
    const ms =
      typeof window.rhGetRefreshIntervalMs === 'function' ? window.rhGetRefreshIntervalMs() : 4000;
    if (ms > 0) timer = setInterval(refresh, ms);
  }

  restoreFilterPrefs();
  const searchInp = $('sensors-search');
  const kindSel = $('sensors-kind-filter');
  if (searchInp) {
    searchInp.addEventListener('input', function () {
      saveFilterPrefs();
      reapplyFiltersOnly();
    });
  }
  if (kindSel) {
    kindSel.addEventListener('change', function () {
      saveFilterPrefs();
      reapplyFiltersOnly();
    });
  }

  function saveFilterPrefs() {
    try {
      const inp = $('sensors-search');
      const sel = $('sensors-kind-filter');
      if (inp) localStorage.setItem(STORAGE_Q, inp.value);
      if (sel) localStorage.setItem(STORAGE_KIND, sel.value);
    } catch (e) {
      /* ignore */
    }
  }

  function showObsToast(msg) {
    if (!toastObs) return;
    toastObs.textContent = msg;
    toastObs.classList.add('visible');
    if (obsToastTimer) clearTimeout(obsToastTimer);
    obsToastTimer = setTimeout(function () {
      toastObs.classList.remove('visible');
      toastObs.textContent = '';
    }, 3500);
  }

  function syncObsKindRows() {
    const k = obsKind ? obsKind.value : 'temperature';
    const rowMotion = $('obs-row-motion-room');
    const rowTemp = $('obs-row-temp');
    const rowHum = $('obs-row-humidity');
    const rowContact = $('obs-row-contact');
    if (rowMotion) rowMotion.hidden = k !== 'motion';
    if (rowTemp) rowTemp.hidden = k !== 'temperature';
    if (rowHum) rowHum.hidden = k !== 'humidity';
    if (rowContact) rowContact.hidden = k !== 'contact';
  }

  function setObservationFormDisabled(disabled) {
    if (formObs) {
      const els = formObs.querySelectorAll('input, select, button');
      for (let i = 0; i < els.length; i++) {
        els[i].disabled = disabled;
      }
    }
  }

  if (!brokerAvailable) {
    setObservationFormDisabled(true);
    if (obsSubmit) obsSubmit.title = 'Nécessite rusthome serve avec broker MQTT intégré';
  }

  if (obsKind) {
    obsKind.addEventListener('change', syncObsKindRows);
    syncObsKindRows();
  }

  if (formObs && brokerAvailable) {
    formObs.addEventListener('submit', async function (ev) {
      ev.preventDefault();
      errEl.classList.remove('visible');
      errEl.textContent = '';
      const kind = obsKind ? obsKind.value : 'temperature';
      const entity = ($('obs-entity') && $('obs-entity').value) || '';
      const payload = { kind: kind, entity: entity };
      if (kind === 'motion') {
        const r = $('obs-room') && $('obs-room').value.trim();
        if (r) payload.room = r;
      } else if (kind === 'temperature') {
        const c = $('obs-celsius') && $('obs-celsius').value;
        const n = c === '' ? NaN : Number(c);
        if (Number.isFinite(n)) payload.celsius = n;
      } else if (kind === 'humidity') {
        const p = $('obs-percent-rh') && $('obs-percent-rh').value;
        const n = p === '' ? NaN : Number(p);
        if (Number.isFinite(n)) payload.percent_rh = n;
      } else if (kind === 'contact') {
        payload.open = ($('obs-open') && $('obs-open').checked) || false;
      }
      if (obsSubmit) obsSubmit.disabled = true;
      try {
        const res = await fetch('/api/observation', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(payload),
        });
        if (!res.ok) {
          const msg = await res.text();
          throw new Error(
            res.status === 503
              ? 'Broker indisponible (lancez rusthome serve sans --no-broker). ' + msg
              : msg,
          );
        }
        showObsToast('Observation publiée sur MQTT');
        setTimeout(refresh, 200);
      } catch (e) {
        errEl.textContent =
          'Publication impossible : ' + (e && e.message ? e.message : e);
        errEl.classList.add('visible');
      } finally {
        if (obsSubmit) obsSubmit.disabled = false;
      }
    });
  }

  function restoreFilterPrefs() {
    const inp = $('sensors-search');
    const sel = $('sensors-kind-filter');
    try {
      const q = localStorage.getItem(STORAGE_Q);
      if (inp && q !== null) inp.value = q;
      const k = localStorage.getItem(STORAGE_KIND);
      if (sel && (k === 'all' || k === 'temperature' || k === 'humidity' || k === 'contact'))
        sel.value = k;
    } catch (e) {
      /* ignore */
    }
  }

  if (btnRefresh) btnRefresh.addEventListener('click', function () {
    refresh();
  });
  if (btnSync) btnSync.addEventListener('click', function () {
    syncFromState();
  });
  if (btnSave) btnSave.addEventListener('click', function () {
    saveLabels();
  });
  window.addEventListener('rh-refresh-interval-changed', schedule);
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
      /* reconnect automatique */
    };
  }
})();
