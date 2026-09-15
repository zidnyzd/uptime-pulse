use std::time::Instant;
use tokio::net::TcpStream;
use tokio::time::{timeout, Duration};
use crate::models::ProbeResult;

// Dispatcher utama pengecekan target berdasarkan tipe protokol yang dipilih
pub async fn probe(
    monitor_id: i64,
    monitor_type: &str,
    target: &str,
    timeout_sec: i64,
) -> ProbeResult {
    let timeout_duration = Duration::from_secs(timeout_sec.max(1) as u64);

    match monitor_type {
        "http" | "https" => probe_http(monitor_id, target, timeout_duration).await,
        "tcp" => probe_tcp(monitor_id, target, timeout_duration).await,
        _ => ProbeResult {
            monitor_id,
            is_up: false,
            status_code: None,
            latency_ms: 0.0,
            error_message: Some(format!("Unsupported monitor type: {}", monitor_type)),
        },
    }
}

// Melakukan HTTP/HTTPS GET request dan mengukur waktu respons (latency)
async fn probe_http(monitor_id: i64, target: &str, timeout_duration: Duration) -> ProbeResult {
    let url = if !target.starts_with("http://") && !target.starts_with("https://") {
        format!("https://{}", target)
    } else {
        target.to_string()
    };

    // Menggunakan Rustls TLS engine (pure Rust) tanpa dependensi OpenSSL C-lib
    let client = match reqwest::Client::builder()
        .timeout(timeout_duration)
        .danger_accept_invalid_certs(false)
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return ProbeResult {
                monitor_id,
                is_up: false,
                status_code: None,
                latency_ms: 0.0,
                error_message: Some(format!("Failed to build HTTP client: {}", e)),
            };
        }
    };

    // Instant::now() menggunakan monotonic clock sistem yang akurat dan tidak terpengaruh pergeseran jam NTP
    let start = Instant::now();
    match client.get(&url).send().await {
        Ok(response) => {
            let status = response.status();
            let latency = start.elapsed().as_secs_f64() * 1000.0;
            let status_code = status.as_u16() as i32;

            // Standar monitoring: Kode 2xx (sukses) dan 3xx (redirect) dianggap UP
            let is_up = status.is_success() || status.is_redirection();
            let error_message = if !is_up {
                Some(format!("HTTP {}", status_code))
            } else {
                None
            };

            ProbeResult {
                monitor_id,
                is_up,
                status_code: Some(status_code),
                latency_ms: (latency * 10.0).round() / 10.0,
                error_message,
            }
        }
        Err(e) => {
            let latency = start.elapsed().as_secs_f64() * 1000.0;
            ProbeResult {
                monitor_id,
                is_up: false,
                status_code: None,
                latency_ms: (latency * 10.0).round() / 10.0,
                error_message: Some(clean_reqwest_error(&e)),
            }
        }
    }
}

// Melakukan pengujian TCP handshake (SYN/ACK) ke target host:port
async fn probe_tcp(monitor_id: i64, target: &str, timeout_duration: Duration) -> ProbeResult {
    let start = Instant::now();

    // Membungkus TcpStream::connect dengan tokio::time::timeout untuk memastikan tidak hang jika port silent/dropped
    match timeout(timeout_duration, TcpStream::connect(target)).await {
        Ok(Ok(_stream)) => {
            let latency = start.elapsed().as_secs_f64() * 1000.0;
            ProbeResult {
                monitor_id,
                is_up: true,
                status_code: None,
                latency_ms: (latency * 10.0).round() / 10.0,
                error_message: None,
            }
        }
        Ok(Err(e)) => {
            let latency = start.elapsed().as_secs_f64() * 1000.0;
            ProbeResult {
                monitor_id,
                is_up: false,
                status_code: None,
                latency_ms: (latency * 10.0).round() / 10.0,
                error_message: Some(format!("Connection refused / error: {}", e)),
            }
        }
        Err(_) => ProbeResult {
            monitor_id,
            is_up: false,
            status_code: None,
            latency_ms: timeout_duration.as_millis() as f64,
            error_message: Some("Connection timed out".to_string()),
        },
    }
}

// Menyederhanakan pesan error teknis reqwest agar mudah dibaca pada dashboard
fn clean_reqwest_error(e: &reqwest::Error) -> String {
    if e.is_timeout() {
        "Request timed out".to_string()
    } else if e.is_connect() {
        "Connection failed / DNS error".to_string()
    } else {
        e.to_string()
    }
}
