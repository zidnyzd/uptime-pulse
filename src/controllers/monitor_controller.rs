use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use crate::database::DbPool;
use crate::engine::{run_probe_and_record_cfg, EventSender};
use crate::models::{CreateMonitorInput, Heartbeat, Monitor};
use crate::prober::HttpRequestConfig;

pub struct MonitorControllerState {
    pub db: DbPool,
    pub event_tx: EventSender,
}

// GET /api/monitors - Mengambil semua monitor (Admin only)
pub async fn index(
    State(state): State<Arc<MonitorControllerState>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    match Monitor::all(&state.db).await {
        Ok(list) => Ok(Json(list)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

// GET /api/incidents - Riwayat insiden untuk konsol admin (SEMUA monitor,
// termasuk yang privat/paused — beda dengan /api/public/summary yang terfilter)
pub async fn incidents(
    State(state): State<Arc<MonitorControllerState>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    match crate::models::Incident::list_recent(&state.db, 100, false).await {
        Ok(list) => Ok(Json(list)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

// GET /api/monitors/{id} - Mengambil detail monitor + riwayat heartbeat 24h
pub async fn show(
    State(state): State<Arc<MonitorControllerState>>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    match Heartbeat::find_detail(&state.db, id).await {
        Ok(Some(detail)) => Ok(Json(detail)),
        Ok(None) => Err((StatusCode::NOT_FOUND, "Monitor tidak ditemukan".to_string())),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

// POST /api/monitors - Menambahkan monitor baru dan memicu probe awal seketika
pub async fn store(
    State(state): State<Arc<MonitorControllerState>>,
    Json(input): Json<CreateMonitorInput>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    if input.name.trim().is_empty() || input.target.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Nama dan target wajib diisi".to_string()));
    }

    let input_clone = input.clone();
    match Monitor::create(&state.db, &input).await {
        Ok(id) => {
            // Trigger probe pertama di background (pakai method/headers/body yang dikonfigurasi)
            let db_clone = state.db.clone();
            let tx_clone = state.event_tx.clone();
            let http_cfg = HttpRequestConfig {
                method: input_clone.method.clone(),
                headers: input_clone.headers.clone(),
                body: input_clone.body.clone(),
                json_path: input_clone.json_path.clone(),
                expected_value: input_clone.expected_value.clone(),
                json_operator: input_clone.json_operator.clone(),
            };
            let m_type = input_clone.monitor_type.clone();
            let m_target = input_clone.target.clone();
            let m_timeout = input_clone.timeout_sec;
            tokio::spawn(async move {
                run_probe_and_record_cfg(
                    &db_clone,
                    &tx_clone,
                    id,
                    &m_type,
                    &m_target,
                    m_timeout,
                    &http_cfg,
                )
                .await;
            });

            Ok((StatusCode::CREATED, Json(json!({ "id": id, "status": "created" }))))
        }
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

// PUT /api/monitors/{id} - Memperbarui konfigurasi monitor yang sudah ada
pub async fn update(
    State(state): State<Arc<MonitorControllerState>>,
    Path(id): Path<i64>,
    Json(input): Json<crate::models::monitor::UpdateMonitorInput>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    if input.name.trim().is_empty() || input.target.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Nama dan target wajib diisi".to_string()));
    }

    match Monitor::update(&state.db, id, &input).await {
        Ok(true) => {
            // Trigger check ulang dengan parameter baru (termasuk method/headers/body)
            let db_clone = state.db.clone();
            let tx_clone = state.event_tx.clone();
            let input_clone = input.clone();
            let http_cfg = HttpRequestConfig {
                method: input_clone.method.clone(),
                headers: input_clone.headers.clone(),
                body: input_clone.body.clone(),
                json_path: input_clone.json_path.clone(),
                expected_value: input_clone.expected_value.clone(),
                json_operator: input_clone.json_operator.clone(),
            };
            let m_type = input_clone.monitor_type.clone();
            let m_target = input_clone.target.clone();
            let m_timeout = input_clone.timeout_sec;
            tokio::spawn(async move {
                run_probe_and_record_cfg(
                    &db_clone,
                    &tx_clone,
                    id,
                    &m_type,
                    &m_target,
                    m_timeout,
                    &http_cfg,
                )
                .await;
            });

            Ok(Json(json!({ "success": true, "message": "Monitor berhasil diperbarui" })))
        }
        Ok(false) => Err((StatusCode::NOT_FOUND, "Monitor tidak ditemukan".to_string())),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

// DELETE /api/monitors/{id} - Menghapus monitor beserta rekaman lognya
pub async fn destroy(
    State(state): State<Arc<MonitorControllerState>>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    match Monitor::delete(&state.db, id).await {
        Ok(true) => Ok(Json(json!({ "success": true, "message": "Monitor berhasil dihapus" }))),
        Ok(false) => Err((StatusCode::NOT_FOUND, "Monitor tidak ditemukan".to_string())),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

// POST /api/monitors/{id}/pause - Mengubah status jeda / aktifkan kembali
pub async fn toggle_pause(
    State(state): State<Arc<MonitorControllerState>>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    match Monitor::toggle_pause(&state.db, id).await {
        Ok(true) => Ok(Json(json!({ "success": true }))),
        Ok(false) => Err((StatusCode::NOT_FOUND, "Monitor tidak ditemukan".to_string())),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

// POST /api/monitors/{id}/check - Memicu probe seketika secara manual
pub async fn check(
    State(state): State<Arc<MonitorControllerState>>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    match Monitor::find(&state.db, id).await {
        Ok(Some(m)) => {
            let http_cfg = HttpRequestConfig {
                method: m.method.clone(),
                headers: m.headers.clone(),
                body: m.body.clone(),
                json_path: m.json_path.clone(),
                expected_value: m.expected_value.clone(),
                json_operator: m.json_operator.clone(),
            };
            let result = run_probe_and_record_cfg(
                &state.db,
                &state.event_tx,
                m.id,
                &m.monitor_type,
                &m.target,
                m.timeout_sec,
                &http_cfg,
            )
            .await;
            Ok(Json(json!(result)))
        }
        Ok(None) => Err((StatusCode::NOT_FOUND, "Monitor tidak ditemukan".to_string())),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

// POST /api/monitors/{id}/reset - Mereset seluruh statistik monitor dari awal
// (hapus heartbeat mentah + agregat harian + insiden, nolkan gagal beruntun,
// status kembali 'pending')
pub async fn reset(
    State(state): State<Arc<MonitorControllerState>>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    match Monitor::reset_stats(&state.db, id).await {
        Ok(Some((hb, daily, inc))) => Ok(Json(json!({
            "success": true,
            "message": format!("Statistik direset: {} heartbeat, {} baris agregat harian, dan {} insiden dihapus", hb, daily, inc),
            "heartbeats_deleted": hb,
            "daily_rows_deleted": daily,
            "incidents_deleted": inc,
        }))),
        Ok(None) => Err((StatusCode::NOT_FOUND, "Monitor tidak ditemukan".to_string())),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

#[derive(Deserialize)]
pub struct ReorderPayload {
    pub ids: Vec<i64>,
}

// POST /api/monitors/reorder - Mengubah susunan urutan monitor secara batch
pub async fn reorder(
    State(state): State<Arc<MonitorControllerState>>,
    Json(payload): Json<ReorderPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    if payload.ids.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Daftar ID tidak boleh kosong" })),
        ));
    }
    match Monitor::reorder(&state.db, &payload.ids).await {
        Ok(_) => Ok(Json(json!({ "success": true, "message": "Urutan monitor berhasil disimpan" }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )),
    }
}
