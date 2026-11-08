//! Regression tests for the auth/session/account/mail QA fixes (package A).
//! These assert the security and logic behaviours added by the fixes; the
//! generic setup is copied from common/mod.rs (with a config override for the
//! per-IP limiter tests, which need a trusted proxy).
mod common;
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use common::*;
use http_body_util::BodyExt;
use retosean::{AppState, config::Config};
use serde_json::{Value, json};
use sqlx::{Executor, postgres::PgPoolOptions};
use tower::ServiceExt;

fn estudiante(username: &str, email: &str) -> Value {
    json!({"tipo":"academico","rol":"ESTUDIANTE","username":username,"email":email,"password":"Clave-Segura-2026","password2":"Clave-Segura-2026","programa_id":PROGRAM,"semestre":3})
}

// AUTHK-3 / #12: a registration whose email only differs by case is rejected.
parity!(test_email_registro_unico_sin_importar_mayusculas, |t| {
    t.ok(
        &t.anon,
        "POST",
        "/api/registro",
        Some(estudiante("dup.email.a", "misma@universidadean.edu.co")),
    )
    .await;
    let (code, _) = t
        .req(
            &t.anon,
            "POST",
            "/api/registro",
            Some(estudiante("dup.email.b", "MISMA@universidadean.edu.co")),
        )
        .await;
    assert_eq!(code, 400);
    assert_eq!(
        t.i64("SELECT count(*) FROM usuarios WHERE lower(email)='misma@universidadean.edu.co'")
            .await,
        1
    );
});

// AUTHK-3 / #12: admin create also enforces case-insensitive email uniqueness.
parity!(test_alta_admin_correo_duplicado_se_rechaza, |t| {
    t.user("dir.correo", "ESTUDIANTE").await;
    let (code, body) = t.req(&t.admin, "POST", "/api/usuarios", Some(json!({"username":"otro.correo","password":"UnaClaveSegura2026","email":"DIR.CORREO@universidadean.edu.co","first_name":"A","last_name":"B","rol":"ESTUDIANTE"}))).await;
    assert_eq!(code, 400, "{body}");
    assert!(body["error"].as_str().unwrap().contains("correo"), "{body}");
});

// AUTHK-3 / #12: a profile edit cannot steal another user's email.
parity!(test_perfil_no_toma_correo_ajeno, |t| {
    t.user("victima.correo", "ESTUDIANTE").await;
    t.user("atacante.correo", "ESTUDIANTE").await;
    let attacker = t.login("atacante.correo").await;
    let (code, body) = t.req(&attacker, "POST", "/api/perfil", Some(json!({"first_name":"A","last_name":"B","email":"victima.correo@universidadean.edu.co","telefono":""}))).await;
    assert_eq!(code, 400, "{body}");
});

// AUTHK-2 / #26: usernames may no longer contain `@`, removing the collision
// with another user's email in the login resolver.
parity!(test_registro_rechaza_arroba_en_usuario, |t| {
    let (code, body) = t
        .req(
            &t.anon,
            "POST",
            "/api/registro",
            Some(estudiante("a@b", "arroba@universidadean.edu.co")),
        )
        .await;
    assert_eq!(code, 400, "{body}");
    assert!(
        t.i64("SELECT count(*) FROM usuarios WHERE username='a@b'")
            .await
            == 0
    );
});

// AUTHK-5 / #80: an inactive account gets the generic invalid-credentials answer.
parity!(test_login_cuenta_inactiva_respuesta_generica, |t| {
    let id = t.user("inactivo.msg", "ESTUDIANTE").await;
    t.exec(&format!(
        "UPDATE usuarios SET activo=false,is_active=false WHERE id={id}"
    ))
    .await;
    let (status, body) = login_response(&t.app, "inactivo.msg", "RetosEAN2026").await;
    assert_eq!(status, 401);
    assert_eq!(
        body["error"].as_str().unwrap(),
        "Usuario o contraseña incorrectos",
        "{body}"
    );
});

// AUTHK-6 / #82: a reset token cannot change an inactive account's password.
parity!(test_reset_confirm_invalido_para_cuenta_inactiva, |t| {
    let id = t.user("inactivo.reset", "ESTUDIANTE").await;
    t.exec(&format!(
        "UPDATE usuarios SET activo=false,is_active=false WHERE id={id}"
    ))
    .await;
    let token = new_token();
    sqlx::query("INSERT INTO rust_password_resets(token_hash,user_id,expires_at) VALUES($1,$2,now()+interval '3 days')")
        .bind(retosean::auth::digest(&token)).bind(id).execute(&t.pool).await.unwrap();
    let (code, _) = t.req(&t.anon, "POST", "/api/password/reset/confirm", Some(json!({"token": token, "nueva":"ClaveRestablecida2026", "nueva2":"ClaveRestablecida2026"}))).await;
    assert_eq!(code, 400);
});

// AUTH-8 / #23: a reset link works once and then is dead.
parity!(test_reset_confirm_solo_una_vez, |t| {
    let id = t.user("reset.una", "ESTUDIANTE").await;
    let token = new_token();
    sqlx::query("INSERT INTO rust_password_resets(token_hash,user_id,expires_at) VALUES($1,$2,now()+interval '3 days')")
        .bind(retosean::auth::digest(&token)).bind(id).execute(&t.pool).await.unwrap();
    let confirm =
        json!({"token": token, "nueva":"ClaveRestablecida2026", "nueva2":"ClaveRestablecida2026"});
    t.ok(
        &t.anon,
        "POST",
        "/api/password/reset/confirm",
        Some(confirm.clone()),
    )
    .await;
    let (code, _) = t
        .req(
            &t.anon,
            "POST",
            "/api/password/reset/confirm",
            Some(confirm),
        )
        .await;
    assert_eq!(code, 400);
});

// WF-2 / #6: only a superuser may grant ADMIN or touch an ADMIN/superuser.
parity!(test_admin_no_superusuario_no_otorga_ni_degrade_admin, |t| {
    t.user("mini.admin", "ADMIN").await;
    let mini = t.login("mini.admin").await;
    let student = t.user("objetivo.rol", "ESTUDIANTE").await;
    let (code, _) = t
        .req(
            &mini,
            "POST",
            &format!("/api/usuarios/{student}/rol"),
            Some(json!({"rol":"ADMIN"})),
        )
        .await;
    assert_eq!(code, 403, "non-superuser cannot grant ADMIN");
    let (code, _) = t
        .req(
            &mini,
            "POST",
            &format!("/api/usuarios/{ADMIN}/rol"),
            Some(json!({"rol":"EXPERTO"})),
        )
        .await;
    assert_eq!(code, 403, "non-superuser cannot touch a superuser");
    t.ok(
        &mini,
        "POST",
        &format!("/api/usuarios/{student}/rol"),
        Some(json!({"rol":"PROFESOR"})),
    )
    .await;
    assert_eq!(
        t.text(&format!("SELECT rol FROM usuarios WHERE id={student}"))
            .await,
        "PROFESOR"
    );
});

// AUTHK-8 / #83: the convention choice requires the EMPRESA role (or an admin).
parity!(test_solo_empresa_elige_modo_convenio, |t| {
    let experto = t.user("experto.conv", "EXPERTO").await;
    t.exec(&format!("INSERT INTO empresas(nit,razon_social,sector_industrial,usuario_id,estado_validacion,estado_listas_restrictivas,estado_convenio) VALUES('901999999','X','',{experto},'PENDIENTE','PENDIENTE','NO_RADICADO')")).await;
    let sess = t.login("experto.conv").await;
    let (code, _) = t
        .req(
            &sess,
            "POST",
            "/api/empresa/modo-convenio",
            Some(json!({"opcion":"si_convenio"})),
        )
        .await;
    assert_eq!(
        code, 403,
        "a non-EMPRESA account with a stray empresas row is rejected"
    );
});

// AUTH-1/AUTH-2/#21/#76: the session cookie and its logout complement carry Secure.
parity!(test_cookie_de_sesion_y_logout_llevan_secure, |t| {
    let (status, cookies) = login_cookies(&t.app, "estudiante.demo", "RetosEAN2026").await;
    assert_eq!(status, 200);
    assert!(
        cookies.iter().any(|c| c.contains("Secure")),
        "login cookie: {cookies:?}"
    );
    let (status, cookies) = logout_cookies(&t.app, &t.student).await;
    assert_eq!(status, 200);
    assert!(
        cookies.iter().any(|c| c.contains("Secure")),
        "logout cookie: {cookies:?}"
    );
});

// AUTH-9 / #24: public registration is throttled per client IP.
#[tokio::test]
#[ignore = "requires local PostgreSQL; each test uses its own schema"]
async fn test_registro_limitado_por_ip() {
    boot_with(|c| c.trust_proxy = true, |t| async move {
        for i in 0..10 {
            let data = json!({"tipo":"academico","rol":"ESTUDIANTE","username":format!("spam.{i}"),"email":format!("spam.{i}@universidadean.edu.co"),"password":"Clave-Segura-2026","password2":"Clave-Segura-2026","programa_id":PROGRAM,"semestre":1});
            let (code, body) = ip_post(&t.app, "203.0.113.7", "/api/registro", data).await;
            assert_eq!(code, StatusCode::OK, "registro {i}: {body}");
        }
        let data = json!({"tipo":"academico","rol":"ESTUDIANTE","username":"spam.over","email":"spam.over@universidadean.edu.co","password":"Clave-Segura-2026","password2":"Clave-Segura-2026","programa_id":PROGRAM,"semestre":1});
        let (code, _) = ip_post(&t.app, "203.0.113.7", "/api/registro", data).await;
        assert_eq!(code, StatusCode::TOO_MANY_REQUESTS);
    }).await;
}

// AUTH-4 / #78: failed logins are counted per client IP (distributed brute force).
#[tokio::test]
#[ignore = "requires local PostgreSQL; each test uses its own schema"]
async fn test_login_registra_fallos_por_ip() {
    boot_with(
        |c| c.trust_proxy = true,
        |t| async move {
            for i in 0..3 {
                t.user(&format!("flood.{i}"), "ESTUDIANTE").await;
            }
            for i in 0..3 {
                let (code, _) =
                    ip_login(&t.app, "198.51.100.9", &format!("flood.{i}"), "mala").await;
                assert_eq!(code, StatusCode::UNAUTHORIZED);
            }
            let subject = retosean::auth::digest("ip:198.51.100.9");
            assert_eq!(
                t.i64(&format!(
                    "SELECT count(*) FROM rust_login_attempts WHERE subject='{subject}'"
                ))
                .await,
                3
            );
        },
    )
    .await;
}

// AUTHK-10 / #84: password-reset requests are also throttled per client IP.
#[tokio::test]
#[ignore = "requires local PostgreSQL; each test uses its own schema"]
async fn test_reset_registra_solicitudes_por_ip() {
    boot_with(
        |c| c.trust_proxy = true,
        |t| async move {
            for i in 0..3 {
                let (code, _) = ip_post(
                    &t.app,
                    "203.0.113.66",
                    "/api/password/reset",
                    json!({"email": format!("nadie{i}@ninguno.co")}),
                )
                .await;
                assert_eq!(code, StatusCode::OK);
            }
            let subject = retosean::auth::digest("reset-ip:203.0.113.66");
            assert_eq!(
                t.i64(&format!(
                    "SELECT count(*) FROM rust_login_attempts WHERE subject='{subject}'"
                ))
                .await,
                3
            );
        },
    )
    .await;
}

fn new_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

async fn login_response(app: &Router, username: &str, password: &str) -> (StatusCode, Value) {
    let (status, body, _) = request(
        app,
        &Session::default(),
        "POST",
        "/api/login",
        Some(json!({"username": username, "password": password})),
    )
    .await;
    (status, body)
}

async fn login_cookies(app: &Router, username: &str, password: &str) -> (StatusCode, Vec<String>) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"username": username, "password": password}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let cookies = cookie_values(&response);
    let _ = response.into_body().collect().await;
    (status, cookies)
}

async fn logout_cookies(app: &Router, session: &Session) -> (StatusCode, Vec<String>) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/logout")
                .header("cookie", &session.cookie)
                .header("x-csrf-token", &session.csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let cookies = cookie_values(&response);
    let _ = response.into_body().collect().await;
    (status, cookies)
}

fn cookie_values(response: &axum::response::Response) -> Vec<String> {
    response
        .headers()
        .get_all("set-cookie")
        .iter()
        .map(|v| v.to_str().unwrap_or_default().to_string())
        .collect()
}

async fn ip_post(app: &Router, ip: &str, path: &str, data: Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("content-type", "application/json")
                .header("x-forwarded-for", ip)
                .body(Body::from(data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn ip_login(app: &Router, ip: &str, username: &str, password: &str) -> (StatusCode, Value) {
    ip_post(
        app,
        ip,
        "/api/login",
        json!({"username": username, "password": password}),
    )
    .await
}

/// common::run with a config override; used to turn on trusted-proxy for the
/// per-IP limiter tests (the oneshot harness has no TCP peer to use otherwise).
async fn boot_with<F, Fut>(configure: fn(&mut Config), body: F)
where
    F: FnOnce(Ctx) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    outbox();
    let mut config = Config::load().unwrap();
    configure(&mut config);
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(config.database.clone())
        .await
        .unwrap();
    let schema = format!("rust_test_{}", uuid::Uuid::new_v4().simple());
    admin
        .execute(format!("CREATE SCHEMA {schema}").as_str())
        .await
        .unwrap();
    let search_path = schema.clone();
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .after_connect(move |conn, _| {
            let s = format!("SET search_path TO {search_path}");
            Box::pin(async move {
                conn.execute(s.as_str()).await?;
                Ok(())
            })
        })
        .connect_with(config.database.clone())
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("../sql/schema.sql"))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(&include_str!("../sql/demo.sql").replace("__DEMO_PASSWORD_HASH__", HASH))
        .execute(&pool)
        .await
        .unwrap();
    retosean::auth::init(&pool).await.unwrap();
    config.media_root = std::env::temp_dir().join(&schema);
    let media = config.media_root.clone();
    let app = retosean::router(AppState {
        pool: pool.clone(),
        config,
        password_slots: std::sync::Arc::new(tokio::sync::Semaphore::new(4)),
        login_slots: std::sync::Arc::new(tokio::sync::Semaphore::new(4)),
    });
    let ctx = Ctx {
        admin: login(&app, "admin.demo").await,
        company: login(&app, "empresa.verificada").await,
        teacher: login(&app, "profesor.demo").await,
        student: login(&app, "estudiante.demo").await,
        expert: login(&app, "experto.demo").await,
        anon: Session::default(),
        app,
        pool: pool.clone(),
        media: media.clone(),
    };
    let outcome = tokio::spawn(body(ctx)).await;
    pool.close().await;
    admin
        .execute(format!("DROP SCHEMA {schema} CASCADE").as_str())
        .await
        .unwrap();
    admin.close().await;
    let _ = std::fs::remove_dir_all(&media);
    if let Err(error) = outcome {
        std::panic::resume_unwind(error.into_panic());
    }
}

// AUTH-3: behind a trusted proxy, an Origin matching X-Forwarded-Host is same-site.
#[tokio::test]
#[ignore = "requires local PostgreSQL; each test uses its own schema"]
async fn test_origen_con_proxy_de_confianza() {
    boot_with(
        |c| c.trust_proxy = true,
        |t| async move {
            let response = t
                .app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/login")
                        .header("content-type", "application/json")
                        .header("host", "127.0.0.1:8080")
                        .header("origin", "https://publico.example.edu.co")
                        .header("x-forwarded-host", "publico.example.edu.co")
                        .body(Body::from(
                            json!({"username":"admin","password":"mala"}).to_string(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        },
    )
    .await;
}

// AUTHK-1 / #25: an attacker failing from one address cannot lock the owner out
// from another, but failures spread over many addresses still hit the
// account-wide cap.
#[tokio::test]
#[ignore = "requires local PostgreSQL; each test uses its own schema"]
async fn test_bloqueo_por_cuenta_e_ip() {
    boot_with(
        |c| c.trust_proxy = true,
        |t| async move {
            t.user("victima.ip", "ESTUDIANTE").await;
            for _ in 0..10 {
                let (code, _) = ip_login(&t.app, "198.51.100.66", "victima.ip", "mala").await;
                assert_eq!(code, StatusCode::UNAUTHORIZED);
            }
            let (code, _) = ip_login(&t.app, "198.51.100.66", "victima.ip", "RetosEAN2026").await;
            assert_eq!(
                code,
                StatusCode::TOO_MANY_REQUESTS,
                "the attacker's address is locked"
            );
            let (code, body) = ip_login(&t.app, "203.0.113.20", "victima.ip", "RetosEAN2026").await;
            assert_eq!(
                code,
                StatusCode::OK,
                "the owner elsewhere still signs in: {body}"
            );
            for i in 0..50 {
                let (code, _) = ip_login(
                    &t.app,
                    &format!("10.9.{}.{}", i / 200, i % 200 + 1),
                    "victima.ip",
                    "mala",
                )
                .await;
                assert_eq!(code, StatusCode::UNAUTHORIZED, "attempt {i}");
            }
            let (code, _) = ip_login(&t.app, "203.0.113.21", "victima.ip", "RetosEAN2026").await;
            assert_eq!(
                code,
                StatusCode::TOO_MANY_REQUESTS,
                "account-wide cap reached"
            );
        },
    )
    .await;
}
