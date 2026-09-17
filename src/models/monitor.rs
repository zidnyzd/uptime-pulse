use rusqlite::{params, Result};
use serde::{Deserialize, Serialize};
use crate::database::DbPool;

// Model Monitor merepresentasikan tabel 'monitors' di database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Monitor {
    pub id: i64,
    pub name: String,
    pub monitor_type: String, // "http", "tcp", atau "ping"
    pub target: String,       // URL atau host:port atau IP
    pub interval_sec: i64,    // Interval pengecekan (detik)
    pub timeout_sec: i64,     // Batas waktu timeout (detik)
    pub max_retries: i64,     // Ambang batas retry sebelum dinyatakan down (default: 3)
    pub consecutive_fails: i64, // Jumlah kegagalan beruntun saat ini
    pub is_active: bool,      // Status aktif atau dijeda
    pub is_public: bool,      // Visibilitas publik: true = tampil di status page publik
    pub status: String,       // "up", "down", "pending", "paused", "retrying"
    pub last_latency_ms: Option<f64>,
    pub last_check_at: Option<String>,
    pub created_at: String,
    pub sort_order: i64,
    // Konfigurasi request HTTP kustom (hanya berlaku untuk tipe http/https).
    // PERINGATAN: `headers` dan `body` dapat memuat kredensial (API key/token).
    // Struct ini hanya boleh diserialisasi pada endpoint admin terautentikasi.
    pub method: String,       // GET/POST/PUT/PATCH/DELETE/HEAD/OPTIONS
    pub headers: String,      // Satu header per baris, format "Nama: Nilai"
    pub body: String,         // Body request (untuk method yang mendukung)
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
    #[serde(default = "default_max_retries")]
    pub max_retries: i64,
    #[serde(default = "default_is_public")]
    pub is_public: bool,
    #[serde(default = "default_method")]
    pub method: String,
    #[serde(default)]
    pub headers: String,
    #[serde(default)]
    pub body: String,
}

// DTO untuk pembaruan monitor yang sudah ada
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMonitorInput {
    pub name: String,
    pub monitor_type: String,
    pub target: String,
    pub interval_sec: i64,
    pub timeout_sec: i64,
    #[serde(default = "default_max_retries")]
    pub max_retries: i64,
    #[serde(default = "default_is_public")]
    pub is_public: bool,
    #[serde(default = "default_method")]
    pub method: String,
    #[serde(default)]
    pub headers: String,
    #[serde(default)]
    pub body: String,
}

fn default_interval() -> i64 { 60 }
fn default_timeout() -> i64 { 10 }
fn default_max_retries() -> i64 { 3 }
fn default_is_public() -> bool { true }
fn default_method() -> String { "GET".to_string() }

/// Menormalkan method HTTP ke bentuk kanonik. Method tak dikenal jatuh ke GET
/// supaya data lama/typo tidak membuat probe gagal total.
pub fn normalize_method(m: &str) -> String {
    match m.trim().to_ascii_uppercase().as_str() {
        "POST" => "POST",
        "PUT" => "PUT",
        "PATCH" => "PATCH",
        "DELETE" => "DELETE",
        "HEAD" => "HEAD",
        "OPTIONS" => "OPTIONS",
        _ => "GET",
    }
    .to_string()
}

impl Monitor {
    // Mengambil seluruh target monitor dari database
    pub async fn all(db: &DbPool) -> Result<Vec<Monitor>> {
        let conn = db.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, name, monitor_type, target, interval_sec, timeout_sec, max_retries, consecutive_fails, is_active, is_public, status, last_latency_ms, last_check_at, created_at, sort_order, method, headers, body
             FROM monitors ORDER BY sort_order ASC, id ASC"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(Monitor {
                id: row.get(0)?,
                name: row.get(1)?,
                monitor_type: row.get(2)?,
                target: row.get(3)?,
                interval_sec: row.get(4)?,
                timeout_sec: row.get(5)?,
                max_retries: row.get(6)?,
                consecutive_fails: row.get(7)?,
                is_active: row.get::<_, i32>(8)? == 1,
                is_public: row.get::<_, i32>(9)? == 1,
                status: row.get(10)?,
                last_latency_ms: row.get(11)?,
                last_check_at: row.get(12)?,
                created_at: row.get(13)?,
                sort_order: row.get(14)?,
                method: row.get(15)?,
                headers: row.get(16)?,
                body: row.get(17)?,
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
            "SELECT id, name, monitor_type, target, interval_sec, timeout_sec, max_retries, consecutive_fails, is_active, is_public, status, last_latency_ms, last_check_at, created_at, sort_order, method, headers, body
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
                max_retries: row.get(6)?,
                consecutive_fails: row.get(7)?,
                is_active: row.get::<_, i32>(8)? == 1,
                is_public: row.get::<_, i32>(9)? == 1,
                status: row.get(10)?,
                last_latency_ms: row.get(11)?,
                last_check_at: row.get(12)?,
                created_at: row.get(13)?,
                sort_order: row.get(14)?,
                method: row.get(15)?,
                headers: row.get(16)?,
                body: row.get(17)?,
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
        let max_retries = input.max_retries.clamp(1, 10);
        let is_public_int = if input.is_public { 1 } else { 0 };
        let method = normalize_method(&input.method);
        conn.execute(
            "INSERT INTO monitors (name, monitor_type, target, interval_sec, timeout_sec, max_retries, consecutive_fails, is_public, sort_order, method, headers, body)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, (SELECT COALESCE(MAX(sort_order), 0) + 1 FROM monitors), ?8, ?9, ?10)",
            params![input.name, input.monitor_type, input.target, input.interval_sec, input.timeout_sec, max_retries, is_public_int, method, input.headers, input.body],
        )?;
        Ok(conn.last_insert_rowid())
    }

    // Memperbarui susunan urutan monitor secara batch
    pub async fn reorder(db: &DbPool, ids: &[i64]) -> Result<()> {
        let mut conn = db.lock().await;
        let tx = conn.transaction()?;
        for (index, id) in ids.iter().enumerate() {
            tx.execute(
                "UPDATE monitors SET sort_order = ?1 WHERE id = ?2",
                params![index as i64, id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    // Menghapus data monitor dan seluruh relasi heartbeat di SQLite
    pub async fn delete(db: &DbPool, id: i64) -> Result<bool> {
        let conn = db.lock().await;
        let affected = conn.execute("DELETE FROM monitors WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    // Mereset seluruh statistik monitor dari awal: hapus heartbeat + insiden,
    // nolkan hitungan gagal beruntun, dan kembalikan status ke 'pending'
    pub async fn reset_stats(db: &DbPool, id: i64) -> Result<Option<(usize, usize)>> {
        let conn = db.lock().await;
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM monitors WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )?;
        if exists == 0 {
            return Ok(None);
        }
        let heartbeats_deleted =
            conn.execute("DELETE FROM heartbeats WHERE monitor_id = ?1", params![id])?;
        let incidents_deleted =
            conn.execute("DELETE FROM incidents WHERE monitor_id = ?1", params![id])?;
        conn.execute(
            "UPDATE monitors
             SET consecutive_fails = 0, status = 'pending',
                 last_latency_ms = NULL, last_check_at = NULL
             WHERE id = ?1",
            params![id],
        )?;
        Ok(Some((heartbeats_deleted, incidents_deleted)))
    }

    // Mengubah status aktif (pause / resume)
    pub async fn toggle_pause(db: &DbPool, id: i64) -> Result<bool> {
        let conn = db.lock().await;
        let affected = conn.execute(
            "UPDATE monitors 
             SET is_active = CASE WHEN is_active = 1 THEN 0 ELSE 1 END,
                 status = CASE WHEN is_active = 1 THEN 'paused' ELSE 'pending' END,
                 consecutive_fails = 0
             WHERE id = ?1",
            params![id],
        )?;
        Ok(affected > 0)
    }

    // Memperbarui konfigurasi monitor yang sudah ada
    // Catatan: headers/body yang dikirim kosong berarti dihapus (bukan dipertahankan),
    // karena form admin selalu memuat nilai saat ini terlebih dahulu.
    pub async fn update(db: &DbPool, id: i64, input: &UpdateMonitorInput) -> Result<bool> {
        let conn = db.lock().await;
        let max_retries = input.max_retries.clamp(1, 10);
        let interval_sec = input.interval_sec.max(5);
        let timeout_sec = input.timeout_sec.max(1);
        let is_public_int = if input.is_public { 1 } else { 0 };
        let method = normalize_method(&input.method);

        let affected = conn.execute(
            "UPDATE monitors 
             SET name = ?1, monitor_type = ?2, target = ?3, interval_sec = ?4, timeout_sec = ?5, max_retries = ?6, is_public = ?7, method = ?8, headers = ?9, body = ?10
             WHERE id = ?11",
            params![input.name, input.monitor_type, input.target, interval_sec, timeout_sec, max_retries, is_public_int, method, input.headers, input.body, id],
        )?;
        Ok(affected > 0)
    }
}
