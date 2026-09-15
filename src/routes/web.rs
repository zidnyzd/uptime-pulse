use axum::{routing::get, Router};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::controllers::public_controller;

pub fn build_web_router() -> Router {
    Router::new()
        // Halaman antarmuka admin
        .route("/admin", get(public_controller::admin_page))
        // Static file assets fallback (index.html, JS, CSS)
        .fallback(public_controller::static_files)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}
