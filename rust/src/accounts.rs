use crate::{
    AppState, auth, domain,
    error::{AppError, Result},
};
use axum::{Json, extract::State, http::HeaderMap};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
pub struct NewUser {
    username: String,
    password: String,
    email: String,
    first_name: String,
    last_name: String,
    rol: String,
    #[serde(default)]
    telefono: String,
    #[serde(default)]
    nit: String,
    #[serde(default)]
    razon_social: String,
    programa_id: Option<i64>,
    facultad_id: Option<i64>,
}
pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<NewUser>,
) -> Result<Json<Value>> {
    let admin = auth::current(&state, &headers, true).await?;
    if !admin.admin() {
        return Err(AppError::forbidden());
    }
    domain::validate_text(&input.username, 150, "Usuario")?;
    domain::validate_text(&input.first_name, 150, "Nombre")?;
    domain::validate_text(&input.last_name, 150, "Apellido")?;
    if !input
        .username
        .chars()
        .all(|c| c.is_alphanumeric() || "@.+-_".contains(c))
        || !input.email.contains('@')
        || input.email.len() > 254
        || input.telefono.chars().count() > 20
        || input.password.chars().count() < 12
        || input.password.len() > 1024
        || !["EMPRESA", "PROFESOR", "ESTUDIANTE", "EXPERTO"].contains(&input.rol.as_str())
    {
        return Err(AppError::bad(
            "Revisa el usuario, correo y rol; la contraseña debe tener al menos 12 caracteres",
        ));
    }
    if input.rol == "EMPRESA" {
        domain::validate_text(&input.nit, 20, "NIT")?;
        domain::validate_text(&input.razon_social, 150, "Razón social")?;
    }
    let permit = state
        .password_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| AppError::bad("Servidor ocupado"))?;
    let password = input.password;
    let hash = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        auth::hash_password(&password)
    })
    .await
    .map_err(|_| AppError::bad("No se pudo guardar la contraseña"))?;
    let mut tx = state.pool.begin().await?;
    let id:i64=sqlx::query_scalar("INSERT INTO usuarios(username,password,email,first_name,last_name,rol,telefono,is_active,activo,is_staff,is_superuser,date_joined,fecha_creacion) VALUES($1,$2,$3,$4,$5,$6,$7,true,true,false,false,now(),now()) RETURNING id")
        .bind(&input.username).bind(hash).bind(&input.email).bind(&input.first_name).bind(&input.last_name).bind(&input.rol).bind(&input.telefono).fetch_one(&mut *tx).await?;
    match input.rol.as_str() {
        "ESTUDIANTE" => {
            sqlx::query("INSERT INTO estudiantes(usuario_id,programa_id,semestre) VALUES($1,$2,1)")
                .bind(id)
                .bind(input.programa_id)
                .execute(&mut *tx)
                .await?;
        }
        "PROFESOR" => {
            sqlx::query(
                "INSERT INTO profesores(usuario_id,facultad_id,especialidad) VALUES($1,$2,'')",
            )
            .bind(id)
            .bind(input.facultad_id)
            .execute(&mut *tx)
            .await?;
        }
        "EMPRESA" => {
            let company:i64=sqlx::query_scalar("INSERT INTO empresas(usuario_id,nit,razon_social,sector_industrial,fecha_creacion,estado_validacion,estado_listas_restrictivas,fecha_ultima_actualizacion,renuncio_a_convenio,estado_convenio) VALUES($1,$2,$3,'',(now() at time zone 'America/Bogota')::date,'PENDIENTE','PENDIENTE',now(),false,'NO_RADICADO') RETURNING id").bind(id).bind(&input.nit).bind(&input.razon_social).fetch_one(&mut *tx).await?;
            // One pending row per active catalog type (HU00), so the company can upload each.
            {
                sqlx::query("INSERT INTO documentos_empresas(empresa_id,tipo_documento,estado,fecha_carga) SELECT $1,codigo,'PENDIENTE',now() FROM tipos_documento_legal WHERE activo ORDER BY orden").bind(company).execute(&mut *tx).await?;
            }
        }
        _ => {}
    }
    sqlx::query("INSERT INTO log_actividad(usuario_id,identificador,accion,detalle,realizado_por_id,creado_en) VALUES($1,$2,'CREACION','Cuenta creada desde Rust',$3,now())").bind(id).bind(&input.username).bind(admin.id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(json!({"id":id})))
}
#[derive(Deserialize)]
pub struct Profile {
    first_name: String,
    last_name: String,
    email: String,
    #[serde(default)]
    telefono: String,
    // EditarPerfilForm role fields; all optional as in Django.
    primera_carrera: Option<i64>,
    semestre: Option<i32>,
    facultad: Option<i64>,
    #[serde(default)]
    especialidad: String,
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
/// The profile form's initial values, per role (EditarPerfilForm.__init__).
pub async fn show_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, false).await?;
    let data: Value = sqlx::query_scalar("SELECT jsonb_build_object('first_name',u.first_name,'last_name',u.last_name,'email',u.email,'telefono',u.telefono,'primera_carrera',s.programa_id,'semestre',s.semestre,'facultad',p.facultad_id,'especialidad',COALESCE(p.especialidad,''),'razon_social',e.razon_social,'sector_industrial',e.sector_industrial,'tipo_convocatoria',e.tipo_convocatoria,'area_responsable',e.area_responsable,'sitio_web',e.sitio_web) FROM usuarios u LEFT JOIN estudiantes s ON s.usuario_id=u.id LEFT JOIN profesores p ON p.usuario_id=u.id LEFT JOIN empresas e ON e.usuario_id=u.id WHERE u.id=$1")
        .bind(user.id).fetch_one(&state.pool).await?;
    Ok(Json(data))
}
pub async fn profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<Profile>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    // AbstractUser: first and last name may be blank (a company has no last name).
    if input.first_name.chars().count() > 150 || input.last_name.chars().count() > 150 {
        return Err(AppError::bad("Nombre y apellido: máximo 150 caracteres"));
    }
    if !input.email.contains('@') || input.email.len() > 254 || input.telefono.chars().count() > 20
    {
        return Err(AppError::bad("Datos de contacto inválidos"));
    }
    if ["ESTUDIANTE", "PROFESOR"].contains(&user.rol.as_str())
        && !input
            .email
            .to_lowercase()
            .ends_with("@universidadean.edu.co")
    {
        return Err(AppError::bad(
            "Estudiantes y profesores deben usar su correo institucional (@universidadean.edu.co).",
        ));
    }
    if input.semestre.is_some_and(|s| !(1..=10).contains(&s))
        || (!input.especialidad.is_empty()
            && !crate::registration::ESPECIALIDADES.contains(&input.especialidad.as_str()))
        || (!input.tipo_convocatoria.is_empty()
            && !["RETO", "HACKATHON", "AMBOS"].contains(&input.tipo_convocatoria.as_str()))
        || input.razon_social.chars().count() > 150
        || input.sector_industrial.chars().count() > 100
        || input.area_responsable.chars().count() > 150
    {
        return Err(AppError::bad("Revisa los datos del perfil"));
    }
    let web = input.sitio_web.trim();
    if !web.is_empty()
        && (web.len() > 200 || !(web.starts_with("https://") || web.starts_with("http://")))
    {
        return Err(AppError::bad("Introduzca una URL válida."));
    }
    let mut tx = state.pool.begin().await?;
    sqlx::query("UPDATE usuarios SET first_name=$2,last_name=$3,email=$4,telefono=$5 WHERE id=$1")
        .bind(user.id)
        .bind(&input.first_name)
        .bind(&input.last_name)
        .bind(&input.email)
        .bind(&input.telefono)
        .execute(&mut *tx)
        .await?;
    match user.rol.as_str() {
        "ESTUDIANTE" => {
            // A missing profile is created; an empty program keeps the current one.
            sqlx::query("INSERT INTO estudiantes(usuario_id,programa_id,semestre) VALUES($1,(SELECT id FROM programas WHERE id=$2),$3) ON CONFLICT(usuario_id) DO UPDATE SET programa_id=COALESCE(excluded.programa_id,estudiantes.programa_id),semestre=excluded.semestre")
                .bind(user.id).bind(input.primera_carrera).bind(input.semestre.unwrap_or(1)).execute(&mut *tx).await?;
        }
        "PROFESOR" => {
            sqlx::query("INSERT INTO profesores(usuario_id,facultad_id,especialidad) VALUES($1,(SELECT id FROM facultades WHERE id=$2),$3) ON CONFLICT(usuario_id) DO UPDATE SET facultad_id=COALESCE(excluded.facultad_id,profesores.facultad_id),especialidad=excluded.especialidad")
                .bind(user.id).bind(input.facultad).bind(&input.especialidad).execute(&mut *tx).await?;
        }
        "EMPRESA" => {
            // Blank legal name, sector or call type keep the stored value, as Django.
            sqlx::query("UPDATE empresas SET razon_social=COALESCE(NULLIF($2,''),razon_social),sector_industrial=COALESCE(NULLIF($3,''),sector_industrial),tipo_convocatoria=COALESCE(NULLIF($4,''),tipo_convocatoria),area_responsable=$5,sitio_web=$6 WHERE usuario_id=$1")
                .bind(user.id).bind(input.razon_social.trim()).bind(input.sector_industrial.trim()).bind(&input.tipo_convocatoria).bind(input.area_responsable.trim()).bind(web).execute(&mut *tx).await?;
        }
        _ => {}
    }
    tx.commit().await?;
    Ok(Json(json!({"ok":true})))
}
#[derive(Deserialize)]
pub struct PasswordChange {
    actual: String,
    nueva: String,
}
pub async fn password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<PasswordChange>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    if input.nueva.chars().count() < 12 || input.nueva.len() > 1024 || input.actual.len() > 1024 {
        return Err(AppError::bad(
            "La contraseña nueva debe tener al menos 12 caracteres",
        ));
    }
    let old: String = sqlx::query_scalar("SELECT password FROM usuarios WHERE id=$1")
        .bind(user.id)
        .fetch_one(&state.pool)
        .await?;
    let comparison = old.clone();
    let permit = state
        .password_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| AppError::bad("Servidor ocupado"))?;
    let hash = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        if !auth::verify_password(&input.actual, &comparison) {
            None
        } else {
            Some(auth::hash_password(&input.nueva))
        }
    })
    .await
    .map_err(|_| AppError::bad("No se pudo cambiar la contraseña"))?
    .ok_or_else(|| AppError::bad("Contraseña actual incorrecta"))?;
    let mut tx = state.pool.begin().await?;
    let changed = sqlx::query("UPDATE usuarios SET password=$2 WHERE id=$1 AND password=$3")
        .bind(user.id)
        .bind(hash)
        .bind(old)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if changed == 0 {
        return Err(AppError::bad(
            "La contraseña cambió durante la operación; inicia sesión de nuevo",
        ));
    }
    // A password change also invalidates pending reset links, as Django's tokens do.
    sqlx::query("DELETE FROM rust_password_resets WHERE user_id=$1")
        .bind(user.id)
        .execute(&state.pool)
        .await?;
    sqlx::query("DELETE FROM rust_sessions WHERE user_id=$1")
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO log_actividad(usuario_id,identificador,accion,detalle,realizado_por_id,creado_en) VALUES($1,$2,'CAMBIO_ESTADO','Contraseña actualizada; sesiones revocadas',$1,now())").bind(user.id).bind(&user.username).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(json!({"ok":true,"login_required":true})))
}

#[derive(Deserialize)]
pub struct LogFilters {
    #[serde(default)]
    accion: String,
    #[serde(default)]
    usuario: String,
    desde: Option<chrono::NaiveDate>,
    hasta: Option<chrono::NaiveDate>,
    page: Option<i64>,
}
/// log_actividad (HU13): access and account changes, filterable, for admins.
pub async fn activity_log(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Query(filters): axum::extract::Query<LogFilters>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, false).await?;
    if !user.admin() {
        return Err(AppError::forbidden());
    }
    let page = filters.page.unwrap_or(1).clamp(1, 100000);
    let records: Value = sqlx::query_scalar("SELECT COALESCE(jsonb_agg(r ORDER BY r.creado_en DESC, r.id DESC),'[]') FROM (SELECT l.id,l.identificador,l.accion,l.detalle,host(l.ip) ip,l.creado_en,u.username usuario,p.username realizado_por FROM log_actividad l LEFT JOIN usuarios u ON u.id=l.usuario_id LEFT JOIN usuarios p ON p.id=l.realizado_por_id WHERE ($1='' OR l.accion=$1) AND ($2='' OR l.identificador ILIKE '%'||$2||'%' OR u.username ILIKE '%'||$2||'%' OR p.username ILIKE '%'||$2||'%') AND ($3::date IS NULL OR (l.creado_en AT TIME ZONE 'America/Bogota')::date>=$3) AND ($4::date IS NULL OR (l.creado_en AT TIME ZONE 'America/Bogota')::date<=$4) ORDER BY l.creado_en DESC,l.id DESC LIMIT 50 OFFSET $5) r")
        .bind(&filters.accion).bind(filters.usuario.trim()).bind(filters.desde).bind(filters.hasta).bind((page - 1) * 50)
        .fetch_one(&state.pool).await?;
    Ok(Json(json!({"registros": records, "page": page})))
}
