use std::sync::Arc;
use tokio::sync::Mutex;
use rusqlite::{Connection, Result};

// DbPool mewakili koneksi SQLite thread-safe yang dibagikan ke seluruh Controller dan Worker
pub type DbPool = Arc<Mutex<Connection>>;

// Inisialisasi koneksi SQLite, pengaturan WAL mode, dan pembuatan schema tabel
pub fn init_db(db_path: &str) -> Result<DbPool> {
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
             status TEXT NOT NULL DEFAULT 'pending',
             last_latency_ms REAL,
             last_check_at TEXT,
             created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
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

         Ok(Arc::new(Mutex::new(conn)))
}
