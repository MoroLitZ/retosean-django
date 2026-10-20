//! Port of Django's apps/dashboard (HU05): same KPIs, series and period filter per role.
use crate::{AppState, auth, error::Result, resources::schema};
use axum::{
    Json,
    extract::{Query, State},
    http::HeaderMap,
};
use chrono::{Duration, NaiveDate};
use serde::Deserialize;
use serde_json::{Value, json};

const COLORES: [&str; 8] = [
    "#2563eb", "#16a34a", "#f59e0b", "#dc2626", "#7c3aed", "#0891b2", "#65a30d", "#db2777",
];
const PERIODOS: [(&str, &str); 5] = [
    ("30d", "Ultimos 30 dias"),
    ("90d", "Ultimos 90 dias"),
    ("anio", "Ultimo anio"),
    ("todo", "Historico completo"),
    ("custom", "Rango personalizado"),
];
// Every query shares these parameters; Django filters on the local (Bogota) date.
const PARAMS: &str = "WITH p AS (SELECT $1::bigint uid, $2::date desde, $3::date hasta, $4::bigint fac, $5::bigint prog, $6::bigint eco)";
const EN_RANGO: &str = "(p.desde IS NULL OR (t.{c} AT TIME ZONE 'America/Bogota')::date >= p.desde) AND (p.hasta IS NULL OR (t.{c} AT TIME ZONE 'America/Bogota')::date <= p.hasta)";

type Params = (
    i64,
    Option<NaiveDate>,
    Option<NaiveDate>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
);

#[derive(Deserialize, Default)]
pub struct Filtro {
    periodo: Option<String>,
    desde: Option<String>,
    hasta: Option<String>,
    facultad: Option<String>,
    programa: Option<String>,
    ecosistema: Option<String>,
}

/// Same rules as apps/dashboard/filters.py::rango_de_fechas.
fn rango(f: &Filtro, hoy: NaiveDate) -> (Option<NaiveDate>, Option<NaiveDate>, String, String) {
    let fecha = |v: &Option<String>| {
        v.as_deref()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
    };
    match f.periodo.as_deref().unwrap_or("90d") {
        "todo" => (None, None, "Historico completo".into(), "todo".into()),
        "custom" => {
            let (mut desde, mut hasta) = (fecha(&f.desde), fecha(&f.hasta));
            if let (Some(a), Some(b)) = (desde, hasta)
                && a > b
            {
                (desde, hasta) = (Some(b), Some(a));
            }
            let etiqueta = match (desde, hasta) {
                (Some(a), Some(b)) => {
                    format!("{} - {}", a.format("%d/%m/%Y"), b.format("%d/%m/%Y"))
                }
                _ => "Rango personalizado".into(),
            };
            (desde, hasta, etiqueta, "custom".into())
        }
        otro => {
            let (periodo, dias) = match otro {
                "30d" => ("30d", 30),
                "anio" => ("anio", 365),
                _ => ("90d", 90),
            };
            let etiqueta = PERIODOS.iter().find(|p| p.0 == periodo).unwrap().1;
            (
                Some(hoy - Duration::days(dias)),
                Some(hoy),
                etiqueta.into(),
                periodo.into(),
            )
        }
    }
}

/// Display label of a choice value, as Django's get_FOO_display.
fn etiqueta(tabla: &str, campo: &str, valor: &str) -> String {
    schema()
        .iter()
        .filter(|m| m["table"] == tabla)
        .flat_map(|m| m["fields"].as_array().into_iter().flatten())
        .filter(|f| f["name"] == campo)
        .flat_map(|f| f["choices"].as_array().into_iter().flatten())
        .find(|c| c[0] == valor)
        .and_then(|c| c[1].as_str())
        .unwrap_or(valor)
        .to_string()
}

pub async fn panel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filtro): Query<Filtro>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, false).await?;
    let hoy: NaiveDate = sqlx::query_scalar("SELECT (now() AT TIME ZONE 'America/Bogota')::date")
        .fetch_one(&state.pool)
        .await?;
    let (desde, hasta, etiqueta_periodo, periodo) = rango(&filtro, hoy);
    let rol = if user.admin() {
        "ADMIN"
    } else {
        user.rol.as_str()
    };
    let id = |v: &Option<String>| v.as_deref().and_then(|s| s.parse::<i64>().ok());
    let (fac, prog, eco) = if rol == "ADMIN" {
        (
            id(&filtro.facultad),
            id(&filtro.programa),
            id(&filtro.ecosistema),
        )
    } else {
        (None, None, None)
    };
    let retos = |extra: &str| {
        format!(
            "{PARAMS}, r AS (SELECT t.* FROM retos t, p WHERE t.eliminado_en IS NULL AND {} AND (p.fac IS NULL OR t.facultad_id = p.fac) AND (p.prog IS NULL OR t.programa_id = p.prog) AND (p.eco IS NULL OR t.ecosistema_id = p.eco) AND {extra})",
            EN_RANGO.replace("{c}", "creado_en")
        )
    };
    let por_estado = "SELECT estado, count(*)::float8 FROM r GROUP BY estado ORDER BY 2 DESC";
    let por_mes = "SELECT to_char(m, 'MM/YYYY'), n FROM (SELECT date_trunc('month', creado_en AT TIME ZONE 'America/Bogota') m, count(*)::float8 n FROM r GROUP BY 1) x ORDER BY m";
    // (cte, kpis, [(serie, query, choice labels from table.field)])
    let (cte, kpis, series): (String, &str, Vec<(&str, &str, Option<(&str, &str)>)>) = match rol {
        "ADMIN" => (
            retos("TRUE"),
            "SELECT jsonb_build_object(
                'impacto_si', (SELECT count(*) FROM cierres_reto WHERE reto_id IN (SELECT id FROM r) AND impacto = 'SI'),
                'impacto_no', (SELECT count(*) FROM cierres_reto WHERE reto_id IN (SELECT id FROM r) AND impacto = 'NO'),
                'impacto_evaluacion', (SELECT count(*) FROM cierres_reto WHERE reto_id IN (SELECT id FROM r) AND impacto = 'EN_EVALUACION'),
                'retos_total', (SELECT count(*) FROM r),
                'retos_activos', (SELECT count(*) FROM r WHERE estado IN ('aprobado','abierto','en_curso','pausado')),
                'retos_finalizados', (SELECT count(*) FROM r WHERE estado = 'finalizado'),
                'retos_en_revision', (SELECT count(*) FROM r WHERE estado = 'en_revision'),
                'empresas_total', (SELECT count(*) FROM empresas),
                'empresas_verificadas', (SELECT count(*) FROM empresas WHERE estado_validacion = 'VERIFICADA'),
                'estudiantes_total', (SELECT count(*) FROM usuarios WHERE rol = 'ESTUDIANTE' AND is_active),
                'profesores_total', (SELECT count(*) FROM usuarios WHERE rol = 'PROFESOR' AND is_active),
                'postulaciones_total', (SELECT count(*) FROM postulaciones WHERE reto_id IN (SELECT id FROM r)),
                'postulaciones_aceptadas', (SELECT count(*) FROM postulaciones WHERE reto_id IN (SELECT id FROM r) AND estado = 'ACEPTADA'),
                'entregables_total', (SELECT count(*) FROM entregables WHERE reto_id IN (SELECT id FROM r)),
                'entregables_calificados', (SELECT count(*) FROM entregables WHERE reto_id IN (SELECT id FROM r) AND estado = 'CALIFICADO'))",
            vec![
                ("retos_por_estado", por_estado, Some(("retos", "estado"))),
                ("retos_por_facultad", "SELECT coalesce(f.nombre, 'Sin facultad'), count(*)::float8 FROM r LEFT JOIN facultades f ON f.id = r.facultad_id GROUP BY 1 ORDER BY 2 DESC LIMIT 8", None),
                ("retos_por_mes", por_mes, None),
                ("integraciones_por_estado", "SELECT estado, count(*)::float8 FROM integraciones_academicas WHERE reto_id IN (SELECT id FROM r) GROUP BY estado ORDER BY 2 DESC", Some(("integraciones_academicas", "estado"))),
                (
                    "impacto",
                    "SELECT impacto, count(*)::float8 FROM cierres_reto WHERE reto_id IN (SELECT id FROM r) GROUP BY impacto ORDER BY 2 DESC",
                    Some(("cierres_reto", "impacto")),
                ),
            ],
        ),
        "EMPRESA" => (
            retos("t.empresa_id = p.uid"),
            "SELECT jsonb_build_object(
                'retos_total', (SELECT count(*) FROM r),
                'retos_activos', (SELECT count(*) FROM r WHERE estado IN ('aprobado','abierto','en_curso','pausado')),
                'retos_finalizados', (SELECT count(*) FROM r WHERE estado = 'finalizado'),
                'retos_en_revision', (SELECT count(*) FROM r WHERE estado = 'en_revision'),
                'postulaciones_total', (SELECT count(*) FROM postulaciones WHERE reto_id IN (SELECT id FROM r)),
                'postulaciones_pendientes', (SELECT count(*) FROM postulaciones WHERE reto_id IN (SELECT id FROM r) AND estado = 'PENDIENTE'),
                'postulaciones_aceptadas', (SELECT count(*) FROM postulaciones WHERE reto_id IN (SELECT id FROM r) AND estado = 'ACEPTADA'),
                'integraciones_total', (SELECT count(*) FROM integraciones_academicas WHERE reto_id IN (SELECT id FROM r)),
                'entregables_total', (SELECT count(*) FROM entregables WHERE reto_id IN (SELECT id FROM r)))",
            vec![
                ("retos_por_estado", por_estado, Some(("retos", "estado"))),
                ("retos_por_mes", por_mes, None),
            ],
        ),
        "PROFESOR" => (
            format!(
                "{PARAMS}, i AS (SELECT t.* FROM integraciones_academicas t, p WHERE t.profesor_id = p.uid AND {}), e AS (SELECT * FROM entregables WHERE reto_id IN (SELECT reto_id FROM i))",
                EN_RANGO.replace("{c}", "creado_en")
            ),
            "SELECT jsonb_build_object(
                'cursos_activos', (SELECT count(*) FROM i WHERE estado IN ('aprobada','publicada')),
                'retos_asignados', (SELECT count(*) FROM i),
                'integraciones_en_revision', (SELECT count(*) FROM i WHERE estado = 'en_revision'),
                'estudiantes_total', (SELECT count(DISTINCT s.usuario_id) FROM equipos_reto_academico_estudiantes s JOIN equipos_reto_academico q ON q.id = s.equiporetoacademico_id WHERE q.reto_id IN (SELECT reto_id FROM i)),
                'entregables_total', (SELECT count(*) FROM e),
                'evaluaciones_pendientes', (SELECT count(*) FROM e WHERE estado IN ('ENVIADO','EN_REVISION')),
                'nota_promedio', (SELECT avg(nota)::float8 FROM e))",
            vec![
                ("entregables_por_estado", "SELECT estado, count(*)::float8 FROM e GROUP BY estado ORDER BY 2 DESC", Some(("entregables", "estado"))),
                ("entregables_por_reto", "SELECT coalesce(rt.titulo, 'Sin titulo'), count(*)::float8 FROM e LEFT JOIN retos rt ON rt.id = e.reto_id GROUP BY 1 ORDER BY 2 DESC LIMIT 8", None),
            ],
        ),
        "ESTUDIANTE" => (
            format!(
                "{PARAMS}, po AS (SELECT t.* FROM postulaciones t, p WHERE t.estudiante_id = p.uid AND {}), e AS (SELECT t.* FROM entregables t, p WHERE t.estudiante_id = p.uid)",
                EN_RANGO.replace("{c}", "fecha_postulacion")
            ),
            "SELECT jsonb_build_object(
                'retos_disponibles', (SELECT count(*) FROM retos WHERE estado = 'abierto' AND eliminado_en IS NULL),
                'favoritos_total', (SELECT count(*) FROM retos_favoritos, p WHERE usuario_id = p.uid),
                'postulaciones_total', (SELECT count(*) FROM po),
                'postulaciones_pendientes', (SELECT count(*) FROM po WHERE estado = 'PENDIENTE'),
                'postulaciones_aceptadas', (SELECT count(*) FROM po WHERE estado = 'ACEPTADA'),
                'entregables_total', (SELECT count(*) FROM e),
                'entregables_pendientes', (SELECT count(*) FROM e WHERE estado IN ('ENVIADO','EN_REVISION')),
                'nota_promedio', (SELECT avg(nota)::float8 FROM e))",
            vec![
                ("postulaciones_por_estado", "SELECT estado, count(*)::float8 FROM po GROUP BY estado ORDER BY 2 DESC", Some(("postulaciones", "estado"))),
                ("notas_recientes", "SELECT k, v FROM (SELECT left(titulo, 24) k, nota::float8 v, fecha_entrega FROM e WHERE nota IS NOT NULL ORDER BY fecha_entrega DESC LIMIT 8) x ORDER BY fecha_entrega", None),
            ],
        ),
        // Django kpis_experto / series_experto: the expert as hackathon juror.
        "EXPERTO" => (
            format!(
                "{PARAMS}, j AS (SELECT * FROM jurados_hackaton, p WHERE usuario_id = p.uid), v AS (SELECT t.* FROM votaciones_hackaton t, p WHERE t.jurado_id IN (SELECT id FROM j) AND {})",
                EN_RANGO.replace("{c}", "fecha")
            ),
            "SELECT jsonb_build_object(
                'hackathones_asignados', (SELECT count(DISTINCT hackathon_id) FROM j),
                'hackathones_en_curso', (SELECT count(*) FROM j JOIN hackathons h ON h.id = j.hackathon_id WHERE h.estado = 'en_curso'),
                'equipos_por_evaluar', (SELECT count(*) FROM inscripciones_hackaton i WHERE i.estado = 'ACEPTADA' AND i.hackathon_id IN (SELECT hackathon_id FROM j)
                    AND NOT EXISTS (SELECT 1 FROM votaciones_hackaton x WHERE x.jurado_id IN (SELECT id FROM j) AND x.hackathon_id = i.hackathon_id AND x.equipo_id = i.equipo_id)),
                'votos_emitidos', (SELECT count(*) FROM v),
                'puntaje_promedio', (SELECT avg(puntaje)::float8 FROM v),
                'equipos_academicos', (SELECT count(*) FROM equipos_reto_academico_expertos_invitados e, p WHERE e.usuario_id = p.uid))",
            vec![(
                "votos_por_hackathon",
                "SELECT coalesce(r.titulo, 'Sin titulo'), count(*)::float8 FROM v JOIN hackathons h ON h.id = v.hackathon_id JOIN retos r ON r.id = h.reto_id GROUP BY 1 ORDER BY 2 DESC LIMIT 8",
                None,
            )],
        ),
        // Like Django, a user without one of the panel roles sees the "sin rol" notice.
        _ => (String::new(), "", vec![]),
    };
    let mut cuerpo = json!({
        "rol": rol, "periodo": periodo, "etiqueta_periodo": etiqueta_periodo,
        "periodos": PERIODOS, "kpis": {}, "series": {}, "sin_rol": cte.is_empty(),
    });
    if cte.is_empty() {
        return Ok(Json(cuerpo));
    }
    let params = (user.id, desde, hasta, fac, prog, eco);
    fn bind(
        sql: &str,
        p: Params,
    ) -> sqlx::query::Query<'_, sqlx::Postgres, sqlx::postgres::PgArguments> {
        sqlx::query(sql)
            .bind(p.0)
            .bind(p.1)
            .bind(p.2)
            .bind(p.3)
            .bind(p.4)
            .bind(p.5)
    }
    use sqlx::Row;
    let sql = format!("{cte} {kpis}");
    cuerpo["kpis"] = bind(&sql, params)
        .fetch_one(&state.pool)
        .await?
        .try_get(0)?;
    for (nombre, sql, choices) in series {
        let sql = format!("{cte} {sql}");
        let filas = bind(&sql, params).fetch_all(&state.pool).await?;
        let mut labels = vec![];
        let mut data = vec![];
        for fila in filas {
            let clave: Option<String> = fila.try_get(0)?;
            let clave = clave.unwrap_or_default();
            labels.push(match choices {
                Some((tabla, campo)) => etiqueta(tabla, campo, &clave),
                None => clave,
            });
            data.push(fila.try_get::<f64, _>(1)?);
        }
        let colors: Vec<_> = (0..data.len())
            .map(|i| COLORES[i % COLORES.len()])
            .collect();
        cuerpo["series"][nombre] = json!({"labels": labels, "data": data, "colors": colors});
    }
    if rol == "ADMIN" {
        cuerpo["facultades"] = sqlx::query_scalar(
            "SELECT coalesce(jsonb_agg(jsonb_build_object('id', id, 'nombre', nombre) ORDER BY nombre), '[]') FROM facultades",
        )
        .fetch_one(&state.pool)
        .await?;
        cuerpo["ecosistemas"] = sqlx::query_scalar(
            "SELECT coalesce(jsonb_agg(jsonb_build_object('id', id, 'nombre', nombre) ORDER BY nombre), '[]') FROM ecosistemas",
        )
        .fetch_one(&state.pool)
        .await?;
        cuerpo["programas"] = sqlx::query_scalar(
            "SELECT coalesce(jsonb_agg(jsonb_build_object('id', id, 'nombre', nombre, 'facultad_id', facultad_id) ORDER BY nombre), '[]') FROM programas",
        )
        .fetch_one(&state.pool)
        .await?;
    }
    Ok(Json(cuerpo))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn periodos_como_django() {
        let hoy = NaiveDate::from_ymd_opt(2026, 10, 14).unwrap();
        let filtro = |periodo: &str, desde: &str, hasta: &str| Filtro {
            periodo: Some(periodo.into()),
            desde: Some(desde.into()),
            hasta: Some(hasta.into()),
            ..Default::default()
        };
        let (d, h, e, p) = rango(&Filtro::default(), hoy);
        assert_eq!(
            (d, h, e.as_str(), p.as_str()),
            (
                Some(hoy - Duration::days(90)),
                Some(hoy),
                "Ultimos 90 dias",
                "90d"
            )
        );
        assert_eq!(rango(&filtro("todo", "", ""), hoy).0, None);
        assert_eq!(rango(&filtro("raro", "", ""), hoy).3, "90d");
        let (d, h, e, _) = rango(&filtro("custom", "2026-10-10", "2026-10-01"), hoy);
        assert_eq!(
            (d.unwrap().to_string(), h.unwrap().to_string()),
            ("2026-10-01".into(), "2026-10-10".into())
        );
        assert_eq!(e, "01/10/2026 - 10/10/2026");
        assert_eq!(etiqueta("postulaciones", "estado", "ACEPTADA"), "Aceptada");
    }
}
