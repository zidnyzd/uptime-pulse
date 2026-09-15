use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::sync::Arc;
use crate::database::DbPool;
use crate::models::User;

pub struct AuthMiddlewareState {
    pub db: DbPool,
}

// Middleware pelindung endpoint admin: menolak akses jika tidak memiliki session valid
pub async fn require_admin_auth(
    State(state): State<Arc<AuthMiddlewareState>>,
    req: Request,
    next: Next,
) -> Response {
    let headers = req.headers();

    // 1. Cek dari Cookie 'uptime_session'
    let token_from_cookie = headers
        .get(header::COOKIE)
        .and_then(|val| val.to_str().ok())
        .and_then(|cookie_str| {
            cookie_str.split(';').find_map(|pair| {
                let mut parts = pair.trim().splitn(2, '=');
                let key = parts.next()?;
                let val = parts.next()?;
                if key == "uptime_session" {
                    Some(val.to_string())
                } else {
                    None
                }
            })
        });

    // 2. Fallback: Cek dari Header 'Authorization: Bearer <token>'
    let token_from_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|val| val.to_str().ok())
        .and_then(|auth_str| {
            if auth_str.starts_with("Bearer ") {
                Some(auth_str[7..].trim().to_string())
            } else {
                None
            }
        });

    let token = token_from_cookie.or(token_from_header);

    match token {
        Some(t) => match User::validate_session(&state.db, &t).await {
            Ok(true) => next.run(req).await,
            _ => unauthorized_response(),
        },
        None => unauthorized_response(),
    }
}

fn unauthorized_response() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({
            "error": "Unauthorized",
            "message": "Akses ditolak. Silakan login sebagai admin terlebih dahulu."
        })),
    )
        .into_response()
}
