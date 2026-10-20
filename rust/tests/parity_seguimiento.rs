//! apps/seguimiento/tests.py: state machines and the shared state history.
mod common;
use common::*;
use retosean::domain;
use serde_json::json;

const PDF: &[u8] = b"%PDF-1.4 documento";

// MaquinaEstadosTests
#[test]
fn test_transicion_valida() {
    assert!(domain::transition("MAQUINA_RETO", "borrador", "en_revision").is_ok());
}
#[test]
fn test_transicion_invalida_explica_el_motivo_en_espanol() {
    let error = domain::transition("MAQUINA_DOCUMENTO", "RECHAZADO", "VERIFICADO").unwrap_err();
    assert!(
        error.1.contains(r#"de "Rechazado" a "Aprobado""#),
        "{}",
        error.1
    );
}
#[test]
fn test_estado_sin_destinos_es_final() {
    assert!(domain::machines()["MAQUINA_RETO"]["finalizado"].is_empty());
    assert!(domain::transition("MAQUINA_RETO", "finalizado", "abierto").is_err());
}

// HistorialEstadoTests
parity!(test_cambiar_estado_invalido_no_modifica_ni_registra, |t| {
    let reto = t.reto(COMPANY_USER, "abierto", "reto").await;
    let application = t.i64(&format!("INSERT INTO postulaciones(reto_id,estudiante_id,estado,motivacion,semestre,programa,fecha_postulacion,habilidades,acepta_terminos) VALUES({reto},{STUDENT},'RECHAZADA','x',5,'',now(),'',true) RETURNING id")).await;
    let (code, body) = t
        .req(
            &t.company,
            "POST",
            &format!("/api/postulaciones/{application}/estado"),
            Some(json!({"estado":"ACEPTADA"})),
        )
        .await;
    assert_eq!(code, 400, "{body}");
    assert_eq!(
        t.text(&format!(
            "SELECT estado FROM postulaciones WHERE id={application}"
        ))
        .await,
        "RECHAZADA"
    );
    assert!(
        t.history("participaciones", "postulacion", application, "estado")
            .await
            .is_empty()
    );
});
parity!(test_ciclo_del_documento_queda_registrado, |t| {
    let today = chrono::Utc::now().date_naive().to_string();
    let fields = [("fecha_expedicion", today.as_str())];
    let (code, body) = multipart(
        &t.app,
        &t.company,
        "/api/empresa/documentos/RUT/cargar",
        &fields,
        &[("archivo", "rut.pdf", PDF)],
    )
    .await;
    assert_eq!(code, 200, "{body}");
    let document = body["id"].as_i64().unwrap();
    t.ok(
        &t.admin,
        "POST",
        &format!("/api/documentos/{document}/revisar"),
        Some(json!({"estado":"RECHAZADO","comentario":"Ilegible"})),
    )
    .await;
    let (code, body) = multipart(
        &t.app,
        &t.company,
        "/api/empresa/documentos/RUT/cargar",
        &fields,
        &[("archivo", "rut2.pdf", PDF)],
    )
    .await;
    assert_eq!(code, 200, "{body}");
    let steps = t
        .history("empresas", "documentoempresa", document, "estado")
        .await;
    let expected = [
        ("", "CARGADO", "empresa.verificada"),
        ("CARGADO", "RECHAZADO", "admin.demo"),
        ("RECHAZADO", "CARGADO", "empresa.verificada"),
    ];
    assert_eq!(steps.len(), 3, "{steps:?}");
    for (step, (from, to, who)) in steps.iter().zip(expected) {
        assert_eq!(
            (step.0.as_str(), step.1.as_str(), step.2.as_str()),
            (from, to, who)
        );
    }
});
parity!(test_no_se_puede_verificar_un_documento_ya_rechazado, |t| {
    let document = t.i64(&format!("INSERT INTO documentos_empresas(empresa_id,tipo_documento,archivo,estado,fecha_carga,motivo_rechazo) VALUES({COMPANY},'RUT','x.pdf','RECHAZADO',now(),'') RETURNING id")).await;
    assert_eq!(
        t.status(
            &t.admin,
            "POST",
            &format!("/api/documentos/{document}/revisar"),
            Some(json!({"estado":"VERIFICADO"}))
        )
        .await,
        400
    );
    assert_eq!(
        t.text(&format!(
            "SELECT estado FROM documentos_empresas WHERE id={document}"
        ))
        .await,
        "RECHAZADO"
    );
});
parity!(test_ciclo_de_la_postulacion_queda_registrado, |t| {
    let reto = t.reto(COMPANY_USER, "abierto", "reto").await;
    let student = t.user("est-hist", "ESTUDIANTE").await;
    let session = t.login("est-hist").await;
    let id = t.ok(&session, "POST", &format!("/api/retos/{reto}/postular"), Some(json!({"motivacion":"Me interesa","habilidades":"Python","semestre":5,"programa":"Ingeniería de Sistemas","acepta_terminos":true}))).await["id"].as_i64().unwrap();
    let _ = student;
    t.ok(
        &session,
        "POST",
        &format!("/api/postulaciones/{id}/estado"),
        Some(json!({"estado":"RETIRADA"})),
    )
    .await;
    let steps: Vec<(String, String)> = t
        .history("participaciones", "postulacion", id, "estado")
        .await
        .into_iter()
        .map(|s| (s.0, s.1))
        .collect();
    assert_eq!(
        steps,
        [
            ("".into(), "PENDIENTE".into()),
            ("PENDIENTE".into(), "RETIRADA".into())
        ]
    );
});
parity!(test_calificar_entregable_queda_registrado, |t| {
    let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
    t.integrate(reto, TEACHER).await;
    let deliverable = t.deliverable(reto, STUDENT).await;
    t.ok(&t.teacher, "GET", "/api/records/entregables", None)
        .await;
    t.ok(
        &t.teacher,
        "POST",
        &format!("/api/entregables/{deliverable}/calificar"),
        Some(json!({"nota":"4","comentario":""})),
    )
    .await;
    let steps: Vec<(String, String)> = t
        .history("evaluacion", "entregable", deliverable, "estado")
        .await
        .into_iter()
        .map(|s| (s.0, s.1))
        .collect();
    assert_eq!(
        steps,
        [
            ("ENVIADO".into(), "EN_REVISION".into()),
            ("EN_REVISION".into(), "CALIFICADO".into())
        ]
    );
});
parity!(test_inactivar_usuario_queda_registrado, |t| {
    let student = t.user("est-inactivo", "ESTUDIANTE").await;
    t.ok(
        &t.admin,
        "POST",
        &format!("/api/usuarios/{student}/estado"),
        Some(json!({"activo":false})),
    )
    .await;
    assert!(
        !t.bool(&format!(
            "SELECT is_active FROM usuarios WHERE id={student}"
        ))
        .await
    );
    let steps = t.history("usuarios", "usuario", student, "is_active").await;
    assert_eq!(steps.len(), 1);
    assert_eq!(
        (
            steps[0].0.as_str(),
            steps[0].1.as_str(),
            steps[0].2.as_str()
        ),
        ("ACTIVO", "INACTIVO", "admin.demo")
    );
});
parity!(test_integracion_no_se_reenvia_si_ya_esta_aprobada, |t| {
    let reto = t.reto(COMPANY_USER, "aprobado", "reto").await;
    let integration = t.integrate(reto, TEACHER).await;
    t.exec(&format!(
        "UPDATE integraciones_academicas SET estado='aprobada' WHERE id={integration}"
    ))
    .await;
    let code = t
        .status(
            &t.teacher,
            "POST",
            &format!("/api/integraciones/{integration}/estado"),
            Some(json!({"estado":"en_revision"})),
        )
        .await;
    assert_ne!(code, 200);
    assert_eq!(
        t.text(&format!(
            "SELECT estado FROM integraciones_academicas WHERE id={integration}"
        ))
        .await,
        "aprobada"
    );
    assert!(
        t.history("seguimiento", "integracionacademica", integration, "estado")
            .await
            .iter()
            .all(|s| s.1 != "en_revision")
    );
});
