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
/// Returns the audit row id.
pub async fn challenge_history(
    tx: &mut Transaction<'_, Postgres>,
    id: i64,
    from: &str,
    to: &str,
    user: i64,
    comment: &str,
) -> Result<i64> {
    history(tx, ("retos", "reto"), id, from, to, user, comment).await
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
        for (date, label) in [
            (self.fecha_inicio_tentativa, "Fecha de inicio"),
            (self.fecha_fin_tentativa, "Fecha de fin"),
            (self.fecha_limite_postulacion, "Fecha límite de postulación"),
        ] {
            domain::validate_date(date, label)?;
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
    challenge_history(&mut tx, id, "", "borrador", user.id, "Reto creado.").await?;
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
    // solo_empresa_con_documentos: the HU00 gate of crear_reto also applies to edits.
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
            "La empresa editó el reto tras el rechazo.",
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
    domain::max_chars(&input.comentario, 4000, "Comentario")?;
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
    let owner_company = user.id == owner && user.rol == "EMPRESA";
    if !owner_company && !user.admin() {
        return Err(AppError::forbidden());
    }
    // MAQUINA_RETO.validar comes first, as cambiar_estado_reto: an invalid
    // change is explained, not reported as a permission problem.
    domain::transition("MAQUINA_RETO", from, &input.estado)?;
    let company_action = owner_company && input.estado == "en_revision";
    let admin_action = user.admin()
        && ["aprobado", "rechazado", "en_curso", "pausado", "cancelado"]
            .contains(&input.estado.as_str());
    if !company_action && !admin_action {
        return Err(AppError::forbidden());
    }
    if company_action {
        for (name, label) in [
            ("titulo", "Título"),
            ("descripcion", "Descripción"),
            ("area", "Área"),
            ("fecha_inicio_tentativa", "Fecha de inicio"),
            ("fecha_fin_tentativa", "Fecha de fin"),
            ("fecha_limite_postulacion", "Fecha límite de postulación"),
        ] {
            if record[name].is_null() || record[name].as_str().is_some_and(|s| s.trim().is_empty())
            {
                return Err(AppError::bad(format!("Completa el campo {label}.")));
            }
        }
        // enviar_revision is also solo_empresa_con_documentos: the HU00 gate applies.
        let company: Option<i64> =
            sqlx::query_scalar("SELECT id FROM empresas WHERE usuario_id=$1")
                .bind(owner)
                .fetch_optional(&mut *tx)
                .await?;
        let reasons = match company {
            Some(company) => crate::permissions::operating_reasons(&mut tx, company).await?,
            None => vec!["La empresa no tiene perfil registrado.".to_string()],
        };
        if !reasons.is_empty() {
            return Err(AppError::bad(format!(
                "Tu empresa aún no puede publicar retos: {}",
                reasons.join(" ")
            )));
        }
    }
    if input.estado == "rechazado" && input.comentario.trim().is_empty() {
        return Err(AppError::bad("Indica el motivo del rechazo"));
    }
    if input.estado == "aprobado" {
        // admin_revisar: only companies that may operate get approved.
        let company: Option<i64> =
            sqlx::query_scalar("SELECT id FROM empresas WHERE usuario_id=$1")
                .bind(owner)
                .fetch_optional(&mut *tx)
                .await?;
        let reasons = match company {
            Some(company) => crate::permissions::operating_reasons(&mut tx, company).await?,
            None => vec!["La empresa no tiene perfil registrado.".to_string()],
        };
        if !reasons.is_empty() {
            return Err(AppError::bad(format!(
                "No se puede aprobar: {}",
                reasons.join(" ")
            )));
        }
        // Serialize number allocation across every native approval, not only this row.
        sqlx::query("SELECT pg_advisory_xact_lock(714032026)")
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE retos SET consecutivo=COALESCE(consecutivo,(SELECT COALESCE(MAX(consecutivo),0)+1 FROM retos WHERE tipo=$2 AND anio_consecutivo=extract(year from now() at time zone 'America/Bogota')::int)),anio_consecutivo=COALESCE(anio_consecutivo,extract(year from now() at time zone 'America/Bogota')::int),fecha_aprobacion=now() WHERE id=$1")
            .bind(id).bind(record["tipo"].as_str().unwrap_or("reto")).execute(&mut *tx).await?;
    }
    sqlx::query("UPDATE retos SET estado=$2,actualizado_en=now(),fecha_envio_revision=CASE WHEN $2='en_revision' THEN now() ELSE fecha_envio_revision END,comentarios_revision=CASE WHEN $2 IN ('aprobado','rechazado') THEN $3 ELSE comentarios_revision END WHERE id=$1")
        .bind(id).bind(&input.estado).bind(&input.comentario).execute(&mut *tx).await?;
    let comment = if company_action && input.comentario.trim().is_empty() {
        "Reto enviado a aprobación."
    } else {
        input.comentario.as_str()
    };
    let audit = challenge_history(&mut tx, id, from, &input.estado, user.id, comment).await?;
    let title = record["titulo"].as_str().unwrap_or("");
    if company_action {
        // notificar_admins (no dedupe), excluding the sender.
        let admins: Vec<i64> = sqlx::query_scalar(
            "SELECT id FROM usuarios WHERE is_active AND (is_superuser OR rol='ADMIN') AND id<>$1",
        )
        .bind(user.id)
        .fetch_all(&mut *tx)
        .await?;
        let message = format!(
            "La empresa {} envió el reto \"{title}\" para aprobación.",
            user.username
        );
        for admin in admins {
            notify(&mut tx, admin, "RETO_ENVIADO_REVISION", &message, "").await?;
        }
    } else {
        // _notificar_cambio_estado.
        let label = crate::dashboard::etiqueta("retos", "estado", &input.estado);
        let mut message = format!("Tu reto \"{title}\" pasó a estado: {label}.");
        if !input.comentario.is_empty() {
            message.push_str(&format!(
                "
Comentario del administrador: {}",
                input.comentario
            ));
        }
        if input.estado == "aprobado" {
            let code: Option<String> = sqlx::query_scalar("SELECT CASE WHEN tipo='hackathon' THEN 'HACK' ELSE 'RETO' END||'-'||anio_consecutivo||'-'||lpad(consecutivo::text,3,'0') FROM retos WHERE id=$1 AND consecutivo IS NOT NULL")
                .bind(id).fetch_optional(&mut *tx).await?;
            if let Some(code) = code {
                message.push_str(&format!(
                    "
Número de reto asignado: {code}."
                ));
            }
        }
        // The audit id makes each occurrence a new notification (a reto rejected twice).
        let key = format!("reto:{id}:{from}->{}:{audit}", input.estado);
        notify(&mut tx, owner, "RETO_ESTADO_CAMBIADO", &message, &key).await?;
        let kind = match input.estado.as_str() {
            "aprobado" | "finalizado" => Some("EXITO"),
            "rechazado" => Some("ERROR"),
            "cancelado" => Some("ADVERTENCIA"),
            _ => None,
        };
        if let Some(kind) = kind {
            sqlx::query(
                "UPDATE notificaciones SET tipo=$3 WHERE usuario_id=$1 AND clave_dedupe=$2",
            )
            .bind(owner)
            .bind(&key)
            .bind(kind)
            .execute(&mut *tx)
            .await?;
        }
        if input.estado == "cancelado" {
            let teachers: Vec<i64> = sqlx::query_scalar(
                "SELECT DISTINCT profesor_id FROM integraciones_academicas WHERE reto_id=$1",
            )
            .bind(id)
            .fetch_all(&mut *tx)
            .await?;
            for teacher in teachers {
                notify(
                    &mut tx,
                    teacher,
                    "RETO_ESTADO_CAMBIADO",
                    &format!("El reto \"{title}\" fue marcado como {label}."),
                    &format!("reto:{id}:cancelado:profesor"),
                )
                .await?;
            }
        }
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
    domain::max_chars(&input.habilidades, 5000, "Habilidades")?;
    domain::max_chars(&input.programa, 150, "Programa")?;
    if input.semestre.is_some_and(|v| !(1..=20).contains(&v)) {
        return Err(AppError::bad("El semestre debe estar entre 1 y 20."));
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
                "Tu postulación a este reto fue rechazada y no puede reenviarse.",
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
                "Elige una opción válida. El programa no está entre las opciones disponibles.",
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
                "Postulación enviada.",
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
                    "Postulación reenviada.",
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
    sqlx::query("UPDATE notificaciones SET titulo='Nueva postulación recibida',tipo='EXITO' WHERE usuario_id=$1 AND clave_dedupe=$2")
        .bind(owner)
        .bind(&new_key)
        .execute(&mut *tx)
        .await?;
    notify(
        &mut tx,
        user.id,
        "POSTULACION_RECIBIDA",
        &format!("Recibimos tu postulación al reto \"{title}\". Te avisaremos cuando la empresa la revise."),
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
    domain::max_chars(&input.comentario, 4000, "Comentario")?;
    let mut tx = state.pool.begin().await?;
    let row: Option<(String,i64,i64,i64,String)> = sqlx::query_as("SELECT p.estado,p.estudiante_id,p.reto_id,r.empresa_id,r.titulo FROM postulaciones p JOIN retos r ON r.id=p.reto_id WHERE p.id=$1 AND r.eliminado_en IS NULL FOR UPDATE OF p,r").bind(id).fetch_optional(&mut *tx).await?;
    let (from, student, challenge, owner, title) = row.ok_or_else(AppError::not_found)?;
    let teacher: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM integraciones_academicas WHERE reto_id=$1 AND profesor_id=$2 AND estado IN ('aprobada','publicada'))",
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
    let comment = match (input.comentario.trim().is_empty(), input.estado.as_str()) {
        (false, _) => input.comentario.as_str(),
        (true, "RETIRADA") => "Postulación retirada por el estudiante.",
        (true, _) if user.id == owner => "Postulación gestionada por la empresa.",
        (true, _) => "Postulación gestionada.",
    };
    history(
        &mut tx,
        ("participaciones", "postulacion"),
        id,
        &from,
        &input.estado,
        user.id,
        comment,
    )
    .await?;
    // _notificar_respuesta_postulacion / _notificar_postulacion_retirada.
    let key = format!("postulacion:{id}:{}", input.estado);
    let (recipient, message, kind, custom_title) = match input.estado.as_str() {
        "ACEPTADA" => (
            student,
            format!(
                "Fuiste aceptado en el reto \"{title}\". Ya puedes cargar tus entregables desde la plataforma."
            ),
            "EXITO",
            None,
        ),
        "RECHAZADA" => (
            student,
            format!("Tu postulación al reto \"{title}\" no fue seleccionada en esta ocasión."),
            "ADVERTENCIA",
            None,
        ),
        _ => {
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
            (
                owner,
                format!("{name} retiró su postulación al reto \"{title}\"."),
                "ADVERTENCIA",
                Some("Postulación retirada"),
            )
        }
    };
    notify(&mut tx, recipient, "POSTULACION_GESTIONADA", &message, &key).await?;
    sqlx::query("UPDATE notificaciones SET tipo=$3,titulo=COALESCE($4,titulo) WHERE usuario_id=$1 AND clave_dedupe=$2")
        .bind(recipient)
        .bind(&key)
        .bind(kind)
        .bind(custom_title)
        .execute(&mut *tx)
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
/// Bell dropdown: the caller's unread count and latest eight notifications.
pub async fn my_notifications(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    let unread: i64 =
        sqlx::query_scalar("SELECT count(*) FROM notificaciones WHERE usuario_id=$1 AND NOT leida")
            .bind(user.id)
            .fetch_one(&state.pool)
            .await?;
    let items: Vec<Value> = sqlx::query_scalar(
        "SELECT json_build_object('id',id,'usuario_id',usuario_id,'tipo',tipo,'evento',evento,'titulo',titulo,'mensaje',mensaje,'leida',leida,'leida_en',leida_en,'link',link,'creada_en',creada_en) FROM notificaciones WHERE usuario_id=$1 ORDER BY id DESC LIMIT 8",
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(json!({"no_leidas": unread, "items": items})))
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
    let target: Option<(String, String, bool)> = sqlx::query_as("SELECT rol,email,last_login IS NULL FROM usuarios WHERE id=$1 AND NOT is_superuser AND rol<>'ADMIN' FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
    let (rol, email, never_logged_in) = target
        .ok_or_else(|| AppError::bad("Usuario no encontrado o cuenta administrativa protegida"))?;
    // cambiar_estado_cuenta: nothing to log when the account is already there.
    if !set_account_active(&mut tx, id, input.activo, user.id, "").await? {
        return Ok(Json(json!({"ok":true,"cambiado":false})));
    }
    tx.commit().await?;
    // A professor approved after signing up learns they can now sign in.
    if input.activo && rol == "PROFESOR" && never_logged_in {
        crate::mail::send_later(
            email,
            "[RetosEAN] Tu cuenta fue aprobada".into(),
            "Tu cuenta de profesor en RetosEAN fue aprobada. Ya puedes iniciar sesión.".into(),
        );
    }
    Ok(Json(json!({"ok":true,"cambiado":true})))
}

/// cambiar_estado_cuenta: activates or deactivates an account, ending its sessions,
/// with the activity log and the is_active audit. Returns false (and does
/// nothing) when the account is already in that state.
pub async fn set_account_active(
    tx: &mut Transaction<'_, Postgres>,
    id: i64,
    active: bool,
    actor: i64,
    comment: &str,
) -> Result<bool> {
    let username: Option<String> = sqlx::query_scalar(
        "UPDATE usuarios SET activo=$2,is_active=$2 WHERE id=$1 AND is_active<>$2 RETURNING username",
    )
    .bind(id)
    .bind(active)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(username) = username else {
        return Ok(false);
    };
    sqlx::query("DELETE FROM rust_sessions WHERE user_id=$1")
        .bind(id)
        .execute(&mut **tx)
        .await?;
    let detail = match (comment.is_empty(), active) {
        (false, _) => comment,
        (true, true) => "Cuenta activada.",
        (true, false) => "Cuenta desactivada.",
    };
    sqlx::query("INSERT INTO log_actividad(usuario_id,identificador,accion,detalle,realizado_por_id,creado_en) VALUES($1,$2,'CAMBIO_ESTADO',$3,$4,now())")
        .bind(id)
        .bind(&username)
        .bind(detail)
        .bind((actor > 0).then_some(actor))
        .execute(&mut **tx)
        .await?;
    let (from, to) = if active {
        ("INACTIVO", "ACTIVO")
    } else {
        ("ACTIVO", "INACTIVO")
    };
    let audit = history(tx, ("usuarios", "usuario"), id, from, to, actor, detail).await?;
    sqlx::query("UPDATE historial_estados SET campo='is_active' WHERE id=$1")
        .bind(audit)
        .execute(&mut **tx)
        .await?;
    Ok(true)
}

/// The company opening its applications moves pending ones to EN_REVISION
/// (participaciones.services, "La empresa abrió la postulación.").
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
            "La empresa abrió la postulación.",
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

#[derive(Deserialize)]
pub struct DeliveryFilters {
    #[serde(default)]
    estado: String,
    #[serde(default)]
    evento: String,
    #[serde(default)]
    usuario: String,
    desde: Option<chrono::NaiveDate>,
    hasta: Option<chrono::NaiveDate>,
    page: Option<i64>,
}
/// registro_envios (HU15): every notification with its delivery state, for admins.
pub async fn delivery_log(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Query(filters): axum::extract::Query<DeliveryFilters>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, false).await?;
    if !user.admin() {
        return Err(AppError::forbidden());
    }
    let condition = match filters.estado.as_str() {
        "enviado" => "n.enviada_por_correo",
        "fallido" => "n.error_envio<>''",
        "leido" => "n.leida",
        "no_leido" => "NOT n.leida",
        "solo_app" => "NOT n.enviada_por_correo AND n.error_envio=''",
        _ => "TRUE",
    };
    let page = filters.page.unwrap_or(1).clamp(1, 100000);
    let records: Value = sqlx::query_scalar(&format!("SELECT COALESCE(jsonb_agg(r ORDER BY r.creada_en DESC, r.id DESC),'[]') FROM (SELECT n.id,n.titulo,n.evento,n.mensaje,n.leida,n.enviada_por_correo,n.error_envio,n.creada_en,u.username FROM notificaciones n JOIN usuarios u ON u.id=n.usuario_id WHERE {condition} AND ($1='' OR n.evento=$1) AND ($2='' OR u.username ILIKE '%'||$2||'%') AND ($3::date IS NULL OR (n.creada_en AT TIME ZONE 'America/Bogota')::date>=$3) AND ($4::date IS NULL OR (n.creada_en AT TIME ZONE 'America/Bogota')::date<=$4) ORDER BY n.creada_en DESC,n.id DESC LIMIT 50 OFFSET $5) r"))
        .bind(&filters.evento).bind(filters.usuario.trim()).bind(filters.desde).bind(filters.hasta).bind((page - 1) * 50)
        .fetch_one(&state.pool).await?;
    let summary: Value = sqlx::query_scalar("SELECT jsonb_build_object('total',count(*),'enviadas',count(*) FILTER (WHERE enviada_por_correo),'fallidas',count(*) FILTER (WHERE error_envio<>''),'leidas',count(*) FILTER (WHERE leida)) FROM notificaciones")
        .fetch_one(&state.pool).await?;
    Ok(Json(
        json!({"registros": records, "resumen": summary, "page": page}),
    ))
}

/// eliminar_reto_borrador: logical delete of the company's own draft, in the history.
pub async fn delete_challenge(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    let mut tx = state.pool.begin().await?;
    let estado: Option<String> = sqlx::query_scalar("SELECT estado FROM retos WHERE id=$1 AND empresa_id=$2 AND $3='EMPRESA' AND eliminado_en IS NULL FOR UPDATE")
        .bind(id)
        .bind(user.id)
        .bind(&user.rol)
        .fetch_optional(&mut *tx)
        .await?;
    let estado = estado.ok_or_else(AppError::not_found)?;
    if estado != "borrador" {
        return Err(AppError::bad("Solo puedes eliminar retos en borrador."));
    }
    sqlx::query("UPDATE retos SET eliminado_en=now(),actualizado_en=now() WHERE id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    challenge_history(
        &mut tx,
        id,
        &estado,
        &estado,
        user.id,
        "Reto eliminado por la empresa.",
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({"ok":true,"mensaje":"Reto eliminado."})))
}
