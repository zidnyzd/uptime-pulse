use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, Mutex};
use tokio::time::{sleep, Duration};
use tracing::{error, info};
use serde::{Deserialize, Serialize};

use crate::database::DbPool;
use crate::models::{Heartbeat, Incident, Monitor, ProbeResult};
use crate::prober;

// Event yang dipancarkan secara real-time ke web browser via Server-Sent Events (SSE)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeEvent {
    pub monitor_id: i64,
    pub status: String,
    pub is_up: bool,
    pub latency_ms: f64,
    pub status_code: Option<i32>,
    pub error_message: Option<String>,
    pub consecutive_fails: i64,
    pub max_retries: i64,
}

// Channel broadcast Tokio untuk pola 1-ke-banyak (satu hasil probe diterima semua tab browser yang terbuka)
pub type EventSender = broadcast::Sender<ProbeEvent>;

// Memulai scheduler background loop yang berjalan terus-menerus selama aplikasi aktif
pub fn start_scheduler(db: DbPool, event_tx: EventSender, retention_days: u32) {
    // HashMap in-memory untuk mencatat waktu terakhir tiap monitor dieksekusi
    let last_checked = Arc::new(Mutex::new(HashMap::new()));

    // Worker background untuk pembersihan otomatis (auto-pruning) berkala demi menjaga flash OpenWrt
    let db_prune = db.clone();
    tokio::spawn(async move {
        info!("Auto-pruning worker initialized (Retention: {} days).", retention_days);
        loop {
            // Worker otomatis tidak pernah VACUUM (mahal di eMMC): hanya rollup,
            // hapus data lewat retensi, dan checkpoint WAL.
            match crate::database::prune_old_records(&db_prune, retention_days, false).await {
                Ok(res) => {
                    if res.heartbeats_deleted > 0
                        || res.sessions_deleted > 0
                        || res.daily_rows_upserted > 0
                    {
                        info!(
                            "Auto-prune executed: {} daily rows rolled up, {} old heartbeats, {} expired sessions purged. WAL truncated.",
                            res.daily_rows_upserted,
                            res.heartbeats_deleted,
                            res.sessions_deleted
                        );
                    }
                }
                Err(e) => {
                    error!("Auto-prune worker error: {}", e);
                }
            }
            // Jalankan pruning setiap 6 jam sekali
            tokio::time::sleep(Duration::from_secs(6 * 3600)).await;
        }
    });

    tokio::spawn(async move {
        info!("Engine scheduler background worker started.");

        loop {
            // Evaluasi interval setiap 1 detik
            sleep(Duration::from_secs(1)).await;

            // Ambil daftar monitor aktif dari SQLite
            let monitors = match Monitor::all(&db).await {
                Ok(m) => m,
                Err(e) => {
                    error!("Scheduler error reading monitors from SQLite: {}", e);
                    continue;
                }
            };

            for monitor in monitors {
                if !monitor.is_active {
                    continue;
                }

                let now = Instant::now();
                let should_check = {
                    let mut tracker = last_checked.lock().await;

                    // Fast retry: jika sedang gagal (1x atau 2x), re-check lebih cepat (15s) untuk konfirmasi
                    let effective_interval = if monitor.consecutive_fails > 0 && monitor.consecutive_fails < monitor.max_retries {
                        15u64
                    } else {
                        monitor.interval_sec as u64
                    };

                    match tracker.get(&monitor.id) {
                        Some(last) => {
                            if now.duration_since(*last).as_secs() >= effective_interval {
                                tracker.insert(monitor.id, now);
                                true
                            } else {
                                false
                            }
                        }
                        None => {
                            // Stagger initial check saat startup agar 15+ monitor tidak menembak serentak di detik yang sama
                            let interval = monitor.interval_sec.max(5) as u64;
                            let stagger_offset = (monitor.id.abs() as u64 * 3) % interval;
                            let initial_time = now - Duration::from_secs(interval.saturating_sub(stagger_offset));
                            tracker.insert(monitor.id, initial_time);
                            stagger_offset == 0
                        }
                    }
                };

                // Setiap probe dieksekusi di tokio::spawn terpisah agar target yang lambat tidak memblokir target lain
                if should_check {
                    let db_task = db.clone();
                    let tx_task = event_tx.clone();
                    // Salin konfigurasi request agar bisa dipindah ke task (monitor tidak 'static)
                    let http_cfg = prober::HttpRequestConfig {
                        method: monitor.method.clone(),
                        headers: monitor.headers.clone(),
                        body: monitor.body.clone(),
                        json_path: monitor.json_path.clone(),
                        expected_value: monitor.expected_value.clone(),
                    };
                    let m_type = monitor.monitor_type.clone();
                    let m_target = monitor.target.clone();
                    let m_timeout = monitor.timeout_sec;
                    let m_id = monitor.id;
                    tokio::spawn(async move {
                        run_probe_and_record_cfg(&db_task, &tx_task, m_id, &m_type, &m_target, m_timeout, &http_cfg).await;
                    });
                }
            }
        }
    });
}

// Varian dengan konfigurasi request HTTP (method/headers/body).
// Dipakai scheduler & controller agar monitor HTTP bisa non-GET / ber-autentikasi.
pub async fn run_probe_and_record_cfg(
    db: &DbPool,
    tx: &EventSender,
    monitor_id: i64,
    monitor_type: &str,
    target: &str,
    timeout_sec: i64,
    http_cfg: &prober::HttpRequestConfig,
) -> ProbeResult {
    let result = prober::probe_with_config(monitor_id, monitor_type, target, timeout_sec, http_cfg).await;

    let (status_str, consecutive_fails, max_retries) = match Heartbeat::record(db, &result).await {
        Ok(res) => res,
        Err(e) => {
            error!("Failed to record heartbeat for monitor {}: {}", monitor_id, e);
            (if result.is_up { "up".to_string() } else { "down".to_string() }, 0, 3)
        }
    };

    if let Err(e) = Incident::process_probe(db, monitor_id, &status_str, result.error_message.as_deref()).await {
        error!("Failed to process incident for monitor {}: {}", monitor_id, e);
    }

    // Broadcast hasil ke semua koneksi SSE aktif
    let _ = tx.send(ProbeEvent {
        monitor_id: result.monitor_id,
        status: status_str,
        is_up: result.is_up,
        latency_ms: result.latency_ms,
        status_code: result.status_code,
        error_message: result.error_message.clone(),
        consecutive_fails,
        max_retries,
    });

    result
}
