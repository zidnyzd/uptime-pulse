// Client script untuk Public Status Page UptimePulse (Opsi A - Unified Grouped List)

const publicI18n = {
  id: {
    brand_title: 'Status Sistem',
    live_active: 'Pembaruan langsung aktif',
    hero_ok_title: 'Semua Sistem Beroperasi Normal',
    hero_ok_sub: 'Seluruh layanan jaringan dan infrastruktur terpantau berjalan lancar.',
    hero_outage_title: 'Gangguan Layanan Terdeteksi',
    hero_outage_sub: 'Pemeriksaan otomatis mendeteksi adanya kendala. Investigasi sedang berjalan.',
    hero_empty_title: 'Belum Ada Layanan',
    hero_empty_sub: 'Tambahkan monitor di panel Admin untuk mulai memantau.',
    kpi_uptime: 'Uptime Sistem 24 Jam',
    kpi_services: 'Layanan Operasional',
    kpi_latency: 'Rata-rata Latensi',
    section_services: 'Layanan & Infrastruktur',
    section_history_30d: 'Riwayat 30 hari',
    section_incidents: 'Riwayat Insiden',
    section_last_30d: '30 hari terakhir',
    no_incidents: 'Tidak ada insiden atau gangguan yang dilaporkan dalam 30 hari terakhir.',
    time_30d_ago: '30 hari lalu',
    time_today: 'Hari ini',
    status_operational: 'Operasional',
    status_outage: 'Gangguan Layanan',
    status_paused: 'Dijeda',
  },
  en: {
    brand_title: 'System Status',
    live_active: 'Live updates active',
    hero_ok_title: 'All Systems Operational',
    hero_ok_sub: 'All monitored network targets and infrastructure are functioning normally.',
    hero_outage_title: 'Service Outage Detected',
    hero_outage_sub: 'Our automated probers have detected downtime. Diagnostics underway.',
    hero_empty_title: 'No Services Configured',
    hero_empty_sub: 'Configure monitors in the Admin console to begin tracking.',
    kpi_uptime: '24h System Uptime',
    kpi_services: 'Services Operational',
    kpi_latency: 'Average Latency',
    section_services: 'Services & Infrastructure',
    section_history_30d: '30 days history',
    section_incidents: 'Incident History',
    section_last_30d: 'Last 30 days',
    no_incidents: 'No incidents or disruptions reported in the past 30 days.',
    time_30d_ago: '30 days ago',
    time_today: 'Today',
    status_operational: 'Operational',
    status_outage: 'Major Outage',
    status_paused: 'Paused',
  }
};

let currentPublicLang = localStorage.getItem('uptime_lang') || 'en';
let latestPublicData = null;

function setLanguage(lang) {
  if (lang !== 'id' && lang !== 'en') lang = 'en';
  currentPublicLang = lang;
  localStorage.setItem('uptime_lang', lang);

  // Update toggle pill active states
  const btnId = document.getElementById('lang-btn-id');
  const btnEn = document.getElementById('lang-btn-en');
  if (btnId && btnEn) {
    btnId.classList.toggle('active', lang === 'id');
    btnEn.classList.toggle('active', lang === 'en');
  }

  // Update static elements with [data-i18n]
  document.querySelectorAll('[data-i18n]').forEach((el) => {
    const key = el.getAttribute('data-i18n');
    if (publicI18n[lang] && publicI18n[lang][key]) {
      el.textContent = publicI18n[lang][key];
    }
  });

  // Re-render views if data has arrived
  if (latestPublicData) {
    renderPublicView(latestPublicData);
  }
}

async function loadPublicData() {
  try {
    const res = await fetch('/api/public/summary');
    if (!res.ok) return;
    const data = await res.json();
    latestPublicData = data;
    renderPublicView(data);
  } catch (e) {
    console.error('Failed to load public status:', e);
  }
}

function renderPublicView(data) {
  const t = publicI18n[currentPublicLang] || publicI18n['en'];
  const heroWrap = document.getElementById('hero-wrap');
  const heroTitle = document.getElementById('hero-title');
  const heroSubtitle = document.getElementById('hero-subtitle');
  const heroSvg = document.getElementById('hero-badge-svg');

  if (data.incident_services > 0) {
    heroWrap.className = 'hero-status-wrap outage';
    heroTitle.textContent = currentPublicLang === 'id' 
      ? `${data.incident_services} Layanan Mengalami Gangguan` 
      : `${data.incident_services} Service Outage${data.incident_services > 1 ? 's' : ''} Detected`;
    heroSubtitle.textContent = t.hero_outage_sub;
    heroSvg.innerHTML = `
      <line x1="18" y1="6" x2="6" y2="18"></line>
      <line x1="6" y1="6" x2="18" y2="18"></line>
    `;
  } else if (data.total_services === 0) {
    heroWrap.className = 'hero-status-wrap';
    heroTitle.textContent = t.hero_empty_title;
    heroSubtitle.textContent = t.hero_empty_sub;
  } else {
    heroWrap.className = 'hero-status-wrap';
    heroTitle.textContent = t.hero_ok_title;
    heroSubtitle.textContent = t.hero_ok_sub;
    heroSvg.innerHTML = `
      <polyline points="20 6 9 17 4 12"></polyline>
    `;
  }

  // Handle Dynamic Site Branding dari Database
  if (data.branding) {
    const brand = data.branding;
    if (brand.site_title) {
      const brandTitleEl = document.getElementById('public-brand-title');
      if (brandTitleEl) brandTitleEl.textContent = brand.site_title;
      document.title = `${brand.site_title} - Status`;
    }

    if (brand.logo_url) {
      const logoWrap = document.getElementById('public-brand-logo-wrap');
      if (logoWrap) {
        logoWrap.innerHTML = `<img src="${escapeHtml(brand.logo_url)}" alt="Logo" style="width: 100%; height: 100%; object-fit: contain; border-radius: 4px;">`;
      }
    }

    if (brand.site_subtitle && data.incident_services === 0 && data.total_services > 0) {
      heroSubtitle.textContent = brand.site_subtitle;
    }

    if (brand.custom_footer) {
      const footerEl = document.getElementById('public-footer-text');
      if (footerEl) footerEl.textContent = brand.custom_footer;
    }
  }

  // Ringkasan Angka Metrik (2 Kolom: Uptime & Layanan Operasional)
  document.getElementById('operational-count').textContent = `${data.operational_services} / ${data.total_services}`;

  if (data.monitors.length > 0) {
    const totalUptime = data.monitors.reduce((acc, m) => acc + m.uptime_24h, 0);
    const avgUptime = (totalUptime / data.monitors.length).toFixed(2);
    document.getElementById('overall-uptime').textContent = `${avgUptime}%`;
  } else {
    document.getElementById('overall-uptime').textContent = '100.0%';
  }

  // Daftar Layanan (Unified Grouped Container)
  const container = document.getElementById('services-list');
  if (data.monitors.length === 0) {
    container.innerHTML = `<div class="loading-state"><span>No services to display.</span></div>`;
    return;
  }

  container.innerHTML = '';
  for (const m of data.monitors) {
    const row = document.createElement('div');
    row.className = 'service-row';

    const isUp = m.status === 'up' || m.status === 'retrying';
    const isPaused = m.status === 'paused';
    const isDown = m.status === 'down';

    const dotClass = isDown ? 'down' : (isPaused ? 'paused' : '');
    const pillClass = isDown ? 'down' : (isPaused ? 'paused' : '');
    const pillText = isDown 
      ? t.status_outage 
      : (isPaused ? t.status_paused : t.status_operational);

    let barsHtml = '';
    const bars = m.daily_bars || [];
    for (const b of bars) {
      let cls = 'empty';
      let title = `${b.label}: No data recorded`;
      if (b.status === 'up') {
        cls = '';
        title = `${b.label}: 100% Uptime (${b.total_checks} checks)`;
      } else if (b.status === 'degraded') {
        cls = 'degraded';
        title = `${b.label}: ${b.uptime_pct}% Uptime (${b.total_checks - b.up_checks} incidents)`;
      } else if (b.status === 'down') {
        cls = 'down';
        title = `${b.label}: ${b.uptime_pct}% Uptime (${b.total_checks - b.up_checks} incidents)`;
      }
      barsHtml += `<div class="strip-bar ${cls}" title="${title}"></div>`;
    }

    const uptimeText = currentPublicLang === 'id'
      ? `${m.uptime_24h.toFixed(1)}% uptime (24 jam)`
      : `${m.uptime_24h.toFixed(1)}% uptime (24h)`;

    row.innerHTML = `
      <div class="service-header">
        <div class="service-name-wrap">
          <div class="service-status-dot ${dotClass}"></div>
          <span class="service-name" title="${escapeHtml(m.name)}">${escapeHtml(m.name)}</span>
        </div>
        <div class="service-status-pill ${pillClass}">${pillText}</div>
      </div>
      <div class="history-strip">
        ${barsHtml}
      </div>
      <div class="strip-footer">
        <span>${t.time_30d_ago}</span>
        <span class="strip-footer-uptime">${uptimeText}</span>
        <span>${t.time_today}</span>
      </div>
    `;
    container.appendChild(row);
  }

  // Active Incidents Live Alert
  const activeContainer = document.getElementById('active-incidents-container');
  if (data.active_incidents && data.active_incidents.length > 0) {
    activeContainer.innerHTML = '';
    for (const inc of data.active_incidents) {
      const card = document.createElement('div');
      card.className = 'active-incident-card';
      card.innerHTML = `
        <div class="incident-top">
          <div class="incident-service-badge">
            <span class="incident-badge-pill">ONGOING OUTAGE</span>
            <span>${escapeHtml(inc.service_name)}</span>
          </div>
          <div class="incident-start-time">Started: ${escapeHtml(inc.started_at)}</div>
        </div>
        <div class="incident-error-detail">
          ${escapeHtml(inc.error_message || 'Connection failed / Target unresponsive')}
        </div>
      `;
      activeContainer.appendChild(card);
    }
    activeContainer.style.display = 'block';
  } else {
    activeContainer.style.display = 'none';
  }

  // Past Incidents List (Clean Feed)
  const incidentsList = document.getElementById('incidents-list');
  if (data.recent_incidents && data.recent_incidents.length > 0) {
    incidentsList.innerHTML = '';
    for (const inc of data.recent_incidents) {
      const item = document.createElement('div');
      item.className = 'incident-feed-item';

      const durationStr = inc.duration_sec 
        ? `Resolved in ${formatDuration(inc.duration_sec)}` 
        : 'Ongoing';

      item.innerHTML = `
        <div class="incident-feed-top">
          <span class="incident-feed-name">${escapeHtml(inc.service_name)}</span>
          <span class="incident-feed-time">${escapeHtml(inc.started_at)} • <strong style="color: var(--green);">${durationStr}</strong></span>
        </div>
        <div class="incident-feed-err">
          ${escapeHtml(inc.error_message || 'Connection Error')}
        </div>
      `;
      incidentsList.appendChild(item);
    }
  } else {
    incidentsList.innerHTML = `
      <div class="empty-incident-feed">
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="var(--green)" stroke-width="2.5" stroke-linecap="round">
          <polyline points="20 6 9 17 4 12"></polyline>
        </svg>
        <span>${t.no_incidents}</span>
      </div>
    `;
  }

  // Update timestamp di footer / bottom bar
  const now = new Date();
  const timeStr = now.toTimeString().split(' ')[0];
  const updatedEl = document.getElementById('public-last-updated');
  if (updatedEl) {
    const label = currentPublicLang === 'id' ? 'Data terakhir diambil' : 'Last updated';
    updatedEl.textContent = `${label}: ${timeStr}`;
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

function escapeHtml(str) {
  if (!str) return '';
  return str.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

// Inisialisasi SSE terkendali
let publicSse = null;
let publicReconnectTimer = null;

function initPublicSSE() {
  if (publicSse) {
    publicSse.close();
    publicSse = null;
  }
  if (publicReconnectTimer) {
    clearTimeout(publicReconnectTimer);
    publicReconnectTimer = null;
  }

  try {
    publicSse = new EventSource('/api/events');
    publicSse.onmessage = () => {
      // Reload ringkasan saat ada event baru
      loadPublicData();
    };
    publicSse.onerror = () => {
      if (publicSse) {
        publicSse.close();
        publicSse = null;
      }
      if (!publicReconnectTimer) {
        publicReconnectTimer = setTimeout(initPublicSSE, 6000);
      }
    };
  } catch (err) {}
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
        btn.title = currentPublicLang === 'id' ? 'Ganti ke Mode Gelap' : 'Switch to Dark Mode';
      } else {
        sunIcon.style.display = 'block';
        moonIcon.style.display = 'none';
        btn.title = currentPublicLang === 'id' ? 'Ganti ke Mode Terang' : 'Switch to Light Mode';
      }
    }
  }
}

document.addEventListener('DOMContentLoaded', () => {
  initTheme();
  setLanguage(currentPublicLang);
  loadPublicData();
  initPublicSSE();
});
