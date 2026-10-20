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
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
pub struct IntegrationInput {
    facultad: String,
    nivel_formacion: String,
    programa_academico: String,
    #[serde(default)]
    ecosistema: String,
    descripcion: String,
    alcance: String,
    entregable_esperado: String,
    // Free schedule notes; the dated sessions travel in `team` (HU03).
    #[serde(default)]
    cronograma_sesiones: String,
    #[serde(flatten)]
    team: crate::team::TeamInput,
}
pub async fn integrate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(reto): Path<i64>,
    Json(input): Json<IntegrationInput>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    if user.rol != "PROFESOR" {
        return Err(AppError::forbidden());
    }
    for (text, max, label) in [
        (&input.facultad, 140, "Facultad"),
        (&input.nivel_formacion, 120, "Nivel"),
        (&input.programa_academico, 160, "Programa"),
        (&input.descripcion, 10000, "Descripción"),
        (&input.alcance, 10000, "Alcance"),
        (&input.entregable_esperado, 180, "Entregable"),
    ] {
        domain::validate_text(text, max, label)?;
    }
    if input.cronograma_sesiones.chars().count() > 10000 {
        return Err(AppError::bad("Notas del cronograma demasiado largas"));
    }
    crate::team::validate(&input.team)?;
    if input.ecosistema.chars().count() > 140 {
        return Err(AppError::bad("Ecosistema demasiado largo"));
    }
    let mut tx = state.pool.begin().await?;
    let eligible:Option<i64>=sqlx::query_scalar("SELECT id FROM retos WHERE id=$1 AND eliminado_en IS NULL AND estado IN ('aprobado','abierto','en_curso','pausado') FOR UPDATE").bind(reto).fetch_optional(&mut *tx).await?;
    if eligible.is_none() {
        return Err(AppError::bad("El reto no admite integraciones académicas"));
    }
    let duplicate: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM integraciones_academicas WHERE reto_id=$1 AND profesor_id=$2)",
    )
    .bind(reto)
    .bind(user.id)
    .fetch_one(&mut *tx)
    .await?;
    if duplicate {
        return Err(AppError::bad("Ya tienes una integración para este reto"));
    }
    let id:i64=sqlx::query_scalar("INSERT INTO integraciones_academicas(reto_id,profesor_id,facultad,nivel_formacion,programa_academico,ecosistema,descripcion,alcance,entregable_esperado,cronograma_sesiones,requerimientos_empresa,requerimientos_internos,espacio_fisico,estado,comentarios_revision,creado_en,actualizado_en) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,'','','','borrador','',now(),now()) RETURNING id")
        .bind(reto).bind(user.id).bind(&input.facultad).bind(&input.nivel_formacion).bind(&input.programa_academico).bind(&input.ecosistema).bind(&input.descripcion).bind(&input.alcance).bind(&input.entregable_esperado).bind(&input.cronograma_sesiones).fetch_one(&mut *tx).await?;
    crate::team::save(&mut tx, id, reto, &input.team).await?;
    history(
        &mut tx,
        ("seguimiento", "integracionacademica"),
        id,
        "",
        "borrador",
        user.id,
        "Integración creada",
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({"id":id,"estado":"borrador"})))
}
pub async fn integration_state(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<StateInput>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    if input.comentario.len() > 4000 {
        return Err(AppError::bad("Comentario demasiado largo"));
    }
    let mut tx = state.pool.begin().await?;
    // Lock the challenge first, matching every operation that touches both records.
    let reto: Option<i64> =
        sqlx::query_scalar("SELECT reto_id FROM integraciones_academicas WHERE id=$1")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    let reto = reto.ok_or_else(AppError::not_found)?;
    let challenge: Option<(String, i64)> = sqlx::query_as(
        "SELECT estado,empresa_id FROM retos WHERE id=$1 AND eliminado_en IS NULL FOR UPDATE",
    )
    .bind(reto)
    .fetch_optional(&mut *tx)
    .await?;
    let (challenge_state, company) = challenge.ok_or_else(AppError::not_found)?;
    let row: Value = sqlx::query_scalar(
        "SELECT to_jsonb(i) FROM integraciones_academicas i WHERE id=$1 FOR UPDATE",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    let owner = row["profesor_id"].as_i64().unwrap_or(0);
    let from = row["estado"].as_str().unwrap_or("");
    let teacher = user.id == owner && user.rol == "PROFESOR";
    let allowed = match input.estado.as_str() {
        "en_revision" | "borrador" | "publicada" => teacher,
        "aprobada" | "rechazada" => user.admin(),
        _ => false,
    };
    if !allowed {
        return Err(AppError::forbidden());
    }
    domain::transition("MAQUINA_INTEGRACION", from, &input.estado)?;
    if input.estado == "en_revision" {
        // Same rule as IntegracionAcademica.campos_faltantes_para_revision.
        let mut faltantes: Vec<&str> = [
            ("facultad", "Facultad"),
            ("programa_academico", "Programa academico"),
            ("nivel_formacion", "Nivel de formacion"),
            ("descripcion", "Descripcion"),
            ("alcance", "Alcance"),
            ("entregable_esperado", "Entregable esperado"),
        ]
        .into_iter()
        .filter(|(field, _)| row[*field].as_str().unwrap_or("").trim().is_empty())
        .map(|(_, label)| label)
        .collect();
        faltantes.extend(crate::team::missing(&mut tx, id).await?);
        if !faltantes.is_empty() {
            return Err(AppError::bad(format!(
                "Completa estos campos antes de enviar: {}.",
                faltantes.join(", ")
            )));
        }
    }
    if input.estado == "rechazada" && input.comentario.trim().is_empty() {
        return Err(AppError::bad("Indica el motivo del rechazo"));
    }
    if input.estado == "publicada" {
        if !["aprobado", "abierto", "en_curso", "pausado"].contains(&challenge_state.as_str()) {
            return Err(AppError::bad("El reto ya no admite publicación"));
        }
        if challenge_state == "aprobado" {
            domain::transition("MAQUINA_RETO", &challenge_state, "abierto")?;
            sqlx::query("UPDATE retos SET estado='abierto',actualizado_en=now() WHERE id=$1")
                .bind(reto)
                .execute(&mut *tx)
                .await?;
            crate::workflows::challenge_history(
                &mut tx,
                reto,
                "aprobado",
                "abierto",
                user.id,
                "Integración publicada",
            )
            .await?;
        }
        let title: String = sqlx::query_scalar("SELECT titulo FROM retos WHERE id=$1")
            .bind(reto)
            .fetch_one(&mut *tx)
            .await?;
        crate::team::notify_students(&mut tx, id, &title).await?;
    }
    sqlx::query("UPDATE integraciones_academicas SET estado=$2,comentarios_revision=$3,fecha_envio_revision=CASE WHEN $2='en_revision' THEN now() ELSE fecha_envio_revision END,fecha_aprobacion=CASE WHEN $2='aprobada' THEN now() ELSE fecha_aprobacion END,actualizado_en=now() WHERE id=$1")
        .bind(id).bind(&input.estado).bind(&input.comentario).execute(&mut *tx).await?;
    history(
        &mut tx,
        ("seguimiento", "integracionacademica"),
        id,
        from,
        &input.estado,
        user.id,
        &input.comentario,
    )
    .await?;
    let message = format!("Integración {id}: {}", input.estado);
    let key = format!("integracion:{id}:{from}->{}", input.estado);
    for recipient in [owner, company] {
        notify(
            &mut tx,
            recipient,
            "INTEGRACION_ESTADO_CAMBIADO",
            &message,
            &key,
        )
        .await?;
    }
    if input.estado == "en_revision" {
        let admins: Vec<i64> = sqlx::query_scalar(
            "SELECT id FROM usuarios WHERE is_active AND (is_superuser OR rol='ADMIN')",
        )
        .fetch_all(&mut *tx)
        .await?;
        for admin in admins {
            notify(
                &mut tx,
                admin,
                "INTEGRACION_ENVIADA_REVISION",
                &message,
                &key,
            )
            .await?;
        }
    }
    tx.commit().await?;
    Ok(Json(json!({"estado":input.estado})))
}
#[derive(Deserialize)]
pub struct GradeInput {
    nota: String,
    #[serde(default)]
    comentario: String,
}
pub async fn grade(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<GradeInput>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    if user.rol != "PROFESOR" {
        return Err(AppError::forbidden());
    }
    let nota = crate::finance::money(&input.nota)?;
    if input.comentario.len() > 10000 {
        return Err(AppError::bad("Comentario demasiado largo"));
    }
    let mut tx = state.pool.begin().await?;
    let record:Option<(i64,i64,String,Decimal)>=sqlx::query_as("SELECT reto_id,estudiante_id,estado,puntaje_maximo FROM entregables WHERE id=$1 FOR UPDATE").bind(id).fetch_optional(&mut *tx).await?;
    let (reto, student, from, max) = record.ok_or_else(AppError::not_found)?;
    let owns:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM integraciones_academicas WHERE reto_id=$1 AND profesor_id=$2 AND estado='publicada')").bind(reto).bind(user.id).fetch_one(&mut *tx).await?;
    if !owns {
        return Err(AppError::forbidden());
    }
    if nota > max {
        return Err(AppError::bad(format!("La nota máxima es {max}")));
    }
    // Rubric scoring needs criteria; never silently substitute a free-form score.
    let rubrics: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM evaluacion_rubrica WHERE reto_id=$1)")
            .bind(reto)
            .fetch_one(&mut *tx)
            .await?;
    if rubrics {
        return Err(AppError::bad(
            "Este reto usa rúbrica. La evaluación por criterios sigue disponible en Django",
        ));
    }
    domain::transition("MAQUINA_ENTREGABLE", &from, "CALIFICADO")?;
    sqlx::query("INSERT INTO evaluaciones(entregable_id,profesor_id,nota,comentario,fecha,actualizado_en) VALUES($1,$2,$3,$4,now(),now()) ON CONFLICT(entregable_id,profesor_id) DO UPDATE SET nota=excluded.nota,comentario=excluded.comentario,actualizado_en=now()")
        .bind(id).bind(user.id).bind(nota).bind(&input.comentario).execute(&mut *tx).await?;
    sqlx::query("UPDATE entregables SET nota=$2,comentario_profesor=$3,estado='CALIFICADO',actualizado_en=now() WHERE id=$1").bind(id).bind(nota).bind(&input.comentario).execute(&mut *tx).await?;
    history(
        &mut tx,
        ("evaluacion", "entregable"),
        id,
        &from,
        "CALIFICADO",
        user.id,
        &input.comentario,
    )
    .await?;
    notify(
        &mut tx,
        student,
        "ENTREGABLE_CALIFICADO",
        &format!("Entregable {id}: nota {nota} de {max}"),
        &format!("entregable:{id}:calificado"),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({"estado":"CALIFICADO","nota":nota.to_string()})))
}
