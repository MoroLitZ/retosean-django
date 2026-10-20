//! apps/cierre/tests.py: formal close, surveys and logistics requests (HU08).
mod common;
use common::*;
use serde_json::json;

/// CierreSprintTests.setUp: reto en curso, an accepted student, agenda, acta and a final artifact.
async fn setup(t: &Ctx) -> (i64, i64) {
    let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
    let student = t.user("est-cierre", "ESTUDIANTE").await;
    t.accept(reto, student).await;
    t.exec(&format!("INSERT INTO cierre_agendacierre(reto_id,fecha_hora,espacio,recursos,invitados,agenda,actualizado_en) VALUES({reto},now(),'Auditorio','','','Presentacion y premiacion',now())")).await;
    let closure = t.i64(&format!("INSERT INTO cierres_reto(reto_id,acta_url,notas,impacto,encuesta_enviada,cerrado_por_id,actualizado_en) VALUES({reto},'acta.pdf','','',false,{ADMIN},now()) RETURNING id")).await;
    t.exec(&format!("INSERT INTO cierre_entregablefinal(cierre_id,nombre,archivo,cargado_por_id,creado_en) VALUES({closure},'Producto final','final.pdf',{ADMIN},now())")).await;
    (reto, student)
}
async fn surveys(t: &Ctx, reto: i64) -> i64 {
    t.i64(&format!("SELECT count(*) FROM cierre_encuestasatisfaccion e JOIN cierres_reto c ON c.id=e.cierre_id WHERE c.reto_id={reto}")).await
}
async fn estado(t: &Ctx, reto: i64) -> String {
    t.text(&format!("SELECT estado FROM retos WHERE id={reto}"))
        .await
}

parity!(
    test_finalizar_cambia_estado_notifica_y_genera_encuestas,
    |t| {
        let (reto, _) = setup(&t).await;
        t.ok(
            &t.admin,
            "POST",
            &format!("/api/retos/{reto}/cerrar"),
            Some(json!({})),
        )
        .await;
        assert_eq!(estado(&t, reto).await, "finalizado");
        assert_eq!(surveys(&t, reto).await, 2);
        assert_eq!(
            t.i64(
                "SELECT count(*) FROM notificaciones WHERE titulo='Reto finalizado: Reto de prueba'"
            )
            .await,
            2
        );
        assert!(
            t.history("retos", "reto", reto, "estado")
                .await
                .iter()
                .any(|s| s.1 == "finalizado")
        );
    }
);
parity!(test_empresa_no_puede_finalizar, |t| {
    let (reto, _) = setup(&t).await;
    assert_eq!(
        t.status(
            &t.company,
            "POST",
            &format!("/api/retos/{reto}/cerrar"),
            Some(json!({}))
        )
        .await,
        403
    );
    assert_eq!(estado(&t, reto).await, "en_curso");
});
parity!(test_participante_responde_encuesta, |t| {
    let (reto, student) = setup(&t).await;
    let survey = t.i64(&format!("INSERT INTO cierre_encuestasatisfaccion(cierre_id,participante_id,comentario,creada_en) SELECT id,{student},'',now() FROM cierres_reto WHERE reto_id={reto} RETURNING id")).await;
    let session = t.login("est-cierre").await;
    t.ok(
        &session,
        "POST",
        &format!("/api/encuestas/{survey}"),
        Some(json!({"calificacion":5,"comentario":"Excelente"})),
    )
    .await;
    assert!(
        t.bool(&format!(
            "SELECT respondida_en IS NOT NULL FROM cierre_encuestasatisfaccion WHERE id={survey}"
        ))
        .await
    );
});
parity!(test_no_se_finaliza_un_reto_que_no_esta_en_curso, |t| {
    let (reto, _) = setup(&t).await;
    t.exec(&format!(
        "UPDATE retos SET estado='abierto' WHERE id={reto}"
    ))
    .await;
    assert_ne!(
        t.status(
            &t.admin,
            "POST",
            &format!("/api/retos/{reto}/cerrar"),
            Some(json!({}))
        )
        .await,
        200
    );
    assert_eq!(estado(&t, reto).await, "abierto");
    assert_eq!(surveys(&t, reto).await, 0);
});
parity!(test_empresa_no_ve_el_boton_de_finalizar, |t| {
    let (reto, _) = setup(&t).await;
    let panel = t
        .ok(
            &t.company,
            "GET",
            &format!("/api/retos/{reto}/cierre"),
            None,
        )
        .await;
    assert_eq!(panel["puede_finalizar"], false);
});
parity!(
    test_empresa_solicita_logistica_y_el_admin_la_confirma,
    |t| {
        let (reto, _) = setup(&t).await;
        let id = t.ok(&t.company, "POST", &format!("/api/retos/{reto}/solicitudes-logistica"), Some(json!({"evento":"CIERRE","tipo":"ESPACIO","descripcion":"Auditorio para 80 personas"}))).await["id"].as_i64().unwrap();
        assert_eq!(
            t.text(&format!(
                "SELECT estado FROM cierre_solicitudlogistica WHERE id={id}"
            ))
            .await,
            "SOLICITADA"
        );
        assert!(t.notices(ADMIN, "LOGISTICA_SOLICITADA").await > 0);
        let url = format!("/api/solicitudes-logistica/{id}/estado");
        assert_eq!(
            t.status(
                &t.company,
                "POST",
                &url,
                Some(json!({"estado":"CONFIRMADA"}))
            )
            .await,
            403
        );
        t.ok(&t.admin, "POST", &url, Some(json!({"estado":"CONFIRMADA"})))
            .await;
        assert_eq!(
            t.text(&format!(
                "SELECT estado FROM cierre_solicitudlogistica WHERE id={id}"
            ))
            .await,
            "CONFIRMADA"
        );
        let steps: Vec<String> = t
            .history("cierre", "solicitudlogistica", id, "estado")
            .await
            .into_iter()
            .map(|s| s.1)
            .collect();
        assert_eq!(steps, ["SOLICITADA", "CONFIRMADA"]);
    }
);
parity!(test_estudiante_no_pide_logistica, |t| {
    let (reto, _) = setup(&t).await;
    let session = t.login("est-cierre").await;
    assert_eq!(
        t.status(
            &session,
            "POST",
            &format!("/api/retos/{reto}/solicitudes-logistica"),
            Some(json!({"evento":"INICIO","tipo":"OTRO","descripcion":"x"}))
        )
        .await,
        403
    );
    assert_eq!(
        t.i64("SELECT count(*) FROM cierre_solicitudlogistica")
            .await,
        0
    );
});
