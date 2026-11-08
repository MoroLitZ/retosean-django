//! Regression tests for Package E (academic, study units, lifecycle, closure).
mod common;
use common::*;
use serde_json::{Value, json};

const PDF: &[u8] = b"%PDF-1.4 documento";
const TXT: &[u8] = b"texto plano";

async fn load_csv(t: &Ctx, session: &Session, header: &str, rows: &[&str]) -> (u16, Value) {
    let csv = format!("{header}\n{}", rows.join("\n"));
    let (code, body) = multipart(
        &t.app,
        session,
        "/api/unidades",
        &[("accion", "cargar_csv")],
        &[("archivo_csv", "unidades.csv", csv.as_bytes())],
    )
    .await;
    (code.as_u16(), body)
}
const CSV_HEADER: &str = "codigo,nombre,programa,periodo,ciclo";

// ACA-1: CSV bulk load enforces faculty scope per row.
parity!(test_csv_rechaza_programa_de_otra_facultad, |t| {
    let negocios = t
        .i64("INSERT INTO facultades(nombre,codigo) VALUES('Negocios','NEG') RETURNING id")
        .await;
    let _ajeno = t
        .i64(&format!("INSERT INTO programas(nombre,facultad_id,nivel) VALUES('Administracion de Empresas',{negocios},'') RETURNING id"))
        .await;
    t.user("profe-ing", "PROFESOR").await;
    let session = t.login("profe-ing").await;
    let (code, body) = load_csv(
        &t,
        &session,
        CSV_HEADER,
        &["ADM-101,Fundamentos,Administracion de Empresas,2026-2,1"],
    )
    .await;
    assert_eq!(code, 200, "{body}");
    assert_eq!(t.i64("SELECT count(*) FROM unidades_estudio").await, 0);
    assert!(
        body["mensaje"]
            .as_str()
            .unwrap()
            .contains("no tienes permiso"),
        "{body}"
    );
});

// ACA-2: editing an integration whose reto was soft-deleted returns 404.
parity!(test_editar_integracion_reto_eliminado, |t| {
    let reto = t.reto(COMPANY_USER, "aprobado", "reto").await;
    let teacher = t.user("profe-aca", "PROFESOR").await;
    let integration = t.integrate(reto, teacher).await;
    t.exec(&format!(
        "UPDATE retos SET eliminado_en=now() WHERE id={reto}"
    ))
    .await;
    let session = t.login("profe-aca").await;
    let (code, _) = t
        .req(
            &session,
            "POST",
            &format!("/api/integraciones/{integration}"),
            Some(json!({"descripcion":"Nueva"})),
        )
        .await;
    assert!(code == 404 || code == 403, "{code}");
});

// ACA-3: approved/published integrations cannot be edited, even when the reto is
// open or running (the old bypass only checked the reto state).
parity!(test_integracion_aprobada_no_editable, |t| {
    let reto = t.reto(COMPANY_USER, "abierto", "reto").await;
    let teacher = t.user("profe-aca2", "PROFESOR").await;
    let integration = t.integrate(reto, teacher).await;
    t.exec(&format!(
        "UPDATE integraciones_academicas SET estado='aprobada' WHERE id={integration}"
    ))
    .await;
    let session = t.login("profe-aca2").await;
    let (code, body) = t
        .req(
            &session,
            "POST",
            &format!("/api/integraciones/{integration}"),
            Some(json!({"descripcion":"Cambio"})),
        )
        .await;
    assert_eq!(code, 400, "{body}");
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("no puede editarse"),
        "{body}"
    );
});

// ACA-4: progress entries blocked in terminal reto states.
parity!(test_avance_bloqueado_en_reto_finalizado, |t| {
    let reto = t.reto(COMPANY_USER, "finalizado", "reto").await;
    let teacher = t.user("profe-aca3", "PROFESOR").await;
    let _ = t.integrate(reto, teacher).await;
    let session = t.login("profe-aca3").await;
    let (code, body) = t
        .req(
            &session,
            "POST",
            &format!("/api/retos/{reto}/seguimiento"),
            Some(json!({"tipo_sesion":"seguimiento","porcentaje_avance":10,"avances":"x"})),
        )
        .await;
    assert!(code == 404 || code == 403, "{body}");
});

// ACA-5/ACA-6/ACA-7: closure save blocked after finalization.
parity!(test_cierre_guardar_bloqueado_despues_de_finalizar, |t| {
    let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
    let teacher = t.user("profe-cierre", "PROFESOR").await;
    t.integrate(reto, teacher).await;
    let session = t.login("profe-cierre").await;
    t.exec(&format!(
        "UPDATE retos SET estado='finalizado' WHERE id={reto}"
    ))
    .await;
    let (code, body) = multipart(
        &t.app,
        &session,
        &format!("/api/retos/{reto}/cierre"),
        &[("agenda", "nueva")],
        &[],
    )
    .await;
    assert_eq!(code, 400, "{body}");
    assert!(
        body["error"].as_str().unwrap().contains("finalizado"),
        "{body}"
    );
});

// ACA-7: closure artifacts/recongnitions/requests blocked after finalization.
parity!(test_cierre_entregable_bloqueado_despues_de_finalizar, |t| {
    let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
    t.exec(&format!(
        "UPDATE retos SET estado='finalizado' WHERE id={reto}"
    ))
    .await;
    let (code, body) = multipart(
        &t.app,
        &t.admin,
        &format!("/api/retos/{reto}/cierre/entregables"),
        &[("nombre", "Nuevo final")],
        &[("archivo", "final.pdf", PDF)],
    )
    .await;
    assert_eq!(code, 400, "{body}");
    assert!(
        body["error"].as_str().unwrap().contains("finalizado"),
        "{body}"
    );
});

parity!(test_cierre_logistica_bloqueada_despues_de_finalizar, |t| {
    let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
    t.exec(&format!(
        "UPDATE retos SET estado='finalizado' WHERE id={reto}"
    ))
    .await;
    let (code, body) = t
        .req(
            &t.admin,
            "POST",
            &format!("/api/retos/{reto}/solicitudes-logistica"),
            Some(json!({"evento":"CIERRE","tipo":"OTRO","descripcion":"x"})),
        )
        .await;
    assert_eq!(code, 400, "{body}");
    assert!(
        body["error"].as_str().unwrap().contains("finalizado"),
        "{body}"
    );
});

// ACA-9: inactive study units cannot be edited.
parity!(test_unidad_inactiva_no_editable, |t| {
    let program = t
        .i64("INSERT INTO programas(nombre,facultad_id,nivel) VALUES('Ingenieria de Sistemas',1,'') RETURNING id")
        .await;
    let id = t
        .i64(&format!("INSERT INTO unidades_estudio(codigo,nombre,programa_id,periodo,ciclo,activo,creado_en,actualizado_en) VALUES('UNI-INA','Inactiva',{program},'2026-2','1',false,now(),now()) RETURNING id"))
        .await;
    t.user("profe-ina", "PROFESOR").await;
    let session = t.login("profe-ina").await;
    let (code, body) = multipart(
        &t.app,
        &session,
        &format!("/api/unidades/{id}"),
        &[
            ("codigo", "UNI-INA2"),
            ("nombre", "Inactiva2"),
            ("programa", program.to_string().as_str()),
            ("periodo", "2026-2"),
            ("ciclo", "1"),
        ],
        &[],
    )
    .await;
    assert_eq!(code, 400, "{body}");
    assert!(
        body["error"].as_str().unwrap().contains("inactiva"),
        "{body}"
    );
});

// ACA-10: duplicate code in CSV is still rejected (regression for advisory lock path).
parity!(test_csv_rechaza_codigo_duplicado, |t| {
    t.i64("INSERT INTO programas(nombre,facultad_id,nivel) VALUES('Ingenieria de Sistemas',1,'') RETURNING id").await;
    let (code, body) = load_csv(
        &t,
        &t.admin,
        CSV_HEADER,
        &[
            "DUP-100,Una,Ingenieria de Sistemas,2026-2,1",
            "DUP-100,Otra,Ingenieria de Sistemas,2026-2,1",
        ],
    )
    .await;
    assert_eq!(code, 200, "{body}");
    assert_eq!(
        t.i64("SELECT count(*) FROM unidades_estudio WHERE codigo='DUP-100'")
            .await,
        1
    );
});

// ACA-12: closure artifacts reject disallowed file types.
parity!(test_cierre_entregable_rechaza_txt, |t| {
    let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
    let (code, body) = multipart(
        &t.app,
        &t.admin,
        &format!("/api/retos/{reto}/cierre/entregables"),
        &[("nombre", "No PDF")],
        &[("archivo", "final.txt", TXT)],
    )
    .await;
    assert_eq!(code, 400, "{body}");
    assert!(
        body["error"].as_str().unwrap().contains("no permitido"),
        "{body}"
    );
});

// ACA-13: logistics state change rejects oversized comment.
parity!(test_estado_solicitud_comentario_demasiado_largo, |t| {
    let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
    let id = t
        .i64(&format!("INSERT INTO cierre_solicitudlogistica(reto_id,evento,tipo,descripcion,estado,solicitada_por_id,creada_en,actualizado_en) VALUES({reto},'CIERRE','OTRO','x','SOLICITADA',{ADMIN},now(),now()) RETURNING id"))
        .await;
    let comentario = "x".repeat(4001);
    let (code, body) = t
        .req(
            &t.admin,
            "POST",
            &format!("/api/solicitudes-logistica/{id}/estado"),
            Some(json!({"estado":"CONFIRMADA","comentario":comentario})),
        )
        .await;
    assert_eq!(code, 400, "{body}");
    assert!(body["error"].as_str().unwrap().contains("máximo"), "{body}");
});

// CIE-03 in the UI: /api/me says whether the user may manage study units, so a
// professor without a faculty is not offered actions the server would refuse.
parity!(test_me_indica_si_gestiona_unidades, |t| {
    let teacher = t.user("profe-sin-facultad", "PROFESOR").await;
    let session = t.login("profe-sin-facultad").await;
    assert_eq!(
        t.ok(&session, "GET", "/api/me", None).await["gestiona_unidades"],
        true
    );
    t.exec(&format!(
        "UPDATE profesores SET facultad_id=NULL WHERE usuario_id={teacher}"
    ))
    .await;
    assert_eq!(
        t.ok(&session, "GET", "/api/me", None).await["gestiona_unidades"],
        false
    );
    assert_eq!(
        t.ok(&t.admin, "GET", "/api/me", None).await["gestiona_unidades"],
        true
    );
    t.user("est-unidades", "ESTUDIANTE").await;
    let student = t.login("est-unidades").await;
    assert_eq!(
        t.ok(&student, "GET", "/api/me", None).await["gestiona_unidades"],
        false
    );
});
