// Deklarasi modul arsitektur MVC (Model-View-Controller)
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
    // Setup structured logging; format log konsol dan filter trace
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "uptime_pulse=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Inisialisasi Database SQLite
    let db_path = std::env::var("UPTIME_DB_PATH").unwrap_or_else(|_| "uptime.db".to_string());
    info!("Initializing database at '{}'...", db_path);
    let db = database::init_db(&db_path)?;

    // Pastikan user admin default tersedia (dapat di-override via env ADMIN_PASSWORD)
    let default_admin_pass = std::env::var("ADMIN_PASSWORD").unwrap_or_else(|_| "admin".to_string());
    models::User::ensure_admin_exists(&db, &default_admin_pass).await?;
    info!("Admin account verified (username: 'admin').");

    // Channel broadcast Tokio untuk pengiriman event probe secara real-time ke web SSE
    let (event_tx, _) = broadcast::channel(100);

    // Menjalankan scheduler loop prober di background task Tokio
    engine::start_scheduler(db.clone(), event_tx.clone());

    // Membangun routing MVC aplikasi (API publik, Admin routes dengan middleware auth, Web views)
    let app = routes::create_router(db, event_tx);

    // Konfigurasi binding port dan host
    let host = std::env::var("UPTIME_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port: u16 = std::env::var("UPTIME_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3001);

    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
    info!("🚀 UptimePulse running at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
