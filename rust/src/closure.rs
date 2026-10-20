//! HU08 closure management (apps/cierre/views.py): agenda and acta, final
//! artifacts, recognitions and logistics requests. Finalizing lives in lifecycle.rs.
use crate::{
    AppState, auth,
    auth::User,
    dashboard::etiqueta,
    error::{AppError, Result},
    workflows::{history, notify},
};
use axum::{
    Json,
    body::Bytes,
    extract::{Multipart, Path, State},
    http::HeaderMap,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use std::collections::HashMap;

const VIGENTES: &str = "('aprobado','abierto','en_curso','pausado','finalizado')";

/// _usuario_puede_gestionar_cierre: admin, the owning company or an integrated professor.
async fn manage(
    tx: &mut Transaction<'_, Postgres>,
    reto: i64,
    user: &User,
    vigente: bool,
) -> Result<()> {
    let filter = if vigente {
        format!(" AND estado IN {VIGENTES}")
    } else {
        String::new()
    };
    let owner: Option<i64> = sqlx::query_scalar(&format!(
        "SELECT empresa_id FROM retos WHERE id=$1 AND eliminado_en IS NULL{filter}"
    ))
    .bind(reto)
    .fetch_optional(&mut **tx)
    .await?;
    let owner = owner.ok_or_else(AppError::not_found)?;
    if user.admin()
        || (user.rol == "EMPRESA" && owner == user.id)
        || teacher(tx, reto, user).await?
    {
        Ok(())
    } else {
        Err(AppError::forbidden())
    }
}
async fn teacher(tx: &mut Transaction<'_, Postgres>, reto: i64, user: &User) -> Result<bool> {
    Ok(user.rol == "PROFESOR"
        && sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM integraciones_academicas WHERE reto_id=$1 AND profesor_id=$2)",
        )
        .bind(reto)
        .bind(user.id)
        .fetch_one(&mut **tx)
        .await?)
}
/// CierreReto.objects.get_or_create(reto, defaults={"cerrado_por": user}).
async fn closure_id(tx: &mut Transaction<'_, Postgres>, reto: i64, user: i64) -> Result<i64> {
    sqlx::query("INSERT INTO cierres_reto(reto_id,notas,impacto,encuesta_enviada,cerrado_por_id,actualizado_en) VALUES($1,'','',false,$2,now()) ON CONFLICT(reto_id) DO NOTHING")
        .bind(reto).bind(user).execute(&mut **tx).await?;
    Ok(
        sqlx::query_scalar("SELECT id FROM cierres_reto WHERE reto_id=$1")
            .bind(reto)
            .fetch_one(&mut **tx)
            .await?,
    )
}

pub async fn show(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(reto): Path<i64>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, false).await?;
    let mut tx = state.pool.begin().await?;
    manage(&mut tx, reto, &user, true).await?;
    let q = |sql: &'static str| sqlx::query_scalar::<_, Value>(sql).bind(reto);
    let agenda = q("SELECT COALESCE((SELECT jsonb_build_object('fecha_hora',to_char(fecha_hora AT TIME ZONE 'America/Bogota','YYYY-MM-DD\"T\"HH24:MI'),'espacio',espacio,'recursos',recursos,'invitados',invitados,'agenda',agenda) FROM cierre_agendacierre WHERE reto_id=$1),'{}')").fetch_one(&mut *tx).await?;
    let closure = q("SELECT COALESCE((SELECT jsonb_build_object('id',id,'notas',notas,'impacto',impacto,'acta',COALESCE(acta_url,'')<>'','cerrado_en',cerrado_en) FROM cierres_reto WHERE reto_id=$1),'{}')").fetch_one(&mut *tx).await?;
    let artifacts = q("SELECT COALESCE(jsonb_agg(jsonb_build_object('id',f.id,'nombre',f.nombre,'cargado_por',COALESCE(NULLIF(trim(u.first_name||' '||u.last_name),''),u.username),'creado_en',f.creado_en) ORDER BY f.creado_en DESC),'[]') FROM cierre_entregablefinal f JOIN cierres_reto c ON c.id=f.cierre_id LEFT JOIN usuarios u ON u.id=f.cargado_por_id WHERE c.reto_id=$1").fetch_one(&mut *tx).await?;
    let recognitions = q("SELECT COALESCE(jsonb_agg(jsonb_build_object('nombre_destinatario',r.nombre_destinatario,'descripcion',r.descripcion,'creado_en',r.creado_en) ORDER BY r.creado_en),'[]') FROM reconocimientos r JOIN cierres_reto c ON c.id=r.cierre_id WHERE c.reto_id=$1").fetch_one(&mut *tx).await?;
    let requests = q("SELECT COALESCE(jsonb_agg(jsonb_build_object('id',s.id,'evento',s.evento,'tipo',s.tipo,'descripcion',s.descripcion,'estado',s.estado,'solicitada_por',COALESCE(NULLIF(trim(u.first_name||' '||u.last_name),''),u.username),'creada_en',s.creada_en) ORDER BY s.evento,s.creada_en),'[]') FROM cierre_solicitudlogistica s LEFT JOIN usuarios u ON u.id=s.solicitada_por_id WHERE s.reto_id=$1").fetch_one(&mut *tx).await?;
    let students = q("SELECT COALESCE(jsonb_agg(jsonb_build_object('value',u.id,'label',COALESCE(NULLIF(trim(u.first_name||' '||u.last_name),''),u.username)) ORDER BY u.first_name,u.last_name),'[]') FROM usuarios u WHERE u.rol='ESTUDIANTE' AND u.id IN (SELECT estudiante_id FROM postulaciones WHERE reto_id=$1 AND estado='ACEPTADA' UNION SELECT m.estudiante_id FROM miembros_equipo m JOIN equipos e ON e.id=m.equipo_id WHERE e.reto_id=$1)").fetch_one(&mut *tx).await?;
    let finish = user.admin() || teacher(&mut tx, reto, &user).await?;
    Ok(Json(json!({
        "agenda": agenda, "cierre": closure, "entregables_finales": artifacts,
        "reconocimientos": recognitions, "solicitudes_logistica": requests,
        "estudiantes": students, "puede_finalizar": finish,
    })))
}

/// Text fields plus at most one file, as Django's multipart forms.
async fn read_form(
    mut multipart: Multipart,
    file: &str,
) -> Result<(HashMap<String, String>, Option<Bytes>)> {
    let mut fields = HashMap::new();
    let mut data = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::bad("Carga inválida"))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == file {
            let bytes = field
                .bytes()
                .await
                .map_err(|_| AppError::bad("Archivo demasiado grande"))?;
            if bytes.len() > 9 * 1024 * 1024 {
                return Err(AppError::bad("Archivo de máximo 9 MB"));
            }
            if !bytes.is_empty() {
                data = Some(bytes);
            }
        } else {
            let text = field
                .text()
                .await
                .map_err(|_| AppError::bad("Campo inválido"))?;
            if text.len() > 20000 {
                return Err(AppError::bad("Texto demasiado largo"));
            }
            fields.insert(name, text);
        }
    }
    Ok((fields, data))
}
async fn store(state: &AppState, data: &[u8]) -> Result<(String, std::path::PathBuf)> {
    let key = format!(
        "{}.{}",
        uuid::Uuid::new_v4().simple(),
        crate::media::extension(data)?
    );
    tokio::fs::create_dir_all(&state.config.media_root)
        .await
        .map_err(|_| AppError::bad("No se pudo guardar el archivo"))?;
    let path = state.config.media_root.join(&key);
    tokio::fs::write(&path, data)
        .await
        .map_err(|_| AppError::bad("No se pudo guardar el archivo"))?;
    Ok((key, path))
}
async fn commit(tx: Transaction<'_, Postgres>, path: Option<std::path::PathBuf>) -> Result<()> {
    if let Err(error) = tx.commit().await {
        if let Some(path) = path {
            let _ = tokio::fs::remove_file(path).await;
        }
        return Err(error.into());
    }
    Ok(())
}

/// gestionar (accion=guardar): AgendaCierreForm + CierreRetoForm.
pub async fn save(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(reto): Path<i64>,
    multipart: Multipart,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    let (fields, acta) = read_form(multipart, "acta_url").await?;
    let get = |name: &str| fields.get(name).map(|v| v.trim()).unwrap_or("");
    let when = get("fecha_hora");
    if !when.is_empty() && chrono::NaiveDateTime::parse_from_str(when, "%Y-%m-%dT%H:%M").is_err() {
        return Err(AppError::bad("Fecha y hora inválidas"));
    }
    if get("espacio").chars().count() > 200 {
        return Err(AppError::bad("Espacio: máximo 200 caracteres"));
    }
    if !["", "SI", "NO", "EN_EVALUACION"].contains(&get("impacto")) {
        return Err(AppError::bad("Impacto inválido"));
    }
    let mut tx = state.pool.begin().await?;
    manage(&mut tx, reto, &user, true).await?;
    sqlx::query("INSERT INTO cierre_agendacierre(reto_id,fecha_hora,espacio,recursos,invitados,agenda,actualizado_en) VALUES($1,NULLIF($2,'')::timestamp AT TIME ZONE 'America/Bogota',$3,$4,$5,$6,now()) ON CONFLICT(reto_id) DO UPDATE SET fecha_hora=excluded.fecha_hora,espacio=excluded.espacio,recursos=excluded.recursos,invitados=excluded.invitados,agenda=excluded.agenda,actualizado_en=now()")
        .bind(reto).bind(when).bind(get("espacio")).bind(get("recursos")).bind(get("invitados")).bind(get("agenda")).execute(&mut *tx).await?;
    let closure = closure_id(&mut tx, reto, user.id).await?;
    let stored = match &acta {
        Some(data) => Some(store(&state, data).await?),
        None => None,
    };
    sqlx::query("UPDATE cierres_reto SET notas=$2,impacto=$3,acta_url=COALESCE($4,acta_url),cerrado_por_id=$5,actualizado_en=now() WHERE id=$1")
        .bind(closure).bind(get("notas")).bind(get("impacto")).bind(stored.as_ref().map(|(k, _)| k)).bind(user.id).execute(&mut *tx).await?;
    commit(tx, stored.map(|(_, p)| p)).await?;
    Ok(Json(
        json!({"ok":true,"mensaje":"Documentacion y agenda guardadas."}),
    ))
}

/// agregar_entregable: EntregableFinalForm (nombre, archivo).
pub async fn artifact(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(reto): Path<i64>,
    multipart: Multipart,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    let (fields, data) = read_form(multipart, "archivo").await?;
    let name = fields.get("nombre").map(|v| v.trim()).unwrap_or("");
    let invalid = "No fue posible cargar el entregable final. Verifica los datos o el archivo.";
    if name.is_empty() || name.chars().count() > 180 {
        return Err(AppError::bad(invalid));
    }
    let data = data.ok_or_else(|| AppError::bad(invalid))?;
    let mut tx = state.pool.begin().await?;
    manage(&mut tx, reto, &user, false).await?;
    let closure = closure_id(&mut tx, reto, user.id).await?;
    let (key, path) = store(&state, &data).await?;
    let id: i64 = sqlx::query_scalar("INSERT INTO cierre_entregablefinal(cierre_id,nombre,archivo,cargado_por_id,creado_en) VALUES($1,$2,$3,$4,now()) RETURNING id")
        .bind(closure).bind(name).bind(&key).bind(user.id).fetch_one(&mut *tx).await?;
    commit(tx, Some(path)).await?;
    Ok(Json(json!({"id":id,"mensaje":"Entregable final cargado."})))
}

#[derive(Deserialize)]
pub struct RecognitionInput {
    usuario_id: Option<i64>,
    nombre_destinatario: String,
    descripcion: String,
}
/// agregar_reconocimiento: a linked user must be a student participant.
pub async fn recognition(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(reto): Path<i64>,
    Json(input): Json<RecognitionInput>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    let name = input.nombre_destinatario.trim();
    if name.is_empty()
        || name.chars().count() > 150
        || input.descripcion.trim().is_empty()
        || input.descripcion.len() > 20000
    {
        return Err(AppError::bad("Revisa los datos del reconocimiento."));
    }
    let mut tx = state.pool.begin().await?;
    manage(&mut tx, reto, &user, false).await?;
    if let Some(student) = input.usuario_id {
        let participant: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM usuarios WHERE id=$2 AND rol='ESTUDIANTE') AND $2 IN (SELECT empresa_id FROM retos WHERE id=$1 UNION SELECT profesor_id FROM integraciones_academicas WHERE reto_id=$1 UNION SELECT estudiante_id FROM postulaciones WHERE reto_id=$1 AND estado='ACEPTADA' UNION SELECT m.estudiante_id FROM miembros_equipo m JOIN equipos e ON e.id=m.equipo_id WHERE e.reto_id=$1)")
            .bind(reto).bind(student).fetch_one(&mut *tx).await?;
        if !participant {
            return Err(AppError::bad(
                "El reconocimiento debe asignarse a un estudiante participante del reto.",
            ));
        }
    }
    let closure = closure_id(&mut tx, reto, user.id).await?;
    let id: i64 = sqlx::query_scalar("INSERT INTO reconocimientos(cierre_id,usuario_id,nombre_destinatario,descripcion,creado_en) VALUES($1,$2,$3,$4,now()) RETURNING id")
        .bind(closure).bind(input.usuario_id).bind(name).bind(input.descripcion.trim()).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(
        json!({"id":id,"mensaje":"Reconocimiento registrado."}),
    ))
}

#[derive(Deserialize)]
pub struct LogisticsInput {
    evento: String,
    tipo: String,
    descripcion: String,
}
/// agregar_solicitud_logistica: company, professor or admin ask for event support.
pub async fn request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(reto): Path<i64>,
    Json(input): Json<LogisticsInput>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    let detail = input.descripcion.trim();
    if !["INICIO", "CIERRE"].contains(&input.evento.as_str())
        || ![
            "ESPACIO",
            "COMUNICACIONES",
            "REFRIGERIOS",
            "EQUIPOS_AV",
            "INVITADOS",
            "OTRO",
        ]
        .contains(&input.tipo.as_str())
        || detail.is_empty()
        || detail.len() > 20000
    {
        return Err(AppError::bad("Revisa los datos de la solicitud."));
    }
    let mut tx = state.pool.begin().await?;
    manage(&mut tx, reto, &user, false).await?;
    let id: i64 = sqlx::query_scalar("INSERT INTO cierre_solicitudlogistica(reto_id,evento,tipo,descripcion,estado,solicitada_por_id,creada_en,actualizado_en) VALUES($1,$2,$3,$4,'SOLICITADA',$5,now(),now()) RETURNING id")
        .bind(reto).bind(&input.evento).bind(&input.tipo).bind(detail).bind(user.id).fetch_one(&mut *tx).await?;
    history(
        &mut tx,
        ("cierre", "solicitudlogistica"),
        id,
        "",
        "SOLICITADA",
        user.id,
        detail,
    )
    .await?;
    let title: String = sqlx::query_scalar("SELECT titulo FROM retos WHERE id=$1")
        .bind(reto)
        .fetch_one(&mut *tx)
        .await?;
    let message = format!(
        "{} para el {} del reto \"{title}\": {detail}",
        etiqueta("cierre_solicitudlogistica", "tipo", &input.tipo),
        etiqueta("cierre_solicitudlogistica", "evento", &input.evento).to_lowercase(),
    );
    let admins: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM usuarios WHERE is_active AND (is_superuser OR rol='ADMIN') AND id<>$1",
    )
    .bind(user.id)
    .fetch_all(&mut *tx)
    .await?;
    for admin in admins {
        notify(
            &mut tx,
            admin,
            "LOGISTICA_SOLICITADA",
            &message,
            &format!("logistica:{id}:solicitada"),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(Json(
        json!({"id":id,"mensaje":"Solicitud de logística registrada."}),
    ))
}

#[derive(Deserialize)]
pub struct RequestState {
    estado: String,
    #[serde(default)]
    comentario: String,
}
/// cambiar_estado_solicitud_logistica with MAQUINA_SOLICITUD_LOGISTICA (cierre/views.py).
pub async fn request_state(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<RequestState>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    if !user.admin() {
        return Err(AppError::forbidden());
    }
    let mut tx = state.pool.begin().await?;
    let from: Option<String> =
        sqlx::query_scalar("SELECT estado FROM cierre_solicitudlogistica WHERE id=$1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    let from = from.ok_or_else(AppError::not_found)?;
    let allowed = match from.as_str() {
        "SOLICITADA" => ["CONFIRMADA", "DESCARTADA"].contains(&input.estado.as_str()),
        "CONFIRMADA" | "DESCARTADA" => input.estado == "SOLICITADA",
        _ => false,
    };
    if !allowed {
        return Err(AppError::bad(format!(
            "No se puede pasar la solicitud de \"{}\" a \"{}\".",
            etiqueta("cierre_solicitudlogistica", "estado", &from),
            etiqueta("cierre_solicitudlogistica", "estado", &input.estado)
        )));
    }
    sqlx::query("UPDATE cierre_solicitudlogistica SET estado=$2,actualizado_en=now() WHERE id=$1")
        .bind(id)
        .bind(&input.estado)
        .execute(&mut *tx)
        .await?;
    history(
        &mut tx,
        ("cierre", "solicitudlogistica"),
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
