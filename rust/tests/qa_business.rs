//! Package D regressions (companies, finance, grading, workflows, reminders).
//! Each test guards one QA fix: BIZ-2 (self-review), BIZ-3 (budget floor),
//! BIZ-4 (reto eligibility), WF-3 (self-approval), WF-7 (dedupe), WF-8 (days).
//! Run: cargo test --test qa_business -- --include-ignored
mod common;
use common::*;
use serde_json::json;

/// A single user who owns a company AND holds admin authority (rol=ADMIN linked
/// to empresas.usuario_id, as a promotion or an import would leave it). Used to
/// prove the separation-of-duties recusals.
async fn admin_owner(t: &Ctx, username: &str, nit: &str) -> (i64, i64, Session) {
    let (user, company) = t.company_user(username, nit).await;
    t.exec(&format!("UPDATE usuarios SET rol='ADMIN' WHERE id={user}"))
        .await;
    let session = t.login(username).await;
    (user, company, session)
}

// BIZ-2 — self-review of the company's own legal validation.
parity!(test_admin_no_revisa_su_propia_empresa, |t| {
    let (_owner, company, self_admin) = admin_owner(&t, "admin-dueno", "900700001").await;
    // The owning admin is recused.
    assert_eq!(
        t.status(
            &self_admin,
            "POST",
            &format!("/api/empresas/{company}/revisar"),
            Some(
                json!({"estado_validacion":"EN_REVISION","estado_listas_restrictivas":"PENDIENTE"})
            )
        )
        .await,
        403
    );
    assert_eq!(
        t.text(&format!(
            "SELECT estado_validacion FROM empresas WHERE id={company}"
        ))
        .await,
        "PENDIENTE"
    );
    // A different (non-owning) administrator still reviews it.
    t.ok(
        &t.admin,
        "POST",
        &format!("/api/empresas/{company}/revisar"),
        Some(json!({"estado_validacion":"EN_REVISION","estado_listas_restrictivas":"PENDIENTE"})),
    )
    .await;
    assert_eq!(
        t.text(&format!(
            "SELECT estado_validacion FROM empresas WHERE id={company}"
        ))
        .await,
        "EN_REVISION"
    );
});

// BIZ-2 — self-review of the company's own legal document.
parity!(test_admin_no_revisa_documento_de_su_propia_empresa, |t| {
    let (_owner, company, self_admin) = admin_owner(&t, "admin-doc", "900700002").await;
    let doc = t.i64(&format!("INSERT INTO documentos_empresas(empresa_id,tipo_documento,archivo,estado,fecha_expedicion,fecha_carga,motivo_rechazo) VALUES({company},'RUT','x.pdf','CARGADO',(now() AT TIME ZONE 'America/Bogota')::date,now(),'') RETURNING id")).await;
    assert_eq!(
        t.status(
            &self_admin,
            "POST",
            &format!("/api/documentos/{doc}/revisar"),
            Some(json!({"estado":"VERIFICADO"}))
        )
        .await,
        403
    );
    assert_eq!(
        t.text(&format!(
            "SELECT estado FROM documentos_empresas WHERE id={doc}"
        ))
        .await,
        "CARGADO"
    );
    // A different administrator verifies it.
    t.ok(
        &t.admin,
        "POST",
        &format!("/api/documentos/{doc}/revisar"),
        Some(json!({"estado":"VERIFICADO"})),
    )
    .await;
    assert_eq!(
        t.text(&format!(
            "SELECT estado FROM documentos_empresas WHERE id={doc}"
        ))
        .await,
        "VERIFICADO"
    );
});

// BIZ-2 — self-processing of the company's own agreement.
parity!(test_admin_no_tramita_convenio_de_su_propia_empresa, |t| {
    let (_owner, company, self_admin) = admin_owner(&t, "admin-conv", "900700003").await;
    assert_eq!(
        t.status(
            &self_admin,
            "POST",
            &format!("/api/empresas/{company}/convenio"),
            Some(json!({"estado":"RADICADO"}))
        )
        .await,
        403
    );
    assert_eq!(
        t.text(&format!(
            "SELECT estado_convenio FROM empresas WHERE id={company}"
        ))
        .await,
        "NO_RADICADO"
    );
});

// WF-3 — an admin linked to the owning company cannot approve their own reto.
parity!(test_admin_no_aprueba_reto_de_su_propia_empresa, |t| {
    let (owner, _company, self_admin) = admin_owner(&t, "admin-reto", "900700004").await;
    let reto = t.reto(owner, "en_revision", "reto").await;
    assert_eq!(
        t.status(
            &self_admin,
            "POST",
            &format!("/api/retos/{reto}/estado"),
            Some(json!({"estado":"aprobado","comentario":"Ok"}))
        )
        .await,
        403
    );
    assert_eq!(
        t.text(&format!("SELECT estado FROM retos WHERE id={reto}"))
            .await,
        "en_revision"
    );
    // Control: a non-owning admin still approves an operable company's reto.
    let (other, other_company) = t.company_user("empresa-aprobable", "900700005").await;
    t.verify_documents(other_company).await;
    let reto2 = t.reto(other, "en_revision", "reto").await;
    t.ok(
        &t.admin,
        "POST",
        &format!("/api/retos/{reto2}/estado"),
        Some(json!({"estado":"aprobado","comentario":"Ok"})),
    )
    .await;
    assert_eq!(
        t.text(&format!("SELECT estado FROM retos WHERE id={reto2}"))
            .await,
        "aprobado"
    );
});

// BIZ-3 — the budget cannot drop below the amount already committed/executed.
parity!(
    test_no_se_reduce_el_presupuesto_por_debajo_de_lo_asignado,
    |t| {
        let reto = t.reto(COMPANY_USER, "aprobado", "reto").await;
        let budget = t.i64(&format!("INSERT INTO presupuestos(reto_id,monto_total,alerta_80_enviada,creado_en,actualizado_en) VALUES({reto},1000.00,false,now(),now()) RETURNING id")).await;
        t.exec(&format!("INSERT INTO gastos(presupuesto_id,categoria,monto,estado,descripcion,creado_en,ejecutado_en) VALUES({budget},'OTROS',900.00,'EJECUTADO','',now(),now())")).await;
        // 500 < 900 already executed: rejected, and the total is untouched.
        let (code, body) = t
            .req(
                &t.admin,
                "POST",
                &format!("/api/retos/{reto}/presupuesto"),
                Some(json!({"monto_total":"500"})),
            )
            .await;
        assert_eq!(code, 400, "{body}");
        assert_eq!(
            t.text(&format!(
                "SELECT monto_total::text FROM presupuestos WHERE id={budget}"
            ))
            .await,
            "1000.00"
        );
        // Reducing to exactly what is assigned is allowed.
        t.ok(
            &t.admin,
            "POST",
            &format!("/api/retos/{reto}/presupuesto"),
            Some(json!({"monto_total":"900"})),
        )
        .await;
        assert_eq!(
            t.text(&format!(
                "SELECT monto_total::text FROM presupuestos WHERE id={budget}"
            ))
            .await,
            "900.00"
        );
    }
);

// BIZ-4 — a cancelled reto freezes new expenses and execution of committed ones.
parity!(test_reto_cancelado_no_admite_gastos_ni_ejecucion, |t| {
    let reto = t.reto(COMPANY_USER, "aprobado", "reto").await;
    let budget = t.i64(&format!("INSERT INTO presupuestos(reto_id,monto_total,alerta_80_enviada,creado_en,actualizado_en) VALUES({reto},1000.00,false,now(),now()) RETURNING id")).await;
    let gasto = t.i64(&format!("INSERT INTO gastos(presupuesto_id,categoria,monto,estado,descripcion,creado_en) VALUES({budget},'OTROS',100.00,'COMPROMETIDO','',now()) RETURNING id")).await;
    t.exec(&format!(
        "UPDATE retos SET estado='cancelado' WHERE id={reto}"
    ))
    .await;
    // New expense rejected.
    let (code, body) = t
        .req(
            &t.admin,
            "POST",
            &format!("/api/presupuestos/{budget}/gastos"),
            Some(json!({"categoria":"OTROS","monto":"50","estado":"EJECUTADO"})),
        )
        .await;
    assert_eq!(code, 400, "{body}");
    assert_eq!(
        t.i64(&format!(
            "SELECT count(*) FROM gastos WHERE presupuesto_id={budget}"
        ))
        .await,
        1
    );
    // Executing the committed expense is rejected too.
    assert_eq!(
        t.status(
            &t.admin,
            "POST",
            &format!("/api/gastos/{gasto}/ejecutar"),
            Some(json!({}))
        )
        .await,
        400
    );
    assert_eq!(
        t.text(&format!("SELECT estado FROM gastos WHERE id={gasto}"))
            .await,
        "COMPROMETIDO"
    );
});

// WF-7 — withdrawing, re-submitting and withdrawing again notifies twice.
parity!(
    test_retirar_y_reenviar_notifica_dos_veces_a_la_empresa,
    |t| {
        let (company_user, _empresa) = t.company_user("empresa-gestiona", "900700010").await;
        let reto = t.reto(company_user, "abierto", "reto").await;
        t.user("est-ciclo", "ESTUDIANTE").await;
        let student = t.login("est-ciclo").await;
        let body = t
            .ok(
                &student,
                "POST",
                &format!("/api/retos/{reto}/postular"),
                Some(json!({"motivacion":"Quiero participar","acepta_terminos":true})),
            )
            .await;
        let application = body["id"].as_i64().unwrap();
        assert_eq!(t.notices(company_user, "POSTULACION_GESTIONADA").await, 0);
        // First withdrawal notifies the company.
        t.ok(
            &student,
            "POST",
            &format!("/api/postulaciones/{application}/estado"),
            Some(json!({"estado":"RETIRADA"})),
        )
        .await;
        assert_eq!(t.notices(company_user, "POSTULACION_GESTIONADA").await, 1);
        // Re-submit (RETIRADA -> PENDIENTE), then withdraw again.
        t.ok(
            &student,
            "POST",
            &format!("/api/retos/{reto}/postular"),
            Some(json!({"motivacion":"Quiero participar de nuevo","acepta_terminos":true})),
        )
        .await;
        t.ok(
            &student,
            "POST",
            &format!("/api/postulaciones/{application}/estado"),
            Some(json!({"estado":"RETIRADA"})),
        )
        .await;
        // The second withdrawal is a distinct notification, not deduped away.
        assert_eq!(t.notices(company_user, "POSTULACION_GESTIONADA").await, 2);
    }
);

// WF-8 — reminders ignore out-of-range day arguments but keep valid ones.
parity!(test_recordatorios_ignoran_dias_fuera_de_rango, |t| {
    let (company_user, _empresa) = t.company_user("empresa-rem", "900700020").await;
    let near = t.reto(company_user, "abierto", "reto").await;
    let far = t.reto(company_user, "abierto", "reto").await;
    t.exec(&format!("UPDATE retos SET fecha_limite_postulacion=(now() AT TIME ZONE 'America/Bogota')::date+1 WHERE id={near}")).await;
    t.exec(&format!("UPDATE retos SET fecha_limite_postulacion=(now() AT TIME ZONE 'America/Bogota')::date+400 WHERE id={far}")).await;
    let student = t.user("est-rem", "ESTUDIANTE").await;
    for reto in [near, far] {
        t.exec(&format!("INSERT INTO postulaciones(reto_id,estudiante_id,estado,motivacion,semestre,programa,fecha_postulacion,habilidades,acepta_terminos) VALUES({reto},{student},'PENDIENTE','x',5,'',now(),'',true)")).await;
    }
    // 400/-5/1000000 are discarded; the duplicate 1 is collapsed; only day 1 runs.
    retosean::reminders::run(&t.pool, &[1, 400, -5, 1000000, 1])
        .await
        .unwrap();
    assert_eq!(
        t.i64(&format!("SELECT count(*) FROM notificaciones WHERE clave_dedupe LIKE 'recordatorio:postulacion:{near}:%'")).await,
        1
    );
    assert_eq!(
        t.i64(&format!("SELECT count(*) FROM notificaciones WHERE clave_dedupe LIKE 'recordatorio:postulacion:{far}:%'")).await,
        0
    );
});

// WF-4 — a finished/cancelled reto freezes its pending applications; only retos
// still open for review move PENDIENTE to EN_REVISION when the company opens the panel.
parity!(
    test_postulaciones_de_reto_finalizado_no_pasan_a_revision,
    |t| {
        let (company, _) = t.company_user("empresa-wf4", "900700030").await;
        let student = t.user("est-wf4", "ESTUDIANTE").await;
        let open = t.reto(company, "abierto", "reto").await;
        let done = t.reto(company, "finalizado", "reto").await;
        for reto in [open, done] {
            t.exec(&format!("INSERT INTO postulaciones(reto_id,estudiante_id,estado,motivacion,semestre,programa,fecha_postulacion,habilidades,acepta_terminos) VALUES({reto},{student},'PENDIENTE','x',5,'',now(),'',true)")).await;
        }
        let session = t.login("empresa-wf4").await;
        t.ok(&session, "GET", "/api/records/postulaciones", None)
            .await;
        assert_eq!(
            t.text(&format!(
                "SELECT estado FROM postulaciones WHERE reto_id={open}"
            ))
            .await,
            "EN_REVISION"
        );
        assert_eq!(
            t.text(&format!(
                "SELECT estado FROM postulaciones WHERE reto_id={done}"
            ))
            .await,
            "PENDIENTE"
        );
    }
);

// BIZ-7 — a published grade is final and cannot be overwritten by re-grading.
parity!(test_no_se_recalifica_una_nota_ya_publicada, |t| {
    let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
    let teacher = t.user("profe-pub", "PROFESOR").await;
    t.integrate(reto, teacher).await;
    let student = t.user("est-pub", "ESTUDIANTE").await;
    let deliverable = t.deliverable(reto, student).await;
    let session = t.login("profe-pub").await;
    t.ok(
        &session,
        "POST",
        &format!("/api/entregables/{deliverable}/calificar"),
        Some(json!({"nota":"4","comentario":"Bien"})),
    )
    .await;
    t.ok(
        &session,
        "POST",
        &format!("/api/retos/{reto}/publicar-calificaciones"),
        Some(json!({})),
    )
    .await;
    assert!(published(&t, deliverable).await);
    let (code, body) = t
        .req(
            &session,
            "POST",
            &format!("/api/entregables/{deliverable}/calificar"),
            Some(json!({"nota":"5","comentario":"Cambiada","confirmar_recalificacion":true})),
        )
        .await;
    assert_eq!(code, 400, "{body}");
    assert_eq!(
        t.text(&format!(
            "SELECT nota::text FROM entregables WHERE id={deliverable}"
        ))
        .await,
        "4.00"
    );
    assert!(published(&t, deliverable).await);
});

// BIZ-6 — a rubric whose total exceeds the deliverable scale is rejected (full
// score would otherwise be unreachable with the scaled nota).
parity!(test_rubrica_que_supera_la_escala_se_rechaza, |t| {
    let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
    let teacher = t.user("profe-rub", "PROFESOR").await;
    t.integrate(reto, teacher).await;
    let student = t.user("est-rub", "ESTUDIANTE").await;
    let deliverable = t.deliverable(reto, student).await;
    let session = t.login("profe-rub").await;
    let rubric = t.i64(&format!("INSERT INTO evaluacion_rubrica(reto_id,nombre,descripcion,activa,creada_por_id,creada_en) VALUES({reto},'Sobre-escala','',true,{teacher},now()) RETURNING id")).await;
    let c1 = t.i64(&format!("INSERT INTO evaluacion_criteriorubrica(rubrica_id,nombre,descripcion,puntaje_maximo,orden) VALUES({rubric},'A','',5.00,1) RETURNING id")).await;
    let c2 = t.i64(&format!("INSERT INTO evaluacion_criteriorubrica(rubrica_id,nombre,descripcion,puntaje_maximo,orden) VALUES({rubric},'B','',5.00,2) RETURNING id")).await;
    // sum = 5 (within the deliverable's 5) but rubric total = 10 > 5: rejected.
    let (code, body) = t
        .req(
            &session,
            "POST",
            &format!("/api/entregables/{deliverable}/calificar"),
            Some(json!({"rubrica_id":rubric,"criterios":{c1.to_string():"5",c2.to_string():"0"}})),
        )
        .await;
    assert_eq!(code, 400, "{body}");
    assert!(
        t.bool(&format!(
            "SELECT nota IS NULL FROM entregables WHERE id={deliverable}"
        ))
        .await
    );
});

// BIZ-6 — a rubric whose total equals the deliverable scale still allows full mark.
parity!(
    test_rubrica_dentro_de_la_escala_permite_la_nota_completa,
    |t| {
        let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
        let teacher = t.user("profe-rubok", "PROFESOR").await;
        t.integrate(reto, teacher).await;
        let student = t.user("est-rubok", "ESTUDIANTE").await;
        let deliverable = t.deliverable(reto, student).await;
        let session = t.login("profe-rubok").await;
        let rubric = t.i64(&format!("INSERT INTO evaluacion_rubrica(reto_id,nombre,descripcion,activa,creada_por_id,creada_en) VALUES({reto},'Escala 5','',true,{teacher},now()) RETURNING id")).await;
        let c1 = t.i64(&format!("INSERT INTO evaluacion_criteriorubrica(rubrica_id,nombre,descripcion,puntaje_maximo,orden) VALUES({rubric},'A','',2.50,1) RETURNING id")).await;
        let c2 = t.i64(&format!("INSERT INTO evaluacion_criteriorubrica(rubrica_id,nombre,descripcion,puntaje_maximo,orden) VALUES({rubric},'B','',2.50,2) RETURNING id")).await;
        let body = t
        .ok(
            &session,
            "POST",
            &format!("/api/entregables/{deliverable}/calificar"),
            Some(json!({"rubrica_id":rubric,"criterios":{c1.to_string():"2.50",c2.to_string():"2.50"}})),
        )
        .await;
        assert_eq!(body["nota"], "5.00");
    }
);

async fn published(t: &Ctx, deliverable: i64) -> bool {
    t.bool(&format!(
        "SELECT calificacion_publicada FROM entregables WHERE id={deliverable}"
    ))
    .await
}
