use crate::{
    AppState,
    auth::{self, User},
    error::{AppError, Result},
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::OnceLock;

pub fn schema() -> &'static Vec<Value> {
    static SCHEMA: OnceLock<Vec<Value>> = OnceLock::new();
    SCHEMA.get_or_init(|| {
        serde_json::from_str(include_str!("../schema.json")).expect("bundled schema")
    })
}
pub fn resource(name: &str) -> Result<&'static Value> {
    schema()
        .iter()
        .find(|m| m["table"].as_str() == Some(name))
        .ok_or_else(AppError::not_found)
}
pub fn scope(table: &str, user: &User) -> Option<String> {
    if user.admin() {
        return Some(
            if table == "retos" {
                "t.eliminado_en IS NULL"
            } else {
                "TRUE"
            }
            .into(),
        );
    }
    let actor = "(SELECT id FROM actor)";
    let challenge = match user.rol.as_str() {
        "EMPRESA" => format!("r.empresa_id={actor}"),
        "PROFESOR" => format!(
            "EXISTS(SELECT 1 FROM integraciones_academicas i WHERE i.reto_id=r.id AND i.profesor_id={actor})"
        ),
        "ESTUDIANTE" => format!(
            "EXISTS(SELECT 1 FROM postulaciones p WHERE p.reto_id=r.id AND p.estudiante_id={actor} AND p.estado='ACEPTADA')"
        ),
        _ => "FALSE".into(),
    };
    let via_challenge = |column: &str| {
        format!(
            "EXISTS(SELECT 1 FROM retos r WHERE r.id=t.{column} AND r.eliminado_en IS NULL AND ({challenge}))"
        )
    };
    Some(match table {
        "facultades" | "programas" | "ecosistemas" | "unidades_estudio" => "TRUE".into(),
        "usuarios" => format!("t.id={actor}"),
        "profesores" if user.rol == "PROFESOR" => format!("t.usuario_id={actor}"),
        "estudiantes" if user.rol == "ESTUDIANTE" => format!("t.usuario_id={actor}"),
        "empresas" if user.rol == "EMPRESA" => format!("t.usuario_id={actor}"),
        "contactos_empresa" | "documentos_empresas" if user.rol == "EMPRESA" => format!("EXISTS(SELECT 1 FROM empresas e WHERE e.id=t.empresa_id AND e.usuario_id={actor})"),
        "retos" => match user.rol.as_str() {
            "EMPRESA" => format!("t.eliminado_en IS NULL AND t.empresa_id={actor}"),
            "PROFESOR" => "t.eliminado_en IS NULL AND t.estado IN ('aprobado','abierto','en_curso','pausado','finalizado')".into(),
            "ESTUDIANTE" => "t.eliminado_en IS NULL AND t.estado IN ('abierto','en_curso','pausado','finalizado')".into(),
            _ => return None,
        },
        "integraciones_academicas" if user.rol == "PROFESOR" => format!("t.profesor_id={actor}"),
        "integraciones_academicas" if user.rol == "EMPRESA" => via_challenge("reto_id"),
        "postulaciones" if user.rol == "ESTUDIANTE" => format!("t.estudiante_id={actor}"),
        "postulaciones" if user.rol == "PROFESOR" || user.rol == "EMPRESA" => via_challenge("reto_id"),
        "entregables" if user.rol == "ESTUDIANTE" => format!("t.estudiante_id={actor}"),
        "entregables" if user.rol == "PROFESOR" => via_challenge("reto_id"),
        "retos_favoritos" => format!("t.usuario_id={actor}"),
        "notificaciones" | "preferencias_notificacion" | "certificados" => format!("t.usuario_id={actor}"),
        "cierre_encuestasatisfaccion" => format!("t.participante_id={actor}"),
        "cierre_agendacierre" => via_challenge("reto_id"),
        "cierre_entregablefinal" => format!("EXISTS(SELECT 1 FROM cierres_reto c JOIN retos r ON r.id=c.reto_id WHERE c.id=t.cierre_id AND ({challenge}))"),
        "equipos" | "equipos_reto_academico" | "seguimientos_reto" => via_challenge("reto_id"),
        // HU03: the schedule is visible to every party, including assigned participants.
        "sesiones_reto_academico" => format!("({}) OR EXISTS(SELECT 1 FROM participantes_integracion p WHERE p.integracion_id=t.integracion_id AND p.usuario_id={actor})", via_challenge("reto_id")),
        "participantes_integracion" => match user.rol.as_str() {
            "PROFESOR" => format!("EXISTS(SELECT 1 FROM integraciones_academicas i WHERE i.id=t.integracion_id AND i.profesor_id={actor})"),
            "EMPRESA" => format!("EXISTS(SELECT 1 FROM integraciones_academicas i JOIN retos r ON r.id=i.reto_id WHERE i.id=t.integracion_id AND ({challenge}))"),
            _ => format!("t.usuario_id={actor}"),
        },
        "tipos_documento_legal" if user.rol == "EMPRESA" => "t.activo".into(),
        "cierre_solicitudlogistica" => via_challenge("reto_id"),
        "invitados_hackaton" => format!("EXISTS(SELECT 1 FROM hackathons h JOIN retos r ON r.id=h.reto_id WHERE h.id=t.hackathon_id AND (({challenge}) OR h.estado IN ('publicado','en_curso','finalizado') OR EXISTS(SELECT 1 FROM jurados_hackaton j WHERE j.hackathon_id=h.id AND j.usuario_id={actor})))"),
        "presupuestos" if user.rol == "EMPRESA" => via_challenge("reto_id"),
        "gastos" if user.rol == "EMPRESA" => format!("EXISTS(SELECT 1 FROM presupuestos b JOIN retos r ON r.id=b.reto_id WHERE b.id=t.presupuesto_id AND ({challenge}))"),
        "cierres_reto" => via_challenge("reto_id"),
        "hackathons" if user.rol=="ESTUDIANTE" => "t.estado IN ('publicado','en_curso','finalizado')".into(),
        "hackathons" => format!("({}) OR EXISTS(SELECT 1 FROM jurados_hackaton j WHERE j.hackathon_id=t.id AND j.usuario_id={actor})",via_challenge("reto_id")),
        "etapas_hackaton" => format!("EXISTS(SELECT 1 FROM hackathons h JOIN retos r ON r.id=h.reto_id WHERE h.id=t.hackathon_id AND (({challenge}) OR h.estado IN ('publicado','en_curso','finalizado') OR EXISTS(SELECT 1 FROM jurados_hackaton j WHERE j.hackathon_id=h.id AND j.usuario_id={actor})))"),
        "jurados_hackaton" => format!("t.usuario_id={actor}"),
        "votaciones_hackaton" => format!("EXISTS(SELECT 1 FROM jurados_hackaton j WHERE j.id=t.jurado_id AND j.usuario_id={actor})"),
        "inscripciones_hackaton" if user.rol=="ESTUDIANTE" => format!("EXISTS(SELECT 1 FROM miembros_equipo m WHERE m.equipo_id=t.equipo_id AND m.estudiante_id={actor})"),
        "inscripciones_hackaton" if user.rol=="PROFESOR" => format!("EXISTS(SELECT 1 FROM hackathons h JOIN retos r ON r.id=h.reto_id WHERE h.id=t.hackathon_id AND ({challenge}))"),
        "miembros_equipo" if user.rol == "ESTUDIANTE" => format!("t.estudiante_id={actor}"),
        _ => return None,
    })
}
/// Students see a grade and its feedback only after the professor publishes it (HU12).
pub fn data_expression_for(table: &str, user: &User) -> String {
    let expr = data_expression(table);
    if table == "entregables" && user.rol == "ESTUDIANTE" && !user.admin() {
        format!(
            "({expr})||CASE WHEN t.calificacion_publicada THEN '{{}}'::jsonb ELSE jsonb_build_object('nota',NULL,'comentario_profesor',NULL) END"
        )
    } else {
        expr
    }
}
pub fn data_expression(table: &str) -> String {
    let mut expr = if table == "usuarios" {
        "to_jsonb(t)-'password'-'is_staff'-'is_superuser'".to_string()
    } else if table == "retos" {
        // Reto.codigo, e.g. RETO-2026-001; empty until approved.
        "to_jsonb(t)||jsonb_build_object('codigo',CASE WHEN t.consecutivo IS NULL OR t.anio_consecutivo IS NULL THEN '' ELSE (CASE t.tipo WHEN 'hackathon' THEN 'HACK' ELSE 'RETO' END)||'-'||t.anio_consecutivo||'-'||lpad(t.consecutivo::text,greatest(3,length(t.consecutivo::text)),'0') END)".to_string()
    } else {
        "to_jsonb(t)".to_string()
    };
    // Like Django's {{ obj.reto }} / get_full_name in list templates: each foreign
    // key also gets its related record's display name, keyed without the _id suffix.
    let fields = schema()
        .iter()
        .find(|m| m["table"] == table)
        .and_then(|m| m["fields"].as_array());
    for f in fields.into_iter().flatten() {
        let (Some(name), Some(related)) = (f["name"].as_str(), f["related_table"].as_str()) else {
            continue;
        };
        let label = match related {
            "usuarios" => "coalesce(nullif(trim(x.first_name||' '||x.last_name),''),x.username)",
            "retos" | "entregables" => "x.titulo",
            "empresas" => "x.razon_social",
            "facultades"
            | "programas"
            | "ecosistemas"
            | "equipos"
            | "evaluacion_rubrica"
            | "evaluacion_criteriorubrica"
            | "jurados_hackaton" => "x.nombre",
            // Integrations and hackathons are known by their reto's title.
            "integraciones_academicas" | "hackathons" => {
                "(SELECT r.titulo FROM retos r WHERE r.id=x.reto_id)"
            }
            _ => continue,
        };
        let key = name.trim_end_matches("_id");
        expr.push_str(&format!(
            "||jsonb_build_object('{key}',(SELECT {label} FROM \"{related}\" x WHERE x.id=t.\"{name}\"))"
        ));
        if related == "retos" {
            // The reto's state decides which actions apply (e.g. editing an integration).
            expr.push_str(&format!(
                "||jsonb_build_object('{key}_estado',(SELECT x.estado FROM retos x WHERE x.id=t.\"{name}\"))"
            ));
        }
    }
    expr
}
pub async fn catalog(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, false).await?;
    let items: Vec<_> = schema()
        .iter()
        .filter(|m| scope(m["table"].as_str().unwrap_or(""), &user).is_some())
        .map(|m| {
            let mut item = m.clone();
            if let Some(fields) = item["fields"].as_array_mut() {
                fields.retain(|f| {
                    !["password", "is_staff", "is_superuser"]
                        .contains(&f["name"].as_str().unwrap_or(""))
                });
            }
            item
        })
        .collect();
    Ok(Json(json!(items)))
}
#[derive(Default, Deserialize)]
pub struct Filters {
    pub q: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}
pub async fn rows(
    state: &AppState,
    user: &User,
    name: &str,
    filters: &Filters,
    export: bool,
) -> Result<Value> {
    let meta = resource(name)?;
    let table = meta["table"].as_str().ok_or_else(AppError::not_found)?;
    let policy = scope(table, user).ok_or_else(AppError::forbidden)?;
    let expr = data_expression_for(table, user);
    let page = filters.page.unwrap_or(1).clamp(1, 100000);
    let limit = if export {
        5000
    } else {
        filters.limit.unwrap_or(20).clamp(1, 100)
    };
    let q = filters.q.as_deref().unwrap_or("");
    if q.len() > 200 {
        return Err(AppError::bad("Búsqueda demasiado larga"));
    }
    let search = format!(
        "%{}%",
        q.replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    );
    let base = format!(
        "WITH actor AS (SELECT $1::bigint id) SELECT {expr} data FROM \"{table}\" t WHERE ({policy}) AND ({expr})::text ILIKE $2"
    );
    let count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM ({base}) visible"))
        .bind(user.id)
        .bind(&search)
        .fetch_one(&state.pool)
        .await?;
    let records: Vec<Value> =
        sqlx::query_scalar(&format!("{base} ORDER BY t.id DESC LIMIT $3 OFFSET $4"))
            .bind(user.id)
            .bind(&search)
            .bind(i64::from(limit))
            .bind(if export {
                0
            } else {
                i64::from(page - 1) * i64::from(limit)
            })
            .fetch_all(&state.pool)
            .await?;
    Ok(json!({"records":records,"total":count,"page":page,"limit":limit,"resource":meta["name"]}))
}
pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(name): Path<String>,
    Query(filters): Query<Filters>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, false).await?;
    Ok(Json(rows(&state, &user, &name, &filters, false).await?))
}
pub async fn detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((name, id)): Path<(String, i64)>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, false).await?;
    crate::event::advance_before_read(&state, &name, id).await;
    let record = visible_record(&state, &user, &name, id).await?;
    Ok(Json(record))
}
pub async fn visible_record(state: &AppState, user: &User, name: &str, id: i64) -> Result<Value> {
    resource(name)?;
    let policy = scope(name, user).ok_or_else(AppError::forbidden)?;
    let expression = data_expression_for(name, user);
    sqlx::query_scalar(&format!("WITH actor AS (SELECT $1::bigint id) SELECT {expression} FROM \"{name}\" t WHERE t.id=$2 AND ({policy})")).bind(user.id).bind(id).fetch_optional(&state.pool).await?.ok_or_else(AppError::not_found)
}
pub async fn export(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(name): Path<String>,
    Query(filters): Query<Filters>,
) -> Result<Response> {
    let user = auth::current(&state, &headers, false).await?;
    let data = rows(&state, &user, &name, &filters, true).await?;
    let fields: Vec<&str> = resource(&name)?["fields"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|f| f["name"].as_str())
        .filter(|f| !["password", "is_staff", "is_superuser"].contains(f))
        .collect();
    let mut csv = format!(
        "\u{feff}{}\r\n",
        fields
            .iter()
            .map(|f| crate::domain::csv_cell(&json!(f)))
            .collect::<Vec<_>>()
            .join(",")
    );
    for row in data["records"].as_array().unwrap() {
        csv.push_str(
            &fields
                .iter()
                .map(|f| crate::domain::csv_cell(&row[*f]))
                .collect::<Vec<_>>()
                .join(","),
        );
        csv.push_str("\r\n");
    }
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=retosean.csv",
            ),
            (
                header::HeaderName::from_static("x-total-count"),
                data["total"].to_string().as_str(),
            ),
        ],
        csv,
    )
        .into_response())
}
pub async fn verify_certificate(
    State(state): State<AppState>,
    Path(code): Path<uuid::Uuid>,
) -> Result<Json<Value>> {
    let record: Option<Value> = sqlx::query_scalar("SELECT jsonb_build_object('codigo',c.codigo_verificacion,'participante',concat(u.first_name,' ',u.last_name),'reto',r.titulo,'rol',c.rol_participacion,'horas',c.horas,'emitido_en',c.emitido_en) FROM certificados c JOIN usuarios u ON u.id=c.usuario_id JOIN retos r ON r.id=c.reto_id WHERE c.codigo_verificacion=$1").bind(code).fetch_optional(&state.pool).await?;
    Ok(Json(record.ok_or_else(AppError::not_found)?))
}
