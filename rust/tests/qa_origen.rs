//! Regression tests for the same-origin guard (AUTH-3 login CSRF and defence in
//! depth): browser requests from another site never reach a handler.
mod common;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use common::*;
use serde_json::json;
use tower::ServiceExt;

async fn login_from(t: &Ctx, headers: &[(&str, &str)]) -> StatusCode {
    let mut request = Request::builder()
        .method("POST")
        .uri("/api/login")
        .header("content-type", "application/json")
        .header("host", "retos.example.edu.co");
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let body = json!({"username":"admin","password":"clave-incorrecta"}).to_string();
    t.app
        .clone()
        .oneshot(request.body(Body::from(body)).unwrap())
        .await
        .unwrap()
        .status()
}

parity!(test_login_desde_otro_sitio_rechazado, |t| {
    for headers in [
        &[("origin", "https://evil.example")][..],
        &[("origin", "null")][..],
        &[("sec-fetch-site", "cross-site")][..],
        &[("sec-fetch-site", "same-site")][..],
    ] {
        assert_eq!(
            login_from(&t, headers).await,
            StatusCode::FORBIDDEN,
            "{headers:?}"
        );
    }
    // A cross-site attempt is refused before the handler, so it is not even counted.
    assert_eq!(t.i64("SELECT count(*) FROM rust_login_attempts").await, 0);
});

parity!(test_login_mismo_origen_llega_al_handler, |t| {
    for headers in [
        &[("origin", "https://retos.example.edu.co")][..],
        &[("sec-fetch-site", "same-origin")][..],
        &[][..],
    ] {
        assert_eq!(
            login_from(&t, headers).await,
            StatusCode::UNAUTHORIZED,
            "{headers:?}"
        );
    }
});

// Without RUST_TRUST_PROXY a client-supplied X-Forwarded-Host is ignored
// (the trusted-proxy case lives in qa_auth.rs, which can override the config).
parity!(test_x_forwarded_host_ignorado_sin_proxy_de_confianza, |t| {
    let status = login_from(
        &t,
        &[
            ("origin", "https://publico.example.edu.co"),
            ("x-forwarded-host", "publico.example.edu.co"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
});

parity!(test_get_de_otro_sitio_no_se_bloquea, |t| {
    let response = t
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health")
                .header("origin", "https://evil.example")
                .header("sec-fetch-site", "cross-site")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
});
