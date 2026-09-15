use std::sync::Arc;
use tokio::sync::Mutex;
use rusqlite::{params, Connection, Result};
use crate::models::{CreateMonitorInput, Heartbeat, Monitor, MonitorDetail, ProbeResult};

// Arc<Mutex<Connection>> memungkinkan koneksi SQLite dibagi dan diakses dengan aman
// antar worker task Tokio yang berjalan asynchronous tanpa race condition.
pub type DbPool = Arc<Mutex<Connection>>;

// Inisialisasi SQLite database lokal dengan pragma optimal untuk performa & efisiensi
pub fn init_db(db_path: &str) -> Result<DbPool> {
    let conn = Connection::open(db_path)?;
    
    // WAL (Write-Ahead Logging) memisahkan operasi baca & tulis sehingga query read tidak memblokir write
    // busy_timeout memberi toleransi 5000ms agar koneksi menunggu jika file SQLite sedang terkunci
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;
         
         CREATE TABLE IF NOT EXISTS monitors (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             name TEXT NOT NULL,
             monitor_type TEXT NOT NULL DEFAULT 'http',
             target TEXT NOT NULL,
             interval_sec INTEGER NOT NULL DEFAULT 60,
             timeout_sec INTEGER NOT NULL DEFAULT 10,
             is_active INTEGER NOT NULL DEFAULT 1,
             status TEXT NOT NULL DEFAULT 'pending',
             last_latency_ms REAL,
             last_check_at TEXT,
             created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
         );

         CREATE TABLE IF NOT EXISTS heartbeats (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             monitor_id INTEGER NOT NULL REFERENCES monitors(id) ON DELETE CASCADE,
             is_up INTEGER NOT NULL,
             status_code INTEGER,
             latency_ms REAL NOT NULL,
             error_message TEXT,
             checked_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
         );

         CREATE INDEX IF NOT EXISTS idx_hb_mon_time ON heartbeats(monitor_id, checked_at DESC);
        "
    )?;

    Ok(Arc::new(Mutex::new(conn)))
}

// Mengambil seluruh daftar monitor untuk panel admin
pub async fn list_monitors(db: &DbPool) -> Result<Vec<Monitor>> {
    let conn = db.lock().await;
    let mut stmt = conn.prepare(
        "SELECT id, name, monitor_type, target, interval_sec, timeout_sec, is_active, status, last_latency_ms, last_check_at, created_at
         FROM monitors ORDER BY id DESC"
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(Monitor {
            id: row.get(0)?,
            name: row.get(1)?,
            monitor_type: row.get(2)?,
            target: row.get(3)?,
            interval_sec: row.get(4)?,
            timeout_sec: row.get(5)?,
            is_active: row.get::<_, i32>(6)? == 1,
            status: row.get(7)?,
            last_latency_ms: row.get(8)?,
            last_check_at: row.get(9)?,
            created_at: row.get(10)?,
        })
    })?;

    let mut list = Vec::new();
    for r in rows {
        list.push(r?);
    }
    Ok(list)
}

// Mengambil detail monitor spesifik beserta riwayat heartbeat terakhir & kalkulasi 24 jam
pub async fn get_monitor_detail(db: &DbPool, id: i64) -> Result<Option<MonitorDetail>> {
    let conn = db.lock().await;
    let mut stmt = conn.prepare(
        "SELECT id, name, monitor_type, target, interval_sec, timeout_sec, is_active, status, last_latency_ms, last_check_at, created_at
         FROM monitors WHERE id = ?1"
    )?;

    let monitor = stmt.query_row([id], |row| {
        Ok(Monitor {
            id: row.get(0)?,
            name: row.get(1)?,
            monitor_type: row.get(2)?,
            target: row.get(3)?,
            interval_sec: row.get(4)?,
            timeout_sec: row.get(5)?,
            is_active: row.get::<_, i32>(6)? == 1,
            status: row.get(7)?,
            last_latency_ms: row.get(8)?,
            last_check_at: row.get(9)?,
            created_at: row.get(10)?,
        })
    });

    let monitor = match monitor {
        Ok(m) => m,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
        Err(e) => return Err(e),
    };

    // Ambil 30 heartbeat terakhir untuk visualisasi sparkline grafik
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
    // Urutkan kronologis dari terlama ke terbaru agar grafik bar digambar dari kiri ke kanan
    heartbeats.reverse();

    // Kalkulasi agregat persentase uptime dan rata-rata latency 24 jam terakhir
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

// Menambahkan entri monitor baru ke database
pub async fn create_monitor(db: &DbPool, input: CreateMonitorInput) -> Result<i64> {
    let conn = db.lock().await;
    conn.execute(
        "INSERT INTO monitors (name, monitor_type, target, interval_sec, timeout_sec)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![input.name, input.monitor_type, input.target, input.interval_sec, input.timeout_sec],
    )?;
    Ok(conn.last_insert_rowid())
}

// Menghapus monitor beserta rekaman heartbeat terkait via cascade constraint
pub async fn delete_monitor(db: &DbPool, id: i64) -> Result<bool> {
    let conn = db.lock().await;
    let affected = conn.execute("DELETE FROM monitors WHERE id = ?1", params![id])?;
    Ok(affected > 0)
}

// Mengaktifkan atau menonaktifkan sementara pemeriksaan berkala pada suatu target
pub async fn toggle_pause_monitor(db: &DbPool, id: i64) -> Result<bool> {
    let conn = db.lock().await;
    let affected = conn.execute(
        "UPDATE monitors 
         SET is_active = CASE WHEN is_active = 1 THEN 0 ELSE 1 END,
             status = CASE WHEN is_active = 1 THEN 'paused' ELSE 'pending' END
         WHERE id = ?1",
        params![id],
    )?;
    Ok(affected > 0)
}

// Mencatat hasil probe dan memangkas riwayat lama agar ukuran file database tetap kecil
pub async fn record_heartbeat(db: &DbPool, result: &ProbeResult) -> Result<()> {
    let conn = db.lock().await;
    let status_str = if result.is_up { "up" } else { "down" };

    // Update status ringkasan pada entitas monitor
    conn.execute(
        "UPDATE monitors 
         SET status = ?1, last_latency_ms = ?2, last_check_at = datetime('now', 'localtime')
         WHERE id = ?3",
        params![status_str, result.latency_ms, result.monitor_id],
    )?;

    // Catat baris baru ke tabel log heartbeats
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

    // Pembatasan log: Simpan maksimal 500 riwayat terakhir per target untuk mencegah flash memory router aus
    conn.execute(
        "DELETE FROM heartbeats 
         WHERE monitor_id = ?1 AND id NOT IN (
             SELECT id FROM heartbeats WHERE monitor_id = ?1 ORDER BY id DESC LIMIT 500
         )",
        params![result.monitor_id],
    )?;

    Ok(())
}

// Struktur data publik untuk disajikan ke user umum di halaman status utama
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct PublicMonitorSummary {
    pub id: i64,
    pub name: String,
    pub status: String,
    pub uptime_24h: f64,
    pub avg_latency_ms: f64,
    pub history: Vec<bool>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct PublicSystemSummary {
    pub overall_status: String, // "operational", "degraded", "outage"
    pub total_services: usize,
    pub operational_services: usize,
    pub incident_services: usize,
    pub monitors: Vec<PublicMonitorSummary>,
}

// Mengumpulkan agregasi status publik tanpa membocorkan endpoint target / IP sensitif
pub async fn get_public_summary(db: &DbPool) -> Result<PublicSystemSummary> {
    let monitors = list_monitors(db).await?;
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

        let detail = get_monitor_detail(db, m.id).await?.unwrap();
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
