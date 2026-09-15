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
}

// Channel broadcast Tokio untuk pola 1-ke-banyak (satu hasil probe diterima semua tab browser yang terbuka)
pub type EventSender = broadcast::Sender<ProbeEvent>;

// Memulai scheduler background loop yang berjalan terus-menerus selama aplikasi aktif
pub fn start_scheduler(db: DbPool, event_tx: EventSender) {
    // HashMap in-memory untuk mencatat waktu terakhir tiap monitor dieksekusi
    let last_checked = Arc::new(Mutex::new(HashMap::new()));
    let db_clone = db.clone();
    let tx_clone = event_tx.clone();
    let last_checked_clone = last_checked.clone();

    tokio::spawn(async move {
        info!("UptimePulse engine scheduler started.");
        loop {
            // Tick interval 1 detik untuk mengevaluasi apakah ada monitor yang sudah jatuh tempo
            sleep(Duration::from_secs(1)).await;

            let monitors = match Monitor::all(&db_clone).await {
                Ok(m) => m,
                Err(e) => {
                    error!("Scheduler failed to query monitors: {}", e);
                    continue;
                }
            };

            for monitor in monitors {
                if !monitor.is_active {
                    continue;
                }

                let now = Instant::now();
                let should_check = {
                    let mut tracker = last_checked_clone.lock().await;
                    match tracker.get(&monitor.id) {
                        Some(last) => {
                            if now.duration_since(*last).as_secs() >= monitor.interval_sec as u64 {
                                tracker.insert(monitor.id, now);
                                true
                            } else {
                                false
                            }
                        }
                        None => {
                            // Cek langsung pada putaran pertama
                            tracker.insert(monitor.id, now);
                            true
                        }
                    }
                };

                // Setiap probe dieksekusi di tokio::spawn terpisah agar target yang lambat tidak memblokir target lain
                if should_check {
                    let db_task = db_clone.clone();
                    let tx_task = tx_clone.clone();
                    tokio::spawn(async move {
                        run_probe_and_record(&db_task, &tx_task, monitor.id, &monitor.monitor_type, &monitor.target, monitor.timeout_sec).await;
                    });
                }
            }
        }
    });
}

// Menjalankan satu putaran probe, menyimpan rekaman ke SQLite, dan menyiarkan hasil ke SSE
pub async fn run_probe_and_record(
    db: &DbPool,
    tx: &EventSender,
    monitor_id: i64,
    monitor_type: &str,
    target: &str,
    timeout_sec: i64,
) -> ProbeResult {
    let result = prober::probe(monitor_id, monitor_type, target, timeout_sec).await;

    if let Err(e) = Heartbeat::record(db, &result).await {
        error!("Failed to record heartbeat for monitor {}: {}", monitor_id, e);
    }

    if let Err(e) = Incident::process_probe(db, monitor_id, result.is_up, result.error_message.as_deref()).await {
        error!("Failed to process incident for monitor {}: {}", monitor_id, e);
    }

    let status_str = if result.is_up { "up" } else { "down" };
    // Broadcast hasil ke semua koneksi SSE aktif
    let _ = tx.send(ProbeEvent {
        monitor_id: result.monitor_id,
        status: status_str.to_string(),
        is_up: result.is_up,
        latency_ms: result.latency_ms,
        status_code: result.status_code,
        error_message: result.error_message.clone(),
    });

    result
}
