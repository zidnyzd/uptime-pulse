// Client script untuk Admin Console UptimePulse
let monitorsMap = new Map();

// Wrapper fetch untuk otomatis menyertakan session cookie & Authorization Bearer
async function apiFetch(url, options = {}) {
  options.credentials = 'same-origin';
  options.headers = options.headers || {};
  const token = localStorage.getItem('uptime_token');
  if (token) {
    options.headers['Authorization'] = `Bearer ${token}`;
  }
  const res = await fetch(url, options);
  if (res.status === 401) {
    showLogin();
    throw new Error('Unauthorized');
  }
  return res;
}

// Cek autentikasi session saat halaman pertama dimuat
async function checkAuth() {
  try {
    const res = await apiFetch('/api/auth/me');
    const data = await res.json();
    if (data.authenticated) {
      showDashboard();
      await loadMonitors();
      initSSE();
    } else {
      showLogin();
    }
  } catch (e) {
    showLogin();
  }
}

function showLogin() {
  document.getElementById('login-modal').style.display = 'flex';
  document.getElementById('main-app').style.display = 'none';
  document.getElementById('login-pass').focus();
}

function showDashboard() {
  document.getElementById('login-modal').style.display = 'none';
  document.getElementById('main-app').style.display = 'block';
}

// Handler form login
async function handleLogin(e) {
  e.preventDefault();
  const username = document.getElementById('login-user').value;
  const password = document.getElementById('login-pass').value;
  const errBox = document.getElementById('login-err-msg');
  const submitBtn = document.getElementById('btn-login-submit');

  errBox.style.display = 'none';
  submitBtn.textContent = 'Signing in...';
  submitBtn.disabled = true;

  try {
    const res = await fetch('/api/auth/login', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username, password })
    });

    const data = await res.json();
    if (res.ok && data.success) {
      localStorage.setItem('uptime_token', data.token);
      showDashboard();
      await loadMonitors();
      initSSE();
    } else {
      errBox.textContent = data.error || 'Login gagal';
      errBox.style.display = 'block';
    }
  } catch (err) {
    errBox.textContent = 'Gagal menghubungi server: ' + err;
    errBox.style.display = 'block';
  } finally {
    submitBtn.textContent = 'Sign In';
    submitBtn.disabled = false;
  }
}

// Handler logout
async function handleLogout() {
  try {
    await apiFetch('/api/auth/logout', { method: 'POST' });
  } catch (e) {}
  localStorage.removeItem('uptime_token');
  window.location.reload();
}

// Memuat daftar monitor
async function loadMonitors() {
  try {
    const res = await apiFetch('/api/monitors');
    const list = await res.json();
    const container = document.getElementById('monitor-container');
    
    if (!list || list.length === 0) {
      container.innerHTML = `
        <div class="empty-state">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
            <circle cx="12" cy="12" r="10"></circle>
            <line x1="12" y1="8" x2="12" y2="12"></line>
            <line x1="12" y1="16" x2="12.01" y2="16"></line>
          </svg>
          <p>Belum ada monitor. Klik "Add Monitor" untuk menambahkan.</p>
        </div>
      `;
      updateBanner(0, 0, 0);
      return;
    }

    container.innerHTML = '';
    for (const m of list) {
      monitorsMap.set(m.id, m);
      container.appendChild(createMonitorCard(m));
      loadDetails(m.id);
    }
    recalcStats();
  } catch (err) {
    console.error('Failed to load monitors:', err);
  }
}

// Memuat detail monitor beserta sparkline riwayat latency
async function loadDetails(id) {
  try {
    const res = await apiFetch(`/api/monitors/${id}`);
    if (!res.ok) return;
    const data = await res.json();
    updateCardMetrics(data);
  } catch (e) {
    console.error('Failed to load detail for', id, e);
  }
}

// Membuat DOM elemen kartu monitor
function createMonitorCard(m) {
  const card = document.createElement('div');
  card.className = 'monitor-card';
  card.id = `card-${m.id}`;

  const statusClass = m.status === 'up' ? 'up' : (m.status === 'down' ? 'down' : 'paused');
  const latencyText = m.last_latency_ms ? `${m.last_latency_ms.toFixed(1)} ms` : '--';

  card.innerHTML = `
    <div class="monitor-info">
      <div class="status-dot ${statusClass}" id="dot-${m.id}"></div>
      <div>
        <div class="monitor-title">
          ${escapeHtml(m.name)}
          <span class="type-badge">${m.monitor_type}</span>
        </div>
        <div class="monitor-target" title="${escapeHtml(m.target)}">${escapeHtml(m.target)}</div>
      </div>
    </div>
    <div class="metric-group">
      <div class="metric">
        <div class="metric-value" id="lat-${m.id}">${latencyText}</div>
        <div class="metric-label">Latency</div>
      </div>
      <div class="metric">
        <div class="metric-value" id="upt-${m.id}">--%</div>
        <div class="metric-label">24h Uptime</div>
      </div>
    </div>
    <div class="bars-container" id="bars-${m.id}">
    </div>
    <div class="card-actions">
      <button class="btn-icon" title="Check Now" onclick="checkNow(${m.id})">
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M23 4v6h-6"></path>
          <path d="M1 20v-6h6"></path>
          <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"></path>
        </svg>
      </button>
      <button class="btn-icon" title="Pause / Resume" onclick="togglePause(${m.id})">
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="12" cy="12" r="10"></circle>
          <line x1="10" y1="15" x2="10" y2="9"></line>
          <line x1="14" y1="15" x2="14" y2="9"></line>
        </svg>
      </button>
      <button class="btn-icon danger" title="Delete" onclick="deleteMonitor(${m.id})">
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <polyline points="3 6 5 6 21 6"></polyline>
          <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
        </svg>
      </button>
    </div>
  `;
  return card;
}

// Memperbarui metrik kartu (latency, uptime %, dan balok-balok sparkline)
function updateCardMetrics(data) {
  const m = data.monitor;
  monitorsMap.set(m.id, m);

  const dot = document.getElementById(`dot-${m.id}`);
  if (dot) {
    dot.className = `status-dot ${m.status === 'up' ? 'up' : (m.status === 'down' ? 'down' : 'paused')}`;
  }

  const latEl = document.getElementById(`lat-${m.id}`);
  if (latEl && m.last_latency_ms) {
    latEl.textContent = `${m.last_latency_ms.toFixed(1)} ms`;
  }

  const uptEl = document.getElementById(`upt-${m.id}`);
  if (uptEl) {
    uptEl.textContent = `${data.uptime_24h.toFixed(1)}%`;
  }

  const barsContainer = document.getElementById(`bars-${m.id}`);
  if (barsContainer && data.recent_heartbeats) {
    barsContainer.innerHTML = '';
    const hbs = data.recent_heartbeats;
    const padCount = Math.max(0, 25 - hbs.length);
    for (let i = 0; i < padCount; i++) {
      const emptyBar = document.createElement('div');
      emptyBar.className = 'bar bar-empty';
      barsContainer.appendChild(emptyBar);
    }
    for (const hb of hbs) {
      const bar = document.createElement('div');
      bar.className = `bar ${hb.is_up ? '' : 'down'}`;
      const height = hb.is_up ? Math.min(28, Math.max(8, Math.round(hb.latency_ms / 15))) : 28;
      bar.style.height = `${height}px`;
      bar.title = `${hb.checked_at} - ${hb.is_up ? hb.latency_ms + 'ms' : (hb.error_message || 'Down')}`;
      barsContainer.appendChild(bar);
    }
  }
  recalcStats();
}

function recalcStats() {
  let up = 0;
  let down = 0;
  let total = monitorsMap.size;

  monitorsMap.forEach(m => {
    if (m.status === 'up') up++;
    else if (m.status === 'down') down++;
  });

  updateBanner(up, down, total);
}

function updateBanner(up, down, total) {
  document.getElementById('count-up').textContent = up;
  document.getElementById('count-down').textContent = down;
  document.getElementById('count-total').textContent = total;

  const dot = document.getElementById('system-dot');
  const text = document.getElementById('system-status');

  if (total === 0) {
    dot.className = 'status-dot paused';
    text.textContent = 'No monitors configured';
  } else if (down > 0) {
    dot.className = 'status-dot down';
    text.textContent = `${down} Service Incident${down > 1 ? 's' : ''} Detected`;
  } else {
    dot.className = 'status-dot up';
    text.textContent = 'All Systems Operational';
  }
}

// Inisialisasi SSE untuk live data sync
function initSSE() {
  const evtSource = new EventSource('/api/events');
  evtSource.onmessage = (e) => {
    try {
      const event = JSON.parse(e.data);
      const m = monitorsMap.get(event.monitor_id);
      if (m) {
        m.status = event.status;
        m.last_latency_ms = event.latency_ms;
        loadDetails(m.id);
      }
    } catch (err) {
      console.error('SSE parse error:', err);
    }
  };
  evtSource.onerror = () => {
    setTimeout(initSSE, 5000);
  };
}

// Aksi manual Check Now
async function checkNow(id) {
  try {
    await apiFetch(`/api/monitors/${id}/check`, { method: 'POST' });
    loadDetails(id);
  } catch (err) {
    alert('Check failed: ' + err);
  }
}

// Aksi Pause / Resume
async function togglePause(id) {
  try {
    await apiFetch(`/api/monitors/${id}/pause`, { method: 'POST' });
    loadDetails(id);
  } catch (err) {
    alert('Action failed: ' + err);
  }
}

// Aksi Delete Monitor
async function deleteMonitor(id) {
  if (!confirm('Apakah Anda yakin ingin menghapus monitor ini?')) return;
  try {
    await apiFetch(`/api/monitors/${id}`, { method: 'DELETE' });
    monitorsMap.delete(id);
    const card = document.getElementById(`card-${id}`);
    if (card) card.remove();
    recalcStats();
  } catch (err) {
    alert('Delete failed: ' + err);
  }
}

function openAddModal() {
  document.getElementById('add-modal').style.display = 'flex';
  document.getElementById('m-name').focus();
}

function closeAddModal() {
  document.getElementById('add-modal').style.display = 'none';
  document.getElementById('monitor-form').reset();
}

function handleTypeChange() {
  const type = document.getElementById('m-type').value;
  const targetInput = document.getElementById('m-target');
  const targetLabel = document.getElementById('target-label');
  if (type === 'tcp') {
    targetLabel.textContent = 'Target Host & Port';
    targetInput.placeholder = '192.168.1.1:80 or example.com:22';
  } else if (type === 'ping') {
    targetLabel.textContent = 'Target Host / IP';
    targetInput.placeholder = '1.1.1.1 or example.com';
  } else {
    targetLabel.textContent = 'Target URL';
    targetInput.placeholder = 'https://example.com';
  }
}

async function handleCreateMonitor(e) {
  e.preventDefault();
  const payload = {
    name: document.getElementById('m-name').value,
    monitor_type: document.getElementById('m-type').value,
    target: document.getElementById('m-target').value,
    interval_sec: parseInt(document.getElementById('m-interval').value) || 60,
    timeout_sec: 10,
  };

  try {
    const res = await apiFetch('/api/monitors', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });

    if (res.ok) {
      closeAddModal();
      await loadMonitors();
    } else {
      const err = await res.text();
      alert('Failed to add monitor: ' + err);
    }
  } catch (err) {
    alert('Error: ' + err);
  }
}

function escapeHtml(str) {
  if (!str) return '';
  return str.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

document.addEventListener('DOMContentLoaded', () => {
  checkAuth();
});
