pub mod academic;
pub mod accounts;
pub mod auth;
pub mod builder;
pub mod catalogs;
pub mod closure;
pub mod companies;
pub mod config;
pub mod dashboard;
pub mod domain;
pub mod error;
pub mod event;
pub mod finance;
pub mod grading;
pub mod hackathon;
pub mod import;
pub mod lifecycle;
pub mod mail;
pub mod media;
pub mod migrate;
pub mod models;
pub mod permissions;
pub mod registration;
pub mod reminders;
pub mod reports;
pub mod resources;
pub mod schema_migrations;
pub mod study_units;
pub mod team;
pub mod workflows;

use axum::{
    Json, Router,
    extract::Request,
    http::{HeaderMap, HeaderValue},
    middleware::{self, Next},
    response::{Html, Response},
    routing::{get, post},
};
use serde_json::json;
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: config::Config,
    /// Hashing slots for registration/reset/profile; login has its own pool so a
    /// login flood cannot block account creation.
    pub password_slots: std::sync::Arc<tokio::sync::Semaphore>,
    pub login_slots: std::sync::Arc<tokio::sync::Semaphore>,
    /// Memory-capped concurrency for heavy exports/imports.
    pub heavy_slots: std::sync::Arc<tokio::sync::Semaphore>,
}

/// One of the few slots for memory-heavy work (exports, imports, report
/// generation). Callers take it after authorisation; waiting is bounded so a
/// burst cannot queue requests forever.
pub async fn heavy_permit(state: &AppState) -> error::Result<tokio::sync::OwnedSemaphorePermit> {
    let busy = || {
        error::AppError(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "El servidor está ocupado generando otros archivos; intenta de nuevo en un momento."
                .into(),
        )
    };
    tokio::time::timeout(
        std::time::Duration::from_secs(30),
        state.heavy_slots.clone().acquire_owned(),
    )
    .await
    .map_err(|_| busy())?
    .map_err(|_| busy())
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route(
            "/",
            get(|| async { Html(include_str!("../web/index.html")) }),
        )
        .route("/assets/{file}", get(asset))
        .route("/health", get(health))
        .route("/api/login", post(auth::login))
        .route("/api/logout", post(auth::logout))
        .route("/api/registro", post(registration::register))
        .route("/api/registro/opciones", get(registration::options))
        .route("/api/registro/verificar", post(registration::verify))
        .route("/api/password/reset", post(registration::reset_request))
        .route(
            "/api/password/reset/confirm",
            post(registration::reset_confirm),
        )
        .route("/api/me", get(auth::me))
        .route("/api/usuarios", post(accounts::create))
        .route(
            "/api/usuarios/importar",
            post(import::import).layer(axum::extract::DefaultBodyLimit::max(6 * 1024 * 1024)),
        )
        .route("/api/usuarios/plantilla", get(import::template))
        .route("/api/log-actividad", get(accounts::activity_log))
        .route(
            "/api/perfil",
            get(accounts::show_profile).post(accounts::profile),
        )
        .route("/api/password", post(accounts::password))
        .route("/api/catalogos/{resource}", post(catalogs::create))
        .route(
            "/api/unidades",
            post(study_units::create).layer(axum::extract::DefaultBodyLimit::max(10 * 1024 * 1024)),
        )
        .route(
            "/api/unidades/{id}",
            post(study_units::edit).layer(axum::extract::DefaultBodyLimit::max(10 * 1024 * 1024)),
        )
        .route("/api/unidades/{id}/activo", post(study_units::set_active))
        .route("/api/resources", get(resources::catalog))
        .route("/api/records/{resource}", get(resources::list))
        .route("/api/records/{resource}/{id}", get(resources::detail))
        .route("/api/export/{resource}", get(resources::export))
        .route("/api/export/{resource}/{format}", get(reports::export))
        .route("/api/certificados/{id}/pdf", get(reports::certificate))
        .route("/api/portafolio", get(reports::portfolio))
        .route("/api/dashboard", get(dashboard::panel))
        .route("/api/retos", post(workflows::create_challenge))
        .route("/api/retos/{id}", post(workflows::edit_challenge))
        .route("/api/retos/{id}/estado", post(workflows::challenge_state))
        .route("/api/retos/{id}/favorito", post(workflows::favorite))
        .route(
            "/api/retos/{id}/postular",
            post(workflows::apply).layer(axum::extract::DefaultBodyLimit::max(6 * 1024 * 1024)),
        )
        .route(
            "/api/postulaciones/{id}/estado",
            post(workflows::application_state),
        )
        .route(
            "/api/notificaciones/{id}/leer",
            post(workflows::read_notification),
        )
        .route("/api/preferencias", post(workflows::preferences))
        .route("/api/notificaciones/registro", get(workflows::delivery_log))
        .route("/api/notificaciones/mias", get(workflows::my_notifications))
        .route("/api/usuarios/{id}/estado", post(workflows::user_status))
        .route("/api/usuarios/{id}/rol", post(permissions::role))
        .route(
            "/api/usuarios/{id}/permisos",
            get(permissions::show_permissions).post(permissions::permissions),
        )
        .route("/api/empresas/{id}/bloqueo", post(permissions::block))
        .route(
            "/api/empresas/{id}/historial-documental",
            get(companies::document_history),
        )
        .route(
            "/api/tipos-documento",
            post(companies::create_document_type),
        )
        .route(
            "/api/tipos-documento/{id}",
            post(companies::edit_document_type),
        )
        .route("/api/retos/{id}/avance", get(companies::progress_panel))
        .route("/api/entregables/{id}/comentario", post(companies::comment))
        .route(
            "/api/empresa/modo-convenio",
            post(permissions::agreement_choice),
        )
        .route(
            "/api/empresa/documentos/{kind}/cargar",
            post(media::own_document).layer(axum::extract::DefaultBodyLimit::max(10 * 1024 * 1024)),
        )
        .route(
            "/api/empresas/{id}/documentos/{kind}/cargar",
            post(media::special_document)
                .layer(axum::extract::DefaultBodyLimit::max(10 * 1024 * 1024)),
        )
        .route("/api/retos/{id}/presupuesto", post(finance::set_budget))
        .route("/api/presupuestos/{id}/gastos", post(finance::expense))
        .route("/api/presupuestos/{id}/resumen", get(finance::summary))
        .route("/api/gastos/{id}/ejecutar", post(finance::execute))
        .route("/api/gastos/{id}/eliminar", post(finance::delete))
        .route("/api/retos/{id}/presupuesto/{format}", get(finance::export))
        .route("/api/retos/{id}/integrar", post(academic::integrate))
        .route("/api/integraciones/opciones", get(academic::options))
        .route("/api/integraciones/{id}", post(academic::edit))
        .route(
            "/api/integraciones/{id}/estado",
            post(academic::integration_state),
        )
        .route(
            "/api/integraciones/{id}/equipo",
            get(team::show).post(team::update),
        )
        .route("/api/entregables/{id}/calificar", post(grading::grade))
        .route(
            "/api/retos/{id}/entregar",
            post(media::submit).layer(axum::extract::DefaultBodyLimit::max(10 * 1024 * 1024)),
        )
        .route("/api/archivo/{resource}/{id}/{field}", get(media::download))
        .route(
            "/api/retos/{id}/seguimiento",
            post(lifecycle::progress)
                .layer(axum::extract::DefaultBodyLimit::max(110 * 1024 * 1024)),
        )
        .route(
            "/api/retos/{id}/archivos",
            post(media::reto_files).layer(axum::extract::DefaultBodyLimit::max(110 * 1024 * 1024)),
        )
        .route(
            "/api/retos/{id}/eliminar",
            post(workflows::delete_challenge),
        )
        .route("/api/retos/{id}/cerrar", post(lifecycle::close))
        .route(
            "/api/retos/{id}/cierre",
            get(closure::show)
                .post(closure::save)
                .layer(axum::extract::DefaultBodyLimit::max(10 * 1024 * 1024)),
        )
        .route(
            "/api/retos/{id}/cierre/entregables",
            post(closure::artifact).layer(axum::extract::DefaultBodyLimit::max(10 * 1024 * 1024)),
        )
        .route(
            "/api/retos/{id}/cierre/reconocimientos",
            post(closure::recognition),
        )
        .route(
            "/api/retos/{id}/solicitudes-logistica",
            post(closure::request),
        )
        .route(
            "/api/solicitudes-logistica/{id}/estado",
            post(closure::request_state),
        )
        .route("/api/encuestas/{id}", post(lifecycle::survey))
        .route("/api/empresas/{id}/revisar", post(companies::review))
        .route("/api/empresas/{id}/convenio", post(companies::agreement))
        .route("/api/documentos/{id}/revisar", post(companies::document))
        .route(
            "/api/documentos/{id}/cargar",
            post(media::company_document)
                .layer(axum::extract::DefaultBodyLimit::max(10 * 1024 * 1024)),
        )
        .route("/api/retos/{id}/equipo", post(hackathon::team))
        .route(
            "/api/equipos-academicos/{id}",
            get(hackathon::show_team).post(hackathon::edit_team),
        )
        .route(
            "/api/equipos-academicos/{id}/eliminar",
            post(hackathon::delete_team),
        )
        .route(
            "/api/retos/{id}/opciones",
            get(hackathon::challenge_options),
        )
        .route("/api/hackathons/{id}/opciones", get(hackathon::options))
        .route("/api/retos/{id}/hackathon", post(hackathon::create))
        .route("/api/hackathons/{id}/estado", post(hackathon::state))
        .route("/api/hackathons/{id}/etapa", post(hackathon::stage))
        .route("/api/hackathons/{id}/jurado", post(hackathon::juror))
        .route("/api/hackathons/{id}/inscribir", post(hackathon::register))
        .route(
            "/api/inscripciones/{id}/estado",
            post(hackathon::registration_state),
        )
        .route("/api/hackathons/{id}/votar", post(hackathon::vote))
        .route("/api/hackathons/{id}/ranking", get(hackathon::ranking))
        .route("/api/reportes/catalogo", get(builder::catalog))
        .route("/api/reportes/vista-previa", post(builder::preview))
        .route("/api/reportes/generar", post(builder::download))
        .route("/api/reportes/enviar", post(builder::send))
        .route("/api/retos/{id}/rubrica", post(grading::create_rubric))
        .route("/api/entregables/{id}/rubricas", get(grading::rubrics))
        .route(
            "/api/retos/{id}/publicar-calificaciones",
            post(grading::publish),
        )
        .route(
            "/api/hackathons/{id}/logistica",
            get(event::show_logistics).post(event::logistics),
        )
        .route("/api/hackathons/{id}/informe/{format}", get(event::report))
        .route("/api/verificar/{code}", get(resources::verify_certificate))
        .fallback(|| async {
            (
                axum::http::StatusCode::NOT_FOUND,
                Json(json!({"error":"Ruta no encontrada"})),
            )
        })
        .layer(axum::extract::DefaultBodyLimit::max(65536))
        .layer(middleware::from_fn_with_state(state.clone(), same_origin))
        .layer(middleware::from_fn(security_headers))
        .with_state(state)
}
/// Front-end files embedded in the binary: no build step and no CDN.
/// `index.html` loads the scripts in this order; they share the global scope.
const ASSETS: &[(&str, &str, &str)] = &[
    ("tokens.css", CSS, include_str!("../web/css/tokens.css")),
    ("base.css", CSS, include_str!("../web/css/base.css")),
    (
        "components.css",
        CSS,
        include_str!("../web/css/components.css"),
    ),
    ("shell.css", CSS, include_str!("../web/css/shell.css")),
    ("auth.css", CSS, include_str!("../web/css/auth.css")),
    (
        "dashboard.css",
        CSS,
        include_str!("../web/css/dashboard.css"),
    ),
    ("records.css", CSS, include_str!("../web/css/records.css")),
    ("detail.css", CSS, include_str!("../web/css/detail.css")),
    ("forms.css", CSS, include_str!("../web/css/forms.css")),
    (
        "workflows.css",
        CSS,
        include_str!("../web/css/workflows.css"),
    ),
    ("builder.css", CSS, include_str!("../web/css/builder.css")),
    ("icons.js", JS, include_str!("../web/js/icons.js")),
    ("core.js", JS, include_str!("../web/js/core.js")),
    ("ui.js", JS, include_str!("../web/js/ui.js")),
    ("dashboard.js", JS, include_str!("../web/js/dashboard.js")),
    ("records.js", JS, include_str!("../web/js/records.js")),
    ("detail.js", JS, include_str!("../web/js/detail.js")),
    ("workflows.js", JS, include_str!("../web/js/workflows.js")),
    ("auth.js", JS, include_str!("../web/js/auth.js")),
    ("evaluation.js", JS, include_str!("../web/js/evaluation.js")),
    ("builder.js", JS, include_str!("../web/js/builder.js")),
    ("forms.js", JS, include_str!("../web/js/forms.js")),
    ("main.js", JS, include_str!("../web/js/main.js")),
    // Same vendored Chart.js 4.4.1 the Django static/vendor used.
    ("chart.js", JS, include_str!("../web/chart.umd.min.js")),
];
const CSS: &str = "text/css; charset=utf-8";
const JS: &str = "text/javascript; charset=utf-8";

async fn asset(axum::extract::Path(file): axum::extract::Path<String>) -> Response {
    use axum::response::IntoResponse;
    match ASSETS.iter().find(|(name, _, _)| *name == file) {
        Some((_, mime, body)) => (
            [("content-type", *mime), ("cache-control", "no-cache")],
            *body,
        )
            .into_response(),
        None => axum::http::StatusCode::NOT_FOUND.into_response(),
    }
}

async fn health(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> error::Result<Json<serde_json::Value>> {
    sqlx::query("SELECT 1").execute(&state.pool).await?;
    Ok(Json(
        json!({"status":"ok", "runtime":"rust", "database":"postgresql"}),
    ))
}
/// AUTH-3 (login CSRF) and defence in depth for every state-changing request:
/// a browser request coming from another site is refused before any handler
/// runs. Clients that send neither header (curl, tests) still need the session
/// CSRF token on authenticated routes.
async fn same_origin(
    axum::extract::State(state): axum::extract::State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    use axum::http::Method;
    use axum::response::IntoResponse;
    let headers = request.headers();
    let unsafe_method = !matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    );
    let fetch_site = headers.get("sec-fetch-site").and_then(|v| v.to_str().ok());
    // Behind a trusted reverse proxy (RUST_TRUST_PROXY) the public host arrives
    // as X-Forwarded-Host; otherwise only the Host header counts.
    let host = state
        .config
        .trust_proxy
        .then(|| headers.get("x-forwarded-host"))
        .flatten()
        .or_else(|| headers.get("host"))
        .and_then(|v| v.to_str().ok());
    let origin_host = headers
        .get("origin")
        .and_then(|v| v.to_str().ok())
        .map(|o| o.split_once("://").map_or(o, |(_, rest)| rest));
    let foreign = matches!(fetch_site, Some("cross-site" | "same-site"))
        || matches!((origin_host, host), (Some(o), Some(h)) if !o.eq_ignore_ascii_case(h))
        || (origin_host.is_some() && host.is_none());
    if unsafe_method && foreign {
        return (
            axum::http::StatusCode::FORBIDDEN,
            Json(json!({"error":"Solicitud de otro sitio rechazada"})),
        )
            .into_response();
    }
    next.run(request).await
}
async fn security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers: &mut HeaderMap = response.headers_mut();
    for (name, value) in [
        ("x-content-type-options", "nosniff"),
        ("x-frame-options", "DENY"),
        ("referrer-policy", "same-origin"),
        ("cache-control", "no-store"),
        (
            "content-security-policy",
            // Everything is served by the app itself: no CDNs.
            "default-src 'self'; script-src 'self'; style-src 'self'; font-src 'self'; img-src 'self' data:; frame-ancestors 'none'; base-uri 'none'; form-action 'self'",
        ),
    ] {
        headers.insert(name, HeaderValue::from_static(value));
    }
    response
}
