<div align="center">
  <h1>⚡ UptimePulse</h1>
  <p><strong>Sistem pemantauan uptime self-hosted ultra-ringan berbasis Rust murni.</strong></p>
  <p>Dirancang untuk konsumsi resource minimal, keamanan memori, dan konkurensi tinggi pada perangkat berspesifikasi rendah seperti STB OpenWrt, Raspberry Pi, single-board ARM, dan server lokal.</p>

  <p>
    <a href="https://github.com/zidnyzd/uptime-pulse"><img src="https://img.shields.io/badge/Rust-2024_Edition-orange?logo=rust&logoColor=white" alt="Rust 2024"></a>
    <a href="https://github.com/tokio-rs/axum"><img src="https://img.shields.io/badge/Axum-0.8-blue?logo=tokio&logoColor=white" alt="Axum 0.8"></a>
    <a href="https://www.sqlite.org/"><img src="https://img.shields.io/badge/SQLite-WAL_Mode-003B57?logo=sqlite&logoColor=white" alt="SQLite WAL"></a>
    <a href="https://github.com/zidnyzd/uptime-pulse/pkgs/container/uptime-pulse"><img src="https://img.shields.io/badge/Arsitektur-ARM64%20|%20AMD64-blueviolet?logo=arm&logoColor=white" alt="Multi-Arch"></a>
    <a href="https://github.com/zidnyzd/uptime-pulse/pkgs/container/uptime-pulse"><img src="https://img.shields.io/badge/Ukuran_Image-~15.8_MB-brightgreen?logo=docker&logoColor=white" alt="Ukuran Container"></a>
    <img src="https://img.shields.io/badge/Konsumsi_RAM-~11_MB_RSS-success" alt="Konsumsi RAM">
    <a href="LICENSE"><img src="https://img.shields.io/badge/Lisensi-MIT-blue.svg" alt="Lisensi MIT"></a>
  </p>

  <p>
    <a href="README.md">English</a> | <strong>Bahasa Indonesia</strong>
  </p>
</div>

---

## 💡 Gambaran Umum

**UptimePulse** adalah alternatif ultra-ringan pengganti tool monitoring berbasis Node.js yang berat (seperti Uptime Kuma). Dibangun seutuhnya menggunakan **Rust** dengan runtime asinkron Tokio dan framework web Axum, UptimePulse berjalan dengan beban CPU yang sangat minim serta hanya mengonsumsi memori sekitar **~11 MB RAM (RSS)** pada beban kerja produksi.

Seluruh aset antarmuka frontend (HTML, CSS, JS) di-embed langsung ke dalam file biner menggunakan pustaka `rust-embed`, menghasilkan **satu file biner statis mandiri** tanpa membutuhkan dependensi runtime luar.

---

## ✨ Fitur Utama

- **Arsitektur Strict MVC:** Pemisahan struktur yang jelas antara `models`, `views`, dan `controllers` secara rapi dan modular.
- **Dukungan Multi-Protokol:** Mendukung pemantauan **ICMP Ping**, **HTTP / HTTPS** (didukung engine pure-Rust `rustls`), **HTTP JSON Query** (`http_json` memeriksa isi respons via `json_path` + `expected_value` yang harus sama persis), dan handshake **TCP Port**.
- **Request HTTP Terkonfigurasi:** Method per monitor (GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS), header kustom (`Nama: Nilai` per baris, CR/LF ditolak), dan request body (default `Content-Type: application/json`). Mengidentifikasi diri dengan `User-Agent: UptimePulse/<versi> (+URL repo)`, bisa diganti per monitor.
- **Engine Anti-False Alarm:** Multi-packet ping (`-c 2`) dengan toleransi jitter jaringan WAN, mekanisme retry bertahap yang fleksibel, dan *staggered scheduling* untuk mencegah lonjakan beban serentak (*thundering herd*).
- **Desain UI Terinspirasi Linear/Vercel:** Halaman status publik dengan daftar terpadu (*Unified Grouped List*) tanpa kotak berlebih, ribbon 90 micro-bar, palet dark warm charcoal (`#202020` / `#282828`), dan kontras light mode yang telah diaudit.
- **Kustomisasi Urutan Monitor Interaktif:** Pengaturan urutan kartu monitor via *HTML5 Drag & Drop* di desktop dengan grip handle 6 titik, serta tombol panah atas/bawah (↑ / ↓) yang responsif pada layar sentuh/mobile.
- **Perlindungan Memori Flash:** SQLite dengan mode WAL (*Write-Ahead Logging*), timeout busy 5000ms, downsampling harian (probe mentah disimpan 7 hari, agregat harian ikut `--retention` default 90), pembersihan otomatis (*auto-pruning*) per 6 jam dengan checkpoint WAL, dan `?vacuum=true` manual saja (VACUUM otomatis tidak pernah jalan, untuk menjaga umur eMMC).
- **Notifikasi Telegram Instan:** Pencatatan insiden otomatis, kalkulasi durasi gangguan, dukungan topik/thread forum, log file persisten (`alerts.log` di samping `--db`, rotasi 512 KB) karena syslog router hanya di RAM, dan format zona waktu global (WIB, WITA, WIT, GMT, dll.).
- **Keamanan Berlapis:** Proteksi *rate-limiting* in-memory (blokir 5 menit setelah 5x gagal login), *security headers* (`nosniff`, `SAMEORIGIN`, `strict-origin-when-cross-origin`), dan validasi password minimal 8 karakter.
- **Backup & Restore Penuh:** Ekspor dan impor data konfigurasi melalui file JSON terstruktur atau unduh langsung salinan mentah basis data SQLite (`.db`).

### Memilih tipe monitor

| Target | Gunakan | Alasan |
|---|---|---|
| Layanan web / API | `http` / `https` | Memeriksa status code asli (2xx/3xx = UP) |
| API JSON yang balas 200 saat error | `http_json` | Memastikan `json_path` sama dengan `expected_value` (harus sama persis); expected kosong berarti path harus ada |
| Host hidup, tanpa web server | `ping` | Hanya membuktikan reachability L3 |
| Port mentah (SSH, DB, kustom) | `tcp` | Membuktikan handshake ke `host:port` |
| Hostname di balik CDN/proxy | `http`, jangan `ping` | Ping hanya mengukur edge CDN, bukan origin Anda; origin bisa mati total sementara ping tetap 0% loss |

Batasan, disampaikan jujur: `http` biasa tidak memeriksa body; perbandingan `http_json` harus sama persis (tanpa mode mengandung); body error non-JSON (halaman maintenance HTML, error plain-text) belum ada assertion.

---

## 🚀 Panduan Memulai Cepat (Docker & Podman)

Image container multi-arsitektur yang mendukung **ARM64** dan **AMD64** otomatis di-build dan dipublikasikan ke GitHub Container Registry (`ghcr.io`).

### 1. Perintah Instan (Docker / Podman)

```bash
docker run -d \
  --name uptime-pulse \
  --restart unless-stopped \
  --cap-add NET_RAW \
  -p 3001:3001 \
  -v uptime-data:/data \
  ghcr.io/zidnyzd/uptime-pulse:latest
```

> **Catatan untuk pengguna Podman:** Cukup ganti perintah `docker` dengan `podman`. Ukuran image container sangat ramping, hanya **~15.8 MB**.

### 2. Docker Compose / Podman Compose

Gunakan file konfigurasi `compose.yaml`:

```yaml
services:
  uptime-pulse:
    image: ghcr.io/zidnyzd/uptime-pulse:latest
    container_name: uptime-pulse
    restart: unless-stopped
    ports:
      - "3001:3001"
    environment:
      - UPTIME_HOST=0.0.0.0
      - UPTIME_PORT=3001
      - UPTIME_DB_PATH=/data/uptime.db
      - UPTIME_RETENTION_DAYS=90
      # - ADMIN_PASSWORD=admin # Opsional: Ganti untuk mengubah password awal admin
    volumes:
      - uptime-data:/data
    cap_add:
      - NET_RAW # Diperlukan untuk socket ICMP ping non-root

volumes:
  uptime-data:
```

Jalankan layanan:
```bash
docker compose up -d
```

Akses aplikasi melalui peramban:
- **Halaman Status Publik:** `http://localhost:3001`
- **Panel Pengelola Admin:** `http://localhost:3001/admin` *(Login default: `admin` / `admin`)*

---

## 📦 Pemasangan di STB / Single Board Computer (ARM64)

### Opsi A: Container (Sangat Direkomendasikan)

Pada STB OpenWrt (misal FiberHome HG680-P, Amlogic S905X), Raspberry Pi, atau Armbian:

```bash
docker pull ghcr.io/zidnyzd/uptime-pulse:latest
docker run -d \
  --name uptime-pulse \
  --restart unless-stopped \
  --cap-add NET_RAW \
  -p 3001:3001 \
  -v /etc/uptime-pulse:/data \
  ghcr.io/zidnyzd/uptime-pulse:latest
```

### Opsi B: Standalone Static Binary (Tanpa Docker/Podman)

Unduh file binary statis `uptime-pulse-linux-arm64` dari halaman [GitHub Releases](../../releases):

```bash
chmod +x uptime-pulse-linux-arm64
./uptime-pulse-linux-arm64 --port 3001 --db /etc/uptime-pulse/uptime.db --retention 90 &
```

---

## ⚙️ Konfigurasi & Variabel Lingkungan

| Opsi CLI | Variabel Lingkungan | Default | Deskripsi |
|---|---|---|---|
| `-h, --host` | `UPTIME_HOST` | `0.0.0.0` | Alamat interface jaringan listen |
| `-p, --port` | `UPTIME_PORT` | `3001` | Port web server |
| `-d, --db` | `UPTIME_DB_PATH` | `uptime.db` | Jalur lokasi file database SQLite |
| `-r, --retention` | `UPTIME_RETENTION_DAYS` | `90` | Hari penyimpanan agregat harian sebelum dibersihkan otomatis (probe mentah selalu 7 hari) |
| `--password` | `ADMIN_PASSWORD` | `admin` | Password default admin jika belum terdaftar di database |

---

## 🛠️ Kompilasi Mandiri dari Source Code

Membutuhkan Rust versi 1.85+ (Edisi 2024).

```bash
git clone https://github.com/zidnyzd/uptime-pulse.git
cd uptime-pulse

# Kompilasi binary release yang dioptimasi
cargo build --release

# Menjalankan aplikasi secara lokal
./target/release/uptime-pulse --port 3001
```

Membangun image container lokal:
```bash
podman build -t uptime-pulse:local .
```

---

## 📂 Struktur Arsitektur Proyek

```text
uptime-pulse/
├── Cargo.toml               # Tokio, Axum 0.8, Rusqlite, Rustls, Rust-Embed
├── Dockerfile               # Multi-stage Alpine runtime (~15.8MB)
├── compose.yaml             # File deployment compose
├── src/
│   ├── main.rs              # Entry point aplikasi, runtime Tokio, graceful shutdown
│   ├── config.rs            # Parser konfigurasi CLI dan environment variable
│   ├── database.rs          # Pool SQLite, pragma WAL, migrasi otomatis & auto-pruning
│   ├── prober.rs            # ICMP multi-packet ping, HTTP(S) prober, TCP handshake
│   ├── engine.rs            # Scheduler background asinkron, staggered startup, fast retries
│   ├── models/              # Monitor, Heartbeat, Incident, Setting, User
│   ├── controllers/         # Controller Monitor, Publik, Auth, Pengaturan, Backup
│   ├── middlewares/         # Middleware sesi auth, login rate limiter, security headers
│   └── routes/              # Routing modular Axum
└── public/
    ├── index.html           # Tampilan halaman status publik
    ├── admin.html           # Tampilan dashboard admin
    ├── css/                 # Variabel tema base, warm dark charcoal, stylesheet publik & admin
    └── js/                  # Real-time SSE feed, drag & drop reorder, dukungan i18n
```

---

## 💖 Dukung Proyek Ini

Jika UptimePulse bermanfaat untuk Anda, pertimbangkan untuk mendukung pengembangannya:

<img src="docs/donate-usdt-bep20.jpg" alt="Donasi USDT via BEP20" width="220">

**USDT (BEP20):** `0x020333425b364d1337e0495c2423141d875414fd`

---

## 📄 Lisensi

Didistribusikan di bawah **Lisensi MIT**. Lihat file [LICENSE](LICENSE) untuk detail selengkapnya.

Dikembangkan oleh [Muhammad Zidny Ilhami](https://github.com/zidnyzd).
