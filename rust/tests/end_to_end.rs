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
async fn legal_upload(
    app: &Router,
    session: &Session,
    uri: &str,
    issued: chrono::NaiveDate,
) -> (StatusCode, Value) {
    let multipart = format!(
        "--DOC\r\nContent-Disposition: form-data; name=\"fecha_expedicion\"\r\n\r\n{issued}\r\n--DOC\r\nContent-Disposition: form-data; name=\"archivo\"; filename=\"legal.pdf\"\r\nContent-Type: application/pdf\r\n\r\n%PDF-1.4 documento de prueba\r\n--DOC--\r\n"
    );
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("cookie", &session.cookie)
                .header("x-csrf-token", &session.csrf)
                .header("content-type", "multipart/form-data; boundary=DOC")
                .body(Body::from(multipart))
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
async fn form_upload(
    app: &Router,
    session: &Session,
    uri: &str,
    fields: &[(&str, &str)],
    file: (&str, &[u8]),
) -> (StatusCode, Value) {
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend(
            format!("--FRM\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n")
                .into_bytes(),
        );
    }
    body.extend(format!("--FRM\r\nContent-Disposition: form-data; name=\"{}\"; filename=\"f.pdf\"\r\nContent-Type: application/pdf\r\n\r\n", file.0).into_bytes());
    body.extend(file.1);
    body.extend(b"\r\n--FRM--\r\n");
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("cookie", &session.cookie)
                .header("x-csrf-token", &session.csrf)
                .header("content-type", "multipart/form-data; boundary=FRM")
                .body(Body::from(body))
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
        // Study units: professors manage them too, with an optional PDF and CSV bulk load.
        let program_text = program.to_string();
        let unit_fields = [
            ("codigo", "RS-01"),
            ("nombre", "Sistemas seguros"),
            ("programa", program_text.as_str()),
            ("periodo", "2026-2"),
            ("ciclo", "6"),
        ];
        let (status, unit) = form_upload(
            &app,
            &teacher,
            "/api/unidades",
            &unit_fields,
            ("archivo", b"%PDF-1.4 unidad"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{unit}");
        let unit = unit["id"].as_i64().unwrap();
        let (status, taken) = form_upload(
            &app,
            &teacher,
            "/api/unidades",
            &unit_fields,
            ("archivo", b""),
        )
        .await;
        assert_eq!(
            (status, taken["error"].as_str()),
            (
                StatusCode::BAD_REQUEST,
                Some("Ese código ya está vinculado a otra materia.")
            )
        );
        assert_eq!(
            form_upload(
                &app,
                &student,
                "/api/unidades",
                &unit_fields,
                ("archivo", b"")
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
        let renamed = [
            ("codigo", "RS-01"),
            ("nombre", "Sistemas seguros II"),
            ("programa", program_text.as_str()),
            ("periodo", "2026-2"),
            ("ciclo", "7"),
        ];
        assert_eq!(
            form_upload(
                &app,
                &teacher,
                &format!("/api/unidades/{unit}"),
                &renamed,
                ("archivo", b"")
            )
            .await
            .0,
            StatusCode::OK
        );
        let off = ok(
            &app,
            &teacher,
            "POST",
            &format!("/api/unidades/{unit}/activo"),
            Some(json!({"activo":false})),
        )
        .await;
        assert_eq!(
            off["mensaje"],
            "Unidad de estudio 'RS-01' desactivada correctamente."
        );
        // Inactive units only show with activo=0, as Django's list.
        let listed = ok(
            &app,
            &teacher,
            "GET",
            "/api/records/unidades_estudio?q=RS-01",
            None,
        )
        .await;
        assert_eq!(listed["total"], 0);
        let listed = ok(
            &app,
            &teacher,
            "GET",
            "/api/records/unidades_estudio?q=RS-01&activo=0",
            None,
        )
        .await;
        assert_eq!(listed["total"], 1);
        ok(
            &app,
            &teacher,
            "POST",
            &format!("/api/unidades/{unit}/activo"),
            Some(json!({"activo":true})),
        )
        .await;
        let (name, file): (String, Option<String>) =
            sqlx::query_as("SELECT nombre,archivo FROM unidades_estudio WHERE id=$1")
                .bind(unit)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(name, "Sistemas seguros II");
        assert!(
            file.is_some_and(|f| f.ends_with(".pdf")),
            "the PDF survives an edit without a new file"
        );
        let csv = "codigo;nombre;programa;periodo\nRS-02;Datos;ingeniería de sistemas;2026-2\nRS-01;Repetida;Ingeniería de Sistemas;\nRS-03;Sin programa;No existe;\n"
        .as_bytes();
        let (status, loaded) = form_upload(
            &app,
            &teacher,
            "/api/unidades",
            &[("accion", "cargar_csv")],
            ("archivo_csv", csv),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{loaded}");
        assert_eq!(
            (loaded["creadas"].as_i64(), loaded["omitidas"].as_i64()),
            (Some(1), Some(2))
        );
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
        ok(&app,&student,"POST","/api/perfil",Some(json!({"first_name":"Estudiante","last_name":"Actualizado","email":"estudiante.actualizado@universidadean.edu.co","semestre":7}))).await;
        // EditarPerfilForm: institutional email for students and teachers; role fields saved.
        assert_eq!(
            request(
                &app,
                &student,
                "POST",
                "/api/perfil",
                Some(json!({"first_name":"E","last_name":"A","email":"otro@example.test"}))
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            ok(&app, &student, "GET", "/api/perfil", None).await["semestre"],
            7
        );
        ok(&app,&company,"POST","/api/perfil",Some(json!({"first_name":"Empresa","last_name":"Demo","email":"empresa@example.test","razon_social":"","area_responsable":"Innovación","sitio_web":"https://demo.test"}))).await;
        let (legal, area): (String, String) =
            sqlx::query_as("SELECT razon_social,area_responsable FROM empresas WHERE id=1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            (legal.as_str(), area.as_str()),
            ("Empresa Demo S.A.S.", "Innovación")
        );
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
        // HU13: only holders of empresas.cargar_listas_informa upload special documents.
        let catalog = ok(
            &app,
            &administrator,
            "GET",
            "/api/usuarios/3/permisos",
            None,
        )
        .await;
        assert_eq!(catalog["permisos"][0]["otorgado"], false);
        assert_eq!(
            legal_upload(
                &app,
                &teacher,
                "/api/empresas/1/documentos/LISTAS_INFORMA/cargar",
                issued
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
        let granted = ok(
            &app,
            &administrator,
            "POST",
            "/api/usuarios/3/permisos",
            Some(json!({"codigos":["empresas.cargar_listas_informa","no.existe"]})),
        )
        .await;
        assert_eq!(
            granted["otorgados"],
            json!(["empresas.cargar_listas_informa"])
        );
        for kind in &required {
            let (status, body) = if kind == "LISTAS_INFORMA" {
                legal_upload(
                    &app,
                    &teacher,
                    &format!("/api/empresas/1/documentos/{kind}/cargar"),
                    issued,
                )
                .await
            } else {
                let document:i64=sqlx::query_scalar("INSERT INTO documentos_empresas(empresa_id,tipo_documento,estado,fecha_carga) VALUES(1,$1,'PENDIENTE',now()) RETURNING id").bind(kind).fetch_one(&pool).await.unwrap();
                // CargarDocumentoForm: the issue date is never in the future.
                assert_eq!(
                    legal_upload(
                        &app,
                        &company,
                        &format!("/api/documentos/{document}/cargar"),
                        issued + chrono::Duration::days(1)
                    )
                    .await
                    .0,
                    StatusCode::BAD_REQUEST
                );
                legal_upload(
                    &app,
                    &company,
                    &format!("/api/documentos/{document}/cargar"),
                    issued,
                )
                .await
            };
            assert_eq!(status, StatusCode::OK, "{kind}: {body}");
            let document = body["id"].as_i64().unwrap();
            if kind == "LISTAS_INFORMA" {
                // The company itself may not reload it.
                assert_eq!(
                    legal_upload(
                        &app,
                        &company,
                        &format!("/api/documentos/{document}/cargar"),
                        issued
                    )
                    .await
                    .0,
                    StatusCode::FORBIDDEN
                );
            }
            ok(
                &app,
                &administrator,
                "POST",
                &format!("/api/documentos/{document}/revisar"),
                Some(json!({"estado":"VERIFICADO"})),
            )
            .await;
        }
        let revoked = ok(
            &app,
            &administrator,
            "POST",
            "/api/usuarios/3/permisos",
            Some(json!({"codigos":[]})),
        )
        .await;
        assert_eq!(
            revoked["retirados"],
            json!(["empresas.cargar_listas_informa"])
        );
        let audit: Vec<String> = sqlx::query_scalar("SELECT detalle FROM log_actividad WHERE accion='CAMBIO_PERMISOS' AND usuario_id=3 ORDER BY id").fetch_all(&pool).await.unwrap();
        assert_eq!(
            audit,
            [
                "Permisos especiales otorgados: empresas.cargar_listas_informa.",
                "Permisos especiales retirados: empresas.cargar_listas_informa."
            ]
        );
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
        // HU13 role change: audited, never on oneself.
        assert_eq!(
            request(
                &app,
                &administrator,
                "POST",
                "/api/usuarios/1/rol",
                Some(json!({"rol":"EXPERTO"}))
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
        let changed = ok(
            &app,
            &administrator,
            "POST",
            &format!("/api/usuarios/{new_user}/rol"),
            Some(json!({"rol":"EXPERTO"})),
        )
        .await;
        assert_eq!(changed["cambiado"], true);
        let (detail, field): (String, String) = sqlx::query_as("SELECT l.detalle,h.campo FROM log_actividad l JOIN historial_estados h ON h.object_id=l.usuario_id AND h.campo='rol' WHERE l.accion='CAMBIO_ROL' AND l.usuario_id=$1").bind(new_user).fetch_one(&pool).await.unwrap();
        assert_eq!(
            (detail.as_str(), field.as_str()),
            ("Rol cambiado de ESTUDIANTE a EXPERTO.", "rol")
        );
        // HU00 participation block: the company cannot publish retos while blocked.
        assert_eq!(
            request(
                &app,
                &administrator,
                "POST",
                "/api/empresas/1/bloqueo",
                Some(json!({"bloquear":true}))
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
        ok(
            &app,
            &administrator,
            "POST",
            "/api/empresas/1/bloqueo",
            Some(json!({"bloquear":true,"motivo":"Revisión de pagos"})),
        )
        .await;
        let (status, blocked, _) = request(
            &app,
            &company,
            "POST",
            "/api/retos",
            Some(json!({"titulo":"Bloqueado","tipo":"reto","descripcion":"x","area":"x"})),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            blocked["error"]
                .as_str()
                .unwrap()
                .contains("Participación bloqueada por el administrador. Revisión de pagos"),
            "{blocked}"
        );
        ok(
            &app,
            &administrator,
            "POST",
            "/api/empresas/1/bloqueo",
            Some(json!({"bloquear":false})),
        )
        .await;
        let notices: Vec<String> = sqlx::query_scalar("SELECT tipo FROM notificaciones WHERE usuario_id=2 AND clave_dedupe LIKE 'empresa:1:bloqueo:%' ORDER BY id").fetch_all(&pool).await.unwrap();
        assert_eq!(notices, ["ERROR", "EXITO"]);
        let choice = ok(
            &app,
            &company,
            "POST",
            "/api/empresa/modo-convenio",
            Some(json!({"opcion":"si_convenio"})),
        )
        .await;
        assert_eq!(choice["siguiente"], "documentos");
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
                {"rol":"ESTUDIANTE","nombre":"Estudiante","documento":"20","email":"estudiante.actualizado@universidadean.edu.co","programa":"Sistemas"},
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
            Some(json!({"motivacion":"Quiero aprender Rust","semestre":6,"acepta_terminos":true})),
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
                Some(json!({"motivacion":"Intento de reapertura","acepta_terminos":true}))
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
            Some(json!({"nombre_equipo":"Equipo Rust","profesores":[3],"estudiantes":[4]})),
        )
        .await["id"]
            .as_i64()
            .unwrap();
        // crear_equipo_academico: professors only (not admins); students are notified.
        assert_eq!(
            request(
                &app,
                &administrator,
                "POST",
                &format!("/api/retos/{id}/equipo"),
                Some(json!({"nombre_equipo":"x"}))
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
        let assigned: String = sqlx::query_scalar("SELECT mensaje FROM notificaciones WHERE usuario_id=4 AND evento='EQUIPO_ASIGNADO' ORDER BY id DESC LIMIT 1").fetch_one(&pool).await.unwrap();
        assert!(
            assigned.starts_with("Fuiste incorporado al equipo \"Equipo Rust\""),
            "{assigned}"
        );
        // Academic team edit/delete keep the operative mirror in step (Django sincronizar).
        let academic: i64 =
            sqlx::query_scalar("SELECT id FROM equipos_reto_academico WHERE equipo_espejo_id=$1")
                .bind(team)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            request(
                &app,
                &student,
                "POST",
                &format!("/api/equipos-academicos/{academic}"),
                Some(json!({"nombre_equipo":"x","estudiantes":[4]}))
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            request(
                &app,
                &teacher,
                "POST",
                &format!("/api/equipos-academicos/{academic}"),
                Some(json!({"nombre_equipo":"x","estudiantes":[3]}))
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
        ok(&app,&teacher,"POST",&format!("/api/equipos-academicos/{academic}"),Some(json!({"nombre_equipo":"Equipo Rust editado","profesores":[3],"estudiantes":[4],"expertos_invitados":[5]}))).await;
        let (mirror_name, leader): (String, String) = sqlx::query_as("SELECT e.nombre,m.rol FROM equipos e JOIN miembros_equipo m ON m.equipo_id=e.id WHERE e.id=$1 AND m.estudiante_id=4").bind(team).fetch_one(&pool).await.unwrap();
        assert_eq!(
            (mirror_name.as_str(), leader.as_str()),
            ("Equipo Rust editado", "LIDER")
        );
        let spare = ok(
            &app,
            &teacher,
            "POST",
            &format!("/api/retos/{id}/equipo"),
            Some(json!({"nombre_equipo":"Equipo descartable","estudiantes":[4]})),
        )
        .await;
        let deleted = ok(
            &app,
            &teacher,
            "POST",
            &format!(
                "/api/equipos-academicos/{}/eliminar",
                spare["equipo_academico"]
            ),
            None,
        )
        .await;
        assert_eq!(
            deleted["mensaje"],
            "Equipo académico \"Equipo descartable\" eliminado."
        );
        let left: i64 = sqlx::query_scalar("SELECT count(*) FROM equipos WHERE id=$1")
            .bind(spare["id"].as_i64().unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(left, 0);
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
        assert_eq!(ranking["ranking"][0]["equipo"], "Equipo Rust editado");
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
        // HU08 closure management: agenda and acta, final artifacts, recognitions, logistics.
        assert_eq!(
            request(
                &app,
                &student,
                "GET",
                &format!("/api/retos/{id}/cierre"),
                None
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
        let (status, saved) = form_upload(
            &app,
            &company,
            &format!("/api/retos/{id}/cierre"),
            &[
                ("fecha_hora", "2099-12-01T09:30"),
                ("espacio", "Auditorio principal"),
                ("agenda", "Presentación de resultados"),
                ("notas", "Acta firmada"),
                ("impacto", "SI"),
            ],
            ("acta_url", b"%PDF-1.4 acta"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{saved}");
        let (status, _) = form_upload(
            &app,
            &company,
            &format!("/api/retos/{id}/cierre/entregables"),
            &[("nombre", "Informe final")],
            ("archivo", b"%PDF-1.4 informe"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            request(
                &app,
                &teacher,
                "POST",
                &format!("/api/retos/{id}/cierre/reconocimientos"),
                Some(json!({"usuario_id":3,"nombre_destinatario":"Profesor","descripcion":"x"}))
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
        ok(&app,&teacher,"POST",&format!("/api/retos/{id}/cierre/reconocimientos"),Some(json!({"usuario_id":4,"nombre_destinatario":"Estudiante Demo","descripcion":"Mejor prototipo"}))).await;
        let logistics = ok(&app,&teacher,"POST",&format!("/api/retos/{id}/solicitudes-logistica"),Some(json!({"evento":"CIERRE","tipo":"ESPACIO","descripcion":"Auditorio para 80 personas"}))).await["id"].as_i64().unwrap();
        let notice: String = sqlx::query_scalar(
            "SELECT mensaje FROM notificaciones WHERE usuario_id=1 AND clave_dedupe=$1",
        )
        .bind(format!("logistica:{logistics}:solicitada"))
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(
            notice.starts_with("Espacio físico para el evento de cierre del reto"),
            "{notice}"
        );
        assert_eq!(
            request(
                &app,
                &teacher,
                "POST",
                &format!("/api/solicitudes-logistica/{logistics}/estado"),
                Some(json!({"estado":"CONFIRMADA"}))
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
        ok(
            &app,
            &administrator,
            "POST",
            &format!("/api/solicitudes-logistica/{logistics}/estado"),
            Some(json!({"estado":"CONFIRMADA"})),
        )
        .await;
        assert_eq!(
            request(
                &app,
                &administrator,
                "POST",
                &format!("/api/solicitudes-logistica/{logistics}/estado"),
                Some(json!({"estado":"DESCARTADA"}))
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
        let panel = ok(
            &app,
            &company,
            "GET",
            &format!("/api/retos/{id}/cierre"),
            None,
        )
        .await;
        assert_eq!(panel["agenda"]["fecha_hora"], "2099-12-01T09:30");
        assert_eq!(panel["cierre"]["impacto"], "SI");
        assert_eq!(panel["cierre"]["acta"], true);
        assert_eq!(panel["entregables_finales"][0]["nombre"], "Informe final");
        assert_eq!(
            panel["reconocimientos"][0]["descripcion"],
            "Mejor prototipo"
        );
        assert_eq!(panel["solicitudes_logistica"][0]["estado"], "CONFIRMADA");
        assert_eq!(panel["puede_finalizar"], false);
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
        // Django's close sends the survey notice with its own title, then the certificate notice.
        let titles: Vec<String> = sqlx::query_scalar("SELECT titulo FROM notificaciones WHERE usuario_id=4 AND evento IN ('ENCUESTA_PENDIENTE','CERTIFICADO_DISPONIBLE') AND clave_dedupe IN ($1,$2) ORDER BY id")
            .bind(format!("cierre:{id}:encuesta")).bind(format!("certificado:{id}")).fetch_all(&pool).await.unwrap();
        assert_eq!(titles.len(), 2);
        assert!(titles[0].starts_with("Reto finalizado: "), "{titles:?}");
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
            choices["estudiantes"]
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
        // HU13 bulk import: preview validates without writing; confirm creates valid rows.
        async fn upload(
            app: &Router,
            session: &Session,
            name: &str,
            bytes: Vec<u8>,
            action: &str,
        ) -> Value {
            let mut body = format!("--IMP\r\nContent-Disposition: form-data; name=\"accion\"\r\n\r\n{action}\r\n--IMP\r\nContent-Disposition: form-data; name=\"archivo\"; filename=\"{name}\"\r\nContent-Type: application/octet-stream\r\n\r\n").into_bytes();
            body.extend(bytes);
            body.extend(b"\r\n--IMP--\r\n");
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/usuarios/importar")
                        .header("cookie", &session.cookie)
                        .header("x-csrf-token", &session.csrf)
                        .header("content-type", "multipart/form-data; boundary=IMP")
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();
            let status = response.status();
            let value: Value =
                serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                    .unwrap();
            assert_eq!(status, StatusCode::OK, "{value}");
            value
        }
        let csv = "username;email;first_name;last_name;rol;telefono;password
imp.uno;uno@ean.edu.co;Uno;Import;estudiante;300;ClaveImportada2026
imp.dos;dos@ean.edu.co;Dos;Import;PROFESOR;;
imp.mal;nueva@universidadean.edu.co;Mal;Import;ROLRARO;;corta
;;;;;;
";
        let preview = upload(
            &app,
            &administrator,
            "usuarios.csv",
            csv.as_bytes().to_vec(),
            "previsualizar",
        )
        .await;
        assert_eq!(preview["validas"].as_array().unwrap().len(), 2, "{preview}");
        let errors = preview["invalidas"][0]["errores"].to_string();
        assert!(
            errors.contains("Ya existe un usuario con ese correo")
                && errors.contains("Rol no valido")
                && errors.contains("Contrasena debil"),
            "{errors}"
        );
        let none: i64 =
            sqlx::query_scalar("SELECT count(*) FROM usuarios WHERE username LIKE 'imp.%'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(none, 0, "preview writes nothing");
        let done = upload(
            &app,
            &administrator,
            "usuarios.csv",
            csv.as_bytes().to_vec(),
            "confirmar",
        )
        .await;
        assert_eq!(
            (done["creados"].as_i64(), done["omitidas"].as_i64()),
            (Some(2), Some(1))
        );
        assert_eq!(
            request(
                &app,
                &public,
                "POST",
                "/api/login",
                Some(json!({"username":"imp.uno","password":"ClaveImportada2026"}))
            )
            .await
            .0,
            StatusCode::OK
        );
        assert_eq!(
            request(
                &app,
                &public,
                "POST",
                "/api/login",
                Some(json!({"username":"imp.dos","password":"!"}))
            )
            .await
            .0,
            StatusCode::UNAUTHORIZED,
            "no usable password"
        );
        let logged: i64 =
            sqlx::query_scalar("SELECT count(*) FROM log_actividad WHERE accion='IMPORTACION'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(logged, 1);
        // Excel works too, and a missing required column is reported for the whole file.
        let mut book = rust_xlsxwriter::Workbook::new();
        let sheet = book.add_worksheet();
        for (c, h) in ["username", "email", "rol"].iter().enumerate() {
            sheet.write_string(0, c as u16, *h).unwrap();
        }
        for (c, v) in ["imp.tres", "tres@ean.edu.co", "EXPERTO"]
            .iter()
            .enumerate()
        {
            sheet.write_string(1, c as u16, *v).unwrap();
        }
        let xlsx = upload(
            &app,
            &administrator,
            "usuarios.xlsx",
            book.save_to_buffer().unwrap(),
            "previsualizar",
        )
        .await;
        assert_eq!(
            xlsx["validas"][0]["datos"]["username"], "imp.tres",
            "{xlsx}"
        );
        let missing = upload(
            &app,
            &administrator,
            "x.csv",
            b"username,rol
z,ESTUDIANTE
"
            .to_vec(),
            "previsualizar",
        )
        .await;
        assert!(missing["error_global"].as_str().unwrap().contains("email"));
        assert_eq!(
            request(&app, &teacher, "GET", "/api/usuarios/plantilla", None)
                .await
                .0,
            StatusCode::FORBIDDEN
        );
        // HU06 report builder: generate (logged), history download, send by email, roles.
        let (status, _, _) = request(&app, &administrator, "POST", "/api/reportes/generar", Some(json!({"reporte":"retos","formato":"EXCEL","periodo":"todo","columnas":["titulo","estado"]}))).await;
        assert_eq!(status, StatusCode::OK);
        let report_id: i64 = sqlx::query_scalar("SELECT id FROM reportes_generados WHERE filtros->>'reporte'='retos' ORDER BY id DESC LIMIT 1").fetch_one(&pool).await.unwrap();
        assert_eq!(
            request(
                &app,
                &administrator,
                "GET",
                &format!("/api/archivo/reportes_generados/{report_id}/archivo"),
                None
            )
            .await
            .0,
            StatusCode::OK
        );
        let preview = ok(
            &app,
            &administrator,
            "POST",
            "/api/reportes/vista-previa",
            Some(json!({"reporte":"empresas","periodo":"todo"})),
        )
        .await;
        assert_eq!(preview["encabezados"][0], "NIT");
        assert!(preview["total"].as_i64().unwrap() >= 1);
        assert_eq!(
            request(
                &app,
                &administrator,
                "POST",
                "/api/reportes/enviar",
                Some(json!({"reporte":"retos","periodo":"todo","destinatarios":"no-es-correo"}))
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
        let sent = ok(&app, &administrator, "POST", "/api/reportes/enviar", Some(json!({"reporte":"presupuesto","formato":"PDF","periodo":"todo","destinatarios":"jefe@ean.edu.co; otra@ean.edu.co"}))).await;
        assert_eq!(sent["enviado"], true);
        let flagged: bool =
            sqlx::query_scalar("SELECT enviado_por_email FROM reportes_generados WHERE id=$1")
                .bind(sent["id"].as_i64().unwrap())
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(flagged);
        assert_eq!(
            inbox
                .lock()
                .unwrap()
                .iter()
                .filter(|(_, subject, _)| subject == "[RetosEAN] Ejecucion presupuestal")
                .count(),
            2
        );
        assert_eq!(
            request(
                &app,
                &teacher,
                "POST",
                "/api/reportes/vista-previa",
                Some(json!({"reporte":"retos","periodo":"todo"}))
            )
            .await
            .0,
            StatusCode::FORBIDDEN,
            "retos report is admin-only"
        );
        assert_eq!(
            request(
                &app,
                &teacher,
                "POST",
                "/api/reportes/vista-previa",
                Some(json!({"reporte":"participacion","periodo":"todo"}))
            )
            .await
            .0,
            StatusCode::OK
        );
        assert_eq!(
            request(
                &app,
                &teacher,
                "GET",
                &format!("/api/archivo/reportes_generados/{report_id}/archivo"),
                None
            )
            .await
            .0,
            StatusCode::NOT_FOUND,
            "not their report"
        );
        assert_eq!(
            request(&app, &student, "GET", "/api/reportes/catalogo", None)
                .await
                .0,
            StatusCode::FORBIDDEN
        );
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
