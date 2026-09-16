use rusqlite::{params, OptionalExtension, Result};
use serde::{Deserialize, Serialize};
use tracing::{error, info};
use crate::database::DbPool;

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

        let resp = client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            error!("Telegram API error: {}", err_text);
            return Err(err_text);
        }

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
        if let Ok(cfg) = Self::load(db).await {
            if cfg.enabled {
                let topic_info = cfg.thread_id.map(|t| format!(" • Topic #{}", t)).unwrap_or_default();
                let text = format!(
                    "🔴 <b>[DOWN] Service Incident Detected</b>{}\n\n\
                     <b>Service:</b> {}\n\
                     <b>Target:</b> <code>{}</code>\n\
                     <b>Time:</b> {} WIB\n\
                     <b>Error:</b> <code>{}</code>",
                    topic_info,
                    html_escape(service_name),
                    html_escape(target),
                    started_at,
                    html_escape(error_msg)
                );

                if let Err(e) = cfg.send_message(&text).await {
                    error!("Failed to send Telegram DOWN alert: {}", e);
                }
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
        if let Ok(cfg) = Self::load(db).await {
            if cfg.enabled {
                let dur_str = format_duration(duration_sec);
                let topic_info = cfg.thread_id.map(|t| format!(" • Topic #{}", t)).unwrap_or_default();
                let text = format!(
                    "🟢 <b>[RECOVERED] Service Restored</b>{}\n\n\
                     <b>Service:</b> {}\n\
                     <b>Target:</b> <code>{}</code>\n\
                     <b>Downtime:</b> <b>{}</b>\n\
                     <b>Restored At:</b> {} WIB",
                    topic_info,
                    html_escape(service_name),
                    html_escape(target),
                    dur_str,
                    resolved_at
                );

                if let Err(e) = cfg.send_message(&text).await {
                    error!("Failed to send Telegram RECOVERY alert: {}", e);
                }
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
