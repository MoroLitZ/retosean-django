//! Real HTTP handlers and PostgreSQL, isolated in a disposable schema.
//! Run explicitly: cargo test --test end_to_end -- --ignored --nocapture
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use retosean::{AppState, config::Config};
use serde_json::{Value, json};
use sqlx::{Executor, postgres::PgPoolOptions};
use tower::ServiceExt;

#[derive(Clone, Default)]
struct Session {
    cookie: String,
    csrf: String,
}
async fn request(
    app: &Router,
    session: &Session,
    method: &str,
    path: &str,
    data: Option<Value>,
) -> (StatusCode, Value, Option<String>) {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("cookie", &session.cookie)
        .header("x-csrf-token", &session.csrf);
    let body = if let Some(data) = data {
        builder = builder.header("content-type", "application/json");
        Body::from(data.to_string())
    } else {
        Body::empty()
    };
    let response = app
        .clone()
        .oneshot(builder.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let cookie = response
        .headers()
        .get("set-cookie")
        .map(|s| s.to_str().unwrap().split(';').next().unwrap().to_string());
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| json!({"raw":String::from_utf8_lossy(&bytes)}));
    (status, value, cookie)
}
async fn ok(
    app: &Router,
    session: &Session,
    method: &str,
    path: &str,
    data: Option<Value>,
) -> Value {
    let (status, body, _) = request(app, session, method, path, data).await;
    assert!(status.is_success(), "{method} {path}: {status} {body}");
    body
}
async fn login(app: &Router, name: &str) -> Session {
    let (status, body, cookie) = request(
        app,
        &Session::default(),
        "POST",
        "/api/login",
        Some(json!({"username":name,"password":"RetosEAN2026"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    Session {
        cookie: cookie.unwrap(),
        csrf: body["csrf"].as_str().unwrap().into(),
    }
}

#[tokio::test]
#[ignore = "requires local PostgreSQL; creates and removes its own isolated schema"]
async fn native_http_lifecycle_and_permissions() {
    let mut config = Config::load().unwrap();
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
        .max_connections(5)
        .after_connect(move |conn, _| {
            let statement = format!("SET search_path TO {search_path}");
            Box::pin(async move {
                conn.execute(statement.as_str()).await?;
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
    let hash = "pbkdf2_sha256$1000$test-salt$CqJFhYAc+mBaD+E6+znRWXbr3hP/JgZjkUaazakv+Ak=";
    sqlx::raw_sql(&include_str!("../sql/demo.sql").replace("__DEMO_PASSWORD_HASH__", hash))
        .execute(&pool)
        .await
        .unwrap();
    retosean::auth::init(&pool).await.unwrap();
    config.media_root = std::env::temp_dir().join(&schema);
    let storage = config.media_root.clone();
    let app = retosean::router(AppState {
        pool: pool.clone(),
        config,
        password_slots: std::sync::Arc::new(tokio::sync::Semaphore::new(4)),
    });
    let test_pool = pool.clone();
    let result = std::panic::AssertUnwindSafe(async move {
        let pool = test_pool;
        assert_eq!(
            request(&app, &Session::default(), "GET", "/api/resources", None)
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
        let administrator = login(&app, "admin.demo").await;
        let company = login(&app, "empresa.verificada").await;
        let teacher = login(&app, "profesor.demo").await;
        let student = login(&app, "estudiante.demo").await;
        let expert = login(&app, "experto.demo").await;
        // The login field says "Usuario o correo": the email works too (case-insensitive).
        let by_email = login(&app, "Profesor.Demo@Example.Test").await;
        assert_eq!(
            ok(&app, &by_email, "GET", "/api/me", None).await["user"]["username"],
            "profesor.demo"
        );
        // Experts get the juror dashboard, not the "no role" notice.
        let panel = ok(&app, &expert, "GET", "/api/dashboard", None).await;
        assert_eq!(panel["sin_rol"], false, "{panel}");
        assert_eq!(panel["kpis"]["hackathones_asignados"], 0, "{panel}");
        for resource in retosean::resources::schema() {
            let name = resource["table"].as_str().unwrap();
            ok(
                &app,
                &administrator,
                "GET",
                &format!("/api/records/{name}"),
                None,
            )
            .await;
        }
        let faculty = ok(
            &app,
            &administrator,
            "POST",
            "/api/catalogos/facultades",
            Some(json!({"nombre":"Facultad Rust","codigo":"RUST"})),
        )
        .await["id"]
            .as_i64()
            .unwrap();
        let program = ok(
            &app,
            &administrator,
            "POST",
            "/api/catalogos/programas",
            Some(json!({"nombre":"Programa Rust","facultad_id":faculty})),
        )
        .await["id"]
            .as_i64()
            .unwrap();
        ok(&app,&administrator,"POST","/api/catalogos/unidades_estudio",Some(json!({"nombre":"Sistemas seguros","codigo":"RS-01","programa_id":program,"periodo":"2026-2","ciclo":"6"}))).await;
        ok(
            &app,
            &administrator,
            "POST",
            "/api/catalogos/ecosistemas",
            Some(json!({"nombre":"Innovación Rust","descripcion":"Ecosistema de prueba"})),
        )
        .await;
        assert_eq!(
            request(
                &app,
                &student,
                "POST",
                "/api/catalogos/facultades",
                Some(json!({"nombre":"Intento bloqueado","codigo":"NO"}))
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
        let new_user=ok(&app,&administrator,"POST","/api/usuarios",Some(json!({"username":"nuevo.estudiante","password":"UnaClaveSegura2026","email":"nuevo@example.test","first_name":"Nuevo","last_name":"Estudiante","rol":"ESTUDIANTE","programa_id":program}))).await["id"].as_i64().unwrap();
        ok(
            &app,
            &administrator,
            "POST",
            &format!("/api/usuarios/{new_user}/estado"),
            Some(json!({"activo":false})),
        )
        .await;
        assert!(
            !sqlx::query_scalar::<_, bool>("SELECT is_active FROM usuarios WHERE id=$1")
                .bind(new_user)
                .fetch_one(&pool)
                .await
                .unwrap()
        );
        ok(&app,&student,"POST","/api/perfil",Some(json!({"first_name":"Estudiante","last_name":"Actualizado","email":"estudiante.actualizado@example.test"}))).await;
        ok(&app,&student,"POST","/api/preferencias",Some(json!({"recibir_email":false,"recibir_recordatorios":true,"eventos_silenciados":["EVENTO_SILENCIADO"]}))).await;
        assert_eq!(request(&app,&administrator,"POST","/api/empresas/1/revisar",Some(json!({"estado_validacion":"VERIFICADA","estado_listas_restrictivas":"APROBADO"}))).await.0,StatusCode::BAD_REQUEST);
        // HU00: every mandatory catalog document must be verified and valid.
        let required: Vec<String> = sqlx::query_scalar(
            "SELECT codigo FROM tipos_documento_legal WHERE activo AND obligatorio ORDER BY orden",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(required.len(), 8, "the brief's eight documents");
        let issued = (chrono::Utc::now() - chrono::Duration::hours(5)).date_naive();
        for kind in &required {
            let document:i64=sqlx::query_scalar("INSERT INTO documentos_empresas(empresa_id,tipo_documento,estado,fecha_carga) VALUES(1,$1,'PENDIENTE',now()) RETURNING id").bind(kind).fetch_one(&pool).await.unwrap();
            let multipart = format!(
                "--DOC\r\nContent-Disposition: form-data; name=\"fecha_expedicion\"\r\n\r\n{issued}\r\n--DOC\r\nContent-Disposition: form-data; name=\"archivo\"; filename=\"legal.pdf\"\r\nContent-Type: application/pdf\r\n\r\n%PDF-1.4 documento de prueba\r\n--DOC--\r\n"
            );
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(format!("/api/documentos/{document}/cargar"))
                        .header("cookie", &company.cookie)
                        .header("x-csrf-token", &company.csrf)
                        .header("content-type", "multipart/form-data; boundary=DOC")
                        .body(Body::from(multipart))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            ok(
                &app,
                &administrator,
                "POST",
                &format!("/api/documentos/{document}/revisar"),
                Some(json!({"estado":"VERIFICADO"})),
            )
            .await;
        }
        ok(&app,&administrator,"POST","/api/empresas/1/revisar",Some(json!({"estado_validacion":"VERIFICADA","estado_listas_restrictivas":"APROBADO","observaciones":"Documentos verificados"}))).await;
        for status in ["RADICADO", "EN_JURIDICA", "FIRMADO"] {
            ok(
                &app,
                &administrator,
                "POST",
                "/api/empresas/1/convenio",
                Some(json!({"estado":status,"fecha_vencimiento":"2099-12-31"})),
            )
            .await;
        }
        let private = ok(&app, &student, "GET", "/api/records/usuarios", None).await;
        assert_eq!(private["total"], 1);
        assert!(private["records"][0].get("password").is_none());
        assert_eq!(
            request(&app, &student, "GET", "/api/records/usuarios/1", None)
                .await
                .0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            request(&app, &student, "GET", "/api/records/log_actividad", None)
                .await
                .0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            request(
                &app,
                &student,
                "GET",
                "/api/records/usuarios%3BDROP%20TABLE%20usuarios",
                None
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
        let no_csrf = Session {
            cookie: company.cookie.clone(),
            csrf: String::new(),
        };
        assert_eq!(
            request(
                &app,
                &no_csrf,
                "POST",
                "/api/retos",
                Some(json!({"titulo":"x","descripcion":"x"}))
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
        let challenge=ok(&app,&company,"POST","/api/retos",Some(json!({"titulo":"Reto de integración Rust","tipo":"hackathon","descripcion":"Validar el ciclo completo sin Python","area":"Tecnología","fecha_inicio_tentativa":"2026-10-14","fecha_fin_tentativa":"2099-12-31","fecha_limite_postulacion":"2099-11-30"}))).await;
        let id = challenge["id"].as_i64().unwrap();
        assert_eq!(
            request(
                &app,
                &student,
                "POST",
                &format!("/api/retos/{id}/estado"),
                Some(json!({"estado":"aprobado"}))
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
        ok(
            &app,
            &company,
            "POST",
            &format!("/api/retos/{id}/estado"),
            Some(json!({"estado":"en_revision"})),
        )
        .await;
        ok(
            &app,
            &administrator,
            "POST",
            &format!("/api/retos/{id}/estado"),
            Some(json!({"estado":"aprobado"})),
        )
        .await;
        let integration=ok(&app,&teacher,"POST",&format!("/api/retos/{id}/integrar"),Some(json!({"facultad":"Ingeniería","nivel_formacion":"Pregrado","programa_academico":"Sistemas","descripcion":"Proyecto aplicado","alcance":"Prototipo","entregable_esperado":"Informe","cronograma_sesiones":"Semanal"}))).await;
        let integration = integration["id"].as_i64().unwrap();
        // HU03: without a team and the required sessions it cannot go to review.
        let (status, body, _) = request(
            &app,
            &teacher,
            "POST",
            &format!("/api/integraciones/{integration}/estado"),
            Some(json!({"estado":"en_revision"})),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert!(
            body["error"]
                .as_str()
                .unwrap()
                .contains("al menos un profesor"),
            "{body}"
        );
        let (status, _, _) = request(&app, &teacher, "POST", &format!("/api/integraciones/{integration}/equipo"), Some(json!({"participantes":[{"rol":"ESTUDIANTE","nombre":"Sin programa","documento":"1","email":"x@ean.edu.co"}]}))).await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "each role requires its data"
        );
        ok(&app, &teacher, "POST", &format!("/api/integraciones/{integration}/equipo"), Some(json!({
            "participantes": [
                {"rol":"PROFESOR","nombre":"Profesor Demo","documento":"10","email":"profe.titular@ean.edu.co","facultad":"Ingeniería"},
                {"rol":"ESTUDIANTE","nombre":"Estudiante","documento":"20","email":"estudiante.actualizado@example.test","programa":"Sistemas"},
                {"rol":"EXPERTO","nombre":"Mentora","documento":"30","email":"mentora@externo.co","facultad":"Innovación SAS"}
            ],
            "sesiones": [
                {"tipo":"inicio","fecha_hora":"2026-11-02T14:00:00Z","enlace_reunion":"https://meet.example/abc"},
                {"tipo":"seguimiento","fecha_hora":"2026-11-16T14:00:00Z"},
                {"tipo":"evaluacion","fecha_hora":"2026-11-30T14:00:00Z"}
            ]
        }))).await;
        for (session, state) in [
            (&teacher, "en_revision"),
            (&administrator, "aprobada"),
            (&teacher, "publicada"),
        ] {
            ok(
                &app,
                session,
                "POST",
                &format!("/api/integraciones/{integration}/estado"),
                Some(json!({"estado":state})),
            )
            .await;
        }
        assert_eq!(
            ok(
                &app,
                &student,
                "GET",
                &format!("/api/records/retos/{id}"),
                None
            )
            .await["estado"],
            "abierto"
        );
        // Publishing told the linked student once, and the schedule is visible to them.
        let team = ok(
            &app,
            &student,
            "GET",
            &format!("/api/integraciones/{integration}/equipo"),
            None,
        )
        .await;
        assert_eq!(team["sesiones"].as_array().unwrap().len(), 3);
        assert_eq!(
            team["participantes"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|p| p["cuenta"] == true)
                .count(),
            1
        );
        let notices = ok(
            &app,
            &student,
            "GET",
            "/api/records/notificaciones?q=EQUIPO_ASIGNADO",
            None,
        )
        .await;
        assert_eq!(notices["total"], 1, "{notices}");
        ok(
            &app,
            &student,
            "POST",
            &format!("/api/retos/{id}/favorito"),
            Some(json!({})),
        )
        .await;
        let application = ok(
            &app,
            &student,
            "POST",
            &format!("/api/retos/{id}/postular"),
            Some(json!({"motivacion":"Quiero aprender Rust","semestre":6})),
        )
        .await;
        let application = application["id"].as_i64().unwrap();
        ok(
            &app,
            &company,
            "POST",
            &format!("/api/postulaciones/{application}/estado"),
            Some(json!({"estado":"ACEPTADA"})),
        )
        .await;
        assert_eq!(
            request(
                &app,
                &student,
                "POST",
                &format!("/api/retos/{id}/postular"),
                Some(json!({"motivacion":"Intento de reapertura"}))
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
        ok(
            &app,
            &administrator,
            "POST",
            &format!("/api/retos/{id}/estado"),
            Some(json!({"estado":"en_curso"})),
        )
        .await;
        let multipart = "--BOUNDARY\r\nContent-Disposition: form-data; name=\"titulo\"\r\n\r\nInforme final\r\n--BOUNDARY\r\nContent-Disposition: form-data; name=\"archivo\"; filename=\"informe.txt\"\r\nContent-Type: text/plain\r\n\r\nPrototipo listo para revisión.\r\n--BOUNDARY--\r\n";
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/retos/{id}/entregar"))
                    .header("cookie", &student.cookie)
                    .header("x-csrf-token", &student.csrf)
                    .header("content-type", "multipart/form-data; boundary=BOUNDARY")
                    .body(Body::from(multipart))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body: Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(status, StatusCode::OK, "{body}");
        let deliverable = body["id"].as_i64().unwrap();
        ok(
            &app,
            &teacher,
            "POST",
            &format!("/api/entregables/{deliverable}/calificar"),
            Some(json!({"nota":"4.50","comentario":"Buen trabajo"})),
        )
        .await;
        assert_eq!(
            request(
                &app,
                &teacher,
                "POST",
                &format!("/api/entregables/{deliverable}/calificar"),
                Some(json!({"nota":"5.01"}))
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
        // HU12: unpublished grades are hidden from the student.
        let mine = ok(
            &app,
            &student,
            "GET",
            &format!("/api/records/entregables/{deliverable}"),
            None,
        )
        .await;
        assert!(
            mine["nota"].is_null() && mine["comentario_profesor"].is_null(),
            "{mine}"
        );
        let (status, body, _) = request(
            &app,
            &teacher,
            "POST",
            &format!("/api/entregables/{deliverable}/calificar"),
            Some(json!({"nota":"3"})),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            body["error"].as_str().unwrap().contains("Confirma"),
            "re-grading needs confirmation"
        );
        assert_eq!(
            request(
                &app,
                &teacher,
                "POST",
                &format!("/api/entregables/{deliverable}/calificar"),
                Some(json!({"nota":"5.01","confirmar_recalificacion":true}))
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
        // Rubric grading: the score is the sum of the criteria.
        ok(&app, &teacher, "POST", &format!("/api/retos/{id}/rubrica"), Some(json!({"nombre":"Rúbrica final","criterios":[{"nombre":"Calidad","puntaje_maximo":"2.5"},{"nombre":"Presentación","puntaje_maximo":"2.5"}]}))).await;
        let rubrics = ok(
            &app,
            &teacher,
            "GET",
            &format!("/api/entregables/{deliverable}/rubricas"),
            None,
        )
        .await;
        let rubric = &rubrics["rubricas"][0];
        let (c1, c2) = (
            rubric["criterios"][0]["id"].to_string(),
            rubric["criterios"][1]["id"].to_string(),
        );
        assert_eq!(request(&app, &teacher, "POST", &format!("/api/entregables/{deliverable}/calificar"), Some(json!({"rubrica_id":rubric["id"],"criterios":{c1.clone():"3",c2.clone():"1"},"confirmar_recalificacion":true}))).await.0, StatusCode::BAD_REQUEST, "criterion over its maximum");
        let graded = ok(&app, &teacher, "POST", &format!("/api/entregables/{deliverable}/calificar"), Some(json!({"rubrica_id":rubric["id"],"criterios":{c1:"2.25",c2:"1.75"},"comentario":"Sólido","confirmar_recalificacion":true}))).await;
        assert_eq!(graded["nota"], "4.00");
        let notices = |app: Router, student: Session| async move {
            ok(
                &app,
                &student,
                "GET",
                "/api/records/notificaciones?q=ENTREGABLE_CALIFICADO",
                None,
            )
            .await["total"]
                .as_i64()
                .unwrap()
        };
        assert_eq!(
            notices(
                app.clone(),
                Session {
                    cookie: student.cookie.clone(),
                    csrf: student.csrf.clone()
                }
            )
            .await,
            0,
            "not notified before publishing"
        );
        assert_eq!(
            ok(
                &app,
                &teacher,
                "POST",
                &format!("/api/retos/{id}/publicar-calificaciones"),
                Some(json!({}))
            )
            .await["publicadas"],
            1
        );
        let mine = ok(
            &app,
            &student,
            "GET",
            &format!("/api/records/entregables/{deliverable}"),
            None,
        )
        .await;
        assert_eq!(
            (mine["nota"].as_f64(), mine["comentario_profesor"].as_str()),
            (Some(4.0), Some("Sólido"))
        );
        assert_eq!(
            notices(
                app.clone(),
                Session {
                    cookie: student.cookie.clone(),
                    csrf: student.csrf.clone()
                }
            )
            .await,
            1
        );
        assert_eq!(
            request(
                &app,
                &company,
                "GET",
                &format!("/api/archivo/entregables/{deliverable}/archivo"),
                None
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            request(
                &app,
                &student,
                "GET",
                &format!("/api/archivo/entregables/{deliverable}/archivo"),
                None
            )
            .await
            .0,
            StatusCode::OK
        );
        let budget = ok(
            &app,
            &administrator,
            "POST",
            &format!("/api/retos/{id}/presupuesto"),
            Some(json!({"monto_total":"1000.00"})),
        )
        .await;
        let budget = budget["id"].as_i64().unwrap();
        for amount in ["800.00", "50.00"] {
            ok(
                &app,
                &administrator,
                "POST",
                &format!("/api/presupuestos/{budget}/gastos"),
                Some(json!({"categoria":"MATERIALES","monto":amount})),
            )
            .await;
        }
        let summary = ok(
            &app,
            &company,
            "GET",
            &format!("/api/presupuestos/{budget}/resumen"),
            None,
        )
        .await;
        assert_eq!(summary["porcentaje"], "85.00");
        let alerts: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM notificaciones WHERE usuario_id=2 AND evento='PRESUPUESTO_80'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(alerts, 1);
        let team = ok(
            &app,
            &teacher,
            "POST",
            &format!("/api/retos/{id}/equipo"),
            Some(json!({"nombre":"Equipo Rust","estudiantes":[4]})),
        )
        .await["id"]
            .as_i64()
            .unwrap();
        let hack = ok(
            &app,
            &administrator,
            "POST",
            &format!("/api/retos/{id}/hackathon"),
            Some(json!({"reglas":"Prototipo y presentación"})),
        )
        .await["id"]
            .as_i64()
            .unwrap();
        ok(
            &app,
            &teacher,
            "POST",
            &format!("/api/hackathons/{hack}/jurado"),
            Some(json!({"usuario_id":5,"especialidad":"Rust"})),
        )
        .await;
        ok(
            &app,
            &teacher,
            "POST",
            &format!("/api/hackathons/{hack}/estado"),
            Some(json!({"estado":"publicado"})),
        )
        .await;
        let registration = ok(
            &app,
            &student,
            "POST",
            &format!("/api/hackathons/{hack}/inscribir"),
            Some(json!({"equipo_id":team})),
        )
        .await["id"]
            .as_i64()
            .unwrap();
        assert_eq!(
            request(
                &app,
                &expert,
                "POST",
                &format!("/api/hackathons/{hack}/votar"),
                Some(json!({"equipo_id":team,"puntaje":"4.75"}))
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
        ok(
            &app,
            &teacher,
            "POST",
            &format!("/api/inscripciones/{registration}/estado"),
            Some(json!({"estado":"ACEPTADA"})),
        )
        .await;
        ok(
            &app,
            &expert,
            "POST",
            &format!("/api/hackathons/{hack}/votar"),
            Some(json!({"equipo_id":team,"puntaje":"4.75"})),
        )
        .await;
        assert_eq!(
            request(
                &app,
                &student,
                "GET",
                &format!("/api/hackathons/{hack}/ranking"),
                None
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
        // HU09 logistics: the format decides what is required.
        let (status, _, _) = request(
            &app,
            &teacher,
            "POST",
            &format!("/api/hackathons/{hack}/logistica"),
            Some(json!({"modalidad":"virtual"})),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "a virtual event needs a streaming link"
        );
        ok(&app, &teacher, "POST", &format!("/api/hackathons/{hack}/logistica"), Some(json!({
            "modalidad":"hibrida","lugar":"Auditorio EAN","enlace_transmision":"https://stream.example/live",
            "invitados":[{"tipo":"MENTOR","nombre":"Ana Mentora","organizacion":"ACME","confirmado":true}]
        }))).await;
        assert_eq!(
            request(
                &app,
                &student,
                "POST",
                &format!("/api/hackathons/{hack}/logistica"),
                Some(json!({"modalidad":"virtual","enlace_transmision":"https://x.example"}))
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
        // HU09 schedule: development started and the awards stage has begun, so
        // opening the ranking starts, closes and publishes the results by itself.
        for (kind, start, order) in [
            ("desarrollo", "2026-01-01T00:00:00Z", 2),
            ("premiacion", "2026-01-03T00:00:00Z", 4),
        ] {
            ok(
                &app,
                &teacher,
                "POST",
                &format!("/api/hackathons/{hack}/etapa"),
                Some(json!({"tipo":kind,"titulo":kind,"inicia_en":start,"orden":order})),
            )
            .await;
        }
        let ranking = ok(
            &app,
            &student,
            "GET",
            &format!("/api/hackathons/{hack}/ranking"),
            None,
        )
        .await;
        assert_eq!(ranking["ranking"][0]["equipo"], "Equipo Rust");
        let finished = ok(
            &app,
            &administrator,
            "GET",
            &format!("/api/records/hackathons/{hack}"),
            None,
        )
        .await;
        assert_eq!(
            (
                finished["estado"].as_str(),
                finished["resultados_publicados"].as_bool()
            ),
            (Some("finalizado"), Some(true))
        );
        let results = ok(
            &app,
            &student,
            "GET",
            "/api/records/notificaciones?q=HACKATON_RESULTADOS",
            None,
        )
        .await;
        assert_eq!(results["total"], 1, "{results}");
        // Final report for managers and the reto's company; not for students.
        for format in ["pdf", "xlsx"] {
            let (status, _, _) = request(
                &app,
                &company,
                "GET",
                &format!("/api/hackathons/{hack}/informe/{format}"),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{format}");
        }
        assert_eq!(
            request(
                &app,
                &student,
                "GET",
                &format!("/api/hackathons/{hack}/informe/pdf"),
                None
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
        ok(&app,&teacher,"POST",&format!("/api/retos/{id}/seguimiento"),Some(json!({"fecha_sesion":"2026-10-14","porcentaje_avance":100,"avances":"Prototipo entregado"}))).await;
        let closed = ok(
            &app,
            &administrator,
            "POST",
            &format!("/api/retos/{id}/cerrar"),
            Some(json!({"notas":"Cierre verificado","horas":40})),
        )
        .await;
        assert_eq!(closed["certificados_emitidos"], 4);
        let repeat = ok(
            &app,
            &administrator,
            "POST",
            &format!("/api/retos/{id}/cerrar"),
            Some(json!({"notas":"Cierre verificado","horas":40})),
        )
        .await;
        assert_eq!(repeat["certificados_emitidos"], 0);
        let certificate: uuid::Uuid = sqlx::query_scalar(
            "SELECT codigo_verificacion FROM certificados WHERE reto_id=$1 AND usuario_id=4",
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let choices = ok(
            &app,
            &teacher,
            "GET",
            &format!("/api/retos/{id}/opciones"),
            None,
        )
        .await;
        assert!(
            choices["students"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p["value"] == 4)
        );
        ok(
            &app,
            &Session::default(),
            "GET",
            &format!("/api/verificar/{certificate}"),
            None,
        )
        .await;
        for path in [
            "/api/export/retos/pdf".to_string(),
            "/api/export/retos/xlsx".to_string(),
            "/api/export/retos".to_string(),
            format!(
                "/api/certificados/{}/pdf",
                sqlx::query_scalar::<_, i64>(
                    "SELECT id FROM certificados WHERE codigo_verificacion=$1"
                )
                .bind(certificate)
                .fetch_one(&pool)
                .await
                .unwrap()
            ),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(&path)
                        .header("cookie", &student.cookie)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{path}");
            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            assert!(bytes.len() > 50);
            if path.ends_with("pdf") {
                assert!(bytes.starts_with(b"%PDF-"));
            }
            if path.ends_with("xlsx") {
                assert!(bytes.starts_with(b"PK"));
            }
            let artifacts =
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/test-artifacts");
            std::fs::create_dir_all(&artifacts).unwrap();
            let filename = if path.contains("certificados") {
                "certificado.pdf"
            } else if path.ends_with("xlsx") {
                "retos.xlsx"
            } else if path.ends_with("pdf") {
                "retos.pdf"
            } else {
                "retos.csv"
            };
            std::fs::write(artifacts.join(filename), &bytes).unwrap();
        }
        let survey:i64=sqlx::query_scalar("SELECT id FROM cierre_encuestasatisfaccion WHERE cierre_id=(SELECT id FROM cierres_reto WHERE reto_id=$1) AND participante_id=4").bind(id).fetch_one(&pool).await.unwrap();
        ok(
            &app,
            &student,
            "POST",
            &format!("/api/encuestas/{survey}"),
            Some(json!({"calificacion":5,"comentario":"Excelente"})),
        )
        .await;
        assert_eq!(
            request(
                &app,
                &student,
                "POST",
                &format!("/api/encuestas/{survey}"),
                Some(json!({"calificacion":5}))
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            request(
                &app,
                &administrator,
                "POST",
                &format!("/api/retos/{id}/estado"),
                Some(json!({"estado":"en_curso"}))
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
        ok(
            &app,
            &expert,
            "POST",
            "/api/password",
            Some(json!({"actual":"RetosEAN2026","nueva":"ExpertoClaveNueva2026"})),
        )
        .await;
        assert_eq!(
            request(&app, &expert, "GET", "/api/me", None).await.0,
            StatusCode::UNAUTHORIZED
        );
        // Self-registration (vista_registro) and password recovery.
        let inbox = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        retosean::mail::install(retosean::mail::Mailer::Memory(inbox.clone()));
        let public = Session::default();
        let academic = json!({"tipo":"academico","rol":"ESTUDIANTE","username":"nueva.estudiante","email":"nueva@gmail.com","password":"ClaveSegura2026!","password2":"ClaveSegura2026!","first_name":"Nueva","last_name":"Estudiante","semestre":3});
        let (status, body, _) = request(
            &app,
            &public,
            "POST",
            "/api/registro",
            Some(academic.clone()),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "institutional email required: {body}"
        );
        let mut academic = academic;
        academic["email"] = json!("nueva@universidadean.edu.co");
        let (status, body, cookie) =
            request(&app, &public, "POST", "/api/registro", Some(academic)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let registered = Session {
            cookie: cookie.unwrap(),
            csrf: body["csrf"].as_str().unwrap().into(),
        };
        assert_eq!(
            ok(&app, &registered, "GET", "/api/me", None).await["user"]["rol"],
            "ESTUDIANTE"
        );
        let semester: i32 = sqlx::query_scalar("SELECT e.semestre FROM estudiantes e JOIN usuarios u ON u.id=e.usuario_id WHERE u.username='nueva.estudiante'").fetch_one(&pool).await.unwrap();
        assert_eq!(semester, 3);
        let company = json!({"tipo":"empresa","username":"acme.sas","email":"contacto@acme.co","password":"ClaveSegura2026!","password2":"ClaveSegura2026!","nit":"901234567","razon_social":"ACME S.A.S.","sector_industrial":"Tecnología","tipo_convocatoria":"AMBOS","area_responsable":"Innovación","sitio_web":"https://acme.co"});
        assert_eq!(
            request(
                &app,
                &public,
                "POST",
                "/api/registro",
                Some(company.clone())
            )
            .await
            .0,
            StatusCode::OK
        );
        let docs: i64 = sqlx::query_scalar("SELECT count(*) FROM documentos_empresas d JOIN empresas e ON e.id=d.empresa_id WHERE e.nit='901234567'").fetch_one(&pool).await.unwrap();
        assert_eq!(docs, 9, "one pending row per active catalog type");
        let mut again = company;
        again["username"] = json!("acme.dos");
        assert_eq!(
            request(&app, &public, "POST", "/api/registro", Some(again))
                .await
                .0,
            StatusCode::BAD_REQUEST,
            "duplicate NIT"
        );
        // Recovery: the emailed link works once and signs the account out everywhere.
        ok(
            &app,
            &public,
            "POST",
            "/api/password/reset",
            Some(json!({"email":"NUEVA@universidadean.edu.co"})),
        )
        .await;
        ok(
            &app,
            &public,
            "POST",
            "/api/password/reset",
            Some(json!({"email":"nadie@ninguno.co"})),
        )
        .await;
        let mut link = None;
        for _ in 0..50 {
            link = inbox.lock().unwrap().iter().find_map(
                |(to, _, text): &(String, String, String)| {
                    (to == "nueva@universidadean.edu.co")
                        .then(|| {
                            text.split("#restablecer/")
                                .nth(1)
                                .map(|t| t[..64].to_string())
                        })
                        .flatten()
                },
            );
            if link.is_some() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        let token = link.expect("reset email sent");
        let confirm =
            json!({"token":token,"nueva":"OtraClaveSegura2026","nueva2":"OtraClaveSegura2026"});
        ok(
            &app,
            &public,
            "POST",
            "/api/password/reset/confirm",
            Some(confirm.clone()),
        )
        .await;
        assert_eq!(
            request(
                &app,
                &public,
                "POST",
                "/api/password/reset/confirm",
                Some(confirm)
            )
            .await
            .0,
            StatusCode::BAD_REQUEST,
            "single use"
        );
        assert_eq!(
            request(&app, &registered, "GET", "/api/me", None).await.0,
            StatusCode::UNAUTHORIZED,
            "sessions revoked"
        );
        let (status, _, _) = request(
            &app,
            &public,
            "POST",
            "/api/login",
            Some(json!({"username":"nueva.estudiante","password":"OtraClaveSegura2026"})),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        // HU15 reminders: a favorited open reto closing applications in 3 days.
        sqlx::query("UPDATE retos SET fecha_limite_postulacion=(now() AT TIME ZONE 'America/Bogota')::date+3 WHERE id=4").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO retos_favoritos(usuario_id,reto_id,fecha) VALUES(4,4,now()) ON CONFLICT DO NOTHING").execute(&pool).await.unwrap();
        let created = retosean::reminders::run(&pool, &[7, 3, 1]).await.unwrap();
        assert!(created >= 1, "reminder created");
        let reminders: i64 = sqlx::query_scalar("SELECT count(*) FROM notificaciones WHERE usuario_id=4 AND clave_dedupe='recordatorio:postulacion:4:3'").fetch_one(&pool).await.unwrap();
        assert_eq!(reminders, 1);
        assert_eq!(
            retosean::reminders::run(&pool, &[7, 3, 1]).await.unwrap(),
            0,
            "idempotent, like Django"
        );
        // HU15: the outbox emails pending notifications once, with Django's titles,
        // and skips users who turned email off (the student did, above).
        let outbox = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mailer = retosean::mail::Mailer::Memory(outbox.clone());
        let sent = retosean::mail::deliver_pending(&pool, &mailer, "http://test")
            .await
            .unwrap();
        assert!(sent > 0, "pending notifications are emailed");
        let mails = outbox.lock().unwrap().clone();
        assert!(
            mails
                .iter()
                .all(|(to, subject, _)| !to.starts_with("estudiante.actualizado")
                    && subject.starts_with("[RetosEAN] "))
        );
        assert!(
            mails.iter().any(
                |(_, subject, _)| subject == "[RetosEAN] Respuesta a tu postulacion"
                    || subject.contains("Nueva postulacion")
            ),
            "{mails:?}"
        );
        assert_eq!(
            retosean::mail::deliver_pending(&pool, &mailer, "http://test")
                .await
                .unwrap(),
            0,
            "never twice"
        );
        ok(&app, &student, "POST", "/api/logout", Some(json!({}))).await;
        assert_eq!(
            request(&app, &student, "GET", "/api/me", None).await.0,
            StatusCode::UNAUTHORIZED
        );
    });
    // A spawned task captures assertion failures so cleanup runs before rethrowing.
    let outcome = tokio::spawn(result).await;
    pool.close().await;
    admin
        .execute(format!("DROP SCHEMA {schema} CASCADE").as_str())
        .await
        .unwrap();
    admin.close().await;
    if storage.exists() {
        std::fs::remove_dir_all(&storage).unwrap();
    }
    if let Err(error) = outcome {
        std::panic::resume_unwind(error.into_panic());
    }
}
