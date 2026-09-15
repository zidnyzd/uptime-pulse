use serde::{Deserialize, Serialize};

// Entitas utama target monitoring yang disimpan di SQLite
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Monitor {
    pub id: i64,
    pub name: String,
    pub monitor_type: String, // "http" atau "tcp"
    pub target: String,       // URL (https://...) atau host:port (1.1.1.1:53)
    pub interval_sec: i64,    // Frekuensi probe dalam detik (misal: 60s)
    pub timeout_sec: i64,     // Batas waktu probe sebelum dianggap timeout
    pub is_active: bool,      // Status aktif/pause pengecekan
    pub status: String,       // Status terkini: "up", "down", "pending", "paused"
    pub last_latency_ms: Option<f64>, // Nullable: bernilai None sebelum probe pertama
    pub last_check_at: Option<String>,
    pub created_at: String,
}

// DTO untuk validasi payload JSON saat user menambahkan monitor baru
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMonitorInput {
    pub name: String,
    pub monitor_type: String,
    pub target: String,
    #[serde(default = "default_interval")]
    pub interval_sec: i64,
    #[serde(default = "default_timeout")]
    pub timeout_sec: i64,
}

fn default_interval() -> i64 {
    60
}

fn default_timeout() -> i64 {
    10
}

// Rekaman log hasil satu kali pemeriksaan probe ke target
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heartbeat {
    pub id: i64,
    pub monitor_id: i64,
    pub is_up: bool,
    pub status_code: Option<i32>,
    pub latency_ms: f64,
    pub error_message: Option<String>,
    pub checked_at: String,
}

// Data sementara hasil probing di layer prober sebelum disimpan ke database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeResult {
    pub monitor_id: i64,
    pub is_up: bool,
    pub status_code: Option<i32>,
    pub latency_ms: f64,
    pub error_message: Option<String>,
}

// Gabungan data monitor + riwayat heartbeat untuk rendering sparkline & metrik 24 jam
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorDetail {
    pub monitor: Monitor,
    pub recent_heartbeats: Vec<Heartbeat>,
    pub uptime_24h: f64,
    pub avg_latency_24h: f64,
}
