use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Monitor {
    pub id: i64,
    pub name: String,
    pub monitor_type: String, // "http", "tcp"
    pub target: String,       // URL or host:port
    pub interval_sec: i64,    // interval in seconds
    pub timeout_sec: i64,     // timeout in seconds
    pub is_active: bool,
    pub status: String,       // "up", "down", "pending", "paused"
    pub last_latency_ms: Option<f64>,
    pub last_check_at: Option<String>,
    pub created_at: String,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeResult {
    pub monitor_id: i64,
    pub is_up: bool,
    pub status_code: Option<i32>,
    pub latency_ms: f64,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorDetail {
    pub monitor: Monitor,
    pub recent_heartbeats: Vec<Heartbeat>,
    pub uptime_24h: f64,
    pub avg_latency_24h: f64,
}
