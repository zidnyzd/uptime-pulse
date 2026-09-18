use std::time::Instant;
use tokio::net::TcpStream;
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use crate::models::ProbeResult;

// Konfigurasi request HTTP kustom (method, header, body, dan assertion JSON).
// Dipakai agar bisa memantau endpoint non-GET, API yang butuh autentikasi,
// serta endpoint yang perlu dicek isi responsnya (bukan hanya status code).
#[derive(Debug, Clone, Default)]
pub struct HttpRequestConfig {
    pub method: String,
    pub headers: String,
    pub body: String,
    /// Jalur (dot notation) ke nilai di dalam JSON respons, mis. "status" atau "data.health".
    /// Kosong berarti tidak ada pemeriksaan isi.
    pub json_path: String,
    /// Nilai yang diharapkan pada `json_path`. Kosong berarti cukup path-nya ada.
    pub expected_value: String,
}

/// User-Agent bawaan yang dikirim pada setiap request HTTP.
///
/// Tanpa ini, reqwest tidak mengirim User-Agent sama sekali, sehingga di access log
/// server target request UptimePulse muncul sebagai "-" dan tidak bisa dibedakan
/// dari bot/scraper. Nilai ini bisa ditimpa per monitor lewat custom header
/// "User-Agent".
pub const DEFAULT_USER_AGENT: &str = concat!(
    "UptimePulse/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/zidnyzd/uptime-pulse)"
);

// Dispatcher dengan konfigurasi request HTTP (method/headers/body/assertion JSON).
// Tipe tcp/ping mengabaikan konfigurasi ini.
pub async fn probe_with_config(
    monitor_id: i64,
    monitor_type: &str,
    target: &str,
    timeout_sec: i64,
    http_cfg: &HttpRequestConfig,
) -> ProbeResult {
    let timeout_duration = Duration::from_secs(timeout_sec.max(1) as u64);

    match monitor_type {
        "http" | "https" => probe_http(monitor_id, target, timeout_duration, http_cfg, false).await,
        // Sama seperti http, tetapi isi respons diperiksa terhadap json_path/expected_value
        "http_json" => probe_http(monitor_id, target, timeout_duration, http_cfg, true).await,
        "tcp" => probe_tcp(monitor_id, target, timeout_duration).await,
        "ping" | "icmp" => probe_ping(monitor_id, target, timeout_sec).await,
        _ => ProbeResult {
            monitor_id,
            is_up: false,
            status_code: None,
            latency_ms: 0.0,
            error_message: Some(format!("Unsupported monitor type: {}", monitor_type)),
        },
    }
}

/// Mencari nilai di dalam JSON memakai jalur dot notation.
/// Mendukung key objek dan indeks array, mis. "data.items.0.state".
/// Mengembalikan nilai sebagai String agar bisa dibandingkan dengan
/// `expected_value` yang diketik pengguna.
fn json_path_lookup(root: &serde_json::Value, path: &str) -> Option<String> {
    let mut current = root;
    for segmen in path.split('.') {
        let segmen = segmen.trim();
        if segmen.is_empty() {
            continue;
        }
        current = match current {
            serde_json::Value::Object(map) => map.get(segmen)?,
            serde_json::Value::Array(arr) => {
                let idx: usize = segmen.parse().ok()?;
                arr.get(idx)?
            }
            _ => return None,
        };
    }

    Some(match current {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        // Objek/array dibandingkan dalam bentuk JSON ringkas
        other => other.to_string(),
    })
}

/// Memeriksa isi respons terhadap json_path/expected_value.
/// Mengembalikan Ok(()) bila sesuai, atau Err(pesan) bila tidak.
fn check_json_assertion(body: &str, json_path: &str, expected_value: &str) -> Result<(), String> {
    let path = json_path.trim();
    if path.is_empty() {
        return Ok(());
    }

    let parsed: serde_json::Value = serde_json::from_str(body)
        .map_err(|e| format!("Respons bukan JSON valid: {}", e))?;

    let found = json_path_lookup(&parsed, path)
        .ok_or_else(|| format!("JSON path '{}' tidak ditemukan", path))?;

    let expected = expected_value.trim();
    if expected.is_empty() {
        // Hanya keberadaan path yang diperiksa
        return Ok(());
    }

    if found.trim() == expected {
        Ok(())
    } else {
        Err(format!("Nilai JSON '{}' adalah '{}', diharapkan '{}'", path, found, expected))
    }
}

// Melakukan HTTP request (method dapat dikonfigurasi) dan mengukur waktu respons.
// Bila `assert_json` aktif, isi respons juga diperiksa.
/// Mengurai header kustom dari format teks "Nama: Nilai" (satu per baris).
/// Baris kosong dan baris tanpa ':' diabaikan. Nama header divalidasi agar
/// tidak bisa menyuntikkan karakter ilegal ke protokol HTTP.
fn parse_headers(raw: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(pos) = line.find(':') {
            let name = line[..pos].trim().to_string();
            let value = line[pos + 1..].trim().to_string();
            // Nama header hanya boleh token HTTP valid; tolak newline/CR (header injection).
            let valid_name = !name.is_empty()
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
            let valid_value = !value.contains('\r') && !value.contains('\n');
            if valid_name && valid_value {
                out.push((name, value));
            }
        }
    }
    out
}

// Melakukan HTTP request (method dapat dikonfigurasi) dan mengukur waktu respons.
// Bila `assert_json` aktif, isi respons juga diperiksa terhadap json_path/expected_value.
async fn probe_http(
    monitor_id: i64,
    target: &str,
    timeout_duration: Duration,
    cfg: &HttpRequestConfig,
    assert_json: bool,
) -> ProbeResult {
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

    // Bangun request sesuai method yang dikonfigurasi
    let method_upper = cfg.method.trim().to_ascii_uppercase();
    let method = match method_upper.as_str() {
        "POST" => reqwest::Method::POST,
        "PUT" => reqwest::Method::PUT,
        "PATCH" => reqwest::Method::PATCH,
        "DELETE" => reqwest::Method::DELETE,
        "HEAD" => reqwest::Method::HEAD,
        "OPTIONS" => reqwest::Method::OPTIONS,
        _ => reqwest::Method::GET,
    };

    let mut req = client.request(method, &url);

    // Sisipkan header kustom (mis. Authorization, X-Api-Key)
    let headers = parse_headers(&cfg.headers);
    for (name, value) in &headers {
        req = req.header(name.as_str(), value.as_str());
    }

    // User-Agent bawaan, supaya di access log server target request ini bisa
    // dikenali (tanpa ini muncul sebagai "-"). Custom header "User-Agent"
    // dari pengguna tetap menang.
    let has_ua = headers.iter().any(|(n, _)| n.eq_ignore_ascii_case("user-agent"));
    if !has_ua {
        req = req.header("User-Agent", DEFAULT_USER_AGENT);
    }

    // Sisipkan body bila diisi dan method memungkinkan.
    // GET/HEAD secara umum tidak berbody; tetap dikirim bila user mengisinya
    // karena beberapa API non-standar memanfaatkannya.
    if !cfg.body.trim().is_empty() {
        // Set Content-Type default JSON hanya bila user belum menentukan sendiri
        let has_ct = headers.iter().any(|(n, _)| n.eq_ignore_ascii_case("content-type"));
        if !has_ct {
            req = req.header("Content-Type", "application/json");
        }
        req = req.body(cfg.body.clone());
    }

    // Instant::now() menggunakan monotonic clock sistem yang akurat dan tidak terpengaruh pergeseran jam NTP
    let start = Instant::now();
    match req.send().await {
        Ok(response) => {
            let status = response.status();
            let latency = start.elapsed().as_secs_f64() * 1000.0;
            let status_code = status.as_u16() as i32;

            // Standar monitoring: Kode 2xx (sukses) dan 3xx (redirect) dianggap UP
            let status_ok = status.is_success() || status.is_redirection();

            if !status_ok {
                return ProbeResult {
                    monitor_id,
                    is_up: false,
                    status_code: Some(status_code),
                    latency_ms: (latency * 10.0).round() / 10.0,
                    error_message: Some(format!("HTTP {}", status_code)),
                };
            }

            // Untuk tipe http_json: status 200 saja belum cukup, isi respons
            // harus memuat nilai yang diharapkan. Di sinilah kasus "HTTP 200
            // tapi body-nya error" tertangkap.
            let error_message = if assert_json {
                match response.text().await {
                    Ok(body) => check_json_assertion(&body, &cfg.json_path, &cfg.expected_value).err(),
                    Err(e) => Some(format!("Gagal membaca isi respons: {}", e)),
                }
            } else {
                None
            };

            ProbeResult {
                monitor_id,
                is_up: error_message.is_none(),
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

// Melakukan ICMP Ping secara non-blocking via perintah ping sistem (kompatibel penuh di Linux & OpenWrt)
async fn probe_ping(monitor_id: i64, target: &str, timeout_sec: i64) -> ProbeResult {
    let host = match sanitize_ping_target(target) {
        Some(h) => h,
        None => {
            return ProbeResult {
                monitor_id,
                is_up: false,
                status_code: None,
                latency_ms: 0.0,
                error_message: Some("Format target IP/host tidak valid".to_string()),
            };
        }
    };

    let timeout_val = timeout_sec.clamp(1, 10).to_string();
    let start = Instant::now();

    // Memanggil ping -c 2 -W <timeout> <host> untuk toleransi fluktuasi jaringan
    let cmd_result = timeout(
        Duration::from_secs((timeout_sec.max(1) + 4) as u64),
        Command::new("ping")
            .args(["-c", "2", "-W", &timeout_val, &host])
            .output(),
    )
    .await;

    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;

    match cmd_result {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            if output.status.success() {
                let latency = parse_ping_latency(&stdout, elapsed_ms);
                ProbeResult {
                    monitor_id,
                    is_up: true,
                    status_code: None,
                    latency_ms: latency,
                    error_message: None,
                }
            } else {
                let err = if !stderr.trim().is_empty() {
                    let raw = stderr.trim();
                    if raw.contains("Name or service not known")
                        || raw.contains("Temporary failure in name resolution")
                    {
                        "DNS resolution failed".to_string()
                    } else {
                        raw.to_string()
                    }
                } else if stdout.contains("100% packet loss") || stdout.contains("100% loss") {
                    "100% packet loss".to_string()
                } else if stdout.contains("Destination Host Unreachable") {
                    "Host unreachable".to_string()
                } else {
                    "Ping failed / no response".to_string()
                };

                ProbeResult {
                    monitor_id,
                    is_up: false,
                    status_code: None,
                    latency_ms: (elapsed_ms * 10.0).round() / 10.0,
                    error_message: Some(err),
                }
            }
        }
        Ok(Err(e)) => ProbeResult {
            monitor_id,
            is_up: false,
            status_code: None,
            latency_ms: 0.0,
            error_message: Some(format!("Eksekusi ping gagal: {}", e)),
        },
        Err(_) => ProbeResult {
            monitor_id,
            is_up: false,
            status_code: None,
            latency_ms: (elapsed_ms * 10.0).round() / 10.0,
            error_message: Some("Ping request timeout".to_string()),
        },
    }
}

// Sanitasi target ping untuk mencegah injeksi argumen/perintah shell
fn sanitize_ping_target(target: &str) -> Option<String> {
    let clean = target
        .trim()
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .split('/')
        .next()?
        .split(':')
        .next()?;

    if clean.is_empty() || !clean.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '_') {
        return None;
    }
    Some(clean.to_string())
}

// Mengambil angka latency (ms) dari baris output ping
fn parse_ping_latency(stdout: &str, fallback_ms: f64) -> f64 {
    // 1. Coba ambil rata-rata (avg) dari baris statistik summary rtt/round-trip
    for line in stdout.lines() {
        if line.contains("min/avg/max") || line.contains("rtt ") || line.contains("round-trip ") {
            if let Some(eq_pos) = line.find('=') {
                let parts: Vec<&str> = line[eq_pos + 1..].split('/').collect();
                if parts.len() >= 2 {
                    if let Ok(val) = parts[1].trim().parse::<f64>() {
                        return (val * 10.0).round() / 10.0;
                    }
                }
            }
        }
    }

    // 2. Fallback: ambil baris time= pertama
    for line in stdout.lines() {
        if let Some(pos) = line.find("time=") {
            let part = &line[pos + 5..];
            let num_str: String = part.chars().take_while(|c| c.is_digit(10) || *c == '.').collect();
            if let Ok(val) = num_str.parse::<f64>() {
                return (val * 10.0).round() / 10.0;
            }
        }
    }
    (fallback_ms * 10.0).round() / 10.0
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
