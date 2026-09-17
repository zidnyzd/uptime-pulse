use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde_json::json;
use std::sync::Arc;

use crate::database::DbPool;
use crate::models::{BrandingSettings, TelegramSettings};

pub struct SettingControllerState {
    pub db: DbPool,
}

// GET /api/settings/telegram - Membaca konfigurasi Telegram saat ini
pub async fn get_telegram_settings(
    State(state): State<Arc<SettingControllerState>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    match TelegramSettings::load(&state.db).await {
        Ok(cfg) => Ok(Json(cfg)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

// POST /api/settings/telegram - Menyimpan konfigurasi Telegram
pub async fn save_telegram_settings(
    State(state): State<Arc<SettingControllerState>>,
    Json(payload): Json<TelegramSettings>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    match payload.save(&state.db).await {
        Ok(_) => Ok(Json(json!({ "success": true, "message": "Pengaturan Telegram berhasil disimpan" }))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

// POST /api/settings/telegram/test - Menguji pengiriman pesan ke bot & topik thread
pub async fn test_telegram_notification(
    State(_state): State<Arc<SettingControllerState>>,
    Json(mut payload): Json<TelegramSettings>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    payload.enabled = true;

    // Validasi konfigurasi lebih dulu. Tanpa ini, `send_message` mengembalikan Ok
    // saat konfigurasi kosong (perilaku benar untuk alur alert biasa), sehingga
    // endpoint test akan melaporkan "berhasil" padahal tidak ada pesan terkirim.
    if payload.bot_token.trim().is_empty() || payload.chat_id.trim().is_empty() {
        let missing = if payload.bot_token.trim().is_empty() {
            "Bot Token"
        } else {
            "Chat ID"
        };
        crate::alert_log::log(
            "ERROR",
            &format!("Tes Telegram gagal: {} belum diisi", missing),
        );
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "error": format!("{} belum diisi — pesan tidak dikirim.", missing)
            })),
        ));
    }

    let thread_info = payload.thread_id.map(|t| format!(" • Topic #{}", t)).unwrap_or_default();
    let test_msg = format!(
        "🔔 <b>[TEST] UptimePulse Telegram Alert</b>{}\n\n\
         Koneksi notifikasi Telegram berhasil terhubung!\n\
         Pesan ini dikirim untuk menguji Bot Token, Chat ID, dan Topik Thread.",
        thread_info
    );

    match payload.send_message(&test_msg).await {
        Ok(_) => Ok(Json(json!({ "success": true, "message": "Pesan tes berhasil dikirim ke Telegram!" }))),
        Err(err) => Err((StatusCode::BAD_REQUEST, Json(json!({ "success": false, "error": err })))),
    }
}

// GET /api/settings/branding - Membaca konfigurasi branding halaman publik
pub async fn get_branding_settings(
    State(state): State<Arc<SettingControllerState>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    match BrandingSettings::load(&state.db).await {
        Ok(cfg) => Ok(Json(cfg)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

// POST /api/settings/branding - Menyimpan konfigurasi branding halaman publik
pub async fn save_branding_settings(
    State(state): State<Arc<SettingControllerState>>,
    Json(payload): Json<BrandingSettings>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    match payload.save(&state.db).await {
        Ok(_) => Ok(Json(json!({ "success": true, "message": "Pengaturan branding berhasil disimpan" }))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}
