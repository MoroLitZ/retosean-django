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
            for kind in ["CAMARA_COMERCIO", "RUT", "CEDULA_REPRESENTANTE", "CONVENIO"] {
                sqlx::query("INSERT INTO documentos_empresas(empresa_id,tipo_documento,estado,fecha_carga) VALUES($1,$2,'PENDIENTE',now())").bind(company).bind(kind).execute(&mut *tx).await?;
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
}
pub async fn profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<Profile>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    domain::validate_text(&input.first_name, 150, "Nombre")?;
    domain::validate_text(&input.last_name, 150, "Apellido")?;
    if !input.email.contains('@') || input.email.len() > 254 || input.telefono.chars().count() > 20
    {
        return Err(AppError::bad("Datos de contacto inválidos"));
    }
    sqlx::query("UPDATE usuarios SET first_name=$2,last_name=$3,email=$4,telefono=$5 WHERE id=$1")
        .bind(user.id)
        .bind(input.first_name)
        .bind(input.last_name)
        .bind(input.email)
        .bind(input.telefono)
        .execute(&state.pool)
        .await?;
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
    sqlx::query("DELETE FROM rust_sessions WHERE user_id=$1")
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO log_actividad(usuario_id,identificador,accion,detalle,realizado_por_id,creado_en) VALUES($1,$2,'CAMBIO_ESTADO','Contraseña actualizada; sesiones revocadas',$1,now())").bind(user.id).bind(&user.username).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(json!({"ok":true,"login_required":true})))
}
