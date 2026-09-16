use axum::{routing::get, Router};
use tower_http::trace::TraceLayer;

use crate::controllers::public_controller;

pub fn build_web_router() -> Router {
    Router::new()
        // Halaman antarmuka admin
        .route("/admin", get(public_controller::admin_page))
        // Static file assets fallback (index.html, JS, CSS)
        .fallback(public_controller::static_files)
        // Tanpa CORS: frontend disajikan same-origin dari server ini,
        // jadi tidak ada kebutuhan cross-origin. CORS permissive sebelumnya
        // membuka API admin ke situs asing bila token bocor via XSS.
        .layer(TraceLayer::new_for_http())
}
