use chrono::{Duration, Local};
use rusqlite::{params, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

// Bucket agregasi bar visual (mewakili 1 hari di publik atau 1 jam di admin)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarBucket {
    pub label: String,       // "15 Sep" atau "14:00"
    pub date_key: String,    // "2026-09-15" atau "2026-09-15 14:00"
    pub total_checks: i64,
    pub up_checks: i64,
    pub uptime_pct: f64,     // 0.0 - 100.0
    pub avg_latency_ms: f64,
    pub status: String,      // "up", "degraded", "down", "empty"
}

// Data detail monitor untuk admin (termasuk 24 bar per-jam untuk 24 jam terakhir)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorDetail {
    pub monitor: Monitor,
    pub hourly_bars: Vec<BarBucket>,
    pub recent_heartbeats: Vec<Heartbeat>,
    pub uptime_24h: f64,
    pub avg_latency_24h: f64,
}

// Ringkasan monitor publik (termasuk 30 bar harian untuk 30 hari terakhir)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicMonitorSummary {
    pub id: i64,
    pub name: String,
    pub status: String,
    pub uptime_24h: f64,
    pub avg_latency_ms: f64,
    pub daily_bars: Vec<BarBucket>,
}

// Ringkasan sistem publik untuk status page
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicSystemSummary {
    pub overall_status: String, // "operational", "degraded", "outage"
    pub total_services: usize,
    pub operational_services: usize,
    pub incident_services: usize,
    pub monitors: Vec<PublicMonitorSummary>,
    pub active_incidents: Vec<crate::models::Incident>,
    pub recent_incidents: Vec<crate::models::Incident>,
    pub branding: crate::models::BrandingSettings,
}

impl Heartbeat {
    // Mencatat hasil probe dan memperbarui status monitor di database
    // Mengembalikan tuple (status_string, consecutive_fails, max_retries)
    pub async fn record(db: &DbPool, result: &ProbeResult) -> Result<(String, i64, i64)> {
        let conn = db.lock().await;

        // Ambil max_retries dan consecutive_fails saat ini
        let (max_retries, current_fails): (i64, i64) = conn
            .query_row(
                "SELECT max_retries, consecutive_fails FROM monitors WHERE id = ?1",
                params![result.monitor_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap_or((3, 0));

        let (status_str, new_fails) = if result.is_up {
            ("up".to_string(), 0)
        } else {
            let next_fails = current_fails + 1;
            if next_fails >= max_retries {
                ("down".to_string(), next_fails)
            } else {
                ("retrying".to_string(), next_fails)
            }
        };

        // Update status terkini dan consecutive_fails pada tabel monitors
        conn.execute(
            "UPDATE monitors 
             SET status = ?1, consecutive_fails = ?2, last_latency_ms = ?3, last_check_at = datetime('now', 'localtime')
             WHERE id = ?4",
            params![status_str, new_fails, result.latency_ms, result.monitor_id],
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

        // Pruning berkala: Hapus riwayat yang lebih lama dari 30 hari agar flash router awet
        conn.execute(
            "DELETE FROM heartbeats 
             WHERE monitor_id = ?1 AND checked_at < datetime('now', '-30 days', 'localtime')",
            params![result.monitor_id],
        )?;

        Ok((status_str, new_fails, max_retries))
    }

    // Mengambil 24 bucket per-jam untuk 24 jam terakhir (untuk visualisasi bar admin)
    pub async fn get_hourly_buckets(db: &DbPool, monitor_id: i64) -> Result<Vec<BarBucket>> {
        let conn = db.lock().await;
        let mut stmt = conn.prepare(
            "SELECT 
                strftime('%Y-%m-%d %H:00', checked_at) as hr,
                COUNT(*) as total,
                SUM(CASE WHEN is_up = 1 THEN 1 ELSE 0 END) as up_cnt,
                AVG(CASE WHEN is_up = 1 THEN latency_ms ELSE NULL END) as avg_lat
             FROM heartbeats 
             WHERE monitor_id = ?1 AND checked_at >= datetime('now', '-23 hours', 'localtime')
             GROUP BY hr"
        )?;

        struct RowData {
            total: i64,
            up: i64,
            lat: Option<f64>,
        }

        let mut map: HashMap<String, RowData> = HashMap::new();
        let rows = stmt.query_map([monitor_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                RowData {
                    total: row.get(1)?,
                    up: row.get::<_, Option<i64>>(2)?.unwrap_or(0),
                    lat: row.get(3)?,
                },
            ))
        })?;

        for r in rows {
            let (hr, data) = r?;
            map.insert(hr, data);
        }

        let now = Local::now();
        let mut buckets = Vec::with_capacity(24);

        // Buat tepat 24 slot jam dari 23 jam lalu hingga jam sekarang
        for i in (0..24).rev() {
            let target_time = now - Duration::hours(i);
            let hr_key = target_time.format("%Y-%m-%d %H:00").to_string();
            let label = target_time.format("%H:00").to_string();

            if let Some(d) = map.get(&hr_key) {
                let uptime_pct = if d.total > 0 {
                    (d.up as f64 / d.total as f64) * 100.0
                } else {
                    100.0
                };

                let status = if d.total == 0 {
                    "empty".to_string()
                } else if d.up == d.total {
                    "up".to_string()
                } else if uptime_pct >= 95.0 {
                    "degraded".to_string()
                } else {
                    "down".to_string()
                };

                buckets.push(BarBucket {
                    label,
                    date_key: hr_key,
                    total_checks: d.total,
                    up_checks: d.up,
                    uptime_pct: (uptime_pct * 10.0).round() / 10.0,
                    avg_latency_ms: (d.lat.unwrap_or(0.0) * 10.0).round() / 10.0,
                    status,
                });
            } else {
                buckets.push(BarBucket {
                    label,
                    date_key: hr_key,
                    total_checks: 0,
                    up_checks: 0,
                    uptime_pct: 100.0,
                    avg_latency_ms: 0.0,
                    status: "empty".to_string(),
                });
            }
        }

        Ok(buckets)
    }

    // Mengambil 30 bucket harian untuk 30 hari terakhir (untuk visualisasi bar publik)
    pub async fn get_daily_buckets(db: &DbPool, monitor_id: i64) -> Result<Vec<BarBucket>> {
        let conn = db.lock().await;
        let mut stmt = conn.prepare(
            "SELECT 
                strftime('%Y-%m-%d', checked_at) as day,
                COUNT(*) as total,
                SUM(CASE WHEN is_up = 1 THEN 1 ELSE 0 END) as up_cnt,
                AVG(CASE WHEN is_up = 1 THEN latency_ms ELSE NULL END) as avg_lat
             FROM heartbeats 
             WHERE monitor_id = ?1 AND checked_at >= date('now', '-29 days', 'localtime')
             GROUP BY day"
        )?;

        struct RowData {
            total: i64,
            up: i64,
            lat: Option<f64>,
        }

        let mut map: HashMap<String, RowData> = HashMap::new();
        let rows = stmt.query_map([monitor_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                RowData {
                    total: row.get(1)?,
                    up: row.get::<_, Option<i64>>(2)?.unwrap_or(0),
                    lat: row.get(3)?,
                },
            ))
        })?;

        for r in rows {
            let (day, data) = r?;
            map.insert(day, data);
        }

        let now = Local::now();
        let mut buckets = Vec::with_capacity(30);

        // Buat tepat 30 slot hari dari 29 hari lalu hingga hari ini
        for i in (0..30).rev() {
            let target_date = now - Duration::days(i);
            let day_key = target_date.format("%Y-%m-%d").to_string();
            let label = target_date.format("%d %b").to_string();

            if let Some(d) = map.get(&day_key) {
                let uptime_pct = if d.total > 0 {
                    (d.up as f64 / d.total as f64) * 100.0
                } else {
                    100.0
                };

                let status = if d.total == 0 {
                    "empty".to_string()
                } else if d.up == d.total {
                    "up".to_string()
                } else if uptime_pct >= 95.0 {
                    "degraded".to_string()
                } else {
                    "down".to_string()
                };

                buckets.push(BarBucket {
                    label,
                    date_key: day_key,
                    total_checks: d.total,
                    up_checks: d.up,
                    uptime_pct: (uptime_pct * 10.0).round() / 10.0,
                    avg_latency_ms: (d.lat.unwrap_or(0.0) * 10.0).round() / 10.0,
                    status,
                });
            } else {
                buckets.push(BarBucket {
                    label,
                    date_key: day_key,
                    total_checks: 0,
                    up_checks: 0,
                    uptime_pct: 100.0,
                    avg_latency_ms: 0.0,
                    status: "empty".to_string(),
                });
            }
        }

        Ok(buckets)
    }

    // Mengambil detail lengkap monitor beserta riwayat hourly bars & 24h stats untuk admin
    pub async fn find_detail(db: &DbPool, id: i64) -> Result<Option<MonitorDetail>> {
        let monitor = match Monitor::find(db, id).await? {
            Some(m) => m,
            None => return Ok(None),
        };

        let hourly_bars = Self::get_hourly_buckets(db, id).await?;

        let conn = db.lock().await;

        // Ambil 30 raw heartbeat terakhir untuk audit log
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
            hourly_bars,
            recent_heartbeats: heartbeats,
            uptime_24h,
            avg_latency_24h: avg_lat.unwrap_or(0.0),
        }))
    }

    // Mengumpulkan agregasi status untuk halaman publik klien (dengan 30 daily buckets)
    pub async fn get_public_summary(db: &DbPool) -> Result<PublicSystemSummary> {
        let monitors = Monitor::all(db).await?;
        let mut public_items = Vec::new();
        let mut up_count = 0;
        let mut down_count = 0;

        for m in &monitors {
            if !m.is_active || !m.is_public {
                continue;
            }

            if m.status == "up" || m.status == "retrying" {
                up_count += 1;
            } else if m.status == "down" {
                down_count += 1;
            }

            let daily_bars = Self::get_daily_buckets(db, m.id).await?;

            // Hitung uptime 24 jam terakhir
            let conn = db.lock().await;
            let mut stats_stmt = conn.prepare(
                "SELECT COUNT(*), SUM(CASE WHEN is_up = 1 THEN 1 ELSE 0 END), AVG(CASE WHEN is_up = 1 THEN latency_ms ELSE NULL END)
                 FROM heartbeats WHERE monitor_id = ?1 AND checked_at >= datetime('now', '-1 day', 'localtime')"
            )?;

            let (total_24h, up_24h, avg_lat_24h): (i64, i64, Option<f64>) = stats_stmt.query_row([m.id], |row| {
                Ok((
                    row.get(0)?,
                    row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                    row.get(2)?,
                ))
            })?;

            let uptime_24h = if total_24h > 0 {
                (up_24h as f64 / total_24h as f64) * 100.0
            } else {
                100.0
            };

            public_items.push(PublicMonitorSummary {
                id: m.id,
                name: m.name.clone(),
                status: m.status.clone(),
                uptime_24h,
                avg_latency_ms: avg_lat_24h.unwrap_or(0.0),
                daily_bars,
            });
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

        let active_incidents = crate::models::Incident::list_ongoing(db).await?;
        let recent_incidents = crate::models::Incident::list_recent(db, 10).await?;
        let branding = crate::models::BrandingSettings::load(db)
            .await
            .unwrap_or_default();

        Ok(PublicSystemSummary {
            overall_status,
            total_services: public_items.len(),
            operational_services: up_count,
            incident_services: down_count,
            monitors: public_items,
            active_incidents,
            recent_incidents,
            branding,
        })
    }
}
