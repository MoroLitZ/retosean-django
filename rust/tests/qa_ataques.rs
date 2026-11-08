//! Package T — adversarial concurrency and CSRF regression suite (tests only).
//! Attacks the router in-process: a CSRF sweep over the route table and a set of
//! high-contention races that must collapse to a single, consistent outcome.
mod common;
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use common::*;
use serde_json::{Value, json};
use tower::ServiceExt;

/// Unsafe-method tokens as they appear in `rust/src/lib.rs`.
const UNSAFE_METHODS: [&str; 4] = ["post(", "put(", "patch(", "delete("];
/// Public POST endpoints that never ask for a session CSRF token; the same-origin
/// middleware (Origin / Sec-Fetch-Site) is what protects them instead.
const PUBLIC_POST: [&str; 4] = [
    "/api/login",
    "/api/registro",
    "/api/password/reset",
    "/api/password/reset/confirm",
];
/// The tables whose row counts form the mutation checksum around the CSRF sweep.
const TABLES: [&str; 16] = [
    "usuarios",
    "empresas",
    "retos",
    "postulaciones",
    "entregables",
    "presupuestos",
    "gastos",
    "notificaciones",
    "historial_estados",
    "log_actividad",
    "votaciones_hackaton",
    "inscripciones_hackaton",
    "jurados_hackaton",
    "hackathons",
    "equipos",
    "integraciones_academicas",
];

/// Index of the `)` balancing the `(` at `open`.
fn matching_paren(src: &str, open: usize) -> usize {
    let mut depth = 0i32;
    for (i, b) in src.as_bytes()[open..].iter().enumerate() {
        match b {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return open + i;
                }
            }
            _ => {}
        }
    }
    src.len().saturating_sub(1)
}

/// Every unsafe-method route registered in the router, straight from the source.
fn unsafe_routes() -> Vec<String> {
    let src = include_str!("../src/lib.rs");
    let mut routes = Vec::new();
    let mut search = 0usize;
    while let Some(found) = src[search..].find(".route(") {
        let start = search + found;
        let open = start + ".route(".len() - 1;
        let close = matching_paren(src, open);
        let block = &src[open..=close];
        // The first string literal in a `.route(...)` registration is the path.
        if let Some(q) = block.find('"') {
            let rest = &block[q + 1..];
            if let Some(q2) = rest.find('"') {
                let path = &rest[..q2];
                if UNSAFE_METHODS.iter().any(|m| block.contains(m)) {
                    routes.push(path.to_string());
                }
            }
        }
        search = close + 1;
    }
    routes
}

/// Replace every `{param}` with the seeded id `1`.
fn substitute(path: &str) -> String {
    path.split(['{', '}'])
        .enumerate()
        .map(|(i, part)| if i % 2 == 1 { "1" } else { part })
        .collect()
}

async fn admin_post(app: &Router, session: &Session, path: &str, csrf: Option<&str>) -> StatusCode {
    let mut builder = Request::builder()
        .method("POST")
        .uri(path)
        .header("cookie", &session.cookie)
        .header("content-type", "application/json");
    if let Some(token) = csrf {
        builder = builder.header("x-csrf-token", token);
    }
    app.clone()
        .oneshot(builder.body(Body::from("{}")).unwrap())
        .await
        .unwrap()
        .status()
}

async fn raw_post(app: Router, session: Session, path: String, body: Value) -> StatusCode {
    app.oneshot(
        Request::builder()
            .method("POST")
            .uri(&path)
            .header("cookie", &session.cookie)
            .header("x-csrf-token", &session.csrf)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
    .status()
}

async fn raw_login(app: Router, username: String, password: String) -> StatusCode {
    app.oneshot(
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
    .unwrap()
    .status()
}

/// Fire `n` independent requests at once and collect their status codes.
async fn race_statuses<F, Fut>(n: usize, fut: F) -> Vec<StatusCode>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = StatusCode> + Send + 'static,
{
    let mut set = tokio::task::JoinSet::new();
    for _ in 0..n {
        set.spawn(fut());
    }
    let mut out = Vec::with_capacity(n);
    while let Some(res) = set.join_next().await {
        out.push(res.expect("request task panicked"));
    }
    out
}

fn count(statuses: &[StatusCode], want: StatusCode) -> usize {
    statuses.iter().filter(|s| **s == want).count()
}

async fn checksum(t: &Ctx) -> Vec<(String, i64)> {
    let mut snapshot = Vec::with_capacity(TABLES.len());
    for table in TABLES {
        snapshot.push((
            table.to_string(),
            t.i64(&format!("SELECT count(*) FROM {table}")).await,
        ));
    }
    snapshot
}

// 1. CSRF sweep: an authenticated-but-tokenless request to any unsafe route is
// never a 2xx and the whole sweep leaves the main tables untouched.
parity!(csrf_sweep_sin_token_y_con_token_erroneo, |t| {
    let bad = "deadbeefdeadbeefdeadbeefdeadbeef";
    let routes = unsafe_routes();
    let protected: Vec<&String> = routes
        .iter()
        .filter(|r| !PUBLIC_POST.contains(&r.as_str()))
        .collect();
    assert!(
        protected.len() >= 65,
        "route parser found only {} unsafe routes",
        protected.len()
    );
    let before = checksum(&t).await;
    for route in protected {
        let path = substitute(route);
        let missing = admin_post(&t.app, &t.admin, &path, None).await;
        let wrong = admin_post(&t.app, &t.admin, &path, Some(bad)).await;
        assert!(
            !missing.is_success(),
            "{path} without CSRF returned {missing}"
        );
        assert!(
            !wrong.is_success(),
            "{path} with a wrong CSRF returned {wrong}"
        );
    }
    let after = checksum(&t).await;
    assert_eq!(before, after, "CSRF sweep mutated the main tables");
});

// 2. The public endpoints reject a browser request from another site with 403.
parity!(test_publicos_de_otro_sitio_rechazados, |t| {
    for path in PUBLIC_POST {
        let response = t
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .header("content-type", "application/json")
                    .header("origin", "https://evil.example")
                    .header("sec-fetch-site", "cross-site")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{path}");
    }
});

// 3. Parallel wrong-password logins for one account: at most ten reach the
// password check, the rest are throttled with 429.
parity!(test_login_paralelo_limita_a_diez_intentos, |t| {
    let id = t.user("race.login", "ESTUDIANTE").await;
    let subject = retosean::auth::digest(&format!("cuenta:{id}"));
    let app = t.app.clone();
    let statuses = race_statuses(20, {
        let app = app.clone();
        move || raw_login(app.clone(), "race.login".into(), "clave-incorrecta".into())
    })
    .await;
    let evaluated = count(&statuses, StatusCode::UNAUTHORIZED);
    let throttled = count(&statuses, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        evaluated + throttled,
        20,
        "unexpected statuses: {statuses:?}"
    );
    assert_eq!(
        evaluated, 10,
        "exactly ten attempts are evaluated: {statuses:?}"
    );
    assert_eq!(
        throttled, 10,
        "the remaining ten are throttled: {statuses:?}"
    );
    assert_eq!(
        t.i64(&format!(
            "SELECT count(*) FROM rust_login_attempts WHERE subject='{subject}'"
        ))
        .await,
        10
    );
});

// 4. One password-reset token confirmed ten times at once: exactly one succeeds.
parity!(test_confirmacion_token_concurrente_sola_una_vez, |t| {
    let id = t.user("race.reset", "ESTUDIANTE").await;
    let token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    sqlx::query("INSERT INTO rust_password_resets(token_hash,user_id,expires_at) VALUES($1,$2,now()+interval '3 days')")
        .bind(retosean::auth::digest(&token))
        .bind(id)
        .execute(&t.pool)
        .await
        .unwrap();
    let app = t.app.clone();
    let body = json!({"token": token, "nueva": "ClaveRestablecida2026", "nueva2": "ClaveRestablecida2026"});
    let statuses = race_statuses(10, {
        let app = app.clone();
        let body = body.clone();
        move || {
            raw_post(
                app.clone(),
                Session::default(),
                "/api/password/reset/confirm".into(),
                body.clone(),
            )
        }
    })
    .await;
    let ok = statuses.iter().filter(|s| s.is_success()).count();
    assert_eq!(ok, 1, "exactly one confirmation succeeds: {statuses:?}");
    assert_eq!(t.i64("SELECT count(*) FROM rust_password_resets").await, 0);
});

/// A published hackathon with an accepted team and a registered juror, ready to vote.
async fn hackathon_vote_setup(t: &Ctx) -> (i64, i64, i64, Session) {
    let teacher = t.user("race.prof", "PROFESOR").await;
    let reto = t.reto(COMPANY_USER, "en_curso", "hackathon").await;
    let integration = t.integrate(reto, teacher).await;
    t.exec(&format!(
        "UPDATE integraciones_academicas SET estado='aprobada' WHERE id={integration}"
    ))
    .await;
    let hack = t.i64(&format!("INSERT INTO hackathons(reto_id,reglas,estado,resultados_publicados,creado_en,actualizado_en) VALUES({reto},'','publicado',false,now(),now()) RETURNING id")).await;
    let team = t.i64(&format!("INSERT INTO equipos(reto_id,nombre,creado_en) VALUES({reto},'Equipo Carrera',now()) RETURNING id")).await;
    t.exec(&format!("INSERT INTO inscripciones_hackaton(hackathon_id,equipo_id,estado,inscrito_por_id,creado_en) VALUES({hack},{team},'ACEPTADA',{ADMIN},now())")).await;
    t.exec(&format!("INSERT INTO etapas_hackaton(hackathon_id,tipo,titulo,inicia_en,termina_en,orden) VALUES({hack},'presentacion','Presentación',now()-interval '1 day',now()+interval '1 day',1)")).await;
    let juror = t.user("race.jurado", "PROFESOR").await;
    t.exec(&format!("INSERT INTO jurados_hackaton(hackathon_id,usuario_id,nombre,email,especialidad) VALUES({hack},{juror},'J','','')")).await;
    let session = t.login("race.jurado").await;
    (hack, team, juror, session)
}

// 5. A juror voting the same team concurrently: exactly one vote row survives.
parity!(test_voto_jurado_concurrente_una_fila, |t| {
    let (hack, team, _, session) = hackathon_vote_setup(&t).await;
    let app = t.app.clone();
    let path = format!("/api/hackathons/{hack}/votar");
    let body = json!({"equipo_id": team, "puntaje": "4.50"});
    let statuses = race_statuses(10, {
        let app = app.clone();
        let session = session.clone();
        let path = path.clone();
        let body = body.clone();
        move || raw_post(app.clone(), session.clone(), path.clone(), body.clone())
    })
    .await;
    assert!(
        statuses.iter().all(|s| s.is_success()),
        "votes: {statuses:?}"
    );
    assert_eq!(
        t.i64(&format!(
            "SELECT count(*) FROM votaciones_hackaton WHERE hackathon_id={hack} AND equipo_id={team}"
        ))
        .await,
        1
    );
});

// 6. A student applying twice to the same reto concurrently: one postulacion.
parity!(test_postulacion_concurrente_una_fila, |t| {
    let reto = t.reto(COMPANY_USER, "abierto", "reto").await;
    let student = t.user("race.post", "ESTUDIANTE").await;
    let session = t.login("race.post").await;
    let app = t.app.clone();
    let path = format!("/api/retos/{reto}/postular");
    let body = json!({"motivacion": "Quiero participar", "acepta_terminos": true});
    let statuses = race_statuses(10, {
        let app = app.clone();
        let session = session.clone();
        let path = path.clone();
        let body = body.clone();
        move || raw_post(app.clone(), session.clone(), path.clone(), body.clone())
    })
    .await;
    assert!(
        statuses.iter().all(|s| s.is_success()),
        "applications: {statuses:?}"
    );
    assert_eq!(
        t.i64(&format!(
            "SELECT count(*) FROM postulaciones WHERE reto_id={reto} AND estudiante_id={student}"
        ))
        .await,
        1
    );
});

// 7. Concurrent expenses against one budget never exceed it (the cap in
// `finance::expense` locks the budget row FOR UPDATE).
parity!(test_gastos_concurrentes_no_superan_presupuesto, |t| {
    let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
    let budget = t.i64(&format!("INSERT INTO presupuestos(reto_id,monto_total,alerta_80_enviada,creado_en,actualizado_en) VALUES({reto},1000,false,now(),now()) RETURNING id")).await;
    let app = t.app.clone();
    let path = format!("/api/presupuestos/{budget}/gastos");
    let body = json!({"categoria": "MATERIALES", "monto": "100", "estado": "EJECUTADO"});
    let statuses = race_statuses(20, {
        let app = app.clone();
        let session = t.admin.clone();
        let path = path.clone();
        let body = body.clone();
        move || raw_post(app.clone(), session.clone(), path.clone(), body.clone())
    })
    .await;
    let ok = statuses.iter().filter(|s| s.is_success()).count();
    assert_eq!(ok, 10, "only ten of 20 x 100 fit in 1000: {statuses:?}");
    assert_eq!(
        t.i64(&format!(
            "SELECT count(*) FROM gastos WHERE presupuesto_id={budget}"
        ))
        .await,
        10
    );
    assert_eq!(
        t.text(&format!(
            "SELECT sum(monto)::text FROM gastos WHERE presupuesto_id={budget}"
        ))
        .await,
        "1000.00"
    );
});

// 8. Concurrent approvals of one reto: exactly one history row for the transition.
parity!(test_aprobacion_concurrente_una_fila_historial, |t| {
    let (owner, company) = t.company_user("race.empresa", "900700050").await;
    t.verify_documents(company).await;
    let reto = t.reto(owner, "en_revision", "reto").await;
    let app = t.app.clone();
    let path = format!("/api/retos/{reto}/estado");
    let body = json!({"estado": "aprobado", "comentario": "Ok"});
    let statuses = race_statuses(10, {
        let app = app.clone();
        let session = t.admin.clone();
        let path = path.clone();
        let body = body.clone();
        move || raw_post(app.clone(), session.clone(), path.clone(), body.clone())
    })
    .await;
    let ok = statuses.iter().filter(|s| s.is_success()).count();
    assert_eq!(ok, 1, "exactly one approval wins: {statuses:?}");
    assert_eq!(
        t.text(&format!("SELECT estado FROM retos WHERE id={reto}"))
            .await,
        "aprobado"
    );
    let history = t.history("retos", "reto", reto, "estado").await;
    let transitions = history
        .iter()
        .filter(|(from, to, _)| from == "en_revision" && to == "aprobado")
        .count();
    assert_eq!(transitions, 1, "one history row: {history:?}");
});
