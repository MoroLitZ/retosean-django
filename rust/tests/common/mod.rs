//! Shared harness for the Django-parity suites (tests/parity_*.rs).
//! Each test gets its own PostgreSQL schema with the demo data and logged-in
//! sessions for the five demo roles. Run: cargo test --test parity_<app> -- --ignored
#![allow(dead_code)]
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use retosean::{AppState, config::Config};
use serde_json::{Value, json};
use sqlx::{Executor, PgPool, postgres::PgPoolOptions};
use tower::ServiceExt;

/// Hash of RetosEAN2026 with 1,000 PBKDF2 rounds: fast logins in tests.
pub const HASH: &str = "pbkdf2_sha256$1000$test-salt$CqJFhYAc+mBaD+E6+znRWXbr3hP/JgZjkUaazakv+Ak=";
/// Parallel tests share PostgreSQL's connection limit.
static SLOTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(6);

#[derive(Clone, Default)]
pub struct Session {
    pub cookie: String,
    pub csrf: String,
}
pub struct Ctx {
    pub app: Router,
    pub pool: PgPool,
    pub media: std::path::PathBuf,
    pub admin: Session,
    pub company: Session,
    pub teacher: Session,
    pub student: Session,
    pub expert: Session,
    pub anon: Session,
}
/// Demo ids (sql/demo.sql).
pub const ADMIN: i64 = 1;
pub const COMPANY_USER: i64 = 2;
pub const TEACHER: i64 = 3;
pub const STUDENT: i64 = 4;
pub const EXPERT: i64 = 5;
pub const COMPANY: i64 = 1;
pub const PROGRAM: i64 = 1;
pub const FACULTY: i64 = 1;

pub async fn request(
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
    let body = match data {
        Some(data) => {
            builder = builder.header("content-type", "application/json");
            Body::from(data.to_string())
        }
        None => Body::empty(),
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
        .unwrap_or_else(|_| json!({"raw": String::from_utf8_lossy(&bytes), "bytes": bytes.len()}));
    (status, value, cookie)
}
/// Raw response bytes (files, PDFs, Excel).
pub async fn download(app: &Router, session: &Session, path: &str) -> (StatusCode, Vec<u8>) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(path)
                .header("cookie", &session.cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    (
        status,
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
}
pub async fn post_bytes(
    app: &Router,
    session: &Session,
    path: &str,
    data: Value,
) -> (StatusCode, Vec<u8>) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("cookie", &session.cookie)
                .header("x-csrf-token", &session.csrf)
                .header("content-type", "application/json")
                .body(Body::from(data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    (
        status,
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
}
pub async fn ok(
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
pub async fn status(
    app: &Router,
    session: &Session,
    method: &str,
    path: &str,
    data: Option<Value>,
) -> StatusCode {
    request(app, session, method, path, data).await.0
}
/// Multipart POST: text fields plus optional files (field, filename, bytes).
pub async fn multipart(
    app: &Router,
    session: &Session,
    path: &str,
    fields: &[(&str, &str)],
    files: &[(&str, &str, &[u8])],
) -> (StatusCode, Value) {
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend(
            format!("--FRM\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n")
                .into_bytes(),
        );
    }
    for (name, filename, bytes) in files {
        body.extend(format!("--FRM\r\nContent-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\nContent-Type: application/octet-stream\r\n\r\n").into_bytes());
        body.extend(*bytes);
        body.extend(b"\r\n");
    }
    body.extend(b"--FRM--\r\n");
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
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
pub async fn login_with(app: &Router, name: &str, password: &str) -> (StatusCode, Session) {
    let (status, body, cookie) = request(
        app,
        &Session::default(),
        "POST",
        "/api/login",
        Some(json!({"username": name, "password": password})),
    )
    .await;
    let session = Session {
        cookie: cookie.unwrap_or_default(),
        csrf: body["csrf"].as_str().unwrap_or_default().into(),
    };
    (status, session)
}
pub async fn login(app: &Router, name: &str) -> Session {
    let (status, session) = login_with(app, name, "RetosEAN2026").await;
    assert_eq!(status, StatusCode::OK, "login {name}");
    session
}

impl Ctx {
    pub async fn ok(&self, s: &Session, method: &str, path: &str, data: Option<Value>) -> Value {
        ok(&self.app, s, method, path, data).await
    }
    pub async fn status(
        &self,
        s: &Session,
        method: &str,
        path: &str,
        data: Option<Value>,
    ) -> StatusCode {
        status(&self.app, s, method, path, data).await
    }
    pub async fn req(
        &self,
        s: &Session,
        method: &str,
        path: &str,
        data: Option<Value>,
    ) -> (StatusCode, Value) {
        let (status, body, _) = request(&self.app, s, method, path, data).await;
        (status, body)
    }
    pub async fn login(&self, name: &str) -> Session {
        login(&self.app, name).await
    }
    /// Executes SQL with optional i64 binds and returns nothing.
    pub async fn exec(&self, sql: &str) {
        sqlx::raw_sql(sql).execute(&self.pool).await.unwrap();
    }
    pub async fn i64(&self, sql: &str) -> i64 {
        sqlx::query_scalar::<_, i64>(sql)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }
    pub async fn text(&self, sql: &str) -> String {
        sqlx::query_scalar::<_, String>(sql)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }
    pub async fn texts(&self, sql: &str) -> Vec<String> {
        sqlx::query_scalar::<_, String>(sql)
            .fetch_all(&self.pool)
            .await
            .unwrap()
    }
    pub async fn bool(&self, sql: &str) -> bool {
        sqlx::query_scalar::<_, bool>(sql)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }
    /// A user with the demo password; returns its id.
    pub async fn user(&self, username: &str, rol: &str) -> i64 {
        let email = format!("{username}@universidadean.edu.co");
        let id: i64 = sqlx::query_scalar("INSERT INTO usuarios(password,is_superuser,username,first_name,last_name,email,is_staff,is_active,date_joined,rol,telefono,activo) VALUES($1,false,$2,$2,'Prueba',$3,false,true,now(),$4,'',true) RETURNING id")
            .bind(HASH).bind(username).bind(email).bind(rol).fetch_one(&self.pool).await.unwrap();
        match rol {
            "ESTUDIANTE" => self.exec(&format!("INSERT INTO estudiantes(usuario_id,programa_id,semestre) VALUES({id},{PROGRAM},5)")).await,
            "PROFESOR" => self.exec(&format!("INSERT INTO profesores(usuario_id,facultad_id,especialidad) VALUES({id},{FACULTY},'')")).await,
            _ => {}
        }
        id
    }
    /// A company user with its Empresa row; returns (user id, empresa id).
    pub async fn company_user(&self, username: &str, nit: &str) -> (i64, i64) {
        let user = self.user(username, "EMPRESA").await;
        let company: i64 = sqlx::query_scalar("INSERT INTO empresas(nit,razon_social,sector_industrial,usuario_id,estado_validacion,estado_listas_restrictivas,estado_convenio) VALUES($1,$2,'Tecnología',$3,'PENDIENTE','PENDIENTE','NO_RADICADO') RETURNING id")
            .bind(nit).bind(format!("{username} S.A.S.")).bind(user).fetch_one(&self.pool).await.unwrap();
        (user, company)
    }
    /// Every required catalog document verified and in force (the HU00 gate opens).
    pub async fn verify_documents(&self, company: i64) {
        self.exec(&format!("INSERT INTO documentos_empresas(empresa_id,tipo_documento,archivo,estado,fecha_expedicion,fecha_vencimiento,fecha_carga,motivo_rechazo) SELECT {company},codigo,'','VERIFICADO',CURRENT_DATE,NULL,now(),'' FROM tipos_documento_legal WHERE activo AND obligatorio ON CONFLICT (empresa_id,tipo_documento) DO UPDATE SET estado='VERIFICADO',fecha_vencimiento=NULL")).await;
        self.exec(&format!("UPDATE documentos_empresas SET estado='VERIFICADO' WHERE empresa_id={company}; UPDATE empresas SET estado_validacion='VERIFICADA',estado_listas_restrictivas='APROBADO' WHERE id={company}")).await;
    }
    /// A reto in the given state owned by the company user; returns its id.
    pub async fn reto(&self, owner: i64, estado: &str, tipo: &str) -> i64 {
        sqlx::query_scalar("INSERT INTO retos(empresa_id,tipo,titulo,descripcion,area,nivel_academico,facultad_id,programa_id,estado,premios,criterios_evaluacion,comentarios_revision,creado_en,actualizado_en,tiene_convenio_institucional,fecha_limite_postulacion) VALUES($1,$2,'Reto de prueba','Descripción del reto de prueba','Tecnología','Pregrado',1,1,$3,'','','',now(),now(),false,CURRENT_DATE+30) RETURNING id")
            .bind(owner).bind(tipo).bind(estado).fetch_one(&self.pool).await.unwrap()
    }
    /// An accepted application of the student to the reto.
    pub async fn accept(&self, reto: i64, student: i64) -> i64 {
        sqlx::query_scalar("INSERT INTO postulaciones(reto_id,estudiante_id,estado,motivacion,semestre,programa,fecha_postulacion,habilidades,acepta_terminos) VALUES($1,$2,'ACEPTADA','Motivación',5,'Ingeniería de Sistemas',now(),'',true) RETURNING id")
            .bind(reto).bind(student).fetch_one(&self.pool).await.unwrap()
    }
    /// A published academic integration of the teacher in the reto.
    pub async fn integrate(&self, reto: i64, teacher: i64) -> i64 {
        sqlx::query_scalar("INSERT INTO integraciones_academicas(reto_id,profesor_id,estado,facultad,programa_academico,nivel_formacion,descripcion,alcance,entregable_esperado,cronograma_sesiones) VALUES($1,$2,'publicada','Ingeniería','Ingeniería de Sistemas','Pregrado','Proyecto','Prototipo','Informe','Semanal') RETURNING id")
            .bind(reto).bind(teacher).fetch_one(&self.pool).await.unwrap()
    }
    /// historial_de(obj, campo): (from, to, username) in order.
    pub async fn history(
        &self,
        app: &str,
        model: &str,
        id: i64,
        campo: &str,
    ) -> Vec<(String, String, String)> {
        sqlx::query_as("SELECT h.estado_anterior,h.estado_nuevo,COALESCE(u.username,'') FROM historial_estados h JOIN django_content_type c ON c.id=h.content_type_id LEFT JOIN usuarios u ON u.id=h.realizado_por_id WHERE c.app_label=$1 AND c.model=$2 AND h.object_id=$3 AND h.campo=$4 ORDER BY h.fecha,h.id")
            .bind(app).bind(model).bind(id).bind(campo).fetch_all(&self.pool).await.unwrap()
    }
    /// A submitted deliverable (ENVIADO) with a stored PDF.
    pub async fn deliverable(&self, reto: i64, student: i64) -> i64 {
        let key = format!("{}.pdf", uuid::Uuid::new_v4().simple());
        std::fs::create_dir_all(&self.media).unwrap();
        std::fs::write(self.media.join(&key), b"%PDF-1.4 entregable").unwrap();
        sqlx::query_scalar("INSERT INTO entregables(reto_id,estudiante_id,titulo,es_final,puntaje_maximo,archivo,comentario_estudiante,comentario_profesor,calificacion_publicada,fecha_entrega,actualizado_en,estado) VALUES($1,$2,'Avance',false,5,$3,'','',false,now(),now(),'ENVIADO') RETURNING id")
            .bind(reto).bind(student).bind(key).fetch_one(&self.pool).await.unwrap()
    }
    pub async fn notices(&self, user: i64, event: &str) -> i64 {
        self.i64(&format!(
            "SELECT count(*) FROM notificaciones WHERE usuario_id={user} AND evento='{event}'"
        ))
        .await
    }
}

/// Runs a test body against a fresh schema and always cleans it up.
pub async fn run<F, Fut>(body: F)
where
    F: FnOnce(Ctx) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let _slot = SLOTS.acquire().await.unwrap();
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
        .max_connections(4)
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
    sqlx::raw_sql(include_str!("../../sql/schema.sql"))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(&include_str!("../../sql/demo.sql").replace("__DEMO_PASSWORD_HASH__", HASH))
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
    // A spawned task captures assertion failures so cleanup runs before rethrowing.
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

/// One Django test → one Rust test with the same name.
#[macro_export]
macro_rules! parity {
    ($name:ident, |$t:ident| $body:block) => {
        #[tokio::test]
        #[ignore = "requires local PostgreSQL; each test uses its own schema"]
        async fn $name() {
            common::run(|$t| async move $body).await;
        }
    };
}
