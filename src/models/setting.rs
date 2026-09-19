use chrono::{Duration, Local, NaiveDateTime};
use rusqlite::{params, OptionalExtension, Result};
use serde::{Deserialize, Serialize};
use tracing::{error, info};
use crate::database::DbPool;

/// Menggeser stempel waktu lokal-OS yang tersimpan di database agar sesuai
/// dengan zona waktu yang dipilih di aplikasi.
///
/// Kenapa perlu digeser: seluruh timestamp disimpan sebagai teks hasil
/// `datetime('now', 'localtime')`, yaitu waktu lokal **sistem operasi**. Zona
/// waktu aplikasi (`branding_timezone`) belum tentu sama dengan zona OS — pada
/// instalasi dengan OS UTC sementara aplikasi diset WIB, notifikasi akan
/// menampilkan waktu 7 jam lebih awal bila tidak dikoreksi.
///
/// Offset zona aplikasi dikirim oleh frontend, karena basis data zona waktu
/// lengkap hanya tersedia di browser (`Intl.DateTimeFormat`); binary ini tidak
/// membundel tzdata dan tzdata perangkat (OpenWrt) sering tidak lengkap.
/// Offset OS dibaca dari `chrono::Local`, sehingga perhitungan tetap benar
/// ketika perangkat memang sudah dikonfigurasi ke zona yang sama (delta = 0).
pub fn shift_local_timestamp(stored_local: &str, app_utc_offset_minutes: i32) -> String {
    let os_offset_minutes = Local::now().offset().local_minus_utc() / 60;
    shift_with_os_offset(stored_local, app_utc_offset_minutes, os_offset_minutes)
}

/// Inti aritmetika pergeseran, dengan offset OS sebagai parameter eksplisit.
///
/// Dipisah dari `shift_local_timestamp` supaya bisa diuji secara deterministik:
/// nilai nyata `Local::now()` bergantung pada zona mesin penguji, sehingga
/// asersi terhadap angka absolut mustahil dilakukan pada fungsi gabungan.
pub fn shift_with_os_offset(
    stored_local: &str,
    app_utc_offset_minutes: i32,
    os_utc_offset_minutes: i32,
) -> String {
    let raw = stored_local.trim();
    let parsed = NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S")
        .or_else(|_| NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S"));

    let dt = match parsed {
        Ok(v) => v,
        // Format tak dikenal: biarkan apa adanya supaya alert tetap terkirim.
        Err(_) => return stored_local.to_string(),
    };

    let delta_minutes = i64::from(app_utc_offset_minutes - os_utc_offset_minutes);

    (dt + Duration::minutes(delta_minutes))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

/// Cuplikan zona waktu aplikasi (offset UTC + singkatan) yang dihitung browser.
///
/// Offset disimpan sebagai menit agar bebas dari parsing string, dan singkatan
/// disimpan terpisah karena peta singkatan yang akurat (`WIB`, `WITA`, `EST`)
/// hanya dimiliki sisi klien. Keduanya adalah data turunan dari
/// `branding_timezone`, bukan preferensi terpisah — jadi menimpanya berulang
/// kali aman (idempoten).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TimezoneSnapshot {
    pub utc_offset_minutes: Option<i32>,
    pub abbr: Option<String>,
}

impl TimezoneSnapshot {
    pub async fn load(db: &DbPool) -> Self {
        let conn = db.lock().await;
        let get_val = |key: &str| -> Option<String> {
            conn.query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |r| r.get(0),
            )
            .optional()
            .unwrap_or(None)
        };

        Self {
            utc_offset_minutes: get_val("branding_utc_offset_minutes")
                .and_then(|v| v.trim().parse::<i32>().ok()),
            abbr: get_val("branding_timezone_abbr").filter(|s| !s.trim().is_empty()),
        }
    }

    pub async fn save(&self, db: &DbPool) -> Result<()> {
        let conn = db.lock().await;
        let offset_str = self.utc_offset_minutes.map(|m| m.to_string()).unwrap_or_default();
        let abbr_str = self.abbr.clone().unwrap_or_default();

        let pairs = [
            ("branding_utc_offset_minutes", offset_str.as_str()),
            ("branding_timezone_abbr", abbr_str.as_str()),
        ];

        for (k, v) in pairs {
            conn.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![k, v],
            )?;
        }
        Ok(())
    }

    /// Offset aplikasi bila tersedia, jika tidak jatuh ke offset zona OS.
    ///
    /// Fallback ke offset OS berarti delta nol: perilaku lama dipertahankan
    /// untuk instalasi yang zona OS-nya sudah benar, dan yang belum pernah
    /// membuka konsol admin sejak pembaruan ini.
    pub fn offset_or_os(&self) -> i32 {
        self.utc_offset_minutes
            .unwrap_or_else(|| Local::now().offset().local_minus_utc() / 60)
    }

    /// Singkatan zona untuk label notifikasi. Bila belum pernah disinkronkan,
    /// diturunkan dari offset dalam bentuk `UTC+7` / `UTC+5:30` / `UTC`.
    pub fn abbr_or_derived(&self) -> String {
        if let Some(a) = self.abbr.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
            return a.to_string();
        }
        let total = self.offset_or_os();
        if total == 0 {
            return "UTC".to_string();
        }
        let sign = if total < 0 { '-' } else { '+' };
        let abs = total.abs();
        let (h, m) = (abs / 60, abs % 60);
        if m == 0 {
            format!("UTC{}{}", sign, h)
        } else {
            format!("UTC{}{}:{:02}", sign, h, m)
        }
    }
}

// Model konfigurasi notifikasi Telegram
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramSettings {
    pub enabled: bool,
    pub bot_token: String,
    pub chat_id: String,
    pub thread_id: Option<i64>, // Mendukung topik terpisah pada Telegram Forum Supergroup
}

impl TelegramSettings {
    // Membaca pengaturan Telegram dari database SQLite
    pub async fn load(db: &DbPool) -> Result<Self> {
        let conn = db.lock().await;
        let get_val = |key: &str| -> Option<String> {
            conn.query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |r| r.get(0),
            )
            .optional()
            .unwrap_or(None)
        };

        let enabled = get_val("telegram_enabled").map(|v| v == "true").unwrap_or(false);
        let bot_token = get_val("telegram_bot_token").unwrap_or_default();
        let chat_id = get_val("telegram_chat_id").unwrap_or_default();
        let thread_id = get_val("telegram_thread_id").and_then(|v| v.parse::<i64>().ok());

        Ok(Self {
            enabled,
            bot_token,
            chat_id,
            thread_id,
        })
    }

    // Menyimpan pengaturan Telegram ke database SQLite
    pub async fn save(&self, db: &DbPool) -> Result<()> {
        let conn = db.lock().await;
        let pairs = [
            ("telegram_enabled", if self.enabled { "true" } else { "false" }),
            ("telegram_bot_token", &self.bot_token),
            ("telegram_chat_id", &self.chat_id),
            ("telegram_thread_id", &self.thread_id.map(|id| id.to_string()).unwrap_or_default()),
        ];

        for (k, v) in pairs {
            conn.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![k, v],
            )?;
        }
        Ok(())
    }

    // Mengirim pesan ke Telegram Bot API (mendukung thread_id / forum topic)
    pub async fn send_message(&self, message_html: &str) -> Result<(), String> {
        if !self.enabled || self.bot_token.trim().is_empty() || self.chat_id.trim().is_empty() {
            crate::alert_log::log(
                "SKIP",
                &format!(
                    "Telegram dilewati (enabled={}, token={}, chat_id={})",
                    self.enabled,
                    if self.bot_token.trim().is_empty() { "kosong" } else { "ada" },
                    if self.chat_id.trim().is_empty() { "kosong" } else { "ada" },
                ),
            );
            return Ok(());
        }

        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.bot_token.trim());

        let mut payload = serde_json::json!({
            "chat_id": self.chat_id.trim(),
            "text": message_html,
            "parse_mode": "HTML",
            "disable_web_page_preview": true
        });

        // Sertakan message_thread_id jika user mengkonfigurasi topik grup terpisah
        if let Some(tid) = self.thread_id {
            if tid > 0 {
                payload["message_thread_id"] = serde_json::json!(tid);
            }
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| e.to_string())?;

        let start = std::time::Instant::now();
        let resp = match client.post(&url).json(&payload).send().await {
            Ok(r) => r,
            Err(e) => {
                let msg = e.to_string();
                // Catat kegagalan jaringan/timeout agar ada jejak permanen
                crate::alert_log::log(
                    "ERROR",
                    &format!(
                        "Telegram GAGAL terkirim setelah {:.1}s: {}",
                        start.elapsed().as_secs_f64(),
                        msg
                    ),
                );
                error!("Telegram send failed: {}", msg);
                return Err(msg);
            }
        };

        let elapsed = start.elapsed().as_secs_f64();

        if !resp.status().is_success() {
            let status = resp.status();
            let err_text = resp.text().await.unwrap_or_default();
            crate::alert_log::log(
                "ERROR",
                &format!("Telegram API menolak (HTTP {}): {}", status.as_u16(), err_text),
            );
            error!("Telegram API error: {}", err_text);
            return Err(err_text);
        }

        crate::alert_log::log(
            "OK",
            &format!("Telegram terkirim ({:.1}s)", elapsed),
        );
        info!("Telegram notification successfully sent.");
        Ok(())
    }

    // Mengirim notifikasi saat target terdeteksi DOWN
    pub async fn notify_down(
        db: &DbPool,
        service_name: &str,
        target: &str,
        error_msg: &str,
        started_at: &str,
    ) {
        if let Ok(cfg) = Self::load(db).await
            && cfg.enabled
        {
            let tz = TimezoneSnapshot::load(db).await;
            let stamp = shift_local_timestamp(started_at, tz.offset_or_os());
            let topic_info = cfg.thread_id.map(|t| format!(" • Topic #{}", t)).unwrap_or_default();
            let text = format!(
                "🔴 <b>[DOWN] Service Incident Detected</b>{}\n\n\
                 <b>Service:</b> {}\n\
                 <b>Target:</b> <code>{}</code>\n\
                 <b>Time:</b> {} {}\n\
                 <b>Error:</b> <code>{}</code>",
                topic_info,
                html_escape(service_name),
                html_escape(target),
                stamp,
                tz.abbr_or_derived(),
                html_escape(error_msg)
            );

            if let Err(e) = cfg.send_message(&text).await {
                error!("Failed to send Telegram DOWN alert: {}", e);
            }
        }
    }

    // Mengirim notifikasi saat target berhasil RECOVERED / kembali pulih
    pub async fn notify_recovery(
        db: &DbPool,
        service_name: &str,
        target: &str,
        duration_sec: i64,
        resolved_at: &str,
    ) {
        if let Ok(cfg) = Self::load(db).await
            && cfg.enabled
        {
            let tz = TimezoneSnapshot::load(db).await;
            let stamp = shift_local_timestamp(resolved_at, tz.offset_or_os());
            let dur_str = format_duration(duration_sec);
            let topic_info = cfg.thread_id.map(|t| format!(" • Topic #{}", t)).unwrap_or_default();
            let text = format!(
                "🟢 <b>[RECOVERED] Service Restored</b>{}\n\n\
                 <b>Service:</b> {}\n\
                 <b>Target:</b> <code>{}</code>\n\
                 <b>Downtime:</b> <b>{}</b>\n\
                 <b>Restored At:</b> {} {}",
                topic_info,
                html_escape(service_name),
                html_escape(target),
                dur_str,
                stamp,
                tz.abbr_or_derived()
            );

            if let Err(e) = cfg.send_message(&text).await {
                error!("Failed to send Telegram RECOVERY alert: {}", e);
            }
        }
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn format_duration(sec: i64) -> String {
    if sec < 60 {
        format!("{}s", sec)
    } else if sec < 3600 {
        format!("{}m {}s", sec / 60, sec % 60)
    } else {
        format!("{}h {}m", sec / 3600, (sec % 3600) / 60)
    }
}

// Model konfigurasi identitas branding & lokalisasi waktu halaman publik
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrandingSettings {
    pub site_title: String,
    pub site_subtitle: String,
    pub logo_url: String,
    pub custom_footer: String,
    #[serde(default = "default_timezone")]
    pub timezone: String,
    #[serde(default = "default_time_format")]
    pub time_format: String,
    #[serde(default = "default_date_format")]
    pub date_format: String,
}

fn default_timezone() -> String {
    "Asia/Jakarta".to_string()
}

fn default_time_format() -> String {
    "24h".to_string()
}

fn default_date_format() -> String {
    "DD-MM-YYYY".to_string()
}

impl Default for BrandingSettings {
    fn default() -> Self {
        Self {
            site_title: "System Status".to_string(),
            site_subtitle: String::new(),
            logo_url: String::new(),
            custom_footer: String::new(),
            timezone: default_timezone(),
            time_format: default_time_format(),
            date_format: default_date_format(),
        }
    }
}

impl BrandingSettings {
    pub async fn load(db: &DbPool) -> Result<Self> {
        let conn = db.lock().await;
        let get_val = |key: &str| -> Option<String> {
            conn.query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |r| r.get(0),
            )
            .optional()
            .unwrap_or(None)
        };

        let site_title = get_val("branding_site_title")
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "System Status".to_string());
        let site_subtitle = get_val("branding_site_subtitle").unwrap_or_default();
        let logo_url = get_val("branding_logo_url").unwrap_or_default();
        let custom_footer = get_val("branding_custom_footer").unwrap_or_default();
        let timezone = get_val("branding_timezone")
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "Asia/Jakarta".to_string());
        let time_format = get_val("branding_time_format")
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "24h".to_string());
        let date_format = get_val("branding_date_format")
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "DD-MM-YYYY".to_string());

        Ok(Self {
            site_title,
            site_subtitle,
            logo_url,
            custom_footer,
            timezone,
            time_format,
            date_format,
        })
    }

    pub async fn save(&self, db: &DbPool) -> Result<()> {
        let conn = db.lock().await;
        let pairs = [
            ("branding_site_title", self.site_title.as_str()),
            ("branding_site_subtitle", self.site_subtitle.as_str()),
            ("branding_logo_url", self.logo_url.as_str()),
            ("branding_custom_footer", self.custom_footer.as_str()),
            ("branding_timezone", self.timezone.as_str()),
            ("branding_time_format", self.time_format.as_str()),
            ("branding_date_format", self.date_format.as_str()),
        ];

        for (k, v) in pairs {
            conn.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![k, v],
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Delta dihitung terhadap offset zona OS saat test berjalan, sehingga
    // asersi di bawah bebas dari zona waktu mesin pengembang.
    fn os_offset_minutes() -> i32 {
        Local::now().offset().local_minus_utc() / 60
    }

    fn expected_shift(stamp: &str, app_offset: i32) -> String {
        let dt = NaiveDateTime::parse_from_str(stamp, "%Y-%m-%d %H:%M:%S").unwrap();
        (dt + Duration::minutes(i64::from(app_offset - os_offset_minutes())))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string()
    }

    #[test]
    fn shift_matches_configured_app_timezone() {
        let stamp = "2026-09-19 08:20:39";
        let app_offset = 7 * 60; // WIB
        assert_eq!(shift_local_timestamp(stamp, app_offset), expected_shift(stamp, app_offset));
    }

    // --- Bukti angka absolut: inti keluhan "waktu Telegram masih UTC" ---
    // Insiden nyata 2026-09-19 10:00 UTC = 17:00 WIB.

    #[test]
    fn os_utc_with_wib_app_shifts_seven_hours_forward() {
        // Kasus laporan: OS UTC, aplikasi diset WIB. Nilai lama akan tampil
        // "10:00 WIB" (7 jam lebih awal); yang benar adalah 17:00 WIB.
        let stored = "2026-09-19 10:00:00";
        assert_eq!(
            shift_with_os_offset(stored, 420, 0),
            "2026-09-19 17:00:00"
        );
    }

    #[test]
    fn os_wib_with_wib_app_is_unchanged() {
        // Kasus STB produksi: OS sudah WIB, jadi tidak boleh bergeser sama sekali.
        let stored = "2026-09-19 17:00:00";
        assert_eq!(
            shift_with_os_offset(stored, 420, 420),
            "2026-09-19 17:00:00"
        );
    }

    #[test]
    fn os_utc_with_new_york_app_shifts_backwards() {
        // 10:00 UTC = 06:00 EDT (UTC-4).
        let stored = "2026-09-19 10:00:00";
        assert_eq!(
            shift_with_os_offset(stored, -240, 0),
            "2026-09-19 06:00:00"
        );
    }

    #[test]
    fn half_hour_offset_is_supported() {
        // 10:00 UTC = 15:30 IST (UTC+5:30) — offset 30 menit tidak boleh hilang.
        let stored = "2026-09-19 10:00:00";
        assert_eq!(
            shift_with_os_offset(stored, 330, 0),
            "2026-09-19 15:30:00"
        );
    }

    #[test]
    fn shift_crosses_day_boundary_correctly() {
        // 2026-09-19 20:00 UTC + 7 jam = 2026-09-20 03:00 WIB (ganti hari).
        let stored = "2026-09-19 20:00:00";
        assert_eq!(
            shift_with_os_offset(stored, 420, 0),
            "2026-09-20 03:00:00"
        );
    }

    #[test]
    fn shift_is_noop_when_app_timezone_equals_os_timezone() {
        let stamp = "2026-09-19 08:20:39";
        assert_eq!(shift_local_timestamp(stamp, os_offset_minutes()), stamp);
    }

    #[test]
    fn shift_handles_negative_offsets() {
        let stamp = "2026-09-19 23:30:00";
        let app_offset = -4 * 60; // EDT
        assert_eq!(shift_local_timestamp(stamp, app_offset), expected_shift(stamp, app_offset));
    }

    #[test]
    fn shift_keeps_unparseable_input_unchanged() {
        // Alert harus tetap terkirim walau format stempel tak dikenal.
        assert_eq!(shift_local_timestamp("bukan-tanggal", 420), "bukan-tanggal");
    }

    #[test]
    fn derived_abbr_formats_offset() {
        let snap = TimezoneSnapshot { utc_offset_minutes: Some(420), abbr: None };
        assert_eq!(snap.abbr_or_derived(), "UTC+7");

        let half = TimezoneSnapshot { utc_offset_minutes: Some(330), abbr: None };
        assert_eq!(half.abbr_or_derived(), "UTC+5:30");

        let zero = TimezoneSnapshot { utc_offset_minutes: Some(0), abbr: None };
        assert_eq!(zero.abbr_or_derived(), "UTC");

        let west = TimezoneSnapshot { utc_offset_minutes: Some(-300), abbr: None };
        assert_eq!(west.abbr_or_derived(), "UTC-5");
    }

    #[test]
    fn stored_abbr_wins_over_derived() {
        let snap = TimezoneSnapshot { utc_offset_minutes: Some(420), abbr: Some("WIB".to_string()) };
        assert_eq!(snap.abbr_or_derived(), "WIB");
    }

    #[test]
    fn offset_falls_back_to_os_when_absent() {
        let snap = TimezoneSnapshot::default();
        assert_eq!(snap.offset_or_os(), os_offset_minutes());
    }
}
