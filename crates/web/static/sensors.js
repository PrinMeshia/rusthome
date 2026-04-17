(function () {
  const $ = (id) => document.getElementById(id);
  const errEl = $('error-bar');
  let timer = null;

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
      em.textContent = 'No temperature sensors yet';
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
      em.textContent = 'No contact sensors yet';
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
        b.textContent = isOpen ? 'Open' : 'Closed';
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
    try {
      const res = await fetch('/api/state');
      if (!res.ok) throw new Error(await res.text());
      renderSensors(await res.json());
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
