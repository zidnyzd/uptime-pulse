// Client Script untuk Option 3: Widget / Tile Cards Grid (UptimePulse Admin)
let monitorsMap = new Map();
let currentFilter = 'all';
let searchQuery = '';

// Wrapper fetch untuk menyertakan auth session & token
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

// Cek autentikasi saat halaman dibuka
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
      errBox.textContent = data.error || 'Kombinasi username atau password salah';
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

// Memuat daftar seluruh monitor dari API
async function loadMonitors() {
  try {
    const res = await apiFetch('/api/monitors');
    const list = await res.json();
    monitorsMap.clear();
    for (const m of list) {
      monitorsMap.set(m.id, m);
      loadDetails(m.id);
    }
    renderMonitors();
    recalcStats();
  } catch (err) {
    console.error('Failed to load monitors:', err);
  }
}

// Memuat detail heartbeat dan sparkline tiap monitor
async function loadDetails(id) {
  try {
    const res = await apiFetch(`/api/monitors/${id}`);
    if (!res.ok) return;
    const data = await res.json();
    monitorsMap.set(id, data.monitor);
    updateCardMetrics(data);
    recalcStats();
  } catch (e) {
    console.error('Failed to load detail for', id, e);
  }
}

// Render grid kartu widget monitor berdasarkan filter & search
function renderMonitors() {
  const container = document.getElementById('monitor-grid');
  const filtered = Array.from(monitorsMap.values()).filter(m => {
    // Filter type
    if (currentFilter !== 'all' && m.monitor_type.toLowerCase() !== currentFilter) {
      return false;
    }
    // Search query
    if (searchQuery) {
      const q = searchQuery.toLowerCase();
      const matchName = m.name.toLowerCase().includes(q);
      const matchTarget = m.target.toLowerCase().includes(q);
      if (!matchName && !matchTarget) return false;
    }
    return true;
  });

  if (filtered.length === 0) {
    container.innerHTML = `
      <div class="empty-state">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
          <circle cx="12" cy="12" r="10"></circle>
          <line x1="12" y1="8" x2="12" y2="12"></line>
          <line x1="12" y1="16" x2="12.01" y2="16"></line>
        </svg>
        <p>${monitorsMap.size === 0 ? 'Belum ada monitor. Klik "Add Monitor" untuk menambahkan.' : 'Tidak ada monitor yang cocok dengan filter.'}</p>
      </div>
    `;
    return;
  }

  container.innerHTML = '';
  for (const m of filtered) {
    container.appendChild(createMonitorWidget(m));
  }
}

// Membuat DOM elemen kartu widget (Tile Card)
function createMonitorWidget(m) {
  const card = document.createElement('div');
  card.className = 'widget-card';
  card.id = `card-${m.id}`;

  const statusClass = m.status === 'up' ? 'up' : (m.status === 'down' ? 'down' : 'paused');
  const latencyText = m.last_latency_ms ? `${m.last_latency_ms.toFixed(1)} ms` : '--';
  const typeClass = m.monitor_type.toLowerCase();

  card.innerHTML = `
    <div class="widget-header">
      <div class="widget-title-area">
        <div class="widget-dot-wrap">
          <div class="status-dot ${statusClass}" id="dot-${m.id}"></div>
        </div>
        <div class="widget-name-wrap">
          <div class="widget-name" title="${escapeHtml(m.name)}">
            <span>${escapeHtml(m.name)}</span>
            <span class="type-pill ${typeClass}">${m.monitor_type}</span>
          </div>
          <div class="widget-endpoint" title="${escapeHtml(m.target)}">${escapeHtml(m.target)}</div>
        </div>
      </div>
      <div class="widget-actions">
        <button class="btn-icon" title="Check Now" onclick="checkNow(${m.id})">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M23 4v6h-6"></path>
            <path d="M1 20v-6h6"></path>
            <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"></path>
          </svg>
        </button>
        <button class="btn-icon" title="Pause / Resume" onclick="togglePause(${m.id})">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="10"></circle>
            <line x1="10" y1="15" x2="10" y2="9"></line>
            <line x1="14" y1="15" x2="14" y2="9"></line>
          </svg>
        </button>
        <button class="btn-icon danger" title="Delete" onclick="deleteMonitor(${m.id})">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="3 6 5 6 21 6"></polyline>
            <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
          </svg>
        </button>
      </div>
    </div>

    <div class="widget-metrics">
      <div class="metric-item">
        <div class="metric-label">Latency</div>
        <div class="metric-value" id="lat-${m.id}">${latencyText}</div>
      </div>
      <div class="metric-item">
        <div class="metric-label">24h Uptime</div>
        <div class="metric-value" id="upt-${m.id}">--%</div>
      </div>
    </div>

    <div class="widget-history">
      <div class="bars-container" id="bars-${m.id}"></div>
    </div>

    <div class="widget-footer">
      <span>Every ${m.interval_sec}s</span>
      <span id="last-check-${m.id}">${m.last_check_at ? m.last_check_at.split(' ')[1] : 'Pending'}</span>
    </div>
  `;
  return card;
}

// Memperbarui metrik kartu (latency, uptime %, dan balok sparkline)
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

  const lastCheckEl = document.getElementById(`last-check-${m.id}`);
  if (lastCheckEl && m.last_check_at) {
    lastCheckEl.textContent = m.last_check_at.split(' ')[1] || m.last_check_at;
  }

  const barsContainer = document.getElementById(`bars-${m.id}`);
  if (barsContainer && data.recent_heartbeats) {
    barsContainer.innerHTML = '';
    const hbs = data.recent_heartbeats;
    const padCount = Math.max(0, 24 - hbs.length);
    for (let i = 0; i < padCount; i++) {
      const emptyBar = document.createElement('div');
      emptyBar.className = 'bar bar-empty';
      barsContainer.appendChild(emptyBar);
    }
    for (const hb of hbs) {
      const bar = document.createElement('div');
      bar.className = `bar ${hb.is_up ? '' : 'down'}`;
      const height = hb.is_up ? Math.min(26, Math.max(6, Math.round(hb.latency_ms / 15))) : 26;
      bar.style.height = `${height}px`;
      bar.title = `${hb.checked_at} - ${hb.is_up ? hb.latency_ms + 'ms' : (hb.error_message || 'Down')}`;
      barsContainer.appendChild(bar);
    }
  }
}

// Menghitung ringkasan KPI sistem
function recalcStats() {
  let up = 0;
  let down = 0;
  let total = monitorsMap.size;
  let totalLat = 0;
  let latCount = 0;

  monitorsMap.forEach(m => {
    if (m.status === 'up') up++;
    else if (m.status === 'down') down++;

    if (m.last_latency_ms) {
      totalLat += m.last_latency_ms;
      latCount++;
    }
  });

  document.getElementById('kpi-total').textContent = total;
  document.getElementById('kpi-up').textContent = up;
  document.getElementById('kpi-down').textContent = down;

  const incidentDot = document.getElementById('kpi-incident-dot');
  const incidentSub = document.getElementById('kpi-incident-sub');

  if (down > 0) {
    incidentDot.className = 'status-dot down';
    incidentSub.textContent = `${down} target down saat ini`;
    incidentSub.style.color = 'var(--red)';
  } else {
    incidentDot.className = 'status-dot';
    incidentDot.style.background = 'var(--text-muted)';
    incidentDot.style.boxShadow = 'none';
    incidentDot.style.animation = 'none';
    incidentSub.textContent = 'No downtime detected';
    incidentSub.style.color = 'var(--text-muted)';
  }

  const avgLat = latCount > 0 ? (totalLat / latCount).toFixed(1) : '--';
  document.getElementById('kpi-latency').textContent = `${avgLat} ms`;
}

// Filter pill click
function setFilter(type, btn) {
  currentFilter = type;
  document.querySelectorAll('.filter-pill').forEach(b => b.classList.remove('active'));
  btn.classList.add('active');
  renderMonitors();
}

// Search input
function handleSearch() {
  searchQuery = document.getElementById('search-input').value.trim();
  renderMonitors();
}

// Inisialisasi SSE terkendali (mencegah duplikasi koneksi & spam reconnect saat server restart)
let sseConnection = null;
let sseReconnectTimer = null;

function initSSE() {
  if (sseConnection) {
    sseConnection.close();
    sseConnection = null;
  }
  if (sseReconnectTimer) {
    clearTimeout(sseReconnectTimer);
    sseReconnectTimer = null;
  }

  try {
    sseConnection = new EventSource('/api/events');

    sseConnection.onmessage = (e) => {
      try {
        const event = JSON.parse(e.data);
        const m = monitorsMap.get(event.monitor_id);
        if (m) {
          m.status = event.status;
          m.last_latency_ms = event.latency_ms;
          
          // Update elemen DOM langsung tanpa spamming HTTP fetch
          const dot = document.getElementById(`dot-${m.id}`);
          if (dot) {
            dot.className = `status-dot ${m.status === 'up' ? 'up' : (m.status === 'down' ? 'down' : 'paused')}`;
          }

          const latEl = document.getElementById(`lat-${m.id}`);
          if (latEl && m.last_latency_ms) {
            latEl.textContent = `${m.last_latency_ms.toFixed(1)} ms`;
          }

          const lastCheckEl = document.getElementById(`last-check-${m.id}`);
          if (lastCheckEl) {
            const now = new Date();
            lastCheckEl.textContent = now.toTimeString().split(' ')[0];
          }

          recalcStats();
        }
      } catch (err) {}
    };

    sseConnection.onerror = () => {
      // Tutup koneksi aktif segera agar browser tidak melakukan auto-reconnect tak terkontrol
      if (sseConnection) {
        sseConnection.close();
        sseConnection = null;
      }
      // Jadwalkan reconnect tunggal setelah 6 detik
      sseReconnectTimer = setTimeout(() => {
        initSSE();
      }, 6000);
    };
  } catch (err) {
    sseReconnectTimer = setTimeout(() => {
      initSSE();
    }, 6000);
  }
}

// Action Check Now
async function checkNow(id) {
  try {
    await apiFetch(`/api/monitors/${id}/check`, { method: 'POST' });
    loadDetails(id);
  } catch (err) {
    alert('Check failed: ' + err);
  }
}

// Action Pause / Resume
async function togglePause(id) {
  try {
    await apiFetch(`/api/monitors/${id}/pause`, { method: 'POST' });
    loadDetails(id);
  } catch (err) {
    alert('Action failed: ' + err);
  }
}

// Action Delete
async function deleteMonitor(id) {
  if (!confirm('Apakah Anda yakin ingin menghapus monitor ini?')) return;
  try {
    await apiFetch(`/api/monitors/${id}`, { method: 'DELETE' });
    monitorsMap.delete(id);
    renderMonitors();
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
