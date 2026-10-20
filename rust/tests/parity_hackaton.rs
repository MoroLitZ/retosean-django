//! apps/hackaton/tests.py: stages, registration, voting, ranking, publication,
//! the HU09 calendar, logistics and final report, and experts as jurors.
mod common;
use calamine::{Data, Reader};
use common::*;
use serde_json::{Value, json};

/// BaseHackathonTests.setUp: a running hackathon reto with an integrated professor
/// and a published hackathon (no stages).
struct Base {
    reto: i64,
    hack: i64,
}
async fn base(t: &Ctx) -> Base {
    let teacher = t.user("profe-hack", "PROFESOR").await;
    let reto = t.reto(COMPANY_USER, "en_curso", "hackathon").await;
    let integration = t.integrate(reto, teacher).await;
    t.exec(&format!(
        "UPDATE integraciones_academicas SET estado='aprobada' WHERE id={integration}"
    ))
    .await;
    let hack = t.i64(&format!("INSERT INTO hackathons(reto_id,reglas,estado,resultados_publicados,modalidad,lugar,enlace_transmision,notas_logistica,creado_en,actualizado_en) VALUES({reto},'','publicado',false,'presencial','','','',now(),now()) RETURNING id")).await;
    Base { reto, hack }
}
impl Base {
    async fn team(&self, t: &Ctx, name: &str, students: &[i64]) -> i64 {
        let team: i64 = sqlx::query_scalar(
            "INSERT INTO equipos(reto_id,nombre,creado_en) VALUES($1,$2,now()) RETURNING id",
        )
        .bind(self.reto)
        .bind(name)
        .fetch_one(&t.pool)
        .await
        .unwrap();
        for student in students {
            t.exec(&format!("INSERT INTO miembros_equipo(equipo_id,estudiante_id,rol,fecha_ingreso) VALUES({team},{student},'MIEMBRO',now())")).await;
        }
        team
    }
    async fn student(&self, t: &Ctx, username: &str) -> i64 {
        let id = t.user(username, "ESTUDIANTE").await;
        t.accept(self.reto, id).await;
        id
    }
    async fn juror(&self, t: &Ctx, username: &str) -> i64 {
        let user = t.user(username, "PROFESOR").await;
        t.i64(&format!("INSERT INTO jurados_hackaton(hackathon_id,usuario_id,nombre,email,especialidad) VALUES({},{user},'{username}','','') RETURNING id", self.hack)).await
    }
    async fn register(&self, t: &Ctx, team: i64, estado: &str) -> i64 {
        t.i64(&format!("INSERT INTO inscripciones_hackaton(hackathon_id,equipo_id,estado,creado_en) VALUES({},{team},'{estado}',now()) RETURNING id", self.hack)).await
    }
    async fn vote(&self, t: &Ctx, juror: i64, team: i64, score: &str) {
        t.exec(&format!("INSERT INTO votaciones_hackaton(hackathon_id,jurado_id,equipo_id,puntaje,comentario,fecha) VALUES({},{juror},{team},{score},'',now())", self.hack)).await;
    }
    async fn stage(&self, t: &Ctx, kind: &str, starts: &str, ends: &str) {
        t.exec(&format!("INSERT INTO etapas_hackaton(hackathon_id,tipo,titulo,inicia_en,termina_en,orden) VALUES({},'{kind}','{kind}',{starts},{ends},1)", self.hack)).await;
    }
    async fn estado(&self, t: &Ctx) -> (String, bool) {
        sqlx::query_as(&format!(
            "SELECT estado,resultados_publicados FROM hackathons WHERE id={}",
            self.hack
        ))
        .fetch_one(&t.pool)
        .await
        .unwrap()
    }
}
async fn ranking(t: &Ctx, session: &Session, hack: i64) -> (u16, Value) {
    let (code, body) = t
        .req(
            session,
            "GET",
            &format!("/api/hackathons/{hack}/ranking"),
            None,
        )
        .await;
    (code.as_u16(), body)
}

// EtapasTests
async fn fresh(t: &Ctx) -> i64 {
    let reto = t.reto(COMPANY_USER, "aprobado", "hackathon").await;
    t.ok(
        &t.admin,
        "POST",
        &format!("/api/retos/{reto}/hackathon"),
        Some(json!({"reglas":""})),
    )
    .await["id"]
        .as_i64()
        .unwrap()
}
parity!(test_crea_las_cuatro_etapas_del_backlog, |t| {
    let hack = fresh(&t).await;
    let kinds = t
        .texts(&format!(
            "SELECT tipo FROM etapas_hackaton WHERE hackathon_id={hack} ORDER BY tipo"
        ))
        .await;
    assert_eq!(
        kinds,
        ["desarrollo", "inscripcion", "premiacion", "presentacion"]
    );
});
parity!(test_no_duplica_etapas_si_ya_existen, |t| {
    let hack = fresh(&t).await;
    let reto = t
        .i64(&format!("SELECT reto_id FROM hackathons WHERE id={hack}"))
        .await;
    assert_ne!(
        t.status(
            &t.admin,
            "POST",
            &format!("/api/retos/{reto}/hackathon"),
            Some(json!({"reglas":""}))
        )
        .await,
        200
    );
    assert_eq!(
        t.i64(&format!(
            "SELECT count(*) FROM etapas_hackaton WHERE hackathon_id={hack}"
        ))
        .await,
        4
    );
});
parity!(test_etapa_fuera_de_fecha_no_esta_vigente, |t| {
    let b = base(&t).await;
    b.stage(
        &t,
        "inscripcion",
        "now()-interval '2 days'",
        "now()-interval '1 day'",
    )
    .await;
    let student = b.student(&t, "est-tarde").await;
    let team = b.team(&t, "Equipo tarde", &[student]).await;
    let session = t.login("est-tarde").await;
    assert_eq!(
        t.status(
            &session,
            "POST",
            &format!("/api/hackathons/{}/inscribir", b.hack),
            Some(json!({"equipo_id":team}))
        )
        .await,
        400
    );
});
parity!(test_etapa_en_curso_abre_las_inscripciones, |t| {
    let b = base(&t).await;
    b.stage(
        &t,
        "inscripcion",
        "now()-interval '1 day'",
        "now()+interval '5 days'",
    )
    .await;
    let student = b.student(&t, "est-atiempo").await;
    let team = b.team(&t, "Equipo a tiempo", &[student]).await;
    let session = t.login("est-atiempo").await;
    t.ok(
        &session,
        "POST",
        &format!("/api/hackathons/{}/inscribir", b.hack),
        Some(json!({"equipo_id":team})),
    )
    .await;
});

// InscripcionTests
parity!(test_estudiante_inscribe_su_equipo, |t| {
    let b = base(&t).await;
    let student = b.student(&t, "est-insc").await;
    let team = b.team(&t, "Equipo Alfa", &[student]).await;
    let session = t.login("est-insc").await;
    t.ok(
        &session,
        "POST",
        &format!("/api/hackathons/{}/inscribir", b.hack),
        Some(json!({"equipo_id":team})),
    )
    .await;
    assert_eq!(
        t.text(&format!(
            "SELECT estado FROM inscripciones_hackaton WHERE equipo_id={team}"
        ))
        .await,
        "PENDIENTE"
    );
});
parity!(test_no_se_puede_inscribir_dos_veces_el_mismo_equipo, |t| {
    let b = base(&t).await;
    let student = b.student(&t, "est-doble").await;
    let team = b.team(&t, "Equipo Beta", &[student]).await;
    b.register(&t, team, "PENDIENTE").await;
    let session = t.login("est-doble").await;
    let options = t
        .ok(
            &session,
            "GET",
            &format!("/api/hackathons/{}/opciones", b.hack),
            None,
        )
        .await;
    assert!(
        options["teams"].as_array().unwrap().is_empty(),
        "the registered team is no longer offered"
    );
    assert_eq!(
        t.status(
            &session,
            "POST",
            &format!("/api/hackathons/{}/inscribir", b.hack),
            Some(json!({"equipo_id":team}))
        )
        .await,
        400
    );
    assert_eq!(
        t.i64(&format!(
            "SELECT count(*) FROM inscripciones_hackaton WHERE equipo_id={team}"
        ))
        .await,
        1
    );
});
parity!(test_gestor_acepta_la_inscripcion, |t| {
    let b = base(&t).await;
    let team = b.team(&t, "Equipo Gamma", &[]).await;
    let registration = b.register(&t, team, "PENDIENTE").await;
    t.ok(
        &t.admin,
        "POST",
        &format!("/api/inscripciones/{registration}/estado"),
        Some(json!({"estado":"ACEPTADA"})),
    )
    .await;
    assert_eq!(
        t.text(&format!(
            "SELECT estado FROM inscripciones_hackaton WHERE id={registration}"
        ))
        .await,
        "ACEPTADA"
    );
});

// VotacionTests
async fn voting(t: &Ctx, b: &Base, team_state: &str) -> (i64, Session) {
    let team = b.team(t, "Equipo Voto", &[]).await;
    b.register(t, team, team_state).await;
    b.juror(t, "jurado-uno").await;
    (team, t.login("jurado-uno").await)
}
parity!(test_voto_fuera_de_rango_es_invalido, |t| {
    let b = base(&t).await;
    let (team, juror) = voting(&t, &b, "ACEPTADA").await;
    assert_eq!(
        t.status(
            &juror,
            "POST",
            &format!("/api/hackathons/{}/votar", b.hack),
            Some(json!({"equipo_id":team,"puntaje":"7.00"}))
        )
        .await,
        400
    );
    assert_eq!(t.i64("SELECT count(*) FROM votaciones_hackaton").await, 0);
});
parity!(
    test_no_se_puede_votar_un_equipo_sin_inscripcion_aceptada,
    |t| {
        let b = base(&t).await;
        let (team, juror) = voting(&t, &b, "PENDIENTE").await;
        assert_eq!(
            t.status(
                &juror,
                "POST",
                &format!("/api/hackathons/{}/votar", b.hack),
                Some(json!({"equipo_id":team,"puntaje":"4.00"}))
            )
            .await,
            400
        );
    }
);
parity!(test_jurado_registra_y_actualiza_su_voto, |t| {
    let b = base(&t).await;
    let (team, juror) = voting(&t, &b, "ACEPTADA").await;
    let url = format!("/api/hackathons/{}/votar", b.hack);
    t.ok(
        &juror,
        "POST",
        &url,
        Some(json!({"equipo_id":team,"puntaje":"4.00","comentario":"Bien"})),
    )
    .await;
    assert_eq!(t.i64("SELECT count(*) FROM votaciones_hackaton").await, 1);
    t.ok(
        &juror,
        "POST",
        &url,
        Some(json!({"equipo_id":team,"puntaje":"4.75","comentario":"Mejor"})),
    )
    .await;
    assert_eq!(t.i64("SELECT count(*) FROM votaciones_hackaton").await, 1);
    assert_eq!(
        t.text("SELECT puntaje::text FROM votaciones_hackaton")
            .await,
        "4.75"
    );
});
parity!(test_quien_no_es_jurado_no_puede_votar, |t| {
    let b = base(&t).await;
    let team = b.team(&t, "Equipo X", &[]).await;
    b.register(&t, team, "ACEPTADA").await;
    let teacher = t.login("profe-hack").await;
    assert_eq!(
        t.status(
            &teacher,
            "POST",
            &format!("/api/hackathons/{}/votar", b.hack),
            Some(json!({"equipo_id":team,"puntaje":"4"}))
        )
        .await,
        403
    );
});

// RankingTests
async fn ranked(t: &Ctx) -> (Base, i64, i64, i64, i64) {
    let b = base(t).await;
    let a = b.team(t, "Equipo A", &[]).await;
    let bb = b.team(t, "Equipo B", &[]).await;
    b.register(t, a, "ACEPTADA").await;
    b.register(t, bb, "ACEPTADA").await;
    let j1 = b.juror(t, "j1").await;
    let j2 = b.juror(t, "j2").await;
    (b, a, bb, j1, j2)
}
parity!(test_ordena_por_promedio_no_por_suma, |t| {
    let (b, a, bb, j1, j2) = ranked(&t).await;
    b.vote(&t, j1, a, "5.00").await;
    b.vote(&t, j1, bb, "4.00").await;
    b.vote(&t, j2, bb, "4.00").await;
    let (_, body) = ranking(&t, &t.admin, b.hack).await;
    let rows = body["ranking"].as_array().unwrap();
    assert_eq!(
        (rows[0]["equipo"].as_str(), rows[0]["posicion"].as_i64()),
        (Some("Equipo A"), Some(1))
    );
    assert_eq!(rows[1]["equipo"], "Equipo B");
    let total = |r: &Value| {
        r["total"]
            .as_str()
            .map(|s| s.parse::<f64>().unwrap())
            .or(r["total"].as_f64())
            .unwrap()
    };
    assert!(total(&rows[1]) > total(&rows[0]));
});
parity!(test_desempate_por_numero_de_votos, |t| {
    let (b, a, bb, j1, j2) = ranked(&t).await;
    b.vote(&t, j1, a, "4.00").await;
    b.vote(&t, j1, bb, "4.00").await;
    b.vote(&t, j2, bb, "4.00").await;
    let (_, body) = ranking(&t, &t.admin, b.hack).await;
    assert_eq!(body["ranking"][0]["equipo"], "Equipo B");
});
parity!(test_equipos_sin_votar_del_jurado, |t| {
    let (b, a, bb, j1, _) = ranked(&t).await;
    b.vote(&t, j1, a, "4.00").await;
    let juror = t.login("j1").await;
    let options = t
        .ok(
            &juror,
            "GET",
            &format!("/api/hackathons/{}/opciones", b.hack),
            None,
        )
        .await;
    let pending: Vec<i64> = options["sin_votar"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["value"].as_i64().unwrap())
        .collect();
    assert_eq!(pending, [bb]);
});
parity!(test_resultados_ocultos_hasta_publicarlos, |t| {
    let (b, ..) = ranked(&t).await;
    b.student(&t, "est-ranking").await;
    let session = t.login("est-ranking").await;
    assert_eq!(ranking(&t, &session, b.hack).await.0, 403);
});
parity!(test_publicar_resultados_los_hace_visibles_y_notifica, |t| {
    let (b, a, _, j1, _) = ranked(&t).await;
    let student = b.student(&t, "est-ganador").await;
    t.exec(&format!("INSERT INTO miembros_equipo(equipo_id,estudiante_id,rol,fecha_ingreso) VALUES({a},{student},'MIEMBRO',now())")).await;
    b.vote(&t, j1, a, "5.00").await;
    t.exec(&format!(
        "UPDATE hackathons SET estado='en_curso' WHERE id={}",
        b.hack
    ))
    .await;
    t.ok(
        &t.admin,
        "POST",
        &format!("/api/hackathons/{}/estado", b.hack),
        Some(json!({"estado":"finalizado"})),
    )
    .await;
    assert!(b.estado(&t).await.1);
    assert_eq!(t.notices(student, "HACKATON_RESULTADOS").await, 1);
    let session = t.login("est-ganador").await;
    assert_eq!(ranking(&t, &session, b.hack).await.0, 200);
});

// PublicacionTests
parity!(test_publicar_notifica_a_los_estudiantes_aceptados, |t| {
    let b = base(&t).await;
    let student = b.student(&t, "est-aviso").await;
    t.exec(&format!(
        "UPDATE hackathons SET estado='borrador' WHERE id={}",
        b.hack
    ))
    .await;
    t.ok(
        &t.admin,
        "POST",
        &format!("/api/hackathons/{}/estado", b.hack),
        Some(json!({"estado":"publicado"})),
    )
    .await;
    assert_eq!(b.estado(&t).await.0, "publicado");
    assert_eq!(t.notices(student, "HACKATON_PUBLICADO").await, 1);
});
parity!(test_profesor_ajeno_no_gestiona_el_hackathon, |t| {
    let b = base(&t).await;
    t.user("profe-ajeno", "PROFESOR").await;
    let other = t.login("profe-ajeno").await;
    assert_eq!(
        t.status(
            &other,
            "POST",
            &format!("/api/hackathons/{}/estado", b.hack),
            Some(json!({"estado":"en_curso"}))
        )
        .await,
        403
    );
    assert_eq!(b.estado(&t).await.0, "publicado");
});

// CalendarioAutomaticoHU09Tests
async fn advance(t: &Ctx, hack: i64) -> Vec<&'static str> {
    retosean::event::advance(&t.pool, hack).await.unwrap()
}
parity!(test_pasa_a_en_curso_al_iniciar_el_desarrollo, |t| {
    let b = base(&t).await;
    b.stage(&t, "desarrollo", "now()-interval '1 hour'", "NULL")
        .await;
    b.stage(&t, "premiacion", "now()+interval '5 days'", "NULL")
        .await;
    assert_eq!(advance(&t, b.hack).await, ["en_curso"]);
    assert_eq!(b.estado(&t).await, ("en_curso".to_string(), false));
});
parity!(
    test_al_cierre_publica_resultados_y_ganador_y_deja_historial,
    |t| {
        let b = base(&t).await;
        b.stage(&t, "desarrollo", "now()-interval '3 days'", "NULL")
            .await;
        b.stage(&t, "premiacion", "now()-interval '5 minutes'", "NULL")
            .await;
        let student = b.student(&t, "est-cierre").await;
        let team = b.team(&t, "Equipo Cierre", &[student]).await;
        b.register(&t, team, "ACEPTADA").await;
        let juror = b.juror(&t, "jur-cierre").await;
        b.vote(&t, juror, team, "4.5").await;
        advance(&t, b.hack).await;
        assert_eq!(b.estado(&t).await, ("finalizado".to_string(), true));
        let notice = t.text(&format!("SELECT mensaje FROM notificaciones WHERE usuario_id={student} AND evento='HACKATON_RESULTADOS'")).await;
        assert!(notice.contains("Equipo Cierre"), "{notice}");
        assert!(
            t.history("hackaton", "hackathon", b.hack, "estado")
                .await
                .iter()
                .any(|s| s.1 == "finalizado")
        );
        assert!(advance(&t, b.hack).await.is_empty(), "idempotent");
    }
);
parity!(test_sin_fechas_no_cierra_solo, |t| {
    let b = base(&t).await;
    b.stage(&t, "premiacion", "NULL", "NULL").await;
    assert!(advance(&t, b.hack).await.is_empty());
});
parity!(
    test_sin_premiacion_cierra_al_terminar_la_ultima_etapa,
    |t| {
        let b = base(&t).await;
        b.stage(
            &t,
            "presentacion",
            "now()-interval '2 days'",
            "now()-interval '1 hour'",
        )
        .await;
        advance(&t, b.hack).await;
        assert!(b.estado(&t).await.1);
    }
);
parity!(test_el_comando_programado_avanza_los_hackathones, |t| {
    let b = base(&t).await;
    b.stage(&t, "premiacion", "now()-interval '1 minute'", "NULL")
        .await;
    // The demo has no other dated hackathons, so exactly this one changes.
    assert_eq!(retosean::event::advance_all(&t.pool).await.unwrap(), 1);
    assert!(b.estado(&t).await.1);
});
parity!(test_abrir_el_detalle_aplica_el_calendario, |t| {
    let b = base(&t).await;
    b.stage(&t, "premiacion", "now()-interval '1 minute'", "NULL")
        .await;
    t.ok(
        &t.admin,
        "GET",
        &format!("/api/records/hackathons/{}", b.hack),
        None,
    )
    .await;
    assert!(b.estado(&t).await.1);
});

// LogisticaEInformeHU09Tests
parity!(test_gestor_registra_logistica_e_invitados, |t| {
    let b = base(&t).await;
    t.ok(&t.admin, "POST", &format!("/api/hackathons/{}/logistica", b.hack), Some(json!({"modalidad":"hibrida","lugar":"Auditorio EAN","enlace_transmision":"https://youtube.com/live/x","notas_logistica":"Refrigerios a las 10","invitados":[{"tipo":"MENTOR","nombre":"Ana Mentora","organizacion":"ACME","email":"ana@acme.co","confirmado":true}]}))).await;
    assert_eq!(
        t.text(&format!("SELECT lugar FROM hackathons WHERE id={}", b.hack))
            .await,
        "Auditorio EAN"
    );
    assert!(
        t.bool(&format!(
            "SELECT confirmado FROM invitados_hackaton WHERE hackathon_id={}",
            b.hack
        ))
        .await
    );
    assert!(
        t.ok(
            &t.admin,
            "GET",
            &format!("/api/hackathons/{}/logistica", b.hack),
            None
        )
        .await
        .to_string()
        .contains("Ana Mentora")
    );
});
parity!(test_evento_virtual_exige_enlace_de_transmision, |t| {
    let b = base(&t).await;
    let (code, body) = t
        .req(
            &t.admin,
            "POST",
            &format!("/api/hackathons/{}/logistica", b.hack),
            Some(json!({"modalidad":"virtual"})),
        )
        .await;
    assert_eq!(code, 400);
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("enlace de transmision")
    );
});
parity!(test_estudiante_no_gestiona_la_logistica, |t| {
    let b = base(&t).await;
    b.student(&t, "est-log").await;
    let session = t.login("est-log").await;
    assert_eq!(
        t.status(
            &session,
            "POST",
            &format!("/api/hackathons/{}/logistica", b.hack),
            Some(json!({"modalidad":"presencial","lugar":"x"}))
        )
        .await,
        403
    );
});
parity!(test_informe_final_en_pdf_y_excel_con_el_ranking, |t| {
    let b = base(&t).await;
    let student = b.student(&t, "est-inf").await;
    let team = b.team(&t, "Equipo =SUMA(A1)", &[student]).await;
    b.register(&t, team, "ACEPTADA").await;
    let juror = b.juror(&t, "jur-inf").await;
    b.vote(&t, juror, team, "5").await;
    t.exec(&format!(
        "UPDATE hackathons SET resultados_publicados=true,estado='finalizado' WHERE id={}",
        b.hack
    ))
    .await;
    // The company that owns the reto may download it too.
    let (code, pdf) = download(
        &t.app,
        &t.company,
        &format!("/api/hackathons/{}/informe/pdf", b.hack),
    )
    .await;
    assert_eq!(code, 200);
    assert!(pdf.starts_with(b"%PDF"));
    let (code, xlsx) = download(
        &t.app,
        &t.company,
        &format!("/api/hackathons/{}/informe/xlsx", b.hack),
    )
    .await;
    assert_eq!(code, 200);
    let mut book: calamine::Xlsx<_> =
        calamine::open_workbook_from_rs(std::io::Cursor::new(xlsx)).unwrap();
    assert!(book.sheet_names().contains(&"Ranking".to_string()));
    let values: Vec<Data> = book
        .worksheet_range("Ranking")
        .unwrap()
        .cells()
        .map(|(_, _, v)| v.clone())
        .collect();
    assert!(values.contains(&Data::String("Equipo =SUMA(A1)".into())));
    assert!(
        book.worksheet_formula("Ranking")
            .unwrap()
            .cells()
            .all(|(_, _, f)| f.is_empty()),
        "text, not a formula"
    );
});
parity!(test_estudiante_no_descarga_el_informe, |t| {
    let b = base(&t).await;
    b.student(&t, "est-noinf").await;
    let session = t.login("est-noinf").await;
    assert!(
        !download(
            &t.app,
            &session,
            &format!("/api/hackathons/{}/informe/pdf", b.hack)
        )
        .await
        .0
        .is_success()
    );
});

// ExpertoJuradoTests
async fn expert(t: &Ctx) -> (Base, i64, Session) {
    let b = base(t).await;
    let expert = t.user("exp-jur", "EXPERTO").await;
    t.exec(&format!("INSERT INTO jurados_hackaton(hackathon_id,usuario_id,nombre,email,especialidad) VALUES({},{expert},'Experto','','')", b.hack)).await;
    let other_reto = t.reto(COMPANY_USER, "en_curso", "hackathon").await;
    let other = t.i64(&format!("INSERT INTO hackathons(reto_id,reglas,estado,resultados_publicados,modalidad,lugar,enlace_transmision,notas_logistica,creado_en,actualizado_en) VALUES({other_reto},'','publicado',false,'presencial','','','',now(),now()) RETURNING id")).await;
    (b, other, t.login("exp-jur").await)
}
parity!(test_lista_solo_sus_hackathones, |t| {
    let (b, _, session) = expert(&t).await;
    let list = t.ok(&session, "GET", "/api/records/hackathons", None).await;
    let ids: Vec<i64> = list["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_i64().unwrap())
        .collect();
    assert_eq!(ids, [b.hack]);
});
parity!(test_ve_el_detalle_del_suyo_pero_no_el_ajeno, |t| {
    let (b, other, session) = expert(&t).await;
    t.ok(
        &session,
        "GET",
        &format!("/api/records/hackathons/{}", b.hack),
        None,
    )
    .await;
    assert_eq!(
        t.status(
            &session,
            "GET",
            &format!("/api/records/hackathons/{other}"),
            None
        )
        .await,
        404
    );
});
parity!(test_vota_como_jurado, |t| {
    let (b, _, session) = expert(&t).await;
    let student = b.student(&t, "est-exp").await;
    let team = b.team(&t, "Equipo Experto", &[student]).await;
    b.register(&t, team, "ACEPTADA").await;
    t.ok(
        &session,
        "POST",
        &format!("/api/hackathons/{}/votar", b.hack),
        Some(json!({"equipo_id":team,"puntaje":"4","comentario":"Bien"})),
    )
    .await;
    assert_eq!(t.i64(&format!("SELECT count(*) FROM votaciones_hackaton v JOIN jurados_hackaton j ON j.id=v.jurado_id JOIN usuarios u ON u.id=j.usuario_id WHERE u.username='exp-jur' AND v.equipo_id={team}")).await, 1);
});
