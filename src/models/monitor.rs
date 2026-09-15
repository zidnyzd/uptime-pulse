use rusqlite::{params, Result};
use serde::{Deserialize, Serialize};
use crate::database::DbPool;

// Model Monitor merepresentasikan tabel 'monitors' di database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Monitor {
    pub id: i64,
    pub name: String,
    pub monitor_type: String, // "http" atau "tcp"
    pub target: String,       // URL atau host:port
    pub interval_sec: i64,    // Interval pengecekan (detik)
    pub timeout_sec: i64,     // Batas waktu timeout (detik)
    pub is_active: bool,      // Status aktif atau dijeda
    pub status: String,       // "up", "down", "pending", "paused"
    pub last_latency_ms: Option<f64>,
    pub last_check_at: Option<String>,
    pub created_at: String,
}

// DTO untuk validasi payload input pembuatan monitor baru
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMonitorInput {
    pub name: String,
    pub monitor_type: String,
    pub target: String,
    #[serde(default = "default_interval")]
    pub interval_sec: i64,
    #[serde(default = "default_timeout")]
    pub timeout_sec: i64,
}

fn default_interval() -> i64 { 60 }
fn default_timeout() -> i64 { 10 }

impl Monitor {
    // Mengambil seluruh target monitor dari database
    pub async fn all(db: &DbPool) -> Result<Vec<Monitor>> {
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

    // Mencari monitor berdasarkan ID
    pub async fn find(db: &DbPool, id: i64) -> Result<Option<Monitor>> {
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

        match monitor {
            Ok(m) => Ok(Some(m)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    // Menyimpan target monitor baru ke SQLite
    pub async fn create(db: &DbPool, input: &CreateMonitorInput) -> Result<i64> {
        let conn = db.lock().await;
        conn.execute(
            "INSERT INTO monitors (name, monitor_type, target, interval_sec, timeout_sec)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![input.name, input.monitor_type, input.target, input.interval_sec, input.timeout_sec],
        )?;
        Ok(conn.last_insert_rowid())
    }

    // Menghapus data monitor dan seluruh relasi heartbeat di SQLite
    pub async fn delete(db: &DbPool, id: i64) -> Result<bool> {
        let conn = db.lock().await;
        let affected = conn.execute("DELETE FROM monitors WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    // Mengubah status aktif (pause / resume)
    pub async fn toggle_pause(db: &DbPool, id: i64) -> Result<bool> {
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
}
