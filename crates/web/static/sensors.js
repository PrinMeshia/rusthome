(function () {
  const $ = (id) => document.getElementById(id);
  const errEl = $('error-bar');
  const btnRefresh = $('btn-refresh');
  const updatedEl = $('updated-at');
  let timer = null;
  let consecutiveFails = 0;

  function fmtCelsius(millideg) {
    return (millideg / 1000).toFixed(1) + '\u00B0C';
  }

  function renderSensors(st) {
    const tempBody = $('temp-body');
    tempBody.replaceChildren();
    const temps = st.temperatures || {};
    const tempKeys = Object.keys(temps).sort();
    if (tempKeys.length === 0) {
      const tr = document.createElement('tr');
      const td = document.createElement('td');
      td.colSpan = 2;
      td.className = 'cell-empty';
      const em = document.createElement('em');
      em.textContent = 'Aucun capteur de température';
      td.appendChild(em);
      tr.appendChild(td);
      tempBody.appendChild(tr);
    } else {
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
        tr.appendChild(tdName);
        tr.appendChild(tdVal);
        tempBody.appendChild(tr);
      }
    }

    const contactBody = $('contact-body');
    contactBody.replaceChildren();
    const contacts = st.contacts || {};
    const contactKeys = Object.keys(contacts).sort();
    if (contactKeys.length === 0) {
      const tr = document.createElement('tr');
      const td = document.createElement('td');
      td.colSpan = 2;
      td.className = 'cell-empty';
      const em = document.createElement('em');
      em.textContent = 'Aucun contact';
      td.appendChild(em);
      tr.appendChild(td);
      contactBody.appendChild(tr);
    } else {
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
        tr.appendChild(tdName);
        tr.appendChild(tdVal);
        contactBody.appendChild(tr);
      }
    }
  }

  async function refresh() {
    errEl.classList.remove('visible');
    errEl.textContent = '';
    if (btnRefresh) btnRefresh.classList.add('is-loading');
    try {
      const res = await fetch('/api/state');
      if (!res.ok) throw new Error(await res.text());
      renderSensors(await res.json());
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
        updatedEl.textContent = consecutiveFails >= 2
          ? 'Hors ligne ou erreur répétée'
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
  window.addEventListener('rh-refresh-interval-changed', schedule);
  schedule();
  refresh();
})();
