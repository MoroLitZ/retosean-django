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
    let (owner, from, lists, _waived) = row.ok_or_else(AppError::not_found)?;
    if input.estado_validacion == "VERIFICADA" {
        if input.estado_listas_restrictivas != "APROBADO" {
            return Err(AppError::bad("Verifica primero las listas restrictivas"));
        }
        let missing: Vec<String> = sqlx::query_scalar(MISSING_DOCUMENTS)
            .bind(id)
            .fetch_all(&mut *tx)
            .await?;
        if !missing.is_empty() {
            return Err(AppError::bad(format!(
                "Faltan documentos obligatorios verificados y vigentes: {}",
                missing.join(", ")
            )));
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
            if input.estado_listas_restrictivas == "RECHAZADO" {
                "EMPRESA_LISTAS_RESTRICTIVAS"
            } else {
                "EMPRESA_PARTICIPACION"
            },
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
        let label = crate::dashboard::etiqueta("empresas", "estado_convenio", &input.estado);
        let mut message = format!("El tramite de tu convenio quedo en estado: {label}.");
        let due = if input.estado == "FIRMADO" {
            input.fecha_vencimiento
        } else {
            None
        };
        if let Some(due) = due {
            message.push_str(&format!("\nVigente hasta el {}.", due.format("%d/%m/%Y")));
        }
        if !input.comentario.is_empty() {
            message.push_str(&format!("\nComentario: {}", input.comentario));
        }
        let key = format!(
            "convenio:{id}:{}:{}",
            input.estado,
            due.map(|d| d.to_string()).unwrap_or_default()
        );
        notify(&mut tx, owner, "CONVENIO_ACTUALIZADO", &message, &key).await?;
        sqlx::query("UPDATE notificaciones SET tipo=$3 WHERE usuario_id=$1 AND clave_dedupe=$2")
            .bind(owner)
            .bind(&key)
            .bind(if input.estado == "FIRMADO" {
                "EXITO"
            } else {
                "INFO"
            })
            .execute(&mut *tx)
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
    let row:Option<(String,Option<String>,Option<i64>,bool,i64)>=sqlx::query_as("SELECT d.estado,d.archivo,e.usuario_id,(d.fecha_vencimiento IS NOT NULL AND d.fecha_vencimiento<(now() at time zone 'America/Bogota')::date),e.id FROM documentos_empresas d JOIN empresas e ON e.id=d.empresa_id WHERE d.id=$1 FOR UPDATE OF d").bind(id).fetch_optional(&mut *tx).await?;
    let (from, file, owner, expired, company) = row.ok_or_else(AppError::not_found)?;
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
    recompute_validation(&mut tx, company).await?;
    if let Some(owner) = owner {
        // _notificar_revision_documento.
        let label: String = sqlx::query_scalar("SELECT COALESCE((SELECT t.nombre FROM tipos_documento_legal t JOIN documentos_empresas d ON d.tipo_documento=t.codigo WHERE d.id=$1),(SELECT tipo_documento FROM documentos_empresas WHERE id=$1))")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
        let (message, kind) = if input.estado == "VERIFICADO" {
            (
                format!("Tu documento \"{label}\" fue verificado y aprobado."),
                "EXITO",
            )
        } else {
            let mut message = format!("Tu documento \"{label}\" fue rechazado.");
            if !input.comentario.is_empty() {
                message.push_str(&format!("\nMotivo: {}", input.comentario));
            }
            message.push_str("\nPuedes volver a cargarlo desde tu panel de documentacion.");
            (message, "ERROR")
        };
        let key = format!("documento:{id}:{}:{id}", input.estado);
        notify(&mut tx, owner, "DOCUMENTO_REVISADO", &message, &key).await?;
        sqlx::query("UPDATE notificaciones SET tipo=$3 WHERE usuario_id=$1 AND clave_dedupe=$2")
            .bind(owner)
            .bind(&key)
            .bind(kind)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(Json(json!({"estado":input.estado})))
}

/// Required catalog documents not yet verified and valid (Django documentacion_completa):
/// a type applies when active, mandatory, and either it applies without an agreement
/// or the company did not waive the agreement.
pub const MISSING_DOCUMENTS: &str = "SELECT t.nombre FROM tipos_documento_legal t JOIN empresas e ON e.id=$1 WHERE t.activo AND t.obligatorio AND (t.aplica_sin_convenio OR NOT e.renuncio_a_convenio) AND NOT EXISTS(SELECT 1 FROM documentos_empresas d WHERE d.empresa_id=e.id AND d.tipo_documento=t.codigo AND d.estado='VERIFICADO' AND (d.fecha_vencimiento IS NULL OR d.fecha_vencimiento>=(now() at time zone 'America/Bogota')::date)) ORDER BY t.orden";

/// Keeps estado_validacion consistent with the documents, as Django's
/// _actualizar_estado_validacion: any rejected -> RECHAZADA, all required
/// verified -> VERIFICADA, something uploaded -> EN_REVISION, else PENDIENTE.
/// Rust pre-creates PENDIENTE rows, so those do not count as uploaded.
pub async fn recompute_validation(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    company: i64,
) -> Result<()> {
    sqlx::query(&format!("UPDATE empresas e SET estado_validacion=CASE WHEN EXISTS(SELECT 1 FROM documentos_empresas d WHERE d.empresa_id=e.id AND d.estado='RECHAZADO') THEN 'RECHAZADA' WHEN NOT EXISTS({}) THEN 'VERIFICADA' WHEN EXISTS(SELECT 1 FROM documentos_empresas d WHERE d.empresa_id=e.id AND d.estado<>'PENDIENTE') THEN 'EN_REVISION' ELSE 'PENDIENTE' END WHERE e.id=$1 AND e.estado_listas_restrictivas<>'RECHAZADO'", MISSING_DOCUMENTS))
        .bind(company)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

#[derive(Deserialize)]
pub struct DocumentTypeInput {
    #[serde(default)]
    codigo: String,
    nombre: String,
    #[serde(default)]
    descripcion: String,
    // Checkboxes: absent means unchecked, as an HTML form.
    #[serde(default)]
    obligatorio: bool,
    #[serde(default)]
    aplica_sin_convenio: bool,
    vigencia_dias: Option<i16>,
    #[serde(default)]
    requiere_permiso_especial: bool,
    #[serde(default)]
    activo: bool,
    #[serde(default)]
    orden: i16,
}
/// editar_tipo_documento: the admin-configurable legal catalog (HU00).
pub async fn document_type(
    State(state): State<AppState>,
    headers: HeaderMap,
    id: Option<Path<i64>>,
    Json(input): Json<DocumentTypeInput>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    if !user.admin() {
        return Err(AppError::forbidden());
    }
    domain::validate_text(&input.nombre, 150, "Nombre")?;
    if input.descripcion.len() > 4000
        || input.vigencia_dias.is_some_and(|d| d < 0)
        || input.orden < 0
    {
        return Err(AppError::bad("Revisa los datos del tipo de documento"));
    }
    let id = match id {
        Some(Path(id)) => {
            let updated: Option<i64> = sqlx::query_scalar("UPDATE tipos_documento_legal SET nombre=$2,descripcion=$3,obligatorio=$4,aplica_sin_convenio=$5,vigencia_dias=$6,requiere_permiso_especial=$7,activo=$8,orden=$9 WHERE id=$1 RETURNING id")
                .bind(id).bind(input.nombre.trim()).bind(&input.descripcion).bind(input.obligatorio).bind(input.aplica_sin_convenio).bind(input.vigencia_dias).bind(input.requiere_permiso_especial).bind(input.activo).bind(input.orden)
                .fetch_optional(&state.pool).await?;
            updated.ok_or_else(AppError::not_found)?
        }
        None => {
            let code = input.codigo.trim().to_uppercase().replace('-', "_");
            if code.is_empty()
                || code.len() > 50
                || !code.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
            {
                return Err(AppError::bad(
                    "Código inválido: letras, números y guion bajo.",
                ));
            }
            let inserted: Option<i64> = sqlx::query_scalar("INSERT INTO tipos_documento_legal(codigo,nombre,descripcion,obligatorio,aplica_sin_convenio,vigencia_dias,requiere_permiso_especial,activo,orden) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT (codigo) DO NOTHING RETURNING id")
                .bind(&code).bind(input.nombre.trim()).bind(&input.descripcion).bind(input.obligatorio).bind(input.aplica_sin_convenio).bind(input.vigencia_dias).bind(input.requiere_permiso_especial).bind(input.activo).bind(input.orden)
                .fetch_optional(&state.pool).await?;
            inserted.ok_or_else(|| AppError::bad("Ya existe un tipo con ese código."))?
        }
    };
    Ok(Json(
        json!({"id":id,"mensaje":format!("Tipo de documento \"{}\" guardado.", input.nombre.trim())}),
    ))
}
pub async fn create_document_type(
    state: State<AppState>,
    headers: HeaderMap,
    input: Json<DocumentTypeInput>,
) -> Result<Json<Value>> {
    document_type(state, headers, None, input).await
}
pub async fn edit_document_type(
    state: State<AppState>,
    headers: HeaderMap,
    id: Path<i64>,
    input: Json<DocumentTypeInput>,
) -> Result<Json<Value>> {
    document_type(state, headers, Some(id), input).await
}
/// historial_documentos: the company's document state changes, newest first;
/// automatic changes show as "Sistema". For the company and admins.
pub async fn document_history(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(company): Path<i64>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, false).await?;
    let owner: Option<Option<i64>> =
        sqlx::query_scalar("SELECT usuario_id FROM empresas WHERE id=$1")
            .bind(company)
            .fetch_optional(&state.pool)
            .await?;
    let owner = owner.ok_or_else(AppError::not_found)?;
    if !user.admin() && owner != Some(user.id) {
        return Err(AppError::not_found());
    }
    let rows: Value = sqlx::query_scalar("SELECT COALESCE(jsonb_agg(r ORDER BY r.fecha DESC, r.id DESC),'[]') FROM (SELECT h.id,h.fecha,h.estado_anterior,h.estado_nuevo,h.comentario,COALESCE(t.nombre,d.tipo_documento) documento,COALESCE(u.username,'Sistema') realizado_por FROM historial_estados h JOIN django_content_type c ON c.id=h.content_type_id AND c.app_label='empresas' AND c.model='documentoempresa' JOIN documentos_empresas d ON d.id=h.object_id LEFT JOIN tipos_documento_legal t ON t.codigo=d.tipo_documento LEFT JOIN usuarios u ON u.id=h.realizado_por_id WHERE d.empresa_id=$1 ORDER BY h.fecha DESC,h.id DESC LIMIT 30) r")
        .bind(company)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(json!({"historial": rows})))
}
/// seguimiento_avance (HU14): the company's read-only view of a reto's progress.
pub async fn progress_panel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(reto): Path<i64>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, false).await?;
    if user.rol != "EMPRESA" {
        return Err(AppError::forbidden());
    }
    let visible: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM retos WHERE id=$1 AND empresa_id=$2 AND eliminado_en IS NULL AND estado IN ('aprobado','abierto','en_curso','pausado','finalizado'))")
        .bind(reto)
        .bind(user.id)
        .fetch_one(&state.pool)
        .await?;
    if !visible {
        return Err(AppError::not_found());
    }
    let data: Value = sqlx::query_scalar("SELECT jsonb_build_object('porcentaje',COALESCE((SELECT porcentaje_avance FROM seguimientos_reto WHERE reto_id=$1 ORDER BY fecha_sesion DESC,creado_en DESC LIMIT 1),0),'seguimientos',(SELECT COALESCE(jsonb_agg(jsonb_build_object('id',s.id,'tipo_sesion',s.tipo_sesion,'fecha_sesion',s.fecha_sesion,'porcentaje_avance',s.porcentaje_avance,'avances',s.avances,'observaciones',s.observaciones,'acuerdos',s.acuerdos,'archivos',(SELECT COALESCE(jsonb_agg(jsonb_build_object('id',a.id,'nombre',a.nombre_original)),'[]') FROM seguimiento_seguimientoarchivo a WHERE a.seguimiento_id=s.id)) ORDER BY s.fecha_sesion DESC,s.creado_en DESC),'[]') FROM seguimientos_reto s WHERE s.reto_id=$1),'entregables',(SELECT COALESCE(jsonb_agg(jsonb_build_object('id',e.id,'titulo',e.titulo,'estado',e.estado,'fecha_entrega',e.fecha_entrega,'estudiante',COALESCE(NULLIF(trim(u.first_name||' '||u.last_name),''),u.username),'comentarios',(SELECT COALESCE(jsonb_agg(jsonb_build_object('autor',COALESCE(a.username,''),'comentario',c.comentario,'creado_en',c.creado_en) ORDER BY c.creado_en),'[]') FROM evaluacion_comentarioentregable c LEFT JOIN usuarios a ON a.id=c.autor_id WHERE c.entregable_id=e.id)) ORDER BY e.fecha_entrega DESC),'[]') FROM entregables e JOIN usuarios u ON u.id=e.estudiante_id WHERE e.reto_id=$1))")
        .bind(reto)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(data))
}
#[derive(Deserialize)]
pub struct CommentInput {
    comentario: String,
}
/// evaluacion:comentario: the student, an integrated professor or the owning
/// company comment a deliverable; a company comment notifies the team and professors.
pub async fn comment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<CommentInput>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    domain::validate_text(&input.comentario, 5000, "Comentario")?;
    let mut tx = state.pool.begin().await?;
    let row: Option<(i64, i64, i64, String, String)> = sqlx::query_as("SELECT e.reto_id,e.estudiante_id,r.empresa_id,e.titulo,r.titulo FROM entregables e JOIN retos r ON r.id=e.reto_id WHERE e.id=$1")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
    let (reto, student, owner, title, reto_title) = row.ok_or_else(AppError::not_found)?;
    let teacher: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM integraciones_academicas WHERE reto_id=$1 AND profesor_id=$2)",
    )
    .bind(reto)
    .bind(user.id)
    .fetch_one(&mut *tx)
    .await?;
    let allowed = (user.rol == "ESTUDIANTE" && student == user.id)
        || (user.rol == "PROFESOR" && teacher)
        || (user.rol == "EMPRESA" && owner == user.id);
    if !allowed {
        return Err(AppError::forbidden());
    }
    let comment: i64 = sqlx::query_scalar("INSERT INTO evaluacion_comentarioentregable(entregable_id,autor_id,comentario,creado_en) VALUES($1,$2,$3,now()) RETURNING id")
        .bind(id)
        .bind(user.id)
        .bind(input.comentario.trim())
        .fetch_one(&mut *tx)
        .await?;
    if user.rol == "EMPRESA" {
        let mut recipients: Vec<i64> = sqlx::query_scalar(
            "SELECT DISTINCT profesor_id FROM integraciones_academicas WHERE reto_id=$1",
        )
        .bind(reto)
        .fetch_all(&mut *tx)
        .await?;
        recipients.push(student);
        recipients.sort_unstable();
        recipients.dedup();
        let message = format!(
            "La empresa comentó el entregable \"{title}\" del reto \"{reto_title}\": {}",
            input.comentario.trim()
        );
        for recipient in recipients {
            notify(
                &mut tx,
                recipient,
                "COMENTARIO_EMPRESA",
                &message,
                &format!("comentario-empresa:{comment}"),
            )
            .await?;
        }
    }
    tx.commit().await?;
    Ok(Json(
        json!({"id":comment,"mensaje":"Comentario agregado al historial."}),
    ))
}
