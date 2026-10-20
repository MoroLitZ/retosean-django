//! apps/empresas/tests.py: legal documents and their notices, Cámara de Comercio
//! validity, the HU00 gate, the agreement, the company's progress panel (HU14),
//! the document catalog and special permission, blocking, expiry and history.
mod common;
use common::*;
use serde_json::{Value, json};

const PDF: &[u8] = b"%PDF-1.4 documento";

async fn can_operate(t: &Ctx, company: i64) -> bool {
    reasons(t, company).await.is_empty()
}
async fn reasons(t: &Ctx, company: i64) -> Vec<String> {
    let mut tx = t.pool.begin().await.unwrap();
    let reasons = retosean::permissions::operating_reasons(&mut tx, company)
        .await
        .unwrap();
    tx.rollback().await.unwrap();
    reasons
}
async fn document(t: &Ctx, company: i64, kind: &str, estado: &str) -> i64 {
    t.i64(&format!("INSERT INTO documentos_empresas(empresa_id,tipo_documento,archivo,estado,fecha_expedicion,fecha_carga,motivo_rechazo) VALUES({company},'{kind}','x.pdf','{estado}',CURRENT_DATE,now(),'') ON CONFLICT (empresa_id,tipo_documento) DO UPDATE SET estado=excluded.estado RETURNING id")).await
}
async fn upload(
    t: &Ctx,
    session: &Session,
    kind: &str,
    issued: chrono::NaiveDate,
    file: (&str, &[u8]),
) -> (u16, Value) {
    let date = issued.to_string();
    let (code, body) = multipart(
        &t.app,
        session,
        &format!("/api/empresa/documentos/{kind}/cargar"),
        &[("fecha_expedicion", date.as_str())],
        &[("archivo", file.0, file.1)],
    )
    .await;
    (code.as_u16(), body)
}
fn today() -> chrono::NaiveDate {
    (chrono::Utc::now() - chrono::Duration::hours(5)).date_naive()
}

// DocumentacionEmpresaTests
async fn acme(t: &Ctx) -> (i64, i64, Session) {
    let (user, company) = t.company_user("empresa-doc", "900123456").await;
    t.exec(&format!(
        "UPDATE empresas SET razon_social='ACME SAS' WHERE id={company}"
    ))
    .await;
    (user, company, t.login("empresa-doc").await)
}
parity!(test_rechazar_documento_notifica_a_la_empresa, |t| {
    let (user, company, _) = acme(&t).await;
    let doc = document(&t, company, "RUT", "CARGADO").await;
    t.ok(
        &t.admin,
        "POST",
        &format!("/api/documentos/{doc}/revisar"),
        Some(json!({"estado":"RECHAZADO","comentario":"El archivo esta ilegible."})),
    )
    .await;
    assert_eq!(
        t.text(&format!(
            "SELECT estado FROM documentos_empresas WHERE id={doc}"
        ))
        .await,
        "RECHAZADO"
    );
    let (event, message): (String, String) = sqlx::query_as(&format!(
        "SELECT evento,mensaje FROM notificaciones WHERE usuario_id={user}"
    ))
    .fetch_one(&t.pool)
    .await
    .unwrap();
    assert_eq!(event, "DOCUMENTO_REVISADO");
    assert!(message.contains("El archivo esta ilegible."), "{message}");
});
parity!(test_aprobar_documento_notifica_a_la_empresa, |t| {
    let (user, company, _) = acme(&t).await;
    let doc = document(&t, company, "RUT", "CARGADO").await;
    t.ok(
        &t.admin,
        "POST",
        &format!("/api/documentos/{doc}/revisar"),
        Some(json!({"estado":"VERIFICADO"})),
    )
    .await;
    assert_eq!(
        t.text(&format!(
            "SELECT tipo FROM notificaciones WHERE usuario_id={user}"
        ))
        .await,
        "EXITO"
    );
});
parity!(test_cargar_documento_notifica_al_admin, |t| {
    let (_, _, session) = acme(&t).await;
    let (code, body) = upload(&t, &session, "RUT", today(), ("rut.pdf", PDF)).await;
    assert_eq!(code, 200, "{body}");
    let message = t.text(&format!("SELECT mensaje FROM notificaciones WHERE usuario_id={ADMIN} AND evento='DOCUMENTO_CARGADO_ADMIN'")).await;
    assert!(
        message.contains("ACME SAS") && message.contains("RUT"),
        "{message}"
    );
});
parity!(test_el_estado_de_validacion_avanza_a_verificada, |t| {
    let (_, company, _) = acme(&t).await;
    t.verify_documents(company).await;
    t.exec(&format!(
        "UPDATE empresas SET estado_validacion='EN_REVISION' WHERE id={company}"
    ))
    .await;
    let rut = document(&t, company, "RUT", "CARGADO").await;
    t.ok(
        &t.admin,
        "POST",
        &format!("/api/documentos/{rut}/revisar"),
        Some(json!({"estado":"VERIFICADO"})),
    )
    .await;
    assert_eq!(
        t.text(&format!(
            "SELECT estado_validacion FROM empresas WHERE id={company}"
        ))
        .await,
        "VERIFICADA"
    );
    assert!(can_operate(&t, company).await);
});
parity!(test_con_solo_rut_y_camara_aun_no_puede_operar, |t| {
    let (_, company, _) = acme(&t).await;
    document(&t, company, "RUT", "VERIFICADO").await;
    document(&t, company, "CAMARA_COMERCIO", "VERIFICADO").await;
    assert!(!can_operate(&t, company).await);
});
parity!(test_un_documento_rechazado_deja_la_empresa_rechazada, |t| {
    let (_, company, _) = acme(&t).await;
    let doc = document(&t, company, "RUT", "CARGADO").await;
    t.ok(
        &t.admin,
        "POST",
        &format!("/api/documentos/{doc}/revisar"),
        Some(json!({"estado":"RECHAZADO","comentario":"No sirve"})),
    )
    .await;
    assert_eq!(
        t.text(&format!(
            "SELECT estado_validacion FROM empresas WHERE id={company}"
        ))
        .await,
        "RECHAZADA"
    );
});

// VigenciaCamaraComercioTests
parity!(test_documento_reciente_es_valido, |t| {
    let (_, _, session) = acme(&t).await;
    assert_eq!(
        upload(
            &t,
            &session,
            "CAMARA_COMERCIO",
            today() - chrono::Duration::days(10),
            ("camara.pdf", PDF)
        )
        .await
        .0,
        200
    );
});
parity!(test_documento_de_mas_de_30_dias_se_rechaza, |t| {
    let (_, _, session) = acme(&t).await;
    assert_eq!(
        upload(
            &t,
            &session,
            "CAMARA_COMERCIO",
            today() - chrono::Duration::days(45),
            ("camara.pdf", PDF)
        )
        .await
        .0,
        400
    );
});

// GateDeDocumentacionTests
async fn gate(t: &Ctx) -> (i64, i64, Session) {
    let (user, company) = t.company_user("empresa-gate", "900777666").await;
    (user, company, t.login("empresa-gate").await)
}
parity!(test_sin_documentos_no_puede_publicar_retos, |t| {
    let (_, _, session) = gate(&t).await;
    let (code, body) = t
        .req(
            &session,
            "POST",
            "/api/retos",
            Some(json!({"titulo":"Reto","tipo":"reto","descripcion":"d","area":"a"})),
        )
        .await;
    assert_eq!(code, 400);
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("no puede publicar"),
        "{body}"
    );
});
parity!(
    test_elegir_sin_convenio_lleva_a_documentos_y_no_en_bucle,
    |t| {
        let (_, company, session) = gate(&t).await;
        let body = t
            .ok(
                &session,
                "POST",
                "/api/empresa/modo-convenio",
                Some(json!({"opcion":"no_convenio"})),
            )
            .await;
        assert_eq!(body["siguiente"], "documentos");
        assert!(
            t.bool(&format!(
                "SELECT renuncio_a_convenio FROM empresas WHERE id={company}"
            ))
            .await
        );
    }
);
parity!(test_sin_convenio_no_se_salta_la_documentacion, |t| {
    let (_, company, _) = gate(&t).await;
    t.exec(&format!(
        "UPDATE empresas SET renuncio_a_convenio=true WHERE id={company}"
    ))
    .await;
    assert!(!can_operate(&t, company).await);
});
parity!(
    test_sin_convenio_no_se_exigen_los_documentos_del_convenio,
    |t| {
        let (_, company, _) = gate(&t).await;
        t.exec(&format!(
            "UPDATE empresas SET renuncio_a_convenio=true WHERE id={company}"
        ))
        .await;
        let without = reasons(&t, company).await.join(" | ");
        assert!(
            !without.contains("Minuta") && without.contains("RUT"),
            "{without}"
        );
        t.exec(&format!(
            "UPDATE empresas SET renuncio_a_convenio=false WHERE id={company}"
        ))
        .await;
        assert!(reasons(&t, company).await.join(" | ").contains("Minuta"));
    }
);
parity!(
    test_con_todos_los_obligatorios_verificados_puede_publicar,
    |t| {
        let (_, company, session) = gate(&t).await;
        t.verify_documents(company).await;
        assert!(can_operate(&t, company).await);
        t.ok(
            &session,
            "POST",
            "/api/retos",
            Some(json!({"titulo":"Reto","tipo":"reto","descripcion":"d","area":"a"})),
        )
        .await;
    }
);
parity!(test_documento_vencido_deshabilita_a_la_empresa, |t| {
    let (_, company, _) = gate(&t).await;
    t.verify_documents(company).await;
    t.exec(&format!("UPDATE documentos_empresas SET fecha_vencimiento=CURRENT_DATE-1 WHERE empresa_id={company} AND tipo_documento='CAMARA_COMERCIO'")).await;
    assert!(!can_operate(&t, company).await);
});

// ConvenioEmpresaTests
async fn agreement(t: &Ctx) -> (i64, i64, Session) {
    t.user("admin-conv", "ADMIN").await;
    let (user, company) = t.company_user("empresa-conv", "900333444").await;
    let _ = user;
    (user, company, t.login("admin-conv").await)
}
async fn advance(t: &Ctx, admin: &Session, company: i64, body: Value) -> u16 {
    t.status(
        admin,
        "POST",
        &format!("/api/empresas/{company}/convenio"),
        Some(body),
    )
    .await
    .as_u16()
}
async fn in_force(t: &Ctx, company: i64) -> bool {
    t.bool(&format!("SELECT estado_convenio='FIRMADO' AND fecha_vencimiento_convenio>=CURRENT_DATE FROM empresas WHERE id={company}")).await
}
parity!(
    test_tramite_completo_queda_firmado_con_vencimiento_e_historial,
    |t| {
        let (user, company, admin) = agreement(&t).await;
        let due = today() + chrono::Duration::days(365);
        advance(&t, &admin, company, json!({"estado":"RADICADO"})).await;
        advance(
            &t,
            &admin,
            company,
            json!({"estado":"EN_JURIDICA","comentario":"En revision de Juridica"}),
        )
        .await;
        advance(
            &t,
            &admin,
            company,
            json!({"estado":"FIRMADO","fecha_vencimiento":due}),
        )
        .await;
        assert_eq!(
            t.text(&format!(
                "SELECT fecha_vencimiento_convenio::text FROM empresas WHERE id={company}"
            ))
            .await,
            due.to_string()
        );
        assert!(in_force(&t, company).await);
        let steps: Vec<(String, String)> = t
            .history("empresas", "empresa", company, "estado_convenio")
            .await
            .into_iter()
            .map(|s| (s.0, s.1))
            .collect();
        let expected = [
            ("NO_RADICADO", "RADICADO"),
            ("RADICADO", "EN_JURIDICA"),
            ("EN_JURIDICA", "FIRMADO"),
        ];
        assert_eq!(
            steps
                .iter()
                .map(|(a, b)| (a.as_str(), b.as_str()))
                .collect::<Vec<_>>(),
            expected
        );
        assert!(t.bool(&format!("SELECT EXISTS(SELECT 1 FROM notificaciones WHERE usuario_id={user} AND evento='CONVENIO_ACTUALIZADO' AND tipo='EXITO')")).await);
    }
);
parity!(test_no_se_salta_juridica, |t| {
    let (_, company, admin) = agreement(&t).await;
    advance(&t, &admin, company, json!({"estado":"RADICADO"})).await;
    assert_eq!(
        advance(
            &t,
            &admin,
            company,
            json!({"estado":"FIRMADO","fecha_vencimiento":today() + chrono::Duration::days(30)})
        )
        .await,
        400
    );
    assert_eq!(
        t.text(&format!(
            "SELECT estado_convenio FROM empresas WHERE id={company}"
        ))
        .await,
        "RADICADO"
    );
});
parity!(test_firmar_exige_fecha_de_vencimiento, |t| {
    let (_, company, admin) = agreement(&t).await;
    assert_eq!(
        advance(&t, &admin, company, json!({"estado":"FIRMADO"})).await,
        400
    );
    assert_eq!(
        t.text(&format!(
            "SELECT estado_convenio FROM empresas WHERE id={company}"
        ))
        .await,
        "NO_RADICADO"
    );
});
parity!(test_convenio_vigente_registrado_directamente, |t| {
    let (_, company, admin) = agreement(&t).await;
    assert_eq!(
        advance(
            &t,
            &admin,
            company,
            json!({"estado":"FIRMADO","fecha_vencimiento":today() + chrono::Duration::days(30)})
        )
        .await,
        200
    );
    assert!(in_force(&t, company).await);
});
parity!(test_convenio_vencido_no_esta_vigente, |t| {
    let (_, company, _) = agreement(&t).await;
    t.exec(&format!("UPDATE empresas SET estado_convenio='FIRMADO',fecha_vencimiento_convenio=CURRENT_DATE-1 WHERE id={company}")).await;
    assert!(!in_force(&t, company).await);
});
parity!(test_solo_el_admin_actualiza_el_convenio, |t| {
    let (_, company, _) = agreement(&t).await;
    let session = t.login("empresa-conv").await;
    assert_eq!(
        advance(&t, &session, company, json!({"estado":"RADICADO"})).await,
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

// SeguimientoAvanceEmpresaTests
async fn monitored(t: &Ctx) -> (i64, i64, Session) {
    let (owner, _) = t.company_user("empresa-seg", "900444100").await;
    let (other, _) = t.company_user("empresa-otra", "900444200").await;
    let reto = t.reto(owner, "en_curso", "reto").await;
    let foreign = t.reto(other, "en_curso", "reto").await;
    t.exec(&format!("INSERT INTO seguimientos_reto(reto_id,tipo_sesion,fecha_sesion,porcentaje_avance,avances,observaciones,acuerdos,creado_en) VALUES({reto},'seguimiento',CURRENT_DATE-7,20,'Kickoff','','',now()),({reto},'seguimiento',CURRENT_DATE,60,'Prototipo','','',now())")).await;
    (reto, foreign, t.login("empresa-seg").await)
}
parity!(test_muestra_el_porcentaje_del_ultimo_seguimiento, |t| {
    let (reto, _, session) = monitored(&t).await;
    let panel = t
        .ok(&session, "GET", &format!("/api/retos/{reto}/avance"), None)
        .await;
    assert_eq!(panel["porcentaje"], 60);
    assert!(panel.to_string().contains("Prototipo"));
});
parity!(test_no_ve_retos_de_otra_empresa, |t| {
    let (_, foreign, session) = monitored(&t).await;
    assert_eq!(
        t.status(
            &session,
            "GET",
            &format!("/api/retos/{foreign}/avance"),
            None
        )
        .await,
        404
    );
});
parity!(test_otros_roles_no_acceden, |t| {
    let (reto, _, _) = monitored(&t).await;
    assert_ne!(
        t.status(
            &t.student,
            "GET",
            &format!("/api/retos/{reto}/avance"),
            None
        )
        .await,
        200
    );
});
parity!(
    test_empresa_comenta_entregable_y_avisa_al_equipo_y_profesor,
    |t| {
        let (reto, _, session) = monitored(&t).await;
        let teacher = t.user("profe-com", "PROFESOR").await;
        t.integrate(reto, teacher).await;
        let student = t.user("est-com", "ESTUDIANTE").await;
        let deliverable = t.deliverable(reto, student).await;
        t.ok(
            &session,
            "POST",
            &format!("/api/entregables/{deliverable}/comentario"),
            Some(json!({"comentario":"Muy buen avance"})),
        )
        .await;
        let company = t
            .i64("SELECT id FROM usuarios WHERE username='empresa-seg'")
            .await;
        assert_eq!(t.i64(&format!("SELECT count(*) FROM evaluacion_comentarioentregable WHERE entregable_id={deliverable} AND autor_id={company}")).await, 1);
        for recipient in [student, teacher] {
            assert_eq!(t.notices(recipient, "COMENTARIO_EMPRESA").await, 1);
        }
        assert!(
            t.ok(&session, "GET", &format!("/api/retos/{reto}/avance"), None)
                .await
                .to_string()
                .contains("Muy buen avance")
        );
    }
);
parity!(test_empresa_no_comenta_entregables_de_otra, |t| {
    let (_, foreign, session) = monitored(&t).await;
    let student = t.user("est-ajeno", "ESTUDIANTE").await;
    let deliverable = t.deliverable(foreign, student).await;
    assert_eq!(
        t.status(
            &session,
            "POST",
            &format!("/api/entregables/{deliverable}/comentario"),
            Some(json!({"comentario":"Hola"}))
        )
        .await,
        403
    );
});

// CatalogoDocumentosTests
async fn catalog(t: &Ctx) -> (i64, i64, Session, Session, Session) {
    t.user("admin-cat", "ADMIN").await;
    let leader = t.user("lider-facultad", "ADMIN").await;
    let admin = t.login("admin-cat").await;
    t.ok(
        &admin,
        "POST",
        &format!("/api/usuarios/{leader}/permisos"),
        Some(json!({"codigos":["empresas.cargar_listas_informa"]})),
    )
    .await;
    let (user, company) = t.company_user("empresa-cat", "900888999").await;
    let _ = user;
    (
        user,
        company,
        admin,
        t.login("lider-facultad").await,
        t.login("empresa-cat").await,
    )
}
parity!(
    test_catalogo_trae_los_ocho_documentos_obligatorios_del_brief,
    |t| {
        let codes = t.texts("SELECT codigo FROM tipos_documento_legal WHERE obligatorio AND activo ORDER BY codigo").await;
        let mut expected = vec![
            "MINUTA_CONVENIO",
            "CAMARA_COMERCIO",
            "RUT",
            "CEDULA_REPRESENTANTE",
            "LISTAS_INFORMA",
            "FORMATO_COMPROMISO",
            "TERMINOS_REFERENCIA",
            "FICHA_RETO",
        ];
        expected.sort();
        assert_eq!(codes, expected);
    }
);
parity!(test_la_empresa_no_puede_cargar_listas_informa, |t| {
    let (_, company, _, _, session) = catalog(&t).await;
    assert_eq!(
        upload(&t, &session, "LISTAS_INFORMA", today(), ("doc.pdf", PDF))
            .await
            .0,
        403
    );
    assert_eq!(t.i64(&format!("SELECT count(*) FROM documentos_empresas WHERE empresa_id={company} AND tipo_documento='LISTAS_INFORMA'")).await, 0);
});
parity!(test_solo_quien_tiene_el_permiso_carga_listas_informa, |t| {
    let (_, company, admin, leader, _) = catalog(&t).await;
    let date = today().to_string();
    let url = format!("/api/empresas/{company}/documentos/LISTAS_INFORMA/cargar");
    assert_eq!(
        multipart(
            &t.app,
            &admin,
            &url,
            &[("fecha_expedicion", date.as_str())],
            &[("archivo", "doc.pdf", PDF)]
        )
        .await
        .0,
        403
    );
    let (code, body) = multipart(
        &t.app,
        &leader,
        &url,
        &[("fecha_expedicion", date.as_str())],
        &[("archivo", "doc.pdf", PDF)],
    )
    .await;
    assert_eq!(code, 200, "{body}");
    assert_eq!(t.text(&format!("SELECT estado FROM documentos_empresas WHERE empresa_id={company} AND tipo_documento='LISTAS_INFORMA'")).await, "CARGADO");
});
parity!(test_formato_distinto_de_pdf_se_rechaza, |t| {
    let (_, company, _, _, session) = catalog(&t).await;
    assert_eq!(
        upload(&t, &session, "RUT", today(), ("rut.docx", b"x"))
            .await
            .0,
        400
    );
    assert_eq!(t.i64(&format!("SELECT count(*) FROM documentos_empresas WHERE empresa_id={company} AND tipo_documento='RUT'")).await, 0);
});
parity!(test_vigencia_fija_la_fecha_de_vencimiento, |t| {
    let (_, company, _, _, session) = catalog(&t).await;
    let issued = today() - chrono::Duration::days(10);
    upload(&t, &session, "CAMARA_COMERCIO", issued, ("camara.pdf", PDF)).await;
    let due = t.text(&format!("SELECT fecha_vencimiento::text FROM documentos_empresas WHERE empresa_id={company} AND tipo_documento='CAMARA_COMERCIO'")).await;
    assert_eq!(due, (issued + chrono::Duration::days(30)).to_string());
});
parity!(test_bloqueo_del_admin_deshabilita_y_queda_registrado, |t| {
    let (user, company, admin, _, _) = catalog(&t).await;
    t.verify_documents(company).await;
    assert!(can_operate(&t, company).await);
    let url = format!("/api/empresas/{company}/bloqueo");
    assert_eq!(
        t.status(
            &admin,
            "POST",
            &url,
            Some(json!({"bloquear":true,"motivo":""}))
        )
        .await,
        400
    );
    assert!(
        !t.bool(&format!(
            "SELECT participacion_bloqueada FROM empresas WHERE id={company}"
        ))
        .await,
        "bloquear exige motivo"
    );
    t.ok(
        &admin,
        "POST",
        &url,
        Some(json!({"bloquear":true,"motivo":"Incumplimiento del convenio"})),
    )
    .await;
    assert!(
        t.bool(&format!(
            "SELECT participacion_bloqueada FROM empresas WHERE id={company}"
        ))
        .await
    );
    assert!(!can_operate(&t, company).await);
    assert!(
        !t.history("empresas", "empresa", company, "participacion_bloqueada")
            .await
            .is_empty()
    );
    assert!(t.notices(user, "EMPRESA_PARTICIPACION").await > 0);
    t.ok(&admin, "POST", &url, Some(json!({"bloquear":false})))
        .await;
    assert!(can_operate(&t, company).await);
});
parity!(
    test_admin_no_aprueba_reto_de_empresa_sin_documentacion,
    |t| {
        let (user, _, admin, _, _) = catalog(&t).await;
        let reto = t.reto(user, "en_revision", "reto").await;
        assert_eq!(
            t.status(
                &admin,
                "POST",
                &format!("/api/retos/{reto}/estado"),
                Some(json!({"estado":"aprobado","comentario":"x"}))
            )
            .await,
            400
        );
        assert_eq!(
            t.text(&format!("SELECT estado FROM retos WHERE id={reto}"))
                .await,
            "en_revision"
        );
    }
);
parity!(test_admin_configura_el_catalogo, |t| {
    let (_, _, admin, _, _) = catalog(&t).await;
    t.ok(&admin, "GET", "/api/records/tipos_documento_legal", None)
        .await;
    t.ok(&admin, "POST", "/api/tipos-documento", Some(json!({"codigo":"poliza","nombre":"Póliza de responsabilidad","descripcion":"","obligatorio":true,"aplica_sin_convenio":true,"activo":true,"orden":20}))).await;
    assert!(
        t.bool("SELECT obligatorio FROM tipos_documento_legal WHERE codigo='POLIZA'")
            .await
    );
    let (id, name, order): (i64, String, i16) = sqlx::query_as(
        "SELECT id,nombre,orden FROM tipos_documento_legal WHERE codigo='FICHA_RETO'",
    )
    .fetch_one(&t.pool)
    .await
    .unwrap();
    t.ok(&admin, "POST", &format!("/api/tipos-documento/{id}"), Some(json!({"nombre":name,"descripcion":"","aplica_sin_convenio":true,"activo":true,"orden":order}))).await;
    assert!(
        !t.bool(&format!(
            "SELECT obligatorio FROM tipos_documento_legal WHERE id={id}"
        ))
        .await
    );
});
parity!(test_empresa_no_accede_al_catalogo, |t| {
    let (_, _, _, _, session) = catalog(&t).await;
    assert_eq!(
        t.status(
            &session,
            "POST",
            "/api/tipos-documento",
            Some(json!({"codigo":"x","nombre":"x"}))
        )
        .await,
        403
    );
});

// VencimientoHistorialYSiembraTests
async fn expiring(t: &Ctx) -> (i64, i64, i64) {
    let (user, company) = t.company_user("empresa-venc", "900888888").await;
    t.verify_documents(company).await;
    let reto = t.reto(user, "en_revision", "reto").await;
    (user, company, reto)
}
async fn expire(t: &Ctx, company: i64, kind: &str) {
    t.exec(&format!("UPDATE documentos_empresas SET fecha_vencimiento=CURRENT_DATE-1 WHERE empresa_id={company} AND tipo_documento='{kind}'")).await;
}
async fn sweep(t: &Ctx) {
    retosean::reminders::run(&t.pool, &[1]).await.unwrap();
}
parity!(
    test_el_documento_vencido_se_marca_avisa_a_la_empresa_y_la_deja_sin_participar,
    |t| {
        let (user, company, _) = expiring(&t).await;
        assert!(can_operate(&t, company).await);
        expire(&t, company, "CAMARA_COMERCIO").await;
        sweep(&t).await;
        assert_eq!(t.text(&format!("SELECT estado FROM documentos_empresas WHERE empresa_id={company} AND tipo_documento='CAMARA_COMERCIO'")).await, "VENCIDO");
        let (kind, message): (String, String) = sqlx::query_as(&format!("SELECT tipo,mensaje FROM notificaciones WHERE usuario_id={user} AND evento='DOCUMENTO_REVISADO'")).fetch_one(&t.pool).await.unwrap();
        assert_eq!(kind, "ADVERTENCIA");
        assert!(message.to_lowercase().contains("vencido"));
        assert!(!can_operate(&t, company).await);
    }
);
parity!(test_repetir_el_comando_no_duplica_el_aviso, |t| {
    let (user, company, _) = expiring(&t).await;
    expire(&t, company, "CAMARA_COMERCIO").await;
    sweep(&t).await;
    sweep(&t).await;
    assert_eq!(t.notices(user, "DOCUMENTO_REVISADO").await, 1);
});
parity!(
    test_el_vencimiento_automatico_queda_en_el_historial_como_sistema,
    |t| {
        let (_, company, _) = expiring(&t).await;
        expire(&t, company, "CAMARA_COMERCIO").await;
        sweep(&t).await;
        let session = t.login("empresa-venc").await;
        let history = t
            .ok(
                &session,
                "GET",
                &format!("/api/empresas/{company}/historial-documental"),
                None,
            )
            .await;
        let change = history["historial"]
            .as_array()
            .unwrap()
            .iter()
            .find(|h| h["estado_nuevo"] == "VENCIDO")
            .cloned()
            .unwrap();
        assert_eq!(change["realizado_por"], "Sistema");
    }
);
parity!(
    test_la_revision_de_un_reto_muestra_el_historial_de_la_empresa,
    |t| {
        let (_, company, _) = expiring(&t).await;
        expire(&t, company, "CAMARA_COMERCIO").await;
        sweep(&t).await;
        let history = t
            .ok(
                &t.admin,
                "GET",
                &format!("/api/empresas/{company}/historial-documental"),
                None,
            )
            .await;
        assert!(history.to_string().contains("VENCIDO"));
    }
);
parity!(
    test_la_siembra_demo_es_idempotente_y_cubre_los_casos_de_hu00,
    |t| {
        // The Rust demo seeds one company; it can publish at once and a second run adds nothing.
        for _ in 0..2 {
            let mut tx = t.pool.begin().await.unwrap();
            retosean::migrate::demo_documents(&mut tx, &t.media, COMPANY)
                .await
                .unwrap();
            tx.commit().await.unwrap();
        }
        assert!(can_operate(&t, COMPANY).await);
        assert_eq!(
            t.i64(&format!(
                "SELECT count(*) FROM documentos_empresas WHERE empresa_id={COMPANY}"
            ))
            .await,
            8
        );
        let (_, other) = t.company_user("empresa-pendiente", "900888777").await;
        assert!(!can_operate(&t, other).await);
    }
);
parity!(
    test_un_documento_con_problemas_no_frena_el_resto_del_barrido,
    |t| {
        let (_, company, _) = expiring(&t).await;
        expire(&t, company, "CAMARA_COMERCIO").await;
        expire(&t, company, "RUT").await;
        // A simulated failure on one of the two documents, inside this test's schema.
        let rut = t.i64(&format!("SELECT id FROM documentos_empresas WHERE empresa_id={company} AND tipo_documento='RUT'")).await;
        t.exec(&format!("CREATE FUNCTION falla() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.id={rut} AND NEW.estado='VENCIDO' THEN RAISE EXCEPTION 'fallo simulado'; END IF; RETURN NEW; END $$; CREATE TRIGGER falla BEFORE UPDATE ON documentos_empresas FOR EACH ROW EXECUTE FUNCTION falla();")).await;
        sweep(&t).await;
        assert_eq!(t.i64(&format!("SELECT count(*) FROM documentos_empresas WHERE empresa_id={company} AND estado='VENCIDO'")).await, 1);
    }
);
