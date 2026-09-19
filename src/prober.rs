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
    /// Cara membandingkan nilai pada `json_path` dengan `expected_value`.
    /// Kosong / tak dikenal diperlakukan sebagai `==` (perilaku lama).
    pub json_operator: String,
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

/// Operator pembanding untuk assertion JSON.
///
/// Sebelumnya hanya ada perbandingan "sama persis", sehingga endpoint yang
/// membalas `{"count": 0}` atau `{"usage": 91.5}` tidak bisa dibedakan antara
/// sehat dan bermasalah. Operator di bawah menutup kasus itu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonOperator {
    Equals,
    NotEquals,
    Contains,
    NotContains,
    GreaterThan,
    GreaterOrEqual,
    LessThan,
    LessOrEqual,
}

impl JsonOperator {
    /// Mengurai operator dari input pengguna. Nilai tak dikenal jatuh ke `==`
    /// supaya monitor lama (yang tidak punya kolom ini) tetap berperilaku sama.
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "!=" | "not_equals" | "neq" | "<>" => Self::NotEquals,
            "contains" | "mengandung" => Self::Contains,
            "not_contains" | "tidak_mengandung" => Self::NotContains,
            ">" | "gt" => Self::GreaterThan,
            ">=" | "gte" => Self::GreaterOrEqual,
            "<" | "lt" => Self::LessThan,
            "<=" | "lte" => Self::LessOrEqual,
            // "" | "==" | "eq" | apa pun yang tak dikenal
            _ => Self::Equals,
        }
    }

    /// Label operator untuk pesan error yang bisa dibaca manusia.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Equals => "==",
            Self::NotEquals => "!=",
            Self::Contains => "contains",
            Self::NotContains => "not contains",
            Self::GreaterThan => ">",
            Self::GreaterOrEqual => ">=",
            Self::LessThan => "<",
            Self::LessOrEqual => "<=",
        }
    }

    /// Apakah operator ini butuh perbandingan numerik.
    fn is_numeric(&self) -> bool {
        matches!(
            self,
            Self::GreaterThan | Self::GreaterOrEqual | Self::LessThan | Self::LessOrEqual
        )
    }

    /// Membandingkan nilai yang ditemukan dengan nilai harapan.
    ///
    /// Perbandingan teks dilakukan case-insensitive agar `ok` cocok dengan `OK`
    /// — penyedia API sering tidak konsisten soal kapitalisasi status.
    pub fn matches(&self, found: &str, expected: &str) -> Result<bool, String> {
        let f = found.trim();
        let e = expected.trim();

        if self.is_numeric() {
            let fnum: f64 = f
                .parse()
                .map_err(|_| format!("Nilai JSON '{}' bukan angka, tidak bisa dibandingkan dengan '{}'", f, self.label()))?;
            let enum_: f64 = e
                .parse()
                .map_err(|_| format!("Nilai harapan '{}' bukan angka, tidak bisa dipakai dengan operator '{}'", e, self.label()))?;
            return Ok(match self {
                Self::GreaterThan => fnum > enum_,
                Self::GreaterOrEqual => fnum >= enum_,
                Self::LessThan => fnum < enum_,
                Self::LessOrEqual => fnum <= enum_,
                _ => unreachable!("operator non-numerik sudah ditangani di atas"),
            });
        }

        let fl = f.to_lowercase();
        let el = e.to_lowercase();
        Ok(match self {
            Self::Equals => fl == el,
            Self::NotEquals => fl != el,
            Self::Contains => fl.contains(&el),
            Self::NotContains => !fl.contains(&el),
            _ => unreachable!("operator numerik sudah ditangani di atas"),
        })
    }
}

/// Mencari nilai di dalam JSON memakai jalur dot notation.
///
/// Mendukung:
/// - key objek: `data.health`
/// - indeks array: `items.0.state`
/// - wildcard array: `items[*].state`, `items.*.state`, atau `items[].state`
///
/// Mengembalikan SELURUH nilai yang cocok (satu untuk path biasa, banyak untuk
/// wildcard). Pemanggil menerapkan operator ke tiap nilai secara terpisah —
/// menggabungkan nilainya lebih dulu lalu membandingkan sebagai satu string
/// akan membuat `items[*].state == up` mustahil terpenuhi.
fn json_path_lookup_all(root: &serde_json::Value, path: &str) -> Option<Vec<String>> {
    // Nilai yang ditemukan sejauh ini. Dimulai dari root, lalu tiap segmen
    // memetakan satu kumpulan kandidat ke kumpulan berikutnya.
    let mut current: Vec<&serde_json::Value> = vec![root];

    for segmen in path.split('.') {
        let segmen = segmen.trim();
        if segmen.is_empty() {
            continue;
        }

        // Dukung bentuk `items[0]`, `items[*]`, dan `items[]` sebagai satu segmen.
        // Segmen `*` tanpa bracket juga diperlakukan sebagai wildcard array,
        // karena `items.*.state` adalah penulisan dot-notation yang wajar.
        let (name, bracket) = if segmen == "*" {
            ("", Some("*"))
        } else if let Some((before, spec)) = segmen
            .strip_suffix(']')
            .and_then(|s| s.split_once('['))
        {
            (before, Some(spec))
        } else {
            (segmen, None)
        };

        let mut next: Vec<&serde_json::Value> = Vec::new();

        for node in &current {
            // Bagian nama: key objek, atau indeks bila node-nya array.
            let after_name: Vec<&serde_json::Value> = if name.is_empty() {
                vec![node]
            } else {
                match node {
                    serde_json::Value::Object(map) => match map.get(name) {
                        Some(v) => vec![v],
                        None => continue,
                    },
                    serde_json::Value::Array(arr) => match name.parse::<usize>().ok().and_then(|i| arr.get(i)) {
                        Some(v) => vec![v],
                        None => continue,
                    },
                    _ => continue,
                }
            };

            // Bagian bracket: indeks spesifik, wildcard, atau tidak ada.
            match bracket {
                None => next.extend(after_name),
                Some(spec) => {
                    let spec = spec.trim();
                    for v in after_name {
                        match v {
                            serde_json::Value::Array(arr) => {
                                if spec.is_empty() || spec == "*" {
                                    next.extend(arr.iter());
                                } else if let Ok(idx) = spec.parse::<usize>() {
                                    if let Some(item) = arr.get(idx) {
                                        next.push(item);
                                    }
                                }
                            }
                            // Bracket pada non-array diabaikan, bukan error:
                            // respons bisa berbentuk objek pada sebagian hari.
                            _ => {}
                        }
                    }
                }
            }
        }

        if next.is_empty() {
            return None;
        }
        current = next;
    }

    Some(current.iter().map(|v| render_json_scalar(v)).collect())
}

/// Mencari satu nilai (hasil pertama) — dipakai test dan pemakaian sederhana.
#[cfg(test)]
fn json_path_lookup(root: &serde_json::Value, path: &str) -> Option<String> {
    json_path_lookup_all(root, path)?.into_iter().next()
}

/// Mengubah nilai JSON menjadi teks untuk dibandingkan dengan `expected_value`.
fn render_json_scalar(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        // Objek/array dibandingkan dalam bentuk JSON ringkas
        other => other.to_string(),
    }
}

/// Memeriksa isi respons terhadap json_path/expected_value.
/// Mengembalikan Ok(()) bila sesuai, atau Err(pesan) bila tidak.
///
/// Untuk wildcard (banyak nilai cocok), SETIAP nilai harus memenuhi operator.
/// Aturan "semua harus lolos" ini yang membuat path seperti `items[*].state`
/// berguna: satu elemen menyimpang sudah cukup menandai target DOWN. Untuk
/// pertanyaan "tidak boleh ada yang error", gunakan `not_contains`.
fn check_json_assertion(
    body: &str,
    json_path: &str,
    expected_value: &str,
    operator_raw: &str,
) -> Result<(), String> {
    let path = json_path.trim();
    if path.is_empty() {
        return Ok(());
    }

    let parsed: serde_json::Value = serde_json::from_str(body)
        .map_err(|e| format!("Respons bukan JSON valid: {}", e))?;

    let found = json_path_lookup_all(&parsed, path)
        .ok_or_else(|| format!("JSON path '{}' tidak ditemukan", path))?;

    let expected = expected_value.trim();
    if expected.is_empty() {
        // Hanya keberadaan path yang diperiksa
        return Ok(());
    }

    let operator = JsonOperator::parse(operator_raw);

    // Sebutkan posisi nilai yang gagal bila wildcard menghasilkan banyak nilai,
    // supaya operator tahu elemen mana yang bermasalah.
    let many = found.len() > 1;
    for (idx, value) in found.iter().enumerate() {
        if !operator.matches(value, expected)? {
            let label = if many {
                format!("{} (nilai ke-{})", path, idx + 1)
            } else {
                path.to_string()
            };
            return Err(format!(
                "Nilai JSON '{}' adalah '{}', diharapkan {} '{}'",
                label,
                value,
                operator.label(),
                expected
            ));
        }
    }

    Ok(())
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
                    Ok(body) => check_json_assertion(
                        &body,
                        &cfg.json_path,
                        &cfg.expected_value,
                        &cfg.json_operator,
                    )
                    .err(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn lookup(value: serde_json::Value, path: &str) -> Option<String> {
        json_path_lookup(&value, path)
    }

    fn lookup_all(value: serde_json::Value, path: &str) -> Option<Vec<String>> {
        json_path_lookup_all(&value, path)
    }

    // --- json_path_lookup: objek & array dasar ---

    #[test]
    fn lookup_reads_nested_object_key() {
        let v = json!({"data": {"health": "ok"}});
        assert_eq!(lookup(v, "data.health"), Some("ok".to_string()));
    }

    #[test]
    fn lookup_reads_array_index_dot_notation() {
        let v = json!({"items": [{"state": "up"}, {"state": "down"}]});
        assert_eq!(lookup(v, "items.1.state"), Some("down".to_string()));
    }

    #[test]
    fn lookup_reads_index_on_root_array() {
        // Kasus dari laporan: respons berupa array di root, dulu harus
        // ditulis "0.status" dan itu memang satu-satunya bentuk yang jalan.
        let v = json!([{"status": "ok"}]);
        assert_eq!(lookup(v, "0.status"), Some("ok".to_string()));
    }

    #[test]
    fn lookup_returns_none_for_missing_path() {
        let v = json!({"data": {"health": "ok"}});
        assert_eq!(lookup(v.clone(), "data.missing"), None);
        assert_eq!(lookup(v, "items.5.state"), None);
    }

    // --- wildcard array (permintaan utama) ---

    #[test]
    fn wildcard_collects_all_elements() {
        let v = json!({"items": [{"state": "up"}, {"state": "down"}]});
        assert_eq!(
            lookup_all(v, "items[*].state"),
            Some(vec!["up".to_string(), "down".to_string()])
        );
    }

    #[test]
    fn wildcard_accepts_alternative_spellings() {
        let v = json!({"items": [{"state": "up"}, {"state": "down"}]});
        let expected = Some(vec!["up".to_string(), "down".to_string()]);
        assert_eq!(lookup_all(v.clone(), "items.*.state"), expected);
        assert_eq!(lookup_all(v.clone(), "items[].state"), expected);
    }

    #[test]
    fn wildcard_on_root_array() {
        let v = json!([{"status": "ok"}, {"status": "ok"}]);
        assert_eq!(
            lookup_all(v, "[*].status"),
            Some(vec!["ok".to_string(), "ok".to_string()])
        );
    }

    #[test]
    fn explicit_index_in_brackets_works() {
        let v = json!({"items": [{"state": "up"}, {"state": "down"}]});
        assert_eq!(lookup(v, "items[1].state"), Some("down".to_string()));
    }

    #[test]
    fn wildcard_skips_elements_missing_the_key() {
        let v = json!({"items": [{"state": "up"}, {"other": 1}, {"state": "down"}]});
        assert_eq!(
            lookup_all(v, "items[*].state"),
            Some(vec!["up".to_string(), "down".to_string()])
        );
    }

    #[test]
    fn wildcard_over_scalar_array() {
        let v = json!({"tags": ["a", "b", "c"]});
        assert_eq!(
            lookup_all(v, "tags[*]"),
            Some(vec!["a".to_string(), "b".to_string(), "c".to_string()])
        );
    }

    // --- operator perbandingan ---

    #[test]
    fn parse_operator_defaults_to_equals() {
        assert_eq!(JsonOperator::parse(""), JsonOperator::Equals);
        assert_eq!(JsonOperator::parse("=="), JsonOperator::Equals);
        assert_eq!(JsonOperator::parse("eq"), JsonOperator::Equals);
        // Nilai tak dikenal tidak boleh membuat monitor lama gagal.
        assert_eq!(JsonOperator::parse("operator-ngawur"), JsonOperator::Equals);
    }

    #[test]
    fn parse_operator_recognizes_all_forms() {
        assert_eq!(JsonOperator::parse("!="), JsonOperator::NotEquals);
        assert_eq!(JsonOperator::parse("contains"), JsonOperator::Contains);
        assert_eq!(JsonOperator::parse(">"), JsonOperator::GreaterThan);
        assert_eq!(JsonOperator::parse(">="), JsonOperator::GreaterOrEqual);
        assert_eq!(JsonOperator::parse("<"), JsonOperator::LessThan);
        assert_eq!(JsonOperator::parse("<="), JsonOperator::LessOrEqual);
        // Bentuk panjang & Bahasa Indonesia
        assert_eq!(JsonOperator::parse("gte"), JsonOperator::GreaterOrEqual);
        assert_eq!(JsonOperator::parse("mengandung"), JsonOperator::Contains);
    }

    #[test]
    fn equals_is_case_insensitive() {
        // "ok" harus cocok dengan "OK" — penyedia API sering tidak konsisten.
        assert!(JsonOperator::Equals.matches("OK", "ok").unwrap());
        assert!(JsonOperator::Equals.matches("ok", "ok").unwrap());
        assert!(!JsonOperator::Equals.matches("degraded", "ok").unwrap());
    }

    #[test]
    fn contains_matches_substring_not_prefix() {
        // Inti keluhan: "okay" tidak boleh dianggap cocok dengan "ok" pada ==,
        // tapi harus cocok pada contains.
        assert!(!JsonOperator::Equals.matches("okay", "ok").unwrap());
        assert!(JsonOperator::Contains.matches("okay", "ok").unwrap());
    }

    #[test]
    fn not_equals_and_not_contains() {
        assert!(JsonOperator::NotEquals.matches("down", "ok").unwrap());
        assert!(!JsonOperator::NotEquals.matches("ok", "ok").unwrap());
        assert!(JsonOperator::NotContains.matches("healthy", "error").unwrap());
        assert!(!JsonOperator::NotContains.matches("has error", "error").unwrap());
    }

    #[test]
    fn numeric_comparisons() {
        assert!(JsonOperator::GreaterThan.matches("91.5", "90").unwrap());
        assert!(!JsonOperator::GreaterThan.matches("89.9", "90").unwrap());
        assert!(JsonOperator::GreaterOrEqual.matches("90", "90").unwrap());
        assert!(JsonOperator::LessThan.matches("0", "1").unwrap());
        assert!(JsonOperator::LessOrEqual.matches("1", "1").unwrap());
    }

    #[test]
    fn numeric_comparison_rejects_non_numeric_values() {
        // Pesan harus menjelaskan masalahnya, bukan gagal diam-diam.
        let err = JsonOperator::GreaterThan.matches("ok", "90").unwrap_err();
        assert!(err.contains("bukan angka"), "pesan: {}", err);

        let err2 = JsonOperator::GreaterThan.matches("91", "banyak").unwrap_err();
        assert!(err2.contains("bukan angka"), "pesan: {}", err2);
    }

    // --- check_json_assertion end-to-end ---

    #[test]
    fn assertion_ok_on_matching_equals() {
        let body = r#"{"status":"ok"}"#;
        assert!(check_json_assertion(body, "status", "ok", "").is_ok());
    }

    #[test]
    fn assertion_fails_and_reports_operator() {
        let body = r#"{"status":"degraded"}"#;
        let err = check_json_assertion(body, "status", "ok", "").unwrap_err();
        assert!(err.contains("degraded"), "pesan: {}", err);
        assert!(err.contains("=="), "pesan harus memuat operator: {}", err);
    }

    #[test]
    fn assertion_with_contains_operator() {
        let body = r#"{"status":"okay-ish"}"#;
        assert!(check_json_assertion(body, "status", "okay", "contains").is_ok());
        assert!(check_json_assertion(body, "status", "ok", "").is_err());
    }

    #[test]
    fn assertion_with_numeric_threshold() {
        let body = r#"{"usage":91.5}"#;
        assert!(check_json_assertion(body, "usage", "90", ">").is_ok());
        assert!(check_json_assertion(body, "usage", "95", ">").is_err());
    }

    #[test]
    fn assertion_over_array_with_wildcard_requires_all_to_match() {
        // Semua elemen 'up' -> UP.
        let body = r#"{"items":[{"state":"up"},{"state":"up"}]}"#;
        assert!(check_json_assertion(body, "items[*].state", "up", "==").is_ok());

        // Satu elemen menyimpang -> DOWN, dan pesannya menyebut elemen ke berapa.
        let bad = r#"{"items":[{"state":"up"},{"state":"down"}]}"#;
        let err = check_json_assertion(bad, "items[*].state", "up", "==").unwrap_err();
        assert!(err.contains("nilai ke-2"), "pesan: {}", err);
    }

    #[test]
    fn assertion_wildcard_not_contains_detects_any_error() {
        // Pertanyaan "tidak boleh ada yang error" memakai not_contains.
        let clean = r#"{"items":[{"state":"up"},{"state":"up"}]}"#;
        assert!(check_json_assertion(clean, "items[*].state", "error", "not_contains").is_ok());

        let dirty = r#"{"items":[{"state":"up"},{"state":"error"}]}"#;
        assert!(check_json_assertion(dirty, "items[*].state", "error", "not_contains").is_err());
    }

    #[test]
    fn assertion_wildcard_all_elements_above_threshold() {
        let body = r#"{"hosts":[{"load":1.2},{"load":0.4}]}"#;
        assert!(check_json_assertion(body, "hosts[*].load", "0.3", ">").is_ok());
        // Elemen kedua di bawah ambang -> DOWN.
        assert!(check_json_assertion(body, "hosts[*].load", "0.5", ">").is_err());
    }

    #[test]
    fn assertion_empty_path_is_always_ok() {
        assert!(check_json_assertion("bukan json", "", "apa pun", "").is_ok());
    }

    #[test]
    fn assertion_empty_expected_only_requires_presence() {
        let body = r#"{"status":"apa pun"}"#;
        assert!(check_json_assertion(body, "status", "", "").is_ok());
        assert!(check_json_assertion(body, "tidak_ada", "", "").is_err());
    }

    #[test]
    fn assertion_rejects_invalid_json_body() {
        let err = check_json_assertion("<html>error</html>", "status", "ok", "").unwrap_err();
        assert!(err.contains("bukan JSON valid"), "pesan: {}", err);
    }

    #[test]
    fn assertion_reports_missing_path() {
        let body = r#"{"lain":"nilai"}"#;
        let err = check_json_assertion(body, "status", "ok", "").unwrap_err();
        assert!(err.contains("tidak ditemukan"), "pesan: {}", err);
    }
}
