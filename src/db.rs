use std::sync::Arc;
use tokio::sync::Mutex;
use rusqlite::{params, Connection, Result};
use crate::models::{CreateMonitorInput, Heartbeat, Monitor, MonitorDetail, ProbeResult};

pub type DbPool = Arc<Mutex<Connection>>;

pub fn init_db(db_path: &str) -> Result<DbPool> {
    let conn = Connection::open(db_path)?;
    
    // SQLite performance & safety pragmas
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

    // Fetch recent 30 heartbeats
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
    // Reverse to chronological order (oldest to newest for sparkline)
    heartbeats.reverse();

    // Calculate 24h stats
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

pub async fn create_monitor(db: &DbPool, input: CreateMonitorInput) -> Result<i64> {
    let conn = db.lock().await;
    conn.execute(
        "INSERT INTO monitors (name, monitor_type, target, interval_sec, timeout_sec)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![input.name, input.monitor_type, input.target, input.interval_sec, input.timeout_sec],
    )?;
    Ok(conn.last_insert_rowid())
}

pub async fn delete_monitor(db: &DbPool, id: i64) -> Result<bool> {
    let conn = db.lock().await;
    let affected = conn.execute("DELETE FROM monitors WHERE id = ?1", params![id])?;
    Ok(affected > 0)
}

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

pub async fn record_heartbeat(db: &DbPool, result: &ProbeResult) -> Result<()> {
    let conn = db.lock().await;
    let status_str = if result.is_up { "up" } else { "down" };

    // Update monitor status
    conn.execute(
        "UPDATE monitors 
         SET status = ?1, last_latency_ms = ?2, last_check_at = datetime('now', 'localtime')
         WHERE id = ?3",
        params![status_str, result.latency_ms, result.monitor_id],
    )?;

    // Insert heartbeat
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

    // Auto-prune old heartbeats (keep last 500 per monitor to save space on flash/RAM)
    conn.execute(
        "DELETE FROM heartbeats 
         WHERE monitor_id = ?1 AND id NOT IN (
             SELECT id FROM heartbeats WHERE monitor_id = ?1 ORDER BY id DESC LIMIT 500
         )",
        params![result.monitor_id],
    )?;

    Ok(())
}
