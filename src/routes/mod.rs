pub mod api;
pub mod web;

use axum::Router;
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

    // Gabungkan routing API dan Web
    api_router.merge(web_router)
}
