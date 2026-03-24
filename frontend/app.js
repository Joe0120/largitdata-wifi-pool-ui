// ============================================================
// State
// ============================================================
let allDevices = [];
let selectedSerials = new Set();
let viewingSerials = new Set();
let refreshTimer = null;
const deviceSizes = {};

let notifications = [];
let nextNotifId = 1;

const API = () => window.location.origin;

// ============================================================
// Init
// ============================================================
async function init() {
  await fetchDevices();
  populateSimSlots();
}

async function fetchDevices() {
  try {
    const resp = await fetch(`${API()}/api/devices`);
    allDevices = await resp.json();
    allDevices.sort((a, b) => (a.model || '').localeCompare(b.model || '') || a.serial.localeCompare(b.serial));
    renderDeviceList();
    document.getElementById('stDevices').textContent = `Devices: ${allDevices.length}`;
  } catch (e) {
    document.getElementById('deviceList').textContent = `Error: ${e.message}`;
  }
}

// ============================================================
// Toast + Notification
// ============================================================
function toast(message, type = 'info') {
  // Toast popup
  const container = document.getElementById('toastContainer');
  const el = document.createElement('div');
  el.className = `toast ${type}`;
  el.textContent = message;
  container.appendChild(el);
  setTimeout(() => {
    el.classList.add('fade-out');
    setTimeout(() => el.remove(), 300);
  }, 3000);

  // Also add to notification history
  notify(message, type);
}

function notify(message, type = 'info') {
  const id = nextNotifId++;
  notifications.unshift({
    id,
    time: new Date().toLocaleTimeString(),
    type,
    message,
    read: false,
  });
  // Keep max 100
  if (notifications.length > 100) notifications.pop();
  updateNotifBadge();
  renderNotifList();
}

function markAsRead(id) {
  const n = notifications.find(n => n.id === id);
  if (n) n.read = true;
  updateNotifBadge();
  renderNotifList();
}

function markAllRead() {
  notifications.forEach(n => n.read = true);
  updateNotifBadge();
  renderNotifList();
}

function clearAllNotifs() {
  notifications = [];
  updateNotifBadge();
  renderNotifList();
}

function updateNotifBadge() {
  const unread = notifications.filter(n => !n.read).length;
  const badge = document.getElementById('notifBadge');
  if (unread > 0) {
    badge.textContent = unread > 99 ? '99+' : unread;
    badge.style.display = 'flex';
  } else {
    badge.style.display = 'none';
  }
}

function renderNotifList() {
  const list = document.getElementById('notifList');
  if (notifications.length === 0) {
    list.innerHTML = '<div class="notif-empty">No notifications</div>';
    return;
  }
  list.innerHTML = '';
  for (const n of notifications) {
    const item = document.createElement('div');
    item.className = `notif-item ${n.read ? 'read' : 'unread'}`;
    item.addEventListener('click', (e) => {
      // Don't mark as read if user is selecting text
      if (window.getSelection().toString()) return;
      markAsRead(n.id);
    });
    item.innerHTML = `
      <div class="notif-type ${n.type}">${n.type.toUpperCase()}</div>
      <div class="notif-time">${n.time}</div>
      <div class="notif-msg">${escapeHtml(n.message)}</div>
    `;
    list.appendChild(item);
  }
}

function escapeHtml(s) {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

// ============================================================
// Dropdown toggle
// ============================================================
function toggleDropdown(id) {
  const menu = document.getElementById(id + 'Menu');
  const wasOpen = menu.classList.contains('open');
  // Close all dropdowns first
  document.querySelectorAll('.dropdown-menu').forEach(m => m.classList.remove('open'));
  if (!wasOpen) menu.classList.add('open');
}

// Close dropdowns when clicking outside — but NOT when selecting text inside notif
document.addEventListener('mousedown', (e) => {
  // Don't close if clicking inside a dropdown menu
  if (e.target.closest('.dropdown-menu')) return;
  // Don't close if clicking on a dropdown trigger (toggleDropdown handles that)
  if (e.target.closest('.dropdown-trigger')) return;
  document.querySelectorAll('.dropdown-menu').forEach(m => m.classList.remove('open'));
});

// ============================================================
// Sidebar: Device list
// ============================================================
function renderDeviceList() {
  const el = document.getElementById('deviceList');
  el.innerHTML = '';
  const groups = {};
  for (const dev of allDevices) {
    const m = dev.model || 'Unknown';
    if (!groups[m]) groups[m] = [];
    groups[m].push(dev);
  }
  for (const [model, devs] of Object.entries(groups)) {
    const hdr = document.createElement('div');
    hdr.className = 'model-header';
    const allChecked = devs.every(d => selectedSerials.has(d.serial));
    hdr.innerHTML = `<span>${model} (${devs.length})</span><input type="checkbox" ${allChecked ? 'checked' : ''}>`;
    hdr.addEventListener('click', (e) => {
      if (e.target.tagName === 'INPUT') return;
      const cb = hdr.querySelector('input');
      cb.checked = !cb.checked;
      cb.dispatchEvent(new Event('change'));
    });
    hdr.querySelector('input').addEventListener('change', (e) => {
      for (const d of devs) {
        if (e.target.checked) selectedSerials.add(d.serial);
        else selectedSerials.delete(d.serial);
      }
      renderDeviceList();
      autoStartViewing();
    });
    el.appendChild(hdr);
    for (const dev of devs) {
      const item = document.createElement('div');
      item.className = `device-item${selectedSerials.has(dev.serial) ? ' checked' : ''}`;
      item.innerHTML = `<input type="checkbox" ${selectedSerials.has(dev.serial) ? 'checked' : ''}><span>${dev.serial}</span>`;
      item.addEventListener('click', (e) => {
        if (e.target.tagName === 'INPUT') return;
        const cb = item.querySelector('input');
        cb.checked = !cb.checked;
        cb.dispatchEvent(new Event('change'));
      });
      item.querySelector('input').addEventListener('change', (e) => {
        if (e.target.checked) { selectedSerials.add(dev.serial); item.classList.add('checked'); }
        else { selectedSerials.delete(dev.serial); item.classList.remove('checked'); }
        autoStartViewing();
      });
      el.appendChild(item);
    }
  }
}

function autoStartViewing() {
  if (selectedSerials.size === 0) { stopViewing(); return; }
  startViewing();
}
function selectAll() { allDevices.forEach(d => selectedSerials.add(d.serial)); renderDeviceList(); autoStartViewing(); }
function selectNone() { selectedSerials.clear(); renderDeviceList(); autoStartViewing(); }

// ============================================================
// Viewing: start/stop (incremental)
// ============================================================
function startViewing() {
  if (selectedSerials.size === 0) { stopViewing(); return; }
  const toAdd = [...selectedSerials].filter(s => !viewingSerials.has(s));
  const toRemove = [...viewingSerials].filter(s => !selectedSerials.has(s));
  for (const serial of toRemove) {
    viewingSerials.delete(serial);
    const card = document.getElementById(`card-${serial}`);
    if (card) card.remove();
  }
  const grid = document.getElementById('grid');
  if (toAdd.length > 0 && grid.querySelector('div[style]') && viewingSerials.size === 0) grid.innerHTML = '';
  for (const serial of toAdd) {
    viewingSerials.add(serial);
    addDeviceCard(serial);
    fetchWindowSize(serial);
    loadScreenshot(serial);
  }
  if (viewingSerials.size > 0) {
    grid.style.gridTemplateColumns = `repeat(${document.getElementById('colSlider').value}, 1fr)`;
    startAutoRefresh();
  }
  document.getElementById('stViewing').textContent = `Viewing: ${viewingSerials.size}`;
}

function stopViewing() {
  if (refreshTimer) { clearInterval(refreshTimer); refreshTimer = null; }
  viewingSerials.clear();
  document.getElementById('grid').innerHTML = '<div style="color:#333;padding:40px;text-align:center">Select devices to view</div>';
  document.getElementById('stViewing').textContent = 'Viewing: 0';
}

// ============================================================
// Grid
// ============================================================
function addDeviceCard(serial) {
  const grid = document.getElementById('grid');
  if (document.getElementById(`card-${serial}`)) return;
  const card = document.createElement('div');
  card.className = 'device-card';
  card.id = `card-${serial}`;
  card.innerHTML = `
    <div class="card-header"><span>${serial}</span></div>
    <div class="card-screen" id="screen-${serial}">
      <img id="img-${serial}" draggable="false">
      <span class="no-signal">Loading...</span>
    </div>
    <div class="card-nav">
      <button onclick="sendCommand('${serial}','appSwitch')" title="Recent">&#9776;</button>
      <button onclick="sendCommand('${serial}','home')" title="Home">&#9679;</button>
      <button onclick="sendCommand('${serial}','back')" title="Back">&#9668;</button>
      <button onclick="forcePortrait('${serial}')" title="Portrait">&#8635;</button>
    </div>
    <div class="card-text">
      <input type="text" placeholder="text..." id="text-${serial}"
             onkeydown="if(event.key==='Enter'){sendText('${serial}');event.preventDefault()}">
      <button onclick="sendText('${serial}')">Send</button>
    </div>
  `;
  setupTouchHandlers(card.querySelector('.card-screen'), serial);
  grid.appendChild(card);
}

function updateCols() {
  const cols = document.getElementById('colSlider').value;
  document.getElementById('colLabel').textContent = cols;
  document.getElementById('grid').style.gridTemplateColumns = `repeat(${cols}, 1fr)`;
}

// ============================================================
// Screenshot polling
// ============================================================
const screenshotInFlight = new Set();

async function loadScreenshot(serial) {
  if (screenshotInFlight.has(serial)) return;
  const img = document.getElementById(`img-${serial}`);
  if (!img) return;
  screenshotInFlight.add(serial);
  try {
    const resp = await fetch(`${API()}/api/devices/${serial}/screenshot?t=${Date.now()}`);
    if (!resp.ok) return;
    const blob = await resp.blob();
    const objUrl = URL.createObjectURL(blob);
    const oldUrl = img.src;
    img.src = objUrl;
    if (oldUrl && oldUrl.startsWith('blob:')) URL.revokeObjectURL(oldUrl);
    const noSig = img.parentElement.querySelector('.no-signal');
    if (noSig) noSig.remove();
  } catch (e) {}
  finally { screenshotInFlight.delete(serial); }
}

function startAutoRefresh() {
  if (refreshTimer) clearInterval(refreshTimer);
  const serials = [...viewingSerials];
  if (serials.length === 0) return;
  let idx = 0;
  const interval = 200;
  refreshTimer = setInterval(() => {
    if (idx >= serials.length) idx = 0;
    if (!actionPending.has(serials[idx])) loadScreenshot(serials[idx]);
    idx++;
  }, interval);
}

const actionPending = new Set();
function actionRefresh(serial) {
  actionPending.add(serial);
  loadScreenshot(serial);
  setTimeout(() => { loadScreenshot(serial); actionPending.delete(serial); }, 800);
}

// ============================================================
// Touch: click = tap, drag = swipe
// ============================================================
let dragState = null;

function getImageContentRect(img) {
  const cr = img.getBoundingClientRect();
  const natW = img.naturalWidth || 1, natH = img.naturalHeight || 1;
  const scale = Math.min(cr.width / natW, cr.height / natH);
  const imgW = natW * scale, imgH = natH * scale;
  return { left: cr.left + (cr.width - imgW) / 2, top: cr.top + (cr.height - imgH) / 2, width: imgW, height: imgH };
}

function mouseToRatio(e, img) {
  const r = getImageContentRect(img);
  return { xR: Math.max(0, Math.min(1, (e.clientX - r.left) / r.width)), yR: Math.max(0, Math.min(1, (e.clientY - r.top) / r.height)) };
}

function setupTouchHandlers(screen, serial) {
  screen.addEventListener('mousedown', (e) => {
    if (e.button !== 0) return;
    e.preventDefault();
    const img = screen.querySelector('img');
    if (!img || !img.src) return;
    const { xR, yR } = mouseToRatio(e, img);
    dragState = { serial, screen, img, startXR: xR, startYR: yR, startCX: e.clientX, startCY: e.clientY, startTime: Date.now(), moved: false, trail: null };
    const card = document.getElementById(`card-${serial}`);
    if (card) card.classList.add('touching');
  });
  screen.addEventListener('contextmenu', (e) => { e.preventDefault(); sendCommand(serial, 'back'); });
}

document.addEventListener('mousemove', (e) => {
  if (!dragState) return;
  if (Math.abs(e.clientX - dragState.startCX) > 5 || Math.abs(e.clientY - dragState.startCY) > 5) {
    dragState.moved = true;
    const sr = dragState.screen.getBoundingClientRect();
    const ex = e.clientX - sr.left, ey = e.clientY - sr.top;
    if (!dragState.trail) {
      const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
      svg.style.cssText = 'position:absolute;top:0;left:0;width:100%;height:100%;pointer-events:none;z-index:20';
      svg.setAttribute('viewBox', `0 0 ${sr.width} ${sr.height}`);
      const pl = document.createElementNS('http://www.w3.org/2000/svg', 'polyline');
      pl.setAttribute('stroke', '#e94560'); pl.setAttribute('stroke-width', '2');
      pl.setAttribute('stroke-linecap', 'round'); pl.setAttribute('fill', 'none'); pl.setAttribute('opacity', '0.8');
      const sx = dragState.startCX - sr.left, sy = dragState.startCY - sr.top;
      const dot = document.createElementNS('http://www.w3.org/2000/svg', 'circle');
      dot.setAttribute('cx', sx); dot.setAttribute('cy', sy); dot.setAttribute('r', '3'); dot.setAttribute('fill', '#e94560');
      svg.appendChild(pl); svg.appendChild(dot);
      dragState.screen.appendChild(svg);
      dragState.trail = { svg, pl, points: `${sx},${sy}` };
    }
    dragState.trail.points += ` ${ex},${ey}`;
    dragState.trail.pl.setAttribute('points', dragState.trail.points);
  }
});

document.addEventListener('mouseup', (e) => {
  if (!dragState) return;
  const { serial, screen, img, startXR, startYR, startTime, moved, trail } = dragState;
  const card = document.getElementById(`card-${serial}`);
  if (card) card.classList.remove('touching');
  if (trail) trail.svg.remove();
  const { xR: endXR, yR: endYR } = mouseToRatio(e, img);
  const size = deviceSizes[serial] || { width: 1080, height: 1920 };
  if (moved) {
    const x1 = Math.round(startXR * size.width), y1 = Math.round(startYR * size.height);
    const x2 = Math.round(endXR * size.width), y2 = Math.round(endYR * size.height);
    const dur = Math.max(100, Math.min(Date.now() - startTime, 1000));
    fetch(`${API()}/api/devices/${serial}/swipe`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ x1, y1, x2, y2, duration_ms: dur }) }).then(() => actionRefresh(serial));
    setStatus(`Swipe ${serial}`);
  } else {
    const x = Math.round(startXR * size.width), y = Math.round(startYR * size.height);
    fetch(`${API()}/api/devices/${serial}/tap`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ x, y }) }).then(() => actionRefresh(serial));
    const sr = screen.getBoundingClientRect();
    showTapIndicator(screen, e.clientX - sr.left, e.clientY - sr.top);
    setStatus(`Tap ${serial} (${x},${y})`);
  }
  dragState = null;
});

function showTapIndicator(screen, x, y) {
  const dot = document.createElement('div');
  dot.className = 'tap-indicator'; dot.style.left = x + 'px'; dot.style.top = y + 'px';
  screen.appendChild(dot); setTimeout(() => dot.remove(), 500);
}

// ============================================================
// Commands
// ============================================================
function sendCommand(serial, command) {
  const keyMap = { home: 3, back: 4, appSwitch: 187 };
  const keycode = keyMap[command];
  if (keycode) {
    fetch(`${API()}/api/devices/${serial}/key`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ keycode }) }).then(() => actionRefresh(serial));
  }
  setStatus(`${command} → ${serial}`);
}

function sendText(serial) {
  const input = document.getElementById(`text-${serial}`);
  const text = input.value;
  if (!text) return;
  fetch(`${API()}/api/devices/${serial}/text`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ text }) })
    .then(() => { input.value = ''; actionRefresh(serial); });
  setStatus(`Text → ${serial}`);
}

function forcePortrait(serial) {
  fetch(`${API()}/api/devices/${serial}/rotate`, { method: 'POST' }).then(() => actionRefresh(serial));
  setStatus(`Portrait → ${serial}`);
}

function forcePortraitAll() {
  for (const serial of viewingSerials) forcePortrait(serial);
}

async function fetchWindowSize(serial) {
  try {
    const resp = await fetch(`${API()}/api/devices/${serial}/window-size`);
    const data = await resp.json();
    if (data.width && data.height) deviceSizes[serial] = data;
  } catch(e) {}
}

// ============================================================
// SIM Switch
// ============================================================
let simDevices = []; // cached from /api/sim/devices

function populateSimSlots() {
  const sel = document.getElementById('simOrder');
  sel.innerHTML = '';
  for (let i = 1; i <= 16; i++) sel.innerHTML += `<option value="${i}">${i}</option>`;
  // Load SIM device data for search
  loadSimDevices();
}

async function loadSimDevices() {
  try {
    const resp = await fetch(`${API()}/api/sim/devices`);
    simDevices = await resp.json();
  } catch(e) {}
}

function setSimBusy(busy) {
  document.querySelectorAll('.sim-el').forEach(el => el.disabled = busy);
}

// --- Search ---
function onSimSearch() {
  const q = document.getElementById('simSearch').value.trim();
  const el = document.getElementById('simSearchResults');
  if (!q) { el.innerHTML = ''; return; }

  const results = [];
  for (const dev of simDevices) {
    for (const card of dev.card) {
      if (card.phone_number && card.phone_number.includes(q)) {
        results.push({ phone: card.phone_number, device_id: dev.device_id, sim_order: card.sim_order });
      }
    }
  }

  if (results.length === 0) {
    el.innerHTML = '<div class="sim-search-empty">No matches</div>';
    return;
  }

  el.innerHTML = '';
  for (const r of results.slice(0, 20)) {
    const item = document.createElement('div');
    item.className = 'sim-search-item';
    const highlighted = r.phone.replace(new RegExp(`(${escapeRegex(q)})`, 'g'), '<mark>$1</mark>');
    const shortId = r.device_id.slice(-6);
    item.innerHTML = `<span class="phone">${highlighted}</span><span class="device-tag">${shortId}</span>`;
    item.addEventListener('click', () => switchByPhone(r.phone, item));
    el.appendChild(item);
  }
}

function escapeRegex(s) { return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'); }

async function switchByPhone(phone, itemEl) {
  if (itemEl) itemEl.classList.add('busy');
  setSimBusy(true);
  toast(`Switching to ${phone}...`, 'info');
  try {
    const resp = await fetch(`${API()}/api/sim/switch-by-phone/${phone}`);
    const data = await resp.json();
    if (data.ok) {
      toast(`Switched ${data.device_id} to ${phone} (slot ${data.sim_order})`, 'success');
    } else {
      toast(`Switch failed: ${data.error || 'unknown'}`, 'error');
    }
  } catch(e) {
    toast(`Switch error: ${e.message}`, 'error');
  }
  if (itemEl) itemEl.classList.remove('busy');
  setSimBusy(false);
}

// --- Switch All ---
async function switchAllSim() {
  const order = document.getElementById('simOrder').value;
  if (!confirm(`Switch ALL devices to group ${order}?`)) return;
  setSimBusy(true);
  toast(`Switching all to group ${order}...`, 'info');
  try {
    const resp = await fetch(`${API()}/api/sim/switch-all`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ sim_order: parseInt(order) }) });
    const data = await resp.json();
    if (data.ok) {
      toast(`Group switch complete`, 'success');
    } else {
      toast(`Switch failed: ${data.error || 'unknown'}`, 'error');
    }
  } catch(e) {
    toast(`Switch error: ${e.message}`, 'error');
  }
  setSimBusy(false);
}

// --- View Current ---
async function viewCurrentSim() {
  setSimBusy(true);
  toast('Querying current SIMs...', 'info');
  try {
    const resp = await fetch(`${API()}/api/sim/current`);
    const data = await resp.json();
    toast('Current SIMs loaded', 'success');
    // Put full output in notification for copying
    notify(data.output || 'No data', 'info');
  } catch(e) {
    toast(`Query failed: ${e.message}`, 'error');
  }
  setSimBusy(false);
}

// ============================================================
// Status bar (for quiet actions like tap/swipe)
// ============================================================
function setStatus(msg) {
  document.getElementById('stInfo').textContent = msg;
}

// ============================================================
// Sidebar resize & toggle
// ============================================================
function toggleSidebar() {
  const sidebar = document.getElementById('sidebar');
  const btn = document.getElementById('sidebar-toggle');
  sidebar.classList.toggle('collapsed');
  btn.textContent = sidebar.classList.contains('collapsed') ? '▶' : '◀';
}

(function initSidebarResize() {
  const resizer = document.getElementById('sidebar-resizer');
  const sidebar = document.getElementById('sidebar');
  if (!resizer) return;
  let startX, startW;
  resizer.addEventListener('mousedown', (e) => {
    e.preventDefault(); startX = e.clientX; startW = sidebar.offsetWidth;
    resizer.classList.add('active');
    document.addEventListener('mousemove', onDrag);
    document.addEventListener('mouseup', onStop);
  });
  function onDrag(e) { sidebar.style.width = Math.max(60, Math.min(500, startW + e.clientX - startX)) + 'px'; }
  function onStop() { resizer.classList.remove('active'); document.removeEventListener('mousemove', onDrag); document.removeEventListener('mouseup', onStop); }
})();

// ============================================================
// Boot
// ============================================================
init();
