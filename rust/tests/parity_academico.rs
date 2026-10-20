//! apps/academico/tests.py: certificate issuance, download, verification and portfolio.
mod common;
use common::*;
use serde_json::json;

async fn issue(t: &Ctx, reto: i64) -> u64 {
    let mut tx = t.pool.begin().await.unwrap();
    let issued = retosean::lifecycle::issue_certificates(&mut tx, reto, ADMIN, None)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    issued
}
/// EmisionDeCertificadosTests.setUp: finished reto, approved integration, one accepted and one rejected student.
async fn setup(t: &Ctx) -> (i64, i64, i64, i64, i64) {
    let (company, _) = t.company_user("empresa-cert", "900100200").await;
    let teacher = t.user("profe-cert", "PROFESOR").await;
    let student = t.user("est-cert", "ESTUDIANTE").await;
    let rejected = t.user("est-rechazado", "ESTUDIANTE").await;
    let reto = t.reto(company, "finalizado", "reto").await;
    let integration = t.integrate(reto, teacher).await;
    t.exec(&format!(
        "UPDATE integraciones_academicas SET estado='aprobada' WHERE id={integration}"
    ))
    .await;
    t.accept(reto, student).await;
    let application = t.accept(reto, rejected).await;
    t.exec(&format!(
        "UPDATE postulaciones SET estado='RECHAZADA' WHERE id={application}"
    ))
    .await;
    (reto, company, teacher, student, rejected)
}
async fn role(t: &Ctx, reto: i64, user: i64) -> String {
    t.text(&format!(
        "SELECT rol_participacion FROM certificados WHERE reto_id={reto} AND usuario_id={user}"
    ))
    .await
}

// EmisionDeCertificadosTests
parity!(test_emite_a_estudiante_profesor_y_empresa, |t| {
    let (reto, company, teacher, student, _) = setup(&t).await;
    issue(&t, reto).await;
    assert_eq!(role(&t, reto, student).await, "ESTUDIANTE");
    assert_eq!(role(&t, reto, teacher).await, "PROFESOR");
    assert_eq!(role(&t, reto, company).await, "EMPRESA");
});
parity!(test_no_emite_a_postulaciones_rechazadas, |t| {
    let (reto, _, _, _, rejected) = setup(&t).await;
    issue(&t, reto).await;
    assert_eq!(
        t.i64(&format!(
            "SELECT count(*) FROM certificados WHERE usuario_id={rejected}"
        ))
        .await,
        0
    );
});
parity!(test_no_emite_si_el_reto_no_esta_finalizado, |t| {
    let (reto, ..) = setup(&t).await;
    t.exec(&format!(
        "UPDATE retos SET estado='en_curso' WHERE id={reto}"
    ))
    .await;
    assert_eq!(issue(&t, reto).await, 0);
    assert_eq!(
        t.i64(&format!(
            "SELECT count(*) FROM certificados WHERE reto_id={reto}"
        ))
        .await,
        0
    );
});
parity!(test_es_idempotente, |t| {
    let (reto, ..) = setup(&t).await;
    issue(&t, reto).await;
    let total = t.i64("SELECT count(*) FROM certificados").await;
    assert_eq!(issue(&t, reto).await, 0);
    assert_eq!(t.i64("SELECT count(*) FROM certificados").await, total);
});
parity!(test_notifica_a_los_participantes, |t| {
    let (reto, _, _, student, _) = setup(&t).await;
    issue(&t, reto).await;
    assert!(t.notices(student, "CERTIFICADO_DISPONIBLE").await > 0);
});
parity!(test_genera_pdf_valido, |t| {
    let (reto, _, _, student, _) = setup(&t).await;
    issue(&t, reto).await;
    let id = t
        .i64(&format!(
            "SELECT id FROM certificados WHERE usuario_id={student}"
        ))
        .await;
    let (code, pdf) = download(&t.app, &t.admin, &format!("/api/certificados/{id}/pdf")).await;
    assert_eq!(code, 200);
    assert!(pdf.starts_with(b"%PDF"));
});

// DescargaYVerificacionTests
async fn certificate(t: &Ctx) -> (i64, String) {
    let (company, _) = t.company_user("empresa-desc", "900100300").await;
    let student = t.user("est-desc", "ESTUDIANTE").await;
    t.user("est-otro", "ESTUDIANTE").await;
    let reto = t.reto(company, "finalizado", "reto").await;
    let id = t.i64(&format!("INSERT INTO certificados(reto_id,usuario_id,rol_participacion,codigo_verificacion,emitido_en) VALUES({reto},{student},'ESTUDIANTE',gen_random_uuid(),now()) RETURNING id")).await;
    let code = t
        .text(&format!(
            "SELECT codigo_verificacion::text FROM certificados WHERE id={id}"
        ))
        .await;
    (id, code)
}
parity!(test_el_titular_descarga_su_certificado, |t| {
    let (id, _) = certificate(&t).await;
    let session = t.login("est-desc").await;
    let response = t.app.clone();
    let (code, pdf) = download(&response, &session, &format!("/api/certificados/{id}/pdf")).await;
    assert_eq!(code, 200);
    assert!(pdf.starts_with(b"%PDF"));
});
parity!(test_otro_estudiante_no_descarga_el_certificado_ajeno, |t| {
    let (id, _) = certificate(&t).await;
    let other = t.login("est-otro").await;
    let (code, _) = download(&t.app, &other, &format!("/api/certificados/{id}/pdf")).await;
    assert!(code == 404 || code == 403, "{code}");
});
parity!(test_la_verificacion_es_publica, |t| {
    let (_, code) = certificate(&t).await;
    let body = t
        .ok(&t.anon, "GET", &format!("/api/verificar/{code}"), None)
        .await;
    assert!(body.to_string().contains("Reto de prueba"), "{body}");
});
parity!(test_codigo_inexistente_no_valida, |t| {
    certificate(&t).await;
    let (code, body) = t
        .req(
            &t.anon,
            "GET",
            "/api/verificar/00000000-0000-0000-0000-000000000000",
            None,
        )
        .await;
    assert!(
        !code.is_success() || body["valido"] == false,
        "{code} {body}"
    );
});
parity!(test_portafolio_devuelve_pdf, |t| {
    certificate(&t).await;
    let session = t.login("est-desc").await;
    let (code, pdf) = download(&t.app, &session, "/api/portafolio").await;
    assert_eq!(code, 200);
    assert!(pdf.starts_with(b"%PDF"));
    assert!(String::from_utf8_lossy(&pdf).contains("Certificados obtenidos"));
});

// CierreEmiteCertificadosTests
parity!(test_finalizar_el_reto_emite_certificados, |t| {
    let (company, _) = t.company_user("empresa-cierre", "900100400").await;
    let student = t.user("est-cierre", "ESTUDIANTE").await;
    let reto = t.reto(company, "en_curso", "reto").await;
    t.accept(reto, student).await;
    let closure = t.i64(&format!("INSERT INTO cierres_reto(reto_id,acta_url,notas,impacto,encuesta_enviada,actualizado_en) VALUES({reto},'acta.pdf','','',false,now()) RETURNING id")).await;
    t.exec(&format!("INSERT INTO cierre_entregablefinal(cierre_id,nombre,archivo,creado_en) VALUES({closure},'Informe final','final.pdf',now())")).await;
    t.ok(
        &t.admin,
        "POST",
        &format!("/api/retos/{reto}/cerrar"),
        Some(json!({})),
    )
    .await;
    assert_eq!(
        t.text(&format!("SELECT estado FROM retos WHERE id={reto}"))
            .await,
        "finalizado"
    );
    assert_eq!(
        t.i64(&format!(
            "SELECT count(*) FROM certificados WHERE reto_id={reto} AND usuario_id={student}"
        ))
        .await,
        1
    );
});
parity!(
    test_estudiante_con_entregable_recibe_certificado_al_finalizar,
    |t| {
        let (company, _) = t.company_user("empresa-cierre-2", "900100500").await;
        let student = t.user("est-entregable", "ESTUDIANTE").await;
        let reto = t.reto(company, "en_curso", "reto").await;
        t.deliverable(reto, student).await;
        t.ok(
            &t.admin,
            "POST",
            &format!("/api/retos/{reto}/cerrar"),
            Some(json!({})),
        )
        .await;
        assert_eq!(
            t.text(&format!("SELECT estado FROM retos WHERE id={reto}"))
                .await,
            "finalizado"
        );
        assert_eq!(
            t.i64(&format!(
                "SELECT count(*) FROM certificados WHERE reto_id={reto} AND usuario_id={student}"
            ))
            .await,
            1
        );
    }
);

// NombreEnCertificadoTests
parity!(test_el_certificado_de_empresa_lleva_la_razon_social, |t| {
    let account = t.user("cuenta-empresa", "EMPRESA").await;
    t.exec(&format!("INSERT INTO empresas(nit,razon_social,sector_industrial,usuario_id) VALUES('900456456','Innovaciones Andinas S.A.S.','',{account})")).await;
    let reto = t.reto(account, "finalizado", "reto").await;
    let id = t.i64(&format!("INSERT INTO certificados(reto_id,usuario_id,rol_participacion,codigo_verificacion,emitido_en) VALUES({reto},{account},'EMPRESA',gen_random_uuid(),now()) RETURNING id")).await;
    let (code, pdf) = download(&t.app, &t.admin, &format!("/api/certificados/{id}/pdf")).await;
    assert_eq!(code, 200);
    assert!(String::from_utf8_lossy(&pdf).contains("Innovaciones Andinas S.A.S."));
});

// CertificadoConSimbolosTests
parity!(
    test_el_certificado_se_genera_con_simbolos_en_nombre_y_titulo,
    |t| {
        let company = t.user("empresa-simb", "EMPRESA").await;
        let student = t.user("est-simb", "ESTUDIANTE").await;
        t.exec(&format!("UPDATE usuarios SET first_name='Ana <i',last_name='O''Brien & Hijos' WHERE id={student}")).await;
        let reto = t.reto(company, "finalizado", "reto").await;
        t.exec(&format!(
            "UPDATE retos SET titulo='Reto <b & cia' WHERE id={reto}"
        ))
        .await;
        let id = t.i64(&format!("INSERT INTO certificados(reto_id,usuario_id,rol_participacion,codigo_verificacion,emitido_en) VALUES({reto},{student},'ESTUDIANTE',gen_random_uuid(),now()) RETURNING id")).await;
        let (code, pdf) = download(&t.app, &t.admin, &format!("/api/certificados/{id}/pdf")).await;
        assert_eq!(code, 200);
        assert!(pdf.starts_with(b"%PDF"));
    }
);
