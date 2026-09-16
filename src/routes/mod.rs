pub mod api;
pub mod web;

use axum::{middleware, Router};
use crate::database::DbPool;
use crate::engine::EventSender;

pub fn create_router(
    db: DbPool,
    event_tx: EventSender,
    db_path: String,
    retention_days: u32,
) -> Router {
    let api_router = api::build_api_router(db, event_tx, db_path, retention_days);
    let web_router = web::build_web_router();

    // Gabungkan routing API dan Web + security headers global
    api_router
        .merge(web_router)
        .layer(middleware::from_fn(
            crate::middlewares::security::security_headers,
        ))
}
