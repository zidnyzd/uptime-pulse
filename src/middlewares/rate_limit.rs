use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

// Kebijakan anti brute-force: 5x login gagal dalam 5 menit -> IP diblokir 5 menit
const MAX_FAILS: u32 = 5;
const WINDOW: Duration = Duration::from_secs(300);
const BLOCK_FOR: Duration = Duration::from_secs(300);

struct Entry {
    fails: u32,
    window_start: Instant,
    blocked_until: Option<Instant>,
}

// Rate limiter login in-memory per IP (cukup untuk single-instance di STB/OpenWrt)
pub struct LoginRateLimiter {
    inner: Mutex<HashMap<IpAddr, Entry>>,
}

impl LoginRateLimiter {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    // Ok(()) = boleh lanjut; Err(sisa_detik) = IP masih diblokir
    pub async fn check(&self, ip: &IpAddr) -> Result<(), u64> {
        let map = self.inner.lock().await;
        if let Some(e) = map.get(ip) {
            if let Some(until) = e.blocked_until {
                let now = Instant::now();
                if now < until {
                    return Err(until.duration_since(now).as_secs().max(1));
                }
            }
        }
        Ok(())
    }

    // Catat login gagal; kunci IP jika melewati ambang
    pub async fn record_fail(&self, ip: &IpAddr) {
        let mut map = self.inner.lock().await;
        let now = Instant::now();
        let e = map.entry(*ip).or_insert(Entry {
            fails: 0,
            window_start: now,
            blocked_until: None,
        });
        // Masa blokir sudah lewat -> mulai hitungan baru
        if e.blocked_until.map(|u| now >= u).unwrap_or(false) {
            e.fails = 0;
            e.blocked_until = None;
            e.window_start = now;
        } else if now.duration_since(e.window_start) > WINDOW {
            // Jendela hitung kedaluwarsa -> reset
            e.fails = 0;
            e.window_start = now;
        }
        e.fails += 1;
        if e.fails >= MAX_FAILS {
            e.blocked_until = Some(now + BLOCK_FOR);
        }
        // Bersihkan entri basi agar HashMap tidak tumbuh tanpa batas
        if map.len() > 1024 {
            map.retain(|_, v| {
                v.blocked_until.map(|u| now < u).unwrap_or(false)
                    || now.duration_since(v.window_start) <= WINDOW
            });
        }
    }

    // Login sukses -> hapus catatan gagal IP ini
    pub async fn record_success(&self, ip: &IpAddr) {
        self.inner.lock().await.remove(ip);
    }
}
