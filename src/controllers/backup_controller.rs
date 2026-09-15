use axum::{
    extract::State,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Local;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

use crate::database::DbPool;
use crate::models::Monitor;

pub struct BackupControllerState {
    pub db: DbPool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupData {
    pub version: String,
    pub app: String,
    pub exported_at: String,
    pub monitors: Vec<BackupMonitorItem>,
    pub settings: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupMonitorItem {
    pub name: String,
    pub monitor_type: String,
    pub target: String,
    pub interval_sec: i64,
    pub timeout_sec: i64,
    pub is_active: bool,
}

#[derive(Debug, Deserialize)]
pub struct RestorePayload {
    #[serde(default = "default_restore_mode")]
    pub mode: String, // "replace" atau "merge"
    pub backup: BackupData,
}

fn default_restore_mode() -> String {
    "replace".to_string()
}

// GET /api/backup/export - Mengunduh seluruh konfigurasi monitor & pengaturan dalam format JSON
pub async fn export_json(
    State(state): State<Arc<BackupControllerState>>,
) -> Result<Response, (StatusCode, String)> {
    let monitors = Monitor::all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let backup_monitors: Vec<BackupMonitorItem> = monitors
        .into_iter()
        .map(|m| BackupMonitorItem {
            name: m.name,
            monitor_type: m.monitor_type,
            target: m.target,
            interval_sec: m.interval_sec,
            timeout_sec: m.timeout_sec,
            is_active: m.is_active,
        })
        .collect();

    // Ambil seluruh setting konfigurasi
    let settings = {
        let conn = state.db.lock().await;
        let mut stmt = conn
            .prepare("SELECT key, value FROM settings")
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let rows = stmt
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let mut map = HashMap::new();
        for r in rows {
            if let Ok((k, v)) = r {
                map.insert(k, v);
            }
        }
        map
    };

    let now_str = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let filename_date = Local::now().format("%Y-%m-%d_%H-%M").to_string();

    let backup = BackupData {
        version: "1.0".to_string(),
        app: "UptimePulse".to_string(),
        exported_at: now_str,
        monitors: backup_monitors,
        settings,
    };

    let json_bytes = serde_json::to_vec_pretty(&backup)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    let disposition = format!("attachment; filename=\"uptimepulse-backup-{}.json\"", filename_date);
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&disposition).unwrap(),
    );

    Ok((headers, json_bytes).into_response())
}

// GET /api/backup/database - Mengunduh file database SQLite (.db) utuh
pub async fn download_database(
    State(state): State<Arc<BackupControllerState>>,
) -> Result<Response, (StatusCode, String)> {
    let db_path = std::env::var("UPTIME_DB_PATH").unwrap_or_else(|_| "uptime.db".to_string());

    // Flush WAL (Write-Ahead Log) ke file utama SQLite sebelum diunduh
    {
        let conn = state.db.lock().await;
        let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
    }

    let bytes = tokio::fs::read(&db_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Gagal membaca file database: {}", e)))?;

    let filename_date = Local::now().format("%Y-%m-%d").to_string();
    let disposition = format!("attachment; filename=\"uptimepulse-{}.db\"", filename_date);

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&disposition).unwrap(),
    );

    Ok((headers, bytes).into_response())
}

// POST /api/backup/restore - Memulihkan konfigurasi dari file JSON backup
pub async fn restore_json(
    State(state): State<Arc<BackupControllerState>>,
    Json(payload): Json<RestorePayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let backup = payload.backup;
    let mode = payload.mode; // "replace" atau "merge"

    if backup.monitors.is_empty() && backup.settings.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "File backup tidak memuat data yang valid" })),
        ));
    }

    let mut imported_count = 0;
    {
        let mut conn = state.db.lock().await;
        let tx = conn
            .transaction()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))))?;

        // Mode Replace: Bersihkan daftar monitor saat ini (cascade otomatis hapus heartbeats & incidents)
        if mode == "replace" {
            tx.execute("DELETE FROM monitors", [])
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))))?;
        }

        // Pulihkan pengaturan (Telegram, dsb)
        for (k, v) in &backup.settings {
            tx.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![k, v],
            )
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))))?;
        }

        // Pulihkan seluruh monitor
        for m in &backup.monitors {
            let interval = m.interval_sec.max(5);
            let timeout = m.timeout_sec.max(1);
            let is_act = if m.is_active { 1 } else { 0 };

            tx.execute(
                "INSERT INTO monitors (name, monitor_type, target, interval_sec, timeout_sec, is_active, status)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending')",
                params![m.name, m.monitor_type, m.target, interval, timeout, is_act],
            )
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))))?;

            imported_count += 1;
        }

        tx.commit()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))))?;
    }

    Ok(Json(json!({
        "success": true,
        "mode": mode,
        "imported_monitors": imported_count,
        "message": format!("Berhasil memulihkan {} monitor (Mode: {})", imported_count, mode)
    })))
}
