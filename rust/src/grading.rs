//! Grading (Django apps/evaluacion, HU12): free score or rubric criteria,
//! explicit confirmation to re-grade, and grades hidden until published.
use crate::{
    AppState, auth,
    auth::User,
    domain,
    error::{AppError, Result},
    workflows::{history, notify},
};
use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;

/// Django `_retos_profesor`: retos the professor integrated (any state).
async fn owns(state: &AppState, user: &User, reto: i64) -> Result<()> {
    let ok: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM integraciones_academicas WHERE reto_id=$1 AND profesor_id=$2)",
    )
    .bind(reto)
    .bind(user.id)
    .fetch_one(&state.pool)
    .await?;
    if user.rol == "PROFESOR" && ok {
        Ok(())
    } else {
        Err(AppError::forbidden())
    }
}

#[derive(Deserialize)]
pub struct Criterion {
    nombre: String,
    puntaje_maximo: String,
}
#[derive(Deserialize)]
pub struct RubricInput {
    nombre: String,
    #[serde(default)]
    descripcion: String,
    criterios: Vec<Criterion>,
}

/// POST: a rubric for a reto the professor integrated (crear_rubrica).
pub async fn create_rubric(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(reto): Path<i64>,
    Json(input): Json<RubricInput>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    owns(&state, &user, reto).await?;
    domain::validate_text(&input.nombre, 150, "Nombre")?;
    if input.descripcion.chars().count() > 5000
        || input.criterios.is_empty()
        || input.criterios.len() > 30
    {
        return Err(AppError::bad("Agrega entre 1 y 30 criterios."));
    }
    let mut criteria = vec![];
    for (n, c) in input.criterios.iter().enumerate() {
        let score = crate::finance::money(&c.puntaje_maximo)
            .map_err(|_| AppError::bad(format!("Criterio {}: puntaje inválido", n + 1)))?;
        if c.nombre.trim().is_empty() || c.nombre.chars().count() > 150 || score <= Decimal::ZERO {
            return Err(AppError::bad(format!("Criterio {} inválido", n + 1)));
        }
        criteria.push((c.nombre.trim().to_string(), score));
    }
    let mut tx = state.pool.begin().await?;
    let id: i64 = sqlx::query_scalar("INSERT INTO evaluacion_rubrica(reto_id,nombre,descripcion,activa,creada_por_id,creada_en) VALUES($1,$2,$3,true,$4,now()) RETURNING id")
        .bind(reto).bind(input.nombre.trim()).bind(input.descripcion.trim()).bind(user.id)
        .fetch_one(&mut *tx).await?;
    for (orden, (nombre, score)) in criteria.iter().enumerate() {
        sqlx::query("INSERT INTO evaluacion_criteriorubrica(rubrica_id,nombre,descripcion,puntaje_maximo,orden) VALUES($1,$2,'',$3,$4)")
            .bind(id).bind(nombre).bind(score).bind(orden as i16).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(Json(json!({"id": id})))
}

/// GET: active rubrics (with criteria) for a deliverable's reto.
pub async fn rubrics(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, false).await?;
    let reto: i64 = sqlx::query_scalar("SELECT reto_id FROM entregables WHERE id=$1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(AppError::not_found)?;
    owns(&state, &user, reto).await?;
    let list: Value = sqlx::query_scalar("SELECT coalesce(jsonb_agg(jsonb_build_object('id',r.id,'nombre',r.nombre,'criterios',(SELECT coalesce(jsonb_agg(jsonb_build_object('id',c.id,'nombre',c.nombre,'puntaje_maximo',c.puntaje_maximo) ORDER BY c.orden,c.id),'[]') FROM evaluacion_criteriorubrica c WHERE c.rubrica_id=r.id)) ORDER BY r.nombre),'[]') FROM evaluacion_rubrica r WHERE r.reto_id=$1 AND r.activa")
        .bind(reto).fetch_one(&state.pool).await?;
    Ok(Json(json!({"rubricas": list})))
}

#[derive(Deserialize)]
pub struct GradeInput {
    #[serde(default)]
    nota: String,
    rubrica_id: Option<i64>,
    #[serde(default)]
    criterios: HashMap<String, String>,
    #[serde(default)]
    comentario: String,
    #[serde(default)]
    confirmar_recalificacion: bool,
}

/// POST: grade a deliverable (calificar). The grade stays unpublished.
pub async fn grade(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<GradeInput>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    if input.comentario.len() > 10000 {
        return Err(AppError::bad("Comentario demasiado largo"));
    }
    let mut tx = state.pool.begin().await?;
    let record: Option<(i64, String, Decimal, String, Option<Decimal>)> = sqlx::query_as(
        "SELECT reto_id,estado,puntaje_maximo,titulo,nota FROM entregables WHERE id=$1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let (reto, from, max, title, previous) = record.ok_or_else(AppError::not_found)?;
    owns(&state, &user, reto).await?;
    // HU12: re-grading requires explicit confirmation.
    if from == "CALIFICADO" && !input.confirmar_recalificacion {
        return Err(AppError::bad(format!(
            "\"{title}\" ya tiene nota ({}). Confirma para volver a calificarlo.",
            previous.map(|n| n.to_string()).unwrap_or_default()
        )));
    }
    let invalid = || AppError::bad("La calificacion o los puntajes de la rubrica no son validos.");
    let (nota, scores) = if let Some(rubric) = input.rubrica_id {
        let criteria: Vec<(i64, Decimal)> = sqlx::query_as("SELECT c.id,c.puntaje_maximo FROM evaluacion_criteriorubrica c JOIN evaluacion_rubrica r ON r.id=c.rubrica_id WHERE r.id=$1 AND r.reto_id=$2 AND r.activa ORDER BY c.orden,c.id")
            .bind(rubric).bind(reto).fetch_all(&mut *tx).await?;
        if criteria.is_empty() {
            return Err(invalid());
        }
        let mut scores = vec![];
        for (criterion, top) in criteria {
            let value = input
                .criterios
                .get(&criterion.to_string())
                .ok_or_else(invalid)?;
            let value = crate::finance::money(value).map_err(|_| invalid())?;
            if value > top {
                return Err(invalid());
            }
            scores.push((criterion, value));
        }
        (scores.iter().map(|s| s.1).sum::<Decimal>(), scores)
    } else {
        let nota = crate::finance::money(&input.nota).map_err(|_| invalid())?;
        if nota > max {
            return Err(invalid());
        }
        (nota, vec![])
    };
    if from != "CALIFICADO" {
        domain::transition("MAQUINA_ENTREGABLE", &from, "CALIFICADO")?;
    }
    let evaluation: i64 = sqlx::query_scalar("INSERT INTO evaluaciones(entregable_id,profesor_id,nota,comentario,rubrica_id,fecha,actualizado_en) VALUES($1,$2,$3,$4,$5,now(),now()) ON CONFLICT(entregable_id,profesor_id) DO UPDATE SET nota=excluded.nota,comentario=excluded.comentario,rubrica_id=excluded.rubrica_id,actualizado_en=now() RETURNING id")
        .bind(id).bind(user.id).bind(nota).bind(&input.comentario).bind(input.rubrica_id)
        .fetch_one(&mut *tx).await?;
    sqlx::query("DELETE FROM evaluacion_evaluacioncriterio WHERE evaluacion_id=$1")
        .bind(evaluation)
        .execute(&mut *tx)
        .await?;
    for (criterion, value) in &scores {
        sqlx::query("INSERT INTO evaluacion_evaluacioncriterio(evaluacion_id,criterio_id,puntaje) VALUES($1,$2,$3)")
            .bind(evaluation).bind(criterion).bind(value).execute(&mut *tx).await?;
    }
    // A new or corrected grade stays unpublished until the professor publishes it.
    sqlx::query("UPDATE entregables SET nota=$2,comentario_profesor=$3,estado='CALIFICADO',calificacion_publicada=false,calificacion_publicada_en=NULL,actualizado_en=now() WHERE id=$1")
        .bind(id).bind(nota).bind(&input.comentario).execute(&mut *tx).await?;
    let verb = if from == "CALIFICADO" {
        "Recalificado"
    } else {
        "Calificado"
    };
    history(
        &mut tx,
        ("evaluacion", "entregable"),
        id,
        &from,
        "CALIFICADO",
        user.id,
        &format!("{verb} con {nota} de {max}."),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(
        json!({"estado": "CALIFICADO", "nota": nota.to_string(), "publicada": false}),
    ))
}

/// POST: publish the reto's unpublished grades and notify students (publicar_calificaciones).
pub async fn publish(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(reto): Path<i64>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    owns(&state, &user, reto).await?;
    let mut tx = state.pool.begin().await?;
    let pending: Vec<(i64, i64, String, String, Option<Decimal>, Decimal, Option<String>)> = sqlx::query_as("UPDATE entregables e SET calificacion_publicada=true,calificacion_publicada_en=now(),actualizado_en=now() FROM retos r WHERE r.id=e.reto_id AND e.reto_id=$1 AND e.estado='CALIFICADO' AND NOT e.calificacion_publicada RETURNING e.id,e.estudiante_id,e.titulo,r.titulo,e.nota,e.puntaje_maximo,e.comentario_profesor")
        .bind(reto).fetch_all(&mut *tx).await?;
    let stamp = chrono::Utc::now().format("%Y%m%d%H%M%S");
    for (id, student, title, reto_title, nota, max, feedback) in &pending {
        let feedback = feedback.clone().unwrap_or_default();
        if !feedback.is_empty() {
            sqlx::query("INSERT INTO evaluacion_comentarioentregable(entregable_id,autor_id,comentario,creado_en) VALUES($1,$2,$3,now())")
                .bind(id).bind(user.id).bind(&feedback).execute(&mut *tx).await?;
        }
        let mut message = format!(
            "Tu entregable \"{title}\" del reto \"{reto_title}\" fue calificado con {} de {max}.",
            nota.map(|n| n.to_string()).unwrap_or_default()
        );
        if !feedback.is_empty() {
            message.push_str(&format!("\nRetroalimentacion: {feedback}"));
        }
        notify(
            &mut tx,
            *student,
            "ENTREGABLE_CALIFICADO",
            &message,
            &format!("entregable:{id}:publicado:{stamp}"),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(Json(json!({"publicadas": pending.len()})))
}

/// marcar_en_revision: opening the professor's deliverables moves submitted ones
/// to EN_REVISION, the only way that state is reached (Django panel_profesor).
pub async fn mark_in_review(state: &AppState, user: &crate::auth::User) -> Result<()> {
    if user.rol != "PROFESOR" {
        return Ok(());
    }
    let mut tx = state.pool.begin().await?;
    let opened: Vec<i64> = sqlx::query_scalar("UPDATE entregables SET estado='EN_REVISION',actualizado_en=now() WHERE estado='ENVIADO' AND reto_id IN (SELECT reto_id FROM integraciones_academicas WHERE profesor_id=$1) RETURNING id")
        .bind(user.id).fetch_all(&mut *tx).await?;
    for id in opened {
        crate::workflows::history(
            &mut tx,
            ("evaluacion", "entregable"),
            id,
            "ENVIADO",
            "EN_REVISION",
            user.id,
            "Entregable abierto para revision.",
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}
