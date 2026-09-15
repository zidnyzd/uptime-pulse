mod db;
mod engine;
mod models;
mod prober;
mod web;

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "uptime_pulse=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let db_path = std::env::var("UPTIME_DB_PATH").unwrap_or_else(|_| "uptime.db".to_string());
    info!("Initializing database at '{}'...", db_path);
    let db = db::init_db(&db_path)?;

    let (event_tx, _) = broadcast::channel(100);

    engine::start_scheduler(db.clone(), event_tx.clone());

    let app_state = Arc::new(web::AppState {
        db,
        event_tx,
    });
    let app = web::create_router(app_state);

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
