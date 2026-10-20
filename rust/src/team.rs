//! HU03: structured team and dated schedule of an academic integration,
//! ported from Django's ParticipanteIntegracion and SesionRetoAcademico.
use crate::{
    AppState, auth,
    error::{AppError, Result},
    resources,
    workflows::notify,
};
use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};

#[derive(Deserialize, Default)]
pub struct ParticipantInput {
    pub rol: String,
    pub nombre: String,
    #[serde(default)]
    pub documento: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub facultad: String,
    #[serde(default)]
    pub programa: String,
}
#[derive(Deserialize)]
pub struct SessionInput {
    pub tipo: String,
    pub fecha_hora: DateTime<Utc>,
    #[serde(default)]
    pub enlace_reunion: String,
    #[serde(default)]
    pub observaciones: String,
}
#[derive(Deserialize, Default)]
pub struct TeamInput {
    #[serde(default)]
    pub participantes: Vec<ParticipantInput>,
    #[serde(default)]
    pub sesiones: Vec<SessionInput>,
}

fn filled(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.chars().count() <= max
}

/// Same per-role requirements as ParticipanteIntegracionForm.clean.
pub fn validate(team: &TeamInput) -> Result<()> {
    if team.participantes.len() > 60 || team.sesiones.len() > 30 {
        return Err(AppError::bad(
            "Demasiadas filas en el equipo o el cronograma",
        ));
    }
    for (i, p) in team.participantes.iter().enumerate() {
        let fila = i + 1;
        if !["PROFESOR", "ESTUDIANTE", "EXPERTO"].contains(&p.rol.as_str()) {
            return Err(AppError::bad(format!("Integrante {fila}: rol inválido")));
        }
        if !filled(&p.nombre, 150) {
            return Err(AppError::bad(format!(
                "Integrante {fila}: indica el nombre"
            )));
        }
        if !filled(&p.documento, 30) {
            return Err(AppError::bad(format!(
                "Integrante {fila}: indica la cédula o documento"
            )));
        }
        let email = p.email.trim();
        if email.len() > 254 || email.split('@').filter(|s| !s.is_empty()).count() != 2 {
            return Err(AppError::bad(format!(
                "Integrante {fila}: indica un correo válido"
            )));
        }
        let (campo, valor, max) = if p.rol == "ESTUDIANTE" {
            ("el programa del estudiante", &p.programa, 160)
        } else {
            ("la facultad u organización", &p.facultad, 150)
        };
        if !filled(valor, max) {
            return Err(AppError::bad(format!("Integrante {fila}: indica {campo}")));
        }
    }
    for (i, s) in team.sesiones.iter().enumerate() {
        let fila = i + 1;
        if !["inicio", "seguimiento", "preseleccion", "evaluacion"].contains(&s.tipo.as_str()) {
            return Err(AppError::bad(format!("Sesión {fila}: tipo inválido")));
        }
        let link = s.enlace_reunion.trim();
        if !link.is_empty()
            && (link.len() > 200 || !(link.starts_with("https://") || link.starts_with("http://")))
        {
            return Err(AppError::bad(format!(
                "Sesión {fila}: el enlace debe ser una URL"
            )));
        }
        if s.observaciones.chars().count() > 5000 {
            return Err(AppError::bad(format!(
                "Sesión {fila}: observaciones demasiado largas"
            )));
        }
    }
    Ok(())
}

/// Replaces the team and schedule, keeping who was already notified.
pub async fn save(
    tx: &mut Transaction<'_, Postgres>,
    integration: i64,
    reto: i64,
    team: &TeamInput,
) -> Result<()> {
    let notified: Vec<(String, String, DateTime<Utc>)> = sqlx::query_as(
        "DELETE FROM participantes_integracion WHERE integracion_id=$1 RETURNING rol,documento,notificado_en",
    )
    .bind(integration)
    .fetch_all(&mut **tx)
    .await
    .map(|rows: Vec<(String, String, Option<DateTime<Utc>>)>| {
        rows.into_iter().filter_map(|(r, d, n)| n.map(|n| (r, d, n))).collect()
    })?;
    for p in &team.participantes {
        let when = notified
            .iter()
            .find(|(r, d, _)| *r == p.rol && *d == p.documento.trim())
            .map(|n| n.2);
        // Links the platform account when the email identifies exactly one (as in Django).
        sqlx::query("INSERT INTO participantes_integracion(integracion_id,rol,nombre,documento,email,facultad,programa,usuario_id,notificado_en) VALUES($1,$2,$3,$4,$5,$6,$7,(SELECT min(id) FROM usuarios WHERE lower(email)=lower($5) HAVING count(*)=1),$8)")
            .bind(integration).bind(&p.rol).bind(p.nombre.trim()).bind(p.documento.trim()).bind(p.email.trim())
            .bind(p.facultad.trim()).bind(p.programa.trim()).bind(when)
            .execute(&mut **tx).await?;
    }
    sqlx::query("DELETE FROM sesiones_reto_academico WHERE integracion_id=$1")
        .bind(integration)
        .execute(&mut **tx)
        .await?;
    for s in &team.sesiones {
        sqlx::query("INSERT INTO sesiones_reto_academico(reto_id,integracion_id,tipo,fecha_hora,enlace_reunion,observaciones) VALUES($1,$2,$3,$4,NULLIF($5,''),$6)")
            .bind(reto).bind(integration).bind(&s.tipo).bind(s.fecha_hora).bind(s.enlace_reunion.trim()).bind(s.observaciones.trim())
            .execute(&mut **tx).await?;
    }
    Ok(())
}

/// What is still missing before review: at least one professor and the
/// start, follow-up and evaluation sessions (pre-selection is optional).
pub async fn missing(
    tx: &mut Transaction<'_, Postgres>,
    integration: i64,
) -> Result<Vec<&'static str>> {
    let (professors, kinds): (i64, Vec<String>) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM participantes_integracion WHERE integracion_id=$1 AND rol='PROFESOR'), COALESCE((SELECT array_agg(DISTINCT tipo) FROM sesiones_reto_academico WHERE integracion_id=$1),'{}')",
    )
    .bind(integration)
    .fetch_one(&mut **tx)
    .await?;
    let mut faltan = vec![];
    if professors == 0 {
        faltan.push("al menos un profesor en el equipo");
    }
    for (tipo, etiqueta) in [
        ("inicio", "una sesion de inicio"),
        ("seguimiento", "al menos una sesion de seguimiento"),
        ("evaluacion", "una sesion de evaluacion"),
    ] {
        if !kinds.iter().any(|k| k == tipo) {
            faltan.push(etiqueta);
        }
    }
    Ok(faltan)
}

/// On publication, linked students are told once (Django notificar_participantes_integracion).
/// notificar_participantes_integracion: once per student with an account.
pub async fn notify_students(
    tx: &mut Transaction<'_, Postgres>,
    integration: i64,
    title: &str,
    teacher: &str,
) -> Result<()> {
    let pending: Vec<(i64, i64)> = sqlx::query_as("UPDATE participantes_integracion SET notificado_en=now() WHERE integracion_id=$1 AND rol='ESTUDIANTE' AND usuario_id IS NOT NULL AND notificado_en IS NULL RETURNING id,usuario_id")
        .bind(integration)
        .fetch_all(&mut **tx)
        .await?;
    let message = format!(
        "Fuiste asignado al reto \"{title}\" en el curso de {teacher}. Revisa el cronograma de sesiones."
    );
    for (_, student) in pending {
        notify(
            tx,
            student,
            "EQUIPO_ASIGNADO",
            &message,
            &format!("integracion:{integration}:asignado"),
        )
        .await?;
    }
    Ok(())
}

/// GET: team and schedule of an integration the caller may see.
pub async fn show(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, false).await?;
    let visible = resources::visible_record(&state, &user, "integraciones_academicas", id).await;
    let participant: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM participantes_integracion WHERE integracion_id=$1 AND usuario_id=$2)")
        .bind(id).bind(user.id).fetch_one(&state.pool).await?;
    if visible.is_err() && !participant {
        return Err(AppError::not_found());
    }
    let team: Value = sqlx::query_scalar("SELECT jsonb_build_object('participantes', COALESCE((SELECT jsonb_agg(jsonb_build_object('rol',rol,'nombre',nombre,'documento',documento,'email',email,'facultad',facultad,'programa',programa,'cuenta',usuario_id IS NOT NULL) ORDER BY rol,nombre) FROM participantes_integracion WHERE integracion_id=$1),'[]'), 'sesiones', COALESCE((SELECT jsonb_agg(jsonb_build_object('tipo',tipo,'fecha_hora',fecha_hora,'enlace_reunion',COALESCE(enlace_reunion,''),'observaciones',observaciones) ORDER BY fecha_hora) FROM sesiones_reto_academico WHERE integracion_id=$1),'[]'))")
        .bind(id).fetch_one(&state.pool).await?;
    Ok(Json(team))
}

/// POST: the owner professor replaces the team and schedule while the
/// integration is editable (draft/rejected, or its reto open or running).
pub async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(team): Json<TeamInput>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    validate(&team)?;
    let mut tx = state.pool.begin().await?;
    let row: Option<(i64, i64, String, String, String)> = sqlx::query_as("SELECT i.reto_id,i.profesor_id,i.estado,r.estado,r.titulo FROM integraciones_academicas i JOIN retos r ON r.id=i.reto_id WHERE i.id=$1 FOR UPDATE OF i")
        .bind(id).fetch_optional(&mut *tx).await?;
    let (reto, owner, estado, reto_estado, titulo) = row.ok_or_else(AppError::not_found)?;
    if owner != user.id || user.rol != "PROFESOR" {
        return Err(AppError::forbidden());
    }
    if !(["borrador", "rechazada"].contains(&estado.as_str())
        || ["abierto", "en_curso"].contains(&reto_estado.as_str()))
    {
        return Err(AppError::bad(
            "Esta integración no puede editarse desde su estado actual",
        ));
    }
    save(&mut tx, id, reto, &team).await?;
    if estado == "publicada" {
        let teacher = format!("{} {}", user.first_name, user.last_name)
            .trim()
            .to_string();
        let teacher = if teacher.is_empty() {
            user.username.clone()
        } else {
            teacher
        };
        notify_students(&mut tx, id, &titulo, &teacher).await?;
    }
    tx.commit().await?;
    Ok(Json(json!({"ok": true})))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn persona(rol: &str) -> ParticipantInput {
        ParticipantInput {
            rol: rol.into(),
            nombre: "Ana".into(),
            documento: "1".into(),
            email: "ana@ean.edu.co".into(),
            ..Default::default()
        }
    }

    #[test]
    fn cada_rol_exige_sus_datos() {
        let mut equipo = TeamInput {
            participantes: vec![persona("ESTUDIANTE")],
            sesiones: vec![],
        };
        assert!(validate(&equipo).is_err(), "student needs a program");
        equipo.participantes[0].programa = "Sistemas".into();
        assert!(validate(&equipo).is_ok());
        equipo.participantes.push(persona("PROFESOR"));
        assert!(validate(&equipo).is_err(), "professor needs a faculty");
        equipo.participantes[1].facultad = "Ingeniería".into();
        assert!(validate(&equipo).is_ok());
        equipo.participantes[1].email = "sin-arroba".into();
        assert!(validate(&equipo).is_err());
    }
}
