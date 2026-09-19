# UptimePulse: Catatan Operasional

Dokumen ini berisi catatan operasional, riwayat perubahan, dan pelajaran yang
didapat dari menjalankan UptimePulse di perangkat nyata. Tujuannya agar siapa pun
yang men-deploy proyek ini tidak mengulangi masalah yang sama.

Contoh di dokumen ini memakai alamat generik (`example.com`, dan rentang IP
dokumentasi RFC 5737) supaya tidak memuat detail infrastruktur siapa pun.

---

## Status Rilis

| Item | Nilai |
|---|---|
| Versi terbaru | `v0.1.6` |
| Commit `master` | `d421d0b` |
| Terakhir diperbarui | 19 September 2026 |

Catatan kondisi lengkap (perangkat, PID, jalur database, daftar backup, dan
hal yang masih tertunda) sengaja tidak ada di sini karena memuat detail
infrastruktur. Berkas itu ada di `.hermes/HANDOVER.local.md` yang sudah masuk
`.gitignore`, jadi tidak ikut ke repo publik ini.

---

## 1. Alur Deploy

UptimePulse adalah binary statis tunggal, jadi deployment tidak memerlukan
runtime apa pun di sisi target.

1. Commit dan push ke `master`.
2. Push tag `v*.*.*` untuk memicu job rilis.
3. CI membangun binary statis musl untuk `amd64` dan `arm64` (target
   `aarch64-unknown-linux-musl`).
4. Ambil artifact `arm64`, lalu pasang secara atomik ke perangkat target.

### Pemasangan atomik (dengan rollback)

Jangan menimpa binary yang sedang berjalan secara langsung. Urutannya:

```sh
# 1. Unggah ke lokasi sementara, jangan ke /usr/bin
scp uptime-pulse-linux-arm64 root@TARGET:/tmp/uptime-pulse-new

# 2. Verifikasi integritas SEBELUM menukar
ssh root@TARGET 'md5sum /tmp/uptime-pulse-new'   # harus sama dengan lokal

# 3. Cadangkan binary dan database
ssh root@TARGET 'cp /usr/bin/uptime-pulse /usr/bin/uptime-pulse.bak-$(date +%Y%m%d-%H%M)'
ssh root@TARGET 'cp /etc/uptime-pulse/uptime.db /etc/uptime-pulse/backup/uptime.db.pre-$(date +%Y%m%d-%H%M)'

# 4. Tukar dan restart
ssh root@TARGET 'cp /tmp/uptime-pulse-new /usr/bin/uptime-pulse && chmod 755 /usr/bin/uptime-pulse'
ssh root@TARGET '/etc/init.d/uptime-pulse restart'
```

**Pastikan PID berubah**, jangan percaya keluaran perintah restart saja. Sebuah
supervisor bisa melaporkan `running` sementara proses lama masih melayani.
Bandingkan PID sebelum dan sesudah.

### Catatan penting

- **Frontend di-embed lewat `rust-embed`.** Setiap perubahan di `public/` wajib
  membangun ulang binary. Mengedit berkas di disk tidak berpengaruh pada layanan
  yang sedang berjalan.
- **Cek ruang penyimpanan sebelum menukar.** Perangkat dengan `/overlay` hampir
  penuh bisa gagal menyalin.
- **Backup database perlu checkpoint.** Dalam mode WAL, transaksi terbaru ada di
  berkas `-wal`. Menyalin `.db` saja menghasilkan backup yang basi.

---

## 2. Riwayat Perubahan

### v0.1.6: Assertion JSON lanjutan, zona waktu notifikasi, umpan balik simpan

Empat perbaikan yang semuanya berasal dari laporan pengguna di issue #1.

**Assertion JSON: wildcard array dan operator pembanding.** Sebelumnya
perbandingan hanya "sama persis" dan path ke elemen array wajib memakai indeks
angka, sehingga respons berupa array di root harus ditulis `0.status` dan
pertanyaan seperti "apakah pemakaian di atas 90%" tidak bisa diungkapkan.

- Path kini mendukung wildcard: `items[*].state`, `items.*.state`, `items[].state`,
  plus indeks eksplisit `items[0].state` dan array di root `[*].status`.
- Operator baru: `==`, `!=`, `contains`, `not_contains`, `>`, `>=`, `<`, `<=`.
- Perbandingan teks mengabaikan besar-kecil huruf, jadi `ok` cocok dengan `OK`.
- Operator numerik menolak nilai non-angka dengan pesan yang menjelaskan
  masalahnya, bukan gagal diam-diam.
- Untuk wildcard, SETIAP nilai yang cocok harus memenuhi operator. Satu elemen
  menyimpang sudah cukup menandai target DOWN, dan pesan errornya menyebut
  nilai ke berapa yang gagal. Untuk pertanyaan "tidak boleh ada yang error",
  gunakan `not_contains`.
- Monitor `http_json` yang sudah ada tetap berjalan persis seperti sebelumnya:
  kolom baru default kosong dan diperlakukan sebagai `==`.

**Zona waktu notifikasi Telegram.** Sebelumnya label `WIB` ditulis tetap di
dalam kode dan nilainya diambil dari waktu lokal sistem operasi, bukan dari
zona waktu yang dipilih di aplikasi. Pada instalasi dengan OS UTC sementara
aplikasi diset WIB, notifikasi menampilkan waktu 7 jam lebih awal dengan label
`WIB` yang salah.

- Offset zona aplikasi dan singkatannya dihitung di browser (satu-satunya
  tempat dengan basis data zona waktu lengkap) lalu disinkronkan ke server
  saat admin masuk atau menyimpan pengaturan branding.
- Backend menggeser stempel waktu sebesar selisih offset aplikasi terhadap
  offset OS, sehingga benar baik ketika OS sudah sewaktu-waktu sama maupun
  berbeda. Bila belum pernah disinkronkan, perilaku lama dipertahankan.
- Label tidak lagi dipaksa `WIB`: memakai singkatan zona terpilih, atau
  turunan dari offset (`UTC+7`, `UTC+5:30`) bila singkatan belum ada.

**Versi di sidebar.** Label versi sebelumnya ditulis tetap `v0.1.0` di HTML
dan tidak pernah ikut naik saat rilis. Sekarang dibaca dari binary yang
sedang berjalan lewat `GET /api/system/version`, dan sidebar menandai bila ada
rilis lebih baru. Pengecekan rilis memakai cache 6 jam agar tidak menghabiskan
kuota GitHub API, dan kegagalan jaringan tidak mengganggu konsol admin.

**Umpan balik setelah menyimpan monitor.** Probe pertama dijalankan di latar
belakang, jadi respons penyimpanan tidak memuat hasilnya dan admin hanya
melihat pesan "berhasil ditambahkan" tanpa tahu status nyatanya. Sekarang
hasil probe pertama dilaporkan lewat toast, misalnya
`Monitor "API" saved — HTTP 200 • 145 ms`, atau pesan kegagalan bila target
belum bisa dihubungi. Bila hasilnya tidak sampai (mis. koneksi SSE terputus),
admin diberi tahu apa adanya, bukan dibiarkan menunggu.

**Perbaikan tambahan pada pemulihan backup.** Restore sekarang menerima file
backup yang tidak memuat `is_active`, `settings`, atau field `json_operator`
(mis. backup lama atau file yang diedit tangan) alih-alih menolak seluruh
proses karena satu field hilang.

### v0.1.5: Downsampling heartbeat harian

Tabel `heartbeats` tumbuh sekitar 24 ribu baris per hari untuk 17 monitor,
dan index memakan 63% ukuran file. Proyeksi retensi 90 hari mencapai ratusan
MB, terlalu besar untuk flash perangkat kecil.

- Tabel baru `heartbeat_daily`: satu baris per monitor per hari berisi
  `total_checks`, `up_checks`, dan `avg_latency_ms`.
- Rollup otomatis tiap prune 6 jam, idempoten via UPSERT, hanya untuk hari
  kalender yang sudah lewat penuh. Hari berjalan tidak di-rollup.
- Data mentah hanya 7 hari, agregat harian ikut `--retention` (default 90).
- Timeline 90 hari tetap lengkap: hari tua dibaca dari agregat, 7 hari
  terakhir dihitung ulang dari data mentah. Bucket per jam, statistik 24 jam,
  dan ringkasan publik tetap dari data mentah.
- Reset statistik ikut menghapus agregat harian monitor tersebut.
- Endpoint prune manual mendukung `?vacuum=true` eksplisit. Worker otomatis
  tidak pernah VACUUM agar umur eMMC terjaga.
- `DbStats` bertambah `total_daily_rows`.

Diukur pada salinan database produksi: 53 ribu baris mentah 4 hari menjadi
39 baris agregat. Proyeksi 90 hari turun dari ratusan MB menjadi belasan MB.

### v0.1.4: User-Agent, HTTP JSON Query, perbaikan modal

**User-Agent.** Sebelumnya request HTTP tidak mengirim User-Agent sama sekali
sehingga di access log server tujuan muncul sebagai `-` dan tidak bisa dibedakan
dari bot. Sekarang setiap request mengirim:

```
User-Agent: UptimePulse/<versi> (+https://github.com/zidnyzd/uptime-pulse)
```

Bisa diganti per monitor lewat Custom Header `User-Agent`.

**Tipe monitor baru: HTTP JSON Query (`http_json`).** Menutup kasus "HTTP 200
tetapi isi respons menandakan error", yang tidak bisa ditangkap pemeriksaan
status code saja.

- `json_path`: jalur ke nilai di dalam JSON, dot notation, mendukung indeks
  array. Contoh `status`, `data.health`, `items.0.state`.
- `expected_value`: nilai yang diharapkan. Kosong berarti cukup memastikan
  jalurnya ada.

Perbandingan bersifat persis (bukan pencarian sebagian) dan dilakukan sebagai
teks, sehingga angka, boolean, `null`, dan string semuanya bisa dicocokkan. Tipe
`http` biasa tidak terpengaruh.

**Perbaikan modal di layar kecil.** Overlay memakai `align-items: center` tanpa
overflow, sehingga modal yang lebih tinggi dari layar terpotong ke atas dan ke
bawah dan tidak bisa dijangkau sama sekali. Diperparah oleh panel HTTP yang
ditambahkan di v0.1.3 (tinggi modal 518 px menjadi 880 px pada layar 320x568).
Perbaikan: overlay menjadi area scroll, modal memakai `margin: auto`.

### v0.1.3: HTTP method, custom headers, request body

Sebelumnya method di-hardcode `GET` di `prober.rs` sehingga endpoint non-GET dan
API ber-token tidak bisa dipantau.

- Kolom baru `method`, `headers`, `body` dengan migrasi otomatis.
- Tujuh method: GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS.
- Header kustom satu per baris, format `Nama: Nilai`.
- Body dengan `Content-Type: application/json` otomatis bila belum diisi.
- Parser header menolak nama non-token dan nilai ber-CR/LF (anti header
  injection).
- Field `timeout` yang sebelumnya di-hardcode 10 detik di form ikut diperbaiki.

### v0.1.2: Logging persisten alert

Log sistem pada router (OpenWrt/BusyBox) disimpan di RAM dan ter-rotate cepat.
Terukur pada perangkat uji: buffer `logread` hanya bertahan sekitar 3 menit
karena layanan DNS yang verbose membanjiri buffer, sehingga kegagalan alert
tidak meninggalkan jejak apa pun.

- Modul `src/alert_log.rs`: catat `OK` / `ERROR` / `SKIP` setiap percobaan kirim.
- Lokasi diturunkan dari `--db`, sehingga berada di folder yang sama dengan
  database.
- Rotasi pada 512 KB, total dibatasi sekitar 1 MB, karena targetnya adalah eMMC
  dengan umur tulis terbatas. Hanya ditulis saat status berubah, bukan per probe.
- Endpoint `GET /api/alerts/log` dan panel di view Backup admin.
- Perbaikan bug: endpoint test Telegram melaporkan "berhasil" (HTTP 200) padahal
  token atau chat_id kosong dan tidak ada pesan terkirim. Sekarang 400.

### v0.1.1: Perbaikan audit halaman publik

Tujuh temuan audit. Yang paling penting:

1. Insiden dari monitor privat atau paused tidak lagi bocor ke halaman publik.
   Solusinya parameter `public_only` pada query, bukan filter mentah, karena
   konsol admin juga membaca endpoint yang sama.
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

### Memilih tipe monitor: di balik Cloudflare atau server langsung

Untuk domain yang berada di balik Cloudflare, gunakan **HTTP(S)**, bukan ping.
Ping ke domain Cloudflare hanya mengukur edge terdekat, bukan server asli.
Origin bisa mati total sementara ping tetap 0% loss, sehingga monitor tidak
pernah berbunyi.

Contoh terukur:

```
app.example.com     (Cloudflare) -> 203.0.113.10    ping OK     HTTPS 200
origin.example.com  (origin)     -> 198.51.100.25   ping GAGAL
```

Perhatikan: domain yang di-proxy merespons ping, sedangkan origin aslinya tidak.
Artinya ping ke domain Cloudflare tidak memberi informasi apa pun tentang kondisi
server Anda.

Cara cepat memeriksa: `whois <IP>`. Kalau muncul `CLOUDFLARENET`, berarti di
balik Cloudflare dan ping tidak tepat.

Sebaliknya, untuk server langsung yang tidak menjalankan web server, ping justru
lebih tepat. Gejalanya: HTTP gagal tetapi ping berhasil.

### Alert Telegram gagal karena record AAAA

Kalau perangkat hanya punya IPv4 sementara resolver mengembalikan record AAAA,
klien akan mencoba IPv6 lebih dulu dan gagal sebelum jatuh ke IPv4. Terukur
pada perangkat uji: sekitar 60% pengiriman alert gagal karena sebab ini.

Solusinya menyaring record AAAA di resolver:

```sh
uci set dhcp.@dnsmasq[0].filter_aaaa='1'
uci commit dhcp
/etc/init.d/dnsmasq restart
```

Perlu ditinjau ulang kalau IPv6 diaktifkan di jaringan tersebut. Pastikan juga
konfigurasi resolver dicadangkan sebelum diubah.

### Vantage point prober

Jika prober melaporkan DOWN padahal target sehat dari tempat lain, periksa jalur
jaringan host prober lebih dulu. Bandingkan dengan layanan pemeriksa eksternal
multi-node.

Kalau loss hanya terjadi dari jaringan lokal sementara node eksternal bersih,
masalahnya ada di peering ISP, bukan di target dan bukan di kode. Mengganti
protokol (ping ke TCP ke HTTP) tidak akan menolong, karena semua melewati jalur
yang sama. Yang perlu dipindahkan adalah **lokasi prober**, bukan protokolnya.

### Ukuran file WAL

Dalam mode WAL, berkas `-wal` bisa tumbuh lebih besar dari berkas `.db` sebelum
checkpoint. Pastikan pekerjaan prune berkala juga menjalankan
`PRAGMA wal_checkpoint(TRUNCATE)`, karena tanpa itu berkas WAL dapat tumbuh tanpa
batas dan menghabiskan partisi.

---

## 4. Verifikasi yang Sudah Dilakukan

- **JSON Query**: 16/16 skenario (cocok, tidak cocok, HTTP 200 dengan body
  error, bukan JSON, path hilang, nested, indeks array, angka, boolean, null,
  objek kosong, dan tipe `http` yang harus tetap UP).
- **User-Agent**: terkirim di 20/20 request, bisa di-override. Dibuktikan pada
  perangkat ARM64 nyata lewat `httpbin.org`, nilai yang diterima persis
  `UptimePulse/<versi> (+https://github.com/zidnyzd/uptime-pulse)`.
- **Migrasi database**: database lama mendapat kolom baru secara otomatis,
  monitor yang sudah ada tidak berubah perilakunya (semua tetap `GET`).
- **Backup dan restore**: field baru ikut diekspor dan dipulihkan, berkas backup
  versi lama tetap bisa direstore.
- **Keamanan**: header dan body yang memuat kredensial tidak pernah muncul di
  `/api/public/summary`. Diuji dengan monitor publik yang memuat token unik.
- **Modal**: tombol aksi terjangkau pada 320x480, 320x568, 360x640, 375x667,
  390x844, 414x896, 768x1024, dan 1440x900.
- **clippy**: 13 warning, semuanya pre-existing dan bukan dari perubahan terakhir.
- **Downsampling**: rollup 39 baris agregat dari 53 ribu baris mentah, cek
  1 monitor 1 hari (1393/1392/184,3) sama persis dengan data mentah. Migrasi
  tabel otomatis saat start, timeline 90 hari tetap utuh setelah deploy.

---

## 5. Keterbatasan yang Diketahui

| Item | Catatan |
|---|---|
| Tipe `http` tidak memeriksa isi respons | Gunakan `http_json` bila target mengembalikan JSON |
| Perbandingan JSON bersifat persis | Tidak ada mode "mengandung"; nilai harus sama persis |
| 13 warning clippy | Belum dibersihkan, semuanya pre-existing |
| Pertumbuhan database | Sejak v0.1.5 data mentah hanya 7 hari, sisanya agregat harian ikut retensi; WAL tetap butuh checkpoint berkala |

### Pelajaran seputar pengujian UI

Dua kesalahan berikut pernah lolos ke rilis dan baru ketahuan dari laporan
pengguna. Keduanya punya akar yang sama: menguji fitur baru tetapi melewatkan
regresi tata letak.

1. **Memeriksa overflow horizontal saja tidak cukup.** Tidak adanya scroll ke
   samping tidak membuktikan tombol Simpan bisa ditekan. Yang perlu diuji adalah
   keterjangkauan vertikal: setelah container di-scroll maksimum, apakah tombol
   aksi benar-benar berada di dalam viewport.

2. **Menambah field form adalah perubahan tata letak.** Setiap field menambah
   tinggi modal. Ukur selisihnya, dan uji pada viewport yang lebih pendek dari
   konten yang dikirim. Layar 320x480 paling cepat memunculkan masalah.

3. **`align-items: center` tanpa overflow menjebak.** Kontainer flex yang
   memusatkan anaknya tanpa overflow tidak menghasilkan area scroll. Konten yang
   lebih tinggi dari layar akan terpotong ke atas dan ke bawah, dan tidak ada
   gesture yang bisa menjangkaunya. Pengguna melaporkannya sebagai "tidak bisa
   scroll", padahal penyebabnya tidak ada container scroll sama sekali.

4. **Perubahan pada kelas tata letak bersama adalah kandidat regresi di
   mana-mana.** Menyentuh `.modal-overlay` memengaruhi semua modal. "Fitur baru
   saya jalan" bukan bukti yang cukup.
