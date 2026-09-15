use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use crate::database::DbPool;
use crate::models::User;

pub struct AuthControllerState {
    pub db: DbPool,
}

#[derive(Debug, Deserialize)]
pub struct LoginPayload {
    pub username: String,
    pub password: String,
}

// Handler login admin: memverifikasi kata sandi dan membuat cookie session
pub async fn login(
    State(state): State<Arc<AuthControllerState>>,
    Json(payload): Json<LoginPayload>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    match User::authenticate(&state.db, &payload.username, &payload.password).await {
        Ok(Some(user)) => {
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
        Ok(None) => Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "success": false,
                "error": "Kombinasi username atau password salah"
            })),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )),
    }
}

// Handler logout: menghapus session token dari database dan mengosongkan cookie
pub async fn logout(
    State(state): State<Arc<AuthControllerState>>,
    headers: HeaderMap,
) -> Response {
    if let Some(cookie_str) = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()) {
        for pair in cookie_str.split(';') {
            let mut parts = pair.trim().splitn(2, '=');
            if parts.next() == Some("uptime_session") {
                if let Some(token) = parts.next() {
                    let _ = User::delete_session(&state.db, token).await;
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
