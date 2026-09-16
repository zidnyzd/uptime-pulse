use axum::{extract::Request, http::HeaderValue, middleware::Next, response::Response};

// Middleware security headers untuk semua respons.
// Tanpa CSP ketat karena frontend masih memakai inline onclick handler;
// X-Frame-Options SAMEORIGIN mencegah clickjacking halaman admin.
pub async fn security_headers(req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    h.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    h.insert("x-frame-options", HeaderValue::from_static("SAMEORIGIN"));
    h.insert(
        "referrer-policy",
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    res
}
