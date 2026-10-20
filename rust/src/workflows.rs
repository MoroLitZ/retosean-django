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
    // solo_empresa_con_documentos: puede_la_empresa_operar.
    let company: Option<i64> = sqlx::query_scalar("SELECT id FROM empresas WHERE usuario_id=$1")
        .bind(user.id)
        .fetch_optional(&mut *tx)
        .await?;
    let company =
        company.ok_or_else(|| AppError::bad("No tienes un perfil de empresa asociado."))?;
    let reasons = crate::permissions::operating_reasons(&mut tx, company).await?;
    if !reasons.is_empty() {
        return Err(AppError::bad(format!(
            "Tu empresa aún no puede publicar retos: {}",
            reasons.join(" ")
        )));
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
#[derive(Deserialize, Default)]
pub struct ApplicationInput {
    #[serde(default)]
    motivacion: String,
    #[serde(default)]
    habilidades: String,
    semestre: Option<i32>,
    #[serde(default)]
    programa: String,
    #[serde(default)]
    acepta_terminos: bool,
}
/// The application as JSON, or multipart when it carries the CV (hoja_de_vida).
async fn application_form(
    state: &AppState,
    request: axum::extract::Request,
) -> Result<(ApplicationInput, Option<axum::body::Bytes>)> {
    use axum::extract::FromRequest;
    let multipart = request
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("multipart/"));
    if !multipart {
        let Json(input) = Json::<ApplicationInput>::from_request(request, state)
            .await
            .map_err(|_| AppError::bad("Datos de postulación inválidos"))?;
        return Ok((input, None));
    }
    let mut form = axum::extract::Multipart::from_request(request, state)
        .await
        .map_err(|_| AppError::bad("Carga inválida"))?;
    let mut input = ApplicationInput::default();
    let mut cv = None;
    while let Some(field) = form
        .next_field()
        .await
        .map_err(|_| AppError::bad("Carga inválida"))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "hoja_de_vida" {
            let filename = field.file_name().unwrap_or("").to_lowercase();
            let bytes = field
                .bytes()
                .await
                .map_err(|_| AppError::bad("La hoja de vida no puede superar 5 MB."))?;
            if bytes.is_empty() {
                continue;
            }
            // PostulacionForm.clean_hoja_de_vida: PDF only, 5 MB.
            if !filename.ends_with(".pdf") || !bytes.starts_with(b"%PDF") {
                return Err(AppError::bad("La hoja de vida debe ser un archivo PDF."));
            }
            if bytes.len() > 5 * 1024 * 1024 {
                return Err(AppError::bad("La hoja de vida no puede superar 5 MB."));
            }
            cv = Some(bytes);
            continue;
        }
        let text = field
            .text()
            .await
            .map_err(|_| AppError::bad("Datos de postulación inválidos"))?;
        match name.as_str() {
            "motivacion" => input.motivacion = text,
            "habilidades" => input.habilidades = text,
            "programa" => input.programa = text.trim().to_string(),
            "semestre" if !text.trim().is_empty() => {
                input.semestre = Some(
                    text.trim()
                        .parse()
                        .map_err(|_| AppError::bad("Semestre inválido"))?,
                )
            }
            "acepta_terminos" => {
                input.acepta_terminos = ["on", "true", "1"].contains(&text.as_str())
            }
            _ => {}
        }
    }
    Ok((input, cv))
}
/// postular_a_reto (HU10/HU11).
pub async fn apply(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    request: axum::extract::Request,
) -> Result<Json<Value>> {
    let headers = request.headers().clone();
    let user = auth::current(&state, &headers, true).await?;
    if user.rol != "ESTUDIANTE" {
        return Err(AppError::forbidden());
    }
    let (input, cv) = application_form(&state, request).await?;
    domain::validate_text(&input.motivacion, 5000, "Motivación")?;
    if input.habilidades.len() > 5000
        || input.programa.chars().count() > 150
        || input.semestre.is_some_and(|v| !(1..=20).contains(&v))
    {
        return Err(AppError::bad("Datos de postulación inválidos"));
    }
    if !input.acepta_terminos {
        return Err(AppError::bad(
            "Debes aceptar los términos y condiciones para postularte.",
        ));
    }
    let mut tx = state.pool.begin().await?;
    let row: Option<(i64, String)> = sqlx::query_as("SELECT empresa_id,titulo FROM retos WHERE id=$1 AND estado='abierto' AND eliminado_en IS NULL AND (fecha_limite_postulacion IS NULL OR fecha_limite_postulacion >= (now() at time zone 'America/Bogota')::date) FOR UPDATE").bind(id).fetch_optional(&mut *tx).await?;
    let (owner, title) = row
        .ok_or_else(|| AppError::bad("El reto no está abierto o venció la fecha de postulación"))?;
    let previous: Option<(i64, String, String)> = sqlx::query_as(
        "SELECT id,estado,programa FROM postulaciones WHERE reto_id=$1 AND estudiante_id=$2 FOR UPDATE",
    )
    .bind(id)
    .bind(user.id)
    .fetch_optional(&mut *tx)
    .await?;
    match previous.as_ref().map(|p| p.1.as_str()) {
        Some("ACEPTADA") => {
            return Err(AppError::bad(
                "Ya fuiste aceptado en este reto. No es necesario volver a postularte.",
            ));
        }
        Some("RECHAZADA") => {
            return Err(AppError::bad(
                "Tu postulacion a este reto fue rechazada y no puede reenviarse.",
            ));
        }
        _ => {}
    }
    // The program comes from the catalog; an older value already saved stays valid.
    if !input.programa.is_empty() && previous.as_ref().is_none_or(|p| p.2 != input.programa) {
        let known: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM programas WHERE nombre=$1)")
                .bind(&input.programa)
                .fetch_one(&mut *tx)
                .await?;
        if !known {
            return Err(AppError::bad(
                "Escoja una opción válida. El programa no está entre las opciones disponibles.",
            ));
        }
    }
    let key = cv
        .as_ref()
        .map(|_| format!("{}.pdf", uuid::Uuid::new_v4().simple()));
    let (application, estado): (i64, String) = match &previous {
        None => {
            let id = sqlx::query_scalar("INSERT INTO postulaciones(reto_id,estudiante_id,fecha_postulacion,estado,motivacion,habilidades,semestre,programa,hoja_de_vida,acepta_terminos) VALUES($1,$2,now(),'PENDIENTE',$3,$4,$5,$6,$7,true) RETURNING id")
                .bind(id).bind(user.id).bind(&input.motivacion).bind(&input.habilidades).bind(input.semestre).bind(&input.programa).bind(key.as_deref().unwrap_or("")).fetch_one(&mut *tx).await?;
            history(
                &mut tx,
                ("participaciones", "postulacion"),
                id,
                "",
                "PENDIENTE",
                user.id,
                "Postulacion enviada.",
            )
            .await?;
            (id, "PENDIENTE".into())
        }
        Some((existing, from, _)) => {
            // Editing keeps the state; a withdrawn application is re-submitted.
            let to = if from == "RETIRADA" {
                "PENDIENTE"
            } else {
                from.as_str()
            };
            sqlx::query("UPDATE postulaciones SET motivacion=$2,habilidades=$3,semestre=$4,programa=$5,hoja_de_vida=COALESCE($6,hoja_de_vida),acepta_terminos=true,estado=$7 WHERE id=$1")
                .bind(existing).bind(&input.motivacion).bind(&input.habilidades).bind(input.semestre).bind(&input.programa).bind(key.as_deref()).bind(to).execute(&mut *tx).await?;
            if from == "RETIRADA" {
                domain::transition("MAQUINA_POSTULACION", from, "PENDIENTE")?;
                history(
                    &mut tx,
                    ("participaciones", "postulacion"),
                    *existing,
                    from,
                    "PENDIENTE",
                    user.id,
                    "Postulacion reenviada.",
                )
                .await?;
            }
            (*existing, to.to_string())
        }
    };
    let name = {
        let full = format!("{} {}", user.first_name, user.last_name)
            .trim()
            .to_string();
        if full.is_empty() {
            user.username.clone()
        } else {
            full
        }
    };
    let new_key = format!("postulacion-nueva:{id}:{}", user.id);
    notify(
        &mut tx,
        owner,
        "POSTULACION_NUEVA",
        &format!("{name} se ha postulado a tu reto \"{title}\"."),
        &new_key,
    )
    .await?;
    sqlx::query("UPDATE notificaciones SET titulo='Nueva postulacion recibida',tipo='EXITO' WHERE usuario_id=$1 AND clave_dedupe=$2")
        .bind(owner)
        .bind(&new_key)
        .execute(&mut *tx)
        .await?;
    notify(
        &mut tx,
        user.id,
        "POSTULACION_RECIBIDA",
        &format!("Recibimos tu postulacion al reto \"{title}\". Te avisaremos cuando la empresa la revise."),
        &format!("postulacion-recibida:{application}:{estado}"),
    )
    .await?;
    let path = match (&key, &cv) {
        (Some(key), Some(bytes)) => {
            tokio::fs::create_dir_all(&state.config.media_root)
                .await
                .map_err(|_| AppError::bad("No se pudo guardar la hoja de vida"))?;
            let path = state.config.media_root.join(key);
            tokio::fs::write(&path, bytes)
                .await
                .map_err(|_| AppError::bad("No se pudo guardar la hoja de vida"))?;
            Some(path)
        }
        _ => None,
    };
    if let Err(error) = tx.commit().await {
        if let Some(path) = path {
            let _ = tokio::fs::remove_file(path).await;
        }
        return Err(error.into());
    }
    Ok(Json(json!({"id":application,"estado":estado})))
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
    // Django's alternar_estado: the shared audit with campo is_active.
    let (from, to) = if input.activo {
        ("INACTIVO", "ACTIVO")
    } else {
        ("ACTIVO", "INACTIVO")
    };
    let audit = history(&mut tx, ("usuarios", "usuario"), id, from, to, user.id, "").await?;
    sqlx::query("UPDATE historial_estados SET campo='is_active' WHERE id=$1")
        .bind(audit)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({"ok":true})))
}

/// The company opening its applications moves pending ones to EN_REVISION
/// (participaciones.services, "La empresa abrio la postulacion.").
pub async fn mark_applications_in_review(state: &AppState, user: &auth::User) -> Result<()> {
    if user.rol != "EMPRESA" {
        return Ok(());
    }
    let mut tx = state.pool.begin().await?;
    let opened: Vec<i64> = sqlx::query_scalar("UPDATE postulaciones SET estado='EN_REVISION' WHERE estado='PENDIENTE' AND reto_id IN (SELECT id FROM retos WHERE empresa_id=$1) RETURNING id")
        .bind(user.id).fetch_all(&mut *tx).await?;
    for id in opened {
        history(
            &mut tx,
            ("participaciones", "postulacion"),
            id,
            "PENDIENTE",
            "EN_REVISION",
            user.id,
            "La empresa abrio la postulacion.",
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}
