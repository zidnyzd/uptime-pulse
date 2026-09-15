use axum::{
    extract::State,
    http::{header, StatusCode, Uri},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    Json,
};
use rust_embed::RustEmbed;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use crate::database::DbPool;
use crate::engine::EventSender;
use crate::models::Heartbeat;

// Macro RustEmbed menyematkan isi folder public/ ke dalam binary
#[derive(RustEmbed)]
#[folder = "public/"]
pub struct Assets;

pub struct PublicControllerState {
    pub db: DbPool,
    pub event_tx: EventSender,
}

// GET /api/public/summary - Menyajikan agregasi metrik untuk halaman status publik
pub async fn summary(
    State(state): State<Arc<PublicControllerState>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    match Heartbeat::get_public_summary(&state.db).await {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

// GET /api/events - Stream Server-Sent Events (SSE) realtime
pub async fn events(
    State(state): State<Arc<PublicControllerState>>,
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

// GET /admin - Melayani file admin.html
pub async fn admin_page() -> Response {
    match Assets::get("admin.html") {
        Some(content) => {
            ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], content.data).into_response()
        }
        None => (StatusCode::NOT_FOUND, "Admin page not found").into_response(),
    }
}

// Fallback untuk aset statis (HTML, CSS, JS) dengan SPA fallback ke index.html
pub async fn static_files(uri: Uri) -> Response {
    let mut path = uri.path().trim_start_matches('/');
    if path.is_empty() {
        path = "index.html";
    }

    match Assets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
        }
        None => match Assets::get("index.html") {
            Some(content) => {
                ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], content.data).into_response()
            }
            None => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
        },
    }
}
