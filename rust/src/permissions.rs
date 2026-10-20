//! HU13 role and per-module permissions, HU00 participation block and the
//! company operating gate (apps/usuarios/services.py, apps/empresas/services.py).
use crate::{
    AppState, auth,
    error::{AppError, Result},
    workflows::{history, notify},
};
use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Transaction};

/// usuarios/permisos.py PERMISOS_MODULO: (app_label.codename, nombre, descripcion).
pub const PERMISOS_MODULO: &[(&str, &str, &str)] = &[(
    "empresas.cargar_listas_informa",
    "Cargar Listas Informa",
    "Carga el documento Listas Informa de las empresas (HU00). Normalmente un líder de facultad designado por el líder de retos.",
)];
const ROLES: &[&str] = &["ADMIN", "EMPRESA", "PROFESOR", "ESTUDIANTE", "EXPERTO"];

/// Django has_perm: superusers always, everyone else by explicit grant.
pub async fn has_perm(pool: &PgPool, user: &auth::User, code: &str) -> Result<bool> {
    if user.is_superuser {
        return Ok(true);
    }
    let (app, codename) = code.split_once('.').unwrap_or(("", code));
    Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM usuarios_user_permissions up JOIN auth_permission p ON p.id=up.permission_id JOIN django_content_type c ON c.id=p.content_type_id WHERE up.usuario_id=$1 AND c.app_label=$2 AND p.codename=$3)")
        .bind(user.id).bind(app).bind(codename).fetch_one(pool).await?)
}
async fn granted(tx: &mut Transaction<'_, Postgres>, id: i64) -> Result<Vec<String>> {
    let codes: Vec<String> = sqlx::query_scalar("SELECT c.app_label||'.'||p.codename FROM usuarios_user_permissions up JOIN auth_permission p ON p.id=up.permission_id JOIN django_content_type c ON c.id=p.content_type_id WHERE up.usuario_id=$1 ORDER BY 1")
        .bind(id).fetch_all(&mut **tx).await?;
    Ok(codes
        .into_iter()
        .filter(|c| PERMISOS_MODULO.iter().any(|(code, ..)| code == c))
        .collect())
}
async fn admin(state: &AppState, headers: &HeaderMap) -> Result<auth::User> {
    let user = auth::current(state, headers, true).await?;
    if !user.admin() {
        return Err(AppError::forbidden());
    }
    Ok(user)
}

#[derive(Deserialize)]
pub struct RoleInput {
    rol: String,
}
/// cambiar_rol: an administrator cannot change their own role.
pub async fn role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<RoleInput>,
) -> Result<Json<Value>> {
    let user = admin(&state, &headers).await?;
    if id == user.id {
        return Err(AppError::bad("No puedes cambiar tu propio rol."));
    }
    if !ROLES.contains(&input.rol.as_str()) {
        return Err(AppError::bad("Rol inválido"));
    }
    let mut tx = state.pool.begin().await?;
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT rol,username FROM usuarios WHERE id=$1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    let (from, username) = row.ok_or_else(AppError::not_found)?;
    if from == input.rol {
        return Ok(Json(json!({"rol":from,"cambiado":false})));
    }
    sqlx::query("UPDATE usuarios SET rol=$2 WHERE id=$1")
        .bind(id)
        .bind(&input.rol)
        .execute(&mut *tx)
        .await?;
    let detail = format!(
        "Rol cambiado de {} a {}.",
        if from.is_empty() { "sin rol" } else { &from },
        input.rol
    );
    sqlx::query("INSERT INTO log_actividad(usuario_id,identificador,accion,detalle,realizado_por_id,creado_en) VALUES($1,$2,'CAMBIO_ROL',$3,$4,now())")
        .bind(id).bind(&username).bind(&detail).bind(user.id).execute(&mut *tx).await?;
    let audit = history(
        &mut tx,
        ("usuarios", "usuario"),
        id,
        &from,
        &input.rol,
        user.id,
        &detail,
    )
    .await?;
    sqlx::query("UPDATE historial_estados SET campo='rol' WHERE id=$1")
        .bind(audit)
        .execute(&mut *tx)
        .await?;
    // The role decides every access check: drop sessions opened with the old one.
    sqlx::query("DELETE FROM rust_sessions WHERE user_id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({"rol":input.rol,"cambiado":true})))
}

pub async fn show_permissions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<Value>> {
    admin(&state, &headers).await?;
    let mut tx = state.pool.begin().await?;
    let owned = granted(&mut tx, id).await?;
    let catalog: Vec<Value> = PERMISOS_MODULO
        .iter()
        .map(|(code, name, help)| json!({"codigo":code,"nombre":name,"descripcion":help,"otorgado":owned.iter().any(|c| c==code)}))
        .collect();
    Ok(Json(json!({"permisos":catalog})))
}
#[derive(Deserialize)]
pub struct PermissionsInput {
    #[serde(default)]
    codigos: Vec<String>,
}
/// asignar_permisos_modulo: leave the user with exactly the requested permissions.
pub async fn permissions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<PermissionsInput>,
) -> Result<Json<Value>> {
    let user = admin(&state, &headers).await?;
    let mut tx = state.pool.begin().await?;
    let username: Option<String> =
        sqlx::query_scalar("SELECT username FROM usuarios WHERE id=$1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    let username = username.ok_or_else(AppError::not_found)?;
    let owned = granted(&mut tx, id).await?;
    let wanted: Vec<&str> = PERMISOS_MODULO
        .iter()
        .map(|(code, ..)| *code)
        .filter(|code| input.codigos.iter().any(|c| c == code))
        .collect();
    let added: Vec<&str> = wanted
        .iter()
        .copied()
        .filter(|c| !owned.iter().any(|o| o == c))
        .collect();
    let removed: Vec<&str> = owned
        .iter()
        .map(String::as_str)
        .filter(|c| !wanted.contains(c))
        .collect();
    for code in &added {
        let permission = permission_id(&mut tx, code).await?;
        sqlx::query("INSERT INTO usuarios_user_permissions(usuario_id,permission_id) VALUES($1,$2) ON CONFLICT DO NOTHING")
            .bind(id).bind(permission).execute(&mut *tx).await?;
    }
    for code in &removed {
        let (app, codename) = code.split_once('.').unwrap_or_default();
        sqlx::query("DELETE FROM usuarios_user_permissions up USING auth_permission p, django_content_type c WHERE up.usuario_id=$1 AND p.id=up.permission_id AND c.id=p.content_type_id AND c.app_label=$2 AND p.codename=$3")
            .bind(id).bind(app).bind(codename).execute(&mut *tx).await?;
    }
    if !added.is_empty() || !removed.is_empty() {
        let mut parts = Vec::new();
        if !added.is_empty() {
            parts.push(format!("otorgados: {}", added.join(", ")));
        }
        if !removed.is_empty() {
            parts.push(format!("retirados: {}", removed.join(", ")));
        }
        sqlx::query("INSERT INTO log_actividad(usuario_id,identificador,accion,detalle,realizado_por_id,creado_en) VALUES($1,$2,'CAMBIO_PERMISOS',$3,$4,now())")
            .bind(id).bind(&username).bind(format!("Permisos especiales {}.", parts.join("; "))).bind(user.id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(Json(
        json!({"otorgados":added,"retirados":removed,"permisos":wanted}),
    ))
}
/// Django creates permissions in post_migrate; init-db does not, so create on demand.
async fn permission_id(tx: &mut Transaction<'_, Postgres>, code: &str) -> Result<i32> {
    let (app, codename) = code.split_once('.').unwrap_or_default();
    let (_, name, _) = PERMISOS_MODULO
        .iter()
        .find(|(c, ..)| *c == code)
        .ok_or_else(|| AppError::bad("Permiso inválido"))?;
    // Every module permission lives on DocumentoEmpresa (Meta.permissions).
    sqlx::query("INSERT INTO django_content_type(app_label,model) VALUES($1,'documentoempresa') ON CONFLICT (app_label,model) DO NOTHING")
        .bind(app)
        .execute(&mut **tx)
        .await?;
    let content_type: i32 = sqlx::query_scalar(
        "SELECT id FROM django_content_type WHERE app_label=$1 AND model='documentoempresa'",
    )
    .bind(app)
    .fetch_one(&mut **tx)
    .await?;
    let existing: Option<i32> = sqlx::query_scalar(
        "SELECT id FROM auth_permission WHERE content_type_id=$1 AND codename=$2",
    )
    .bind(content_type)
    .bind(codename)
    .fetch_optional(&mut **tx)
    .await?;
    if let Some(id) = existing {
        return Ok(id);
    }
    Ok(sqlx::query_scalar(
        "INSERT INTO auth_permission(name,content_type_id,codename) VALUES($1,$2,$3) RETURNING id",
    )
    .bind(name)
    .bind(content_type)
    .bind(codename)
    .fetch_one(&mut **tx)
    .await?)
}

/// motivos_para_no_operar: an empty list means the company may operate.
pub async fn operating_reasons(
    tx: &mut Transaction<'_, Postgres>,
    company: i64,
) -> Result<Vec<String>> {
    let row: Option<(bool, String, String)> = sqlx::query_as(
        "SELECT participacion_bloqueada,COALESCE(motivo_bloqueo,''),estado_listas_restrictivas FROM empresas WHERE id=$1",
    )
    .bind(company)
    .fetch_optional(&mut **tx)
    .await?;
    let (blocked, reason, lists) = row.ok_or_else(AppError::not_found)?;
    let mut reasons = Vec::new();
    if blocked {
        reasons.push(
            format!("Participación bloqueada por el administrador. {reason}")
                .trim()
                .to_string(),
        );
    }
    if lists == "RECHAZADO" {
        reasons.push("La empresa aparece en listas restrictivas.".into());
    }
    let documents: Vec<String> = sqlx::query_scalar("SELECT t.nombre||': '||CASE WHEN d.id IS NULL OR d.estado='PENDIENTE' THEN 'Falta cargarlo.' WHEN d.estado='RECHAZADO' THEN 'Rechazado: '||COALESCE(NULLIF(d.motivo_rechazo,''),'sin motivo')||'. Debe cargarse de nuevo.' WHEN d.estado='VENCIDO' OR d.fecha_vencimiento<(now() at time zone 'America/Bogota')::date THEN 'Vencido: debe actualizarse.' WHEN d.estado='CARGADO' THEN 'En revisión por el administrador.' ELSE 'pendiente de verificación.' END FROM tipos_documento_legal t JOIN empresas e ON e.id=$1 LEFT JOIN documentos_empresas d ON d.empresa_id=e.id AND d.tipo_documento=t.codigo WHERE t.activo AND t.obligatorio AND (t.aplica_sin_convenio OR NOT e.renuncio_a_convenio) AND NOT (d.estado IS NOT DISTINCT FROM 'VERIFICADO' AND (d.fecha_vencimiento IS NULL OR d.fecha_vencimiento>=(now() at time zone 'America/Bogota')::date)) ORDER BY t.orden,t.nombre")
        .bind(company).fetch_all(&mut **tx).await?;
    reasons.extend(documents);
    Ok(reasons)
}

#[derive(Deserialize)]
pub struct BlockInput {
    bloquear: bool,
    #[serde(default)]
    motivo: String,
}
/// cambiar_bloqueo_participacion (HU00).
pub async fn block(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<BlockInput>,
) -> Result<Json<Value>> {
    let user = admin(&state, &headers).await?;
    let reason = input.motivo.trim();
    if reason.len() > 10000 {
        return Err(AppError::bad("Motivo demasiado largo"));
    }
    let mut tx = state.pool.begin().await?;
    let row: Option<(bool, Option<i64>)> = sqlx::query_as(
        "SELECT participacion_bloqueada,usuario_id FROM empresas WHERE id=$1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let (blocked, owner) = row.ok_or_else(AppError::not_found)?;
    if blocked == input.bloquear {
        return Ok(Json(
            json!({"participacion_bloqueada":blocked,"cambiado":false}),
        ));
    }
    if input.bloquear && reason.is_empty() {
        return Err(AppError::bad("Indica el motivo del bloqueo."));
    }
    sqlx::query("UPDATE empresas SET participacion_bloqueada=$2,motivo_bloqueo=$3 WHERE id=$1")
        .bind(id)
        .bind(input.bloquear)
        .bind(if input.bloquear { reason } else { "" })
        .execute(&mut *tx)
        .await?;
    let (from, to) = if input.bloquear {
        ("HABILITADA", "BLOQUEADA")
    } else {
        ("BLOQUEADA", "HABILITADA")
    };
    let audit = history(
        &mut tx,
        ("empresas", "empresa"),
        id,
        from,
        to,
        user.id,
        reason,
    )
    .await?;
    sqlx::query("UPDATE historial_estados SET campo='participacion_bloqueada' WHERE id=$1")
        .bind(audit)
        .execute(&mut *tx)
        .await?;
    if let Some(owner) = owner {
        let key = format!(
            "empresa:{id}:bloqueo:{}:{}",
            if input.bloquear { "True" } else { "False" },
            chrono::Utc::now().format("%Y%m%d%H%M%S")
        );
        let message = if input.bloquear {
            format!("El administrador bloqueó la participación de tu empresa. Motivo: {reason}")
        } else {
            "El administrador habilitó de nuevo la participación de tu empresa.".into()
        };
        notify(&mut tx, owner, "EMPRESA_PARTICIPACION", &message, &key).await?;
        // Django overrides the catalog type: ERROR when blocking, EXITO when enabling.
        sqlx::query("UPDATE notificaciones SET tipo=$3 WHERE usuario_id=$1 AND clave_dedupe=$2")
            .bind(owner)
            .bind(&key)
            .bind(if input.bloquear { "ERROR" } else { "EXITO" })
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(Json(
        json!({"participacion_bloqueada":input.bloquear,"cambiado":true}),
    ))
}

#[derive(Deserialize)]
pub struct AgreementChoice {
    opcion: String,
}
/// elegir_modo_convenio: the company chooses the framework agreement or none.
pub async fn agreement_choice(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<AgreementChoice>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    let mut tx = state.pool.begin().await?;
    let company: Option<i64> =
        sqlx::query_scalar("SELECT id FROM empresas WHERE usuario_id=$1 FOR UPDATE")
            .bind(user.id)
            .fetch_optional(&mut *tx)
            .await?;
    let company =
        company.ok_or_else(|| AppError::bad("No tienes un perfil de empresa asociado."))?;
    match input.opcion.as_str() {
        "si_convenio" => Ok(Json(json!({"siguiente":"documentos"}))),
        "no_convenio" => {
            sqlx::query("UPDATE empresas SET renuncio_a_convenio=true WHERE id=$1")
                .bind(company)
                .execute(&mut *tx)
                .await?;
            let reasons = operating_reasons(&mut tx, company).await?;
            tx.commit().await?;
            Ok(Json(json!({
                "siguiente": if reasons.is_empty() { "crear_reto" } else { "documentos" },
                "motivos": reasons,
            })))
        }
        _ => Err(AppError::bad(
            "Selecciona una de las dos opciones para continuar.",
        )),
    }
}
