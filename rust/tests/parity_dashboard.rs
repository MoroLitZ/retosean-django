//! apps/dashboard/tests.py: route walk for every role, admin filters, public
//! routes and the expert panel.
mod common;
use common::*;

/// GET routes declared in src/lib.rs, so new routes are walked automatically.
fn get_routes() -> Vec<String> {
    let source = include_str!("../src/lib.rs");
    let mut routes = Vec::new();
    for chunk in source.split(".route(").skip(1) {
        let Some(path) = chunk
            .trim_start()
            .strip_prefix('"')
            .and_then(|r| r.split('"').next())
        else {
            continue;
        };
        let handler = &chunk[..chunk.find(".route(").unwrap_or(chunk.len()).min(400)];
        if handler.contains("get(") {
            routes.push(path.to_string());
        }
    }
    routes
}

/// Extra rows so parameterised routes have real objects: a hackathon with a juror,
/// stage, team registration and vote, a deliverable, a closure and an academic team.
async fn enrich(t: &Ctx) {
    let reto = t.reto(COMPANY_USER, "en_curso", "hackathon").await;
    t.integrate(reto, TEACHER).await;
    t.accept(reto, STUDENT).await;
    let hack = t.i64(&format!("INSERT INTO hackathons(reto_id,reglas,estado,resultados_publicados,modalidad,lugar,enlace_transmision,notas_logistica,creado_en,actualizado_en) VALUES({reto},'','en_curso',false,'presencial','','','',now(),now()) RETURNING id")).await;
    let juror = t.i64(&format!("INSERT INTO jurados_hackaton(hackathon_id,usuario_id,nombre,email,especialidad) VALUES({hack},{EXPERT},'Experto','e@x.co','') RETURNING id")).await;
    t.exec(&format!("INSERT INTO etapas_hackaton(hackathon_id,tipo,titulo,inicia_en,termina_en,orden) VALUES({hack},'DESARROLLO','Desarrollo',now()-interval '1 day',now()+interval '1 day',1)")).await;
    let team = t.i64(&format!("INSERT INTO equipos(reto_id,nombre,creado_en) VALUES({reto},'Equipo',now()) RETURNING id")).await;
    t.exec(&format!("INSERT INTO miembros_equipo(equipo_id,estudiante_id,rol,fecha_ingreso) VALUES({team},{STUDENT},'LIDER',now())")).await;
    t.exec(&format!("INSERT INTO inscripciones_hackaton(hackathon_id,equipo_id,estado,inscrito_por_id,creado_en) VALUES({hack},{team},'ACEPTADA',{STUDENT},now())")).await;
    t.exec(&format!("INSERT INTO votaciones_hackaton(hackathon_id,jurado_id,equipo_id,puntaje,comentario,fecha) VALUES({hack},{juror},{team},4,'',now())")).await;
    let academic = t.i64(&format!("INSERT INTO equipos_reto_academico(reto_id,nombre_equipo,equipo_espejo_id) VALUES({reto},'Equipo',{team}) RETURNING id")).await;
    t.exec(&format!("INSERT INTO equipos_reto_academico_estudiantes(equiporetoacademico_id,usuario_id) VALUES({academic},{STUDENT})")).await;
    t.deliverable(reto, STUDENT).await;
    t.exec(&format!("INSERT INTO cierres_reto(reto_id,notas,impacto,encuesta_enviada,cerrado_por_id,actualizado_en) VALUES({reto},'','SI',false,{ADMIN},now())")).await;
    t.exec("INSERT INTO unidades_estudio(codigo,nombre,programa_id,periodo,ciclo,activo,creado_en,actualizado_en) VALUES('U-1','Unidad',1,'2026-2','1',true,now(),now())").await;
}

/// Concrete URLs for a route: every reto, two rows for everything else.
async fn urls(t: &Ctx, route: &str) -> Vec<String> {
    let ids =
        |table: &str, limit: &str| format!("SELECT id::text FROM \"{table}\" ORDER BY id {limit}");
    let mut out = vec![route.to_string()];
    if route.contains("{resource}") {
        out = Vec::new();
        for model in retosean::resources::schema() {
            let table = model["table"].as_str().unwrap();
            let base = route.replace("{resource}", table);
            if base.contains("{id}") {
                for id in t.texts(&ids(table, "LIMIT 2")).await {
                    let with_id = base.replace("{id}", &id);
                    if with_id.contains("{field}") {
                        for field in model["fields"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .filter(|f| f["type"] == "FileField")
                        {
                            out.push(with_id.replace("{field}", field["name"].as_str().unwrap()));
                        }
                    } else {
                        out.push(with_id);
                    }
                }
            } else if base.contains("{format}") {
                out.push(base.replace("{format}", "pdf"));
                out.push(base.replace("{format}", "xlsx"));
            } else {
                out.push(base);
            }
        }
        return out;
    }
    if route.contains("{code}") {
        return t
            .texts("SELECT codigo_verificacion::text FROM certificados LIMIT 2")
            .await
            .into_iter()
            .map(|c| route.replace("{code}", &c))
            .collect();
    }
    if route.contains("{id}") {
        let table = [
            ("/api/retos/", "retos"),
            ("/api/hackathons/", "hackathons"),
            ("/api/integraciones/", "integraciones_academicas"),
            ("/api/equipos-academicos/", "equipos_reto_academico"),
            ("/api/entregables/", "entregables"),
            ("/api/presupuestos/", "presupuestos"),
            ("/api/usuarios/", "usuarios"),
            ("/api/certificados/", "certificados"),
            ("/api/empresas/", "empresas"),
        ]
        .iter()
        .find(|(prefix, _)| route.starts_with(prefix))
        .unwrap_or_else(|| panic!("map the {{id}} of {route} to a table"))
        .1;
        let limit = if table == "retos" { "" } else { "LIMIT 2" };
        out = t
            .texts(&ids(table, limit))
            .await
            .into_iter()
            .map(|id| route.replace("{id}", &id))
            .collect();
    }
    if route.contains("{format}") {
        out = out
            .iter()
            .flat_map(|u| [u.replace("{format}", "pdf"), u.replace("{format}", "xlsx")])
            .collect();
    }
    assert!(
        out.iter().all(|u| !u.contains('{')),
        "unfilled parameter in {route}: {out:?}"
    );
    out
}

// RecorridoDeRutasTests
parity!(test_ninguna_ruta_responde_500_para_ningun_rol, |t| {
    enrich(&t).await;
    let mut all = Vec::new();
    for route in get_routes() {
        all.extend(urls(&t, &route).await);
    }
    assert!(all.len() > 100, "{} urls", all.len());
    let mut failures = Vec::new();
    for (name, session) in [
        ("admin", &t.admin),
        ("empresa", &t.company),
        ("profesor", &t.teacher),
        ("estudiante", &t.student),
        ("experto", &t.expert),
    ] {
        for url in &all {
            let (code, _) = download(&t.app, session, url).await;
            if code.is_server_error() {
                failures.push(format!("{name:12} {url} -> {code}"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "Rutas con error:\n{}",
        failures.join("\n")
    );
});
parity!(
    test_dashboard_admin_filtra_por_ecosistema_y_muestra_impacto,
    |t| {
        let ecosystem = t.i64("INSERT INTO ecosistemas(nombre,descripcion) VALUES('Ecosistema de prueba HU05','') RETURNING id").await;
        let reto = t.reto(COMPANY_USER, "finalizado", "reto").await;
        t.exec(&format!("UPDATE retos SET ecosistema_id={ecosystem} WHERE id={reto}; INSERT INTO cierres_reto(reto_id,notas,impacto,encuesta_enviada,actualizado_en) VALUES({reto},'','SI',false,now())")).await;
        let panel = t
            .ok(
                &t.admin,
                "GET",
                &format!("/api/dashboard?periodo=todo&ecosistema={ecosystem}"),
                None,
            )
            .await;
        assert_eq!(panel["kpis"]["retos_total"], 1, "{panel}");
        assert_eq!(panel["kpis"]["impacto_si"], 1, "{panel}");
        let data: Vec<f64> = panel["series"]["impacto"]["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                v.as_f64()
                    .unwrap_or_else(|| v.as_str().unwrap().parse().unwrap())
            })
            .collect();
        assert_eq!(data, [1.0]);
    }
);
parity!(test_rutas_publicas_sin_sesion, |t| {
    let code = t
        .text("SELECT codigo_verificacion::text FROM certificados LIMIT 1")
        .await;
    for url in [
        "/",
        "/app.js",
        "/style.css",
        "/health",
        "/api/registro/opciones",
        &format!("/api/verificar/{code}"),
    ] {
        let (status, _) = download(&t.app, &t.anon, url).await;
        assert!(status.as_u16() < 400, "{url} -> {status}");
    }
});

// PanelExpertoTests
parity!(test_el_experto_tiene_su_panel_de_jurado, |t| {
    let expert = t.user("exp-panel", "EXPERTO").await;
    let (owner, _) = t.company_user("emp-panel", "900111333").await;
    let reto = t.reto(owner, "en_curso", "hackathon").await;
    let hack = t.i64(&format!("INSERT INTO hackathons(reto_id,reglas,estado,resultados_publicados,modalidad,lugar,enlace_transmision,notas_logistica,creado_en,actualizado_en) VALUES({reto},'','en_curso',false,'presencial','','','',now(),now()) RETURNING id")).await;
    t.exec(&format!("INSERT INTO jurados_hackaton(hackathon_id,usuario_id,nombre,email,especialidad) VALUES({hack},{expert},'Exp','','')")).await;
    let session = t.login("exp-panel").await;
    let panel = t.ok(&session, "GET", "/api/dashboard", None).await;
    assert_eq!(panel["sin_rol"], false, "{panel}");
    assert_eq!(panel["kpis"]["hackathones_asignados"], 1, "{panel}");
    assert_eq!(panel["kpis"]["hackathones_en_curso"], 1, "{panel}");
});
