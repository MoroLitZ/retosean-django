use crate::{
    AppState,
    auth::{self, User},
    domain,
    error::{AppError, Result},
    workflows::{StateInput, history, notify},
};

pub async fn challenge_options(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, false).await?;
    let mut tx = state.pool.begin().await?;
    integrated(&mut tx, id, &user).await?;
    let mut lists = serde_json::Map::new();
    for (name, roles) in [
        ("profesores", "{PROFESOR}"),
        ("estudiantes", "{ESTUDIANTE}"),
        ("expertos_invitados", "{EMPRESA,EXPERTO}"),
    ] {
        let options: Vec<Value> = sqlx::query_scalar("SELECT jsonb_build_object('value',id,'label',COALESCE(NULLIF(trim(first_name||' '||last_name),''),username),'selected',id=$2) FROM usuarios WHERE rol=ANY($1::text[]) ORDER BY first_name,last_name")
            .bind(roles).bind(user.id).fetch_all(&mut *tx).await?;
        lists.insert(name.into(), Value::Array(options));
    }
    Ok(Json(Value::Object(lists)))
}
/// solo_profesor + IntegracionAcademica(profesor=user): admins do not pass in Django.
async fn integrated(tx: &mut Transaction<'_, Postgres>, reto: i64, user: &User) -> Result<()> {
    let ok: bool = sqlx::query_scalar("SELECT $3='PROFESOR' AND EXISTS(SELECT 1 FROM integraciones_academicas i JOIN retos r ON r.id=i.reto_id WHERE i.reto_id=$1 AND i.profesor_id=$2 AND i.estado IN ('aprobada','publicada') AND r.eliminado_en IS NULL)")
        .bind(reto).bind(user.id).bind(&user.rol).fetch_one(&mut **tx).await?;
    if ok {
        Ok(())
    } else {
        Err(AppError::forbidden())
    }
}
/// `IntegracionAcademica.puede_editar_profesor`: the professor owns an integration
/// on the reto whose configuration is still editable (borrador/rechazada, or the
/// reto open/running). Returns the integration that owns the new academic team.
async fn own_integration(
    tx: &mut Transaction<'_, Postgres>,
    reto: i64,
    user: &User,
) -> Result<i64> {
    if user.rol != "PROFESOR" {
        return Err(AppError::forbidden());
    }
    let id: Option<i64> = sqlx::query_scalar(
        "SELECT i.id FROM integraciones_academicas i JOIN retos r ON r.id=i.reto_id WHERE i.reto_id=$1 AND i.profesor_id=$2 AND r.eliminado_en IS NULL AND (i.estado IN ('borrador','rechazada') OR r.estado IN ('abierto','en_curso')) ORDER BY i.id LIMIT 1",
    )
    .bind(reto)
    .bind(user.id)
    .fetch_optional(&mut **tx)
    .await?;
    id.ok_or_else(AppError::forbidden)
}
pub async fn options(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, false).await?;
    let row = crate::resources::visible_record(&state, &user, "hackathons", id).await?;
    let reto = row["reto_id"].as_i64().ok_or_else(AppError::not_found)?;
    let mut tx = state.pool.begin().await?;
    let managing = manager(&mut tx, reto, &user).await.is_ok();
    let jurors: Vec<Value> = if managing {
        sqlx::query_scalar("SELECT jsonb_build_object('value',id,'label',COALESCE(NULLIF(trim(first_name||' '||last_name),''),username)||' ('||username||')') FROM usuarios WHERE is_active AND rol IN ('EXPERTO','PROFESOR','EMPRESA','ADMIN') ORDER BY first_name,last_name").fetch_all(&mut *tx).await?
    } else {
        Vec::new()
    };
    let teams: Vec<Value> = if user.rol == "ESTUDIANTE" {
        sqlx::query_scalar("SELECT jsonb_build_object('value',e.id,'label',e.nombre) FROM equipos e JOIN miembros_equipo m ON m.equipo_id=e.id WHERE e.reto_id=$1 AND m.estudiante_id=$2 AND NOT EXISTS(SELECT 1 FROM inscripciones_hackaton i WHERE i.hackathon_id=$3 AND i.equipo_id=e.id) ORDER BY e.nombre").bind(reto).bind(user.id).bind(id).fetch_all(&mut *tx).await?
    } else {
        sqlx::query_scalar("SELECT jsonb_build_object('value',e.id,'label',e.nombre) FROM equipos e JOIN inscripciones_hackaton i ON i.equipo_id=e.id WHERE i.hackathon_id=$1 AND i.estado='ACEPTADA' ORDER BY e.nombre").bind(id).fetch_all(&mut *tx).await?
    };
    // equipos_sin_votar for the juror asking.
    let pending: Vec<Value> = sqlx::query_scalar("SELECT jsonb_build_object('value',e.id,'label',e.nombre) FROM inscripciones_hackaton i JOIN equipos e ON e.id=i.equipo_id WHERE i.hackathon_id=$1 AND i.estado='ACEPTADA' AND EXISTS(SELECT 1 FROM jurados_hackaton j WHERE j.hackathon_id=$1 AND j.usuario_id=$2) AND NOT EXISTS(SELECT 1 FROM votaciones_hackaton v JOIN jurados_hackaton j ON j.id=v.jurado_id WHERE v.hackathon_id=$1 AND j.usuario_id=$2 AND v.equipo_id=e.id) ORDER BY e.nombre")
        .bind(id).bind(user.id).fetch_all(&mut *tx).await?;
    Ok(Json(
        json!({"jurors":jurors,"teams":teams,"sin_votar":pending}),
    ))
}
use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};

async fn manager(tx: &mut Transaction<'_, Postgres>, reto: i64, user: &User) -> Result<()> {
    if user.admin() {
        return Ok(());
    }
    let teacher: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM integraciones_academicas i JOIN retos r ON r.id=i.reto_id AND r.eliminado_en IS NULL WHERE i.reto_id=$1 AND i.profesor_id=$2 AND i.estado IN ('aprobada','publicada'))",
    )
    .bind(reto)
    .bind(user.id)
    .fetch_one(&mut **tx)
    .await?;
    if user.rol == "PROFESOR" && teacher {
        Ok(())
    } else {
        Err(AppError::forbidden())
    }
}
async fn lock(tx: &mut Transaction<'_, Postgres>, id: i64) -> Result<(i64, String, bool)> {
    sqlx::query_as(
        "SELECT reto_id,estado,resultados_publicados FROM hackathons WHERE id=$1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(AppError::not_found)
}
async fn window(tx: &mut Transaction<'_, Postgres>, id: i64, kind: &str) -> Result<()> {
    let allowed: bool = match kind {
        // HCK-05: the vote fails closed — a dated, currently-vigent presentation
        // stage is required, otherwise the window that yields a winner never opens.
        "presentacion" => sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM etapas_hackaton WHERE hackathon_id=$1 AND tipo=$2 AND (inicia_en IS NOT NULL OR termina_en IS NOT NULL) AND (inicia_en IS NULL OR inicia_en<=now()) AND (termina_en IS NULL OR termina_en>=now()))").bind(id).bind(kind).fetch_one(&mut **tx).await?,
        // inscripcion fails open: without a stage the published hackathon stays open.
        _ => sqlx::query_scalar("SELECT NOT EXISTS(SELECT 1 FROM etapas_hackaton WHERE hackathon_id=$1 AND tipo=$2 AND ((inicia_en IS NOT NULL AND inicia_en>now()) OR (termina_en IS NOT NULL AND termina_en<now())))").bind(id).bind(kind).fetch_one(&mut **tx).await?,
    };
    if allowed {
        Ok(())
    } else {
        Err(AppError::bad(if kind == "presentacion" {
            "La etapa de presentación no está abierta"
        } else {
            "La etapa no está abierta"
        }))
    }
}
/// crear_equipo_academico: the academic team plus its operative mirror; students notified.
pub async fn team(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(reto): Path<i64>,
    Json(mut input): Json<AcademicTeamInput>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    validate_team(&state, &mut input).await?;
    let mut tx = state.pool.begin().await?;
    let integration = own_integration(&mut tx, reto, &user).await?;
    let name = input.nombre_equipo.trim();
    let academic: i64 = sqlx::query_scalar(
        "INSERT INTO equipos_reto_academico(reto_id,nombre_equipo,integracion_id) VALUES($1,$2,$3) RETURNING id",
    )
    .bind(reto)
    .bind(name)
    .bind(integration)
    .fetch_one(&mut *tx)
    .await?;
    for (table, list) in [
        ("profesores", &input.profesores),
        ("estudiantes", &input.estudiantes),
        ("expertos_invitados", &input.expertos_invitados),
    ] {
        sqlx::query(&format!("INSERT INTO equipos_reto_academico_{table}(equiporetoacademico_id,usuario_id) SELECT $1,u FROM unnest($2::bigint[]) u"))
            .bind(academic).bind(list).execute(&mut *tx).await?;
    }
    sync_mirror(&mut tx, academic, reto, None, name, &input.estudiantes).await?;
    let mirror: i64 =
        sqlx::query_scalar("SELECT equipo_espejo_id FROM equipos_reto_academico WHERE id=$1")
            .bind(academic)
            .fetch_one(&mut *tx)
            .await?;
    history(
        &mut tx,
        ("participaciones", "equipo"),
        mirror,
        "",
        "creado",
        user.id,
        name,
    )
    .await?;
    let title: String = sqlx::query_scalar("SELECT titulo FROM retos WHERE id=$1")
        .bind(reto)
        .fetch_one(&mut *tx)
        .await?;
    for student in &input.estudiantes {
        notify(
            &mut tx,
            *student,
            "EQUIPO_ASIGNADO",
            &format!("Fuiste incorporado al equipo \"{name}\" del reto \"{title}\"."),
            &format!("equipo:{academic}:asignado"),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(Json(json!({
        "id": mirror,
        "equipo_academico": academic,
        "mensaje": format!("Equipo académico configurado y {} estudiante(s) notificado(s).", input.estudiantes.len()),
    })))
}
/// editar/eliminar_equipo_academico: only the owning professor (or an admin)
/// whose integration is still editable — Django's `_puede_gestionar_equipo`.
async fn academic_team(
    tx: &mut Transaction<'_, Postgres>,
    id: i64,
    user: &User,
) -> Result<(i64, Option<i64>)> {
    let row: Option<(i64, Option<i64>)> = sqlx::query_as(
        "SELECT reto_id,equipo_espejo_id FROM equipos_reto_academico WHERE id=$1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?;
    let team = row.ok_or_else(AppError::not_found)?;
    if user.admin() {
        return Ok(team);
    }
    let owner: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM equipos_reto_academico q JOIN integraciones_academicas i ON i.id=q.integracion_id JOIN retos r ON r.id=q.reto_id AND r.eliminado_en IS NULL WHERE q.id=$1 AND q.integracion_id IS NOT NULL AND i.profesor_id=$2 AND (i.estado IN ('borrador','rechazada') OR r.estado IN ('abierto','en_curso')))",
    )
    .bind(id)
    .bind(user.id)
    .fetch_one(&mut **tx)
    .await?;
    if owner {
        Ok(team)
    } else {
        Err(AppError::not_found())
    }
}
/// EquipoRetoAcademicoForm: each list only takes users of its roles.
async fn validate_team(state: &AppState, input: &mut AcademicTeamInput) -> Result<()> {
    domain::validate_text(&input.nombre_equipo, 100, "Nombre del equipo")?;
    for (list, roles) in [
        (&mut input.profesores, &["PROFESOR"][..]),
        (&mut input.estudiantes, &["ESTUDIANTE"][..]),
        (&mut input.expertos_invitados, &["EMPRESA", "EXPERTO"][..]),
    ] {
        list.sort_unstable();
        list.dedup();
        if list.len() > 50 {
            return Err(AppError::bad("Máximo 50 integrantes por lista"));
        }
        let valid: i64 =
            sqlx::query_scalar("SELECT count(*) FROM usuarios WHERE id=ANY($1) AND rol=ANY($2)")
                .bind(&*list)
                .bind(roles)
                .fetch_one(&state.pool)
                .await?;
        if valid != list.len() as i64 {
            return Err(AppError::bad(
                "No se pudo actualizar el equipo. Revisa los datos del formulario.",
            ));
        }
    }
    Ok(())
}
#[derive(Deserialize)]
pub struct AcademicTeamInput {
    nombre_equipo: String,
    #[serde(default)]
    profesores: Vec<i64>,
    #[serde(default)]
    estudiantes: Vec<i64>,
    #[serde(default)]
    expertos_invitados: Vec<i64>,
}
pub async fn edit_team(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(mut input): Json<AcademicTeamInput>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    validate_team(&state, &mut input).await?;
    let mut tx = state.pool.begin().await?;
    let (reto, mirror) = academic_team(&mut tx, id, &user).await?;
    let closed: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM hackathons WHERE reto_id=$1 AND (resultados_publicados OR estado='finalizado'))")
        .bind(reto)
        .fetch_one(&mut *tx)
        .await?;
    if closed {
        return Err(AppError::bad(
            "No se puede editar el equipo: el hackathon ya finalizó.",
        ));
    }
    sqlx::query("UPDATE equipos_reto_academico SET nombre_equipo=$2 WHERE id=$1")
        .bind(id)
        .bind(input.nombre_equipo.trim())
        .execute(&mut *tx)
        .await?;
    for (table, list) in [
        ("profesores", &input.profesores),
        ("estudiantes", &input.estudiantes),
        ("expertos_invitados", &input.expertos_invitados),
    ] {
        sqlx::query(&format!("DELETE FROM equipos_reto_academico_{table} WHERE equiporetoacademico_id=$1 AND NOT usuario_id=ANY($2)"))
            .bind(id).bind(list).execute(&mut *tx).await?;
        sqlx::query(&format!("INSERT INTO equipos_reto_academico_{table}(equiporetoacademico_id,usuario_id) SELECT $1,u FROM unnest($2::bigint[]) u ON CONFLICT DO NOTHING"))
            .bind(id).bind(list).execute(&mut *tx).await?;
    }
    sync_mirror(
        &mut tx,
        id,
        reto,
        mirror,
        input.nombre_equipo.trim(),
        &input.estudiantes,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({"id":id})))
}
/// sincronizar_equipo_academico: keep the operative participaciones.Equipo in step.
async fn sync_mirror(
    tx: &mut Transaction<'_, Postgres>,
    academic: i64,
    reto: i64,
    mirror: Option<i64>,
    name: &str,
    students: &[i64],
) -> Result<()> {
    let mirror = match mirror {
        Some(mirror) => {
            sqlx::query("UPDATE equipos SET nombre=$2 WHERE id=$1 AND nombre<>$2")
                .bind(mirror)
                .bind(name)
                .execute(&mut **tx)
                .await?;
            mirror
        }
        None => {
            let mirror: i64 = sqlx::query_scalar(
                "INSERT INTO equipos(reto_id,nombre,creado_en) VALUES($1,$2,now()) RETURNING id",
            )
            .bind(reto)
            .bind(name)
            .fetch_one(&mut **tx)
            .await?;
            sqlx::query("UPDATE equipos_reto_academico SET equipo_espejo_id=$2 WHERE id=$1")
                .bind(academic)
                .bind(mirror)
                .execute(&mut **tx)
                .await?;
            mirror
        }
    };
    sqlx::query("DELETE FROM miembros_equipo WHERE equipo_id=$1 AND NOT estudiante_id=ANY($2)")
        .bind(mirror)
        .bind(students)
        .execute(&mut **tx)
        .await?;
    sqlx::query("INSERT INTO miembros_equipo(equipo_id,estudiante_id,rol,fecha_ingreso) SELECT $1,s,'MIEMBRO',now() FROM unnest($2::bigint[]) s WHERE NOT EXISTS(SELECT 1 FROM miembros_equipo WHERE equipo_id=$1 AND estudiante_id=s) AND NOT EXISTS(SELECT 1 FROM miembros_equipo m JOIN equipos e ON e.id=m.equipo_id WHERE e.reto_id=$3 AND m.estudiante_id=s AND m.equipo_id<>$1)")
        .bind(mirror).bind(students).bind(reto).execute(&mut **tx).await?;
    // The first member by id leads, the rest are members.
    sqlx::query("UPDATE miembros_equipo SET rol=CASE WHEN estudiante_id=$2 THEN 'LIDER' ELSE 'MIEMBRO' END WHERE equipo_id=$1")
        .bind(mirror).bind(students.first().copied()).execute(&mut **tx).await?;
    Ok(())
}
pub async fn show_team(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, false).await?;
    let mut tx = state.pool.begin().await?;
    academic_team(&mut tx, id, &user).await?;
    let name: String =
        sqlx::query_scalar("SELECT nombre_equipo FROM equipos_reto_academico WHERE id=$1")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
    let mut lists = serde_json::Map::new();
    for (table, roles) in [
        ("profesores", "{PROFESOR}"),
        ("estudiantes", "{ESTUDIANTE}"),
        ("expertos_invitados", "{EMPRESA,EXPERTO}"),
    ] {
        let options: Vec<Value> = sqlx::query_scalar(&format!("SELECT jsonb_build_object('value',u.id,'label',COALESCE(NULLIF(trim(u.first_name||' '||u.last_name),''),u.username),'selected',EXISTS(SELECT 1 FROM equipos_reto_academico_{table} m WHERE m.equiporetoacademico_id=$1 AND m.usuario_id=u.id)) FROM usuarios u WHERE u.rol=ANY($2::text[]) ORDER BY u.first_name,u.last_name"))
            .bind(id).bind(roles).fetch_all(&mut *tx).await?;
        lists.insert(table.into(), Value::Array(options));
    }
    Ok(Json(json!({"nombre_equipo":name,"opciones":lists})))
}
pub async fn delete_team(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    let mut tx = state.pool.begin().await?;
    let (reto, mirror) = academic_team(&mut tx, id, &user).await?;
    let closed: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM hackathons WHERE reto_id=$1 AND (resultados_publicados OR estado='finalizado'))")
        .bind(reto)
        .fetch_one(&mut *tx)
        .await?;
    if closed {
        return Err(AppError::bad(
            "No se puede eliminar el equipo: el hackathon ya finalizó.",
        ));
    }
    let name: String =
        sqlx::query_scalar("SELECT nombre_equipo FROM equipos_reto_academico WHERE id=$1")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
    for table in ["profesores", "estudiantes", "expertos_invitados"] {
        sqlx::query(&format!(
            "DELETE FROM equipos_reto_academico_{table} WHERE equiporetoacademico_id=$1"
        ))
        .bind(id)
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query("DELETE FROM equipos_reto_academico WHERE id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    if let Some(mirror) = mirror {
        // Django's Equipo.delete(): deliverables keep their row (SET_NULL),
        // votes, hackathon registrations and members go with the team (CASCADE).
        sqlx::query("UPDATE entregables SET equipo_id=NULL WHERE equipo_id=$1")
            .bind(mirror)
            .execute(&mut *tx)
            .await?;
        for table in [
            "votaciones_hackaton",
            "inscripciones_hackaton",
            "miembros_equipo",
        ] {
            sqlx::query(&format!("DELETE FROM {table} WHERE equipo_id=$1"))
                .bind(mirror)
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query(
            "UPDATE equipos_reto_academico SET equipo_espejo_id=NULL WHERE equipo_espejo_id=$1",
        )
        .bind(mirror)
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM equipos WHERE id=$1")
            .bind(mirror)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(Json(
        json!({"ok":true,"mensaje":format!("Equipo académico \"{name}\" eliminado.")}),
    ))
}
#[derive(Deserialize)]
pub struct RulesInput {
    #[serde(default)]
    reglas: String,
}
pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(reto): Path<i64>,
    Json(input): Json<RulesInput>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    domain::max_chars(&input.reglas, 20000, "Reglas")?;
    let mut tx = state.pool.begin().await?;
    manager(&mut tx, reto, &user).await?;
    let kind: Option<String> = sqlx::query_scalar(
        "SELECT tipo FROM retos WHERE id=$1 AND eliminado_en IS NULL FOR UPDATE",
    )
    .bind(reto)
    .fetch_optional(&mut *tx)
    .await?;
    if kind.as_deref() != Some("hackathon") {
        return Err(AppError::bad("El reto debe ser de tipo hackathon"));
    }
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM hackathons WHERE reto_id=$1)")
            .bind(reto)
            .fetch_one(&mut *tx)
            .await?;
    if exists {
        return Err(AppError::bad(
            "Este reto ya tiene un hackathon configurado.",
        ));
    }
    let id:i64=sqlx::query_scalar("INSERT INTO hackathons(reto_id,reglas,estado,resultados_publicados,creado_en,actualizado_en) VALUES($1,$2,'borrador',false,now(),now()) RETURNING id").bind(reto).bind(&input.reglas).fetch_one(&mut *tx).await?;
    // crear_etapas_por_defecto: the backlog's four stages, without dates.
    sqlx::query("INSERT INTO etapas_hackaton(hackathon_id,tipo,titulo,orden) SELECT $1,t,titulo,n FROM unnest(ARRAY['inscripcion','desarrollo','presentacion','premiacion'],ARRAY['Inscripción','Desarrollo','Presentación','Premiación']) WITH ORDINALITY k(t,titulo,n) WHERE NOT EXISTS(SELECT 1 FROM etapas_hackaton WHERE hackathon_id=$1)")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    history(
        &mut tx,
        ("hackaton", "hackathon"),
        id,
        "",
        "borrador",
        user.id,
        "",
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({"id":id})))
}
pub async fn state(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<StateInput>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    let mut tx = state.pool.begin().await?;
    let (reto, from, _) = lock(&mut tx, id).await?;
    manager(&mut tx, reto, &user).await?;
    let valid = matches!(
        (from.as_str(), input.estado.as_str()),
        ("borrador", "publicado" | "cancelado")
            | ("publicado", "en_curso" | "cancelado")
            | ("en_curso", "finalizado" | "cancelado")
    );
    domain::max_chars(&input.comentario, 4000, "Comentario")?;
    if !valid {
        return Err(AppError::bad("Transición de hackathon inválida"));
    }
    if input.estado == "finalizado" {
        let comment = if input.comentario.is_empty() {
            "Resultados publicados."
        } else {
            input.comentario.as_str()
        };
        crate::event::publish_results(&mut tx, id, &from, user.id, comment).await?;
    } else {
        sqlx::query("UPDATE hackathons SET estado=$2,actualizado_en=now() WHERE id=$1")
            .bind(id)
            .bind(&input.estado)
            .execute(&mut *tx)
            .await?;
        history(
            &mut tx,
            ("hackaton", "hackathon"),
            id,
            &from,
            &input.estado,
            user.id,
            &input.comentario,
        )
        .await?;
    }
    if input.estado == "publicado" {
        // publicar: the reto's accepted applicants are invited to register.
        let title: String = sqlx::query_scalar("SELECT titulo FROM retos WHERE id=$1")
            .bind(reto)
            .fetch_one(&mut *tx)
            .await?;
        let students: Vec<i64> = sqlx::query_scalar("SELECT DISTINCT p.estudiante_id FROM postulaciones p JOIN usuarios u ON u.id=p.estudiante_id WHERE p.reto_id=$1 AND p.estado='ACEPTADA' AND u.is_active")
            .bind(reto)
            .fetch_all(&mut *tx)
            .await?;
        let message = format!(
            "El hackathon del reto '{title}' ya está publicado. Inscribe a tu equipo desde la plataforma."
        );
        for student in students {
            notify(
                &mut tx,
                student,
                "HACKATON_PUBLICADO",
                &message,
                &format!("hackaton:{id}:publicado"),
            )
            .await?;
        }
    }
    tx.commit().await?;
    Ok(Json(json!({"estado":input.estado})))
}
#[derive(Deserialize)]
pub struct StageInput {
    tipo: String,
    titulo: String,
    inicia_en: Option<chrono::DateTime<chrono::Utc>>,
    termina_en: Option<chrono::DateTime<chrono::Utc>>,
    orden: i16,
}
pub async fn stage(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<StageInput>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    domain::validate_text(&input.titulo, 150, "Título")?;
    if !["inscripcion", "desarrollo", "presentacion", "premiacion"].contains(&input.tipo.as_str())
        || input.orden < 0
        || input
            .inicia_en
            .zip(input.termina_en)
            .is_some_and(|(a, b)| a > b)
    {
        return Err(AppError::bad("Etapa inválida"));
    }
    crate::domain::validate_date(
        input.inicia_en.map(|d| d.date_naive()),
        "Inicio de la etapa",
    )?;
    crate::domain::validate_date(input.termina_en.map(|d| d.date_naive()), "Fin de la etapa")?;
    let mut tx = state.pool.begin().await?;
    let (reto, status, _) = lock(&mut tx, id).await?;
    manager(&mut tx, reto, &user).await?;
    if ["finalizado", "cancelado"].contains(&status.as_str()) {
        return Err(AppError::bad("Hackathon cerrado"));
    }
    sqlx::query("DELETE FROM etapas_hackaton WHERE hackathon_id=$1 AND tipo=$2")
        .bind(id)
        .bind(&input.tipo)
        .execute(&mut *tx)
        .await?;
    let stage:i64=sqlx::query_scalar("INSERT INTO etapas_hackaton(hackathon_id,tipo,titulo,inicia_en,termina_en,orden) VALUES($1,$2,$3,$4,$5,$6) RETURNING id").bind(id).bind(&input.tipo).bind(&input.titulo).bind(input.inicia_en).bind(input.termina_en).bind(input.orden).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(json!({"id":stage})))
}
#[derive(Deserialize)]
pub struct JurorInput {
    usuario_id: i64,
    #[serde(default)]
    especialidad: String,
}
pub async fn juror(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<JurorInput>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    if input.especialidad.chars().count() > 150 {
        return Err(AppError::bad("Especialidad demasiado larga"));
    }
    let mut tx = state.pool.begin().await?;
    let (reto, status, _) = lock(&mut tx, id).await?;
    manager(&mut tx, reto, &user).await?;
    if ["finalizado", "cancelado"].contains(&status.as_str()) {
        return Err(AppError::bad("Hackathon cerrado"));
    }
    let person:Option<(String,String)>=sqlx::query_as("SELECT left(COALESCE(NULLIF(trim(first_name||' '||last_name),''),username),150),email FROM usuarios WHERE id=$1 AND is_active AND rol IN ('EXPERTO','PROFESOR','EMPRESA','ADMIN')").bind(input.usuario_id).fetch_optional(&mut *tx).await?;
    let (name, email) = person.ok_or_else(|| {
        AppError::bad(
            "Selecciona un usuario activo con rol de experto, profesor, empresa o administrador",
        )
    })?;
    let already: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM jurados_hackaton WHERE hackathon_id=$1 AND usuario_id=$2)",
    )
    .bind(id)
    .bind(input.usuario_id)
    .fetch_one(&mut *tx)
    .await?;
    if already {
        return Err(AppError::bad("El usuario ya es jurado"));
    }
    let juror:i64=sqlx::query_scalar("INSERT INTO jurados_hackaton(hackathon_id,usuario_id,nombre,email,especialidad) VALUES($1,$2,$3,$4,$5) RETURNING id").bind(id).bind(input.usuario_id).bind(name).bind(email).bind(&input.especialidad).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(json!({"id":juror})))
}
#[derive(Deserialize)]
pub struct TeamId {
    equipo_id: i64,
}
pub async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<TeamId>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    if user.rol != "ESTUDIANTE" {
        return Err(AppError::forbidden());
    }
    let mut tx = state.pool.begin().await?;
    let (reto, status, _) = lock(&mut tx, id).await?;
    if !["publicado", "en_curso"].contains(&status.as_str()) {
        return Err(AppError::bad("Inscripciones cerradas"));
    }
    window(&mut tx, id, "inscripcion").await?;
    let member:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM miembros_equipo m JOIN equipos e ON e.id=m.equipo_id WHERE m.equipo_id=$1 AND m.estudiante_id=$2 AND e.reto_id=$3)").bind(input.equipo_id).bind(user.id).bind(reto).fetch_one(&mut *tx).await?;
    if !member {
        return Err(AppError::forbidden());
    }
    let accepted:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM postulaciones WHERE reto_id=$1 AND estudiante_id=$2 AND estado='ACEPTADA')").bind(reto).bind(user.id).fetch_one(&mut *tx).await?;
    if !accepted {
        return Err(AppError::bad(
            "Solo pueden inscribirse estudiantes con postulación aceptada.",
        ));
    }
    let registered: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM inscripciones_hackaton WHERE hackathon_id=$1 AND equipo_id=$2)")
        .bind(id)
        .bind(input.equipo_id)
        .fetch_one(&mut *tx)
        .await?;
    if registered {
        return Err(AppError::bad(
            "Este equipo ya está inscrito en el hackathon.",
        ));
    }
    // HCK-08: one team per student per hackathon; another registered team closes it.
    let member_of_registered: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM inscripciones_hackaton i JOIN miembros_equipo m ON m.equipo_id=i.equipo_id WHERE i.hackathon_id=$1 AND m.estudiante_id=$2)",
    )
    .bind(id)
    .bind(user.id)
    .fetch_one(&mut *tx)
    .await?;
    if member_of_registered {
        return Err(AppError::bad(
            "Ya estás inscrito con otro equipo en este hackathon.",
        ));
    }
    let registration:i64=sqlx::query_scalar("INSERT INTO inscripciones_hackaton(hackathon_id,equipo_id,estado,inscrito_por_id,creado_en) VALUES($1,$2,'PENDIENTE',$3,now()) RETURNING id").bind(id).bind(input.equipo_id).bind(user.id).fetch_one(&mut *tx).await?;
    history(
        &mut tx,
        ("hackaton", "inscripcionhackaton"),
        registration,
        "",
        "PENDIENTE",
        user.id,
        "",
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({"id":registration})))
}
pub async fn registration_state(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<StateInput>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    let mut tx = state.pool.begin().await?;
    let hack: Option<i64> =
        sqlx::query_scalar("SELECT hackathon_id FROM inscripciones_hackaton WHERE id=$1")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    let hack = hack.ok_or_else(AppError::not_found)?;
    let (reto, status, _) = lock(&mut tx, hack).await?;
    manager(&mut tx, reto, &user).await?;
    if ["finalizado", "cancelado"].contains(&status.as_str()) {
        return Err(AppError::bad("Hackathon cerrado"));
    }
    // Accepting a registration respects the inscription window; without a dated
    // stage the window fails open (as `register` and the parity tests assert).
    if input.estado == "ACEPTADA" {
        window(&mut tx, hack, "inscripcion").await?;
    }
    let from: String =
        sqlx::query_scalar("SELECT estado FROM inscripciones_hackaton WHERE id=$1 FOR UPDATE")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
    domain::transition("MAQUINA_INSCRIPCION", &from, &input.estado)?;
    sqlx::query("UPDATE inscripciones_hackaton SET estado=$2 WHERE id=$1")
        .bind(id)
        .bind(&input.estado)
        .execute(&mut *tx)
        .await?;
    history(
        &mut tx,
        ("hackaton", "inscripcionhackaton"),
        id,
        &from,
        &input.estado,
        user.id,
        &input.comentario,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({"estado":input.estado})))
}
#[derive(Deserialize)]
pub struct VoteInput {
    equipo_id: i64,
    puntaje: String,
    #[serde(default)]
    comentario: String,
}
pub async fn vote(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<VoteInput>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    let score = input
        .puntaje
        .parse::<rust_decimal::Decimal>()
        .map_err(|_| AppError::bad("El puntaje debe ser un número entre 0 y 5."))?;
    domain::max_chars(&input.comentario, 4000, "Comentario")?;
    if score < rust_decimal::Decimal::ZERO
        || score > rust_decimal::Decimal::from(5)
        || score.scale() > 2
    {
        return Err(AppError::bad("El puntaje debe estar entre 0 y 5."));
    }
    let mut tx = state.pool.begin().await?;
    let (_, status, _) = lock(&mut tx, id).await?;
    if !["publicado", "en_curso"].contains(&status.as_str()) {
        return Err(AppError::bad("Votaciones cerradas"));
    }
    // As in Django, a non-juror is rejected before the presentation window,
    // so a missing stage does not mask the "not a juror" response.
    let juror:Option<i64>=sqlx::query_scalar("SELECT id FROM jurados_hackaton WHERE hackathon_id=$1 AND usuario_id=$2 ORDER BY id LIMIT 1").bind(id).bind(user.id).fetch_optional(&mut *tx).await?;
    let juror = juror.ok_or_else(AppError::forbidden)?;
    window(&mut tx, id, "presentacion").await?;
    let accepted:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM inscripciones_hackaton WHERE hackathon_id=$1 AND equipo_id=$2 AND estado='ACEPTADA')").bind(id).bind(input.equipo_id).fetch_one(&mut *tx).await?;
    if !accepted {
        return Err(AppError::bad("El equipo no tiene inscripción aceptada"));
    }
    // Conflict of interest: a juror never scores a team they lead, belong to or
    // mentor (owning professor, co-professor, student, member, invited expert).
    let involved: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM miembros_equipo m WHERE m.equipo_id=$1 AND m.estudiante_id=$2) \
         OR EXISTS(SELECT 1 FROM equipos_reto_academico q WHERE q.equipo_espejo_id=$1 AND ( \
            EXISTS(SELECT 1 FROM integraciones_academicas i WHERE i.id=q.integracion_id AND i.profesor_id=$2) \
            OR EXISTS(SELECT 1 FROM equipos_reto_academico_profesores p WHERE p.equiporetoacademico_id=q.id AND p.usuario_id=$2) \
            OR EXISTS(SELECT 1 FROM equipos_reto_academico_estudiantes s WHERE s.equiporetoacademico_id=q.id AND s.usuario_id=$2) \
            OR EXISTS(SELECT 1 FROM equipos_reto_academico_expertos_invitados x WHERE x.equiporetoacademico_id=q.id AND x.usuario_id=$2)))",
    )
    .bind(input.equipo_id)
    .bind(user.id)
    .fetch_one(&mut *tx)
    .await?;
    if involved {
        return Err(AppError(
            axum::http::StatusCode::FORBIDDEN,
            "No puedes votar por un equipo que diriges, integras o asesoras.".into(),
        ));
    }
    let voted:i64=sqlx::query_scalar("INSERT INTO votaciones_hackaton(hackathon_id,jurado_id,equipo_id,puntaje,comentario,fecha) VALUES($1,$2,$3,$4,$5,now()) ON CONFLICT(jurado_id,equipo_id) DO UPDATE SET puntaje=excluded.puntaje,comentario=excluded.comentario,fecha=now() RETURNING id").bind(id).bind(juror).bind(input.equipo_id).bind(score).bind(&input.comentario).fetch_one(&mut *tx).await?;
    history(
        &mut tx,
        ("hackaton", "votacionhackaton"),
        voted,
        "",
        "registrada",
        user.id,
        &input.comentario,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({"id":voted})))
}
pub async fn ranking(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, false).await?;
    crate::resources::visible_record(&state, &user, "hackathons", id).await?;
    crate::event::advance(&state.pool, id).await?;
    let (published, reto): (bool, i64) =
        sqlx::query_as("SELECT resultados_publicados,reto_id FROM hackathons WHERE id=$1")
            .bind(id)
            .fetch_one(&state.pool)
            .await?;
    if !published && !user.admin() {
        // Django: managers (_puede_gestionar) and jurors (_jurado_de) see it early.
        let juror: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM jurados_hackaton WHERE hackathon_id=$1 AND usuario_id=$2)",
        )
        .bind(id)
        .bind(user.id)
        .fetch_one(&state.pool)
        .await?;
        let managing = if juror {
            false
        } else {
            let mut tx = state.pool.begin().await?;
            manager(&mut tx, reto, &user).await.is_ok()
        };
        if !juror && !managing {
            return Err(AppError::forbidden());
        }
    }
    // services.ranking: only accepted teams score; accepted teams without votes
    // close the table at zero (HCK-02/HCK-10). By average, then votes, then name.
    let rows: Vec<Value> = sqlx::query_scalar("SELECT jsonb_build_object('posicion',row_number() OVER (ORDER BY coalesce(avg(v.puntaje),0) DESC,count(v.id) DESC,e.nombre),'equipo_id',e.id,'equipo',e.nombre,'promedio',coalesce(round(avg(v.puntaje),2),0),'total',coalesce(sum(v.puntaje),0),'votos',count(v.id)) FROM inscripciones_hackaton i JOIN equipos e ON e.id=i.equipo_id LEFT JOIN votaciones_hackaton v ON v.hackathon_id=i.hackathon_id AND v.equipo_id=i.equipo_id WHERE i.hackathon_id=$1 AND i.estado='ACEPTADA' GROUP BY e.id,e.nombre ORDER BY coalesce(avg(v.puntaje),0) DESC,count(v.id) DESC,e.nombre")
        .bind(id).fetch_all(&state.pool).await?;
    Ok(Json(json!({"ranking":rows})))
}
