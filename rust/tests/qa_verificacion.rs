//! Regression tests for finding #1: self-registered accounts must verify their
//! email before logging in. Setup copied from common/mod.rs; the memory mailer
//! (common::outbox) captures the verification link.
mod common;
use common::*;
use serde_json::{Value, json};

fn estudiante(username: &str, email: &str) -> Value {
    json!({"tipo":"academico","rol":"ESTUDIANTE","username":username,"email":email,"password":"Clave-Segura-2026","password2":"Clave-Segura-2026","programa_id":PROGRAM,"semestre":3})
}

fn empresa(username: &str, email: &str, nit: &str) -> Value {
    json!({"tipo":"empresa","nit":nit,"razon_social":"ACME S.A.S.","sector_industrial":"Tecnología","tipo_convocatoria":"AMBOS","area_responsable":"Innovación","sitio_web":"https://acme.co","username":username,"email":email,"password":"Clave-Segura-2026","password2":"Clave-Segura-2026"})
}

fn new_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

async fn seed(token: &str, user: i64, t: &Ctx) {
    sqlx::query("INSERT INTO rust_email_verifications(token_hash,user_id,expires_at) VALUES($1,$2,now()+interval '3 days')")
        .bind(retosean::auth::digest(token))
        .bind(user)
        .execute(&t.pool)
        .await
        .unwrap();
}

async fn login_error(t: &Ctx, name: &str, password: &str) -> (u16, String) {
    let (status, body, _) = request(
        &t.app,
        &Session::default(),
        "POST",
        "/api/login",
        Some(json!({"username": name, "password": password})),
    )
    .await;
    (
        status.as_u16(),
        body["error"].as_str().unwrap_or_default().to_string(),
    )
}

async fn verification_token(address: &str) -> String {
    for _ in 0..50 {
        let mails = mails_to(address);
        if let Some((_, _, text)) = mails.iter().find(|m| m.1.contains("Confirma tu correo"))
            && let Some(rest) = text.split("#verificar/").nth(1)
        {
            return rest[..64].to_string();
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("no verification email sent to {address}");
}

// #1: a self-registered student/company gets no session and cannot log in until
// the email is confirmed.
parity!(test_registro_estudiante_no_abre_sesion, |t| {
    let (code, body, cookie) = request(
        &t.app,
        &t.anon,
        "POST",
        "/api/registro",
        Some(estudiante("verif.est", "verif.est@universidadean.edu.co")),
    )
    .await;
    assert_eq!(code, 200, "{body}");
    assert_eq!(body["correo_pendiente"], true, "{body}");
    assert!(
        body["mensaje"]
            .as_str()
            .unwrap()
            .contains("Revisa tu correo"),
        "{body}"
    );
    assert!(cookie.is_none(), "no session before verification");
    assert!(
        !t.bool("SELECT correo_verificado FROM usuarios WHERE username='verif.est'")
            .await
    );
    assert!(
        t.bool("SELECT activo AND is_active FROM usuarios WHERE username='verif.est'")
            .await
    );
    let (status, message) = login_error(&t, "verif.est", "Clave-Segura-2026").await;
    assert_eq!(status, 401, "{message}");
    assert_eq!(message, "Confirma tu correo antes de iniciar sesión");
});

parity!(test_registro_empresa_tambien_requiere_verificacion, |t| {
    t.ok(
        &t.anon,
        "POST",
        "/api/registro",
        Some(empresa("verif.emp", "contacto@verifemp.co", "901999001")),
    )
    .await;
    assert!(
        !t.bool("SELECT correo_verificado FROM usuarios WHERE username='verif.emp'")
            .await
    );
    let (status, message) = login_error(&t, "verif.emp", "Clave-Segura-2026").await;
    assert_eq!(status, 401);
    assert_eq!(message, "Confirma tu correo antes de iniciar sesión");
});

// The emailed link is single-use and enables login.
parity!(test_verificacion_activa_la_cuenta_y_permite_entrar, |t| {
    t.ok(
        &t.anon,
        "POST",
        "/api/registro",
        Some(estudiante("verif.link", "verif.link@universidadean.edu.co")),
    )
    .await;
    let token = verification_token("verif.link@universidadean.edu.co").await;
    t.ok(
        &t.anon,
        "POST",
        "/api/registro/verificar",
        Some(json!({"token": token})),
    )
    .await;
    assert!(
        t.bool("SELECT correo_verificado FROM usuarios WHERE username='verif.link'")
            .await
    );
    let (status, session) = login_with(&t.app, "verif.link", "Clave-Segura-2026").await;
    assert_eq!(status, 200, "verified account logs in");
    t.ok(&session, "GET", "/api/me", None).await;
});

parity!(test_enlace_de_verificacion_es_de_un_solo_uso, |t| {
    let id = t.user("verif.once", "ESTUDIANTE").await;
    t.exec(&format!(
        "UPDATE usuarios SET correo_verificado=false WHERE id={id}"
    ))
    .await;
    let token = new_token();
    seed(&token, id, &t).await;
    t.ok(
        &t.anon,
        "POST",
        "/api/registro/verificar",
        Some(json!({"token": token})),
    )
    .await;
    assert_eq!(
        t.status(
            &t.anon,
            "POST",
            "/api/registro/verificar",
            Some(json!({"token": token}))
        )
        .await
        .as_u16(),
        400
    );
});

parity!(test_verificacion_rechaza_enlace_invalido_o_expirado, |t| {
    let id = t.user("verif.bad", "ESTUDIANTE").await;
    t.exec(&format!(
        "UPDATE usuarios SET correo_verificado=false WHERE id={id}"
    ))
    .await;
    // Wrong length / malformed.
    let (code, _) = t
        .req(
            &t.anon,
            "POST",
            "/api/registro/verificar",
            Some(json!({"token":"corto"})),
        )
        .await;
    assert_eq!(code, 400);
    // Expired.
    let token = new_token();
    sqlx::query("INSERT INTO rust_email_verifications(token_hash,user_id,expires_at) VALUES($1,$2,now()-interval '1 day')")
        .bind(retosean::auth::digest(&token)).bind(id).execute(&t.pool).await.unwrap();
    let (code, _) = t
        .req(
            &t.anon,
            "POST",
            "/api/registro/verificar",
            Some(json!({"token": token})),
        )
        .await;
    assert_eq!(code, 400);
    assert!(
        !t.bool(&format!(
            "SELECT correo_verificado FROM usuarios WHERE id={id}"
        ))
        .await
    );
});

// Admin-created, imported and demo seed accounts count as verified (default).
parity!(test_admin_import_y_demo_quedan_verificados, |t| {
    assert!(
        t.bool("SELECT correo_verificado FROM usuarios WHERE username='admin.demo'")
            .await
    );
    assert!(
        t.bool("SELECT correo_verificado FROM usuarios WHERE username='estudiante.demo'")
            .await
    );
    let id = t.user("creado.admin", "ESTUDIANTE").await;
    assert!(
        t.bool(&format!(
            "SELECT correo_verificado FROM usuarios WHERE id={id}"
        ))
        .await
    );
});
