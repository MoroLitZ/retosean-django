//! Self-registration and password recovery (Django usuarios: vista_registro,
//! RegistroAcademicoForm, RegistroEmpresaForm, PasswordReset* views).
use crate::{
    AppState, auth, domain,
    error::{AppError, Result},
    mail,
};
use axum::{
    Json,
    extract::State,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};

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

fn site() -> String {
    std::env::var("RUST_SITE_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".into())
}

/// GET (public): catalogs the registration forms need.
pub async fn options(State(state): State<AppState>) -> Result<Json<Value>> {
    let programas: Value = sqlx::query_scalar("SELECT coalesce(jsonb_agg(jsonb_build_object('value',p.id,'label',p.nombre||' ('||f.nombre||')') ORDER BY p.nombre),'[]') FROM programas p JOIN facultades f ON f.id=p.facultad_id")
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
    let permit = state
        .password_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| AppError::bad("Servidor ocupado. Intenta de nuevo"))?;
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
    Json(input): Json<Registration>,
) -> Result<Response> {
    let academic = match input.tipo.as_str() {
        "academico" => true,
        "empresa" => false,
        _ => return Err(AppError::bad("Tipo de registro inválido")),
    };
    domain::validate_text(&input.username, 150, "Usuario")?;
    let email = input.email.trim().to_string();
    if !input
        .username
        .chars()
        .all(|c| c.is_alphanumeric() || "@.+-_".contains(c))
    {
        return Err(AppError::bad("Usuario: solo letras, números y @ . + - _"));
    }
    if email.len() > 254 || email.split('@').filter(|p| !p.is_empty()).count() != 2 {
        return Err(AppError::bad("Indica un correo electrónico válido"));
    }
    if input.password != input.password2 {
        return Err(AppError::bad("Las contraseñas no coinciden"));
    }
    if input.password.chars().count() < 12 || input.password.len() > 1024 {
        return Err(AppError::bad(
            "La contraseña debe tener al menos 12 caracteres",
        ));
    }
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
        domain::validate_text(&input.nit, 20, "NIT de la Empresa")?;
        domain::validate_text(&input.razon_social, 150, "Razon Social")?;
        domain::validate_text(&input.sector_industrial, 100, "Sector Industrial")?;
        domain::validate_text(&input.area_responsable, 150, "Área responsable")?;
        let web = input.sitio_web.trim();
        if !["RETO", "HACKATHON", "AMBOS"].contains(&input.tipo_convocatoria.as_str())
            || (!web.is_empty()
                && (web.len() > 200
                    || !(web.starts_with("https://") || web.starts_with("http://"))))
        {
            return Err(AppError::bad(
                "Revisa el tipo de convocatoria y el sitio web",
            ));
        }
    }
    let taken: (bool, bool) = sqlx::query_as("SELECT EXISTS(SELECT 1 FROM usuarios WHERE lower(username)=lower($1)), EXISTS(SELECT 1 FROM empresas WHERE nit=$2 AND $2<>'')")
        .bind(&input.username).bind(input.nit.trim()).fetch_one(&state.pool).await?;
    if taken.0 {
        return Err(AppError::bad("Ya existe un usuario con ese nombre."));
    }
    if taken.1 {
        return Err(AppError::bad("Este NIT ya se encuentra registrado."));
    }
    let password = hash(&state, input.password.clone()).await?;
    let (rol, first_name) = if academic {
        (input.rol.clone(), input.first_name.trim().to_string())
    } else {
        ("EMPRESA".to_string(), input.razon_social.trim().to_string())
    };
    let mut tx = state.pool.begin().await?;
    let id: i64 = sqlx::query_scalar("INSERT INTO usuarios(username,password,email,first_name,last_name,rol,telefono,is_active,activo,is_staff,is_superuser,date_joined,fecha_creacion) VALUES($1,$2,$3,$4,$5,$6,'',true,true,false,false,now(),now()) RETURNING id")
        .bind(&input.username).bind(password).bind(&email).bind(&first_name).bind(input.last_name.trim()).bind(&rol)
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
    // enviar_bienvenida
    let name = if first_name.is_empty() {
        input.username.clone()
    } else {
        format!("{first_name} {}", input.last_name.trim())
            .trim()
            .to_string()
    };
    mail::send_later(
        email,
        "Bienvenido a RetosEAN".into(),
        format!(
            "Hola {name},\n\nTe damos la bienvenida a RetosEAN, la plataforma de retos Universidad-Empresa de la Universidad EAN.\n\nUsuario: {}\nIngresa en: {}/\n\nEquipo RetosEAN",
            input.username,
            site().trim_end_matches('/')
        ),
    );
    auth::start_session(&state, id).await
}

#[derive(Deserialize)]
pub struct ResetRequest {
    email: String,
}

/// POST (public): emails a single-use reset link valid for 3 days to every active
/// account with that email. Always answers the same, so it reveals no accounts.
pub async fn reset_request(
    State(state): State<AppState>,
    Json(input): Json<ResetRequest>,
) -> Result<Json<Value>> {
    let email = input.email.trim();
    // Throttle: at most 5 requests per address every 10 minutes (shares the login table).
    let subject = auth::digest(&format!("reset:{}", email.to_lowercase()));
    let recent: i64 = sqlx::query_scalar("SELECT count(*) FROM rust_login_attempts WHERE subject=$1 AND attempted_at>now()-interval '10 minutes'")
        .bind(&subject).fetch_one(&state.pool).await?;
    if recent >= 5 {
        return Ok(Json(json!({"ok": true})));
    }
    sqlx::query("INSERT INTO rust_login_attempts(subject) VALUES($1)")
        .bind(&subject)
        .execute(&state.pool)
        .await?;
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
                    site().trim_end_matches('/')
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
    if input.nueva.chars().count() < 12 || input.nueva.len() > 1024 || input.token.len() != 64 {
        return Err(AppError::bad(
            "La contraseña debe tener al menos 12 caracteres",
        ));
    }
    let user: Option<i64> = sqlx::query_scalar(
        "SELECT user_id FROM rust_password_resets WHERE token_hash=$1 AND expires_at>now()",
    )
    .bind(auth::digest(&input.token))
    .fetch_optional(&state.pool)
    .await?;
    let user = user.ok_or_else(|| {
        AppError::bad("El enlace para restablecer la contraseña no es válido o ya fue usado.")
    })?;
    let password = hash(&state, input.nueva.clone()).await?;
    let mut tx = state.pool.begin().await?;
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
