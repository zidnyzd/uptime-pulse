use chrono::Local;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

// Log persisten untuk percobaan pengiriman alert (Telegram dll).
//
// Kenapa perlu: log OpenWrt disimpan di RAM (tmpfs) dan ter-rotate cepat, sehingga
// kegagalan alert tidak meninggalkan jejak apa pun. Untuk alat monitoring ini
// berbahaya: alert bisa gagal terkirim dan tidak ada cara mengetahuinya.
//
// Kenapa ada rotasi: perangkat target (STB OpenWrt) memakai eMMC dengan umur tulis
// terbatas. File dibatasi 512 KB dan hanya menyimpan 1 file lama (total maks 1 MB),
// dan hanya ditulis saat terjadi perubahan status (jarang), bukan per probe.

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

// Batas ukuran satu file log sebelum dirotasi.
const MAX_BYTES: u64 = 512 * 1024;

/// Inisialisasi path log, diturunkan dari lokasi database agar otomatis
/// mengikuti `--db` (mis. `/etc/uptime-pulse/uptime.db` -> `/etc/uptime-pulse/alerts.log`).
pub fn init(db_path: &str) {
    let path = match Path::new(db_path).parent() {
        Some(p) if !p.as_os_str().is_empty() => p.join("alerts.log"),
        _ => PathBuf::from("alerts.log"),
    };
    let _ = LOG_PATH.set(path);
}

/// Mengembalikan path log (untuk ditampilkan di API/dashboard).
pub fn path() -> Option<String> {
    LOG_PATH.get().map(|p| p.display().to_string())
}

/// Menulis satu baris log. Best-effort: kegagalan menulis TIDAK boleh
/// mengganggu alur pengiriman alert.
pub fn log(level: &str, msg: &str) {
    let Some(path) = LOG_PATH.get() else {
        return;
    };

    rotate_if_needed(path);

    let ts = Local::now().format("%Y-%m-%d %H:%M:%S");
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        // Buang newline agar satu entri tetap satu baris.
        let clean = msg.replace('\n', " ").replace('\r', "");
        let _ = writeln!(f, "{} [{}] {}", ts, level, clean);
    }
}

/// Merotasi file bila melewati batas ukuran: `alerts.log` -> `alerts.log.1`.
/// File `.1` lama ditimpa, sehingga total pemakaian dibatasi ~2x MAX_BYTES.
fn rotate_if_needed(path: &Path) {
    let over_limit = fs::metadata(path)
        .map(|m| m.len() > MAX_BYTES)
        .unwrap_or(false);
    if over_limit {
        let old = path.with_extension("log.1");
        let _ = fs::remove_file(&old);
        let _ = fs::rename(path, &old);
    }
}

/// Membaca `max_lines` baris terakhir dari log (untuk endpoint admin).
/// Membaca dari file aktif, dan menyambung file rotasi bila baris masih kurang.
pub fn tail(max_lines: usize) -> Vec<String> {
    let Some(path) = LOG_PATH.get() else {
        return Vec::new();
    };

    let mut lines: Vec<String> = Vec::new();

    // File rotasi lebih lama, jadi dibaca lebih dulu.
    let old = path.with_extension("log.1");
    if let Ok(content) = fs::read_to_string(&old) {
        lines.extend(content.lines().map(|s| s.to_string()));
    }
    if let Ok(content) = fs::read_to_string(path) {
        lines.extend(content.lines().map(|s| s.to_string()));
    }

    if lines.len() > max_lines {
        lines.split_off(lines.len() - max_lines)
    } else {
        lines
    }
}

/// Ukuran file log dalam byte (file aktif + rotasi), untuk ditampilkan di UI.
pub fn size_bytes() -> u64 {
    let Some(path) = LOG_PATH.get() else {
        return 0;
    };
    let a = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let b = fs::metadata(path.with_extension("log.1"))
        .map(|m| m.len())
        .unwrap_or(0);
    a + b
}
