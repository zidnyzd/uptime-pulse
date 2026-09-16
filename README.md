# UptimePulse

<div align="center">
  <h3>Ultra-Lightweight Self-Hosted Uptime Monitoring System</h3>
  <p>Built with pure Rust (Axum, Tokio, Rusqlite, Rustls). Designed for high performance on low-resource hardware like OpenWrt STB ARM64, Raspberry Pi, and local servers.</p>
</div>

---

## Highlights

* **Ultra-Low Resource Usage:** Uses only ~11 MB RAM (RSS) and compiles to a single static binary (~5.5 MB) with all HTML, CSS, and JS embedded via `rust-embed`.
* **Multi-Arch Support:** Fully supports **ARM64 (aarch64)** and **AMD64 (x86_64)**.
* **Modern Clean UI:** Unified Grouped List status page (Linear/Vercel style), dark warm charcoal palette (`#202020` / `#282828`), audited light mode contrast, and responsive mobile view.
* **Anti-False Alarm Engine:** Multi-packet ICMP ping (`-c 2`) with WAN jitter tolerance, staggered scheduling, and customizable fast-retries before declaring downtime.
* **Custom Monitor Ordering:** Native HTML5 Drag & Drop reordering on desktop with responsive up/down touch buttons on mobile.
* **Telegram Notifications:** Instant alerts with duration calculation and global timezone formatting (WIB, WITA, WIT, GMT, etc.).
* **Flash Storage Safe:** SQLite WAL mode with 5000ms busy timeout, auto-pruning every 6 hours, and automatic WAL checkpoint truncation to minimize flash wear on routers.
* **Security Hardened:** Built-in in-memory login rate-limiting (5 failed attempts / 5 mins), security headers (`nosniff`, `SAMEORIGIN`, `referrer-policy`), and minimum 8-character passwords.

---

## Quick Start (Docker / Podman)

UptimePulse images are published to GitHub Container Registry (`ghcr.io`) for both `linux/amd64` and `linux/arm64`.

### 1. One-Liner Run (Docker / Podman)

```bash
docker run -d \
  --name uptime-pulse \
  --restart unless-stopped \
  --cap-add NET_RAW \
  -p 3001:3001 \
  -v uptime-data:/data \
  ghcr.io/zidnyzd/uptime-pulse:latest
```

> **Catatan Podman:** Ganti `docker` dengan `podman`. Volume `uptime-data` akan menyimpan file database `uptime.db` secara persisten.

### 2. Docker Compose / Podman Compose

Gunakan file `compose.yaml`:

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
      # - ADMIN_PASSWORD=admin # Opsional: Override password awal admin
    volumes:
      - uptime-data:/data
    cap_add:
      - NET_RAW # Diperlukan untuk socket ICMP ping

volumes:
  uptime-data:
```

Jalankan dengan:
```bash
docker compose up -d
```

Buka peramban:
- **Status Publik:** `http://localhost:3001`
- **Panel Admin:** `http://localhost:3001/admin` (Kredensial default: `admin` / `admin`)

---

## Deployment di STB OpenWrt (ARM64)

### Opsi A: Container via Docker / Podman (Rekomendasi)

Jika STB OpenWrt HG680-P atau Armbian Anda sudah memiliki `docker` atau `podman`:
```bash
docker pull ghcr.io/zidnyzd/uptime-pulse:latest
docker run -d --name uptime-pulse --restart unless-stopped --cap-add NET_RAW -p 3001:3001 -v /etc/uptime-pulse:/data ghcr.io/zidnyzd/uptime-pulse:latest
```

### Opsi B: Standalone Binary (Tanpa Container)

Unduh binary static `uptime-pulse-linux-arm64` dari [Releases](../../releases), beri izin eksekusi, lalu jalankan sebagai service:
```bash
chmod +x uptime-pulse-linux-arm64
./uptime-pulse-linux-arm64 --port 3001 --db /etc/uptime.db --retention 90 &
```

---

## Konfigurasi CLI & Environment Variables

| Opsi CLI | Environment Variable | Default | Deskripsi |
|---|---|---|---|
| `-h, --host` | `UPTIME_HOST` | `0.0.0.0` | Alamat host listen |
| `-p, --port` | `UPTIME_PORT` | `3001` | Port web server |
| `-d, --db` | `UPTIME_DB_PATH` | `uptime.db` | Jalur file database SQLite |
| `-r, --retention`| `UPTIME_RETENTION_DAYS` | `90` | Batas hari retensi log riwayat probe |
| `--password` | `ADMIN_PASSWORD` | `admin` | Password default akun admin jika belum ada |

---

## Build Mandiri dari Sumber (Local Build)

Prasyarat: Rust 1.85+ (Edition 2024).

```bash
git clone https://github.com/zidnyzd/uptime-pulse.git
cd uptime-pulse

# Build binary rilis yang dioptimasi
cargo build --release

# Menjalankan binary
./target/release/uptime-pulse --port 3001
```

Build container lokal dengan Podman / Docker:
```bash
podman build -t uptime-pulse:local .
```

---

## Lisensi

Didistribusikan di bawah lisensi MIT.
Dibuat oleh [Muhammad Zidny Ilhami](https://github.com/zidnyzd).
