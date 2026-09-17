// Client Script untuk Option 3: Widget / Tile Cards Grid (UptimePulse Admin)
let monitorsMap = new Map();
let detailsMap = new Map();
let currentFilter = 'all';
let searchQuery = '';
let currentActiveView = 'monitors';

// --- Toast & Confirm UI (pengganti alert()/confirm() bawaan browser) ---
function showToast(message, type = 'error') {
  const container = document.getElementById('toast-container');
  if (!container) return;
  const el = document.createElement('div');
  el.className = `toast toast-${type === 'success' ? 'success' : 'error'}`;
  el.innerHTML = `<span class="toast-icon">${type === 'success' ? '✓' : '⚠'}</span><span></span>`;
  el.querySelector('span:last-child').textContent = message;
  const dismiss = () => {
    el.classList.add('toast-out');
    setTimeout(() => el.remove(), 220);
  };
  el.addEventListener('click', dismiss);
  container.appendChild(el);
  setTimeout(dismiss, 5000);
  // Batasi tumpukan agar tidak menutupi layar
  while (container.children.length > 4) container.firstChild.remove();
}

let confirmResolve = null;
function showConfirm(message, opts = {}) {
  const modal = document.getElementById('confirm-modal');
  document.getElementById('confirm-title').textContent = opts.title || (currentLang === 'id' ? 'Konfirmasi' : 'Confirm');
  document.getElementById('confirm-message').textContent = message;
  const okBtn = document.getElementById('confirm-ok-btn');
  okBtn.textContent = opts.okLabel || (currentLang === 'id' ? 'Ya, Lanjutkan' : 'Yes, Continue');
  okBtn.classList.toggle('danger', opts.danger !== false);
  document.getElementById('confirm-cancel-btn').textContent = currentLang === 'id' ? 'Batal' : 'Cancel';
  modal.style.display = 'flex';
  return new Promise((resolve) => { confirmResolve = resolve; });
}
function closeConfirmModal(result) {
  document.getElementById('confirm-modal').style.display = 'none';
  if (confirmResolve) { confirmResolve(!!result); confirmResolve = null; }
}

// --- Internationalization (i18n: ID & EN) ---
const i18n = {
  id: {
    nav_monitoring: 'Pemantauan',
    nav_monitors: 'Daftar Monitor',
    nav_incidents: 'Log Insiden',
    nav_settings: 'Pengaturan & Tautan',
    nav_telegram: 'Notifikasi Telegram',
    nav_branding: 'Identitas & Branding',
    nav_backup: 'Backup & Restore',
    nav_public: 'Status Publik',
    nav_change_pwd: 'Ubah Kata Sandi',
    nav_logout: 'Keluar (Sign Out)',

    modal_branding_title: 'Identitas & Branding Publik',
    branding_site_title_label: 'Nama Situs / Judul Status',
    branding_site_sub_label: 'Subjudul Status Normal',
    branding_logo_label: 'URL Logo Kustom',
    branding_footer_label: 'Teks Atribusi Footer',

    title_overview_desktop: 'Ringkasan Infrastruktur',
    title_overview_mobile: 'Ringkasan',
    title_incidents_desktop: 'Riwayat Insiden & Downtime',
    title_incidents_mobile: 'Insiden',
    title_telegram_desktop: 'Konfigurasi Notifikasi Telegram',
    title_telegram_mobile: 'Telegram',
    title_branding_desktop: 'Kustomisasi Identitas & Branding',
    title_branding_mobile: 'Branding',
    title_backup_desktop: 'Backup & Pemeliharaan Storage',
    title_backup_mobile: 'Backup & Storage',

    breadcrumb_monitors: 'UptimePulse / Prober Real-time / Monitor Aktif',
    breadcrumb_incidents: 'UptimePulse / Siklus Insiden / Catatan Gangguan',
    breadcrumb_telegram: 'UptimePulse / Pengaturan / Notifikasi Telegram',
    breadcrumb_branding: 'UptimePulse / Pengaturan / Branding Publik',
    breadcrumb_backup: 'UptimePulse / Pengaturan / Backup & Storage',
    top_live_sync: 'Live Sync',
    btn_add: 'Tambah',
    btn_monitor: 'Monitor',

    kpi_total: 'TOTAL LAYANAN',
    kpi_total_sub: 'Target terpantau',
    kpi_up: 'OPERASIONAL',
    kpi_up_sub: 'Merespons normal',
    kpi_down: 'INSIDEN AKTIF',
    kpi_down_sub_none: 'Tidak ada gangguan',
    kpi_down_sub_alert: 'Layanan terganggu',
    kpi_latency: 'RATA-RATA LATENSI',
    kpi_latency_sub: 'Waktu respons jaringan',

    search_placeholder: 'Cari target berdasarkan nama atau host...',
    filter_all: 'Semua',

    card_latency: 'Latensi',
    card_uptime: 'Uptime 24 Jam',
    card_every: 'Setiap',
    card_retry: 'Retry',
    card_pending: 'Menunggu',
    card_retrying: 'Mencoba Ulang',
    btn_check_now: 'Cek Sekarang',
    btn_pause: 'Jeda / Lanjutkan',
    btn_edit: 'Edit Monitor',
    btn_delete: 'Hapus Monitor',
    btn_reset: 'Reset Statistik',
    drag_handle_tip: 'Tarik untuk mengubah urutan (Drag & Drop)',
    btn_move_up: 'Pindah ke Atas',
    btn_move_down: 'Pindah ke Bawah',
    reorder_filter_warn: 'Pembersihan filter / pencarian diperlukan sebelum mengatur urutan.',
    reorder_success: 'Urutan monitor berhasil disimpan.',
    reorder_failed: 'Gagal menyimpan urutan monitor: ',

    footer_engine: 'UptimePulse Engine • Rust Axum & SQLite WAL',
    footer_arch: 'Single Static Binary • Konkurensi via Tokio'
  },
  en: {
    nav_monitoring: 'Monitoring',
    nav_monitors: 'Monitors Grid',
    nav_incidents: 'Incidents Log',
    nav_settings: 'Settings & Links',
    nav_telegram: 'Telegram Alert',
    nav_branding: 'Site Branding',
    nav_backup: 'Backup & Restore',
    nav_public: 'Public Status',
    nav_change_pwd: 'Change Password',
    nav_logout: 'Sign Out',

    modal_branding_title: 'Site Branding & Identity',
    branding_site_title_label: 'Site Title',
    branding_site_sub_label: 'Normal Status Subtitle',
    branding_logo_label: 'Custom Logo Image URL',
    branding_footer_label: 'Custom Footer Text',

    title_overview_desktop: 'Infrastructure Overview',
    title_overview_mobile: 'Overview',
    title_incidents_desktop: 'Incident History & Downtime Log',
    title_incidents_mobile: 'Incidents',
    title_telegram_desktop: 'Telegram Alert Configuration',
    title_telegram_mobile: 'Telegram',
    title_branding_desktop: 'Site Branding & Customization',
    title_branding_mobile: 'Branding',
    title_backup_desktop: 'Backup & Storage Maintenance',
    title_backup_mobile: 'Backup & Storage',

    breadcrumb_monitors: 'UptimePulse / Realtime Prober / Active Monitors',
    breadcrumb_incidents: 'UptimePulse / Incident Lifecycle / Outage History',
    breadcrumb_telegram: 'UptimePulse / Settings / Telegram Alert',
    breadcrumb_branding: 'UptimePulse / Settings / Site Branding',
    breadcrumb_backup: 'UptimePulse / Settings / Backup & Storage',
    top_live_sync: 'Live Sync',
    btn_add: 'Add',
    btn_monitor: 'Monitor',

    kpi_total: 'TOTAL SERVICES',
    kpi_total_sub: 'Monitored endpoints',
    kpi_up: 'OPERATIONAL',
    kpi_up_sub: 'Responding normally',
    kpi_down: 'ACTIVE INCIDENTS',
    kpi_down_sub_none: 'No downtime detected',
    kpi_down_sub_alert: 'Services impacted',
    kpi_latency: 'AVERAGE LATENCY',
    kpi_latency_sub: 'Overall probe response time',

    search_placeholder: 'Search targets by name or host...',
    filter_all: 'All',

    card_latency: 'Latency',
    card_uptime: '24h Uptime',
    card_every: 'Every',
    card_retry: 'Retry',
    card_pending: 'Pending',
    card_retrying: 'Retrying',
    btn_check_now: 'Check Now',
    btn_pause: 'Pause / Resume',
    btn_edit: 'Edit Monitor',
    btn_delete: 'Delete Monitor',
    btn_reset: 'Reset Stats',
    drag_handle_tip: 'Drag to reorder (Drag & Drop)',
    btn_move_up: 'Move Up',
    btn_move_down: 'Move Down',
    reorder_filter_warn: 'Please clear filter / search before reordering.',
    reorder_success: 'Monitor order saved successfully.',
    reorder_failed: 'Failed to save monitor order: ',

    footer_engine: 'UptimePulse Engine • Rust Axum & SQLite WAL',
    footer_arch: 'Single Static Binary • Concurrency via Tokio'
  }
};

let currentLang = localStorage.getItem('uptime_lang') || 'en';

function setLanguage(lang) {
  if (lang !== 'id' && lang !== 'en') lang = 'en';
  currentLang = lang;
  localStorage.setItem('uptime_lang', lang);

  // Update switcher button active state
  const btnId = document.getElementById('lang-btn-id');
  const btnEn = document.getElementById('lang-btn-en');
  if (btnId && btnEn) {
    btnId.classList.toggle('active', lang === 'id');
    btnEn.classList.toggle('active', lang === 'en');
  }

  // Update static text elements with [data-i18n]
  document.querySelectorAll('[data-i18n]').forEach((el) => {
    const key = el.getAttribute('data-i18n');
    if (i18n[lang] && i18n[lang][key]) {
      el.textContent = i18n[lang][key];
    }
  });

  // Update placeholder attributes with [data-i18n-placeholder]
  document.querySelectorAll('[data-i18n-placeholder]').forEach((el) => {
    const key = el.getAttribute('data-i18n-placeholder');
    if (i18n[lang] && i18n[lang][key]) {
      el.setAttribute('placeholder', i18n[lang][key]);
    }
  });

  // Update dynamic titles
  updateViewTitles();

  // Re-render monitors to apply translated labels & tooltips
  renderMonitors();
  recalcStats();
  updateAdminLastFetched();
}

function updateViewTitles() {
  const pageTitle = document.getElementById('page-title');
  const breadcrumb = document.getElementById('page-breadcrumb');
  const addBtn = document.getElementById('btn-add-monitor-top');

  let titleDesk = i18n[currentLang].title_overview_desktop;
  let titleMob = i18n[currentLang].title_overview_mobile;
  let bc = i18n[currentLang].breadcrumb_monitors;

  if (currentActiveView === 'incidents') {
    titleDesk = i18n[currentLang].title_incidents_desktop;
    titleMob = i18n[currentLang].title_incidents_mobile;
    bc = i18n[currentLang].breadcrumb_incidents;
  } else if (currentActiveView === 'telegram') {
    titleDesk = i18n[currentLang].title_telegram_desktop;
    titleMob = i18n[currentLang].title_telegram_mobile;
    bc = i18n[currentLang].breadcrumb_telegram;
  } else if (currentActiveView === 'branding') {
    titleDesk = i18n[currentLang].title_branding_desktop;
    titleMob = i18n[currentLang].title_branding_mobile;
    bc = i18n[currentLang].breadcrumb_branding;
  } else if (currentActiveView === 'backup') {
    titleDesk = i18n[currentLang].title_backup_desktop;
    titleMob = i18n[currentLang].title_backup_mobile;
    bc = i18n[currentLang].breadcrumb_backup;
  }

  if (pageTitle) {
    pageTitle.innerHTML = `<span class="desktop-title">${titleDesk}</span><span class="mobile-title">${titleMob}</span>`;
  }
  if (breadcrumb) {
    breadcrumb.textContent = bc;
  }
  if (addBtn) {
    addBtn.style.display = currentActiveView === 'monitors' ? 'inline-flex' : 'none';
  }
}

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
      applyBrandingFavicon();
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
  document.getElementById('main-app').style.display = 'flex';
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
      applyBrandingFavicon();
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
    updateAdminLastFetched();
  } catch (err) {
    console.error('Failed to load monitors:', err);
  }
}

function updateAdminLastFetched() {
  const el = document.getElementById('admin-last-fetched-text');
  if (el) {
    const now = new Date();
    const timeStr = now.toTimeString().split(' ')[0];
    const tz = configuredTimezone || 'Asia/Jakarta';
    const abbr = getTimezoneAbbr(tz);
    const label = currentLang === 'id' ? 'Data terakhir diambil' : 'Last fetched';
    el.textContent = `${label}: ${timeStr} ${abbr}`;
  }
}

// Memuat detail heartbeat dan sparkline tiap monitor
async function loadDetails(id) {
  try {
    const res = await apiFetch(`/api/monitors/${id}`);
    if (!res.ok) return;
    const data = await res.json();
    detailsMap.set(id, data);
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
    if (detailsMap.has(m.id)) {
      updateCardMetrics(detailsMap.get(m.id));
    }
  }
}

// Membuat DOM elemen kartu widget (Tile Card)
function createMonitorWidget(m) {
  const card = document.createElement('div');
  const isPaused = !m.is_active || m.status === 'paused';
  card.className = isPaused ? 'widget-card paused' : 'widget-card';
  card.id = `card-${m.id}`;

  const statusClass = m.status === 'up' ? 'up' : (m.status === 'down' ? 'down' : (m.status === 'retrying' ? 'retrying' : 'paused'));
  const latencyText = m.last_latency_ms ? `${m.last_latency_ms.toFixed(1)} ms` : '--';
  const typeClass = m.monitor_type.toLowerCase();

  const retryInfo = m.status === 'retrying' 
    ? `<span style="color: var(--yellow); font-size: 11px; font-weight: 600;">⚠️ ${i18n[currentLang].card_retrying} (${m.consecutive_fails || 1}/${m.max_retries || 3})</span>`
    : `<span>${i18n[currentLang].card_every} ${m.interval_sec}s • ${i18n[currentLang].card_retry} ${m.max_retries || 3}x</span>`;

  const pausedBadge = isPaused
    ? `<span class="paused-badge" id="paused-badge-${m.id}">⏸ ${currentLang === 'id' ? 'Dijeda' : 'Paused'}</span>`
    : `<span class="paused-badge" id="paused-badge-${m.id}" style="display: none;">⏸ ${currentLang === 'id' ? 'Dijeda' : 'Paused'}</span>`;

  const isPub = m.is_public !== false;
  const privateBadge = !isPub
    ? `<span class="private-badge" id="private-badge-${m.id}" title="${currentLang === 'id' ? 'Khusus Admin (Tidak tampil di Status Publik)' : 'Admin Only (Hidden from Public Status)'}">🔒 ${currentLang === 'id' ? 'Privat' : 'Private'}</span>`
    : `<span class="private-badge" id="private-badge-${m.id}" style="display: none;">🔒</span>`;

  card.innerHTML = `
    <div class="widget-header">
      <div class="widget-title-area">
        <div class="widget-reorder-group">
          <div class="drag-handle" title="${i18n[currentLang].drag_handle_tip}">
            <svg width="12" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2">
              <circle cx="8" cy="5" r="1.5"></circle>
              <circle cx="16" cy="5" r="1.5"></circle>
              <circle cx="8" cy="12" r="1.5"></circle>
              <circle cx="16" cy="12" r="1.5"></circle>
              <circle cx="8" cy="19" r="1.5"></circle>
              <circle cx="16" cy="19" r="1.5"></circle>
            </svg>
          </div>
          <div class="reorder-arrows">
            <button type="button" class="btn-arrow" title="${i18n[currentLang].btn_move_up}" onclick="event.stopPropagation(); moveMonitorOrder(${m.id}, -1)">
              <svg width="8" height="8" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round"><polyline points="18 15 12 9 6 15"></polyline></svg>
            </button>
            <button type="button" class="btn-arrow" title="${i18n[currentLang].btn_move_down}" onclick="event.stopPropagation(); moveMonitorOrder(${m.id}, 1)">
              <svg width="8" height="8" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round"><polyline points="6 9 12 15 18 9"></polyline></svg>
            </button>
          </div>
        </div>
        <div class="widget-dot-wrap">
          <div class="status-dot ${statusClass}" id="dot-${m.id}" title="Status: ${m.status}"></div>
        </div>
        <div class="widget-name-wrap">
          <div class="widget-name-header">
            <span class="widget-name-title" title="${escapeHtml(m.name)}">${escapeHtml(m.name)}</span>
            <div class="widget-badges">
              <span class="type-pill ${typeClass}">${m.monitor_type}</span>
              ${privateBadge}
              ${pausedBadge}
            </div>
          </div>
          <div class="widget-endpoint" title="${escapeHtml(m.target)}">${escapeHtml(m.target)}</div>
        </div>
      </div>
    </div>

    <div class="widget-metrics">
      <div class="metric-item">
        <div class="metric-label">${i18n[currentLang].card_latency}</div>
        <div class="metric-value" id="lat-${m.id}">${latencyText}</div>
      </div>
      <div class="metric-item">
        <div class="metric-label">${i18n[currentLang].card_uptime}</div>
        <div class="metric-value" id="upt-${m.id}">--%</div>
      </div>
    </div>

    <div class="widget-history">
      <div class="bars-container" id="bars-${m.id}"></div>
    </div>

    <div class="widget-footer">
      <div class="widget-footer-meta">
        <span id="retry-info-${m.id}">${retryInfo}</span>
        <span class="footer-time-sep">•</span>
        <span class="footer-time" id="last-check-${m.id}">${m.last_check_at ? m.last_check_at.split(' ')[1] : 'Pending'}</span>
      </div>
      <div class="widget-actions">
        <button class="btn-icon" title="${i18n[currentLang].btn_check_now}" onclick="checkNow(${m.id})">
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M23 4v6h-6"></path>
            <path d="M1 20v-6h6"></path>
            <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"></path>
          </svg>
        </button>
        <button class="btn-icon" title="${i18n[currentLang].btn_pause}" onclick="togglePause(${m.id})">
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="10"></circle>
            <line x1="10" y1="15" x2="10" y2="9"></line>
            <line x1="14" y1="15" x2="14" y2="9"></line>
          </svg>
        </button>
        <button class="btn-icon" title="${i18n[currentLang].btn_edit}" onclick="openEditModal(${m.id})">
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path>
            <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>
          </svg>
        </button>
        <button class="btn-icon" title="${i18n[currentLang].btn_reset}" onclick="resetStats(${m.id})">
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="1 4 1 10 7 10"></polyline>
            <path d="M3.51 15a9 9 0 1 0 2.13-9.36L1 10"></path>
          </svg>
        </button>
        <button class="btn-icon danger" title="${i18n[currentLang].btn_delete}" onclick="deleteMonitor(${m.id})">
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="3 6 5 6 21 6"></polyline>
            <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
          </svg>
        </button>
      </div>
    </div>
  `;

  // HTML5 Drag and Drop attributes & listeners
  card.setAttribute('draggable', 'true');
  card.dataset.id = String(m.id);
  card.addEventListener('dragstart', handleCardDragStart);
  card.addEventListener('dragover', handleCardDragOver);
  card.addEventListener('dragleave', handleCardDragLeave);
  card.addEventListener('drop', handleCardDrop);
  card.addEventListener('dragend', handleCardDragEnd);

  return card;
}

// --- Reordering Logic (HTML5 Drag & Drop + Mobile Arrows) ---
let draggedMonitorId = null;

function handleCardDragStart(e) {
  // Cegah drag jika pengguna mengklik tombol atau kontrol interaktif di dalam kartu
  if (e.target.closest('button, input, select, textarea, a, .btn-icon, .btn-arrow')) {
    e.preventDefault();
    return;
  }
  // Hanya izinkan drag jika ditarik dari drag-handle (.drag-handle) agar seleksi teks tetap normal
  if (!e.target.closest('.drag-handle')) {
    e.preventDefault();
    return;
  }
  if (currentFilter !== 'all' || (searchQuery && searchQuery.trim() !== '')) {
    e.preventDefault();
    showToast(i18n[currentLang].reorder_filter_warn);
    return;
  }
  const id = parseInt(this.dataset.id, 10);
  draggedMonitorId = id;
  this.classList.add('is-dragging');
  e.dataTransfer.effectAllowed = 'move';
  e.dataTransfer.setData('text/plain', String(id));
}

function handleCardDragOver(e) {
  e.preventDefault();
  if (!draggedMonitorId) return;
  e.dataTransfer.dropEffect = 'move';
  const targetCard = this.closest('.widget-card');
  if (targetCard && parseInt(targetCard.dataset.id, 10) !== draggedMonitorId) {
    targetCard.classList.add('drag-over');
  }
}

function handleCardDragLeave(e) {
  const targetCard = this.closest('.widget-card');
  if (targetCard) {
    targetCard.classList.remove('drag-over');
  }
}

async function handleCardDrop(e) {
  e.preventDefault();
  const targetCard = this.closest('.widget-card');
  if (targetCard) {
    targetCard.classList.remove('drag-over');
  }
  if (!draggedMonitorId) return;

  const targetId = parseInt(this.dataset.id, 10);
  const sourceId = draggedMonitorId;
  draggedMonitorId = null;

  if (sourceId === targetId) return;

  const ids = Array.from(monitorsMap.keys());
  const fromIdx = ids.indexOf(sourceId);
  const toIdx = ids.indexOf(targetId);

  if (fromIdx === -1 || toIdx === -1) return;

  // Pindahkan ID ke indeks target
  ids.splice(fromIdx, 1);
  ids.splice(toIdx, 0, sourceId);

  // Rekonstruksi monitorsMap secara optimistik
  const newMap = new Map();
  for (const id of ids) {
    newMap.set(id, monitorsMap.get(id));
  }
  monitorsMap = newMap;
  renderMonitors();

  await saveNewMonitorOrder(ids);
}

function handleCardDragEnd(e) {
  this.classList.remove('is-dragging');
  document.querySelectorAll('.widget-card.drag-over').forEach(el => el.classList.remove('drag-over'));
  draggedMonitorId = null;
}

// Action Panah Naik / Turun
async function moveMonitorOrder(id, delta) {
  if (currentFilter !== 'all' || (searchQuery && searchQuery.trim() !== '')) {
    showToast(i18n[currentLang].reorder_filter_warn);
    return;
  }

  const ids = Array.from(monitorsMap.keys());
  const idx = ids.indexOf(id);
  if (idx === -1) return;

  const newIdx = idx + delta;
  if (newIdx < 0 || newIdx >= ids.length) return; // Batas atas/bawah tercapai

  // Tukar posisi ID
  const temp = ids[idx];
  ids[idx] = ids[newIdx];
  ids[newIdx] = temp;

  const newMap = new Map();
  for (const mid of ids) {
    newMap.set(mid, monitorsMap.get(mid));
  }
  monitorsMap = newMap;
  renderMonitors();

  await saveNewMonitorOrder(ids);
}

// Simpan susunan urutan baru ke API backend
async function saveNewMonitorOrder(ids) {
  try {
    const res = await apiFetch('/api/monitors/reorder', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ ids }),
    });
    if (!res.ok) {
      const data = await res.json().catch(() => ({}));
      showToast((i18n[currentLang].reorder_failed || 'Gagal: ') + (data.error || data.message || res.status));
      await loadMonitors();
      return;
    }
    showToast(i18n[currentLang].reorder_success, 'success');
  } catch (err) {
    showToast((i18n[currentLang].reorder_failed || 'Gagal: ') + err);
    await loadMonitors();
  }
}

// Memperbarui metrik kartu (latency, uptime %, dan balok sparkline)
function updateCardMetrics(data) {
  const m = data.monitor;
  monitorsMap.set(m.id, m);

  const isPaused = !m.is_active || m.status === 'paused';
  const card = document.getElementById(`card-${m.id}`);
  if (card) {
    card.className = isPaused ? 'widget-card paused' : 'widget-card';
  }

  const pausedBadgeEl = document.getElementById(`paused-badge-${m.id}`);
  if (pausedBadgeEl) {
    pausedBadgeEl.style.display = isPaused ? 'inline-flex' : 'none';
  }

  const dot = document.getElementById(`dot-${m.id}`);
  if (dot) {
    dot.className = `status-dot ${m.status === 'up' ? 'up' : (m.status === 'down' ? 'down' : (m.status === 'retrying' ? 'retrying' : 'paused'))}`;
  }

  const retryEl = document.getElementById(`retry-info-${m.id}`);
  if (retryEl) {
    retryEl.innerHTML = m.status === 'retrying'
      ? `<span style="color: var(--yellow); font-size: 11px; font-weight: 600;">⚠️ ${i18n[currentLang].card_retrying} (${m.consecutive_fails || 1}/${m.max_retries || 3})</span>`
      : `<span>${i18n[currentLang].card_every} ${m.interval_sec}s • ${i18n[currentLang].card_retry} ${m.max_retries || 3}x</span>`;
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
  if (barsContainer && data.hourly_bars) {
    barsContainer.innerHTML = '';
    const bars = data.hourly_bars;
    for (const b of bars) {
      const bar = document.createElement('div');
      let cls = 'bar-empty';
      let title = `${b.label}: No data`;
      let height = 4;
      if (b.status === 'up') {
        cls = '';
        title = `${b.label}: 100% Uptime, ${b.avg_latency_ms} ms (${b.total_checks} checks)`;
        height = Math.min(26, Math.max(8, Math.round(b.avg_latency_ms / 10)));
      } else if (b.status === 'degraded') {
        cls = 'degraded';
        title = `${b.label}: ${b.uptime_pct}% Uptime (${b.total_checks - b.up_checks} incidents)`;
        height = 26;
      } else if (b.status === 'down') {
        cls = 'down';
        title = `${b.label}: ${b.uptime_pct}% Uptime (${b.total_checks - b.up_checks} incidents)`;
        height = 26;
      }
      bar.className = `bar ${cls}`.trim();
      bar.style.height = `${height}px`;
      bar.title = title;
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

  // Update sidebar counter badges
  const navMon = document.getElementById('nav-count-monitors');
  if (navMon) navMon.textContent = total;

  const navInc = document.getElementById('nav-count-incidents');
  if (navInc) {
    navInc.textContent = down;
    navInc.style.display = down > 0 ? 'inline-block' : 'none';
  }

  const incidentDot = document.getElementById('kpi-incident-dot');
  const incidentSub = document.getElementById('kpi-incident-sub');

  if (down > 0) {
    incidentDot.className = 'status-dot down';
    incidentSub.textContent = currentLang === 'id' ? `${down} target down saat ini` : `${down} service(s) currently down`;
    incidentSub.style.color = 'var(--red)';
  } else {
    incidentDot.className = 'status-dot';
    incidentDot.style.background = 'var(--text-muted)';
    incidentDot.style.boxShadow = 'none';
    incidentDot.style.animation = 'none';
    incidentSub.textContent = i18n[currentLang] ? i18n[currentLang].kpi_down_sub_none : 'No downtime detected';
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
          m.consecutive_fails = event.consecutive_fails;
          m.max_retries = event.max_retries;
          m.last_latency_ms = event.latency_ms;
          
          // Update elemen DOM langsung tanpa spamming HTTP fetch
          const dot = document.getElementById(`dot-${m.id}`);
          if (dot) {
            dot.className = `status-dot ${m.status === 'up' ? 'up' : (m.status === 'down' ? 'down' : (m.status === 'retrying' ? 'retrying' : 'paused'))}`;
          }

          const retryEl = document.getElementById(`retry-info-${m.id}`);
          if (retryEl) {
            retryEl.innerHTML = m.status === 'retrying'
              ? `<span style="color: var(--yellow); font-size: 11px; font-weight: 600;">⚠️ Retrying (${m.consecutive_fails || 1}/${m.max_retries || 3})</span>`
              : `<span>Every ${m.interval_sec}s • Retry ${m.max_retries || 3}x</span>`;
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
          updateAdminLastFetched();
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
    showToast('Check failed: ' + err);
  }
}

// Action Pause / Resume
async function togglePause(id) {
  try {
    await apiFetch(`/api/monitors/${id}/pause`, { method: 'POST' });
    loadDetails(id);
  } catch (err) {
    showToast('Action failed: ' + err);
  }
}

// Action Delete
async function deleteMonitor(id) {
  const msg = currentLang === 'id'
    ? 'Hapus monitor ini beserta seluruh riwayatnya?'
    : 'Delete this monitor and all its history?';
  if (!(await showConfirm(msg, { danger: true }))) return;
  try {
    await apiFetch(`/api/monitors/${id}`, { method: 'DELETE' });
    monitorsMap.delete(id);
    detailsMap.delete(id);
    renderMonitors();
    recalcStats();
    showToast(currentLang === 'id' ? 'Monitor berhasil dihapus.' : 'Monitor deleted.', 'success');
  } catch (err) {
    showToast('Delete failed: ' + err);
  }
}

// Action Reset Stats: hapus seluruh heartbeat + insiden monitor, ukur ulang dari awal
async function resetStats(id) {
  const msg = currentLang === 'id'
    ? 'Reset seluruh statistik monitor ini dari awal?\nSemua riwayat heartbeat dan insiden akan dihapus permanen.'
    : 'Reset all stats for this monitor from scratch?\nAll heartbeat history and incidents will be permanently deleted.';
  if (!(await showConfirm(msg, { danger: true }))) return;
  try {
    const res = await apiFetch(`/api/monitors/${id}/reset`, { method: 'POST' });
    const data = await res.json().catch(() => ({}));
    if (!res.ok || data.success === false) {
      showToast((currentLang === 'id' ? 'Reset gagal: ' : 'Reset failed: ') + (data.error || data.message || res.status));
      return;
    }
    detailsMap.delete(id);
    await loadDetails(id);
    renderMonitors();
    recalcStats();
    showToast(data.message || (currentLang === 'id' ? 'Statistik berhasil direset.' : 'Stats reset.'), 'success');
  } catch (err) {
    showToast('Reset failed: ' + err);
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
    max_retries: parseInt(document.getElementById('m-retries').value) || 3,
    is_public: document.getElementById('m-public').checked,
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
      showToast(currentLang === 'id' ? 'Monitor berhasil ditambahkan.' : 'Monitor added.', 'success');
    } else {
      const err = await res.text();
      showToast('Failed to add monitor: ' + err);
    }
  } catch (err) {
    showToast('Error: ' + err);
  }
}

// --- Edit Monitor Modal ---
function openEditModal(id) {
  const m = monitorsMap.get(id);
  if (!m) return;

  document.getElementById('edit-id').value = m.id;
  document.getElementById('edit-name').value = m.name;
  document.getElementById('edit-type').value = m.monitor_type;
  document.getElementById('edit-interval').value = m.interval_sec;
  document.getElementById('edit-target').value = m.target;
  document.getElementById('edit-retries').value = m.max_retries || 3;
  document.getElementById('edit-public').checked = m.is_public !== false;

  handleEditTypeChange();
  document.getElementById('edit-modal').style.display = 'flex';
}

function closeEditModal() {
  document.getElementById('edit-modal').style.display = 'none';
}

function handleEditTypeChange() {
  const type = document.getElementById('edit-type').value;
  const targetLabel = document.getElementById('edit-target-label');
  const targetInput = document.getElementById('edit-target');

  if (type === 'tcp') {
    targetLabel.textContent = 'Target Host:Port';
    targetInput.placeholder = '1.1.1.1:53 or example.com:80';
  } else if (type === 'ping') {
    targetLabel.textContent = 'Target Host / IP';
    targetInput.placeholder = '1.1.1.1 or example.com';
  } else {
    targetLabel.textContent = 'Target URL';
    targetInput.placeholder = 'https://example.com';
  }
}

async function handleUpdateMonitor(e) {
  e.preventDefault();
  const id = document.getElementById('edit-id').value;
  const payload = {
    name: document.getElementById('edit-name').value,
    monitor_type: document.getElementById('edit-type').value,
    target: document.getElementById('edit-target').value,
    interval_sec: parseInt(document.getElementById('edit-interval').value) || 60,
    timeout_sec: 10,
    max_retries: parseInt(document.getElementById('edit-retries').value) || 3,
    is_public: document.getElementById('edit-public').checked,
  };

  const btn = document.getElementById('btn-edit-submit');
  btn.disabled = true;
  btn.textContent = 'Saving...';

  try {
    const res = await apiFetch(`/api/monitors/${id}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });

    if (res.ok) {
      closeEditModal();
      await loadMonitors();
      showToast(currentLang === 'id' ? 'Monitor berhasil diperbarui.' : 'Monitor updated.', 'success');
    } else {
      const err = await res.text();
      showToast('Gagal memperbarui monitor: ' + err);
    }
  } catch (err) {
    showToast('Error: ' + err);
  } finally {
    btn.disabled = false;
    btn.textContent = 'Update Monitor';
  }
}

// --- Change Password Modal ---
function openPasswordModal() {
  document.getElementById('password-modal').style.display = 'flex';
  const form = document.getElementById('password-form');
  if (form) form.reset();
  const statusMsg = document.getElementById('pwd-status-msg');
  if (statusMsg) statusMsg.style.display = 'none';
}

function closePasswordModal() {
  document.getElementById('password-modal').style.display = 'none';
}

async function handleChangePassword(e) {
  e.preventDefault();
  const oldPassword = document.getElementById('pwd-old').value;
  const newPassword = document.getElementById('pwd-new').value;
  const confirmPassword = document.getElementById('pwd-confirm').value;
  const statusMsg = document.getElementById('pwd-status-msg');
  const submitBtn = document.getElementById('btn-pwd-save');

  if (newPassword !== confirmPassword) {
    statusMsg.textContent = 'Konfirmasi kata sandi baru tidak cocok.';
    statusMsg.style.color = 'var(--red)';
    statusMsg.style.display = 'block';
    return;
  }

  if (newPassword.length < 8) {
    statusMsg.textContent = 'Kata sandi baru minimal 8 karakter.';
    statusMsg.style.color = 'var(--red)';
    statusMsg.style.display = 'block';
    return;
  }

  statusMsg.style.display = 'none';
  submitBtn.disabled = true;
  submitBtn.textContent = 'Menyimpan...';

  try {
    const res = await apiFetch('/api/auth/change-password', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        old_password: oldPassword,
        new_password: newPassword
      })
    });

    const data = await res.json();
    if (res.ok && data.success) {
      statusMsg.textContent = data.message || 'Kata sandi berhasil diubah!';
      statusMsg.style.color = 'var(--green)';
      statusMsg.style.display = 'block';
      setTimeout(() => {
        closePasswordModal();
      }, 1500);
    } else {
      statusMsg.textContent = data.error || 'Gagal mengubah kata sandi.';
      statusMsg.style.color = 'var(--red)';
      statusMsg.style.display = 'block';
    }
  } catch (err) {
    statusMsg.textContent = 'Terjadi kesalahan: ' + err;
    statusMsg.style.color = 'var(--red)';
    statusMsg.style.display = 'block';
  } finally {
    submitBtn.disabled = false;
    submitBtn.textContent = 'Simpan Sandi Baru';
  }
}

// --- User Profile Dropup Menu (Ke Atas) ---
function toggleUserDropup(e) {
  if (e) e.stopPropagation();
  const menu = document.getElementById('user-dropup-menu');
  const card = document.getElementById('user-profile-card');
  if (!menu) return;

  const isShown = menu.classList.contains('show');
  if (isShown) {
    closeUserDropup();
  } else {
    menu.classList.add('show');
    if (card) card.classList.add('active');
  }
}

function closeUserDropup() {
  const menu = document.getElementById('user-dropup-menu');
  const card = document.getElementById('user-profile-card');
  if (menu) menu.classList.remove('show');
  if (card) card.classList.remove('active');
}

// Auto-close dropup saat klik di luar area profil
document.addEventListener('click', (e) => {
  const sidebarBottom = document.querySelector('.sidebar-bottom');
  if (sidebarBottom && !sidebarBottom.contains(e.target)) {
    closeUserDropup();
  }
});

let configuredTimezone = 'Asia/Jakarta';
let configuredTimeFormat = '24h';
let configuredDateFormat = 'DD-MM-YYYY';

function getTimezoneAbbr(tz) {
  const map = {
    'Asia/Jakarta': 'WIB',
    'Asia/Makassar': 'WITA',
    'Asia/Jayapura': 'WIT',
    'Asia/Singapore': 'SGT',
    'UTC': 'UTC',
    'Europe/London': 'GMT',
    'Europe/Berlin': 'CET',
    'America/New_York': 'EST',
    'America/Los_Angeles': 'PST',
    'Asia/Tokyo': 'JST',
    'Australia/Sydney': 'AEST',
  };
  if (map[tz]) return map[tz];

  try {
    const parts = new Intl.DateTimeFormat('id-ID', { timeZone: tz, timeZoneName: 'short' })
      .formatToParts(new Date());
    const found = parts.find(p => p.type === 'timeZoneName')?.value;
    if (found) return found;
  } catch (e) {}

  return tz;
}

function formatCustomDate(dateInput, tz = 'Asia/Jakarta', timeFormat = '24h', dateFormat = 'DD-MM-YYYY') {
  if (!dateInput) return '';

  const str = String(dateInput).trim();
  const m = str.match(/^(\d{4})-(\d{2})-(\d{2})(?:[ T](\d{2}):(\d{2})(?::(\d{2}))?)?/);
  if (!m) {
    const abbr = getTimezoneAbbr(tz);
    return `${dateInput} ${abbr}`;
  }

  let year = m[1];
  let month = m[2];
  let day = m[3];
  let hour = m[4] || '00';
  let min = m[5] || '00';
  let sec = m[6] || '00';

  // Format tanggal sesuai pilihan user (seperti TT-BB-TTTT)
  let datePart = `${day}-${month}-${year}`;
  if (dateFormat === 'YYYY-MM-DD') {
    datePart = `${year}-${month}-${day}`;
  } else if (dateFormat === 'DD/MM/YYYY') {
    datePart = `${day}/${month}/${year}`;
  } else if (dateFormat === 'MM/DD/YYYY') {
    datePart = `${month}/${day}/${year}`;
  } else if (dateFormat === 'DD MMM YYYY') {
    const months = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
    const mIdx = parseInt(month, 10) - 1;
    const mName = months[mIdx] || month;
    datePart = `${day} ${mName} ${year}`;
  }

  // Format jam sesuai 24h atau 12h
  let timePart = `${hour}:${min}:${sec}`;
  if (timeFormat === '12h') {
    let hNum = parseInt(hour, 10);
    const period = hNum >= 12 ? 'PM' : 'AM';
    hNum = hNum % 12;
    if (hNum === 0) hNum = 12;
    const hStr = hNum < 10 ? `0${hNum}` : `${hNum}`;
    timePart = `${hStr}:${min}:${sec} ${period}`;
  }

  const abbr = getTimezoneAbbr(tz);
  return `${datePart} ${timePart} ${abbr}`;
}

function formatTimeWithTz(dateStr) {
  if (!dateStr) return '';
  return formatCustomDate(dateStr, configuredTimezone, configuredTimeFormat, configuredDateFormat);
}

// Escape untuk konteks HTML teks MAUPUN nilai atribut (title="...", value="...").
// Kutip ikut di-escape karena fungsi ini dipakai di dalam atribut — tanpa ini,
// nama monitor bertanda kutip bisa keluar dari atribut dan merusak markup.
function escapeHtml(str) {
  if (str === null || str === undefined) return '';
  return String(str)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

// --- Telegram Notification Settings ---
async function openTelegramModal() {
  document.getElementById('telegram-modal').style.display = 'flex';
  const statusMsg = document.getElementById('tg-status-msg');
  statusMsg.style.display = 'none';
  await loadTelegramSettings();
}

function closeTelegramModal() {
  document.getElementById('telegram-modal').style.display = 'none';
}

async function loadTelegramSettings() {
  try {
    const res = await apiFetch('/api/settings/telegram');
    if (res.ok) {
      const cfg = await res.json();
      document.getElementById('tg-enabled').checked = cfg.enabled;
      document.getElementById('tg-bot-token').value = cfg.bot_token || '';
      document.getElementById('tg-chat-id').value = cfg.chat_id || '';
      document.getElementById('tg-thread-id').value = cfg.thread_id || '';
    }
  } catch (err) {
    console.error('Failed to load telegram settings:', err);
  }
}

async function handleSaveTelegramSettings(e) {
  e.preventDefault();
  const threadVal = document.getElementById('tg-thread-id').value.trim();
  const payload = {
    enabled: document.getElementById('tg-enabled').checked,
    bot_token: document.getElementById('tg-bot-token').value.trim(),
    chat_id: document.getElementById('tg-chat-id').value.trim(),
    thread_id: threadVal ? parseInt(threadVal) : null,
  };

  const statusMsg = document.getElementById('tg-status-msg');
  const saveBtn = document.getElementById('btn-tg-save');
  saveBtn.disabled = true;
  saveBtn.textContent = 'Saving...';

  try {
    const res = await apiFetch('/api/settings/telegram', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });

    if (res.ok) {
      statusMsg.textContent = currentLang === 'id' ? 'Pengaturan Telegram berhasil disimpan!' : 'Telegram settings saved successfully!';
      statusMsg.style.color = 'var(--green)';
      statusMsg.style.display = 'block';
    } else {
      const err = await res.text();
      statusMsg.textContent = 'Gagal menyimpan: ' + err;
      statusMsg.style.color = 'var(--red)';
      statusMsg.style.display = 'block';
    }
  } catch (err) {
    statusMsg.textContent = 'Error: ' + err;
    statusMsg.style.color = 'var(--red)';
    statusMsg.style.display = 'block';
  } finally {
    saveBtn.disabled = false;
    saveBtn.textContent = 'Save Settings';
  }
}

async function handleTestTelegram() {
  const threadVal = document.getElementById('tg-thread-id').value.trim();
  const payload = {
    enabled: true, // test mode
    bot_token: document.getElementById('tg-bot-token').value.trim(),
    chat_id: document.getElementById('tg-chat-id').value.trim(),
    thread_id: threadVal ? parseInt(threadVal) : null,
  };

  if (!payload.bot_token || !payload.chat_id) {
    showToast(currentLang === 'id' ? 'Harap isi Bot Token dan Chat ID terlebih dahulu.' : 'Please fill in Bot Token and Chat ID first.');
    return;
  }

  const testBtn = document.getElementById('btn-tg-test');
  const statusMsg = document.getElementById('tg-status-msg');
  testBtn.disabled = true;
  testBtn.textContent = 'Sending...';
  statusMsg.style.display = 'none';

  try {
    const res = await apiFetch('/api/settings/telegram/test', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });

    const data = await res.json();
    if (res.ok && data.success) {
      statusMsg.textContent = data.message || 'Pesan tes berhasil dikirim!';
      statusMsg.style.color = 'var(--green)';
      statusMsg.style.display = 'block';
    } else {
      statusMsg.textContent = 'Gagal mengirim pesan: ' + (data.error || 'Cek kembali Token/ID');
      statusMsg.style.color = 'var(--red)';
      statusMsg.style.display = 'block';
    }
  } catch (err) {
    statusMsg.textContent = 'Error koneksi: ' + err;
    statusMsg.style.color = 'var(--red)';
    statusMsg.style.display = 'block';
  } finally {
    testBtn.disabled = false;
    testBtn.textContent = 'Test Message';
  }
}

// --- Site Branding Settings ---
async function openBrandingModal() {
  document.getElementById('branding-modal').style.display = 'flex';
  const statusMsg = document.getElementById('branding-status-msg');
  if (statusMsg) statusMsg.style.display = 'none';

  // Pasang live preview listener
  const titleInput = document.getElementById('branding-title-input');
  const logoInput = document.getElementById('branding-logo-input');

  if (titleInput) titleInput.oninput = updateBrandingPreview;
  if (logoInput) logoInput.oninput = updateBrandingPreview;

  await loadBrandingSettings();
}

function closeBrandingModal() {
  document.getElementById('branding-modal').style.display = 'none';
}

function updateBrandingPreview() {
  const title = document.getElementById('branding-title-input').value.trim() || 'System Status';
  const logoUrl = document.getElementById('branding-logo-input').value.trim();

  const previewTitle = document.getElementById('branding-preview-title');
  const previewLogo = document.getElementById('branding-preview-logo');

  if (previewTitle) previewTitle.textContent = title;
  if (previewLogo) {
    if (logoUrl) {
      previewLogo.innerHTML = `<img src="${escapeHtml(logoUrl)}" style="width: 100%; height: 100%; object-fit: cover; border-radius: 4px;" onerror="this.parentElement.innerHTML='<svg width=\\'14\\' height=\\'14\\' viewBox=\\'0 0 24 24\\' fill=\\'none\\' stroke=\\'var(--green)\\' stroke-width=\\'2.5\\'><polyline points=\\'22 12 18 12 15 21 9 3 6 12 2 12\\'></polyline></svg>'">`;
    } else {
      previewLogo.innerHTML = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--green)" stroke-width="2.5"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"></polyline></svg>`;
    }
  }
}

async function loadBrandingSettings() {
  try {
    const res = await apiFetch('/api/settings/branding');
    if (res.ok) {
      const cfg = await res.json();
      document.getElementById('branding-title-input').value = cfg.site_title || '';
      document.getElementById('branding-sub-input').value = cfg.site_subtitle || '';
      document.getElementById('branding-logo-input').value = cfg.logo_url || '';
      document.getElementById('branding-footer-input').value = cfg.custom_footer || '';
      if (document.getElementById('branding-tz-select')) {
        document.getElementById('branding-tz-select').value = cfg.timezone || 'Asia/Jakarta';
      }
      if (document.getElementById('branding-date-format-select')) {
        document.getElementById('branding-date-format-select').value = cfg.date_format || 'DD-MM-YYYY';
      }
      if (document.getElementById('branding-format-select')) {
        document.getElementById('branding-format-select').value = cfg.time_format || '24h';
      }
      configuredTimezone = cfg.timezone || 'Asia/Jakarta';
      configuredTimeFormat = cfg.time_format || '24h';
      configuredDateFormat = cfg.date_format || 'DD-MM-YYYY';
      updateBrandingPreview();
    }
  } catch (err) {
    console.error('Failed to load branding settings:', err);
  }
}

async function handleSaveBranding(e) {
  e.preventDefault();
  const payload = {
    site_title: document.getElementById('branding-title-input').value.trim() || 'System Status',
    site_subtitle: document.getElementById('branding-sub-input').value.trim(),
    logo_url: document.getElementById('branding-logo-input').value.trim(),
    custom_footer: document.getElementById('branding-footer-input').value.trim(),
    timezone: document.getElementById('branding-tz-select') ? document.getElementById('branding-tz-select').value : 'Asia/Jakarta',
    time_format: document.getElementById('branding-format-select') ? document.getElementById('branding-format-select').value : '24h',
    date_format: document.getElementById('branding-date-format-select') ? document.getElementById('branding-date-format-select').value : 'DD-MM-YYYY',
  };

  const statusMsg = document.getElementById('branding-status-msg');
  const submitBtn = document.getElementById('btn-branding-save');
  submitBtn.disabled = true;
  submitBtn.textContent = 'Saving...';

  try {
    const res = await apiFetch('/api/settings/branding', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });
    const data = await res.json();
    if (res.ok) {
      statusMsg.textContent = currentLang === 'id' ? 'Branding & pengaturan waktu berhasil disimpan!' : 'Branding & time settings saved successfully!';
      statusMsg.style.color = 'var(--green)';
      statusMsg.style.display = 'block';
      configuredTimezone = payload.timezone;
      configuredTimeFormat = payload.time_format;
      configuredDateFormat = payload.date_format;
      updateAdminLastFetched();
      if (payload.logo_url && payload.logo_url.trim()) {
        updateFavicon(payload.logo_url.trim());
      } else {
        updateFavicon(DEFAULT_FAVICON);
      }
    } else {
      throw new Error(data.error || 'Failed to save branding');
    }
  } catch (err) {
    statusMsg.textContent = 'Error: ' + err.message;
    statusMsg.style.color = 'var(--red)';
    statusMsg.style.display = 'block';
  } finally {
    submitBtn.disabled = false;
    submitBtn.textContent = 'Save Branding';
  }
}

// --- Navigation & View Switching ---
function toggleSidebar() {
  const sidebar = document.getElementById('sidebar');
  const backdrop = document.getElementById('sidebar-backdrop');
  if (sidebar.classList.contains('open')) {
    sidebar.classList.remove('open');
    backdrop.classList.remove('active');
  } else {
    sidebar.classList.add('open');
    backdrop.classList.add('active');
  }
}

async function switchView(viewName) {
  currentActiveView = viewName;

  const views = {
    monitors: document.getElementById('view-monitors'),
    incidents: document.getElementById('view-incidents'),
    telegram: document.getElementById('view-telegram'),
    branding: document.getElementById('view-branding'),
    backup: document.getElementById('view-backup'),
  };

  const navs = {
    monitors: document.getElementById('nav-monitors'),
    incidents: document.getElementById('nav-incidents'),
    telegram: document.getElementById('nav-telegram'),
    branding: document.getElementById('nav-branding'),
    backup: document.getElementById('nav-backup'),
  };

  for (const [key, el] of Object.entries(views)) {
    if (el) el.style.display = key === viewName ? 'block' : 'none';
  }

  for (const [key, el] of Object.entries(navs)) {
    if (el) {
      if (key === viewName) el.classList.add('active');
      else el.classList.remove('active');
    }
  }

  updateViewTitles();

  // Muat data yang sesuai dengan halaman pengaturan/view
  if (viewName === 'monitors') {
    renderMonitors();
    recalcStats();
  } else if (viewName === 'incidents') {
    await loadIncidents();
  } else if (viewName === 'telegram') {
    await loadTelegramSettings();
  } else if (viewName === 'branding') {
    const titleInput = document.getElementById('branding-title-input');
    const logoInput = document.getElementById('branding-logo-input');
    if (titleInput) titleInput.oninput = updateBrandingPreview;
    if (logoInput) logoInput.oninput = updateBrandingPreview;
    await loadBrandingSettings();
  } else if (viewName === 'backup') {
    await loadDbStats();
  }

  // Tutup drawer sidebar mobile jika sedang terbuka
  const sidebar = document.getElementById('sidebar');
  const backdrop = document.getElementById('sidebar-backdrop');
  if (sidebar && sidebar.classList.contains('open')) {
    sidebar.classList.remove('open');
    if (backdrop) backdrop.classList.remove('active');
  }
}

async function loadIncidents() {
  const feed = document.getElementById('incidents-feed');
  try {
    // Endpoint admin: menampilkan insiden SEMUA monitor (termasuk privat/paused).
    // /api/public/summary kini terfilter hanya monitor publik.
    const res = await apiFetch('/api/incidents');
    if (!res.ok) return;
    const incs = await res.json();

    if (incs.length === 0) {
      feed.innerHTML = `
        <div class="empty-state">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
            <circle cx="12" cy="12" r="10"></circle>
            <polyline points="12 6 12 12 14 14"></polyline>
          </svg>
          <p>Belum ada rekaman insiden. Semua sistem berjalan stabil.</p>
        </div>
      `;
      return;
    }

    feed.innerHTML = '';
    for (const inc of incs) {
      const row = document.createElement('div');
      row.className = 'incident-row';
      const isOngoing = inc.is_ongoing;
      const statusPill = isOngoing
        ? `<span class="type-pill ping" style="background: rgba(248,81,73,0.15); color: var(--red);">Ongoing Outage</span>`
        : `<span class="type-pill" style="background: var(--green-bg); color: var(--green);">Resolved in ${formatDuration(inc.duration_sec || 0)}</span>`;

      row.innerHTML = `
        <div class="incident-row-meta">
          <div class="incident-row-title">
            <span>${escapeHtml(inc.service_name)}</span>
            ${statusPill}
          </div>
          <div class="incident-row-time">${escapeHtml(inc.started_at)}</div>
        </div>
        <div class="incident-row-err">
          ${escapeHtml(inc.error_message || 'Connection Error')}
        </div>
      `;
      feed.appendChild(row);
    }
  } catch (err) {
    feed.innerHTML = `<div class="empty-state"><p>Gagal memuat insiden: ${err}</p></div>`;
  }
}

function formatDuration(sec) {
  if (sec < 60) return `${sec}s`;
  const m = Math.floor(sec / 60);
  const s = sec % 60;
  if (m < 60) return `${m}m ${s}s`;
  const h = Math.floor(m / 60);
  return `${h}h ${m % 60}m`;
}

// --- Backup & Restore Functions ---
function openBackupModal() {
  document.getElementById('backup-modal').style.display = 'flex';
  const statusMsg = document.getElementById('restore-status-msg');
  if (statusMsg) statusMsg.style.display = 'none';
  const pruneMsg = document.getElementById('prune-status-msg');
  if (pruneMsg) pruneMsg.style.display = 'none';
  loadDbStats();
}

function closeBackupModal() {
  document.getElementById('backup-modal').style.display = 'none';
  const form = document.getElementById('restore-form');
  if (form) form.reset();
}

async function handleExportJson() {
  try {
    const res = await apiFetch('/api/backup/export');
    if (!res.ok) throw new Error('Gagal mengunduh backup');
    const blob = await res.blob();
    const url = window.URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    const now = new Date().toISOString().slice(0, 10);
    a.download = `uptimepulse-backup-${now}.json`;
    document.body.appendChild(a);
    a.click();
    a.remove();
    window.URL.revokeObjectURL(url);
  } catch (err) {
    showToast('Export backup gagal: ' + err);
  }
}

async function handleDownloadDb() {
  try {
    const res = await apiFetch('/api/backup/database');
    if (!res.ok) throw new Error('Gagal mengunduh file database');
    const blob = await res.blob();
    const url = window.URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    const now = new Date().toISOString().slice(0, 10);
    a.download = `uptimepulse-${now}.db`;
    document.body.appendChild(a);
    a.click();
    a.remove();
    window.URL.revokeObjectURL(url);
  } catch (err) {
    showToast('Download database gagal: ' + err);
  }
}

async function handleRestoreBackup(e) {
  e.preventDefault();
  const fileInput = document.getElementById('restore-file-input');
  const modeSelect = document.getElementById('restore-mode');
  const statusMsg = document.getElementById('restore-status-msg');
  const submitBtn = document.getElementById('btn-restore-submit');

  if (!fileInput.files || fileInput.files.length === 0) {
    showToast(currentLang === 'id' ? 'Pilih file backup .json terlebih dahulu.' : 'Please select a .json backup file first.');
    return;
  }

  const file = fileInput.files[0];
  const mode = modeSelect.value;

  if (mode === 'replace') {
    const warnMsg = currentLang === 'id'
      ? 'PERINGATAN: Mode Ganti Total (Replace) akan menghapus seluruh data monitor lama dan menggantinya dengan isi file backup ini. Lanjutkan?'
      : 'WARNING: Replace mode will delete all existing monitor data and replace it with this backup file. Continue?';
    if (!(await showConfirm(warnMsg, { danger: true }))) {
      return;
    }
  }

  statusMsg.style.display = 'none';
  submitBtn.disabled = true;
  submitBtn.textContent = 'Restoring...';

  const reader = new FileReader();
  reader.onload = async (event) => {
    try {
      const jsonContent = JSON.parse(event.target.result);

      const payload = {
        mode: mode,
        backup: jsonContent
      };

      const res = await apiFetch('/api/backup/restore', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload)
      });

      const data = await res.json();
      if (res.ok && data.success) {
        statusMsg.textContent = data.message || 'Data berhasil dipulihkan!';
        statusMsg.style.color = 'var(--green)';
        statusMsg.style.display = 'block';
        await loadMonitors();
        setTimeout(closeBackupModal, 1500);
      } else {
        statusMsg.textContent = 'Gagal memulihkan: ' + (data.error || 'Format tidak cocok');
        statusMsg.style.color = 'var(--red)';
        statusMsg.style.display = 'block';
      }
    } catch (parseErr) {
      statusMsg.textContent = 'File bukan JSON yang valid: ' + parseErr.message;
      statusMsg.style.color = 'var(--red)';
      statusMsg.style.display = 'block';
    } finally {
      submitBtn.disabled = false;
      submitBtn.textContent = 'Restore Backup';
    }
  };

  reader.onerror = () => {
    statusMsg.textContent = 'Gagal membaca file dari disk.';
    statusMsg.style.color = 'var(--red)';
    statusMsg.style.display = 'block';
    submitBtn.disabled = false;
    submitBtn.textContent = 'Restore Backup';
  };

  reader.readAsText(file);
}

// --- Database Storage & Maintenance (Pruning & Stats) ---
async function loadDbStats() {
  try {
    const res = await apiFetch('/api/backup/stats');
    if (!res.ok) return;
    const data = await res.json();

    const pathEl = document.getElementById('db-stat-path');
    if (pathEl) pathEl.textContent = data.db_path;

    const sizeEl = document.getElementById('db-stat-size');
    if (sizeEl) {
      const dbKb = (data.db_size_bytes / 1024).toFixed(1);
      const walKb = (data.wal_size_bytes / 1024).toFixed(1);
      sizeEl.textContent = `${dbKb} KB (WAL: ${walKb} KB)`;
    }

    const hbEl = document.getElementById('db-stat-heartbeats');
    if (hbEl) hbEl.textContent = Number(data.total_heartbeats).toLocaleString();

    const retEl = document.getElementById('db-stat-retention');
    if (retEl) {
      retEl.textContent = `${data.retention_days} ${currentLang === 'id' ? 'Hari (Otomatis)' : 'Days (Automatic)'}`;
    }
  } catch (err) {
    console.error('Failed to load DB stats:', err);
  }
}

async function handleManualPrune() {
  const confirmMsg = currentLang === 'id'
    ? 'Hapus seluruh log riwayat pemeriksaan yang melebihi batas retensi dan rampingkan file WAL database?'
    : 'Purge all heartbeat logs older than retention days and truncate WAL database file?';
  if (!(await showConfirm(confirmMsg, { danger: true }))) return;

  const btn = document.getElementById('btn-manual-prune');
  const msg = document.getElementById('prune-status-msg');
  if (btn) btn.disabled = true;

  try {
    const res = await apiFetch('/api/backup/prune', { method: 'POST' });
    const data = await res.json();
    if (res.ok) {
      if (msg) {
        msg.style.display = 'block';
        msg.style.color = 'var(--green)';
        msg.textContent = data.message;
      }
      loadDbStats();
    } else {
      throw new Error(data.error || 'Gagal melakukan pruning');
    }
  } catch (err) {
    if (msg) {
      msg.style.display = 'block';
      msg.style.color = 'var(--red)';
      msg.textContent = 'Error: ' + err.message;
    }
  } finally {
    if (btn) btn.disabled = false;
  }
}

// --- Favicon Dynamic Updater ---
const DEFAULT_FAVICON = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='%232ea043' stroke-width='2.5' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpolyline points='22 12 18 12 15 21 9 3 6 12 2 12'/%3E%3C/svg%3E";

function updateFavicon(url) {
  const href = (url && url.trim()) ? url.trim() : DEFAULT_FAVICON;
  
  // Hapus semua elemen favicon lama untuk memicu refresh favicon di tab browser
  const existingIcons = document.querySelectorAll("link[rel*='icon']");
  existingIcons.forEach(el => el.remove());

  const newLink = document.createElement('link');
  newLink.id = 'app-favicon';
  newLink.rel = 'icon';
  if (href.startsWith('data:image/svg')) {
    newLink.type = 'image/svg+xml';
  } else if (href.includes('.png')) {
    newLink.type = 'image/png';
  } else if (href.includes('.jpg') || href.includes('.jpeg')) {
    newLink.type = 'image/jpeg';
  } else if (href.includes('.ico')) {
    newLink.type = 'image/x-icon';
  }
  newLink.href = href;
  document.head.appendChild(newLink);
}

async function applyBrandingFavicon() {
  try {
    const res = await apiFetch('/api/settings/branding');
    if (res.ok) {
      const cfg = await res.json();
      if (cfg.logo_url && cfg.logo_url.trim()) {
        updateFavicon(cfg.logo_url.trim());
      } else {
        updateFavicon(DEFAULT_FAVICON);
      }
    }
  } catch (e) {}
}

// --- Theme Handling (Dark / Light) ---
let currentTheme = localStorage.getItem('uptime_theme') || 'dark';

function initTheme() {
  setTheme(currentTheme);
}

function toggleTheme() {
  const nextTheme = currentTheme === 'dark' ? 'light' : 'dark';
  setTheme(nextTheme);
}

function setTheme(theme) {
  currentTheme = theme;
  localStorage.setItem('uptime_theme', theme);
  document.documentElement.setAttribute('data-theme', theme);

  const btn = document.getElementById('theme-toggle');
  if (btn) {
    const sunIcon = btn.querySelector('.sun-icon');
    const moonIcon = btn.querySelector('.moon-icon');
    if (sunIcon && moonIcon) {
      if (theme === 'light') {
        sunIcon.style.display = 'none';
        moonIcon.style.display = 'block';
        btn.title = currentLang === 'id' ? 'Ganti ke Mode Gelap' : 'Switch to Dark Mode';
      } else {
        sunIcon.style.display = 'block';
        moonIcon.style.display = 'none';
        btn.title = currentLang === 'id' ? 'Ganti ke Mode Terang' : 'Switch to Light Mode';
      }
    }
  }
}

document.addEventListener('DOMContentLoaded', () => {
  initTheme();
  setLanguage(currentLang);
  checkAuth();
  applyBrandingFavicon();
});
