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

#[derive(RustEmbed)]
#[folder = "public/"]
struct Assets;

pub struct AppState {
    pub db: DbPool,
    pub event_tx: EventSender,
}

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        // API routes
        .route("/api/public/summary", get(get_public_summary_handler))
        .route("/api/monitors", get(list_monitors).post(create_monitor))
        .route("/api/monitors/{id}", get(get_monitor_detail))
        .route("/api/monitors/{id}", delete(delete_monitor))
        .route("/api/monitors/{id}/pause", post(toggle_pause))
        .route("/api/monitors/{id}/check", post(trigger_check))
        .route("/api/events", get(sse_handler))
        .route("/admin", get(admin_page_handler))
        // Static assets fallback
        .fallback(static_handler)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn get_public_summary_handler(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    match db::get_public_summary(&state.db).await {
        Ok(summary) => Ok(Json(summary)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

async fn admin_page_handler() -> Response {
    match Assets::get("admin.html") {
        Some(content) => {
            ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], content.data).into_response()
        }
        None => (StatusCode::NOT_FOUND, "Admin page not found").into_response(),
    }
}

async fn list_monitors(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    match db::list_monitors(&state.db).await {
        Ok(monitors) => Ok(Json(monitors)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

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
            // Trigger initial probe immediately in background
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

async fn sse_handler(
    State(state): State<Arc<AppState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = state.event_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|msg| match msg {
        Ok(event) => {
            let json_str = serde_json::to_string(&event).unwrap_or_default();
            Some(Ok(Event::default().data(json_str)))
        }
        Err(_) => None,
    });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

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
            // SPA fallback: return index.html if file not found
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
