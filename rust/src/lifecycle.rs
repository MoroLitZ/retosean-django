use crate::{
    AppState, auth, domain,
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

#[derive(Deserialize)]
pub struct ProgressInput {
    pub fecha_sesion: chrono::NaiveDate,
    pub porcentaje_avance: i32,
    #[serde(default)]
    pub avances: String,
    #[serde(default)]
    pub observaciones: String,
    #[serde(default)]
    pub acuerdos: String,
}
pub async fn progress(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(reto): Path<i64>,
    Json(input): Json<ProgressInput>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    if !(0..=100).contains(&input.porcentaje_avance)
        || input.avances.len() + input.observaciones.len() + input.acuerdos.len() > 20000
    {
        return Err(AppError::bad(
            "Avance inválido; el porcentaje debe estar entre 0 y 100",
        ));
    }
    let mut tx = state.pool.begin().await?;
    let owner:Option<i64>=sqlx::query_scalar("SELECT empresa_id FROM retos WHERE id=$1 AND eliminado_en IS NULL AND estado IN ('abierto','en_curso','pausado') FOR UPDATE").bind(reto).fetch_optional(&mut *tx).await?;
    let owner = owner.ok_or_else(|| AppError::bad("El reto no admite seguimiento"))?;
    let teacher:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM integraciones_academicas WHERE reto_id=$1 AND profesor_id=$2 AND estado='publicada')").bind(reto).bind(user.id).fetch_one(&mut *tx).await?;
    if !user.admin() && !(user.rol == "PROFESOR" && teacher) {
        return Err(AppError::forbidden());
    }
    let id:i64=sqlx::query_scalar("INSERT INTO seguimientos_reto(reto_id,tipo_sesion,fecha_sesion,porcentaje_avance,avances,observaciones,acuerdos,creado_por_id,creado_en) VALUES($1,'seguimiento',$2,$3,$4,$5,$6,$7,now()) RETURNING id")
        .bind(reto).bind(input.fecha_sesion).bind(input.porcentaje_avance as i16).bind(&input.avances).bind(&input.observaciones).bind(&input.acuerdos).bind(user.id).fetch_one(&mut *tx).await?;
    notify(
        &mut tx,
        owner,
        "SEGUIMIENTO_REGISTRADO",
        &format!(
            "Reto {reto}: avance {}%. {}",
            input.porcentaje_avance, input.avances
        ),
        &format!("seguimiento:{id}"),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({"id":id})))
}
#[derive(Deserialize)]
pub struct CloseInput {
    #[serde(default)]
    pub notas: String,
    pub horas: Option<i32>,
}
pub async fn close(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(reto): Path<i64>,
    Json(input): Json<CloseInput>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    if input.notas.len() > 10000 || input.horas.is_some_and(|h| !(0..=10000).contains(&h)) {
        return Err(AppError::bad("Datos de cierre inválidos"));
    }
    let mut tx = state.pool.begin().await?;
    let from: Option<String> = sqlx::query_scalar(
        "SELECT estado FROM retos WHERE id=$1 AND eliminado_en IS NULL FOR UPDATE",
    )
    .bind(reto)
    .fetch_optional(&mut *tx)
    .await?;
    let from = from.ok_or_else(AppError::not_found)?;
    let teacher: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM integraciones_academicas WHERE reto_id=$1 AND profesor_id=$2)",
    )
    .bind(reto)
    .bind(user.id)
    .fetch_one(&mut *tx)
    .await?;
    if !user.admin() && !(user.rol == "PROFESOR" && teacher) {
        return Err(AppError::forbidden());
    }
    if from != "finalizado" {
        domain::transition("MAQUINA_RETO", &from, "finalizado")?;
    }
    let submitted:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM entregables WHERE reto_id=$1 AND archivo<>'') OR EXISTS(SELECT 1 FROM cierre_entregablefinal e JOIN cierres_reto c ON c.id=e.cierre_id WHERE c.reto_id=$1)").bind(reto).fetch_one(&mut *tx).await?;
    if !submitted {
        return Err(AppError::bad(
            "Se necesita al menos un entregable para cerrar el reto",
        ));
    }
    let cierre:i64=sqlx::query_scalar("INSERT INTO cierres_reto(reto_id,notas,encuesta_enviada,cerrado_por_id,cerrado_en,actualizado_en) VALUES($1,$2,true,$3,now(),now()) ON CONFLICT(reto_id) DO UPDATE SET notas=excluded.notas,encuesta_enviada=true,cerrado_por_id=excluded.cerrado_por_id,cerrado_en=COALESCE(cierres_reto.cerrado_en,now()),actualizado_en=now() RETURNING id").bind(reto).bind(&input.notas).bind(user.id).fetch_one(&mut *tx).await?;
    sqlx::query("INSERT INTO cierre_agendacierre(reto_id,fecha_hora,espacio,recursos,invitados,agenda,actualizado_en) VALUES($1,now(),'Plataforma RetosEAN','','','Cierre formal del reto y emisión de certificados',now()) ON CONFLICT(reto_id) DO NOTHING").bind(reto).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO cierre_entregablefinal(cierre_id,nombre,archivo,cargado_por_id,creado_en) SELECT $1,left(e.titulo,180),e.archivo,$2,now() FROM entregables e WHERE e.reto_id=$3 AND e.archivo<>'' AND NOT EXISTS(SELECT 1 FROM cierre_entregablefinal f WHERE f.cierre_id=$1 AND f.archivo=e.archivo)").bind(cierre).bind(user.id).bind(reto).execute(&mut *tx).await?;
    sqlx::query("UPDATE retos SET estado='finalizado',actualizado_en=now() WHERE id=$1")
        .bind(reto)
        .execute(&mut *tx)
        .await?;
    if from != "finalizado" {
        crate::workflows::challenge_history(
            &mut tx,
            reto,
            &from,
            "finalizado",
            user.id,
            &input.notas,
        )
        .await?;
    }
    // cierre._participantes: the company, professors, accepted applicants and team members.
    let participants: Vec<i64> = sqlx::query_scalar("SELECT empresa_id FROM retos WHERE id=$1 UNION SELECT profesor_id FROM integraciones_academicas WHERE reto_id=$1 AND profesor_id IS NOT NULL UNION SELECT estudiante_id FROM postulaciones WHERE reto_id=$1 AND estado='ACEPTADA' UNION SELECT m.estudiante_id FROM miembros_equipo m JOIN equipos e ON e.id=m.equipo_id WHERE e.reto_id=$1")
        .bind(reto).fetch_all(&mut *tx).await?;
    let title: String = sqlx::query_scalar("SELECT titulo FROM retos WHERE id=$1")
        .bind(reto)
        .fetch_one(&mut *tx)
        .await?;
    let key = format!("cierre:{reto}:encuesta");
    for participant in participants {
        sqlx::query("INSERT INTO cierre_encuestasatisfaccion(cierre_id,participante_id,comentario,creada_en) VALUES($1,$2,'',now()) ON CONFLICT(cierre_id,participante_id) DO NOTHING").bind(cierre).bind(participant).execute(&mut *tx).await?;
        notify(
            &mut tx,
            participant,
            "ENCUESTA_PENDIENTE",
            "El reto fue cerrado formalmente. Te invitamos a responder la encuesta de satisfaccion.",
            &key,
        )
        .await?;
    }
    // Django passes its own title for this notice.
    sqlx::query("UPDATE notificaciones SET titulo=$2 WHERE clave_dedupe=$1")
        .bind(&key)
        .bind(format!("Reto finalizado: {title}"))
        .execute(&mut *tx)
        .await?;
    let issued = issue_certificates(&mut tx, reto, user.id, input.horas).await?;
    tx.commit().await?;
    Ok(Json(
        json!({"estado":"finalizado","certificados_emitidos":issued}),
    ))
}
/// emitir_certificados_de_reto: idempotent; only a finished reto, notifies new holders.
pub async fn issue_certificates(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    reto: i64,
    by: i64,
    hours: Option<i32>,
) -> Result<u64> {
    let row: Option<(String, String, i64)> =
        sqlx::query_as("SELECT estado,titulo,empresa_id FROM retos WHERE id=$1")
            .bind(reto)
            .fetch_optional(&mut **tx)
            .await?;
    let Some((_, title, company)) = row.filter(|r| r.0 == "finalizado") else {
        return Ok(0);
    };
    // _participantes_del_reto, in Django's precedence order.
    let mut eligible: std::collections::BTreeMap<i64, &str> = std::collections::BTreeMap::new();
    let accepted: Vec<i64> = sqlx::query_scalar(
        "SELECT estudiante_id FROM postulaciones WHERE reto_id=$1 AND estado='ACEPTADA'",
    )
    .bind(reto)
    .fetch_all(&mut **tx)
    .await?;
    for id in accepted {
        eligible.insert(id, "ESTUDIANTE");
    }
    let students: Vec<i64> = sqlx::query_scalar("SELECT m.usuario_id FROM equipos_reto_academico_estudiantes m JOIN equipos_reto_academico e ON e.id=m.equiporetoacademico_id WHERE e.reto_id=$1 UNION SELECT estudiante_id FROM entregables WHERE reto_id=$1 AND estudiante_id IS NOT NULL")
        .bind(reto).fetch_all(&mut **tx).await?;
    for id in students {
        eligible.entry(id).or_insert("ESTUDIANTE");
    }
    let teachers: Vec<i64> = sqlx::query_scalar(
        "SELECT profesor_id FROM integraciones_academicas WHERE reto_id=$1 AND profesor_id IS NOT NULL",
    )
    .bind(reto)
    .fetch_all(&mut **tx)
    .await?;
    for id in teachers {
        eligible.insert(id, "PROFESOR");
    }
    let jurors: Vec<i64> = sqlx::query_scalar("SELECT j.usuario_id FROM jurados_hackaton j JOIN hackathons h ON h.id=j.hackathon_id WHERE h.reto_id=$1 AND j.usuario_id IS NOT NULL")
        .bind(reto).fetch_all(&mut **tx).await?;
    for id in jurors {
        eligible.entry(id).or_insert("JURADO");
    }
    eligible.entry(company).or_insert("EMPRESA");
    let mut issued = 0u64;
    for (participant, role) in eligible {
        let created = sqlx::query("INSERT INTO certificados(reto_id,usuario_id,rol_participacion,codigo_verificacion,horas,emitido_en,emitido_por_id) VALUES($1,$2,$3,$4,$5,now(),$6) ON CONFLICT(reto_id,usuario_id,rol_participacion) DO NOTHING")
            .bind(reto).bind(participant).bind(role).bind(uuid::Uuid::new_v4()).bind(hours).bind(by).execute(&mut **tx).await?.rows_affected();
        if created > 0 {
            notify(
                tx,
                participant,
                "CERTIFICADO_DISPONIBLE",
                &format!(
                    "Ya puedes descargar tu certificado de participacion en el reto '{title}'."
                ),
                &format!("certificado:{reto}"),
            )
            .await?;
        }
        issued += created;
    }
    Ok(issued)
}
#[derive(Deserialize)]
pub struct SurveyInput {
    calificacion: i32,
    #[serde(default)]
    comentario: String,
}
pub async fn survey(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<SurveyInput>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    if !(1..=5).contains(&input.calificacion) || input.comentario.len() > 10000 {
        return Err(AppError::bad("La calificación debe estar entre 1 y 5"));
    }
    let mut tx = state.pool.begin().await?;
    let changed=sqlx::query("UPDATE cierre_encuestasatisfaccion SET calificacion=$2,comentario=$3,respondida_en=now() WHERE id=$1 AND participante_id=$4 AND respondida_en IS NULL").bind(id).bind(input.calificacion as i16).bind(&input.comentario).bind(user.id).execute(&mut *tx).await?.rows_affected();
    if changed == 0 {
        return Err(AppError::bad("Encuesta no encontrada o ya respondida"));
    }
    history(
        &mut tx,
        ("cierre", "encuestasatisfaccion"),
        id,
        "pendiente",
        "respondida",
        user.id,
        "",
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({"ok":true})))
}
