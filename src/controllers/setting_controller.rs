use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde_json::json;
use std::sync::Arc;

use crate::database::DbPool;
use crate::models::{BrandingSettings, TelegramSettings, TimezoneSnapshot};
pub struct SettingControllerState {
    pub db: DbPool,
    /// Cache hasil pengecekan versi terbaru. Pengecekan memanggil GitHub API
    /// yang punya batas laju per IP, jadi hasilnya disimpan sementara agar
    /// membuka konsol admin berulang kali tidak menghabiskan kuota.
    pub version_cache: tokio::sync::Mutex<Option<VersionCheckCache>>,
}

/// Hasil pengecekan rilis terbaru beserta waktu pengambilannya.
#[derive(Debug, Clone)]
pub struct VersionCheckCache {
    pub checked_at: std::time::Instant,
    pub latest: Option<String>,
    pub error: Option<String>,
}

/// Versi binary yang sedang berjalan, dibaca saat kompilasi.
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Selang minimum antar pengecekan rilis terbaru.
const VERSION_CHECK_TTL_SECS: u64 = 6 * 3600;

/// Membandingkan dua versi semver sederhana (`MAJOR.MINOR.PATCH`).
/// Mengembalikan true bila `candidate` lebih baru dari `current`.
///
/// Prefiks `v` dan sufiks pra-rilis (`-rc1`) diabaikan; tujuannya hanya
/// memberi tahu "ada versi lebih baru", bukan memeringkatkan pra-rilis.
pub fn is_newer_version(candidate: &str, current: &str) -> bool {
    fn parse(v: &str) -> (u64, u64, u64) {
        let core = v.trim().trim_start_matches('v');
        let core = core.split(['-', '+']).next().unwrap_or(core);
        let mut parts = core.split('.').map(|p| p.trim().parse::<u64>().unwrap_or(0));
        (
            parts.next().unwrap_or(0),
            parts.next().unwrap_or(0),
            parts.next().unwrap_or(0),
        )
    }
    parse(candidate) > parse(current)
}

// GET /api/system/version - Versi yang berjalan + info rilis terbaru
pub async fn get_version_info(
    State(state): State<Arc<SettingControllerState>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let cached = {
        let guard = state.version_cache.lock().await;
        guard.as_ref().and_then(|c| {
            if c.checked_at.elapsed().as_secs() < VERSION_CHECK_TTL_SECS {
                Some((c.latest.clone(), c.error.clone()))
            } else {
                None
            }
        })
    };

    let (latest, check_error) = match cached {
        Some(v) => v,
        None => {
            let (latest, error) = fetch_latest_release().await;
            let mut guard = state.version_cache.lock().await;
            *guard = Some(VersionCheckCache {
                checked_at: std::time::Instant::now(),
                latest: latest.clone(),
                error: error.clone(),
            });
            (latest, error)
        }
    };

    let update_available = latest
        .as_deref()
        .map(|l| is_newer_version(l, CURRENT_VERSION))
        .unwrap_or(false);

    Ok(Json(json!({
        "current": CURRENT_VERSION,
        "latest": latest,
        "update_available": update_available,
        // Kegagalan pengecekan bukan error endpoint: versi berjalan tetap
        // dilaporkan supaya sidebar tidak kosong saat perangkat offline.
        "check_error": check_error,
    })))
}

/// Mengambil tag rilis terbaru dari GitHub API.
///
/// Best-effort: perangkat di balik jaringan ketat atau tanpa internet akan
/// gagal di sini, dan itu bukan kondisi fatal — versi berjalan tetap
/// ditampilkan tanpa penanda pembaruan.
async fn fetch_latest_release() -> (Option<String>, Option<String>) {
    let url = "https://api.github.com/repos/zidnyzd/uptime-pulse/releases/latest";
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => return (None, Some(format!("Gagal membuat HTTP client: {}", e))),
    };

    let resp = match client
        .get(url)
        .header("User-Agent", crate::prober::DEFAULT_USER_AGENT)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return (None, Some(format!("Gagal menghubungi GitHub: {}", e))),
    };

    if !resp.status().is_success() {
        return (
            None,
            Some(format!("GitHub membalas HTTP {}", resp.status().as_u16())),
        );
    }

    match resp.json::<serde_json::Value>().await {
        Ok(body) => {
            let tag = body
                .get("tag_name")
                .and_then(|v| v.as_str())
                .map(|s| s.trim_start_matches('v').to_string());
            match tag {
                Some(t) if !t.is_empty() => (Some(t), None),
                _ => (None, Some("Rilis terbaru tidak memuat tag".to_string())),
            }
        }
        Err(e) => (None, Some(format!("Respons GitHub tidak terbaca: {}", e))),
    }
}

// POST /api/settings/branding/timezone-sync - Menyimpan offset UTC & singkatan

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

// POST /api/settings/branding/timezone-sync - Menyimpan offset UTC & singkatan
// zona waktu yang dihitung browser.
//
// Kenapa dari browser: perhitungan stempel waktu yang benar butuh basis data
// zona waktu (tzdata), dan yang pasti lengkap hanya ada di sisi klien. Binary
// ini tidak membundel tzdata, sedangkan tzdata perangkat (OpenWrt) sering
// tidak lengkap — pada pengujian, `TZ=America/New_York` di perangkat justru
// jatuh ke UTC. Nilai ini dipakai backend untuk mengoreksi stempel waktu pada
// notifikasi Telegram agar ikut zona waktu aplikasi, bukan zona OS.
pub async fn sync_timezone_snapshot(
    State(state): State<Arc<SettingControllerState>>,
    Json(payload): Json<TimezoneSnapshot>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    match payload.save(&state.db).await {
        Ok(_) => Ok(Json(json!({ "success": true }))),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_newer_patch_minor_and_major() {
        assert!(is_newer_version("0.1.6", "0.1.5"));
        assert!(is_newer_version("0.2.0", "0.1.5"));
        assert!(is_newer_version("1.0.0", "0.1.5"));
    }

    #[test]
    fn rejects_same_or_older() {
        assert!(!is_newer_version("0.1.5", "0.1.5"));
        assert!(!is_newer_version("0.1.4", "0.1.5"));
        assert!(!is_newer_version("0.0.9", "0.1.5"));
    }

    #[test]
    fn tolerates_v_prefix_and_prerelease_suffix() {
        assert!(is_newer_version("v0.1.6", "0.1.5"));
        assert!(!is_newer_version("v0.1.5", "0.1.5"));
        // Pra-rilis untuk versi yang sama tidak dianggap lebih baru.
        assert!(!is_newer_version("0.1.5-rc1", "0.1.5"));
    }

    #[test]
    fn compares_numerically_not_lexically() {
        // Perbandingan teks akan salah di sini: "0.1.10" < "0.1.9" secara leksikal.
        assert!(is_newer_version("0.1.10", "0.1.9"));
        assert!(is_newer_version("0.10.0", "0.9.0"));
    }

    #[test]
    fn malformed_input_is_treated_as_zero() {
        assert!(!is_newer_version("", "0.1.5"));
        assert!(!is_newer_version("bukan-versi", "0.1.5"));
        assert!(is_newer_version("0.1.5", ""));
    }
}
