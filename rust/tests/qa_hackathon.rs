//! Regressions for package C (hackathons, events, teams): findings 7, 8, 11,
//! 13, 18, 36, 94.
mod common;
use common::*;
use serde_json::json;

/// A running hackathon reto with an integrated professor and a hackathon in the
/// requested state (no stages), mirroring `parity_hackaton::Base`.
struct B {
    reto: i64,
    hack: i64,
}
async fn setup(t: &Ctx, estado: &str) -> B {
    let teacher = t.user("profe-qa", "PROFESOR").await;
    let reto = t.reto(COMPANY_USER, "en_curso", "hackathon").await;
    let integration = t.integrate(reto, teacher).await;
    t.exec(&format!(
        "UPDATE integraciones_academicas SET estado='aprobada' WHERE id={integration}"
    ))
    .await;
    let hack = t.i64(&format!("INSERT INTO hackathons(reto_id,reglas,estado,resultados_publicados,modalidad,lugar,enlace_transmision,notas_logistica,creado_en,actualizado_en) VALUES({reto},'','{estado}',false,'presencial','','','',now(),now()) RETURNING id")).await;
    B { reto, hack }
}

// HCK-06 (finding 7): a dated inscription/development stage alone must never
// close the hackathon; only an explicit presentation end or award start does.
parity!(test_no_cierra_sin_presentacion_ni_premiacion, |t| {
    let b = setup(&t, "publicado").await;
    t.exec(&format!("INSERT INTO etapas_hackaton(hackathon_id,tipo,titulo,inicia_en,termina_en,orden) VALUES({},'inscripcion','Inscripción',now()-interval '2 days',now()-interval '1 day',1)", b.hack)).await;
    assert!(
        retosean::event::advance(&t.pool, b.hack)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        t.text(&format!(
            "SELECT estado FROM hackathons WHERE id={}",
            b.hack
        ))
        .await,
        "publicado"
    );
    assert!(
        !t.bool(&format!(
            "SELECT resultados_publicados FROM hackathons WHERE id={}",
            b.hack
        ))
        .await
    );
});

// HCK-08 (finding 8): deleting an academic team after results are published is
// rejected and must not erase votes or registrations.
parity!(test_no_elimina_equipo_con_resultados_publicados, |t| {
    let b = setup(&t, "en_curso").await;
    let student = t.user("est-del", "ESTUDIANTE").await;
    t.accept(b.reto, student).await;
    let session = t.login("profe-qa").await;
    let creado = t
        .ok(
            &session,
            "POST",
            &format!("/api/retos/{}/equipo", b.reto),
            Some(json!({"nombre_equipo":"Equipo A","estudiantes":[student]})),
        )
        .await;
    let academic = creado["equipo_academico"].as_i64().unwrap();
    let mirror = creado["id"].as_i64().unwrap();
    let juror = t.user("jur-del", "PROFESOR").await;
    t.exec(&format!("INSERT INTO jurados_hackaton(hackathon_id,usuario_id,nombre,email,especialidad) VALUES({},{juror},'J','','')", b.hack)).await;
    t.exec(&format!("INSERT INTO inscripciones_hackaton(hackathon_id,equipo_id,estado,inscrito_por_id,creado_en) VALUES({},{mirror},'ACEPTADA',{student},now())", b.hack)).await;
    t.exec(&format!("INSERT INTO votaciones_hackaton(hackathon_id,jurado_id,equipo_id,puntaje,comentario,fecha) VALUES({},(SELECT id FROM jurados_hackaton WHERE hackathon_id={} AND usuario_id={juror}),{mirror},4.5,'',now())", b.hack, b.hack)).await;
    t.exec(&format!(
        "UPDATE hackathons SET estado='finalizado',resultados_publicados=true WHERE id={}",
        b.hack
    ))
    .await;
    assert_eq!(
        t.status(
            &session,
            "POST",
            &format!("/api/equipos-academicos/{academic}/eliminar"),
            None
        )
        .await,
        400
    );
    assert!(
        t.bool(&format!(
            "SELECT EXISTS(SELECT 1 FROM equipos_reto_academico WHERE id={academic})"
        ))
        .await
    );
    assert!(
        t.bool(&format!(
            "SELECT EXISTS(SELECT 1 FROM equipos WHERE id={mirror})"
        ))
        .await
    );
    assert_eq!(
        t.i64(&format!(
            "SELECT count(*) FROM votaciones_hackaton WHERE equipo_id={mirror}"
        ))
        .await,
        1
    );
    assert_eq!(
        t.i64(&format!(
            "SELECT count(*) FROM inscripciones_hackaton WHERE equipo_id={mirror}"
        ))
        .await,
        1
    );
});

// HCK-01 (finding 11): a student may not end up in two operative teams of the
// same reto; sync_mirror skips the student already rostered elsewhere.
parity!(
    test_estudiante_no_queda_en_dos_equipos_del_mismo_reto,
    |t| {
        let b = setup(&t, "en_curso").await;
        let student = t.user("est-dup", "ESTUDIANTE").await;
        t.accept(b.reto, student).await;
        let session = t.login("profe-qa").await;
        for name in ["Equipo A", "Equipo B"] {
            t.ok(
                &session,
                "POST",
                &format!("/api/retos/{}/equipo", b.reto),
                Some(json!({"nombre_equipo":name,"estudiantes":[student]})),
            )
            .await;
        }
        assert_eq!(
        t.i64(&format!("SELECT count(*) FROM miembros_equipo m JOIN equipos e ON e.id=m.equipo_id WHERE e.reto_id={} AND m.estudiante_id={student}", b.reto)).await,
        1
    );
    }
);

// DB-5 (finding 13): the juror() handler already rejects a duplicate juror per
// hackathon (application guard; the DB partial unique index is package F).
parity!(test_rechaza_jurado_duplicado, |t| {
    let b = setup(&t, "publicado").await;
    let session = t.login("profe-qa").await;
    let expert = t.user("exp-qa", "EXPERTO").await;
    let url = format!("/api/hackathons/{}/jurado", b.hack);
    t.ok(
        &session,
        "POST",
        &url,
        Some(json!({"usuario_id":expert,"especialidad":"Rust"})),
    )
    .await;
    assert_eq!(
        t.status(
            &session,
            "POST",
            &url,
            Some(json!({"usuario_id":expert,"especialidad":"Rust"}))
        )
        .await,
        400
    );
    assert_eq!(
        t.i64(&format!(
            "SELECT count(*) FROM jurados_hackaton WHERE hackathon_id={} AND usuario_id={expert}",
            b.hack
        ))
        .await,
        1
    );
});

// HCK-03 (finding 18): votes are bounded to 0..5, negatives included.
parity!(test_voto_negativo_es_invalido, |t| {
    let b = setup(&t, "publicado").await;
    t.exec(&format!("INSERT INTO etapas_hackaton(hackathon_id,tipo,titulo,inicia_en,termina_en,orden) VALUES({},'presentacion','Presentación',now()-interval '1 day',now()+interval '1 day',1)", b.hack)).await;
    let team = t
        .i64(&format!(
            "INSERT INTO equipos(reto_id,nombre,creado_en) VALUES({},'Equipo',now()) RETURNING id",
            b.reto
        ))
        .await;
    t.exec(&format!("INSERT INTO inscripciones_hackaton(hackathon_id,equipo_id,estado,inscrito_por_id,creado_en) VALUES({},{team},'ACEPTADA',{ADMIN},now())", b.hack)).await;
    let juror = t.user("jurado-qa", "PROFESOR").await;
    t.exec(&format!("INSERT INTO jurados_hackaton(hackathon_id,usuario_id,nombre,email,especialidad) VALUES({},{juror},'J','','')", b.hack)).await;
    let session = t.login("jurado-qa").await;
    let (code, body) = t
        .req(
            &session,
            "POST",
            &format!("/api/hackathons/{}/votar", b.hack),
            Some(json!({"equipo_id":team,"puntaje":"-5.00"})),
        )
        .await;
    assert_eq!(code, 400, "{body}");
    assert!(
        body["error"].as_str().unwrap().contains("entre 0 y 5"),
        "{body}"
    );
    assert_eq!(t.i64("SELECT count(*) FROM votaciones_hackaton").await, 0);
});

// HCK-02 (finding 36): registration requires an accepted postulation.
parity!(test_inscripcion_exige_postulacion_aceptada, |t| {
    let b = setup(&t, "publicado").await;
    let student = t.user("est-sinposta", "ESTUDIANTE").await;
    t.exec(&format!("INSERT INTO postulaciones(reto_id,estudiante_id,estado,motivacion,semestre,programa,fecha_postulacion,habilidades,acepta_terminos) VALUES({},{student},'RECHAZADA','',5,'',now(),'',true)", b.reto)).await;
    let team = t.i64(&format!("INSERT INTO equipos(reto_id,nombre,creado_en) VALUES({},'Equipo X',now()) RETURNING id", b.reto)).await;
    t.exec(&format!("INSERT INTO miembros_equipo(equipo_id,estudiante_id,rol,fecha_ingreso) VALUES({team},{student},'MIEMBRO',now())")).await;
    let session = t.login("est-sinposta").await;
    let (code, body) = t
        .req(
            &session,
            "POST",
            &format!("/api/hackathons/{}/inscribir", b.hack),
            Some(json!({"equipo_id":team})),
        )
        .await;
    assert_eq!(code, 400, "{body}");
    assert_eq!(
        t.i64(&format!(
            "SELECT count(*) FROM inscripciones_hackaton WHERE hackathon_id={}",
            b.hack
        ))
        .await,
        0
    );
});

// HCK-09 (finding 94): logistics is rejected once the hackathon is closed.
parity!(test_logistica_rechazada_en_hackathon_cerrado, |t| {
    let b = setup(&t, "finalizado").await;
    let session = t.login("profe-qa").await;
    let (code, body) = t
        .req(
            &session,
            "POST",
            &format!("/api/hackathons/{}/logistica", b.hack),
            Some(json!({"modalidad":"presencial","lugar":"Auditorio"})),
        )
        .await;
    assert_eq!(code, 400, "{body}");
    assert!(
        body["error"].as_str().unwrap().contains("cerrado"),
        "{body}"
    );
    assert_eq!(
        t.i64(&format!(
            "SELECT count(*) FROM invitados_hackaton WHERE hackathon_id={}",
            b.hack
        ))
        .await,
        0
    );
});
