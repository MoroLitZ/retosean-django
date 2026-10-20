use crate::{
    AppState, auth, domain,
    error::{AppError, Result},
};
use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};

pub async fn notify(
    tx: &mut Transaction<'_, Postgres>,
    user: i64,
    event: &str,
    message: &str,
    key: &str,
) -> Result<()> {
    // Title and type come from Django's event catalog; the outbox emails it later.
    let (tipo, titulo, _) = crate::mail::event(event);
    sqlx::query("INSERT INTO notificaciones(usuario_id,tipo,evento,titulo,mensaje,leida,link,clave_dedupe,enviada_por_correo,error_envio,creada_en) SELECT $1,$5,$2,$6,$3,false,'/',$4,false,'',now() WHERE NOT EXISTS(SELECT 1 FROM preferencias_notificacion WHERE usuario_id=$1 AND eventos_silenciados ? $2) ON CONFLICT (usuario_id,clave_dedupe) WHERE clave_dedupe<>'' DO NOTHING")
        .bind(user).bind(event).bind(message).bind(key).bind(tipo).bind(titulo).execute(&mut **tx).await?;
    Ok(())
}
/// Reto state changes go to the shared audit table, like every other entity (Django).
pub async fn challenge_history(
    tx: &mut Transaction<'_, Postgres>,
    id: i64,
    from: &str,
    to: &str,
    user: i64,
    comment: &str,
) -> Result<()> {
    history(tx, ("retos", "reto"), id, from, to, user, comment).await?;
    Ok(())
}
pub async fn history(
    tx: &mut Transaction<'_, Postgres>,
    entity: (&str, &str),
    id: i64,
    from: &str,
    to: &str,
    user: i64,
    comment: &str,
) -> Result<i64> {
    let (app, model) = entity;
    let ct: i32 = sqlx::query_scalar("INSERT INTO django_content_type(app_label,model) VALUES($1,$2) ON CONFLICT(app_label,model) DO UPDATE SET model=excluded.model RETURNING id").bind(app).bind(model).fetch_one(&mut **tx).await?;
    let audit = sqlx::query_scalar("INSERT INTO historial_estados(content_type_id,object_id,campo,estado_anterior,estado_nuevo,realizado_por_id,comentario,fecha) VALUES($1,$2,'estado',$3,$4,$5,$6,now()) RETURNING id").bind(ct).bind(id).bind(from).bind(to).bind((user > 0).then_some(user)).bind(comment).fetch_one(&mut **tx).await?;
    Ok(audit)
}
#[derive(Deserialize)]
pub struct ChallengeInput {
    pub titulo: String,
    pub descripcion: String,
    #[serde(default)]
    pub area: String,
    #[serde(default = "default_kind")]
    pub tipo: String,
    #[serde(default)]
    pub nivel_academico: String,
    pub fecha_inicio_tentativa: Option<NaiveDate>,
    pub fecha_fin_tentativa: Option<NaiveDate>,
    pub fecha_limite_postulacion: Option<NaiveDate>,
}
fn default_kind() -> String {
    "reto".into()
}
impl ChallengeInput {
    fn validate(&self) -> Result<()> {
        domain::validate_text(&self.titulo, 180, "Título")?;
        domain::validate_text(&self.descripcion, 20000, "Descripción")?;
        if !["reto", "hackathon"].contains(&self.tipo.as_str()) {
            return Err(AppError::bad("Tipo inválido"));
        }
        if self.area.chars().count() > 120 || self.nivel_academico.chars().count() > 120 {
            return Err(AppError::bad("Área o nivel demasiado largo"));
        }
        if self
            .fecha_inicio_tentativa
            .zip(self.fecha_fin_tentativa)
            .is_some_and(|(a, b)| a > b)
            || self
                .fecha_limite_postulacion
                .zip(self.fecha_fin_tentativa)
                .is_some_and(|(a, b)| a > b)
        {
            return Err(AppError::bad("Revisa el orden de las fechas"));
        }
        Ok(())
    }
}
pub async fn create_challenge(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ChallengeInput>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    if user.rol != "EMPRESA" {
        return Err(AppError::forbidden());
    }
    input.validate()?;
    let mut tx = state.pool.begin().await?;
    let verified: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM empresas WHERE usuario_id=$1 AND estado_validacion='VERIFICADA' AND estado_listas_restrictivas='APROBADO')").bind(user.id).fetch_one(&mut *tx).await?;
    if !verified {
        return Err(AppError::bad(
            "La empresa debe estar verificada y aprobada en listas restrictivas",
        ));
    }
    let id: i64 = sqlx::query_scalar("INSERT INTO retos(empresa_id,tipo,titulo,descripcion,area,nivel_academico,fecha_inicio_tentativa,fecha_fin_tentativa,fecha_limite_postulacion,premios,criterios_evaluacion,estado,comentarios_revision,creado_en,actualizado_en,tiene_convenio_institucional) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,'','','borrador','',now(),now(),false) RETURNING id")
        .bind(user.id).bind(&input.tipo).bind(input.titulo.trim()).bind(&input.descripcion).bind(&input.area).bind(&input.nivel_academico).bind(input.fecha_inicio_tentativa).bind(input.fecha_fin_tentativa).bind(input.fecha_limite_postulacion).fetch_one(&mut *tx).await?;
    challenge_history(
        &mut tx,
        id,
        "",
        "borrador",
        user.id,
        "Reto creado desde Rust",
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({"id":id,"estado":"borrador"})))
}
pub async fn edit_challenge(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<ChallengeInput>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    input.validate()?;
    let mut tx = state.pool.begin().await?;
    let record: Option<(i64, String)> = sqlx::query_as(
        "SELECT empresa_id,estado FROM retos WHERE id=$1 AND eliminado_en IS NULL FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let (owner, previous) = record.ok_or_else(AppError::not_found)?;
    if user.id != owner || user.rol != "EMPRESA" {
        return Err(AppError::forbidden());
    }
    if !["borrador", "rechazado"].contains(&previous.as_str()) {
        return Err(AppError::bad(
            "Solo se editan borradores o retos rechazados",
        ));
    }
    sqlx::query("UPDATE retos SET titulo=$2,descripcion=$3,area=$4,nivel_academico=$5,fecha_inicio_tentativa=$6,fecha_fin_tentativa=$7,fecha_limite_postulacion=$8,estado='borrador',comentarios_revision='',actualizado_en=now() WHERE id=$1")
        .bind(id).bind(input.titulo.trim()).bind(&input.descripcion).bind(&input.area).bind(&input.nivel_academico).bind(input.fecha_inicio_tentativa).bind(input.fecha_fin_tentativa).bind(input.fecha_limite_postulacion).execute(&mut *tx).await?;
    if previous != "borrador" {
        challenge_history(
            &mut tx,
            id,
            &previous,
            "borrador",
            user.id,
            "Reto corregido",
        )
        .await?;
    }
    tx.commit().await?;
    Ok(Json(json!({"id":id,"estado":"borrador"})))
}
#[derive(Deserialize)]
pub struct StateInput {
    pub estado: String,
    #[serde(default)]
    pub comentario: String,
}
pub async fn challenge_state(
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
    let record: Option<Value> = sqlx::query_scalar(
        "SELECT to_jsonb(r) FROM retos r WHERE id=$1 AND eliminado_en IS NULL FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let record = record.ok_or_else(AppError::not_found)?;
    let from = record["estado"].as_str().unwrap_or("");
    let owner = record["empresa_id"]
        .as_i64()
        .ok_or_else(AppError::not_found)?;
    let company_action = user.id == owner && user.rol == "EMPRESA" && input.estado == "en_revision";
    let admin_action = user.admin()
        && ["aprobado", "rechazado", "en_curso", "pausado", "cancelado"]
            .contains(&input.estado.as_str());
    if !company_action && !admin_action {
        return Err(AppError::forbidden());
    }
    domain::transition("MAQUINA_RETO", from, &input.estado)?;
    if company_action {
        for name in [
            "titulo",
            "descripcion",
            "area",
            "fecha_inicio_tentativa",
            "fecha_fin_tentativa",
            "fecha_limite_postulacion",
        ] {
            if record[name].is_null() || record[name].as_str().is_some_and(|s| s.trim().is_empty())
            {
                return Err(AppError::bad(format!("Completa el campo {name}")));
            }
        }
    }
    if input.estado == "rechazado" && input.comentario.trim().is_empty() {
        return Err(AppError::bad("Indica el motivo del rechazo"));
    }
    if input.estado == "aprobado" {
        // Serialize number allocation across every native approval, not only this row.
        sqlx::query("SELECT pg_advisory_xact_lock(714032026)")
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE retos SET consecutivo=COALESCE(consecutivo,(SELECT COALESCE(MAX(consecutivo),0)+1 FROM retos WHERE tipo=$2 AND anio_consecutivo=extract(year from now() at time zone 'America/Bogota')::int)),anio_consecutivo=COALESCE(anio_consecutivo,extract(year from now() at time zone 'America/Bogota')::int),fecha_aprobacion=now() WHERE id=$1")
            .bind(id).bind(record["tipo"].as_str().unwrap_or("reto")).execute(&mut *tx).await?;
    }
    sqlx::query("UPDATE retos SET estado=$2,actualizado_en=now(),fecha_envio_revision=CASE WHEN $2='en_revision' THEN now() ELSE fecha_envio_revision END,comentarios_revision=CASE WHEN $2 IN ('aprobado','rechazado') THEN $3 ELSE comentarios_revision END WHERE id=$1")
        .bind(id).bind(&input.estado).bind(&input.comentario).execute(&mut *tx).await?;
    challenge_history(&mut tx, id, from, &input.estado, user.id, &input.comentario).await?;
    let message = format!(
        "El reto {} pasó a {}. {}",
        record["titulo"].as_str().unwrap_or(""),
        input.estado,
        input.comentario
    );
    let key = format!("reto:{id}:{from}->{}", input.estado);
    if company_action {
        let admins: Vec<i64> = sqlx::query_scalar(
            "SELECT id FROM usuarios WHERE is_active AND (is_superuser OR rol='ADMIN')",
        )
        .fetch_all(&mut *tx)
        .await?;
        for admin in admins {
            notify(&mut tx, admin, "RETO_ENVIADO_REVISION", &message, &key).await?;
        }
    } else {
        notify(&mut tx, owner, "RETO_ESTADO_CAMBIADO", &message, &key).await?;
    }
    tx.commit().await?;
    Ok(Json(json!({"id":id,"estado":input.estado})))
}
pub async fn favorite(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    if user.rol != "ESTUDIANTE" {
        return Err(AppError::forbidden());
    }
    crate::resources::visible_record(&state, &user, "retos", id).await?;
    let mut tx = state.pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(format!("favorite:{}:{id}", user.id))
        .execute(&mut *tx)
        .await?;
    let deleted = sqlx::query("DELETE FROM retos_favoritos WHERE usuario_id=$1 AND reto_id=$2")
        .bind(user.id)
        .bind(id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if deleted == 0 {
        sqlx::query("INSERT INTO retos_favoritos(usuario_id,reto_id,fecha) VALUES($1,$2,now())")
            .bind(user.id)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(Json(json!({"favorito":deleted==0})))
}
#[derive(Deserialize)]
pub struct ApplicationInput {
    motivacion: String,
    #[serde(default)]
    habilidades: String,
    semestre: Option<i32>,
    #[serde(default)]
    programa: String,
}
pub async fn apply(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<ApplicationInput>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    if user.rol != "ESTUDIANTE" {
        return Err(AppError::forbidden());
    }
    domain::validate_text(&input.motivacion, 5000, "Motivación")?;
    if input.habilidades.len() > 5000
        || input.programa.chars().count() > 150
        || input.semestre.is_some_and(|v| !(1..=20).contains(&v))
    {
        return Err(AppError::bad("Datos de postulación inválidos"));
    }
    let mut tx = state.pool.begin().await?;
    let owner: Option<i64> = sqlx::query_scalar("SELECT empresa_id FROM retos WHERE id=$1 AND estado='abierto' AND eliminado_en IS NULL AND (fecha_limite_postulacion IS NULL OR fecha_limite_postulacion >= (now() at time zone 'America/Bogota')::date) FOR UPDATE").bind(id).fetch_optional(&mut *tx).await?;
    let owner = owner
        .ok_or_else(|| AppError::bad("El reto no está abierto o venció la fecha de postulación"))?;
    let previous: Option<(i64, String)> = sqlx::query_as(
        "SELECT id,estado FROM postulaciones WHERE reto_id=$1 AND estudiante_id=$2 FOR UPDATE",
    )
    .bind(id)
    .bind(user.id)
    .fetch_optional(&mut *tx)
    .await?;
    if previous
        .as_ref()
        .is_some_and(|(_, s)| ["ACEPTADA", "RECHAZADA"].contains(&s.as_str()))
    {
        return Err(AppError::bad(
            "Una postulación resuelta no se puede reenviar",
        ));
    }
    let application: i64 = sqlx::query_scalar("INSERT INTO postulaciones(reto_id,estudiante_id,fecha_postulacion,estado,motivacion,habilidades,semestre,programa) VALUES($1,$2,now(),'PENDIENTE',$3,$4,$5,$6) ON CONFLICT(reto_id,estudiante_id) DO UPDATE SET estado='PENDIENTE',motivacion=excluded.motivacion,habilidades=excluded.habilidades,semestre=excluded.semestre,programa=excluded.programa RETURNING id")
        .bind(id).bind(user.id).bind(&input.motivacion).bind(&input.habilidades).bind(input.semestre).bind(&input.programa).fetch_one(&mut *tx).await?;
    history(
        &mut tx,
        ("participaciones", "postulacion"),
        application,
        previous.as_ref().map(|(_, s)| s.as_str()).unwrap_or(""),
        "PENDIENTE",
        user.id,
        "Postulación registrada",
    )
    .await?;
    notify(
        &mut tx,
        owner,
        "NUEVA_POSTULACION",
        &format!("{} se postuló al reto {id}", user.username),
        &format!("postulacion:{application}:pendiente"),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({"id":application,"estado":"PENDIENTE"})))
}
pub async fn application_state(
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
    let row: Option<(String,i64,i64,i64)> = sqlx::query_as("SELECT p.estado,p.estudiante_id,p.reto_id,r.empresa_id FROM postulaciones p JOIN retos r ON r.id=p.reto_id WHERE p.id=$1 AND r.eliminado_en IS NULL FOR UPDATE OF p,r").bind(id).fetch_optional(&mut *tx).await?;
    let (from, student, challenge, owner) = row.ok_or_else(AppError::not_found)?;
    let teacher: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM integraciones_academicas WHERE reto_id=$1 AND profesor_id=$2)",
    )
    .bind(challenge)
    .bind(user.id)
    .fetch_one(&mut *tx)
    .await?;
    let allowed = match input.estado.as_str() {
        "RETIRADA" => user.id == student,
        "ACEPTADA" | "RECHAZADA" => {
            user.admin() || user.id == owner || (user.rol == "PROFESOR" && teacher)
        }
        _ => false,
    };
    if !allowed {
        return Err(AppError::forbidden());
    }
    domain::transition("MAQUINA_POSTULACION", &from, &input.estado)?;
    sqlx::query("UPDATE postulaciones SET estado=$2 WHERE id=$1")
        .bind(id)
        .bind(&input.estado)
        .execute(&mut *tx)
        .await?;
    history(
        &mut tx,
        ("participaciones", "postulacion"),
        id,
        &from,
        &input.estado,
        user.id,
        &input.comentario,
    )
    .await?;
    notify(
        &mut tx,
        student,
        "POSTULACION_RESUELTA",
        &format!("Tu postulación está {}", input.estado),
        &format!("postulacion:{id}:{}", input.estado),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({"estado":input.estado})))
}
pub async fn read_notification(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    let changed=sqlx::query("UPDATE notificaciones SET leida=true,leida_en=COALESCE(leida_en,now()) WHERE id=$1 AND usuario_id=$2").bind(id).bind(user.id).execute(&state.pool).await?.rows_affected();
    if changed == 0 {
        return Err(AppError::not_found());
    }
    Ok(Json(json!({"ok":true})))
}
#[derive(Deserialize)]
pub struct Preferences {
    recibir_email: bool,
    recibir_recordatorios: bool,
    eventos_silenciados: Vec<String>,
}
pub async fn preferences(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<Preferences>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    if input.eventos_silenciados.len() > 100
        || input.eventos_silenciados.iter().any(|s| s.len() > 60)
    {
        return Err(AppError::bad("Preferencias inválidas"));
    }
    sqlx::query("INSERT INTO preferencias_notificacion(usuario_id,recibir_email,recibir_recordatorios,eventos_silenciados,actualizado_en) VALUES($1,$2,$3,$4,now()) ON CONFLICT(usuario_id) DO UPDATE SET recibir_email=excluded.recibir_email,recibir_recordatorios=excluded.recibir_recordatorios,eventos_silenciados=excluded.eventos_silenciados,actualizado_en=now()")
        .bind(user.id).bind(input.recibir_email).bind(input.recibir_recordatorios).bind(json!(input.eventos_silenciados)).execute(&state.pool).await?;
    Ok(Json(json!({"ok":true})))
}
#[derive(Deserialize)]
pub struct UserStatus {
    activo: bool,
}
pub async fn user_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<UserStatus>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    if !user.admin() || id == user.id {
        return Err(AppError::forbidden());
    }
    let mut tx = state.pool.begin().await?;
    // Do not let an administrator deactivate another superuser via this endpoint.
    let changed=sqlx::query("UPDATE usuarios SET activo=$2,is_active=$2 WHERE id=$1 AND NOT is_superuser AND rol<>'ADMIN'").bind(id).bind(input.activo).execute(&mut *tx).await?.rows_affected();
    if changed == 0 {
        return Err(AppError::bad(
            "Usuario no encontrado o cuenta administrativa protegida",
        ));
    }
    sqlx::query("DELETE FROM rust_sessions WHERE user_id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO log_actividad(usuario_id,identificador,accion,detalle,realizado_por_id,creado_en) VALUES($1,'','CAMBIO_ESTADO',$2,$3,now())").bind(id).bind(format!("activo={}",input.activo)).bind(user.id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(json!({"ok":true})))
}
