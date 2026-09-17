use rusqlite::{params, OptionalExtension, Result};
use serde::{Deserialize, Serialize};
use crate::database::DbPool;

// Model representasi insiden downtime & durasi pemulihan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Incident {
    pub id: i64,
    pub monitor_id: i64,
    pub service_name: String,
    pub started_at: String,
    pub resolved_at: Option<String>,
    pub duration_sec: Option<i64>,
    pub error_message: Option<String>,
    pub is_ongoing: bool,
}

impl Incident {
    // Mengevaluasi hasil probe: jika status down maka catat insiden baru, jika pulih (up) maka selesaikan insiden
    pub async fn process_probe(
        db: &DbPool,
        monitor_id: i64,
        status: &str,
        error_message: Option<&str>,
    ) -> Result<()> {
        let mut trigger_down_info: Option<(String, String, String, String)> = None;
        let mut trigger_up_info: Option<(String, String, i64, String)> = None;

        {
            let conn = db.lock().await;

            if status == "down" {
                // Cek apakah sudah ada insiden aktif yang belum terselesaikan
                let open_incident_id: Option<i64> = conn
                    .query_row(
                        "SELECT id FROM incidents WHERE monitor_id = ?1 AND resolved_at IS NULL ORDER BY id DESC LIMIT 1",
                        params![monitor_id],
                        |r| r.get(0),
                    )
                    .optional()?;

                if open_incident_id.is_none() {
                    let now_str: String = conn.query_row("SELECT datetime('now', 'localtime')", [], |r| r.get(0))?;
                    // Catat dimulainya insiden baru
                    conn.execute(
                        "INSERT INTO incidents (monitor_id, started_at, error_message)
                         VALUES (?1, ?2, ?3)",
                        params![monitor_id, now_str, error_message],
                    )?;

                    if let Ok(mon_info) = conn.query_row::<(String, String), _, _>(
                        "SELECT name, target FROM monitors WHERE id = ?1",
                        params![monitor_id],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    ) {
                        trigger_down_info = Some((mon_info.0, mon_info.1, error_message.unwrap_or("Unknown error").to_string(), now_str));
                    }
                }
            } else if status == "up" {
                // Jika server sudah UP, selesaikan insiden yang masih terbuka dan hitung durasi downtime-nya
                let open_incident: Option<i64> = conn
                    .query_row(
                        "SELECT id FROM incidents WHERE monitor_id = ?1 AND resolved_at IS NULL ORDER BY id DESC LIMIT 1",
                        params![monitor_id],
                        |r| r.get(0),
                    )
                    .optional()?;

                if let Some(inc_id) = open_incident {
                    let now_str: String = conn.query_row("SELECT datetime('now', 'localtime')", [], |r| r.get(0))?;
                    conn.execute(
                        "UPDATE incidents 
                         SET resolved_at = ?2,
                             duration_sec = MAX(1, CAST((strftime('%s', ?2) - strftime('%s', started_at)) AS INTEGER))
                         WHERE id = ?1",
                        params![inc_id, now_str],
                    )?;

                    let duration_sec: i64 = conn.query_row(
                        "SELECT duration_sec FROM incidents WHERE id = ?1",
                        params![inc_id],
                        |r| r.get(0),
                    ).unwrap_or(1);

                    if let Ok(mon_info) = conn.query_row::<(String, String), _, _>(
                        "SELECT name, target FROM monitors WHERE id = ?1",
                        params![monitor_id],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    ) {
                        trigger_up_info = Some((mon_info.0, mon_info.1, duration_sec, now_str));
                    }
                }
            }
        } // Lock connection dilepas sebelum kirim notifikasi async

        // Kirim alert Telegram jika terjadi perubahan state (DOWN baru atau RECOVERY)
        if let Some((name, target, err, started_at)) = trigger_down_info {
            crate::models::TelegramSettings::notify_down(db, &name, &target, &err, &started_at).await;
        }

        if let Some((name, target, dur, resolved_at)) = trigger_up_info {
            crate::models::TelegramSettings::notify_recovery(db, &name, &target, dur, &resolved_at).await;
        }

        Ok(())
    }

    // Mengambil daftar seluruh insiden yang sedang aktif berlangsung.
    // `public_only = true` membatasi hasil ke monitor aktif & publik saja
    // (dipakai halaman status publik). Konsol admin memakai `false` agar
    // insiden monitor privat/paused tetap terlihat.
    pub async fn list_ongoing(db: &DbPool, public_only: bool) -> Result<Vec<Incident>> {
        let conn = db.lock().await;
        let public_flag: i32 = if public_only { 1 } else { 0 };
        let mut stmt = conn.prepare(
            "SELECT i.id, i.monitor_id, m.name, i.started_at, i.resolved_at, i.duration_sec, i.error_message
             FROM incidents i
             JOIN monitors m ON m.id = i.monitor_id
             WHERE i.resolved_at IS NULL
               AND (?1 = 0 OR (m.is_active = 1 AND m.is_public = 1))
             ORDER BY i.id DESC"
        )?;

        let rows = stmt.query_map([public_flag], |row| {
            Ok(Incident {
                id: row.get(0)?,
                monitor_id: row.get(1)?,
                service_name: row.get(2)?,
                started_at: row.get(3)?,
                resolved_at: row.get(4)?,
                duration_sec: row.get(5)?,
                error_message: row.get(6)?,
                is_ongoing: true,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    // Mengambil riwayat insiden terbaru.
    // `public_only = true` -> hanya monitor aktif & publik (halaman status publik).
    pub async fn list_recent(db: &DbPool, limit: i64, public_only: bool) -> Result<Vec<Incident>> {
        let conn = db.lock().await;
        let public_flag: i32 = if public_only { 1 } else { 0 };
        let mut stmt = conn.prepare(
            "SELECT i.id, i.monitor_id, m.name, i.started_at, i.resolved_at, i.duration_sec, i.error_message
             FROM incidents i
             JOIN monitors m ON m.id = i.monitor_id
             WHERE (?2 = 0 OR (m.is_active = 1 AND m.is_public = 1))
             ORDER BY i.id DESC
             LIMIT ?1"
        )?;

        let rows = stmt.query_map(params![limit, public_flag], |row| {
            let resolved: Option<String> = row.get(4)?;
            let is_ongoing = resolved.is_none();
            Ok(Incident {
                id: row.get(0)?,
                monitor_id: row.get(1)?,
                service_name: row.get(2)?,
                started_at: row.get(3)?,
                resolved_at: resolved,
                duration_sec: row.get(5)?,
                error_message: row.get(6)?,
                is_ongoing,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }
}
