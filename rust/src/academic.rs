//! Academic integrations (HU03): create, edit, submit, review and publish, with
//! the rules of IntegracionAcademicaForm and the retos integration views.
use crate::{
    AppState, auth,
    auth::User,
    domain,
    error::{AppError, Result},
    workflows::{StateInput, history, notify},
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};

/// Reto.NIVEL_ACADEMICO_CHOICES.
const NIVELES: [&str; 6] = [
    "Pregrado",
    "Posgrado",
    "Especialización",
    "Maestría",
    "Doctorado",
    "Educación Continua",
];

#[derive(Deserialize, Default)]
pub struct IntegrationFields {
    #[serde(default)]
    facultad: String,
    #[serde(default)]
    nivel_formacion: String,
    unidad_estudio: Option<i64>,
    /// Kept for older clients; a study unit overrides it.
    #[serde(default)]
    programa_academico: String,
    #[serde(default)]
    ecosistema: String,
    #[serde(default)]
    descripcion: String,
    #[serde(default)]
    alcance: String,
    #[serde(default)]
    entregable_esperado: String,
    #[serde(default)]
    cronograma_sesiones: String,
    #[serde(default)]
    requerimientos_empresa: String,
    #[serde(default)]
    requerimientos_internos: String,
    #[serde(default)]
    espacio_fisico: String,
    /// "enviar" submits right after saving, as the form's second button.
    #[serde(default)]
    accion: String,
}
#[derive(Deserialize)]
pub struct IntegrationInput {
    #[serde(flatten)]
    fields: IntegrationFields,
    #[serde(flatten)]
    team: crate::team::TeamInput,
}

/// IntegracionAcademicaForm: catalog selects (a stored legacy value stays valid
/// on edit), the study unit filtered by faculty, and the field lengths.
async fn clean(
    tx: &mut Transaction<'_, Postgres>,
    input: &mut IntegrationFields,
    current: Option<&Value>,
) -> Result<()> {
    let legacy = |field: &str| {
        current
            .and_then(|c| c[field].as_str())
            .unwrap_or("")
            .trim()
            .to_string()
    };
    let mut errors = Vec::new();
    let faculty = input.facultad.trim().to_string();
    if !faculty.is_empty() && faculty != legacy("facultad") {
        let known: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM facultades WHERE nombre=$1)")
                .bind(&faculty)
                .fetch_one(&mut **tx)
                .await?;
        if !known {
            errors.push("facultad");
        }
    }
    let level = input.nivel_formacion.trim().to_string();
    if !level.is_empty() && level != legacy("nivel_formacion") && !NIVELES.contains(&level.as_str())
    {
        errors.push("nivel_formacion");
    }
    let ecosystem = input.ecosistema.trim().to_string();
    if !ecosystem.is_empty() && ecosystem != legacy("ecosistema") {
        let known: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM ecosistemas WHERE nombre=$1)")
                .bind(&ecosystem)
                .fetch_one(&mut **tx)
                .await?;
        if !known {
            errors.push("ecosistema");
        }
    }
    if let Some(unit) = input.unidad_estudio {
        let name: Option<String> = sqlx::query_scalar("SELECT u.nombre FROM unidades_estudio u JOIN programas p ON p.id=u.programa_id JOIN facultades f ON f.id=p.facultad_id WHERE u.id=$1 AND u.activo AND ($2='' OR f.nombre=$2)")
            .bind(unit)
            .bind(&faculty)
            .fetch_optional(&mut **tx)
            .await?;
        match name {
            Some(name) => input.programa_academico = name,
            None => errors.push("unidad_estudio"),
        }
    }
    if !errors.is_empty() {
        return Err(AppError::bad(format!(
            "Escoja una opción válida en: {}.",
            errors.join(", ")
        )));
    }
    for (text, max, label) in [
        (&input.facultad, 150, "Facultad"),
        (&input.nivel_formacion, 50, "Nivel de formacion"),
        (&input.programa_academico, 150, "Programa academico"),
        (&input.ecosistema, 150, "Ecosistema"),
        (&input.entregable_esperado, 255, "Entregable esperado"),
        (&input.espacio_fisico, 200, "Espacio fisico"),
    ] {
        if text.chars().count() > max {
            return Err(AppError::bad(format!("{label}: máximo {max} caracteres")));
        }
    }
    for text in [
        &input.descripcion,
        &input.alcance,
        &input.cronograma_sesiones,
        &input.requerimientos_empresa,
        &input.requerimientos_internos,
    ] {
        if text.len() > 20000 {
            return Err(AppError::bad("Texto demasiado largo"));
        }
    }
    Ok(())
}

/// GET: the form's choices (facultades, niveles, ecosistemas and active units,
/// filtered by faculty when given).
#[derive(Deserialize)]
pub struct OptionsQuery {
    #[serde(default)]
    facultad: String,
}
pub async fn options(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<OptionsQuery>,
) -> Result<Json<Value>> {
    auth::current(&state, &headers, false).await?;
    let data: Value = sqlx::query_scalar("SELECT jsonb_build_object('facultades',(SELECT COALESCE(jsonb_agg(nombre ORDER BY nombre),'[]') FROM facultades),'ecosistemas',(SELECT COALESCE(jsonb_agg(nombre ORDER BY nombre),'[]') FROM ecosistemas),'unidades',(SELECT COALESCE(jsonb_agg(jsonb_build_object('value',u.id,'label',u.codigo||' - '||u.nombre||' ('||p.nombre||')') ORDER BY p.nombre,u.ciclo,u.nombre),'[]') FROM unidades_estudio u JOIN programas p ON p.id=u.programa_id JOIN facultades f ON f.id=p.facultad_id WHERE u.activo AND ($1='' OR f.nombre=$1)))")
        .bind(query.facultad.trim()).fetch_one(&state.pool).await?;
    let mut data = data;
    data["niveles"] = json!(NIVELES);
    Ok(Json(data))
}

/// crear_integracion: saved as a draft; "enviar" submits it right away.
pub async fn integrate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(reto): Path<i64>,
    Json(mut input): Json<IntegrationInput>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    if user.rol != "PROFESOR" {
        return Err(AppError::forbidden());
    }
    crate::team::validate(&input.team)?;
    let mut tx = state.pool.begin().await?;
    let eligible: Option<i64> = sqlx::query_scalar("SELECT id FROM retos WHERE id=$1 AND eliminado_en IS NULL AND estado IN ('aprobado','abierto','en_curso','pausado') FOR UPDATE")
        .bind(reto)
        .fetch_optional(&mut *tx)
        .await?;
    if eligible.is_none() {
        return Err(AppError::bad(
            "Solo se pueden integrar retos aprobados, abiertos o en curso.",
        ));
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
    clean(&mut tx, &mut input.fields, None).await?;
    let f = &input.fields;
    let id: i64 = sqlx::query_scalar("INSERT INTO integraciones_academicas(reto_id,profesor_id,facultad,nivel_formacion,programa_academico,ecosistema,descripcion,alcance,entregable_esperado,cronograma_sesiones,requerimientos_empresa,requerimientos_internos,espacio_fisico,estado,comentarios_revision,creado_en,actualizado_en) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,'borrador','',now(),now()) RETURNING id")
        .bind(reto).bind(user.id).bind(f.facultad.trim()).bind(f.nivel_formacion.trim()).bind(&f.programa_academico).bind(f.ecosistema.trim()).bind(&f.descripcion).bind(&f.alcance).bind(&f.entregable_esperado).bind(&f.cronograma_sesiones).bind(&f.requerimientos_empresa).bind(&f.requerimientos_internos).bind(&f.espacio_fisico)
        .fetch_one(&mut *tx).await?;
    crate::team::save(&mut tx, id, reto, &input.team).await?;
    history(
        &mut tx,
        ("seguimiento", "integracionacademica"),
        id,
        "",
        "borrador",
        user.id,
        "Integracion creada.",
    )
    .await?;
    let estado = if input.fields.accion == "enviar" {
        // _enviar_integracion: a draft with gaps simply stays a draft.
        match submit(&mut tx, id, &user).await {
            Ok(()) => "en_revision",
            Err(error) if error.0 == axum::http::StatusCode::BAD_REQUEST => "borrador",
            Err(error) => return Err(error),
        }
    } else {
        "borrador"
    };
    tx.commit().await?;
    Ok(Json(json!({"id":id,"estado":estado})))
}

#[derive(Deserialize)]
pub struct EditInput {
    #[serde(flatten)]
    fields: IntegrationFields,
}
/// editar_integracion: while a draft or rejected, or the reto is open or running.
pub async fn edit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(mut input): Json<EditInput>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    let mut tx = state.pool.begin().await?;
    let current: Option<Value> = sqlx::query_scalar("SELECT to_jsonb(i)||jsonb_build_object('reto_estado',r.estado) FROM integraciones_academicas i JOIN retos r ON r.id=i.reto_id WHERE i.id=$1 AND i.profesor_id=$2 AND $3='PROFESOR' FOR UPDATE OF i")
        .bind(id)
        .bind(user.id)
        .bind(&user.rol)
        .fetch_optional(&mut *tx)
        .await?;
    let current = current.ok_or_else(AppError::not_found)?;
    let estado = current["estado"].as_str().unwrap_or("").to_string();
    let editable = ["borrador", "rechazada"].contains(&estado.as_str())
        || ["abierto", "en_curso"].contains(&current["reto_estado"].as_str().unwrap_or(""));
    if !editable {
        return Err(AppError::bad(
            "Esta integracion no puede editarse desde su estado actual.",
        ));
    }
    clean(&mut tx, &mut input.fields, Some(&current)).await?;
    let f = &input.fields;
    sqlx::query("UPDATE integraciones_academicas SET facultad=$2,nivel_formacion=$3,programa_academico=$4,ecosistema=$5,descripcion=$6,alcance=$7,entregable_esperado=$8,cronograma_sesiones=$9,requerimientos_empresa=$10,requerimientos_internos=$11,espacio_fisico=$12,actualizado_en=now() WHERE id=$1")
        .bind(id).bind(f.facultad.trim()).bind(f.nivel_formacion.trim()).bind(&f.programa_academico).bind(f.ecosistema.trim()).bind(&f.descripcion).bind(&f.alcance).bind(&f.entregable_esperado).bind(&f.cronograma_sesiones).bind(&f.requerimientos_empresa).bind(&f.requerimientos_internos).bind(&f.espacio_fisico)
        .execute(&mut *tx).await?;
    let mut estado = estado;
    if estado == "rechazada" {
        domain::transition("MAQUINA_INTEGRACION", "rechazada", "borrador")?;
        sqlx::query("UPDATE integraciones_academicas SET estado='borrador',comentarios_revision='' WHERE id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        history(
            &mut tx,
            ("seguimiento", "integracionacademica"),
            id,
            "rechazada",
            "borrador",
            user.id,
            "El profesor edito la integracion tras el rechazo.",
        )
        .await?;
        estado = "borrador".into();
    }
    if input.fields.accion == "enviar" {
        submit(&mut tx, id, &user).await?;
        estado = "en_revision".into();
    }
    tx.commit().await?;
    Ok(Json(json!({"id":id,"estado":estado})))
}

/// _enviar_integracion: required fields, team and schedule, then the notices.
async fn submit(tx: &mut Transaction<'_, Postgres>, id: i64, user: &User) -> Result<()> {
    let row: Value =
        sqlx::query_scalar("SELECT to_jsonb(i) FROM integraciones_academicas i WHERE id=$1")
            .bind(id)
            .fetch_one(&mut **tx)
            .await?;
    let mut missing: Vec<&str> = [
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
    missing.extend(crate::team::missing(tx, id).await?);
    if !missing.is_empty() {
        return Err(AppError::bad(format!(
            "Completa estos campos antes de enviar: {}.",
            missing.join(", ")
        )));
    }
    let from = row["estado"].as_str().unwrap_or("");
    domain::transition("MAQUINA_INTEGRACION", from, "en_revision")?;
    let sent: chrono::DateTime<chrono::Utc> = sqlx::query_scalar("UPDATE integraciones_academicas SET estado='en_revision',fecha_envio_revision=now(),actualizado_en=now() WHERE id=$1 RETURNING fecha_envio_revision")
        .bind(id)
        .fetch_one(&mut **tx)
        .await?;
    history(
        tx,
        ("seguimiento", "integracionacademica"),
        id,
        from,
        "en_revision",
        user.id,
        "Integracion enviada a revision.",
    )
    .await?;
    let (title, company): (String, i64) = sqlx::query_as("SELECT r.titulo,r.empresa_id FROM integraciones_academicas i JOIN retos r ON r.id=i.reto_id WHERE i.id=$1")
        .bind(id)
        .fetch_one(&mut **tx)
        .await?;
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
    let message = format!(
        "{name} envio una postulacion de integracion para el reto \"{title}\" y esta pendiente de revision."
    );
    let key = format!("integracion:{id}:en_revision:{}", sent.to_rfc3339());
    notify(
        tx,
        company,
        "INTEGRACION_ENVIADA_REVISION",
        &message,
        &format!("{key}:empresa"),
    )
    .await?;
    let admins: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM usuarios WHERE is_active AND (is_superuser OR rol='ADMIN') AND id<>$1",
    )
    .bind(user.id)
    .fetch_all(&mut **tx)
    .await?;
    for admin in admins {
        notify(
            tx,
            admin,
            "INTEGRACION_ENVIADA_REVISION",
            &message,
            &format!("{key}:admin"),
        )
        .await?;
    }
    Ok(())
}

/// Submit (professor), review (admin or the owning company) and publish (professor).
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
    let challenge: Option<(String, i64, String)> = sqlx::query_as("SELECT estado,empresa_id,titulo FROM retos WHERE id=$1 AND eliminado_en IS NULL FOR UPDATE")
        .bind(reto)
        .fetch_optional(&mut *tx)
        .await?;
    let (challenge_state, company, title) = challenge.ok_or_else(AppError::not_found)?;
    let row: Value = sqlx::query_scalar(
        "SELECT to_jsonb(i) FROM integraciones_academicas i WHERE id=$1 FOR UPDATE",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    let owner = row["profesor_id"].as_i64().unwrap_or(0);
    let from = row["estado"].as_str().unwrap_or("").to_string();
    let teacher = user.id == owner && user.rol == "PROFESOR";
    let reviewer = user.admin() || (user.rol == "EMPRESA" && user.id == company);
    let allowed = match input.estado.as_str() {
        "en_revision" | "publicada" => teacher,
        "aprobada" | "rechazada" => reviewer,
        _ => false,
    };
    if !allowed {
        return Err(AppError::forbidden());
    }
    match input.estado.as_str() {
        "en_revision" => submit(&mut tx, id, &user).await?,
        "aprobada" | "rechazada" => {
            domain::transition("MAQUINA_INTEGRACION", &from, &input.estado)?;
            let approve = input.estado == "aprobada";
            sqlx::query("UPDATE integraciones_academicas SET estado=$2,comentarios_revision=$3,fecha_aprobacion=CASE WHEN $2='aprobada' THEN now() END,actualizado_en=now() WHERE id=$1")
                .bind(id).bind(&input.estado).bind(&input.comentario).execute(&mut *tx).await?;
            history(
                &mut tx,
                ("seguimiento", "integracionacademica"),
                id,
                &from,
                &input.estado,
                user.id,
                &input.comentario,
            )
            .await?;
            let mut message = format!(
                "Tu integracion academica del reto \"{title}\" fue {}.",
                input.estado
            );
            if !input.comentario.is_empty() {
                message.push_str(&format!("\nComentario: {}", input.comentario));
            }
            let key = format!("integracion:{id}:{}", input.estado);
            notify(&mut tx, owner, "INTEGRACION_REVISADA", &message, &key).await?;
            sqlx::query(
                "UPDATE notificaciones SET tipo=$3 WHERE usuario_id=$1 AND clave_dedupe=$2",
            )
            .bind(owner)
            .bind(&key)
            .bind(if approve { "EXITO" } else { "ADVERTENCIA" })
            .execute(&mut *tx)
            .await?;
        }
        _ => {
            // publicar_integracion: only approved ones; the reto opens to students.
            if from != "aprobada" {
                return Err(AppError::bad(
                    "Solo puedes publicar integraciones aprobadas.",
                ));
            }
            domain::transition("MAQUINA_INTEGRACION", &from, "publicada")?;
            sqlx::query("UPDATE integraciones_academicas SET estado='publicada',actualizado_en=now() WHERE id=$1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            history(
                &mut tx,
                ("seguimiento", "integracionacademica"),
                id,
                &from,
                "publicada",
                user.id,
                "Integracion publicada para los estudiantes.",
            )
            .await?;
            let teacher_name = {
                let full = format!("{} {}", user.first_name, user.last_name)
                    .trim()
                    .to_string();
                if full.is_empty() {
                    user.username.clone()
                } else {
                    full
                }
            };
            crate::team::notify_students(&mut tx, id, &title, &teacher_name).await?;
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
                    "Integracion academica publicada: reto abierto a estudiantes.",
                )
                .await?;
                // cambiar_estado_reto tells the company.
                notify(
                    &mut tx,
                    company,
                    "RETO_ESTADO_CAMBIADO",
                    &format!("Tu reto \"{title}\" paso a estado: Abierto."),
                    &format!("reto:{reto}:aprobado->abierto"),
                )
                .await?;
            }
        }
    }
    tx.commit().await?;
    Ok(Json(json!({"estado":input.estado})))
}
