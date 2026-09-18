# UptimePulse: Catatan Serah Terima

Dokumen ini adalah catatan status terkini proyek, riwayat perubahan, dan hal yang
masih tertunda. Disimpan di dalam repo supaya tidak hilang, karena `/tmp` adalah
tmpfs yang bisa terhapus kapan saja.

Terakhir diperbarui: 19 September 2026

---

## 1. Status Terkini

| Item | Nilai |
|---|---|
| Repo | https://github.com/zidnyzd/uptime-pulse |
| Branch | `master` (bersih, sinkron dengan origin) |
| Commit terakhir | `39ea352` |
| Rilis terbaru | `v0.1.4` |
| Halaman publik | https://status.zidstore.net |
| Perangkat produksi | STB `192.168.1.2` (OpenWrt, aarch64) |
| Binary produksi | `/usr/bin/uptime-pulse` md5 `b894a3f37a9f88cc6bdad62e2f4b176c` |
| Database produksi | `/etc/uptime-pulse/uptime.db` (btrfs, permanen) |
| Log alert | `/etc/uptime-pulse/alerts.log` |
| Monitor aktif | 17 (16 publik, 1 privat) |
| Status live | `operational`, 16/16 |

### Alur deploy

1. Commit dan push ke `master`.
2. Push tag `v*.*.*` untuk memicu job rilis.
3. CI membangun binary statis musl untuk amd64 dan arm64.
4. Ambil artifact arm64, lalu pasang ke STB secara atomik:
   backup binary dan database, unggah ke `/tmp`, verifikasi md5, tukar, restart,
   lalu pastikan PID berubah.
5. Frontend di-embed lewat `rust-embed`, jadi setiap perubahan di `public/`
   wajib membangun ulang binary.

---

## 2. Riwayat Perubahan

### v0.1.4: User-Agent, HTTP JSON Query, perbaikan modal

Menutup tiga saran lanjutan di issue #1.

**User-Agent.** Sebelumnya request HTTP tidak mengirim User-Agent sama sekali
sehingga di access log server tujuan muncul sebagai `-`. Sekarang setiap request
mengirim:

```
User-Agent: UptimePulse/0.1.4 (+https://github.com/zidnyzd/uptime-pulse)
```

Bisa diganti per monitor lewat Custom Header `User-Agent`. Versi di `Cargo.toml`
diselaraskan ke 0.1.4 karena sebelumnya tertinggal di 0.1.0 meski tag sudah
v0.1.3.

**Tipe monitor baru: HTTP JSON Query (`http_json`).** Menutup kasus "HTTP 200
tetapi isi respons menandakan error". Dua field baru:

- `json_path`: jalur ke nilai di dalam JSON, dot notation, mendukung indeks
  array. Contoh `status`, `data.health`, `items.0.state`.
- `expected_value`: nilai yang diharapkan. Kosong berarti cukup memastikan
  jalurnya ada.

Perbandingan bersifat persis (bukan pencarian sebagian) dan dilakukan sebagai
teks, sehingga angka, boolean, `null`, dan string semuanya bisa dicocokkan.
Tipe `http` biasa tidak terpengaruh.

**Perbaikan modal di layar kecil.** Overlay memakai `align-items: center` tanpa
overflow, sehingga modal yang lebih tinggi dari layar terpotong ke atas dan ke
bawah dan tidak bisa dijangkau sama sekali. Diperparah oleh panel HTTP yang
ditambahkan di v0.1.3 (tinggi modal 518 px menjadi 880 px pada layar 320x568).
Perbaikan: overlay menjadi area scroll, modal memakai `margin: auto`.

### v0.1.3: HTTP method, custom headers, request body

Menutup saran pertama di issue #1. Sebelumnya method di-hardcode `GET` di
`prober.rs` sehingga endpoint non-GET dan API ber-token tidak bisa dipantau.

- Kolom baru `method`, `headers`, `body` dengan migrasi otomatis.
- Tujuh method: GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS.
- Header kustom satu per baris, format `Nama: Nilai`.
- Body dengan `Content-Type: application/json` otomatis bila belum diisi.
- Parser header menolak nama non-token dan nilai ber-CR/LF (anti header
  injection).
- Field `timeout` yang sebelumnya di-hardcode 10 detik di form ikut diperbaiki.

### v0.1.2: Logging persisten alert

Log OpenWrt disimpan di RAM dan ter-rotate cepat. Terukur: buffer `logread`
hanya bertahan sekitar 3 menit karena NextDNS sangat verbose (183 dari 578 baris
adalah log DNS), sehingga kegagalan alert tidak meninggalkan jejak apa pun.

- Modul `src/alert_log.rs`: catat `OK` / `ERROR` / `SKIP` setiap percobaan kirim.
- Lokasi diturunkan dari `--db`, jadi `/etc/uptime-pulse/alerts.log` di STB.
- Rotasi pada 512 KB, total dibatasi sekitar 1 MB, karena target adalah eMMC
  dengan umur tulis terbatas. Hanya ditulis saat status berubah, bukan per probe.
- Endpoint `GET /api/alerts/log` dan panel di view Backup admin.
- Perbaikan bug: endpoint test Telegram melaporkan "berhasil" (HTTP 200) padahal
  token atau chat_id kosong dan tidak ada pesan terkirim. Sekarang 400.

### v0.1.1: Perbaikan audit halaman publik

Tujuh temuan audit. Yang paling penting:

1. Insiden dari monitor privat atau paused tidak lagi bocor ke halaman publik.
   Solusinya parameter `public_only` pada query, bukan filter mentah, karena
   `admin.js` juga membaca endpoint yang sama.
2. Prune per-insert dihapus (sekitar 23.000 query DELETE per hari yang hampir
   selalu menghapus 0 baris) dan index `idx_hb_time` ditambahkan agar prune
   global tidak melakukan full table scan.
3. Jam footer memakai `formatToParts` dengan `hourCycle: 'h23'`.
4. Ambang amber disatukan ke satu helper `bucket_status`.
5. Throttle SSE 10 detik untuk mencegah refetch storm.
6. `escapeHtml` juga meng-escape kutip di `public.js` dan `admin.js`.
7. Fitur hero tiga status dilengkapi, bukan dihapus.

---

## 3. Catatan Operasional

### Memilih tipe monitor: Cloudflare versus server langsung

Untuk domain yang berada di balik Cloudflare, gunakan **HTTP(S)**, bukan ping.
Ping ke domain Cloudflare hanya mengukur edge terdekat, bukan server asli.
Origin bisa mati total sementara ping tetap 0% loss.

Bukti dari STB:

```
id.zidstore.net   (Cloudflare) -> 104.20.17.32   ping OK    HTTPS 200
i.id.zidstore.net (origin)     -> 103.191.92.38  ping GAGAL
```

Cara cepat memeriksa: `whois <IP>`, kalau muncul `CLOUDFLARENET` berarti di
balik Cloudflare.

Untuk server langsung yang tidak menjalankan web server, ping lebih tepat.
Contoh di produksi: `i.idX.zidstore.net` gagal HTTP tetapi merespons ping.

### Telegram

`api.telegram.org` di-resolve NextDNS menjadi IPv6, sedangkan STB tidak punya
IPv6 yang berfungsi, sehingga pengiriman alert gagal sekitar 60%. Diperbaiki
dengan `filter_aaaa` di dnsmasq. Hasil: 2/5 menjadi 5/5, dan sejak itu
`alerts.log` mencatat semua pengiriman berhasil.

Perintah yang dipakai:

```sh
uci set dhcp.@dnsmasq[0].filter_aaaa='1'
uci commit dhcp
/etc/init.d/dnsmasq restart
```

Backup konfigurasi sebelum perubahan ada di STB:
`/tmp/dhcp-backup-before-aaaa.txt`. Perlu ditinjau ulang kalau IPv6 diaktifkan
di upstream.

### Vantage point prober

Jika prober melaporkan DOWN padahal target sehat, periksa jalur jaringan host
prober lebih dulu. Bandingkan dengan layanan pemeriksa eksternal multi-node.
Kalau loss hanya terjadi lokal, masalahnya peering ISP, bukan server atau kode.

---

## 4. Yang Masih Tertunda

| Prioritas | Item | Catatan |
|---|---|---|
| Rendah | Balasan issue #1 untuk v0.1.4 | Draf sudah disiapkan, tinggal dikirim |
| Rendah | Keyword check untuk tipe `http` biasa | Sudah tertutup oleh `http_json` bila target mengembalikan JSON |
| Rendah | 14 warning clippy lama | Semuanya pre-existing, bukan dari perubahan terakhir |
| Pantau | `SG NEWMEDIA` uptime sekitar 99.6% | Belum diselidiki apakah sisa masalah lama atau baru |
| Pantau | Ukuran file WAL | Checkpoint tiap 6 jam, pernah lebih besar dari file DB |

---

## 5. Verifikasi yang Sudah Dilakukan

- **JSON Query**: 16/16 skenario (cocok, tidak cocok, HTTP 200 dengan body
  error, bukan JSON, path hilang, nested, indeks array, angka, boolean, null,
  objek kosong, dan tipe `http` yang harus tetap UP).
- **User-Agent**: terkirim di 20/20 request, bisa di-override. Dibuktikan di
  hardware STB lewat `httpbin.org`: nilai yang diterima persis
  `UptimePulse/0.1.4 (+https://github.com/zidnyzd/uptime-pulse)`.
- **Migrasi database**: DB lama mendapat kolom baru, monitor yang ada tidak
  berubah perilakunya (semua tetap `GET`).
- **Backup dan restore**: field baru ikut diekspor dan dipulihkan, file backup
  versi lama tetap bisa direstore.
- **Keamanan**: header dan body yang memuat kredensial tidak pernah muncul di
  `/api/public/summary`. Diuji dengan monitor publik yang memuat token unik.
- **Modal**: bisa di-scroll pada 320x480, 320x568, 360x640, 375x667, 390x844,
  414x896, 768x1024, dan 1440x900.
- **clippy**: tetap 14 warning, tidak ada yang baru.
