use axum::{
    extract::{ConnectInfo, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use crate::database::DbPool;
use crate::middlewares::rate_limit::LoginRateLimiter;
use crate::models::User;

pub struct AuthControllerState {
    pub db: DbPool,
    pub login_limiter: Arc<LoginRateLimiter>,
}

#[derive(Debug, Deserialize)]
pub struct LoginPayload {
    pub username: String,
    pub password: String,
}

// Handler login admin: rate-limit anti brute-force + cookie session aman
pub async fn login(
    State(state): State<Arc<AuthControllerState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(payload): Json<LoginPayload>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let ip: IpAddr = addr.ip();
    // Tolak dini jika IP sedang diblokir (HTTP 429 + info sisa blokir)
    if let Err(wait_secs) = state.login_limiter.check(&ip).await {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({
                "success": false,
                "error": format!("Terlalu banyak percobaan gagal. Coba lagi dalam {} detik.", wait_secs)
            })),
        ));
    }

    match User::authenticate(&state.db, &payload.username, &payload.password).await {
        Ok(Some(user)) => {
            // Login sukses -> reset hitungan gagal IP ini
            state.login_limiter.record_success(&ip).await;
            let token = User::create_session(&state.db, user.id).await.map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": e.to_string() })),
                )
            })?;

            // Pasang cookie session yang aman (HttpOnly, SameSite=Lax)
            let cookie_header = format!(
                "uptime_session={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=604800",
                token
            );

            let body = Json(json!({
                "success": true,
                "token": token,
                "username": user.username,
            }));

            Ok((
                StatusCode::OK,
                [(header::SET_COOKIE, cookie_header)],
                body,
            ).into_response())
        }
        Ok(None) => {
            // Kredensial salah -> catat agar brute-force terkunci setelah 5x
            state.login_limiter.record_fail(&ip).await;
            Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "success": false,
                    "error": "Kombinasi username atau password salah"
                })),
            ))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )),
    }
}

// Handler logout: menghapus SEMUA session user (semua perangkat) + mengosongkan cookie
pub async fn logout(
    State(state): State<Arc<AuthControllerState>>,
    headers: HeaderMap,
) -> Response {
    if let Some(cookie_str) = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()) {
        for pair in cookie_str.split(';') {
            let mut parts = pair.trim().splitn(2, '=');
            if parts.next() == Some("uptime_session") {
                if let Some(token) = parts.next() {
                    // Hanguskan sesi di semua perangkat, bukan cuma token ini
                    let _ = User::delete_all_user_sessions(&state.db, token).await;
                }
            }
        }
    }

    let clear_cookie = "uptime_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0";
    (
        StatusCode::OK,
        [(header::SET_COOKIE, clear_cookie)],
        Json(json!({ "success": true, "message": "Berhasil logout" })),
    ).into_response()
}

// Handler cek status autentikasi user saat ini
pub async fn me(
    State(state): State<Arc<AuthControllerState>>,
    headers: HeaderMap,
) -> Response {
    let token = headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|cookie_str| {
            cookie_str.split(';').find_map(|pair| {
                let mut parts = pair.trim().splitn(2, '=');
                if parts.next()? == "uptime_session" {
                    Some(parts.next()?.to_string())
                } else {
                    None
                }
            })
        })
        .or_else(|| {
            headers
                .get(header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.strip_prefix("Bearer ").map(|t| t.trim().to_string()))
        });

    let is_authenticated = match token {
        Some(t) => User::validate_session(&state.db, &t).await.unwrap_or(false),
        None => false,
    };

    Json(json!({
        "authenticated": is_authenticated,
        "username": if is_authenticated { "admin" } else { "" }
    })).into_response()
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordPayload {
    pub old_password: String,
    pub new_password: String,
}

// Handler ubah kata sandi admin (minimal 8 karakter)
pub async fn change_password(
    State(state): State<Arc<AuthControllerState>>,
    Json(payload): Json<ChangePasswordPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    if payload.new_password.trim().len() < 8 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "Kata sandi baru minimal 8 karakter" })),
        ));
    }

    match User::change_password(&state.db, "admin", &payload.old_password, &payload.new_password).await {
        Ok(Ok(())) => Ok(Json(json!({
            "success": true,
            "message": "Kata sandi admin berhasil diperbarui"
        }))),
        Ok(Err(msg)) => Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": msg })),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )),
    }
}
