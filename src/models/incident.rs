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
    // Mengevaluasi hasil probe: jika target down maka catat insiden baru, jika pulih maka selesaikan insiden
    pub async fn process_probe(
        db: &DbPool,
        monitor_id: i64,
        is_up: bool,
        error_message: Option<&str>,
    ) -> Result<()> {
        let conn = db.lock().await;

        if !is_up {
            // Cek apakah sudah ada insiden aktif yang belum terselesaikan
            let open_incident_id: Option<i64> = conn
                .query_row(
                    "SELECT id FROM incidents WHERE monitor_id = ?1 AND resolved_at IS NULL ORDER BY id DESC LIMIT 1",
                    params![monitor_id],
                    |r| r.get(0),
                )
                .optional()?;

            if open_incident_id.is_none() {
                // Catat dimulainya insiden baru
                conn.execute(
                    "INSERT INTO incidents (monitor_id, started_at, error_message)
                     VALUES (?1, datetime('now', 'localtime'), ?2)",
                    params![monitor_id, error_message],
                )?;
            }
        } else {
            // Jika server sudah UP, selesaikan insiden yang masih terbuka dan hitung durasi downtime-nya
            let open_incident: Option<i64> = conn
                .query_row(
                    "SELECT id FROM incidents WHERE monitor_id = ?1 AND resolved_at IS NULL ORDER BY id DESC LIMIT 1",
                    params![monitor_id],
                    |r| r.get(0),
                )
                .optional()?;

            if let Some(inc_id) = open_incident {
                conn.execute(
                    "UPDATE incidents 
                     SET resolved_at = datetime('now', 'localtime'),
                         duration_sec = MAX(1, CAST((strftime('%s', 'now', 'localtime') - strftime('%s', started_at)) AS INTEGER))
                     WHERE id = ?1",
                    params![inc_id],
                )?;
            }
        }

        Ok(())
    }

    // Mengambil daftar seluruh insiden yang sedang aktif berlangsung
    pub async fn list_ongoing(db: &DbPool) -> Result<Vec<Incident>> {
        let conn = db.lock().await;
        let mut stmt = conn.prepare(
            "SELECT i.id, i.monitor_id, m.name, i.started_at, i.resolved_at, i.duration_sec, i.error_message
             FROM incidents i
             JOIN monitors m ON m.id = i.monitor_id
             WHERE i.resolved_at IS NULL
             ORDER BY i.id DESC"
        )?;

        let rows = stmt.query_map([], |row| {
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

    // Mengambil riwayat insiden terbaru untuk publik
    pub async fn list_recent(db: &DbPool, limit: i64) -> Result<Vec<Incident>> {
        let conn = db.lock().await;
        let mut stmt = conn.prepare(
            "SELECT i.id, i.monitor_id, m.name, i.started_at, i.resolved_at, i.duration_sec, i.error_message
             FROM incidents i
             JOIN monitors m ON m.id = i.monitor_id
             ORDER BY i.id DESC
             LIMIT ?1"
        )?;

        let rows = stmt.query_map([limit], |row| {
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
