use std::fs::File;
use std::io::Read;
use rusqlite::{params, OptionalExtension, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use crate::database::DbPool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
    pub salt: String,
    pub created_at: String,
}

impl User {
    // Menghasilkan hash kata sandi menggunakan SHA-256 dan salt acak
    pub fn hash_password(password: &str, salt: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(salt.as_bytes());
        hasher.update(b":");
        hasher.update(password.as_bytes());
        let result = hasher.finalize();
        result.iter().map(|b| format!("{:02x}", b)).collect()
    }

    // Menghasilkan random string hex untuk salt atau session token
    pub fn generate_random_hex(byte_len: usize) -> String {
        let mut buf = vec![0u8; byte_len];
        if let Ok(mut f) = File::open("/dev/urandom") {
            let _ = f.read_exact(&mut buf);
        } else {
            // Fallback jika tidak ada /dev/urandom
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            for (i, b) in buf.iter_mut().enumerate() {
                *b = ((ts >> (i % 8)) & 0xFF) as u8;
            }
        }
        buf.iter().map(|b| format!("{:02x}", b)).collect()
    }

    // Memastikan akun admin default tersedia saat server pertama kali dinyalakan
    pub async fn ensure_admin_exists(db: &DbPool, default_pass: &str) -> Result<()> {
        let conn = db.lock().await;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM users WHERE username = 'admin'",
            [],
            |r| r.get(0),
        )?;

        if count == 0 {
            let salt = Self::generate_random_hex(16);
            let hash = Self::hash_password(default_pass, &salt);
            conn.execute(
                "INSERT INTO users (username, password_hash, salt) VALUES ('admin', ?1, ?2)",
                params![hash, salt],
            )?;
        }
        Ok(())
    }

    // Verifikasi username dan password saat login
    pub async fn authenticate(db: &DbPool, username: &str, password: &str) -> Result<Option<User>> {
        let conn = db.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, username, password_hash, salt, created_at FROM users WHERE username = ?1"
        )?;

        let user = stmt.query_row([username], |row| {
            Ok(User {
                id: row.get(0)?,
                username: row.get(1)?,
                password_hash: row.get(2)?,
                salt: row.get(3)?,
                created_at: row.get(4)?,
            })
        });

        match user {
            Ok(u) => {
                let computed_hash = Self::hash_password(password, &u.salt);
                if computed_hash == u.password_hash {
                    Ok(Some(u))
                } else {
                    Ok(None)
                }
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    // Membuat session token baru dengan masa berlaku 7 hari
    pub async fn create_session(db: &DbPool, user_id: i64) -> Result<String> {
        let token = Self::generate_random_hex(32);
        let conn = db.lock().await;
        conn.execute(
            "INSERT INTO sessions (token, user_id, expires_at)
             VALUES (?1, ?2, datetime('now', '+7 days', 'localtime'))",
            params![token, user_id],
        )?;
        Ok(token)
    }

    // Validasi session token dari cookie atau Authorization header
    pub async fn validate_session(db: &DbPool, token: &str) -> Result<bool> {
        if token.trim().is_empty() {
            return Ok(false);
        }
        let conn = db.lock().await;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sessions WHERE token = ?1 AND expires_at > datetime('now', 'localtime')",
            [token],
            |r| r.get(0),
        )?;
        Ok(count > 0)
    }

    // Menghapus SEMUA session milik user pemilik token (dipakai saat logout
    // agar sesi lain di perangkat/browser lain ikut hangus)
    pub async fn delete_all_user_sessions(db: &DbPool, token: &str) -> Result<()> {
        let conn = db.lock().await;
        conn.execute(
            "DELETE FROM sessions WHERE user_id = (SELECT user_id FROM sessions WHERE token = ?1)",
            [token],
        )?;
        Ok(())
    }

    // Memperbarui kata sandi admin setelah memvalidasi kata sandi lama
    pub async fn change_password(
        db: &DbPool,
        username: &str,
        old_password: &str,
        new_password: &str,
    ) -> Result<Result<(), String>> {
        let conn = db.lock().await;

        let user: Option<(i64, String, String)> = conn
            .query_row(
                "SELECT id, password_hash, salt FROM users WHERE username = ?1",
                params![username],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;

        let (user_id, current_hash, current_salt) = match user {
            Some(u) => u,
            None => return Ok(Err("Pengguna tidak ditemukan".to_string())),
        };

        // Verifikasi password lama
        let computed = Self::hash_password(old_password, &current_salt);
        if computed != current_hash {
            return Ok(Err("Kata sandi lama tidak cocok".to_string()));
        }

        // Buat salt acak baru dan hash kata sandi baru
        let new_salt = Self::generate_random_hex(16);
        let new_hash = Self::hash_password(new_password, &new_salt);

        conn.execute(
            "UPDATE users SET password_hash = ?1, salt = ?2 WHERE id = ?3",
            params![new_hash, new_salt, user_id],
        )?;

        Ok(Ok(()))
    }
}
