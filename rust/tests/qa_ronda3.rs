//! Round 3 independent-auditor regression suite. Each fix in the `.work/r3`
//! package gets a test here that fails without the fix.
mod common;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use common::*;
use serde_json::json;
use std::time::Duration;
use tower::ServiceExt;

/// A hackathon reto (en_curso) with an integrated, approved professor and a
/// hackathon in `estado`, mirroring `qa_hackathon::setup`. Returns a session for
/// the professor (the reto's re-use may be soft-deleted after creation).
struct Hack {
    reto: i64,
    hack: i64,
    session: Session,
}
async fn hackaton(t: &Ctx, estado: &str) -> Hack {
    let teacher = t.user("profe-r3", "PROFESOR").await;
    let reto = t.reto(COMPANY_USER, "en_curso", "hackathon").await;
    let integration = t.integrate(reto, teacher).await;
    t.exec(&format!(
        "UPDATE integraciones_academicas SET estado='aprobada' WHERE id={integration}"
    ))
    .await;
    let hack = t.i64(&format!("INSERT INTO hackathons(reto_id,reglas,estado,resultados_publicados,modalidad,lugar,enlace_transmision,notas_logistica,creado_en,actualizado_en) VALUES({reto},'','{estado}',false,'presencial','','','',now(),now()) RETURNING id")).await;
    Hack {
        reto,
        hack,
        session: t.login("profe-r3").await,
    }
}

// d-hackathon #1: a professor with an approved integration must not manage the
// logistics of (or download the report for) a hackathon whose reto is soft-deleted.
parity!(test_profesor_no_gestiona_hackathon_de_reto_eliminado, |t| {
    let h = hackaton(&t, "publicado").await;
    t.exec(&format!(
        "UPDATE retos SET eliminado_en=now() WHERE id={}",
        h.reto
    ))
    .await;
    let logistics = t
        .status(
            &h.session,
            "POST",
            &format!("/api/hackathons/{}/logistica", h.hack),
            Some(json!({
                "modalidad":"virtual",
                "enlace_transmision":"https://x.example/stream",
                "notas_logistica":"cambio",
                "invitados":[]
            })),
        )
        .await;
    assert_eq!(
        logistics,
        StatusCode::FORBIDDEN,
        "soft-deleted reto logistics"
    );
    let report = t
        .status(
            &h.session,
            "GET",
            &format!("/api/hackathons/{}/informe/pdf", h.hack),
            None,
        )
        .await;
    assert_eq!(report, StatusCode::FORBIDDEN, "soft-deleted reto report");
});

// d-hackathon #2: the owning professor cannot replace the team/schedule of an
// integration whose reto was soft-deleted.
parity!(test_profesor_no_edita_equipo_de_reto_eliminado, |t| {
    let teacher = t.user("profe-equipo-r3", "PROFESOR").await;
    let reto = t.reto(COMPANY_USER, "abierto", "reto").await;
    let integration = t.integrate(reto, teacher).await;
    t.exec(&format!(
        "UPDATE integraciones_academicas SET estado='borrador' WHERE id={integration}"
    ))
    .await;
    let session = t.login("profe-equipo-r3").await;
    t.exec(&format!(
        "UPDATE retos SET eliminado_en=now() WHERE id={reto}"
    ))
    .await;
    let payload = json!({
        "participantes":[{"rol":"PROFESOR","nombre":"Ana","documento":"1","email":"ana@ean.edu.co","facultad":"Ingeniería"}],
        "sesiones":[{"tipo":"inicio","fecha_hora":"2030-01-01T10:00:00Z"}],
    });
    let status = t
        .status(
            &session,
            "POST",
            &format!("/api/integraciones/{integration}/equipo"),
            Some(payload),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "soft-deleted reto team edit");
    assert_eq!(
        t.i64(&format!(
            "SELECT count(*) FROM participantes_integracion WHERE integracion_id={integration}"
        ))
        .await,
        0
    );
});

// d-hackathon #4: editing an academic team is blocked once the hackathon finished
// (mirrors the existing delete guard).
parity!(
    test_no_edita_equipo_academico_con_hackathon_finalizado,
    |t| {
        let h = hackaton(&t, "publicado").await;
        let student = t.user("est-edit-r3", "ESTUDIANTE").await;
        t.accept(h.reto, student).await;
        let creado = t
            .ok(
                &h.session,
                "POST",
                &format!("/api/retos/{}/equipo", h.reto),
                Some(json!({"nombre_equipo":"Equipo A","estudiantes":[student]})),
            )
            .await;
        let academic = creado["equipo_academico"].as_i64().unwrap();
        t.exec(&format!(
            "UPDATE hackathons SET estado='finalizado',resultados_publicados=true WHERE id={}",
            h.hack
        ))
        .await;
        let (status, body) = t
            .req(
                &h.session,
                "POST",
                &format!("/api/equipos-academicos/{academic}"),
                Some(json!({"nombre_equipo":"Cambiado","estudiantes":[student]})),
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert!(
            body["error"].as_str().unwrap().contains("ya finalizó"),
            "{body}"
        );
        assert_eq!(
            t.text(&format!(
                "SELECT nombre_equipo FROM equipos_reto_academico WHERE id={academic}"
            ))
            .await,
            "Equipo A"
        );
    }
);

// d-hackathon #5: accepting a registration after a dated inscription stage ended is
// rejected; without a dated stage the window stays open (existing parity covers it).
parity!(test_gestor_no_acepta_inscripcion_fuera_de_la_etapa, |t| {
    let h = hackaton(&t, "publicado").await;
    let team = t
        .i64(&format!(
            "INSERT INTO equipos(reto_id,nombre,creado_en) VALUES({},'Equipo',now()) RETURNING id",
            h.reto
        ))
        .await;
    let registration = t.i64(&format!("INSERT INTO inscripciones_hackaton(hackathon_id,equipo_id,estado,inscrito_por_id,creado_en) VALUES({},{team},'PENDIENTE',{ADMIN},now()) RETURNING id", h.hack)).await;
    t.exec(&format!(
        "INSERT INTO etapas_hackaton(hackathon_id,tipo,titulo,inicia_en,termina_en,orden) VALUES({},'inscripcion','Inscripción',now()-interval '2 days',now()-interval '1 day',1)",
        h.hack
    ))
    .await;
    let status = t
        .status(
            &t.admin,
            "POST",
            &format!("/api/inscripciones/{registration}/estado"),
            Some(json!({"estado":"ACEPTADA"})),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "late acceptance rejected");
    assert_eq!(
        t.text(&format!(
            "SELECT estado FROM inscripciones_hackaton WHERE id={registration}"
        ))
        .await,
        "PENDIENTE"
    );
});

// c-workflows #2: accept/reject decisions on a cancelled or finalized reto are frozen.
parity!(test_postulaciones_congeladas_en_reto_cancelado, |t| {
    let reto = t.reto(COMPANY_USER, "abierto", "reto").await;
    let student = t.user("est-post-r3", "ESTUDIANTE").await;
    let application = t.i64(&format!("INSERT INTO postulaciones(reto_id,estudiante_id,estado,motivacion,semestre,programa,fecha_postulacion,habilidades,acepta_terminos) VALUES({reto},{student},'PENDIENTE','',5,'',now(),'',true) RETURNING id")).await;
    t.exec(&format!(
        "UPDATE retos SET estado='cancelado' WHERE id={reto}"
    ))
    .await;
    let (status, body) = t
        .req(
            &t.company,
            "POST",
            &format!("/api/postulaciones/{application}/estado"),
            Some(json!({"estado":"ACEPTADA"})),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(
        body["error"].as_str().unwrap().contains("congeladas"),
        "{body}"
    );
    assert_eq!(
        t.text(&format!(
            "SELECT estado FROM postulaciones WHERE id={application}"
        ))
        .await,
        "PENDIENTE"
    );
});

// c-workflows #3a: a company comment only notifies professors with an approved or
// published integration.
parity!(test_comentario_no_notifica_a_profesor_no_aprobado, |t| {
    let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
    let student = t.user("est-com-r3", "ESTUDIANTE").await;
    t.accept(reto, student).await;
    let deliverable = t.deliverable(reto, student).await;
    let rejected = t.user("profe-rej-r3", "PROFESOR").await;
    let rejected_int = t.integrate(reto, rejected).await;
    t.exec(&format!(
        "UPDATE integraciones_academicas SET estado='rechazada' WHERE id={rejected_int}"
    ))
    .await;
    let approved = t.user("profe-apr-r3", "PROFESOR").await;
    t.integrate(reto, approved).await;
    t.ok(
        &t.company,
        "POST",
        &format!("/api/entregables/{deliverable}/comentario"),
        Some(json!({"comentario":"Comentario confidencial de la empresa"})),
    )
    .await;
    assert_eq!(
        t.notices(rejected, "COMENTARIO_EMPRESA").await,
        0,
        "rejected professor must not be notified"
    );
    assert_eq!(
        t.notices(approved, "COMENTARIO_EMPRESA").await,
        1,
        "approved professor notified once"
    );
});

// c-workflows #3b: cancelling a reto only notifies approved/published professors.
parity!(test_cancelacion_no_notifica_a_profesor_no_aprobado, |t| {
    let reto = t.reto(COMPANY_USER, "abierto", "reto").await;
    let rejected = t.user("profe-rej-cancel-r3", "PROFESOR").await;
    let rejected_int = t.integrate(reto, rejected).await;
    t.exec(&format!(
        "UPDATE integraciones_academicas SET estado='rechazada' WHERE id={rejected_int}"
    ))
    .await;
    let approved = t.user("profe-apr-cancel-r3", "PROFESOR").await;
    t.integrate(reto, approved).await;
    t.ok(
        &t.admin,
        "POST",
        &format!("/api/retos/{reto}/estado"),
        Some(json!({"estado":"cancelado"})),
    )
    .await;
    assert_eq!(
        t.notices(rejected, "RETO_ESTADO_CAMBIADO").await,
        0,
        "rejected professor must not learn of cancellation"
    );
    assert_eq!(t.notices(approved, "RETO_ESTADO_CAMBIADO").await, 1);
});

// c-workflows #4: a cancellation that holds the reto row lock cannot be overtaken by
// an expense: the expense blocks on the reto lock and then rejects the movement.
parity!(test_cancelacion_no_atropella_gasto_en_vuelo, |t| {
    let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
    let budget = t.i64(&format!("INSERT INTO presupuestos(reto_id,monto_total,alerta_80_enviada,creado_en,actualizado_en) VALUES({reto},1000,false,now(),now()) RETURNING id")).await;

    // Hold the reto row lock with an uncommitted cancellation.
    let mut lock = t.pool.begin().await.unwrap();
    sqlx::query("UPDATE retos SET estado='cancelado',actualizado_en=now() WHERE id=$1")
        .bind(reto)
        .execute(&mut *lock)
        .await
        .unwrap();

    let app = t.app.clone();
    let session = t.admin.clone();
    let path = format!("/api/presupuestos/{budget}/gastos");
    let body = json!({"categoria":"MATERIALES","monto":"100","estado":"EJECUTADO"});
    let handle = tokio::spawn({
        let app = app.clone();
        let session = session.clone();
        let body = body.clone();
        async move {
            app.oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .header("cookie", &session.cookie)
                    .header("x-csrf-token", &session.csrf)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap()
            .status()
        }
    });

    // While the reto row is locked by the in-flight cancellation, the expense must
    // block on the reto lock instead of committing a gasto on the way out.
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(
        t.i64(&format!(
            "SELECT count(*) FROM gastos WHERE presupuesto_id={budget}"
        ))
        .await,
        0,
        "a gasto slipped in while cancellation held the reto lock"
    );

    lock.commit().await.unwrap();
    let status = handle.await.unwrap();
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "expense rejects after cancellation"
    );
    assert_eq!(
        t.i64(&format!(
            "SELECT count(*) FROM gastos WHERE presupuesto_id={budget}"
        ))
        .await,
        0
    );
});

// e-lifecycle #1: re-invoking close on an already-finalized reto is idempotent and
// never rewrites the frozen closure record.
parity!(test_reinvocar_cierre_no_reescribe_el_cierre, |t| {
    let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
    let student = t.user("est-cierre-r3", "ESTUDIANTE").await;
    t.accept(reto, student).await;
    let teacher = t.user("profe-cierre-r3", "PROFESOR").await;
    t.integrate(reto, teacher).await;
    t.exec(&format!("INSERT INTO cierre_agendacierre(reto_id,fecha_hora,espacio,recursos,invitados,agenda,actualizado_en) VALUES({reto},now(),'Auditorio','','','Cierre',now())")).await;
    let closure = t.i64(&format!("INSERT INTO cierres_reto(reto_id,acta_url,notas,impacto,encuesta_enviada,cerrado_por_id,actualizado_en) VALUES({reto},'acta.pdf','','',false,{ADMIN},now()) RETURNING id")).await;
    t.exec(&format!("INSERT INTO cierre_entregablefinal(cierre_id,nombre,archivo,cargado_por_id,creado_en) VALUES({closure},'Producto final','final.pdf',{ADMIN},now())")).await;

    t.ok(
        &t.admin,
        "POST",
        &format!("/api/retos/{reto}/cerrar"),
        Some(json!({"notas":"Cierre original del revisor"})),
    )
    .await;
    assert_eq!(
        t.text(&format!("SELECT estado FROM retos WHERE id={reto}"))
            .await,
        "finalizado"
    );

    // A second close by another authorized actor must not overwrite notas/cerrado_por_id.
    let teacher_session = t.login("profe-cierre-r3").await;
    let again = t
        .ok(
            &teacher_session,
            "POST",
            &format!("/api/retos/{reto}/cerrar"),
            Some(json!({"notas":"Intento de reescritura posterior"})),
        )
        .await;
    assert_eq!(again["certificados_emitidos"].as_i64(), Some(0));
    assert_eq!(
        t.text(&format!(
            "SELECT notas FROM cierres_reto WHERE reto_id={reto}"
        ))
        .await,
        "Cierre original del revisor"
    );
    assert_eq!(
        t.i64(&format!(
            "SELECT cerrado_por_id FROM cierres_reto WHERE reto_id={reto}"
        ))
        .await,
        ADMIN
    );
});

// b-data #2: replacing a deliverable must unlink the previous stored file.
parity!(test_reemplazar_entregable_elimina_archivo_anterior, |t| {
    let reto = t.reto(COMPANY_USER, "abierto", "reto").await;
    let student = t.user("est-archivo-r3", "ESTUDIANTE").await;
    t.accept(reto, student).await;
    let session = t.login("est-archivo-r3").await;
    let submit = |content: &'static [u8]| {
        let app = t.app.clone();
        let session = session.clone();
        let path = format!("/api/retos/{reto}/entregar");
        async move {
            multipart(
                &app,
                &session,
                &path,
                &[("titulo", "Trabajo")],
                &[("archivo", "entrega.pdf", content)],
            )
            .await
        }
    };
    assert_eq!(submit(b"%PDF-1.4 primera version").await.0, 200);
    let old_key = t
        .text(&format!(
            "SELECT archivo FROM entregables WHERE reto_id={reto} AND estudiante_id={student} AND titulo='Trabajo'"
        ))
        .await;
    assert_eq!(submit(b"%PDF-1.4 segunda version").await.0, 200);
    let new_key = t
        .text(&format!(
            "SELECT archivo FROM entregables WHERE reto_id={reto} AND estudiante_id={student} AND titulo='Trabajo'"
        ))
        .await;
    assert_ne!(old_key, new_key, "replacement generates a fresh key");
    assert!(!t.media.join(&old_key).is_file(), "old file must be gone");
    assert!(t.media.join(&new_key).is_file(), "new file must remain");
});

// b-data #3: a request that carries more than the allowed number of files is rejected.
parity!(test_carga_rechaza_exceso_de_archivos, |t| {
    let reto = t.reto(COMPANY_USER, "borrador", "reto").await;
    let mut files = Vec::new();
    for i in 0..11 {
        files.push((
            "archivos".to_string(),
            format!("soporte-{i}.pdf"),
            b"%PDF-1.4 soporte".to_vec(),
        ));
    }
    let file_refs: Vec<(&str, &str, &[u8])> = files
        .iter()
        .map(|(f, n, b)| (f.as_str(), n.as_str(), b.as_slice()))
        .collect();
    let (code, body) = multipart(
        &t.app,
        &t.company,
        &format!("/api/retos/{reto}/archivos"),
        &[],
        &file_refs,
    )
    .await;
    assert_eq!(code, 400, "{body}");
    assert!(
        body["error"].as_str().unwrap().contains("Máximo 10"),
        "{body}"
    );
    assert_eq!(t.i64("SELECT count(*) FROM retos_retoarchivo").await, 0);
});

// CSV/XLSX export "verify": the CSV cell writer neutralizes every spreadsheet
// formula trigger (`=`, `+`, `-`, `@`, even after leading whitespace); XLSX cells
// use write_string, so they are already read as literal strings, not formulas.
#[test]
fn csv_cell_neutralizes_formulas() {
    for dangerous in ["=1+1", "+1+1", "-1+1", "@cmd", "\t=1+1", "\r=1+1"] {
        let cell = retosean::domain::csv_cell(&serde_json::json!(dangerous));
        assert!(
            cell.starts_with("\"'"),
            "cell {cell:?} must be single-quote prefixed"
        );
    }
    assert_eq!(
        retosean::domain::csv_cell(&serde_json::json!("plain")),
        "\"plain\""
    );
}
