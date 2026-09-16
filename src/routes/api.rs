use axum::{
    middleware::from_fn_with_state,
    routing::{get, post},
    Router,
};
use std::sync::Arc;

use crate::controllers::{
    auth_controller::{self, AuthControllerState},
    backup_controller::{self, BackupControllerState},
    monitor_controller::{self, MonitorControllerState},
    public_controller::{self, PublicControllerState},
    setting_controller::{self, SettingControllerState},
};
use crate::database::DbPool;
use crate::engine::EventSender;
use crate::middlewares::auth::{require_admin_auth, AuthMiddlewareState};
use crate::middlewares::rate_limit::LoginRateLimiter;

pub fn build_api_router(
    db: DbPool,
    event_tx: EventSender,
    db_path: String,
    retention_days: u32,
) -> Router {
    let auth_controller_state = Arc::new(AuthControllerState {
        db: db.clone(),
        login_limiter: Arc::new(LoginRateLimiter::new()),
    });
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
        .with_state(auth_controller_state.clone());

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
            get(monitor_controller::show)
                .put(monitor_controller::update)
                .delete(monitor_controller::destroy),
        )
        .route("/api/monitors/{id}/pause", post(monitor_controller::toggle_pause))
        .route("/api/monitors/{id}/check", post(monitor_controller::check))
        .with_state(monitor_controller_state)
        .route_layer(from_fn_with_state(auth_mw_state.clone(), require_admin_auth));

    let setting_controller_state = Arc::new(SettingControllerState { db: db.clone() });
    let admin_setting_routes = Router::new()
        .route(
            "/api/settings/telegram",
            get(setting_controller::get_telegram_settings)
                .post(setting_controller::save_telegram_settings),
        )
        .route(
            "/api/settings/telegram/test",
            post(setting_controller::test_telegram_notification),
        )
        .route(
            "/api/settings/branding",
            get(setting_controller::get_branding_settings)
                .post(setting_controller::save_branding_settings),
        )
        .with_state(setting_controller_state)
        .route_layer(from_fn_with_state(auth_mw_state.clone(), require_admin_auth));

    let admin_auth_routes = Router::new()
        .route(
            "/api/auth/change-password",
            post(auth_controller::change_password),
        )
        .with_state(auth_controller_state)
        .route_layer(from_fn_with_state(auth_mw_state.clone(), require_admin_auth));

    let backup_controller_state = Arc::new(BackupControllerState {
        db: db.clone(),
        db_path,
        retention_days,
    });
    let admin_backup_routes = Router::new()
        .route("/api/backup/export", get(backup_controller::export_json))
        .route(
            "/api/backup/database",
            get(backup_controller::download_database),
        )
        .route("/api/backup/restore", post(backup_controller::restore_json))
        .route("/api/backup/stats", get(backup_controller::db_stats))
        .route("/api/backup/prune", post(backup_controller::prune_db))
        .with_state(backup_controller_state)
        .route_layer(from_fn_with_state(auth_mw_state, require_admin_auth));

    Router::new()
        .merge(auth_routes)
        .merge(public_routes)
        .merge(admin_monitor_routes)
        .merge(admin_setting_routes)
        .merge(admin_backup_routes)
        .merge(admin_auth_routes)
}
