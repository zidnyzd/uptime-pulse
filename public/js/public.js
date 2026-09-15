// Client script untuk Public Status Page UptimePulse
async function loadPublicData() {
  try {
    const res = await fetch('/api/public/summary');
    if (!res.ok) return;
    const data = await res.json();
    renderPublicView(data);
  } catch (e) {
    console.error('Failed to load public status:', e);
  }
}

function renderPublicView(data) {
  const heroBox = document.getElementById('hero-box');
  const heroTitle = document.getElementById('hero-title');
  const heroSubtitle = document.getElementById('hero-subtitle');
  const heroIcon = document.getElementById('hero-icon');

  if (data.incident_services > 0) {
    heroBox.className = 'hero-banner outage';
    heroTitle.textContent = `${data.incident_services} Service Incident${data.incident_services > 1 ? 's' : ''} Detected`;
    heroSubtitle.textContent = 'Our automated prober has detected downtime. Diagnostics underway.';
    heroIcon.innerHTML = `
      <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#fff" stroke-width="2.5" stroke-linecap="round">
        <line x1="18" y1="6" x2="6" y2="18"></line>
        <line x1="6" y1="6" x2="18" y2="18"></line>
      </svg>
    `;
  } else if (data.total_services === 0) {
    heroBox.className = 'hero-banner';
    heroTitle.textContent = 'No Services Configured';
    heroSubtitle.textContent = 'Configure monitors in the Admin console to begin tracking.';
  } else {
    heroBox.className = 'hero-banner';
    heroTitle.textContent = 'All Systems Operational';
    heroSubtitle.textContent = 'All monitored network targets are functioning within normal parameters.';
    heroIcon.innerHTML = `
      <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#fff" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
        <polyline points="20 6 9 17 4 12"></polyline>
      </svg>
    `;
  }

  // Ringkasan Angka Metrik
  document.getElementById('operational-count').textContent = `${data.operational_services} / ${data.total_services}`;

  if (data.monitors.length > 0) {
    const totalUptime = data.monitors.reduce((acc, m) => acc + m.uptime_24h, 0);
    const avgUptime = (totalUptime / data.monitors.length).toFixed(2);
    document.getElementById('overall-uptime').textContent = `${avgUptime}%`;

    const totalLat = data.monitors.reduce((acc, m) => acc + m.avg_latency_ms, 0);
    const avgLat = (totalLat / data.monitors.length).toFixed(1);
    document.getElementById('avg-latency').textContent = `${avgLat} ms`;
  } else {
    document.getElementById('overall-uptime').textContent = '100.0%';
    document.getElementById('avg-latency').textContent = '-- ms';
  }

  // Daftar Layanan & Riwayat Bar Segmen
  const container = document.getElementById('services-list');
  if (data.monitors.length === 0) {
    container.innerHTML = `<div style="padding: 32px; text-align: center; color: var(--text-muted);">No services to display.</div>`;
    return;
  }

  container.innerHTML = '';
  for (const m of data.monitors) {
    const row = document.createElement('div');
    row.className = 'service-row';

    const isUp = m.status === 'up';
    const pillClass = isUp ? '' : (m.status === 'down' ? 'down' : 'paused');
    const pillText = isUp ? 'Operational' : (m.status === 'down' ? 'Major Outage' : 'Paused');

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

    row.innerHTML = `
      <div class="service-header">
        <div class="service-name">${escapeHtml(m.name)}</div>
        <div class="service-status-pill ${pillClass}">${pillText}</div>
      </div>
      <div class="history-strip">
        ${barsHtml}
      </div>
      <div class="strip-footer">
        <span>30 days ago</span>
        <span>${m.uptime_24h.toFixed(1)}% uptime (24h)</span>
        <span>Today</span>
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
            <span class="status-dot down"></span>
            <span>${escapeHtml(inc.service_name)}</span>
            <span class="incident-status-tag">Ongoing Outage</span>
          </div>
          <div class="incident-time">Started: ${escapeHtml(inc.started_at)}</div>
        </div>
        <div class="incident-detail-text">
          ${escapeHtml(inc.error_message || 'Service is unreachable or returning error status')}
        </div>
      `;
      activeContainer.appendChild(card);
    }
    activeContainer.style.display = 'flex';
  } else {
    activeContainer.innerHTML = '';
    activeContainer.style.display = 'none';
  }

  // Incident History Section (Past 30 Days)
  const historyContainer = document.getElementById('incidents-list');
  if (data.recent_incidents && data.recent_incidents.length > 0) {
    historyContainer.innerHTML = '';
    for (const inc of data.recent_incidents) {
      const row = document.createElement('div');
      row.className = 'incident-row';
      const isOngoing = inc.is_ongoing;
      const durationText = inc.duration_sec ? formatDuration(inc.duration_sec) : 'Ongoing';
      const statusPill = isOngoing 
        ? `<span class="incident-ongoing-pill">Ongoing Outage</span>`
        : `<span class="incident-resolved-pill">Resolved in ${durationText}</span>`;

      row.innerHTML = `
        <div class="incident-header-meta">
          <div class="incident-name">
            <span>${escapeHtml(inc.service_name)}</span>
            ${statusPill}
          </div>
          <div class="incident-time">${escapeHtml(inc.started_at)}</div>
        </div>
        <div class="incident-desc">
          ${escapeHtml(inc.error_message || 'Unreachable / Connection Failure')}
        </div>
      `;
      historyContainer.appendChild(row);
    }
  } else {
    historyContainer.innerHTML = `
      <div style="padding: 24px; text-align: center; color: var(--text-muted); font-size: 13px;">
        No incidents reported in the last 30 days.
      </div>
    `;
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

// Koneksi SSE (Server-Sent Events) terkendali untuk update real-time
let publicSse = null;
let publicReconnectTimer = null;

function initLiveUpdates() {
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
      loadPublicData();
    };
    publicSse.onerror = () => {
      if (publicSse) {
        publicSse.close();
        publicSse = null;
      }
      publicReconnectTimer = setTimeout(() => {
        initLiveUpdates();
      }, 6000);
    };
  } catch (err) {
    publicReconnectTimer = setTimeout(() => {
      initLiveUpdates();
    }, 6000);
  }
}

function escapeHtml(str) {
  if (!str) return '';
  return str.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

document.addEventListener('DOMContentLoaded', () => {
  loadPublicData();
  initLiveUpdates();
});
