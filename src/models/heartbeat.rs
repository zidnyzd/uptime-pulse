use rusqlite::{params, Result};
use serde::{Deserialize, Serialize};
use crate::database::DbPool;
use crate::models::monitor::Monitor;

// Log catatan hasil satu kali eksekusi probe
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heartbeat {
    pub id: i64,
    pub monitor_id: i64,
    pub is_up: bool,
    pub status_code: Option<i32>,
    pub latency_ms: f64,
    pub error_message: Option<String>,
    pub checked_at: String,
}

// Data sementara hasil probing di layer jaringan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeResult {
    pub monitor_id: i64,
    pub is_up: bool,
    pub status_code: Option<i32>,
    pub latency_ms: f64,
    pub error_message: Option<String>,
}

// Data gabungan monitor + riwayat untuk panel admin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorDetail {
    pub monitor: Monitor,
    pub recent_heartbeats: Vec<Heartbeat>,
    pub uptime_24h: f64,
    pub avg_latency_24h: f64,
}

// Ringkasan monitor publik (disanitasi tanpa info endpoint internal)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicMonitorSummary {
    pub id: i64,
    pub name: String,
    pub status: String,
    pub uptime_24h: f64,
    pub avg_latency_ms: f64,
    pub history: Vec<bool>,
}

// Ringkasan sistem publik untuk landing page / status page
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicSystemSummary {
    pub overall_status: String, // "operational", "degraded", "outage"
    pub total_services: usize,
    pub operational_services: usize,
    pub incident_services: usize,
    pub monitors: Vec<PublicMonitorSummary>,
}

impl Heartbeat {
    // Mencatat hasil probe dan memperbarui status monitor di database
    pub async fn record(db: &DbPool, result: &ProbeResult) -> Result<()> {
        let conn = db.lock().await;
        let status_str = if result.is_up { "up" } else { "down" };

        // Update status terkini pada tabel monitors
        conn.execute(
            "UPDATE monitors 
             SET status = ?1, last_latency_ms = ?2, last_check_at = datetime('now', 'localtime')
             WHERE id = ?3",
            params![status_str, result.latency_ms, result.monitor_id],
        )?;

        // Catat ke tabel log heartbeats
        conn.execute(
            "INSERT INTO heartbeats (monitor_id, is_up, status_code, latency_ms, error_message)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                result.monitor_id,
                if result.is_up { 1 } else { 0 },
                result.status_code,
                result.latency_ms,
                result.error_message,
            ],
        )?;

        // Pruning otomatis: Batasi 500 riwayat per target agar storage SQLite tidak membesar
        conn.execute(
            "DELETE FROM heartbeats 
             WHERE monitor_id = ?1 AND id NOT IN (
                 SELECT id FROM heartbeats WHERE monitor_id = ?1 ORDER BY id DESC LIMIT 500
             )",
            params![result.monitor_id],
        )?;

        Ok(())
    }

    // Mengambil detail lengkap monitor beserta riwayat heartbeat & persentase 24 jam
    pub async fn find_detail(db: &DbPool, id: i64) -> Result<Option<MonitorDetail>> {
        let monitor = match Monitor::find(db, id).await? {
            Some(m) => m,
            None => return Ok(None),
        };

        let conn = db.lock().await;

        // Ambil 30 heartbeat terakhir
        let mut hb_stmt = conn.prepare(
            "SELECT id, monitor_id, is_up, status_code, latency_ms, error_message, checked_at
             FROM heartbeats WHERE monitor_id = ?1 ORDER BY id DESC LIMIT 30"
        )?;

        let hb_rows = hb_stmt.query_map([id], |row| {
            Ok(Heartbeat {
                id: row.get(0)?,
                monitor_id: row.get(1)?,
                is_up: row.get::<_, i32>(2)? == 1,
                status_code: row.get(3)?,
                latency_ms: row.get(4)?,
                error_message: row.get(5)?,
                checked_at: row.get(6)?,
            })
        })?;

        let mut heartbeats = Vec::new();
        for hb in hb_rows {
            heartbeats.push(hb?);
        }
        heartbeats.reverse();

        // Hitung persentase uptime dan rata-rata latency 24 jam terakhir
        let mut stats_stmt = conn.prepare(
            "SELECT COUNT(*), SUM(CASE WHEN is_up = 1 THEN 1 ELSE 0 END), AVG(CASE WHEN is_up = 1 THEN latency_ms ELSE NULL END)
             FROM heartbeats WHERE monitor_id = ?1 AND checked_at >= datetime('now', '-1 day', 'localtime')"
        )?;

        let (total_checks, up_checks, avg_lat): (i64, i64, Option<f64>) = stats_stmt.query_row([id], |row| {
            Ok((
                row.get(0)?,
                row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                row.get(2)?,
            ))
        })?;

        let uptime_24h = if total_checks > 0 {
            (up_checks as f64 / total_checks as f64) * 100.0
        } else {
            100.0
        };

        Ok(Some(MonitorDetail {
            monitor,
            recent_heartbeats: heartbeats,
            uptime_24h,
            avg_latency_24h: avg_lat.unwrap_or(0.0),
        }))
    }

    // Mengumpulkan agregasi status untuk halaman publik klien
    pub async fn get_public_summary(db: &DbPool) -> Result<PublicSystemSummary> {
        let monitors = Monitor::all(db).await?;
        let mut public_items = Vec::new();
        let mut up_count = 0;
        let mut down_count = 0;

        for m in &monitors {
            if !m.is_active {
                continue;
            }

            if m.status == "up" {
                up_count += 1;
            } else if m.status == "down" {
                down_count += 1;
            }

            if let Some(detail) = Self::find_detail(db, m.id).await? {
                let history = detail.recent_heartbeats.iter().map(|h| h.is_up).collect();
                public_items.push(PublicMonitorSummary {
                    id: m.id,
                    name: m.name.clone(),
                    status: m.status.clone(),
                    uptime_24h: detail.uptime_24h,
                    avg_latency_ms: detail.avg_latency_24h,
                    history,
                });
            }
        }

        let overall_status = if down_count == 0 && !public_items.is_empty() {
            "operational".to_string()
        } else if down_count > 0 && up_count > 0 {
            "degraded".to_string()
        } else if down_count > 0 && up_count == 0 {
            "outage".to_string()
        } else {
            "operational".to_string()
        };

        Ok(PublicSystemSummary {
            overall_status,
            total_services: public_items.len(),
            operational_services: up_count,
            incident_services: down_count,
            monitors: public_items,
        })
    }
}
