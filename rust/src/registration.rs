//! Self-registration and password recovery (Django usuarios: vista_registro,
//! RegistroAcademicoForm, RegistroEmpresaForm, PasswordReset* views).
use crate::{
    AppState, auth, domain,
    error::{AppError, Result},
    mail,
};
use axum::{
    Json,
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::net::SocketAddr;

pub(crate) const ESPECIALIDADES: [&str; 10] = [
    "Ingeniería de Software",
    "Sistemas de Información",
    "Inteligencia Artificial",
    "Gestión de Operaciones",
    "Finanzas Corporativas",
    "Marketing Digital",
    "Innovación y Emprendimiento",
    "Sostenibilidad y Ambiente",
    "Diseño y Creatividad",
    "Otra",
];

/// Public base URL for links in emails: RUST_SITE_URL, else the bind address.
/// Shared with the notification outbox (main.rs) so every link agrees.
pub fn site_url(config: &crate::config::Config) -> String {
    std::env::var("RUST_SITE_URL")
        .unwrap_or_else(|_| format!("http://{}", config.bind))
        .trim_end_matches('/')
        .to_string()
}

/// GET (public): catalogs the registration forms need.
pub async fn options(State(state): State<AppState>) -> Result<Json<Value>> {
    let programas: Value = sqlx::query_scalar("SELECT coalesce(jsonb_agg(jsonb_build_object('value',p.id,'nombre',p.nombre,'label',p.nombre||' ('||f.nombre||')') ORDER BY p.nombre),'[]') FROM programas p JOIN facultades f ON f.id=p.facultad_id")
        .fetch_one(&state.pool).await?;
    let facultades: Value = sqlx::query_scalar("SELECT coalesce(jsonb_agg(jsonb_build_object('value',id,'label',nombre) ORDER BY nombre),'[]') FROM facultades")
        .fetch_one(&state.pool).await?;
    Ok(Json(json!({
        "programas": programas, "facultades": facultades, "especialidades": ESPECIALIDADES,
        "convocatorias": [["RETO","Reto"],["HACKATHON","Hackathon"],["AMBOS","Reto y hackathon"]],
    })))
}

#[derive(Deserialize)]
pub struct Registration {
    tipo: String,
    username: String,
    email: String,
    password: String,
    password2: String,
    #[serde(default)]
    first_name: String,
    #[serde(default)]
    last_name: String,
    // academic
    #[serde(default)]
    rol: String,
    programa_id: Option<i64>,
    semestre: Option<i32>,
    facultad_id: Option<i64>,
    #[serde(default)]
    especialidad: String,
    // company
    #[serde(default)]
    nit: String,
    #[serde(default)]
    razon_social: String,
    #[serde(default)]
    sector_industrial: String,
    #[serde(default)]
    tipo_convocatoria: String,
    #[serde(default)]
    area_responsable: String,
    #[serde(default)]
    sitio_web: String,
}

fn bounded(value: &str, max: usize) -> bool {
    value.chars().count() <= max
}

async fn hash(state: &AppState, password: String) -> Result<String> {
    let permit = auth::hash_slot(&state.password_slots).await?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        auth::hash_password(&password)
    })
    .await
    .map_err(|_| AppError::bad("No se pudo guardar la contraseña"))
}

/// POST (public): creates the account and its profile, sends the welcome email
/// and opens a session, like vista_registro.
pub async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    peer: Option<axum::Extension<ConnectInfo<SocketAddr>>>,
    Json(input): Json<Registration>,
) -> Result<Response> {
    let academic = match input.tipo.as_str() {
        "academico" => true,
        "empresa" => false,
        _ => return Err(AppError::bad("Tipo de registro inválido")),
    };
    domain::validate_text(&input.username, 150, "Usuario")?;
    let email = input.email.trim().to_string();
    // `@` is not allowed so a username can never equal another user's email and
    // hijack the email-login path (AUTHK-2).
    if !input
        .username
        .chars()
        .all(|c| c.is_alphanumeric() || ".+-_".contains(c))
    {
        return Err(AppError::bad("Usuario: solo letras, números y . + - _"));
    }
    if email.len() > 254 || !domain::valid_email(&email) {
        return Err(AppError::bad("Indica un correo electrónico válido"));
    }
    if input.password != input.password2 {
        return Err(AppError::bad("Las contraseñas no coinciden"));
    }
    // CFG-02: shared validator (length, numeric, common, similar to the user).
    domain::validate_password(&input.password, Some(&input.username), Some(&email))?;
    if !bounded(&input.first_name, 150) || !bounded(&input.last_name, 150) {
        return Err(AppError::bad("Nombre o apellido demasiado largos"));
    }
    if academic {
        if !["ESTUDIANTE", "PROFESOR"].contains(&input.rol.as_str()) {
            return Err(AppError::bad("Elige estudiante o profesor"));
        }
        if !email.to_lowercase().ends_with("@universidadean.edu.co") {
            return Err(AppError::bad(
                "Estudiantes y profesores deben registrarse con su correo institucional.",
            ));
        }
        if input.semestre.is_some_and(|s| !(1..=10).contains(&s))
            || (!input.especialidad.is_empty()
                && !ESPECIALIDADES.contains(&input.especialidad.as_str()))
        {
            return Err(AppError::bad("Revisa el semestre y la especialidad"));
        }
    } else {
        domain::validate_text(&input.nit, 20, "NIT de la empresa")?;
        domain::validate_text(&input.razon_social, 150, "Razón social")?;
        domain::validate_text(&input.sector_industrial, 100, "Sector industrial")?;
        domain::validate_text(&input.area_responsable, 150, "Área responsable")?;
        let web = input.sitio_web.trim();
        if !["RETO", "HACKATHON", "AMBOS"].contains(&input.tipo_convocatoria.as_str())
            || (!web.is_empty()
                && (web.chars().count() > 200
                    || !(web.starts_with("https://") || web.starts_with("http://"))))
        {
            return Err(AppError::bad(
                "Revisa el tipo de convocatoria y el sitio web",
            ));
        }
    }
    // Per-IP registration throttle, reusing the login-attempt table (AUTH-9).
    let ip = auth::client_ip(
        &headers,
        state.config.trust_proxy,
        peer.map(|axum::Extension(ConnectInfo(addr))| addr),
    );
    if let Some(ip) = ip.as_deref() {
        auth::check_subject(&state, &auth::digest(&format!("registro:{ip}")), 10).await?;
    }
    let password = hash(&state, input.password.clone()).await?;
    let (rol, first_name) = if academic {
        (input.rol.clone(), input.first_name.trim().to_string())
    } else {
        ("EMPRESA".to_string(), input.razon_social.trim().to_string())
    };
    // A self-registered professor starts inactive, pending admin approval
    // (the admin approves with the user-status endpoint); everyone else enters at once.
    let pending = rol == "PROFESOR";
    let mut tx = state.pool.begin().await?;
    // Uniqueness is re-checked under an advisory lock inside the transaction so
    // concurrent sign-ups cannot both pass a check before inserting (AUTHK-12).
    // The conflict response is uniform so it does not reveal which value exists.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(format!("usuario:{}", input.username.trim().to_lowercase()))
        .execute(&mut *tx)
        .await?;
    // Same email, different username: serialized too (always username, then email).
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(format!("correo:{}", email.to_lowercase()))
        .execute(&mut *tx)
        .await?;
    let occupied: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM usuarios WHERE lower(username)=lower($1) OR lower(email)=lower($2))",
    )
    .bind(&input.username)
    .bind(&email)
    .fetch_one(&mut *tx)
    .await?;
    if occupied {
        return Err(AppError::bad(
            "No se pudo completar el registro. Revisa los datos e inténtalo de nuevo.",
        ));
    }
    if !academic {
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
            .bind(format!("nit:{}", input.nit.trim()))
            .execute(&mut *tx)
            .await?;
        let nit_taken: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM empresas WHERE nit=$1 AND $1<>'')")
                .bind(input.nit.trim())
                .fetch_one(&mut *tx)
                .await?;
        if nit_taken {
            return Err(AppError::bad(
                "No se pudo completar el registro. Revisa los datos e inténtalo de nuevo.",
            ));
        }
    }
    // Self-registered students/companies stay unverified until they open the link
    // sent to their email (finding #1); pending professors are verified once an
    // administrator approves them, so `correo_verificado` mirrors `pending`.
    let id: i64 = sqlx::query_scalar("INSERT INTO usuarios(username,password,email,first_name,last_name,rol,telefono,is_active,activo,correo_verificado,is_staff,is_superuser,date_joined,fecha_creacion) VALUES($1,$2,$3,$4,$5,$6,'',$7,$7,$8,false,false,now(),now()) RETURNING id")
        .bind(&input.username).bind(password).bind(&email).bind(&first_name).bind(input.last_name.trim()).bind(&rol).bind(!pending).bind(pending)
        .fetch_one(&mut *tx).await?;
    match rol.as_str() {
        "ESTUDIANTE" => {
            sqlx::query(
                "INSERT INTO estudiantes(usuario_id,programa_id,semestre) VALUES($1,$2,$3)",
            )
            .bind(id)
            .bind(input.programa_id)
            .bind(input.semestre.unwrap_or(1))
            .execute(&mut *tx)
            .await?;
        }
        "PROFESOR" => {
            sqlx::query(
                "INSERT INTO profesores(usuario_id,facultad_id,especialidad) VALUES($1,$2,$3)",
            )
            .bind(id)
            .bind(input.facultad_id)
            .bind(&input.especialidad)
            .execute(&mut *tx)
            .await?;
        }
        _ => {
            let company: i64 = sqlx::query_scalar("INSERT INTO empresas(usuario_id,nit,razon_social,sector_industrial,tipo_convocatoria,area_responsable,sitio_web,fecha_creacion,estado_validacion,estado_listas_restrictivas,fecha_ultima_actualizacion,renuncio_a_convenio,estado_convenio) VALUES($1,$2,$3,$4,$5,$6,$7,(now() at time zone 'America/Bogota')::date,'PENDIENTE','PENDIENTE',now(),false,'NO_RADICADO') RETURNING id")
                .bind(id).bind(input.nit.trim()).bind(input.razon_social.trim()).bind(input.sector_industrial.trim())
                .bind(&input.tipo_convocatoria).bind(input.area_responsable.trim()).bind(input.sitio_web.trim())
                .fetch_one(&mut *tx).await?;
            sqlx::query("INSERT INTO documentos_empresas(empresa_id,tipo_documento,estado,fecha_carga) SELECT $1,codigo,'PENDIENTE',now() FROM tipos_documento_legal WHERE activo ORDER BY orden")
                .bind(company).execute(&mut *tx).await?;
        }
    }
    sqlx::query("INSERT INTO log_actividad(usuario_id,identificador,accion,detalle,realizado_por_id,creado_en) VALUES($1,$2,'CREACION','Registro desde la plataforma',$1,now())")
        .bind(id).bind(&input.username).execute(&mut *tx).await?;
    tx.commit().await?;
    let name = if first_name.is_empty() {
        input.username.clone()
    } else {
        format!("{first_name} {}", input.last_name.trim())
            .trim()
            .to_string()
    };
    if pending {
        // No session: an administrator must approve the account first.
        mail::send_later(
            email,
            "Bienvenido a RetosEAN".into(),
            format!(
                "Hola {name}:\n\nRecibimos tu registro como profesor en RetosEAN. Tu cuenta está pendiente de aprobación por un administrador; te avisaremos cuando puedas ingresar.\n\nUsuario: {}\n\nEquipo RetosEAN",
                input.username
            ),
        );
        return Ok(Json(json!({"ok":true,"pendiente_aprobacion":true,"mensaje":"Tu cuenta de profesor fue creada y está pendiente de aprobación por un administrador."})).into_response());
    }
    // ESTUDIANTE / EMPRESA (finding #1): no session until the email is proven.
    // Reuse the reset-token mechanism (digested, expiring, single-use link).
    let token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    sqlx::query("INSERT INTO rust_email_verifications(token_hash,user_id,expires_at) VALUES($1,$2,now()+interval '3 days')")
        .bind(auth::digest(&token))
        .bind(id)
        .execute(&state.pool)
        .await?;
    mail::send_later(
        email,
        "Confirma tu correo de RetosEAN".into(),
        format!(
            "Hola {name}:\n\nGracias por registrarte en RetosEAN.\n\nPara activar tu cuenta, confirma tu correo abriendo este enlace:\n\n{}/#verificar/{token}\n\nSi no creaste esta cuenta, ignora este mensaje.\n\n— Equipo RetosEAN",
            site_url(&state.config)
        ),
    );
    Ok(Json(json!({"ok":true,"correo_pendiente":true,"mensaje":"Revisa tu correo para confirmar tu cuenta antes de iniciar sesión."})).into_response())
}

#[derive(Deserialize)]
pub struct ResetRequest {
    email: String,
}

/// POST (public): emails a single-use reset link valid for 3 days to every active
/// account with that email. Always answers the same, so it reveals no accounts.
pub async fn reset_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    peer: Option<axum::Extension<ConnectInfo<SocketAddr>>>,
    Json(input): Json<ResetRequest>,
) -> Result<Json<Value>> {
    let email = input.email.trim();
    let ip = auth::client_ip(
        &headers,
        state.config.trust_proxy,
        peer.map(|axum::Extension(ConnectInfo(addr))| addr),
    );
    // Per-client throttle (AUTHK-10): a flood from one address is capped regardless
    // of how many different emails it targets. Throttling stays silent so the
    // endpoint keeps its uniform response.
    if let Some(ip) = ip.as_deref()
        && let Err(error) =
            auth::check_subject(&state, &auth::digest(&format!("reset-ip:{ip}")), 20).await
    {
        if error.0 == StatusCode::TOO_MANY_REQUESTS {
            return Ok(Json(json!({"ok": true})));
        }
        return Err(error);
    }
    // Per-address throttle under an advisory lock so concurrent requests cannot
    // all pass the check before any insert commits (AUTH-6). Stale rows are also
    // purged here, since this is another write path into the shared table (AUTH-7).
    let subject = auth::digest(&format!("reset:{}", email.to_lowercase()));
    let mut tx = state.pool.begin().await?;
    sqlx::query("DELETE FROM rust_login_attempts WHERE attempted_at < now()-interval '10 minutes'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(&subject)
        .execute(&mut *tx)
        .await?;
    let recent: i64 = sqlx::query_scalar("SELECT count(*) FROM rust_login_attempts WHERE subject=$1 AND attempted_at>now()-interval '10 minutes'")
        .bind(&subject).fetch_one(&mut *tx).await?;
    if recent >= 5 {
        tx.commit().await?;
        return Ok(Json(json!({"ok": true})));
    }
    sqlx::query("INSERT INTO rust_login_attempts(subject) VALUES($1)")
        .bind(&subject)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    if email.len() <= 254 && email.contains('@') {
        let users: Vec<(i64, String, String)> = sqlx::query_as("SELECT id,coalesce(nullif(trim(first_name||' '||last_name),''),username),email FROM usuarios WHERE lower(email)=lower($1) AND is_active AND activo")
            .bind(email).fetch_all(&state.pool).await?;
        for (id, name, address) in users {
            let token = format!(
                "{}{}",
                uuid::Uuid::new_v4().simple(),
                uuid::Uuid::new_v4().simple()
            );
            sqlx::query("INSERT INTO rust_password_resets(token_hash,user_id,expires_at) VALUES($1,$2,now()+interval '3 days')")
                .bind(auth::digest(&token)).bind(id).execute(&state.pool).await?;
            mail::send_later(
                address,
                "Restablece tu contraseña de RetosEAN".into(),
                format!(
                    "Hola {name}:\n\nRecibimos una solicitud para restablecer la contraseña de tu cuenta en RetosEAN.\n\nPara elegir una nueva contraseña, abre el siguiente enlace:\n\n{}/#restablecer/{token}\n\nSi no solicitaste este cambio, ignora este mensaje y tu contraseña seguirá siendo la misma.\n\n— Equipo RetosEAN",
                    site_url(&state.config)
                ),
            );
        }
    }
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
pub struct ResetConfirm {
    token: String,
    nueva: String,
    nueva2: String,
}

/// POST (public): sets the new password, consumes the link and signs the account
/// out everywhere (PasswordResetConfirmView).
pub async fn reset_confirm(
    State(state): State<AppState>,
    Json(input): Json<ResetConfirm>,
) -> Result<impl IntoResponse> {
    if input.nueva != input.nueva2 {
        return Err(AppError::bad("Las contraseñas no coinciden"));
    }
    if input.token.len() != 64 {
        return Err(AppError::bad(
            "El enlace para restablecer la contraseña no es válido o ya fue usado.",
        ));
    }
    // The link is resolved before validating the password, so CFG-02 can check
    // similarity against the account's attributes (Django's SetPasswordForm
    // receives the user for exactly that).
    let account: Option<(i64, String, String)> = sqlx::query_as(
        "SELECT r.user_id,u.username,u.email FROM rust_password_resets r JOIN usuarios u ON u.id=r.user_id WHERE r.token_hash=$1 AND r.expires_at>now() AND u.is_active AND u.activo",
    )
    .bind(auth::digest(&input.token))
    .fetch_optional(&state.pool)
    .await?;
    let (user, username, email) = account.ok_or_else(|| {
        AppError::bad("El enlace para restablecer la contraseña no es válido o ya fue usado.")
    })?;
    domain::validate_password(&input.nueva, Some(&username), Some(&email))?;
    let password = hash(&state, input.nueva.clone()).await?;
    let mut tx = state.pool.begin().await?;
    // Consume the token atomically within the same transaction that updates the
    // password, so a concurrent request loses the DELETE race (AUTH-8).
    let consumed: Option<i64> = sqlx::query_scalar(
        "DELETE FROM rust_password_resets WHERE token_hash=$1 AND expires_at>now() RETURNING user_id",
    )
    .bind(auth::digest(&input.token))
    .fetch_optional(&mut *tx)
    .await?;
    if consumed.is_none() {
        return Err(AppError::bad(
            "El enlace para restablecer la contraseña no es válido o ya fue usado.",
        ));
    }
    sqlx::query("UPDATE usuarios SET password=$2 WHERE id=$1")
        .bind(user)
        .bind(password)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM rust_password_resets WHERE user_id=$1")
        .bind(user)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM rust_sessions WHERE user_id=$1")
        .bind(user)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
pub struct VerifyConfirm {
    token: String,
}

/// POST (public): confirms the email of a self-registered account and consumes
/// the single-use link (finding #1). No session is opened here; the user logs in
/// afterwards.
pub async fn verify(
    State(state): State<AppState>,
    Json(input): Json<VerifyConfirm>,
) -> Result<Json<Value>> {
    if input.token.len() != 64 {
        return Err(AppError::bad(
            "El enlace de verificación no es válido o ya fue usado.",
        ));
    }
    let mut tx = state.pool.begin().await?;
    // Consume the token atomically so a concurrent/next attempt loses the race.
    let consumed: Option<i64> = sqlx::query_scalar(
        "DELETE FROM rust_email_verifications WHERE token_hash=$1 AND expires_at>now() RETURNING user_id",
    )
    .bind(auth::digest(&input.token))
    .fetch_optional(&mut *tx)
    .await?;
    let Some(user) = consumed else {
        return Err(AppError::bad(
            "El enlace de verificación no es válido o ya fue usado.",
        ));
    };
    sqlx::query("UPDATE usuarios SET correo_verificado=true WHERE id=$1 AND is_active AND activo")
        .bind(user)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({"ok": true})))
}
