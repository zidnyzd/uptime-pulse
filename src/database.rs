use std::sync::Arc;
use tokio::sync::Mutex;
use rusqlite::{Connection, Result};

// DbPool mewakili koneksi SQLite thread-safe yang dibagikan ke seluruh Controller dan Worker
pub type DbPool = Arc<Mutex<Connection>>;

// Inisialisasi koneksi SQLite, pengaturan WAL mode, dan pembuatan schema tabel
pub fn init_db(db_path: &str) -> Result<DbPool> {
    // Pastikan direktori folder database dibuat jika belum ada (misal /data saat mount volume container)
    if let Some(parent) = std::path::Path::new(db_path).parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }

    let conn = Connection::open(db_path)?;

    // Pengaturan pragma performa & konkurensi SQLite
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;

         -- Tabel target monitor
         CREATE TABLE IF NOT EXISTS monitors (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             name TEXT NOT NULL,
             monitor_type TEXT NOT NULL DEFAULT 'http',
             target TEXT NOT NULL,
             interval_sec INTEGER NOT NULL DEFAULT 60,
             timeout_sec INTEGER NOT NULL DEFAULT 10,
             max_retries INTEGER NOT NULL DEFAULT 3,
             consecutive_fails INTEGER NOT NULL DEFAULT 0,
             is_active INTEGER NOT NULL DEFAULT 1,
             is_public INTEGER NOT NULL DEFAULT 1,
             status TEXT NOT NULL DEFAULT 'pending',
             last_latency_ms REAL,
             last_check_at TEXT,
             created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
             sort_order INTEGER NOT NULL DEFAULT 0
         );

         -- Tabel log riwayat probe
         CREATE TABLE IF NOT EXISTS heartbeats (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             monitor_id INTEGER NOT NULL REFERENCES monitors(id) ON DELETE CASCADE,
             is_up INTEGER NOT NULL,
             status_code INTEGER,
             latency_ms REAL NOT NULL,
             error_message TEXT,
             checked_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
         );

         -- Tabel kredensial admin
         CREATE TABLE IF NOT EXISTS users (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             username TEXT UNIQUE NOT NULL,
             password_hash TEXT NOT NULL,
             salt TEXT NOT NULL,
             created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
         );

         -- Tabel session token login
         CREATE TABLE IF NOT EXISTS sessions (
             token TEXT PRIMARY KEY,
             user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
             expires_at TEXT NOT NULL,
             created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
         );

         -- Tabel log insiden downtime & durasi pemulihan
         CREATE TABLE IF NOT EXISTS incidents (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             monitor_id INTEGER NOT NULL REFERENCES monitors(id) ON DELETE CASCADE,
             started_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
             resolved_at TEXT,
             duration_sec INTEGER,
             error_message TEXT
         );

         -- Tabel konfigurasi pengaturan aplikasi (Telegram, notifikasi, dll)
         CREATE TABLE IF NOT EXISTS settings (
             key TEXT PRIMARY KEY,
             value TEXT NOT NULL
         );

         CREATE INDEX IF NOT EXISTS idx_hb_mon_time ON heartbeats(monitor_id, checked_at DESC);
         -- Index terpisah untuk prune global (DELETE ... WHERE checked_at < ?) yang
         -- tidak menyebut monitor_id, agar tidak full-table-scan di STB low-power.
         CREATE INDEX IF NOT EXISTS idx_hb_time ON heartbeats(checked_at);
         CREATE INDEX IF NOT EXISTS idx_sessions_token ON sessions(token);
         CREATE INDEX IF NOT EXISTS idx_incidents_mon ON incidents(monitor_id, started_at DESC);
         "
         )?;

    // Migrasi kolom toleransi retry (anti-false alarm) jika belum ada di database lama
    let has_max_retries = conn.prepare("SELECT max_retries FROM monitors LIMIT 1").is_ok();
    if !has_max_retries {
        let _ = conn.execute("ALTER TABLE monitors ADD COLUMN max_retries INTEGER NOT NULL DEFAULT 3", []);
        let _ = conn.execute("ALTER TABLE monitors ADD COLUMN consecutive_fails INTEGER NOT NULL DEFAULT 0", []);
    }

    // Migrasi kolom visibilitas publik/privat jika belum ada
    let has_is_public = conn.prepare("SELECT is_public FROM monitors LIMIT 1").is_ok();
    if !has_is_public {
        let _ = conn.execute("ALTER TABLE monitors ADD COLUMN is_public INTEGER NOT NULL DEFAULT 1", []);
    }

    // Migrasi kolom urutan tampilan (sort_order) jika belum ada
    let has_sort_order = conn.prepare("SELECT sort_order FROM monitors LIMIT 1").is_ok();
    if !has_sort_order {
        let _ = conn.execute("ALTER TABLE monitors ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0", []);
        let _ = conn.execute("UPDATE monitors SET sort_order = id", []);
    }

    // Migrasi kolom HTTP request kustom (method, headers, body) jika belum ada.
    // Memungkinkan pemantauan endpoint non-GET dan API yang butuh autentikasi.
    // CATATAN KEAMANAN: headers/body dapat memuat kredensial (API key/token).
    // Nilai ini HANYA boleh keluar lewat endpoint admin yang terautentikasi —
    // jangan pernah sertakan di /api/public/summary atau payload publik lainnya.
    let has_method = conn.prepare("SELECT method FROM monitors LIMIT 1").is_ok();
    if !has_method {
        let _ = conn.execute("ALTER TABLE monitors ADD COLUMN method TEXT NOT NULL DEFAULT 'GET'", []);
        let _ = conn.execute("ALTER TABLE monitors ADD COLUMN headers TEXT NOT NULL DEFAULT ''", []);
        let _ = conn.execute("ALTER TABLE monitors ADD COLUMN body TEXT NOT NULL DEFAULT ''", []);
    }

    Ok(Arc::new(Mutex::new(conn)))
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PruneResult {
    pub heartbeats_deleted: usize,
    pub sessions_deleted: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DbStats {
    pub db_size_bytes: u64,
    pub wal_size_bytes: u64,
    pub total_heartbeats: i64,
    pub total_monitors: i64,
    pub total_incidents: i64,
    pub total_sessions: i64,
    pub retention_days: u32,
    pub db_path: String,
}

/// Menghapus log riwayat lama sesuai batas hari retensi dan merampingkan WAL SQLite
pub async fn prune_old_records(db: &DbPool, retention_days: u32) -> Result<PruneResult> {
    let conn = db.lock().await;
    let threshold_modifier = format!("-{} days", retention_days);

    // Hapus log heartbeat yang melebihi batas retensi
    let heartbeats_deleted = conn.execute(
        "DELETE FROM heartbeats WHERE checked_at < datetime('now', 'localtime', ?)",
        [&threshold_modifier],
    )?;

    // Hapus session login yang sudah expired
    let sessions_deleted = conn.execute(
        "DELETE FROM sessions WHERE expires_at < datetime('now', 'localtime')",
        [],
    )?;

    // Rampingkan WAL log ke file database utama untuk menghemat ruang flash OpenWrt
    let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");

    Ok(PruneResult {
        heartbeats_deleted,
        sessions_deleted,
    })
}

/// Mengambil informasi ukuran file database dan jumlah rekaman log
pub async fn get_db_stats(db: &DbPool, db_path: &str, retention_days: u32) -> Result<DbStats> {
    let conn = db.lock().await;

    let db_size_bytes = std::fs::metadata(db_path).map(|m| m.len()).unwrap_or(0);
    let wal_size_bytes = std::fs::metadata(format!("{}-wal", db_path))
        .map(|m| m.len())
        .unwrap_or(0);

    let total_heartbeats: i64 = conn
        .query_row("SELECT COUNT(*) FROM heartbeats", [], |row| row.get(0))
        .unwrap_or(0);

    let total_monitors: i64 = conn
        .query_row("SELECT COUNT(*) FROM monitors", [], |row| row.get(0))
        .unwrap_or(0);

    let total_incidents: i64 = conn
        .query_row("SELECT COUNT(*) FROM incidents", [], |row| row.get(0))
        .unwrap_or(0);

    let total_sessions: i64 = conn
        .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
        .unwrap_or(0);

    Ok(DbStats {
        db_size_bytes,
        wal_size_bytes,
        total_heartbeats,
        total_monitors,
        total_incidents,
        total_sessions,
        retention_days,
        db_path: db_path.to_string(),
    })
}
