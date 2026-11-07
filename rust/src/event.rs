//! HU09 (Django apps/hackaton): advance by schedule with automatic results at
//! close, event logistics with guests, and the exportable final report.
use crate::{
    AppState, auth,
    error::{AppError, Result},
    reports::{Section, pdf_document},
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
    let row: Option<(String, bool)> = sqlx::query_as(
        "SELECT estado,resultados_publicados FROM hackathons WHERE id=$1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((mut estado, published)) = row else {
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
            "Avance automático: inicio de la etapa de desarrollo.",
        )
        .await?;
        estado = "en_curso".into();
        changes.push("en_curso");
    }
    // Close = start of the awards stage, else the end of the last dated stage.
    let closed: bool = sqlx::query_scalar("SELECT COALESCE(COALESCE((SELECT min(inicia_en) FROM etapas_hackaton WHERE hackathon_id=$1 AND tipo='premiacion'),(SELECT max(termina_en) FROM etapas_hackaton WHERE hackathon_id=$1)) <= now(), false)")
        .bind(id).fetch_one(&mut *tx).await?;
    if ["publicado", "en_curso"].contains(&estado.as_str()) && !published && closed {
        publish_results(
            &mut tx,
            id,
            &estado,
            0,
            "Cierre automático según el calendario: resultados publicados.",
        )
        .await?;
        changes.push("resultados_publicados");
    }
    tx.commit().await?;
    Ok(changes)
}

/// publicar_resultados: results public, the hackathon finished, and every
/// registered team member told the winner.
pub async fn publish_results(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    id: i64,
    from: &str,
    user: i64,
    comment: &str,
) -> Result<()> {
    sqlx::query("UPDATE hackathons SET estado='finalizado',resultados_publicados=true,actualizado_en=now() WHERE id=$1")
        .bind(id)
        .execute(&mut **tx)
        .await?;
    if from != "finalizado" {
        history(
            tx,
            ("hackaton", "hackathon"),
            id,
            from,
            "finalizado",
            user,
            comment,
        )
        .await?;
    }
    let title: String = sqlx::query_scalar(
        "SELECT r.titulo FROM hackathons h JOIN retos r ON r.id=h.reto_id WHERE h.id=$1",
    )
    .bind(id)
    .fetch_one(&mut **tx)
    .await?;
    let winner: Option<String> = sqlx::query_scalar("SELECT e.nombre FROM votaciones_hackaton v JOIN equipos e ON e.id=v.equipo_id WHERE v.hackathon_id=$1 GROUP BY e.id ORDER BY avg(v.puntaje) DESC,count(v.id) DESC,e.nombre LIMIT 1")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?;
    let mut message = format!("Ya están publicados los resultados del hackathon '{title}'.");
    if let Some(w) = winner {
        message.push_str(&format!(" Equipo ganador: {w}."));
    }
    let members: Vec<i64> = sqlx::query_scalar("SELECT DISTINCT m.estudiante_id FROM miembros_equipo m JOIN inscripciones_hackaton i ON i.equipo_id=m.equipo_id JOIN usuarios u ON u.id=m.estudiante_id WHERE i.hackathon_id=$1 AND u.is_active")
        .bind(id)
        .fetch_all(&mut **tx)
        .await?;
    for member in members {
        notify(
            tx,
            member,
            "HACKATON_RESULTADOS",
            &message,
            &format!("hackaton:{id}:resultados"),
        )
        .await?;
    }
    Ok(())
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
            && sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM integraciones_academicas WHERE reto_id=$1 AND profesor_id=$2 AND estado IN ('aprobada','publicada'))")
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
            "Un evento presencial o híbrido necesita un espacio físico.",
        ));
    }
    if input.modalidad != "presencial" && enlace.is_empty() {
        return Err(AppError::bad(
            "Un evento virtual o híbrido necesita el enlace de transmisión.",
        ));
    }
    crate::domain::max_chars(lugar, 180, "Espacio físico")?;
    crate::domain::max_chars(enlace, 200, "Enlace de transmisión")?;
    crate::domain::max_chars(input.notas_logistica.trim(), 5000, "Notas de logística")?;
    if input.invitados.len() > 100 {
        return Err(AppError::bad("Máximo 100 invitados por hackathon."));
    }
    if !enlace.is_empty() && !(enlace.starts_with("https://") || enlace.starts_with("http://")) {
        return Err(AppError::bad("El enlace de transmisión debe ser una URL"));
    }
    for g in &input.invitados {
        if !["INVITADO", "MENTOR", "PONENTE", "PATROCINADOR"].contains(&g.tipo.as_str())
            || g.nombre.trim().is_empty()
        {
            return Err(AppError::bad("Revisa los datos de los invitados"));
        }
        crate::domain::max_chars(g.nombre.trim(), 150, "Nombre del invitado")?;
        crate::domain::max_chars(g.organizacion.trim(), 150, "Organización del invitado")?;
        crate::domain::max_chars(g.email.trim(), 254, "Correo del invitado")?;
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

/// Report sections as (title, headers, rows), shared by the PDF and the Excel
/// file (informe.datos_informe). Stored codes are shown with their labels.
async fn sections(
    state: &AppState,
    id: i64,
) -> Result<Vec<(String, Vec<String>, Vec<Vec<String>>)>> {
    use crate::dashboard::etiqueta;
    #[allow(clippy::type_complexity)]
    let call: (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        bool,
        String,
        String,
        String,
        String,
    ) = sqlx::query_as("SELECT r.titulo,CASE WHEN r.consecutivo IS NULL THEN '-' ELSE (CASE r.tipo WHEN 'hackathon' THEN 'HACK' ELSE 'RETO' END)||'-'||r.anio_consecutivo||'-'||lpad(r.consecutivo::text,3,'0') END,coalesce(nullif(trim(u.first_name||' '||u.last_name),''),u.username),h.estado,coalesce(to_char(r.fecha_inicio_tentativa,'DD/MM/YYYY'),'-')||' a '||coalesce(to_char(r.fecha_fin_tentativa,'DD/MM/YYYY'),'-'),coalesce(nullif(r.descripcion,''),'-'),coalesce(nullif(h.reglas,''),'-'),coalesce(nullif(r.premios,''),'-'),h.resultados_publicados,h.modalidad,coalesce(nullif(h.lugar,''),'-'),coalesce(nullif(h.enlace_transmision,''),'-'),coalesce(nullif(h.notas_logistica,''),'-') FROM hackathons h JOIN retos r ON r.id=h.reto_id JOIN usuarios u ON u.id=r.empresa_id WHERE h.id=$1")
        .bind(id)
        .fetch_one(&state.pool)
        .await?;
    let q = |sql: &'static str| sqlx::query_scalar::<_, Value>(sql).bind(id);
    let stages = q("SELECT coalesce(jsonb_agg(jsonb_build_array(titulo,tipo,coalesce(to_char(inicia_en AT TIME ZONE 'America/Bogota','DD/MM/YYYY HH24:MI'),'-'),coalesce(to_char(termina_en AT TIME ZONE 'America/Bogota','DD/MM/YYYY HH24:MI'),'-')) ORDER BY orden),'[]') FROM etapas_hackaton WHERE hackathon_id=$1").fetch_one(&state.pool).await?;
    let guests = q("SELECT coalesce(jsonb_agg(jsonb_build_array(nombre,tipo,coalesce(nullif(organizacion,''),'-'),CASE WHEN confirmado THEN 'Sí' ELSE 'No' END) ORDER BY tipo,nombre),'[]') FROM invitados_hackaton WHERE hackathon_id=$1").fetch_one(&state.pool).await?;
    let jurors = q("SELECT coalesce(jsonb_agg(jsonb_build_array(nombre,coalesce(nullif(email,''),'-'),coalesce(nullif(especialidad,''),'-')) ORDER BY nombre),'[]') FROM jurados_hackaton WHERE hackathon_id=$1").fetch_one(&state.pool).await?;
    let teams = q("SELECT coalesce(jsonb_agg(jsonb_build_array(e.nombre,i.estado,coalesce((SELECT string_agg(coalesce(nullif(trim(u.first_name||' '||u.last_name),''),u.username),', ') FROM miembros_equipo m JOIN usuarios u ON u.id=m.estudiante_id WHERE m.equipo_id=e.id),'-'),to_char(i.creado_en AT TIME ZONE 'America/Bogota','DD/MM/YYYY HH24:MI')) ORDER BY i.creado_en),'[]') FROM inscripciones_hackaton i JOIN equipos e ON e.id=i.equipo_id WHERE i.hackathon_id=$1").fetch_one(&state.pool).await?;
    let ranking = q("SELECT coalesce(jsonb_agg(jsonb_build_array(puesto,nombre,round(prom,2)::text,round(total,2)::text,votos) ORDER BY puesto),'[]') FROM (SELECT row_number() OVER (ORDER BY avg(v.puntaje) DESC,count(v.id) DESC,e.nombre) puesto,e.nombre,avg(v.puntaje) prom,sum(v.puntaje) total,count(v.id) votos FROM votaciones_hackaton v JOIN equipos e ON e.id=v.equipo_id WHERE v.hackathon_id=$1 GROUP BY e.id) x").fetch_one(&state.pool).await?;
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
    // Replaces the stored code in column `col` with its display label.
    let labelled = |mut rows: Vec<Vec<String>>, col: usize, table: &str, field: &str| {
        for row in &mut rows {
            if let Some(cell) = row.get_mut(col) {
                let label = etiqueta(table, field, cell.as_str());
                *cell = label;
            }
        }
        rows
    };
    let ranking = rows(&ranking);
    let winner = match ranking.first() {
        Some(first) if call.8 => first.get(1).cloned().unwrap_or_else(|| "-".into()),
        _ => "-".into(),
    };
    let pair = |k: &str, v: String| vec![k.to_string(), v];
    let convocatoria = vec![
        pair("Reto", call.0),
        pair("Código", call.1),
        pair("Empresa", call.2),
        pair("Estado", etiqueta("hackathons", "estado", &call.3)),
        pair("Fechas", call.4),
        pair("Descripción", call.5),
        pair("Reglas", call.6),
        pair("Premios", call.7),
        pair(
            "Resultados publicados",
            if call.8 { "Sí" } else { "No" }.into(),
        ),
        pair("Equipo ganador", winner),
    ];
    let logistica = vec![
        pair("Modalidad", etiqueta("hackathons", "modalidad", &call.9)),
        pair("Espacio físico", call.10),
        pair("Transmisión", call.11),
        pair("Notas", call.12),
    ];
    let h = |cols: &[&str]| cols.iter().map(|c| c.to_string()).collect::<Vec<_>>();
    Ok(vec![
        ("Convocatoria".into(), h(&["Campo", "Valor"]), convocatoria),
        (
            "Logística del evento".into(),
            h(&["Campo", "Valor"]),
            logistica,
        ),
        (
            "Etapas".into(),
            h(&["Etapa", "Tipo", "Inicia", "Termina"]),
            labelled(rows(&stages), 1, "etapas_hackaton", "tipo"),
        ),
        (
            "Invitados".into(),
            h(&["Nombre", "Tipo", "Organización", "Confirmado"]),
            labelled(rows(&guests), 1, "invitados_hackaton", "tipo"),
        ),
        (
            "Jurados".into(),
            h(&["Nombre", "Correo", "Especialidad"]),
            rows(&jurors),
        ),
        (
            "Equipos inscritos".into(),
            h(&["Equipo", "Estado", "Integrantes", "Inscrito"]),
            labelled(rows(&teams), 1, "inscripciones_hackaton", "estado"),
        ),
        (
            "Ranking".into(),
            h(&["Puesto", "Equipo", "Promedio", "Total", "Votos"]),
            ranking,
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
            // Branded tables like informe_pdf: long text wraps inside its cell.
            let columns: Vec<Vec<(&str, f32)>> = data
                .iter()
                .map(|(_, headers, _)| {
                    let two = headers.len() == 2;
                    headers
                        .iter()
                        .enumerate()
                        .map(|(i, h)| {
                            let weight = match (two, i) {
                                (true, 0) => 28.,
                                (true, _) => 72.,
                                _ => 1.,
                            };
                            (h.as_str(), weight)
                        })
                        .collect()
                })
                .collect();
            let parts: Vec<Section<'_>> = data
                .iter()
                .zip(&columns)
                .map(|((section, _, rows), columns)| Section {
                    heading: Some(section.as_str()),
                    columns,
                    rows,
                    empty: "Sin registros.",
                })
                .collect();
            let generated = chrono::Utc::now()
                .with_timezone(&chrono::FixedOffset::west_opt(5 * 3600).unwrap())
                .format("%d/%m/%Y %H:%M");
            let meta = [format!("Generado el {generated}")];
            (
                "application/pdf",
                "pdf",
                pdf_document(&heading, &meta, &parts, ""),
            )
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
    let manager = is_manager(&state, &user, reto).await?;
    // Guest emails only for admins and hackathon managers.
    let guests: Value = sqlx::query_scalar("SELECT coalesce(jsonb_agg(jsonb_build_object('tipo',tipo,'nombre',nombre,'organizacion',organizacion,'email',CASE WHEN $2 THEN email ELSE '' END,'confirmado',confirmado) ORDER BY tipo,nombre),'[]') FROM invitados_hackaton WHERE hackathon_id=$1")
        .bind(id).bind(manager).fetch_one(&state.pool).await?;
    let company: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM retos WHERE id=$1 AND empresa_id=$2)")
            .bind(reto)
            .bind(user.id)
            .fetch_one(&state.pool)
            .await?;
    let closes: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar("SELECT COALESCE((SELECT min(inicia_en) FROM etapas_hackaton WHERE hackathon_id=$1 AND tipo='premiacion'),(SELECT max(termina_en) FROM etapas_hackaton WHERE hackathon_id=$1))")
        .bind(id).fetch_one(&state.pool).await?;
    Ok(Json(json!({
        "modalidad": record["modalidad"], "lugar": record["lugar"], "enlace_transmision": record["enlace_transmision"],
        "notas_logistica": record["notas_logistica"], "invitados": guests,
        "puede_gestionar": manager, "puede_exportar": manager || company, "cierre": closes,
    })))
}
