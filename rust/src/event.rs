//! HU09 (Django apps/hackaton): advance by schedule with automatic results at
//! close, event logistics with guests, and the exportable final report.
use crate::{
    AppState, auth,
    error::{AppError, Result},
    reports::pdf_lines,
    workflows::{history, notify},
};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;

/// Same rules as services.avanzar_por_calendario. Idempotent; returns what changed.
pub async fn advance(pool: &PgPool, id: i64) -> Result<Vec<&'static str>> {
    let mut tx = pool.begin().await?;
    let row: Option<(String, bool, String)> = sqlx::query_as("SELECT h.estado,h.resultados_publicados,r.titulo FROM hackathons h JOIN retos r ON r.id=h.reto_id WHERE h.id=$1 FOR UPDATE OF h")
        .bind(id).fetch_optional(&mut *tx).await?;
    let Some((mut estado, published, title)) = row else {
        return Ok(vec![]);
    };
    let mut changes = vec![];
    let started: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM etapas_hackaton WHERE hackathon_id=$1 AND tipo='desarrollo' AND inicia_en<=now())")
        .bind(id).fetch_one(&mut *tx).await?;
    if estado == "publicado" && started {
        sqlx::query("UPDATE hackathons SET estado='en_curso',actualizado_en=now() WHERE id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        history(
            &mut tx,
            ("hackaton", "hackathon"),
            id,
            "publicado",
            "en_curso",
            0,
            "Avance automatico: inicio de la etapa de desarrollo.",
        )
        .await?;
        estado = "en_curso".into();
        changes.push("en_curso");
    }
    // Close = start of the awards stage, else the end of the last dated stage.
    let closed: bool = sqlx::query_scalar("SELECT COALESCE(COALESCE((SELECT min(inicia_en) FROM etapas_hackaton WHERE hackathon_id=$1 AND tipo='premiacion'),(SELECT max(termina_en) FROM etapas_hackaton WHERE hackathon_id=$1)) <= now(), false)")
        .bind(id).fetch_one(&mut *tx).await?;
    if ["publicado", "en_curso"].contains(&estado.as_str()) && !published && closed {
        sqlx::query("UPDATE hackathons SET estado='finalizado',resultados_publicados=true,actualizado_en=now() WHERE id=$1").bind(id).execute(&mut *tx).await?;
        history(
            &mut tx,
            ("hackaton", "hackathon"),
            id,
            &estado,
            "finalizado",
            0,
            "Cierre automatico segun el calendario: resultados publicados.",
        )
        .await?;
        let winner: Option<String> = sqlx::query_scalar("SELECT e.nombre FROM votaciones_hackaton v JOIN equipos e ON e.id=v.equipo_id WHERE v.hackathon_id=$1 GROUP BY e.id ORDER BY avg(v.puntaje) DESC,count(v.id) DESC,e.nombre LIMIT 1")
            .bind(id).fetch_optional(&mut *tx).await?;
        let mut message = format!("Ya estan publicados los resultados del hackathon '{title}'.");
        if let Some(w) = winner {
            message.push_str(&format!(" Equipo ganador: {w}."));
        }
        let members: Vec<i64> = sqlx::query_scalar("SELECT DISTINCT m.estudiante_id FROM miembros_equipo m JOIN inscripciones_hackaton i ON i.equipo_id=m.equipo_id WHERE i.hackathon_id=$1")
            .bind(id).fetch_all(&mut *tx).await?;
        for member in members {
            notify(
                &mut tx,
                member,
                "HACKATON_RESULTADOS",
                &message,
                &format!("hackaton:{id}:resultados"),
            )
            .await?;
        }
        changes.push("resultados_publicados");
    }
    tx.commit().await?;
    Ok(changes)
}

/// Advances every published or running hackathon (CLI `advance-hackathons`).
pub async fn advance_all(pool: &PgPool) -> Result<usize> {
    let ids: Vec<i64> =
        sqlx::query_scalar("SELECT id FROM hackathons WHERE estado IN ('publicado','en_curso')")
            .fetch_all(pool)
            .await?;
    let mut changed = 0;
    for id in ids {
        if !advance(pool, id).await?.is_empty() {
            changed += 1;
        }
    }
    Ok(changed)
}

async fn is_manager(state: &AppState, user: &auth::User, reto: i64) -> Result<bool> {
    Ok(user.admin()
        || (user.rol == "PROFESOR"
            && sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM integraciones_academicas WHERE reto_id=$1 AND profesor_id=$2)")
                .bind(reto).bind(user.id).fetch_one(&state.pool).await?))
}

#[derive(Deserialize)]
pub struct GuestInput {
    #[serde(default = "default_guest")]
    tipo: String,
    nombre: String,
    #[serde(default)]
    organizacion: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    confirmado: bool,
}
fn default_guest() -> String {
    "INVITADO".into()
}
#[derive(Deserialize)]
pub struct LogisticsInput {
    modalidad: String,
    #[serde(default)]
    lugar: String,
    #[serde(default)]
    enlace_transmision: String,
    #[serde(default)]
    notas_logistica: String,
    #[serde(default)]
    invitados: Vec<GuestInput>,
}

/// POST: venue, format, streaming and guests (LogisticaForm + InvitadoFormSet).
pub async fn logistics(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<LogisticsInput>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    let reto: i64 = sqlx::query_scalar("SELECT reto_id FROM hackathons WHERE id=$1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(AppError::not_found)?;
    if !is_manager(&state, &user, reto).await? {
        return Err(AppError::forbidden());
    }
    if !["presencial", "virtual", "hibrida"].contains(&input.modalidad.as_str()) {
        return Err(AppError::bad("Modalidad inválida"));
    }
    let lugar = input.lugar.trim();
    let enlace = input.enlace_transmision.trim();
    if input.modalidad != "virtual" && lugar.is_empty() {
        return Err(AppError::bad(
            "Un evento presencial o hibrido necesita un espacio fisico.",
        ));
    }
    if input.modalidad != "presencial" && enlace.is_empty() {
        return Err(AppError::bad(
            "Un evento virtual o hibrido necesita el enlace de transmision.",
        ));
    }
    if lugar.chars().count() > 180
        || enlace.len() > 200
        || input.notas_logistica.chars().count() > 5000
        || input.invitados.len() > 100
    {
        return Err(AppError::bad("Datos de logística demasiado largos"));
    }
    if !enlace.is_empty() && !(enlace.starts_with("https://") || enlace.starts_with("http://")) {
        return Err(AppError::bad("El enlace de transmisión debe ser una URL"));
    }
    for g in &input.invitados {
        if !["INVITADO", "MENTOR", "PONENTE", "PATROCINADOR"].contains(&g.tipo.as_str())
            || g.nombre.trim().is_empty()
            || g.nombre.chars().count() > 150
            || g.organizacion.chars().count() > 150
            || g.email.len() > 254
        {
            return Err(AppError::bad("Revisa los datos de los invitados"));
        }
    }
    let mut tx = state.pool.begin().await?;
    sqlx::query("UPDATE hackathons SET modalidad=$2,lugar=$3,enlace_transmision=$4,notas_logistica=$5,actualizado_en=now() WHERE id=$1")
        .bind(id).bind(&input.modalidad).bind(lugar).bind(enlace).bind(input.notas_logistica.trim()).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM invitados_hackaton WHERE hackathon_id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    for g in &input.invitados {
        sqlx::query("INSERT INTO invitados_hackaton(hackathon_id,tipo,nombre,organizacion,email,confirmado) VALUES($1,$2,$3,$4,$5,$6)")
            .bind(id).bind(&g.tipo).bind(g.nombre.trim()).bind(g.organizacion.trim()).bind(g.email.trim()).bind(g.confirmado)
            .execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(Json(json!({"ok": true})))
}

/// Report sections as (title, headers, rows), shared by the PDF and the Excel file.
async fn sections(
    state: &AppState,
    id: i64,
) -> Result<Vec<(String, Vec<String>, Vec<Vec<String>>)>> {
    let q = |sql: &'static str| sqlx::query_scalar::<_, Value>(sql).bind(id);
    let call = q("SELECT jsonb_build_array(jsonb_build_array('Reto',r.titulo),jsonb_build_array('Codigo',CASE WHEN r.consecutivo IS NULL THEN '-' ELSE (CASE r.tipo WHEN 'hackathon' THEN 'HACK' ELSE 'RETO' END)||'-'||r.anio_consecutivo||'-'||lpad(r.consecutivo::text,3,'0') END),jsonb_build_array('Empresa',coalesce(nullif(trim(u.first_name||' '||u.last_name),''),u.username)),jsonb_build_array('Estado',h.estado),jsonb_build_array('Descripcion',coalesce(nullif(r.descripcion,''),'-')),jsonb_build_array('Reglas',coalesce(nullif(h.reglas,''),'-')),jsonb_build_array('Premios',coalesce(nullif(r.premios,''),'-')),jsonb_build_array('Resultados publicados',CASE WHEN h.resultados_publicados THEN 'Si' ELSE 'No' END),jsonb_build_array('Modalidad',h.modalidad),jsonb_build_array('Espacio fisico',coalesce(nullif(h.lugar,''),'-')),jsonb_build_array('Transmision',coalesce(nullif(h.enlace_transmision,''),'-'))) FROM hackathons h JOIN retos r ON r.id=h.reto_id JOIN usuarios u ON u.id=r.empresa_id WHERE h.id=$1").fetch_one(&state.pool).await?;
    let stages = q("SELECT coalesce(jsonb_agg(jsonb_build_array(titulo,tipo,coalesce(to_char(inicia_en AT TIME ZONE 'America/Bogota','DD/MM/YYYY HH24:MI'),'-'),coalesce(to_char(termina_en AT TIME ZONE 'America/Bogota','DD/MM/YYYY HH24:MI'),'-')) ORDER BY orden),'[]') FROM etapas_hackaton WHERE hackathon_id=$1").fetch_one(&state.pool).await?;
    let guests = q("SELECT coalesce(jsonb_agg(jsonb_build_array(nombre,tipo,coalesce(nullif(organizacion,''),'-'),CASE WHEN confirmado THEN 'Si' ELSE 'No' END) ORDER BY tipo,nombre),'[]') FROM invitados_hackaton WHERE hackathon_id=$1").fetch_one(&state.pool).await?;
    let jurors = q("SELECT coalesce(jsonb_agg(jsonb_build_array(nombre,coalesce(nullif(email,''),'-'),coalesce(nullif(especialidad,''),'-')) ORDER BY nombre),'[]') FROM jurados_hackaton WHERE hackathon_id=$1").fetch_one(&state.pool).await?;
    let teams = q("SELECT coalesce(jsonb_agg(jsonb_build_array(e.nombre,i.estado,coalesce((SELECT string_agg(coalesce(nullif(trim(u.first_name||' '||u.last_name),''),u.username),', ') FROM miembros_equipo m JOIN usuarios u ON u.id=m.estudiante_id WHERE m.equipo_id=e.id),'-'),to_char(i.creado_en AT TIME ZONE 'America/Bogota','DD/MM/YYYY HH24:MI')) ORDER BY i.creado_en),'[]') FROM inscripciones_hackaton i JOIN equipos e ON e.id=i.equipo_id WHERE i.hackathon_id=$1").fetch_one(&state.pool).await?;
    let ranking = q("SELECT coalesce(jsonb_agg(jsonb_build_array(puesto,nombre,round(prom,2),round(total,2),votos) ORDER BY puesto),'[]') FROM (SELECT row_number() OVER (ORDER BY avg(v.puntaje) DESC,count(v.id) DESC,e.nombre) puesto,e.nombre,avg(v.puntaje) prom,sum(v.puntaje) total,count(v.id) votos FROM votaciones_hackaton v JOIN equipos e ON e.id=v.equipo_id WHERE v.hackathon_id=$1 GROUP BY e.id) x").fetch_one(&state.pool).await?;
    let rows = |v: &Value| -> Vec<Vec<String>> {
        v.as_array()
            .into_iter()
            .flatten()
            .map(|r| {
                r.as_array()
                    .into_iter()
                    .flatten()
                    .map(|c| match c {
                        Value::String(s) => s.clone(),
                        Value::Null => "-".into(),
                        o => o.to_string(),
                    })
                    .collect()
            })
            .collect()
    };
    let h = |cols: &[&str]| cols.iter().map(|c| c.to_string()).collect::<Vec<_>>();
    Ok(vec![
        ("Convocatoria".into(), h(&["Campo", "Valor"]), rows(&call)),
        (
            "Etapas".into(),
            h(&["Etapa", "Tipo", "Inicia", "Termina"]),
            rows(&stages),
        ),
        (
            "Invitados".into(),
            h(&["Nombre", "Tipo", "Organizacion", "Confirmado"]),
            rows(&guests),
        ),
        (
            "Jurados".into(),
            h(&["Nombre", "Correo", "Especialidad"]),
            rows(&jurors),
        ),
        (
            "Equipos inscritos".into(),
            h(&["Equipo", "Estado", "Integrantes", "Inscrito"]),
            rows(&teams),
        ),
        (
            "Ranking".into(),
            h(&["Puesto", "Equipo", "Promedio", "Total", "Votos"]),
            rows(&ranking),
        ),
    ])
}

/// GET: final report (PDF or xlsx) for managers and the reto's company.
pub async fn report(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, format)): Path<(i64, String)>,
) -> Result<Response> {
    let user = auth::current(&state, &headers, false).await?;
    let (reto, company, title): (i64, i64, String) = sqlx::query_as("SELECT r.id,r.empresa_id,r.titulo FROM hackathons h JOIN retos r ON r.id=h.reto_id WHERE h.id=$1")
        .bind(id).fetch_optional(&state.pool).await?.ok_or_else(AppError::not_found)?;
    if !(is_manager(&state, &user, reto).await? || company == user.id) {
        return Err(AppError::forbidden());
    }
    advance(&state.pool, id).await?;
    let data = sections(&state, id).await?;
    let heading = format!("Informe final del hackathon: {title}");
    let (content_type, extension, bytes) = match format.as_str() {
        "pdf" => {
            let mut lines = vec![heading.clone(), String::new()];
            for (section, headers, rows) in &data {
                lines.push(format!("== {section} =="));
                if rows.is_empty() {
                    lines.push("Sin registros.".into());
                }
                for row in rows {
                    let line = headers
                        .iter()
                        .zip(row)
                        .map(|(h, v)| format!("{h}: {v}"))
                        .collect::<Vec<_>>()
                        .join(" | ");
                    let chars: Vec<_> = line.chars().collect();
                    lines.extend(chars.chunks(95).map(|c| c.iter().collect::<String>()));
                }
                lines.push(String::new());
            }
            ("application/pdf", "pdf", pdf_lines(&heading, &lines))
        }
        "xlsx" => {
            let fail = |e: rust_xlsxwriter::XlsxError| {
                tracing::error!(%e, "final report failed");
                AppError(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "No se pudo generar el archivo Excel".into(),
                )
            };
            let mut book = rust_xlsxwriter::Workbook::new();
            let bold = rust_xlsxwriter::Format::new()
                .set_bold()
                .set_font_color("FFFFFF")
                .set_background_color("3BAC52");
            for (section, headers, rows) in &data {
                let sheet = book.add_worksheet().set_name(section).map_err(fail)?;
                sheet
                    .write_string(0, 0, "retosean · Universidad EAN")
                    .map_err(fail)?;
                sheet
                    .write_string(1, 0, format!("{section} - {title}"))
                    .map_err(fail)?;
                for (c, name) in headers.iter().enumerate() {
                    sheet
                        .write_string_with_format(3, c as u16, name, &bold)
                        .map_err(fail)?;
                    sheet.set_column_width(c as u16, 28).map_err(fail)?;
                }
                for (r, row) in rows.iter().enumerate() {
                    for (c, value) in row.iter().enumerate() {
                        // Strings only: formula-safe, as in the Django export.
                        sheet
                            .write_string(r as u32 + 4, c as u16, value)
                            .map_err(fail)?;
                    }
                }
            }
            (
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                "xlsx",
                book.save_to_buffer().map_err(fail)?,
            )
        }
        _ => return Err(AppError::not_found()),
    };
    let disposition = format!("attachment; filename=informe-hackathon-{id}.{extension}");
    Ok((
        [
            (header::CONTENT_TYPE, content_type.to_string()),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        bytes,
    )
        .into_response())
}

/// Hackathon detail in the generic browser also applies the schedule first.
pub async fn advance_before_read(state: &AppState, name: &str, id: i64) {
    if name == "hackathons" {
        if let Err(error) = advance(&state.pool, id).await {
            tracing::warn!(?error, "hackathon schedule advance failed");
        }
    }
}

/// GET: logistics and guests of a hackathon the caller may see.
pub async fn show_logistics(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, false).await?;
    let record = crate::resources::visible_record(&state, &user, "hackathons", id).await?;
    let reto = record["reto_id"].as_i64().unwrap_or(0);
    let guests: Value = sqlx::query_scalar("SELECT coalesce(jsonb_agg(jsonb_build_object('tipo',tipo,'nombre',nombre,'organizacion',organizacion,'email',email,'confirmado',confirmado) ORDER BY tipo,nombre),'[]') FROM invitados_hackaton WHERE hackathon_id=$1")
        .bind(id).fetch_one(&state.pool).await?;
    let company: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM retos WHERE id=$1 AND empresa_id=$2)")
            .bind(reto)
            .bind(user.id)
            .fetch_one(&state.pool)
            .await?;
    let manager = is_manager(&state, &user, reto).await?;
    let closes: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar("SELECT COALESCE((SELECT min(inicia_en) FROM etapas_hackaton WHERE hackathon_id=$1 AND tipo='premiacion'),(SELECT max(termina_en) FROM etapas_hackaton WHERE hackathon_id=$1))")
        .bind(id).fetch_one(&state.pool).await?;
    Ok(Json(json!({
        "modalidad": record["modalidad"], "lugar": record["lugar"], "enlace_transmision": record["enlace_transmision"],
        "notas_logistica": record["notas_logistica"], "invitados": guests,
        "puede_gestionar": manager, "puede_exportar": manager || company, "cierre": closes,
    })))
}
