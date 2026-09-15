use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{Path, State},
    http::{header, StatusCode, Uri},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{delete, get, post},
    Json, Router,
};
use rust_embed::RustEmbed;
use serde_json::json;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::db::{self, DbPool};
use crate::engine::{run_probe_and_record, EventSender};
use crate::models::CreateMonitorInput;

// Macro RustEmbed memasukkan semua file di dalam folder public/ langsung ke binary saat kompilasi
#[derive(RustEmbed)]
#[folder = "public/"]
struct Assets;

// State global yang dibagikan ke semua handler endpoint Axum
pub struct AppState {
    pub db: DbPool,
    pub event_tx: EventSender,
}

// Konfigurasi routing REST API, SSE streaming, dan static web assets
pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        // Endpoint data status publik (tanpa mengekspos target sensitif)
        .route("/api/public/summary", get(get_public_summary_handler))
        // Endpoint manajemen admin
        .route("/api/monitors", get(list_monitors).post(create_monitor))
        .route("/api/monitors/{id}", get(get_monitor_detail))
        .route("/api/monitors/{id}", delete(delete_monitor))
        .route("/api/monitors/{id}/pause", post(toggle_pause))
        .route("/api/monitors/{id}/check", post(trigger_check))
        // Stream event real-time ke browser
        .route("/api/events", get(sse_handler))
        // Halaman panel admin
        .route("/admin", get(admin_page_handler))
        // Fallback untuk melayani file static embedded (index.html, css, js)
        .fallback(static_handler)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

// Menyajikan ringkasan status publik untuk tampilan user utama
async fn get_public_summary_handler(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    match db::get_public_summary(&state.db).await {
        Ok(summary) => Ok(Json(summary)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

// Menyajikan halaman admin.html dari memori binary
async fn admin_page_handler() -> Response {
    match Assets::get("admin.html") {
        Some(content) => {
            ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], content.data).into_response()
        }
        None => (StatusCode::NOT_FOUND, "Admin page not found").into_response(),
    }
}

// Mengambil seluruh daftar target monitor yang ada di database
async fn list_monitors(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    match db::list_monitors(&state.db).await {
        Ok(monitors) => Ok(Json(monitors)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

// Mengambil detail satu monitor beserta riwayat latency sparkline
async fn get_monitor_detail(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    match db::get_monitor_detail(&state.db, id).await {
        Ok(Some(detail)) => Ok(Json(detail)),
        Ok(None) => Err((StatusCode::NOT_FOUND, "Monitor not found".to_string())),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

// Menambahkan target baru dan langsung memicu probe pertama agar status segera terisi
async fn create_monitor(
    State(state): State<Arc<AppState>>,
    Json(input): Json<CreateMonitorInput>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    if input.name.trim().is_empty() || input.target.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Name and target are required".to_string()));
    }

    let input_clone = input.clone();
    match db::create_monitor(&state.db, input).await {
        Ok(id) => {
            // Trigger probe pertama segera di background task terpisah
            let db_clone = state.db.clone();
            let tx_clone = state.event_tx.clone();
            tokio::spawn(async move {
                run_probe_and_record(
                    &db_clone,
                    &tx_clone,
                    id,
                    &input_clone.monitor_type,
                    &input_clone.target,
                    input_clone.timeout_sec,
                )
                .await;
            });

            Ok((StatusCode::CREATED, Json(json!({ "id": id, "status": "created" }))))
        }
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

// Menghapus target monitor dari database
async fn delete_monitor(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    match db::delete_monitor(&state.db, id).await {
        Ok(true) => Ok(Json(json!({ "success": true }))),
        Ok(false) => Err((StatusCode::NOT_FOUND, "Monitor not found".to_string())),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

// Mengubah status aktif / pause pada monitor
async fn toggle_pause(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    match db::toggle_pause_monitor(&state.db, id).await {
        Ok(true) => Ok(Json(json!({ "success": true }))),
        Ok(false) => Err((StatusCode::NOT_FOUND, "Monitor not found".to_string())),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

// Memicu manual re-check seketika tanpa menunggu siklus interval
async fn trigger_check(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    match db::get_monitor_detail(&state.db, id).await {
        Ok(Some(detail)) => {
            let m = detail.monitor;
            let result = run_probe_and_record(
                &state.db,
                &state.event_tx,
                m.id,
                &m.monitor_type,
                &m.target,
                m.timeout_sec,
            )
            .await;
            Ok(Json(json!(result)))
        }
        Ok(None) => Err((StatusCode::NOT_FOUND, "Monitor not found".to_string())),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

// Handler Server-Sent Events (SSE) yang mengalirkan pembaruan status ke browser secara searah
async fn sse_handler(
    State(state): State<Arc<AppState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    // Masing-masing koneksi HTTP yang tersambung membuat subscriber baru ke broadcast channel
    let rx = state.event_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|msg| match msg {
        Ok(event) => {
            let json_str = serde_json::to_string(&event).unwrap_or_default();
            Some(Ok(Event::default().data(json_str)))
        }
        Err(_) => None,
    });

    // KeepAlive 15 detik untuk mencegah koneksi diputus oleh reverse proxy / gateway
    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

// Melayani file statis dari memori binary (HTML, CSS, JS) dengan auto mime detection
async fn static_handler(uri: Uri) -> Response {
    let mut path = uri.path().trim_start_matches('/');
    if path.is_empty() {
        path = "index.html";
    }

    match Assets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
        }
        None => {
            // SPA fallback: jika path tidak cocok dengan file fisik, arahkan ke index.html
            match Assets::get("index.html") {
                Some(content) => {
                    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], content.data)
                        .into_response()
                }
                None => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
            }
        }
    }
}
