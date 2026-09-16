// Deklarasi modul arsitektur MVC (Model-View-Controller)
mod config;
mod database;
mod engine;
mod models;
mod prober;
mod middlewares;
mod controllers;
mod routes;

use std::net::SocketAddr;
use tokio::sync::broadcast;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse runtime configuration (CLI flags & Env Variables)
    let app_config = match config::AppConfig::parse() {
        Ok(c) => c,
        Err(err) => {
            eprintln!("Configuration error: {}", err);
            std::process::exit(1);
        }
    };

    // Setup structured logging; format log konsol dan filter trace
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "uptime_pulse=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!(
        "Config loaded: Host={}, Port={}, DB='{}', Retention={} days",
        app_config.host, app_config.port, app_config.db_path, app_config.retention_days
    );

    // Inisialisasi Database SQLite
    info!("Initializing database at '{}'...", app_config.db_path);
    let db = database::init_db(&app_config.db_path)?;

    // Pastikan user admin default tersedia (dapat di-override via flag --password atau env ADMIN_PASSWORD)
    let default_admin_pass = app_config
        .admin_password
        .unwrap_or_else(|| "admin".to_string());
    models::User::ensure_admin_exists(&db, &default_admin_pass).await?;
    info!("Admin account verified (username: 'admin').");

    // Channel broadcast Tokio untuk pengiriman event probe secara real-time ke web SSE
    let (event_tx, _) = broadcast::channel(100);

    // Menjalankan scheduler prober & auto-pruning loop di background task Tokio
    engine::start_scheduler(db.clone(), event_tx.clone(), app_config.retention_days);

    // Membangun routing MVC aplikasi (API publik, Admin routes dengan middleware auth, Web views)
    let app = routes::create_router(
        db,
        event_tx,
        app_config.db_path.clone(),
        app_config.retention_days,
    );

    // Konfigurasi binding port dan host
    let addr: SocketAddr = format!("{}:{}", app_config.host, app_config.port).parse()?;
    info!("🚀 UptimePulse running at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}
