//! Report builder (Django apps/reportes, HU06): the same five reports with their
//! columns, filters and roles; preview, PDF/Excel generation logged in
//! reportes_generados, and sending a report by email.
use crate::{
    AppState, auth,
    auth::User,
    error::{AppError, Result},
    mail,
    reports::pdf_lines,
};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::{Duration, NaiveDate};
use serde::Deserialize;
use serde_json::{Value, json};

/// (key, label, width, Some((table, field)) when the value is a choice code).
type Col = (
    &'static str,
    &'static str,
    u16,
    Option<(&'static str, &'static str)>,
);

struct Report {
    key: &'static str,
    name: &'static str,
    description: &'static str,
    roles: &'static [&'static str],
    columns: &'static [Col],
    /// SELECT of one jsonb object per row, aliased `fila`, with `{where}` for filters.
    sql: &'static str,
    order: &'static str,
    /// (filter key, SQL predicate on $1), applied only to this report, as in catalogo.py.
    filters: &'static [(&'static str, &'static str)],
    date_column: Option<&'static str>,
}

// Timestamp columns print their UTC date, as Django's _fecha on aware datetimes does.
macro_rules! d {
    ($e:expr) => {
        concat!(
            "coalesce(to_char((",
            $e,
            ") AT TIME ZONE 'UTC','DD/MM/YYYY'),'')"
        )
    };
}
macro_rules! day {
    ($e:expr) => {
        concat!("coalesce(to_char(", $e, ",'DD/MM/YYYY'),'')")
    };
}

const REPORTS: [Report; 5] = [
    Report {
        key: "retos",
        name: "Retos publicados",
        description: "Listado de retos con su empresa, estado y fechas clave.",
        roles: &["ADMIN"],
        columns: &[
            ("consecutivo", "N.o de reto", 14, None),
            ("titulo", "Titulo", 40, None),
            ("empresa", "Empresa", 28, None),
            ("estado", "Estado", 16, Some(("retos", "estado"))),
            ("tipo", "Tipo", 16, Some(("retos", "tipo"))),
            ("area", "Area", 22, None),
            ("nivel_academico", "Nivel academico", 20, None),
            ("facultad", "Facultad", 28, None),
            ("programa", "Programa", 28, None),
            ("ecosistema", "Ecosistema", 24, None),
            ("convenio", "Convenio institucional", 18, None),
            ("premios", "Premios", 40, None),
            ("criterios_evaluacion", "Criterios de evaluacion", 44, None),
            ("fecha_inicio", "Inicio tentativo", 18, None),
            ("fecha_fin", "Fin tentativo", 18, None),
            (
                "fecha_limite_postulacion",
                "Limite de postulacion",
                20,
                None,
            ),
            ("fecha_aprobacion", "Fecha de aprobacion", 20, None),
            ("postulaciones", "Postulaciones", 14, None),
            ("equipos_academicos", "Equipos academicos", 18, None),
            ("archivos", "Archivos adjuntos", 16, None),
            ("comentarios_revision", "Comentarios de revision", 40, None),
            ("nombre_corto", "Nombre corto", 22, None),
            ("uso_esperado", "Uso esperado de los resultados", 40, None),
            ("insumos_inicio", "Insumos al inicio", 36, None),
            ("insumos_cierre", "Insumos al cierre", 36, None),
            (
                "impacto",
                "La empresa aplico la solucion",
                24,
                Some(("cierres_reto", "impacto")),
            ),
            ("creado_en", "Creado", 16, None),
        ],
        sql: concat!(
            "SELECT jsonb_build_object('consecutivo',coalesce(r.consecutivo::text,''),'titulo',coalesce(r.titulo,''),",
            "'empresa',(SELECT u.username||' ('||u.rol||')' FROM usuarios u WHERE u.id=r.empresa_id),",
            "'estado',r.estado,'tipo',coalesce(r.tipo,''),'area',coalesce(r.area,''),'nivel_academico',coalesce(r.nivel_academico,''),",
            "'facultad',coalesce((SELECT nombre FROM facultades WHERE id=r.facultad_id),''),",
            "'programa',coalesce((SELECT p.nombre||' - '||f.nombre FROM programas p JOIN facultades f ON f.id=p.facultad_id WHERE p.id=r.programa_id),''),",
            "'ecosistema',coalesce((SELECT nombre FROM ecosistemas WHERE id=r.ecosistema_id),''),",
            "'convenio',CASE WHEN r.tiene_convenio_institucional THEN 'Si' ELSE 'No' END,",
            "'premios',coalesce(r.premios,''),'criterios_evaluacion',coalesce(r.criterios_evaluacion,''),",
            "'fecha_inicio',",
            day!("r.fecha_inicio_tentativa"),
            ",'fecha_fin',",
            day!("r.fecha_fin_tentativa"),
            ",",
            "'fecha_limite_postulacion',",
            day!("r.fecha_limite_postulacion"),
            ",'fecha_aprobacion',",
            d!("r.fecha_aprobacion"),
            ",",
            "'postulaciones',(SELECT count(*) FROM postulaciones WHERE reto_id=r.id),",
            "'equipos_academicos',(SELECT count(*) FROM equipos_reto_academico WHERE reto_id=r.id),",
            "'archivos',(SELECT count(*) FROM retos_retoarchivo WHERE reto_id=r.id),",
            "'comentarios_revision',coalesce(r.comentarios_revision,''),'nombre_corto',coalesce(r.nombre_corto,''),",
            "'uso_esperado',coalesce(r.uso_esperado_resultados,''),'insumos_inicio',coalesce(r.insumos_inicio,''),'insumos_cierre',coalesce(r.insumos_cierre,''),",
            "'impacto',coalesce((SELECT c.impacto FROM cierres_reto c WHERE c.reto_id=r.id),''),",
            "'creado_en',",
            d!("r.creado_en"),
            ") fila FROM retos r WHERE r.eliminado_en IS NULL {where}"
        ),
        order: "r.creado_en DESC, r.id DESC",
        filters: &[
            ("estado", "r.estado=$1"),
            ("facultad", "r.facultad_id=$1::bigint"),
            ("programa", "r.programa_id=$1::bigint"),
            ("empresa", "r.empresa_id=$1::bigint"),
        ],
        date_column: Some("r.creado_en"),
    },
    Report {
        key: "participacion",
        name: "Participacion estudiantil",
        description: "Postulaciones de estudiantes a retos, con su estado y programa.",
        roles: &["ADMIN", "PROFESOR"],
        columns: &[
            ("estudiante", "Estudiante", 30, None),
            ("correo", "Correo", 32, None),
            ("telefono", "Telefono", 16, None),
            ("programa", "Programa", 28, None),
            ("semestre", "Semestre", 12, None),
            ("reto", "Reto", 36, None),
            ("empresa", "Empresa", 26, None),
            ("area_reto", "Area del reto", 22, None),
            ("estado", "Estado", 20, Some(("postulaciones", "estado"))),
            ("motivacion", "Motivacion", 44, None),
            ("habilidades", "Habilidades", 36, None),
            ("hoja_de_vida", "Adjunto hoja de vida", 18, None),
            ("acepta_terminos", "Acepto los terminos", 18, None),
            ("fecha", "Fecha de postulacion", 20, None),
        ],
        sql: concat!(
            "SELECT jsonb_build_object('estudiante',coalesce(nullif(trim(u.first_name||' '||u.last_name),''),u.username),",
            "'correo',u.email,'telefono',coalesce(u.telefono,''),'programa',coalesce(p.programa,''),'semestre',coalesce(p.semestre::text,''),",
            "'reto',coalesce(r.titulo,''),'empresa',eu.username||' ('||eu.rol||')','area_reto',coalesce(r.area,''),'estado',p.estado,",
            "'motivacion',coalesce(p.motivacion,''),'habilidades',coalesce(p.habilidades,''),",
            "'hoja_de_vida',CASE WHEN coalesce(p.hoja_de_vida,'')<>'' THEN 'Si' ELSE 'No' END,",
            "'acepta_terminos',CASE WHEN p.acepta_terminos THEN 'Si' ELSE 'No' END,'fecha',",
            d!("p.fecha_postulacion"),
            ") fila FROM postulaciones p JOIN usuarios u ON u.id=p.estudiante_id JOIN retos r ON r.id=p.reto_id JOIN usuarios eu ON eu.id=r.empresa_id WHERE TRUE {where}"
        ),
        order: "p.fecha_postulacion DESC, p.id DESC",
        filters: &[
            ("estado_postulacion", "p.estado=$1"),
            ("facultad", "r.facultad_id=$1::bigint"),
            ("programa", "r.programa_id=$1::bigint"),
            ("empresa", "r.empresa_id=$1::bigint"),
        ],
        date_column: Some("p.fecha_postulacion"),
    },
    Report {
        key: "evaluacion",
        name: "Evaluacion de entregables",
        description: "Entregables recibidos, su estado, nota y retroalimentacion.",
        roles: &["ADMIN", "PROFESOR"],
        columns: &[
            ("reto", "Reto", 34, None),
            ("estudiante", "Estudiante", 30, None),
            ("correo", "Correo", 32, None),
            ("equipo", "Equipo", 22, None),
            ("titulo", "Entregable", 30, None),
            ("es_final", "Entrega final", 14, None),
            ("estado", "Estado", 16, Some(("entregables", "estado"))),
            ("nota", "Nota", 10, None),
            ("puntaje_maximo", "Sobre", 10, None),
            (
                "comentario_estudiante",
                "Comentario del estudiante",
                40,
                None,
            ),
            ("comentario", "Retroalimentacion", 44, None),
            ("rubrica", "Rubrica usada", 26, None),
            ("evaluador", "Evaluado por", 26, None),
            ("historial_comentarios", "N.o de comentarios", 16, None),
            (
                "calificacion_publicada",
                "Calificacion publicada al estudiante",
                22,
                None,
            ),
            ("fecha_publicacion", "Fecha de publicacion", 18, None),
            ("fecha", "Fecha de entrega", 18, None),
            ("actualizado_en", "Ultima actualizacion", 18, None),
        ],
        sql: concat!(
            "SELECT jsonb_build_object('reto',coalesce(r.titulo,''),'estudiante',coalesce(nullif(trim(u.first_name||' '||u.last_name),''),u.username),",
            "'correo',u.email,'equipo',coalesce(q.nombre,''),'titulo',coalesce(e.titulo,''),'es_final',CASE WHEN e.es_final THEN 'Si' ELSE 'No' END,",
            "'estado',e.estado,'nota',coalesce(e.nota::text,''),'puntaje_maximo',e.puntaje_maximo::text,",
            "'comentario_estudiante',coalesce(e.comentario_estudiante,''),'comentario',coalesce(e.comentario_profesor,''),",
            "'rubrica',coalesce((SELECT rb.nombre||' - '||coalesce(nullif(rr.titulo,''),'Reto #'||rr.id) FROM evaluaciones v JOIN evaluacion_rubrica rb ON rb.id=v.rubrica_id JOIN retos rr ON rr.id=rb.reto_id WHERE v.id=(SELECT min(id) FROM evaluaciones WHERE entregable_id=e.id)),''),",
            "'evaluador',coalesce((SELECT pu.username||' ('||pu.rol||')' FROM evaluaciones v JOIN usuarios pu ON pu.id=v.profesor_id WHERE v.id=(SELECT min(id) FROM evaluaciones WHERE entregable_id=e.id)),''),",
            "'historial_comentarios',(SELECT count(*) FROM evaluacion_comentarioentregable WHERE entregable_id=e.id),",
            "'calificacion_publicada',CASE WHEN e.calificacion_publicada THEN 'Si' ELSE 'No' END,'fecha_publicacion',",
            d!("e.calificacion_publicada_en"),
            ",",
            "'fecha',",
            d!("e.fecha_entrega"),
            ",'actualizado_en',",
            d!("e.actualizado_en"),
            ") fila FROM entregables e JOIN retos r ON r.id=e.reto_id JOIN usuarios u ON u.id=e.estudiante_id LEFT JOIN equipos q ON q.id=e.equipo_id WHERE TRUE {where}"
        ),
        order: "e.fecha_entrega DESC, e.id DESC",
        filters: &[
            ("facultad", "r.facultad_id=$1::bigint"),
            ("programa", "r.programa_id=$1::bigint"),
            ("empresa", "r.empresa_id=$1::bigint"),
        ],
        date_column: Some("e.fecha_entrega"),
    },
    Report {
        key: "presupuesto",
        name: "Ejecucion presupuestal",
        description: "Presupuesto asignado, comprometido, ejecutado y disponible por reto.",
        roles: &["ADMIN"],
        columns: &[
            ("reto", "Reto", 36, None),
            ("empresa", "Empresa", 26, None),
            ("monto_total", "Presupuesto", 16, None),
            ("comprometido", "Comprometido", 16, None),
            ("ejecutado", "Ejecutado", 16, None),
            ("disponible", "Disponible", 16, None),
            ("porcentaje", "% asignado", 14, None),
            ("gastos", "N.o de gastos", 14, None),
            ("gasto_refrigerios", "Refrigerios", 16, None),
            ("gasto_materiales", "Materiales", 16, None),
            ("gasto_impresiones", "Impresiones", 16, None),
            ("gasto_espacios", "Espacios", 16, None),
            ("gasto_otros", "Otros", 16, None),
            ("alerta_80", "Alerta 80% enviada", 18, None),
        ],
        sql: concat!(
            "SELECT jsonb_build_object('reto',coalesce(r.titulo,''),'empresa',eu.username||' ('||eu.rol||')','monto_total',b.monto_total::text,",
            "'comprometido',g.comp::text,'ejecutado',g.ejec::text,'disponible',(b.monto_total-g.comp-g.ejec)::text,",
            "'porcentaje',CASE WHEN b.monto_total<>0 THEN round((g.comp+g.ejec)/b.monto_total*100,1)::text ELSE '0.0' END||'%',",
            "'gastos',g.n,'gasto_refrigerios',g.refr::text,'gasto_materiales',g.mat::text,'gasto_impresiones',g.imp::text,",
            "'gasto_espacios',g.esp::text,'gasto_otros',g.otr::text,'alerta_80',CASE WHEN b.alerta_80_enviada THEN 'Si' ELSE 'No' END",
            ") fila FROM presupuestos b JOIN retos r ON r.id=b.reto_id JOIN usuarios eu ON eu.id=r.empresa_id CROSS JOIN LATERAL (SELECT ",
            "coalesce(sum(monto) FILTER (WHERE estado='COMPROMETIDO'),0.00) comp,coalesce(sum(monto) FILTER (WHERE estado<>'COMPROMETIDO'),0.00) ejec,count(*) n,",
            "coalesce(sum(monto) FILTER (WHERE categoria='REFRIGERIOS'),0.00) refr,coalesce(sum(monto) FILTER (WHERE categoria='MATERIALES'),0.00) mat,",
            "coalesce(sum(monto) FILTER (WHERE categoria='IMPRESIONES'),0.00) imp,coalesce(sum(monto) FILTER (WHERE categoria='ESPACIOS'),0.00) esp,",
            "coalesce(sum(monto) FILTER (WHERE categoria='OTROS'),0.00) otr FROM gastos WHERE presupuesto_id=b.id) g WHERE TRUE {where}"
        ),
        order: "b.creado_en DESC, b.id DESC",
        filters: &[
            ("empresa", "r.empresa_id=$1::bigint"),
            ("facultad", "r.facultad_id=$1::bigint"),
        ],
        date_column: Some("b.creado_en"),
    },
    Report {
        key: "empresas",
        name: "Empresas aliadas",
        description: "Empresas registradas, su estado de validacion y documentacion.",
        roles: &["ADMIN"],
        columns: &[
            ("nit", "NIT", 18, None),
            ("razon_social", "Razon social", 36, None),
            ("sector", "Sector", 26, None),
            (
                "estado",
                "Estado de validacion",
                22,
                Some(("empresas", "estado_validacion")),
            ),
            (
                "listas",
                "Listas restrictivas",
                20,
                Some(("empresas", "estado_listas_restrictivas")),
            ),
            ("convenio", "Renuncio a convenio", 18, None),
            ("retos_publicados", "Retos publicados", 18, None),
            ("documentos", "Documentos cargados", 18, None),
            ("documentos_verificados", "Documentos verificados", 20, None),
            ("contacto_nombre", "Contacto principal", 28, None),
            ("contacto_cargo", "Cargo del contacto", 22, None),
            ("contacto_telefono", "Telefono del contacto", 18, None),
            ("correo", "Correo de contacto", 30, None),
            (
                "tipo_convocatoria",
                "Tipo de convocatoria",
                20,
                Some(("empresas", "tipo_convocatoria")),
            ),
            ("area_responsable", "Area responsable", 26, None),
            ("sitio_web", "Sitio web", 30, None),
            (
                "estado_convenio",
                "Tramite del convenio",
                22,
                Some(("empresas", "estado_convenio")),
            ),
            ("vence_convenio", "Vencimiento del convenio", 22, None),
            ("puede_participar", "Puede participar", 16, None),
            (
                "pendientes_documentales",
                "Pendientes para participar",
                60,
                None,
            ),
            ("bloqueada", "Participacion bloqueada", 20, None),
            ("motivo_bloqueo", "Motivo del bloqueo", 36, None),
            ("observaciones", "Observaciones del admin", 40, None),
            ("fecha_creacion", "Registrada el", 18, None),
            ("fecha_actualizacion", "Ultima actualizacion", 20, None),
        ],
        sql: concat!(
            "SELECT jsonb_build_object('nit',e.nit,'razon_social',e.razon_social,'sector',coalesce(e.sector_industrial,''),",
            "'estado',e.estado_validacion,'listas',e.estado_listas_restrictivas,'convenio',CASE WHEN e.renuncio_a_convenio THEN 'Si' ELSE 'No' END,",
            "'retos_publicados',(SELECT count(*) FROM retos WHERE empresa_id=e.usuario_id AND eliminado_en IS NULL),",
            "'documentos',(SELECT count(*) FROM documentos_empresas WHERE empresa_id=e.id),",
            "'documentos_verificados',(SELECT count(*) FROM documentos_empresas WHERE empresa_id=e.id AND estado='VERIFICADO'),",
            "'contacto_nombre',coalesce(k.nombre,''),'contacto_cargo',coalesce(k.cargo,''),'contacto_telefono',coalesce(k.telefono,''),",
            "'correo',coalesce(u.email,''),'tipo_convocatoria',e.tipo_convocatoria,'area_responsable',coalesce(e.area_responsable,''),",
            "'sitio_web',coalesce(e.sitio_web,''),'estado_convenio',e.estado_convenio,'vence_convenio',",
            day!("e.fecha_vencimiento_convenio"),
            ",",
            "'puede_participar',CASE WHEN cardinality(m.motivos)>0 THEN 'No' ELSE 'Si' END,'pendientes_documentales',array_to_string(m.motivos,'; '),",
            "'bloqueada',CASE WHEN e.participacion_bloqueada THEN 'Si' ELSE 'No' END,'motivo_bloqueo',coalesce(e.motivo_bloqueo,''),",
            "'observaciones',coalesce(e.observaciones_admin,''),'fecha_creacion',",
            day!("e.fecha_creacion"),
            ",",
            "'fecha_actualizacion',",
            d!("e.fecha_ultima_actualizacion"),
            ") fila FROM empresas e LEFT JOIN usuarios u ON u.id=e.usuario_id ",
            "LEFT JOIN LATERAL (SELECT nombre,cargo,telefono FROM contactos_empresa WHERE empresa_id=e.id ORDER BY es_principal DESC,id LIMIT 1) k ON TRUE ",
            // motivos_para_no_operar: block, restrictive lists, then each required catalog document not up to date.
            "CROSS JOIN LATERAL (SELECT array_remove(ARRAY[",
            "CASE WHEN e.participacion_bloqueada THEN trim('Participación bloqueada por el administrador. '||coalesce(e.motivo_bloqueo,'')) END,",
            "CASE WHEN e.estado_listas_restrictivas='RECHAZADO' THEN 'La empresa aparece en listas restrictivas.' END],NULL)||",
            "coalesce((SELECT array_agg(t.nombre||': '||CASE WHEN dc.id IS NULL OR dc.estado='PENDIENTE' THEN 'Falta cargarlo.' ",
            "WHEN dc.estado='RECHAZADO' THEN 'Rechazado: '||coalesce(nullif(dc.motivo_rechazo,''),'sin motivo')||'. Debe cargarse de nuevo.' ",
            "WHEN dc.estado='VENCIDO' OR dc.fecha_vencimiento<(now() AT TIME ZONE 'America/Bogota')::date THEN 'Vencido: debe actualizarse.' ",
            "WHEN dc.estado='CARGADO' THEN 'En revisión por el administrador.' ELSE 'pendiente de verificación.' END ORDER BY t.orden,t.nombre) ",
            "FROM tipos_documento_legal t LEFT JOIN LATERAL (SELECT * FROM documentos_empresas x WHERE x.empresa_id=e.id AND x.tipo_documento=t.codigo ORDER BY x.id DESC LIMIT 1) dc ON TRUE ",
            "WHERE t.activo AND t.obligatorio AND (t.aplica_sin_convenio OR NOT e.renuncio_a_convenio) ",
            "AND NOT (dc.estado='VERIFICADO' AND (dc.fecha_vencimiento IS NULL OR dc.fecha_vencimiento>=(now() AT TIME ZONE 'America/Bogota')::date)) IS TRUE),'{}') motivos) m WHERE TRUE {where}"
        ),
        order: "e.razon_social, e.id",
        filters: &[("estado_empresa", "e.estado_validacion=$1")],
        date_column: None,
    },
];
fn report(key: &str) -> Option<&'static Report> {
    REPORTS.iter().find(|r| r.key == key)
}
fn allowed(user: &User, r: &Report) -> bool {
    user.admin() || r.roles.contains(&user.rol.as_str())
}

#[derive(Deserialize, Default, Clone)]
pub struct Request {
    reporte: String,
    #[serde(default)]
    columnas: Vec<String>,
    #[serde(default = "default_format")]
    formato: String,
    #[serde(default)]
    periodo: String,
    desde: Option<NaiveDate>,
    hasta: Option<NaiveDate>,
    facultad: Option<i64>,
    programa: Option<i64>,
    /// Empresa id; reports filter by its user, as ReporteForm.filtros does.
    empresa: Option<i64>,
    #[serde(default)]
    estado: String,
    #[serde(default)]
    estado_postulacion: String,
    #[serde(default)]
    estado_empresa: String,
    #[serde(default)]
    destinatarios: String,
}
fn default_format() -> String {
    "PDF".into()
}

struct Built {
    report: &'static Report,
    columns: Vec<Col>,
    rows: Vec<Vec<String>>,
    total: usize,
    filters: Vec<(&'static str, String)>,
    desde: Option<NaiveDate>,
    hasta: Option<NaiveDate>,
}

async fn build(
    state: &AppState,
    user: &User,
    input: &Request,
    limit: Option<i64>,
) -> Result<Built> {
    let report = report(&input.reporte)
        .ok_or_else(|| AppError::bad("El reporte seleccionado no existe."))?;
    if !allowed(user, report) {
        return Err(AppError::forbidden());
    }
    // ReporteForm.clean: the period sets the range; "custom" uses desde/hasta (swapped if reversed).
    let today: NaiveDate = sqlx::query_scalar("SELECT (now() AT TIME ZONE 'America/Bogota')::date")
        .fetch_one(&state.pool)
        .await?;
    let (desde, hasta) = match input.periodo.as_str() {
        "custom" => match (input.desde, input.hasta) {
            (Some(a), Some(b)) if a > b => (Some(b), Some(a)),
            other => other,
        },
        "todo" => (None, None),
        p => {
            let days = match p {
                "30d" => 30,
                "anio" => 365,
                _ => 90,
            };
            (Some(today - Duration::days(days)), Some(today))
        }
    };
    let empresa_user: Option<i64> = match input.empresa {
        Some(id) => sqlx::query_scalar("SELECT usuario_id FROM empresas WHERE id=$1")
            .bind(id)
            .fetch_optional(&state.pool)
            .await?
            .flatten(),
        None => None,
    };
    let text = |v: &str| Some(v.to_string()).filter(|v| !v.is_empty());
    let values: Vec<(&'static str, Option<String>)> = vec![
        ("estado", text(&input.estado)),
        ("estado_postulacion", text(&input.estado_postulacion)),
        ("estado_empresa", text(&input.estado_empresa)),
        ("facultad", input.facultad.map(|v| v.to_string())),
        ("programa", input.programa.map(|v| v.to_string())),
        ("empresa", empresa_user.map(|v| v.to_string())),
    ];
    let mut clauses = String::new();
    let mut binds: Vec<String> = vec![];
    for (key, predicate) in report.filters {
        if let Some((_, Some(value))) = values.iter().find(|(k, _)| k == key) {
            binds.push(value.clone());
            clauses.push_str(&format!(
                " AND {}",
                predicate.replace("$1", &format!("${}", binds.len()))
            ));
        }
    }
    if let Some(column) = report.date_column {
        // acotar(): range on the local (Bogota) date.
        for (bound, op) in [(desde, ">="), (hasta, "<=")] {
            if let Some(day) = bound {
                binds.push(day.to_string());
                clauses.push_str(&format!(
                    " AND ({column} AT TIME ZONE 'America/Bogota')::date{op}${}::date",
                    binds.len()
                ));
            }
        }
    }
    let base = report.sql.replace("{where}", &clauses);
    let count_sql = format!("SELECT count(*) FROM ({base}) x");
    let mut count = sqlx::query_scalar::<_, i64>(&count_sql);
    for b in &binds {
        count = count.bind(b);
    }
    let total = count.fetch_one(&state.pool).await? as usize;
    let mut sql = format!("{base} ORDER BY {}", report.order);
    if let Some(l) = limit {
        sql.push_str(&format!(" LIMIT {l}"));
    }
    let mut query = sqlx::query_scalar::<_, Value>(&sql);
    for b in &binds {
        query = query.bind(b);
    }
    let raw = query.fetch_all(&state.pool).await?;
    // No columns chosen means all of them (columnas_por_clave).
    let columns: Vec<Col> = report
        .columns
        .iter()
        .filter(|c| input.columnas.is_empty() || input.columnas.iter().any(|k| k == c.0))
        .copied()
        .collect();
    let rows = raw
        .iter()
        .map(|row| {
            columns
                .iter()
                .map(|(key, _, _, choice)| {
                    let value = match &row[*key] {
                        Value::String(s) => s.clone(),
                        Value::Null => String::new(),
                        v => v.to_string(),
                    };
                    match choice {
                        Some((table, field)) if !value.is_empty() => {
                            crate::dashboard::etiqueta(table, field, &value)
                        }
                        _ => value,
                    }
                })
                .collect()
        })
        .collect();
    let mut filters = vec![];
    if let Some(day) = desde {
        filters.push(("desde", day.to_string()));
    }
    if let Some(day) = hasta {
        filters.push(("hasta", day.to_string()));
    }
    for (key, value) in values {
        if let Some(v) = value {
            filters.push((key, v));
        }
    }
    Ok(Built {
        report,
        columns,
        rows,
        total,
        filters,
        desde,
        hasta,
    })
}

/// _describir_filtros: readable filters for the report header.
fn describe(built: &Built) -> String {
    let labels = [
        ("desde", "Desde"),
        ("hasta", "Hasta"),
        ("estado", "Estado"),
        ("estado_postulacion", "Estado de postulacion"),
        ("estado_empresa", "Estado de empresa"),
        ("facultad", "Facultad"),
        ("programa", "Programa"),
        ("empresa", "Empresa"),
    ];
    let parts: Vec<String> = labels
        .iter()
        .filter_map(|(key, label)| {
            let value = match *key {
                "desde" => built.desde.map(|d| d.format("%d/%m/%Y").to_string()),
                "hasta" => built.hasta.map(|d| d.format("%d/%m/%Y").to_string()),
                _ => built
                    .filters
                    .iter()
                    .find(|(k, _)| k == key)
                    .map(|(_, v)| v.clone()),
            };
            value.map(|v| format!("{label}: {v}"))
        })
        .collect();
    if parts.is_empty() {
        "Sin filtros adicionales".into()
    } else {
        parts.join(" | ")
    }
}

fn bogota_now() -> chrono::DateTime<chrono::FixedOffset> {
    chrono::Utc::now().with_timezone(&chrono::FixedOffset::west_opt(5 * 3600).expect("offset"))
}

/// GET: reports this user may generate, with columns and filter options.
pub async fn catalog(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, false).await?;
    let reports: Vec<Value> = REPORTS
        .iter()
        .filter(|r| allowed(&user, r))
        .map(|r| {
            json!({
                "clave": r.key, "nombre": r.name, "descripcion": r.description,
                "columnas": r.columns.iter().map(|c| json!([c.0, c.1])).collect::<Vec<_>>(),
                "filtros": r.filters.iter().map(|f| f.0).collect::<Vec<_>>(),
            })
        })
        .collect();
    if reports.is_empty() {
        return Err(AppError::forbidden());
    }
    let options: Value = sqlx::query_scalar("SELECT jsonb_build_object('facultades',(SELECT coalesce(jsonb_agg(jsonb_build_array(id,nombre) ORDER BY nombre),'[]') FROM facultades),'programas',(SELECT coalesce(jsonb_agg(jsonb_build_array(p.id,p.nombre||' - '||f.nombre) ORDER BY p.nombre),'[]') FROM programas p JOIN facultades f ON f.id=p.facultad_id),'empresas',(SELECT coalesce(jsonb_agg(jsonb_build_array(id,razon_social) ORDER BY razon_social),'[]') FROM empresas))")
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(json!({"reportes": reports, "opciones": options})))
}

/// POST: first 50 rows and the total, with no file and no history entry.
pub async fn preview(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<Request>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    let built = build(&state, &user, &input, Some(50)).await?;
    Ok(Json(json!({
        "nombre": built.report.name,
        "encabezados": built.columns.iter().map(|c| c.1).collect::<Vec<_>>(),
        "filas": built.rows, "total": built.total, "mostradas": built.rows.len(), "filtros": describe(&built),
    })))
}

fn render(built: &Built, format: &str) -> Result<(Vec<u8>, &'static str, &'static str)> {
    let generated = bogota_now().format("%d/%m/%Y %H:%M").to_string();
    if format == "EXCEL" {
        let fail = |e: rust_xlsxwriter::XlsxError| {
            tracing::error!(%e, "report failed");
            AppError(
                StatusCode::INTERNAL_SERVER_ERROR,
                "No se pudo generar el archivo Excel".into(),
            )
        };
        let mut book = rust_xlsxwriter::Workbook::new();
        let brand = rust_xlsxwriter::Format::new()
            .set_bold()
            .set_font_color("3BAC52");
        let title = rust_xlsxwriter::Format::new().set_bold().set_font_size(14);
        let head = rust_xlsxwriter::Format::new()
            .set_bold()
            .set_font_color("FFFFFF")
            .set_background_color("3BAC52")
            .set_text_wrap();
        let name: String = built.report.name.chars().take(31).collect();
        let sheet = book.add_worksheet().set_name(&name).map_err(fail)?;
        sheet
            .write_string_with_format(0, 0, "retosean · Universidad EAN", &brand)
            .map_err(fail)?;
        sheet
            .write_string_with_format(1, 0, built.report.name, &title)
            .map_err(fail)?;
        sheet
            .write_string(2, 0, format!("Generado el {generated}"))
            .map_err(fail)?;
        sheet.write_string(3, 0, describe(built)).map_err(fail)?;
        for (c, col) in built.columns.iter().enumerate() {
            sheet
                .write_string_with_format(5, c as u16, col.1, &head)
                .map_err(fail)?;
            sheet
                .set_column_width(c as u16, col.2 as f64)
                .map_err(fail)?;
        }
        for (r, row) in built.rows.iter().enumerate() {
            for (c, value) in row.iter().enumerate() {
                // Strings only: identifiers, decimals and formula-looking text stay literal.
                sheet
                    .write_string(r as u32 + 6, c as u16, value)
                    .map_err(fail)?;
            }
        }
        sheet.set_freeze_panes(6, 0).map_err(fail)?;
        sheet
            .write_string(
                built.rows.len() as u32 + 7,
                0,
                format!("Total de registros: {}", built.rows.len()),
            )
            .map_err(fail)?;
        Ok((
            book.save_to_buffer().map_err(fail)?,
            "xlsx",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ))
    } else {
        // Limited to 2000 rows, as Django's MAX_FILAS_PDF.
        let mut lines = vec![
            built.report.description.to_string(),
            format!("Generado el {generated}"),
            describe(built),
            String::new(),
        ];
        for row in built.rows.iter().take(2000) {
            for ((_, label, _, _), value) in built.columns.iter().zip(row) {
                let line = format!("{label}: {value}");
                let chars: Vec<char> = line.chars().collect();
                lines.extend(chars.chunks(95).map(|c| c.iter().collect::<String>()));
            }
            lines.push("-".repeat(60));
        }
        let shown = built.rows.len().min(2000);
        let truncated = if built.rows.len() > 2000 {
            " (truncado a 2000; exporta a Excel para el listado completo)"
        } else {
            ""
        };
        lines.push(format!("Total de registros: {shown}{truncated}"));
        Ok((
            pdf_lines(built.report.name, &lines),
            "pdf",
            "application/pdf",
        ))
    }
}

/// Generates the file and logs it in reportes_generados (construir_reporte).
async fn generate(
    state: &AppState,
    user: &User,
    input: &Request,
) -> Result<(i64, Built, Vec<u8>, String, &'static str)> {
    let format = if input.formato == "EXCEL" {
        "EXCEL"
    } else {
        "PDF"
    };
    let built = build(state, user, input, None).await?;
    let (bytes, extension, mime) = render(&built, format)?;
    let filename = format!(
        "{}-{}.{extension}",
        built.report.key,
        bogota_now().format("%Y%m%d-%H%M%S")
    );
    let key = format!("{}.{extension}", uuid::Uuid::new_v4().simple());
    let save_error = |_| AppError::bad("No se pudo guardar el reporte");
    tokio::fs::create_dir_all(&state.config.media_root)
        .await
        .map_err(save_error)?;
    tokio::fs::write(state.config.media_root.join(&key), &bytes)
        .await
        .map_err(save_error)?;
    let mut filtros = serde_json::Map::new();
    filtros.insert("reporte".into(), json!(built.report.key));
    filtros.insert(
        "columnas".into(),
        json!(built.columns.iter().map(|c| c.0).collect::<Vec<_>>()),
    );
    filtros.insert("total_registros".into(), json!(built.rows.len()));
    for (k, v) in &built.filters {
        filtros.insert((*k).into(), json!(v));
    }
    let id: i64 = sqlx::query_scalar("INSERT INTO reportes_generados(usuario_id,formato,filtros,archivo,enviado_por_email,creado_en) VALUES($1,$2,$3,$4,false,now()) RETURNING id")
        .bind(user.id)
        .bind(format)
        .bind(Value::Object(filtros))
        .bind(&key)
        .fetch_one(&state.pool)
        .await?;
    Ok((id, built, bytes, filename, mime))
}

/// POST: generate and download.
pub async fn download(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<Request>,
) -> Result<Response> {
    let user = auth::current(&state, &headers, true).await?;
    let (_, _, bytes, filename, mime) = generate(&state, &user, &input).await?;
    Ok((
        [
            (header::CONTENT_TYPE, mime.to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        bytes,
    )
        .into_response())
}

/// POST: generate, log and email as an attachment (enviar_por_correo).
pub async fn send(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<Request>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    let mut recipients: Vec<String> = input
        .destinatarios
        .replace(';', ",")
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if recipients.len() > 10 {
        return Err(AppError::bad("Maximo 10 destinatarios."));
    }
    if let Some(bad) = recipients
        .iter()
        .find(|r| r.len() > 254 || r.split('@').filter(|p| !p.is_empty()).count() != 2)
    {
        return Err(AppError::bad(format!("Correo invalido: {bad}")));
    }
    if recipients.is_empty() {
        if user.email.is_empty() {
            return Err(AppError::bad(
                "Indica a que correo enviarlo: tu cuenta no tiene correo registrado.",
            ));
        }
        recipients.push(user.email.clone());
    }
    let (id, built, bytes, filename, mime) = generate(&state, &user, &input).await?;
    let author = format!("{} {}", user.first_name, user.last_name)
        .trim()
        .to_string();
    let author = if author.is_empty() {
        user.username.clone()
    } else {
        author
    };
    let text = format!(
        "{author} te envia el reporte \"{}\" generado en RetosEAN.\n\nRegistros: {}.\nEl archivo va adjunto.",
        built.report.name,
        built.rows.len()
    );
    let subject = format!("[RetosEAN] {}", built.report.name);
    match mail::global()
        .send_attachment(&recipients, &subject, &text, &filename, bytes, mime)
        .await
    {
        Ok(()) => {
            sqlx::query("UPDATE reportes_generados SET enviado_por_email=true,filtros=filtros||jsonb_build_object('enviado_a',$2::jsonb) WHERE id=$1")
                .bind(id)
                .bind(json!(recipients))
                .execute(&state.pool)
                .await?;
            Ok(Json(
                json!({"enviado": true, "destinatarios": recipients, "id": id}),
            ))
        }
        Err(error) => {
            // The generated report stays in the history, marked as failed.
            let error: String = error.chars().take(300).collect();
            sqlx::query("UPDATE reportes_generados SET filtros=filtros||jsonb_build_object('error_envio',$2::text) WHERE id=$1")
                .bind(id)
                .bind(&error)
                .execute(&state.pool)
                .await?;
            Err(AppError::bad(format!(
                "El reporte se genero pero no se pudo enviar: {error}"
            )))
        }
    }
}
