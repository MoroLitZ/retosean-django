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
    manager(&mut tx, id, &user).await?;
    let students:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('value',u.id,'label',trim(u.first_name||' '||u.last_name)||' ('||u.username||')') FROM usuarios u JOIN postulaciones p ON p.estudiante_id=u.id WHERE p.reto_id=$1 AND p.estado='ACEPTADA' AND u.is_active ORDER BY u.first_name,u.last_name").bind(id).fetch_all(&mut *tx).await?;
    Ok(Json(json!({"students":students})))
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
        sqlx::query_scalar("SELECT jsonb_build_object('value',id,'label',trim(first_name||' '||last_name)||' ('||username||')') FROM usuarios WHERE is_active AND rol IN ('EXPERTO','PROFESOR') ORDER BY first_name,last_name").fetch_all(&mut *tx).await?
    } else {
        Vec::new()
    };
    let teams: Vec<Value> = if user.rol == "ESTUDIANTE" {
        sqlx::query_scalar("SELECT jsonb_build_object('value',e.id,'label',e.nombre) FROM equipos e JOIN miembros_equipo m ON m.equipo_id=e.id WHERE e.reto_id=$1 AND m.estudiante_id=$2 ORDER BY e.nombre").bind(reto).bind(user.id).fetch_all(&mut *tx).await?
    } else {
        sqlx::query_scalar("SELECT jsonb_build_object('value',e.id,'label',e.nombre) FROM equipos e JOIN inscripciones_hackaton i ON i.equipo_id=e.id WHERE i.hackathon_id=$1 AND i.estado='ACEPTADA' ORDER BY e.nombre").bind(id).fetch_all(&mut *tx).await?
    };
    Ok(Json(json!({"jurors":jurors,"teams":teams})))
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
        "SELECT EXISTS(SELECT 1 FROM integraciones_academicas WHERE reto_id=$1 AND profesor_id=$2)",
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
    let allowed:bool=sqlx::query_scalar("SELECT NOT EXISTS(SELECT 1 FROM etapas_hackaton WHERE hackathon_id=$1 AND tipo=$2 AND ((inicia_en IS NOT NULL AND inicia_en>now()) OR (termina_en IS NOT NULL AND termina_en<now())))").bind(id).bind(kind).fetch_one(&mut **tx).await?;
    if allowed {
        Ok(())
    } else {
        Err(AppError::bad("La etapa no está abierta"))
    }
}
#[derive(Deserialize)]
pub struct TeamInput {
    nombre: String,
    estudiantes: Vec<i64>,
}
pub async fn team(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(reto): Path<i64>,
    Json(mut input): Json<TeamInput>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    domain::validate_text(&input.nombre, 100, "Nombre del equipo")?;
    input.estudiantes.sort_unstable();
    input.estudiantes.dedup();
    if input.estudiantes.is_empty() || input.estudiantes.len() > 50 {
        return Err(AppError::bad("Selecciona entre 1 y 50 estudiantes"));
    }
    let mut tx = state.pool.begin().await?;
    let active:Option<i64>=sqlx::query_scalar("SELECT id FROM retos WHERE id=$1 AND eliminado_en IS NULL AND estado IN ('aprobado','abierto','en_curso','pausado') FOR UPDATE").bind(reto).fetch_optional(&mut *tx).await?;
    if active.is_none() {
        return Err(AppError::bad("El reto no admite equipos"));
    }
    manager(&mut tx, reto, &user).await?;
    let accepted:i64=sqlx::query_scalar("SELECT count(*) FROM postulaciones p JOIN usuarios u ON u.id=p.estudiante_id WHERE p.reto_id=$1 AND p.estudiante_id=ANY($2) AND p.estado='ACEPTADA' AND u.is_active AND u.rol='ESTUDIANTE'").bind(reto).bind(&input.estudiantes).fetch_one(&mut *tx).await?;
    if accepted != input.estudiantes.len() as i64 {
        return Err(AppError::bad(
            "Todos los integrantes deben tener postulación aceptada",
        ));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO equipos(reto_id,nombre,creado_en) VALUES($1,$2,now()) RETURNING id",
    )
    .bind(reto)
    .bind(&input.nombre)
    .fetch_one(&mut *tx)
    .await?;
    let academic:i64=sqlx::query_scalar("INSERT INTO equipos_reto_academico(reto_id,nombre_equipo,equipo_espejo_id) VALUES($1,$2,$3) RETURNING id").bind(reto).bind(&input.nombre).bind(id).fetch_one(&mut *tx).await?;
    if user.rol == "PROFESOR" {
        sqlx::query("INSERT INTO equipos_reto_academico_profesores(equiporetoacademico_id,usuario_id) VALUES($1,$2)").bind(academic).bind(user.id).execute(&mut *tx).await?;
    }
    for (index, student) in input.estudiantes.iter().enumerate() {
        sqlx::query("INSERT INTO miembros_equipo(equipo_id,estudiante_id,rol,fecha_ingreso) VALUES($1,$2,$3,now())").bind(id).bind(student).bind(if index==0{"LIDER"}else{"MIEMBRO"}).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO equipos_reto_academico_estudiantes(equiporetoacademico_id,usuario_id) VALUES($1,$2)").bind(academic).bind(student).execute(&mut *tx).await?;
    }
    history(
        &mut tx,
        ("participaciones", "equipo"),
        id,
        "",
        "creado",
        user.id,
        &input.nombre,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({"id":id,"equipo_academico":academic})))
}
/// editar/eliminar_equipo_academico: only a professor integrated in the reto.
async fn academic_team(
    tx: &mut Transaction<'_, Postgres>,
    id: i64,
    user: &User,
) -> Result<(i64, Option<i64>)> {
    let row: Option<(i64, Option<i64>)> = sqlx::query_as("SELECT q.reto_id,q.equipo_espejo_id FROM equipos_reto_academico q WHERE q.id=$1 AND $3='PROFESOR' AND EXISTS(SELECT 1 FROM integraciones_academicas i WHERE i.reto_id=q.reto_id AND i.profesor_id=$2) FOR UPDATE")
        .bind(id).bind(user.id).bind(&user.rol).fetch_optional(&mut **tx).await?;
    row.ok_or_else(AppError::not_found)
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
    domain::validate_text(&input.nombre_equipo, 100, "Nombre del equipo")?;
    // EquipoRetoAcademicoForm choices: each list only takes users of its roles.
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
    let mut tx = state.pool.begin().await?;
    let (reto, mirror) = academic_team(&mut tx, id, &user).await?;
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
    sqlx::query("INSERT INTO miembros_equipo(equipo_id,estudiante_id,rol,fecha_ingreso) SELECT $1,s,'MIEMBRO',now() FROM unnest($2::bigint[]) s WHERE NOT EXISTS(SELECT 1 FROM miembros_equipo WHERE equipo_id=$1 AND estudiante_id=s)")
        .bind(mirror).bind(students).execute(&mut **tx).await?;
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
    let (_, mirror) = academic_team(&mut tx, id, &user).await?;
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
    if !user.admin() {
        return Err(AppError::forbidden());
    }
    if input.reglas.len() > 20000 {
        return Err(AppError::bad("Reglas demasiado largas"));
    }
    let mut tx = state.pool.begin().await?;
    let kind: Option<String> = sqlx::query_scalar(
        "SELECT tipo FROM retos WHERE id=$1 AND eliminado_en IS NULL FOR UPDATE",
    )
    .bind(reto)
    .fetch_optional(&mut *tx)
    .await?;
    if kind.as_deref() != Some("hackathon") {
        return Err(AppError::bad("El reto debe ser de tipo hackathon"));
    }
    let id:i64=sqlx::query_scalar("INSERT INTO hackathons(reto_id,reglas,estado,resultados_publicados,creado_en,actualizado_en) VALUES($1,$2,'borrador',false,now(),now()) RETURNING id").bind(reto).bind(&input.reglas).fetch_one(&mut *tx).await?;
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
    if !valid || input.comentario.len() > 4000 {
        return Err(AppError::bad("Transición de hackathon inválida"));
    }
    sqlx::query("UPDATE hackathons SET estado=$2,resultados_publicados=($2='finalizado'),actualizado_en=now() WHERE id=$1").bind(id).bind(&input.estado).execute(&mut *tx).await?;
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
    let members:Vec<i64>=sqlx::query_scalar("SELECT DISTINCT m.estudiante_id FROM miembros_equipo m JOIN inscripciones_hackaton i ON i.equipo_id=m.equipo_id WHERE i.hackathon_id=$1 AND i.estado='ACEPTADA'").bind(id).fetch_all(&mut *tx).await?;
    for member in members {
        notify(
            &mut tx,
            member,
            match input.estado.as_str() {
                "publicado" => "HACKATON_PUBLICADO",
                "finalizado" => "HACKATON_RESULTADOS",
                _ => "HACKATON_ESTADO_CAMBIADO",
            },
            &format!("Hackathon {id}: {}", input.estado),
            &format!("hackathon:{id}:{}", input.estado),
        )
        .await?;
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
    let person:Option<(String,String)>=sqlx::query_as("SELECT left(trim(first_name||' '||last_name),150),email FROM usuarios WHERE id=$1 AND is_active AND rol IN ('EXPERTO','PROFESOR')").bind(input.usuario_id).fetch_optional(&mut *tx).await?;
    let (name, email) =
        person.ok_or_else(|| AppError::bad("Selecciona un experto o profesor activo"))?;
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
    let score = crate::finance::money(&input.puntaje)?;
    if score > rust_decimal::Decimal::from(5) || input.comentario.len() > 4000 {
        return Err(AppError::bad("Puntaje entre 0 y 5"));
    }
    let mut tx = state.pool.begin().await?;
    let (_, status, _) = lock(&mut tx, id).await?;
    if !["publicado", "en_curso"].contains(&status.as_str()) {
        return Err(AppError::bad("Votaciones cerradas"));
    }
    window(&mut tx, id, "presentacion").await?;
    let juror:Option<i64>=sqlx::query_scalar("SELECT id FROM jurados_hackaton WHERE hackathon_id=$1 AND usuario_id=$2 ORDER BY id LIMIT 1").bind(id).bind(user.id).fetch_optional(&mut *tx).await?;
    let juror = juror.ok_or_else(AppError::forbidden)?;
    let accepted:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM inscripciones_hackaton WHERE hackathon_id=$1 AND equipo_id=$2 AND estado='ACEPTADA')").bind(id).bind(input.equipo_id).fetch_one(&mut *tx).await?;
    if !accepted {
        return Err(AppError::bad("El equipo no tiene inscripción aceptada"));
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
    let published: bool =
        sqlx::query_scalar("SELECT resultados_publicados FROM hackathons WHERE id=$1")
            .bind(id)
            .fetch_one(&state.pool)
            .await?;
    if !published && !user.admin() {
        return Err(AppError::forbidden());
    }
    let rows:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('equipo_id',e.id,'equipo',e.nombre,'promedio',round(avg(v.puntaje),2),'votos',count(v.id)) FROM votaciones_hackaton v JOIN equipos e ON e.id=v.equipo_id JOIN inscripciones_hackaton i ON i.equipo_id=e.id AND i.hackathon_id=v.hackathon_id WHERE v.hackathon_id=$1 AND i.estado='ACEPTADA' GROUP BY e.id ORDER BY avg(v.puntaje) DESC,count(v.id) DESC,e.nombre").bind(id).fetch_all(&state.pool).await?;
    Ok(Json(json!({"ranking":rows})))
}
