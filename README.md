<div align="center">
  <h1>⚡ UptimePulse</h1>
  <p><strong>Ultra-lightweight self-hosted uptime monitoring system built with pure Rust.</strong></p>
  <p>Engineered for minimal footprint, memory safety, and high concurrency on low-resource hardware such as OpenWrt STBs, Raspberry Pi, ARM boards, and local servers.</p>

  <p>
    <a href="https://github.com/zidnyzd/uptime-pulse"><img src="https://img.shields.io/badge/Rust-2024_Edition-orange?logo=rust&logoColor=white" alt="Rust 2024"></a>
    <a href="https://github.com/tokio-rs/axum"><img src="https://img.shields.io/badge/Axum-0.8-blue?logo=tokio&logoColor=white" alt="Axum 0.8"></a>
    <a href="https://www.sqlite.org/"><img src="https://img.shields.io/badge/SQLite-WAL_Mode-003B57?logo=sqlite&logoColor=white" alt="SQLite WAL"></a>
    <a href="https://github.com/zidnyzd/uptime-pulse/pkgs/container/uptime-pulse"><img src="https://img.shields.io/badge/Arch-ARM64%20|%20AMD64-blueviolet?logo=arm&logoColor=white" alt="Multi-Arch"></a>
    <a href="https://github.com/zidnyzd/uptime-pulse/pkgs/container/uptime-pulse"><img src="https://img.shields.io/badge/Container_Image-~15.8_MB-brightgreen?logo=docker&logoColor=white" alt="Container Size"></a>
    <img src="https://img.shields.io/badge/Memory-~11_MB_RSS-success" alt="RAM Usage">
    <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="MIT License"></a>
  </p>

  <p>
    <strong>English</strong> | <a href="README.id.md">Bahasa Indonesia</a>
  </p>
</div>

---

## 💡 Overview

**UptimePulse** is an ultra-lightweight, drop-in alternative to resource-heavy Node.js-based monitoring tools like Uptime Kuma. Written completely in **Rust** using an asynchronous Tokio runtime and Axum web framework, UptimePulse runs with negligible CPU load and consumes only **~11 MB of RAM** under production workloads.

Frontend web views (HTML, CSS, JS) are bundled directly into the executable using `rust-embed`, delivering a **single static binary** with zero external runtime dependencies.

---

## ✨ Features

- **Strict MVC Architecture:** Clean separation of concerns across `models`, `views`, and `controllers` in idiomatic Rust.
- **Multi-Protocol Monitoring:** Supports **ICMP Ping**, **HTTP / HTTPS** (powered by pure-Rust `rustls`), **HTTP JSON Query** (`http_json` asserts on response body via `json_path` + exact-match `expected_value`), and **TCP port** handshakes.
- **Configurable HTTP Requests:** Per-monitor method (GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS), custom headers (`Name: Value` per line, CR/LF rejected), and request body (defaults to `Content-Type: application/json`). Identifies itself with `User-Agent: UptimePulse/<version> (+repo URL)`, overridable per monitor.
- **Advanced Anti-False Alarm Engine:** Multi-packet ping (`-c 2`) with WAN jitter tolerance, customizable retries, and staggered scheduling to prevent thundering herd spikes.
- **Linear/Vercel-Inspired UI:** Borderless Unified Grouped List status page, 90-bar fine micro-timeline, warm dark charcoal theme (`#202020` / `#282828`), and contrast-audited light mode.
- **Interactive Monitor Reordering:** Native HTML5 Drag & Drop ordering on desktop with dedicated 6-dot grip handles, alongside responsive up/down touch arrow buttons on mobile devices.
- **Flash Storage Safety:** SQLite with WAL mode, 5000ms busy timeout, daily downsampling (raw probes kept 7 days, daily aggregates follow `--retention`, default 90), automatic pruning every 6 hours with WAL checkpoint, and manual `?vacuum=true` only (auto VACUUM never runs, to protect eMMC lifespan).
- **Instant Telegram Alerts:** Automated incident logging, recovery duration tracking, forum topic/thread support, persistent file log (`alerts.log` next to `--db`, 512 KB rotation) because router syslog is RAM-only, and dynamic global timezone formatting (WIB, WITA, WIT, GMT, etc.).
- **Security Hardened:** Built-in in-memory rate limiting (5 failed attempts / 5 mins), security headers (`nosniff`, `SAMEORIGIN`, `strict-origin-when-cross-origin`), and minimum 8-character password enforcement.
- **Full Backup & Restore:** Export and import system state via structured JSON or download raw SQLite `.db` snapshots.

### Choosing a monitor type

| Target | Use | Why |
|---|---|---|
| Web service / API | `http` / `https` | Checks real status code (2xx/3xx = UP) |
| JSON API that returns 200 on errors | `http_json` | Asserts `json_path` equals `expected_value` (exact match); empty expected means path must exist |
| Host alive, no web server | `ping` | Proves L3 reachability only |
| Raw port (SSH, DB, custom) | `tcp` | Proves handshake on `host:port` |
| Hostname behind a CDN/proxy | `http`, never `ping` | Ping measures the CDN edge, not your origin; the origin can be fully down while ping reports 0% loss |

Limits, stated honestly: plain `http` does not inspect the body; `http_json` comparison is exact (no contains mode); non-JSON error bodies (HTML maintenance page, plain-text error) have no assertion yet.

---

## 🚀 Quick Start (Docker & Podman)

Multi-architecture container images supporting both **ARM64** and **AMD64** are automatically built and published to GitHub Container Registry (`ghcr.io`).

### 1. One-Liner Command

```bash
docker run -d \
  --name uptime-pulse \
  --restart unless-stopped \
  --cap-add NET_RAW \
  -p 3001:3001 \
  -v uptime-data:/data \
  ghcr.io/zidnyzd/uptime-pulse:latest
```

> **Note for Podman users:** Simply replace `docker` with `podman`. The container image is only **~15.8 MB**.

### 2. Docker Compose / Podman Compose

Use the provided `compose.yaml`:

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
      # - ADMIN_PASSWORD=admin # Optional: override initial default admin password
    volumes:
      - uptime-data:/data
    cap_add:
      - NET_RAW # Required for non-root ICMP ping sockets

volumes:
  uptime-data:
```

Start the service:
```bash
docker compose up -d
```

Access the web interface:
- **Public Status Page:** `http://localhost:3001`
- **Admin Console:** `http://localhost:3001/admin` *(Default login: `admin` / `admin`)*

---

## 📦 Deployment on STB / Single Board Computers (ARM64)

### Option A: Container (Recommended)

On OpenWrt STB (e.g. FiberHome HG680-P, Amlogic S905X), Raspberry Pi, or Armbian:

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

### Option B: Standalone Static Binary (No Docker Required)

Download the static `uptime-pulse-linux-arm64` binary from [GitHub Releases](../../releases):

```bash
chmod +x uptime-pulse-linux-arm64
./uptime-pulse-linux-arm64 --port 3001 --db /etc/uptime-pulse/uptime.db --retention 90 &
```

---

## ⚙️ Configuration & Environment Variables

| CLI Flag | Environment Variable | Default | Description |
|---|---|---|---|
| `-h, --host` | `UPTIME_HOST` | `0.0.0.0` | Listening network interface address |
| `-p, --port` | `UPTIME_PORT` | `3001` | Web server port |
| `-d, --db` | `UPTIME_DB_PATH` | `uptime.db` | File path to SQLite database |
| `-r, --retention` | `UPTIME_RETENTION_DAYS` | `90` | Days to keep daily aggregates before auto-pruning (raw probes always 7 days) |
| `--password` | `ADMIN_PASSWORD` | `admin` | Initial admin password if not already configured in database |

---

## 🛠️ Building from Source

Requires Rust 1.85+ (Edition 2024).

```bash
git clone https://github.com/zidnyzd/uptime-pulse.git
cd uptime-pulse

# Compile optimized release binary
cargo build --release

# Run locally
./target/release/uptime-pulse --port 3001
```

Build local container image:
```bash
podman build -t uptime-pulse:local .
```

---

## 📂 Project Architecture

```text
uptime-pulse/
├── Cargo.toml               # Tokio, Axum 0.8, Rusqlite, Rustls, Rust-Embed
├── Dockerfile               # Multi-stage Alpine runtime (~15.8MB)
├── compose.yaml             # Compose deployment file
├── src/
│   ├── main.rs              # App entry point, CLI config, Tokio runtime, graceful shutdown
│   ├── config.rs            # CLI and environment variable parser
│   ├── database.rs          # SQLite pool, WAL pragmas, automated migrations & pruning
│   ├── prober.rs            # ICMP multi-packet ping, HTTP(S) prober, TCP handshake
│   ├── engine.rs            # Async background scheduler, staggered startup, fast retries
│   ├── models/              # Monitor, Heartbeat, Incident, Setting, User
│   ├── controllers/         # Monitor, Public, Auth, Setting, Backup controllers
│   ├── middlewares/         # Session auth, login rate limiter, security headers
│   └── routes/              # Modular Axum routing table
└── public/
    ├── index.html           # Public status page view
    ├── admin.html           # Admin management dashboard
    ├── css/                 # Base variables, warm dark theme, public & admin stylesheets
    └── js/                  # Real-time SSE feeds, drag & drop reorder, i18n support
```

---

## 💖 Support This Project

If UptimePulse is useful for you, consider supporting its development:

<img src="docs/donate-usdt-bep20.jpg" alt="Donate USDT via BEP20" width="220">

**USDT (BEP20):** `0x020333425b364d1337e0495c2423141d875414fd`

---

## 📄 License

Distributed under the **MIT License**. See [LICENSE](LICENSE) for details.

Developed by [Muhammad Zidny Ilhami](https://github.com/zidnyzd).
