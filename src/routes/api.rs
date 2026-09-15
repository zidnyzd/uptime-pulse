use axum::{
    middleware::from_fn_with_state,
    routing::{get, post},
    Router,
};
use std::sync::Arc;

use crate::controllers::{
    auth_controller::{self, AuthControllerState},
    monitor_controller::{self, MonitorControllerState},
    public_controller::{self, PublicControllerState},
};
use crate::database::DbPool;
use crate::engine::EventSender;
use crate::middlewares::auth::{require_admin_auth, AuthMiddlewareState};

pub fn build_api_router(db: DbPool, event_tx: EventSender) -> Router {
    let auth_controller_state = Arc::new(AuthControllerState { db: db.clone() });
    let monitor_controller_state = Arc::new(MonitorControllerState {
        db: db.clone(),
        event_tx: event_tx.clone(),
    });
    let public_controller_state = Arc::new(PublicControllerState {
        db: db.clone(),
        event_tx: event_tx.clone(),
    });
    let auth_mw_state = Arc::new(AuthMiddlewareState { db: db.clone() });

    // Rute Publik untuk Auth
    let auth_routes = Router::new()
        .route("/api/auth/login", post(auth_controller::login))
        .route("/api/auth/logout", post(auth_controller::logout))
        .route("/api/auth/me", get(auth_controller::me))
        .with_state(auth_controller_state);

    // Rute Publik untuk Status & Event Stream
    let public_routes = Router::new()
        .route("/api/public/summary", get(public_controller::summary))
        .route("/api/events", get(public_controller::events))
        .with_state(public_controller_state);

    // Rute Terproteksi Khusus Admin (dijaga oleh middleware require_admin_auth)
    let admin_monitor_routes = Router::new()
        .route(
            "/api/monitors",
            get(monitor_controller::index).post(monitor_controller::store),
        )
        .route(
            "/api/monitors/{id}",
            get(monitor_controller::show).delete(monitor_controller::destroy),
        )
        .route("/api/monitors/{id}/pause", post(monitor_controller::toggle_pause))
        .route("/api/monitors/{id}/check", post(monitor_controller::check))
        .with_state(monitor_controller_state)
        .route_layer(from_fn_with_state(auth_mw_state, require_admin_auth));

    Router::new()
        .merge(auth_routes)
        .merge(public_routes)
        .merge(admin_monitor_routes)
}
