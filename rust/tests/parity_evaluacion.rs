//! apps/evaluacion/tests.py: grading, publication and re-grading (HU12).
mod common;
use common::*;
use serde_json::json;

/// EvaluacionSprintTests.setUp: reto en curso, linked professor, a team deliverable.
async fn setup(t: &Ctx, integration: &str) -> (i64, i64, i64) {
    let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
    let teacher = t.user("profe-eval", "PROFESOR").await;
    let student = t.user("est-eval", "ESTUDIANTE").await;
    let integ = t.integrate(reto, teacher).await;
    t.exec(&format!(
        "UPDATE integraciones_academicas SET estado='{integration}' WHERE id={integ}"
    ))
    .await;
    let team = t.i64(&format!("INSERT INTO equipos(reto_id,nombre,creado_en) VALUES({reto},'Equipo uno',now()) RETURNING id")).await;
    let deliverable = t.deliverable(reto, student).await;
    t.exec(&format!(
        "UPDATE entregables SET equipo_id={team} WHERE id={deliverable}"
    ))
    .await;
    (reto, deliverable, teacher)
}
async fn nota(t: &Ctx, deliverable: i64) -> String {
    t.text(&format!(
        "SELECT nota::text FROM entregables WHERE id={deliverable}"
    ))
    .await
}
async fn published(t: &Ctx, deliverable: i64) -> bool {
    t.bool(&format!(
        "SELECT calificacion_publicada FROM entregables WHERE id={deliverable}"
    ))
    .await
}

// EvaluacionSprintTests
parity!(test_profesor_vinculado_califica_y_genera_historial, |t| {
    let (reto, deliverable, teacher) = setup(&t, "aprobada").await;
    let session = t.login("profe-eval").await;
    t.ok(
        &session,
        "POST",
        &format!("/api/entregables/{deliverable}/calificar"),
        Some(json!({"nota":"4.25","comentario":"Buen avance"})),
    )
    .await;
    assert_eq!(nota(&t, deliverable).await, "4.25");
    assert_eq!(
        t.text(&format!(
            "SELECT estado FROM entregables WHERE id={deliverable}"
        ))
        .await,
        "CALIFICADO"
    );
    assert_eq!(t.i64(&format!("SELECT count(*) FROM evaluaciones WHERE entregable_id={deliverable} AND profesor_id={teacher}")).await, 1);
    assert!(!published(&t, deliverable).await);
    assert_eq!(
        t.i64(&format!(
            "SELECT count(*) FROM evaluacion_comentarioentregable WHERE entregable_id={deliverable}"
        ))
        .await,
        0
    );
    t.ok(
        &session,
        "POST",
        &format!("/api/retos/{reto}/publicar-calificaciones"),
        Some(json!({})),
    )
    .await;
    assert!(published(&t, deliverable).await);
    assert_eq!(t.i64(&format!("SELECT count(*) FROM evaluacion_comentarioentregable WHERE entregable_id={deliverable} AND autor_id={teacher}")).await, 1);
});
parity!(test_profesor_no_vinculado_no_puede_calificar, |t| {
    let (_, deliverable, _) = setup(&t, "aprobada").await;
    t.user("otro-profe", "PROFESOR").await;
    let other = t.login("otro-profe").await;
    let code = t
        .status(
            &other,
            "POST",
            &format!("/api/entregables/{deliverable}/calificar"),
            Some(json!({"nota":"5"})),
        )
        .await;
    assert!(code == 404 || code == 403, "{code}");
    assert!(
        t.bool(&format!(
            "SELECT nota IS NULL FROM entregables WHERE id={deliverable}"
        ))
        .await
    );
});
parity!(test_empresa_no_puede_acceder_a_calificacion, |t| {
    let (_, deliverable, _) = setup(&t, "aprobada").await;
    assert_eq!(
        t.status(
            &t.company,
            "POST",
            &format!("/api/entregables/{deliverable}/calificar"),
            Some(json!({"nota":"5"}))
        )
        .await,
        403
    );
});

// PublicacionYRecalificacionTests
async fn graded(t: &Ctx) -> (i64, i64, Session) {
    let (reto, deliverable, _) = setup(t, "publicada").await;
    let session = t.login("profe-eval").await;
    t.ok(
        &session,
        "POST",
        &format!("/api/entregables/{deliverable}/calificar"),
        Some(json!({"nota":"3","comentario":"Bien hecho"})),
    )
    .await;
    (reto, deliverable, session)
}
parity!(
    test_el_estudiante_no_ve_la_nota_ni_recibe_aviso_hasta_publicar,
    |t| {
        let (reto, deliverable, teacher) = graded(&t).await;
        let student = t
            .i64("SELECT id FROM usuarios WHERE username='est-eval'")
            .await;
        assert_eq!(t.notices(student, "ENTREGABLE_CALIFICADO").await, 0);
        let session = t.login("est-eval").await;
        let path = format!("/api/records/entregables/{deliverable}");
        assert!(
            !t.ok(&session, "GET", &path, None)
                .await
                .to_string()
                .contains("Bien hecho")
        );
        t.ok(
            &teacher,
            "POST",
            &format!("/api/retos/{reto}/publicar-calificaciones"),
            Some(json!({})),
        )
        .await;
        assert!(t.notices(student, "ENTREGABLE_CALIFICADO").await > 0);
        assert!(
            t.ok(&session, "GET", &path, None)
                .await
                .to_string()
                .contains("Bien hecho")
        );
    }
);
parity!(test_recalificar_exige_confirmacion, |t| {
    let (_, deliverable, teacher) = graded(&t).await;
    let url = format!("/api/entregables/{deliverable}/calificar");
    assert_ne!(
        t.status(
            &teacher,
            "POST",
            &url,
            Some(json!({"nota":"5","comentario":"Corregida"}))
        )
        .await,
        200
    );
    assert_eq!(nota(&t, deliverable).await, "3.00");
    t.ok(
        &teacher,
        "POST",
        &url,
        Some(json!({"nota":"5","comentario":"Corregida","confirmar_recalificacion":true})),
    )
    .await;
    assert_eq!(nota(&t, deliverable).await, "5.00");
    assert!(!published(&t, deliverable).await);
});
parity!(test_otro_profesor_no_publica, |t| {
    let (reto, deliverable, _) = graded(&t).await;
    t.user("otro-pub", "PROFESOR").await;
    let other = t.login("otro-pub").await;
    assert_eq!(
        t.status(
            &other,
            "POST",
            &format!("/api/retos/{reto}/publicar-calificaciones"),
            Some(json!({}))
        )
        .await,
        403
    );
    assert!(!published(&t, deliverable).await);
});
