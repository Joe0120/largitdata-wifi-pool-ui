// ============================================================
// State
// ============================================================
let allDevices = [];
let selectedSerials = new Set();
let viewingSerials = new Set();
let refreshTimer = null;
const deviceSizes = {}; // serial -> {width, height}

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
// Sidebar: Device list
// ============================================================
function renderDeviceList() {
  const el = document.getElementById('deviceList');
  el.innerHTML = '';

  // Group by model
  const groups = {};
  for (const dev of allDevices) {
    const m = dev.model || 'Unknown';
    if (!groups[m]) groups[m] = [];
    groups[m].push(dev);
  }

  for (const [model, devs] of Object.entries(groups)) {
    // Model header with checkbox
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
      const checked = e.target.checked;
      for (const d of devs) {
        if (checked) selectedSerials.add(d.serial);
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

// Auto-start viewing when selection changes
function autoStartViewing() {
  if (selectedSerials.size === 0) {
    stopViewing();
    return;
  }
  startViewing();
}

function selectAll() {
  allDevices.forEach(d => selectedSerials.add(d.serial));
  renderDeviceList();
  autoStartViewing();
}
function selectNone() {
  selectedSerials.clear();
  renderDeviceList();
  autoStartViewing();
}

// ============================================================
// Viewing: start/stop (incremental)
// ============================================================
function startViewing() {
  if (selectedSerials.size === 0) { stopViewing(); return; }

  // Find what to add and remove
  const toAdd = [...selectedSerials].filter(s => !viewingSerials.has(s));
  const toRemove = [...viewingSerials].filter(s => !selectedSerials.has(s));

  // Remove cards for deselected devices
  for (const serial of toRemove) {
    viewingSerials.delete(serial);
    const card = document.getElementById(`card-${serial}`);
    if (card) card.remove();
  }

  // Add cards for newly selected devices
  const grid = document.getElementById('grid');
  // Clear placeholder if present
  if (toAdd.length > 0 && grid.querySelector('div[style]') && viewingSerials.size === 0) {
    grid.innerHTML = '';
  }

  for (const serial of toAdd) {
    viewingSerials.add(serial);
    addDeviceCard(serial);
    fetchWindowSize(serial);
    loadScreenshot(serial);
  }

  // Update columns and refresh
  const count = viewingSerials.size;
  if (count > 0) {
    const cols = document.getElementById('colSlider').value;
    grid.style.gridTemplateColumns = `repeat(${cols}, 1fr)`;
    startAutoRefresh();
  }
  document.getElementById('stViewing').textContent = `Viewing: ${count}`;
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
  // Don't add duplicate
  if (document.getElementById(`card-${serial}`)) return;

  const card = document.createElement('div');
  card.className = 'device-card';
  card.id = `card-${serial}`;
  card.innerHTML = `
    <div class="card-header">
      <span>${serial}</span>
    </div>
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
  const screen = card.querySelector('.card-screen');
  setupTouchHandlers(screen, serial);
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
async function loadScreenshot(serial) {
  const img = document.getElementById(`img-${serial}`);
  if (!img) return;
  try {
    const url = `${API()}/api/devices/${serial}/screenshot?t=${Date.now()}`;
    const resp = await fetch(url);
    if (!resp.ok) return;
    const blob = await resp.blob();
    const objUrl = URL.createObjectURL(blob);
    const oldUrl = img.src;
    img.src = objUrl;
    if (oldUrl && oldUrl.startsWith('blob:')) URL.revokeObjectURL(oldUrl);
    // Remove "Loading..." on first image
    const noSig = img.parentElement.querySelector('.no-signal');
    if (noSig) noSig.remove();
  } catch (e) { /* keep existing */ }
}

function startAutoRefresh() {
  if (refreshTimer) clearInterval(refreshTimer);
  const serials = [...viewingSerials];
  if (serials.length === 0) return;
  let idx = 0;
  // Stagger: one device at a time, round-robin, fast interval
  // 15 devices × 400ms each ≈ 6s per full round
  const interval = 200;
  refreshTimer = setInterval(() => {
    if (idx >= serials.length) idx = 0;
    // Skip if this device has a pending action screenshot
    if (!actionPending.has(serials[idx])) {
      loadScreenshot(serials[idx]);
    }
    idx++;
  }, interval);
}

// Track devices with pending action screenshots to avoid double-fetching
const actionPending = new Set();

// Quick refresh after tap/swipe — bypasses the round-robin queue
function actionRefresh(serial) {
  actionPending.add(serial);
  loadScreenshot(serial);
  setTimeout(() => loadScreenshot(serial), 400);
  setTimeout(() => {
    loadScreenshot(serial);
    actionPending.delete(serial);
  }, 900);
}

// ============================================================
// Touch: click = tap, drag = swipe
// ============================================================
let dragState = null;

// Calculate the actual image display area within a container using object-fit: contain
function getImageContentRect(img) {
  const containerRect = img.getBoundingClientRect();
  const natW = img.naturalWidth || 1;
  const natH = img.naturalHeight || 1;
  const contW = containerRect.width;
  const contH = containerRect.height;

  const scale = Math.min(contW / natW, contH / natH);
  const imgW = natW * scale;
  const imgH = natH * scale;
  const offsetX = (contW - imgW) / 2;
  const offsetY = (contH - imgH) / 2;

  return {
    left: containerRect.left + offsetX,
    top: containerRect.top + offsetY,
    width: imgW,
    height: imgH,
  };
}

// Convert mouse event to image-relative ratio (0~1), accounting for black bars
function mouseToRatio(e, img) {
  const r = getImageContentRect(img);
  const xR = Math.max(0, Math.min(1, (e.clientX - r.left) / r.width));
  const yR = Math.max(0, Math.min(1, (e.clientY - r.top) / r.height));
  return { xR, yR };
}

function setupTouchHandlers(screen, serial) {
  screen.addEventListener('mousedown', (e) => {
    if (e.button !== 0) return;
    e.preventDefault();
    const img = screen.querySelector('img');
    if (!img || !img.src) return;
    const { xR, yR } = mouseToRatio(e, img);
    dragState = {
      serial, screen, img,
      startXR: xR,
      startYR: yR,
      startCX: e.clientX, startCY: e.clientY,
      startTime: Date.now(),
      moved: false, trail: null,
    };
    const card = document.getElementById(`card-${serial}`);
    if (card) card.classList.add('touching');
  });

  screen.addEventListener('contextmenu', (e) => {
    e.preventDefault();
    sendCommand(serial, 'back');
  });
}

document.addEventListener('mousemove', (e) => {
  if (!dragState) return;
  const dx = e.clientX - dragState.startCX;
  const dy = e.clientY - dragState.startCY;
  if (Math.abs(dx) > 5 || Math.abs(dy) > 5) {
    dragState.moved = true;
    const screenRect = dragState.screen.getBoundingClientRect();
    const ex = e.clientX - screenRect.left;
    const ey = e.clientY - screenRect.top;
    if (!dragState.trail) {
      const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
      svg.style.cssText = 'position:absolute;top:0;left:0;width:100%;height:100%;pointer-events:none;z-index:20';
      svg.setAttribute('viewBox', `0 0 ${screenRect.width} ${screenRect.height}`);
      const polyline = document.createElementNS('http://www.w3.org/2000/svg', 'polyline');
      polyline.setAttribute('stroke', '#e94560');
      polyline.setAttribute('stroke-width', '2');
      polyline.setAttribute('stroke-linecap', 'round');
      polyline.setAttribute('fill', 'none');
      polyline.setAttribute('opacity', '0.8');
      const sx = dragState.startCX - screenRect.left;
      const sy = dragState.startCY - screenRect.top;
      const dot = document.createElementNS('http://www.w3.org/2000/svg', 'circle');
      dot.setAttribute('cx', sx); dot.setAttribute('cy', sy);
      dot.setAttribute('r', '3'); dot.setAttribute('fill', '#e94560');
      svg.appendChild(polyline); svg.appendChild(dot);
      dragState.screen.appendChild(svg);
      dragState.trail = { svg, polyline, points: `${sx},${sy}` };
    }
    dragState.trail.points += ` ${ex},${ey}`;
    dragState.trail.polyline.setAttribute('points', dragState.trail.points);
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
    const x1 = Math.round(startXR * size.width);
    const y1 = Math.round(startYR * size.height);
    const x2 = Math.round(endXR * size.width);
    const y2 = Math.round(endYR * size.height);
    const dur = Math.max(100, Math.min(Date.now() - startTime, 1000));
    fetch(`${API()}/api/devices/${serial}/swipe`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ x1, y1, x2, y2, duration_ms: dur })
    }).then(() => actionRefresh(serial));
    setStatus(`Swipe ${serial} (${x1},${y1})→(${x2},${y2})`);
  } else {
    const x = Math.round(startXR * size.width);
    const y = Math.round(startYR * size.height);
    fetch(`${API()}/api/devices/${serial}/tap`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ x, y })
    }).then(() => actionRefresh(serial));
    // Tap indicator
    const screenRect = screen.getBoundingClientRect();
    showTapIndicator(screen, e.clientX - screenRect.left, e.clientY - screenRect.top);
    setStatus(`Tap ${serial} @ (${x}, ${y})`);
  }
  dragState = null;
});

function showTapIndicator(screen, x, y) {
  const dot = document.createElement('div');
  dot.className = 'tap-indicator';
  dot.style.left = x + 'px';
  dot.style.top = y + 'px';
  screen.appendChild(dot);
  setTimeout(() => dot.remove(), 500);
}

// ============================================================
// Commands
// ============================================================
function sendCommand(serial, command) {
  const keyMap = { home: 3, back: 4, appSwitch: 187 };
  const keycode = keyMap[command];
  if (keycode) {
    fetch(`${API()}/api/devices/${serial}/key`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ keycode })
    }).then(() => actionRefresh(serial));
  }
  setStatus(`${command} → ${serial}`);
}

function sendText(serial) {
  const input = document.getElementById(`text-${serial}`);
  const text = input.value;
  if (!text) return;
  fetch(`${API()}/api/devices/${serial}/text`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ text })
  }).then(() => { input.value = ''; actionRefresh(serial); });
  setStatus(`Text → ${serial}`);
}

function forcePortrait(serial) {
  fetch(`${API()}/api/devices/${serial}/rotate`, { method: 'POST' })
    .then(() => actionRefresh(serial));
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
// SIM
// ============================================================
function populateSimSlots() {
  const sel = document.getElementById('simOrder');
  sel.innerHTML = '';
  for (let i = 1; i <= 16; i++) {
    sel.innerHTML += `<option value="${i}">${i}</option>`;
  }
}

function simLog(msg, cls) {
  const el = document.getElementById('simLog');
  const time = new Date().toLocaleTimeString();
  el.innerHTML += `<div class="${cls || ''}">[${time}] ${msg}</div>`;
  el.scrollTop = el.scrollHeight;
}

async function switchAllSim() {
  const order = document.getElementById('simOrder').value;
  if (!confirm(`Switch ALL devices to SIM slot ${order}?`)) return;
  simLog(`Switching all to slot ${order}...`, 'info');
  try {
    const resp = await fetch(`${API()}/api/sim/switch-all`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ sim_order: parseInt(order) })
    });
    const data = await resp.json();
    simLog(data.output || 'Done', data.ok ? 'ok' : 'fail');
  } catch(e) {
    simLog(`Error: ${e.message}`, 'fail');
  }
}

async function viewCurrentSim() {
  simLog('Querying current SIMs...', 'info');
  try {
    const resp = await fetch(`${API()}/api/sim/current`);
    const data = await resp.json();
    simLog(data.output || 'No data', 'ok');
  } catch(e) {
    simLog(`Error: ${e.message}`, 'fail');
  }
}

// ============================================================
// Status
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
    e.preventDefault();
    startX = e.clientX;
    startW = sidebar.offsetWidth;
    resizer.classList.add('active');
    document.addEventListener('mousemove', onDrag);
    document.addEventListener('mouseup', onStop);
  });

  function onDrag(e) {
    const w = Math.max(60, Math.min(500, startW + e.clientX - startX));
    sidebar.style.width = w + 'px';
  }
  function onStop() {
    resizer.classList.remove('active');
    document.removeEventListener('mousemove', onDrag);
    document.removeEventListener('mouseup', onStop);
  }
})();

// ============================================================
// Boot
// ============================================================
init();
