use crate::{
    AppState, auth, domain,
    error::{AppError, Result},
    workflows::{StateInput, history, notify},
};
use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
pub struct CompanyReview {
    estado_validacion: String,
    estado_listas_restrictivas: String,
    #[serde(default)]
    observaciones: String,
}
pub async fn review(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<CompanyReview>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    if !user.admin() {
        return Err(AppError::forbidden());
    }
    if !["PENDIENTE", "EN_REVISION", "VERIFICADA", "RECHAZADA"]
        .contains(&input.estado_validacion.as_str())
        || !["PENDIENTE", "APROBADO", "RECHAZADO"]
            .contains(&input.estado_listas_restrictivas.as_str())
        || input.observaciones.len() > 10000
    {
        return Err(AppError::bad("Revisión inválida"));
    }
    let mut tx = state.pool.begin().await?;
    let row:Option<(Option<i64>,String,String,bool)>=sqlx::query_as("SELECT usuario_id,estado_validacion,estado_listas_restrictivas,renuncio_a_convenio FROM empresas WHERE id=$1 FOR UPDATE").bind(id).fetch_optional(&mut *tx).await?;
    let (owner, from, lists, waived) = row.ok_or_else(AppError::not_found)?;
    if input.estado_validacion == "VERIFICADA" {
        if input.estado_listas_restrictivas != "APROBADO" {
            return Err(AppError::bad("Verifica primero las listas restrictivas"));
        }
        let verified:i64=sqlx::query_scalar("SELECT count(DISTINCT tipo_documento) FROM documentos_empresas WHERE empresa_id=$1 AND tipo_documento IN ('RUT','CAMARA_COMERCIO') AND estado='VERIFICADO' AND (fecha_vencimiento IS NULL OR fecha_vencimiento>=(now() at time zone 'America/Bogota')::date)").bind(id).fetch_one(&mut *tx).await?;
        if !waived && verified != 2 {
            return Err(AppError::bad(
                "Verifica el RUT y la Cámara de Comercio vigentes",
            ));
        }
    }
    if input.estado_listas_restrictivas == "RECHAZADO" {
        let documents:Vec<(i64,String)>=sqlx::query_as("SELECT id,estado FROM documentos_empresas WHERE empresa_id=$1 AND estado NOT IN ('VERIFICADO','RECHAZADO') FOR UPDATE").bind(id).fetch_all(&mut *tx).await?;
        for (doc, previous) in documents {
            domain::transition("MAQUINA_DOCUMENTO", &previous, "RECHAZADO")?;
            sqlx::query("UPDATE documentos_empresas SET estado='RECHAZADO',motivo_rechazo='Rechazado por listas restrictivas' WHERE id=$1").bind(doc).execute(&mut *tx).await?;
            history(
                &mut tx,
                ("empresas", "documentoempresa"),
                doc,
                &previous,
                "RECHAZADO",
                user.id,
                "Listas restrictivas",
            )
            .await?;
        }
    }
    sqlx::query("UPDATE empresas SET estado_validacion=$2,estado_listas_restrictivas=$3,observaciones_admin=$4,fecha_ultima_actualizacion=now() WHERE id=$1").bind(id).bind(&input.estado_validacion).bind(&input.estado_listas_restrictivas).bind(&input.observaciones).execute(&mut *tx).await?;
    let audit = history(
        &mut tx,
        ("empresas", "empresa"),
        id,
        &from,
        &input.estado_validacion,
        user.id,
        &input.observaciones,
    )
    .await?;
    sqlx::query("UPDATE historial_estados SET campo='estado_validacion' WHERE id=$1")
        .bind(audit)
        .execute(&mut *tx)
        .await?;
    // Record both axes rather than silently overwriting the screening decision.
    if lists != input.estado_listas_restrictivas {
        let audit = history(
            &mut tx,
            ("empresas", "empresa"),
            id,
            &lists,
            &input.estado_listas_restrictivas,
            user.id,
            "Listas restrictivas",
        )
        .await?;
        sqlx::query("UPDATE historial_estados SET campo='estado_listas_restrictivas' WHERE id=$1")
            .bind(audit)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(owner) = owner {
        notify(
            &mut tx,
            owner,
            "EMPRESA_REVISADA",
            &format!(
                "Empresa: {}. {}",
                input.estado_validacion, input.observaciones
            ),
            &format!("empresa:{id}:{}", uuid::Uuid::new_v4().simple()),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(Json(json!({"ok":true})))
}
#[derive(Deserialize)]
pub struct AgreementInput {
    estado: String,
    fecha_vencimiento: Option<chrono::NaiveDate>,
    #[serde(default)]
    comentario: String,
}
pub async fn agreement(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<AgreementInput>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    if !user.admin() {
        return Err(AppError::forbidden());
    }
    if input.comentario.len() > 10000
        || (input.estado == "FIRMADO" && input.fecha_vencimiento.is_none())
    {
        return Err(AppError::bad("Para firmar, indica la fecha de vencimiento"));
    }
    let mut tx = state.pool.begin().await?;
    let row: Option<(String, Option<i64>)> =
        sqlx::query_as("SELECT estado_convenio,usuario_id FROM empresas WHERE id=$1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    let (from, owner) = row.ok_or_else(AppError::not_found)?;
    domain::transition("MAQUINA_CONVENIO", &from, &input.estado)?;
    sqlx::query("UPDATE empresas SET estado_convenio=$2,fecha_vencimiento_convenio=$3,fecha_ultima_actualizacion=now() WHERE id=$1").bind(id).bind(&input.estado).bind(if input.estado=="FIRMADO"{input.fecha_vencimiento}else{None}).execute(&mut *tx).await?;
    let audit = history(
        &mut tx,
        ("empresas", "empresa"),
        id,
        &from,
        &input.estado,
        user.id,
        &input.comentario,
    )
    .await?;
    sqlx::query("UPDATE historial_estados SET campo='estado_convenio' WHERE id=$1")
        .bind(audit)
        .execute(&mut *tx)
        .await?;
    if let Some(owner) = owner {
        notify(
            &mut tx,
            owner,
            "CONVENIO_ESTADO_CAMBIADO",
            &format!("Convenio: {}", input.estado),
            &format!("convenio:{id}:{from}->{}", input.estado),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(Json(json!({"estado":input.estado})))
}
pub async fn document(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<StateInput>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    if !user.admin() {
        return Err(AppError::forbidden());
    }
    if !["VERIFICADO", "RECHAZADO"].contains(&input.estado.as_str())
        || input.comentario.len() > 4000
        || (input.estado == "RECHAZADO" && input.comentario.trim().is_empty())
    {
        return Err(AppError::bad("Indica una decisión y el motivo del rechazo"));
    }
    let mut tx = state.pool.begin().await?;
    let row:Option<(String,Option<String>,Option<i64>,bool)>=sqlx::query_as("SELECT d.estado,d.archivo,e.usuario_id,(d.fecha_vencimiento IS NOT NULL AND d.fecha_vencimiento<(now() at time zone 'America/Bogota')::date) FROM documentos_empresas d JOIN empresas e ON e.id=d.empresa_id WHERE d.id=$1 FOR UPDATE OF d").bind(id).fetch_optional(&mut *tx).await?;
    let (from, file, owner, expired) = row.ok_or_else(AppError::not_found)?;
    domain::transition("MAQUINA_DOCUMENTO", &from, &input.estado)?;
    if input.estado == "VERIFICADO" && (expired || file.as_deref().unwrap_or("").is_empty()) {
        return Err(AppError::bad(
            "No puedes verificar un documento vencido o sin archivo",
        ));
    }
    sqlx::query("UPDATE documentos_empresas SET estado=$2,motivo_rechazo=$3 WHERE id=$1")
        .bind(id)
        .bind(&input.estado)
        .bind(if input.estado == "RECHAZADO" {
            &input.comentario
        } else {
            ""
        })
        .execute(&mut *tx)
        .await?;
    history(
        &mut tx,
        ("empresas", "documentoempresa"),
        id,
        &from,
        &input.estado,
        user.id,
        &input.comentario,
    )
    .await?;
    if let Some(owner) = owner {
        notify(
            &mut tx,
            owner,
            "DOCUMENTO_REVISADO",
            &format!("Documento {id}: {}. {}", input.estado, input.comentario),
            &format!("documento:{id}:{}", uuid::Uuid::new_v4().simple()),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(Json(json!({"estado":input.estado})))
}
