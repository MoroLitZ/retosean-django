//! apps/reportes/tests.py: report generation, access, exported fields, hostile
//! data, preview and sending by email (HU06).
mod common;
use calamine::{Data, Reader};
use common::*;
use serde_json::{Value, json};

const REPORTS: [&str; 5] = [
    "retos",
    "participacion",
    "evaluacion",
    "presupuesto",
    "empresas",
];

async fn generate(t: &Ctx, session: &Session, report: &str, format: &str) -> (u16, Vec<u8>) {
    let (code, bytes) = post_bytes(
        &t.app,
        session,
        "/api/reportes/generar",
        json!({"reporte":report,"formato":format,"periodo":"todo"}),
    )
    .await;
    (code.as_u16(), bytes)
}
async fn preview(t: &Ctx, report: &str, columns: Value) -> Value {
    t.ok(
        &t.admin,
        "POST",
        "/api/reportes/vista-previa",
        Some(json!({"reporte":report,"periodo":"todo","columnas":columns})),
    )
    .await
}
/// The value of `column` in the preview row that mentions `mark` (GeneracionDeReportes._valor).
async fn value(t: &Ctx, report: &str, mark: &str, column: &str) -> String {
    let data = preview(t, report, json!([])).await;
    let headers: Vec<&str> = data["encabezados"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h.as_str().unwrap())
        .collect();
    let index = headers
        .iter()
        .position(|h| *h == column)
        .unwrap_or_else(|| panic!("{report}: no column {column} in {headers:?}"));
    let row = data["filas"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r.to_string().contains(mark))
        .unwrap_or_else(|| panic!("{report}: no row with {mark}"));
    row[index].as_str().unwrap_or_default().to_string()
}

/// GeneracionDeReportesTests.setUp: an approved reto with an accepted student.
async fn setup(t: &Ctx) -> (i64, i64, i64) {
    t.exec("TRUNCATE retos CASCADE").await;
    let (company, _) = t.company_user("empresa-rep", "900300100").await;
    let student = t.user("est-rep", "ESTUDIANTE").await;
    let teacher = t.user("prof-rep", "PROFESOR").await;
    let reto = t.reto(company, "aprobado", "reto").await;
    t.accept(reto, student).await;
    (reto, student, teacher)
}
/// _sembrar_datos_ricos: data for every extended column of the five reports.
async fn rich(t: &Ctx, reto: i64, student: i64, teacher: i64) {
    let deliverable = t.deliverable(reto, student).await;
    t.exec(&format!("UPDATE entregables SET estado='CALIFICADO',nota=4.5,comentario_estudiante='Aqui esta mi avance',comentario_profesor='Buen trabajo' WHERE id={deliverable};
        INSERT INTO evaluacion_comentarioentregable(entregable_id,autor_id,comentario,creado_en) VALUES({deliverable},{teacher},'Recibido.',now());")).await;
    let rubric = t.i64(&format!("INSERT INTO evaluacion_rubrica(reto_id,nombre,descripcion,activa,creada_por_id,creada_en) VALUES({reto},'Rubrica 1','',true,{teacher},now()) RETURNING id")).await;
    t.exec(&format!("INSERT INTO evaluacion_criteriorubrica(rubrica_id,nombre,descripcion,puntaje_maximo,orden) VALUES({rubric},'Impacto','',5,1);
        INSERT INTO evaluaciones(entregable_id,profesor_id,nota,comentario,rubrica_id,fecha,actualizado_en) VALUES({deliverable},{teacher},4.5,'',{rubric},now(),now());")).await;
    let budget = t.i64(&format!("INSERT INTO presupuestos(reto_id,monto_total,alerta_80_enviada,creado_en,actualizado_en) VALUES({reto},1000,false,now(),now()) RETURNING id")).await;
    t.exec(&format!("INSERT INTO gastos(presupuesto_id,categoria,monto,estado,descripcion,creado_en) VALUES({budget},'MATERIALES',200,'COMPROMETIDO','',now()),({budget},'REFRIGERIOS',100,'EJECUTADO','',now())")).await;
    let company = t.i64("SELECT id FROM empresas WHERE nit='900300100'").await;
    t.exec(&format!("UPDATE empresas SET estado_validacion='VERIFICADA' WHERE id={company};
        INSERT INTO contactos_empresa(empresa_id,nombre,cargo,email,telefono,es_principal) VALUES({company},'Contacto Uno','Gerente','contacto@empresa.co','3000000000',true);
        INSERT INTO documentos_empresas(empresa_id,tipo_documento,estado,fecha_carga,motivo_rechazo) VALUES({company},'RUT','VERIFICADO',now(),'');")).await;
}

// GeneracionDeReportesTests
parity!(test_genera_pdf_y_registra_el_log, |t| {
    setup(&t).await;
    let (code, bytes) = generate(&t, &t.admin, "retos", "PDF").await;
    assert_eq!(code, 200);
    assert!(bytes.starts_with(b"%PDF"));
    let (user, report, total, file): (i64, String, i64, String) = sqlx::query_as("SELECT usuario_id,filtros->>'reporte',(filtros->>'total_registros')::bigint,archivo FROM reportes_generados").fetch_one(&t.pool).await.unwrap();
    assert_eq!((user, report.as_str(), total), (ADMIN, "retos", 1));
    assert!(file.ends_with(".pdf"));
});
parity!(test_genera_excel, |t| {
    setup(&t).await;
    let (code, bytes) = generate(&t, &t.admin, "participacion", "EXCEL").await;
    assert_eq!(code, 200);
    assert!(bytes.starts_with(b"PK"));
    assert!(
        t.text("SELECT archivo FROM reportes_generados")
            .await
            .ends_with(".xlsx")
    );
});
parity!(test_solo_incluye_las_columnas_elegidas, |t| {
    setup(&t).await;
    let data = preview(&t, "retos", json!(["titulo", "estado"])).await;
    assert_eq!(data["encabezados"].as_array().unwrap().len(), 2, "{data}");
});
parity!(test_sin_columnas_elegidas_se_incluyen_todas, |t| {
    setup(&t).await;
    let all = preview(&t, "retos", json!([])).await["encabezados"]
        .as_array()
        .unwrap()
        .len();
    let catalog = t.ok(&t.admin, "GET", "/api/reportes/catalogo", None).await;
    let retos = catalog["reportes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["clave"] == "retos" || r["key"] == "retos")
        .cloned()
        .unwrap();
    let declared = retos["columnas"].as_array().unwrap().len();
    assert_eq!(all, declared);
});
parity!(test_todos_los_reportes_del_catalogo_generan_pdf, |t| {
    setup(&t).await;
    for report in REPORTS {
        let (code, bytes) = generate(&t, &t.admin, report, "PDF").await;
        assert_eq!(code, 200, "{report}");
        assert!(bytes.starts_with(b"%PDF"), "{report}");
    }
});
parity!(
    test_todos_los_reportes_generan_excel_con_columnas_ampliadas,
    |t| {
        let (reto, student, teacher) = setup(&t).await;
        rich(&t, reto, student, teacher).await;
        for report in REPORTS {
            let (code, bytes) = generate(&t, &t.admin, report, "EXCEL").await;
            assert_eq!(code, 200, "{report}");
            assert!(bytes.starts_with(b"PK"), "{report}");
        }
    }
);

// AccesoAReportesTests
parity!(test_admin_abre_el_constructor, |t| {
    t.ok(&t.admin, "GET", "/api/reportes/catalogo", None).await;
});
parity!(test_estudiante_no_accede_a_reportes, |t| {
    assert_eq!(
        t.status(&t.student, "GET", "/api/reportes/catalogo", None)
            .await,
        403
    );
});
parity!(test_no_se_descarga_el_reporte_de_otro, |t| {
    let key = format!("{}.pdf", uuid::Uuid::new_v4().simple());
    std::fs::create_dir_all(&t.media).unwrap();
    std::fs::write(t.media.join(&key), b"%PDF-1.4 reporte").unwrap();
    let report = t.i64(&format!("INSERT INTO reportes_generados(usuario_id,formato,filtros,archivo,enviado_por_email,creado_en) VALUES({ADMIN},'PDF','{{}}','{key}',false,now()) RETURNING id")).await;
    let url = format!("/api/archivo/reportes_generados/{report}/archivo");
    assert_eq!(
        download(&t.app, &t.admin, &url).await.0,
        200,
        "the owner downloads it"
    );
    t.user("profe-acc", "PROFESOR").await;
    let other = t.login("profe-acc").await;
    assert!(!download(&t.app, &other, &url).await.0.is_success());
});
parity!(test_generar_desde_la_vista_devuelve_el_archivo, |t| {
    setup(&t).await;
    let (code, bytes) = generate(&t, &t.admin, "retos", "EXCEL").await;
    assert_eq!(code, 200);
    assert!(bytes.starts_with(b"PK"));
    assert_eq!(t.i64("SELECT count(*) FROM reportes_generados").await, 1);
});

// DatosNuevosEnLosReportesTests
async fn registered(t: &Ctx) -> i64 {
    let user = t.user("empresa-nuevos", "EMPRESA").await;
    t.exec(&format!("INSERT INTO empresas(nit,razon_social,sector_industrial,tipo_convocatoria,area_responsable,sitio_web,participacion_bloqueada,motivo_bloqueo,usuario_id) VALUES('900111000','Epsilon SAS','','AMBOS','Innovacion','https://epsilon.example',true,'Pendiente de revision juridica',{user})")).await;
    let reto = t.reto(user, "aprobado", "reto").await;
    t.exec(&format!("UPDATE retos SET titulo='Reto con registro completo',nombre_corto='RCRC',uso_esperado_resultados='Prototipo en produccion',insumos_inicio='Datos historicos',insumos_cierre='Informe final' WHERE id={reto}")).await;
    reto
}
parity!(
    test_empresas_exporta_registro_bloqueo_y_pendientes_documentales,
    |t| {
        registered(&t).await;
        let mark = "Epsilon SAS";
        assert_eq!(
            value(&t, "empresas", mark, "Tipo de convocatoria").await,
            "Reto y hackathon"
        );
        assert_eq!(
            value(&t, "empresas", mark, "Area responsable").await,
            "Innovacion"
        );
        assert_eq!(
            value(&t, "empresas", mark, "Sitio web").await,
            "https://epsilon.example"
        );
        assert_eq!(
            value(&t, "empresas", mark, "Participacion bloqueada").await,
            "Si"
        );
        assert_eq!(
            value(&t, "empresas", mark, "Motivo del bloqueo").await,
            "Pendiente de revision juridica"
        );
        assert_eq!(value(&t, "empresas", mark, "Puede participar").await, "No");
        assert!(
            value(&t, "empresas", mark, "Pendientes para participar")
                .await
                .contains("Falta cargarlo")
        );
    }
);
parity!(test_retos_exporta_registro_e_impacto_del_cierre, |t| {
    let reto = registered(&t).await;
    t.exec(&format!("INSERT INTO cierres_reto(reto_id,notas,impacto,encuesta_enviada,actualizado_en) VALUES({reto},'','SI',false,now())")).await;
    let mark = "Reto con registro completo";
    assert_eq!(value(&t, "retos", mark, "Nombre corto").await, "RCRC");
    assert_eq!(
        value(&t, "retos", mark, "Uso esperado de los resultados").await,
        "Prototipo en produccion"
    );
    assert_eq!(
        value(&t, "retos", mark, "Insumos al inicio").await,
        "Datos historicos"
    );
    assert_eq!(
        value(&t, "retos", mark, "Insumos al cierre").await,
        "Informe final"
    );
    assert_eq!(
        value(&t, "retos", mark, "La empresa aplico la solucion").await,
        "Sí, la aplicó"
    );
});
parity!(
    test_participacion_y_evaluacion_exportan_los_campos_nuevos,
    |t| {
        let reto = registered(&t).await;
        let student = t.user("est-nuevos", "ESTUDIANTE").await;
        t.exec(&format!("UPDATE usuarios SET first_name='Eva',last_name='Nueva' WHERE id={student};
        INSERT INTO postulaciones(reto_id,estudiante_id,estado,motivacion,semestre,programa,fecha_postulacion,habilidades,acepta_terminos) VALUES({reto},{student},'PENDIENTE','',5,'',now(),'',true);")).await;
        let deliverable = t.deliverable(reto, student).await;
        t.exec(&format!("UPDATE entregables SET titulo='Avance publicado',estado='CALIFICADO',nota=4,calificacion_publicada=true WHERE id={deliverable}")).await;
        assert_eq!(
            value(&t, "participacion", "Eva Nueva", "Acepto los terminos").await,
            "Si"
        );
        assert_eq!(
            value(&t, "participacion", "Eva Nueva", "Adjunto hoja de vida").await,
            "No"
        );
        assert_eq!(
            value(
                &t,
                "evaluacion",
                "Avance publicado",
                "Calificacion publicada al estudiante"
            )
            .await,
            "Si"
        );
    }
);

// ReportesConDatosHostilesTests
parity!(test_todos_los_reportes_se_generan_en_pdf_y_excel, |t| {
    let company = t.user("empresa-hostil-rep", "EMPRESA").await;
    let reto = t.reto(company, "aprobado", "reto").await;
    sqlx::query("UPDATE retos SET titulo='Reto <b & =cmd',descripcion=$2 WHERE id=$1")
        .bind(reto)
        .bind("con\u{1}control y <i")
        .execute(&t.pool)
        .await
        .unwrap();
    let budget = t.i64(&format!("INSERT INTO presupuestos(reto_id,monto_total,alerta_80_enviada,creado_en,actualizado_en) VALUES({reto},100,false,now(),now()) RETURNING id")).await;
    // A category no longer in the catalog (old data or loaded from the admin).
    t.exec(&format!("INSERT INTO gastos(presupuesto_id,categoria,monto,estado,descripcion,creado_en) VALUES({budget},'VIEJA',10,'EJECUTADO','',now())")).await;
    for report in REPORTS {
        for (format, signature) in [("PDF", &b"%PDF"[..]), ("EXCEL", &b"PK"[..])] {
            let (code, bytes) = generate(&t, &t.admin, report, format).await;
            assert_eq!(code, 200, "{report} {format}");
            assert!(bytes.starts_with(signature), "{report} {format}");
        }
    }
});

// VistaPreviaYEnvioHU06Tests
async fn three(t: &Ctx) {
    let company = t.user("emp-hu06", "EMPRESA").await;
    t.exec("TRUNCATE retos CASCADE").await;
    for i in 0..3 {
        let reto = t.reto(company, "aprobado", "reto").await;
        t.exec(&format!(
            "UPDATE retos SET titulo='Reto previo {i}' WHERE id={reto}"
        ))
        .await;
    }
}
async fn send(t: &Ctx, to: &str) -> (u16, Value) {
    let (code, body) = t
        .req(
            &t.admin,
            "POST",
            "/api/reportes/enviar",
            Some(json!({"reporte":"retos","formato":"PDF","periodo":"todo","destinatarios":to})),
        )
        .await;
    (code.as_u16(), body)
}
parity!(test_vista_previa_muestra_filas_sin_generar_archivo, |t| {
    three(&t).await;
    let data = preview(&t, "retos", json!([])).await;
    assert_eq!(data["total"], 3);
    assert_eq!(data["filas"].as_array().unwrap().len(), 3);
    assert!(data.to_string().contains("Reto previo 0"));
    assert_eq!(t.i64("SELECT count(*) FROM reportes_generados").await, 0);
});
parity!(
    test_enviar_por_correo_adjunta_el_archivo_y_lo_registra,
    |t| {
        three(&t).await;
        let (code, body) = send(&t, "jefe@ean.edu.co; otra@ean.edu.co").await;
        assert_eq!(code, 200, "{body}");
        let mails = mails_to("jefe@ean.edu.co");
        assert!(
            !mails.is_empty() && mails[0].2.contains("[adjunto: ") && mails[0].2.contains(".pdf]"),
            "{mails:?}"
        );
        assert!(!mails_to("otra@ean.edu.co").is_empty());
        let (sent, to): (bool, Value) =
            sqlx::query_as("SELECT enviado_por_email,filtros->'enviado_a' FROM reportes_generados")
                .fetch_one(&t.pool)
                .await
                .unwrap();
        assert!(sent);
        assert_eq!(to, json!(["jefe@ean.edu.co", "otra@ean.edu.co"]));
    }
);
parity!(test_sin_destinatarios_se_envia_al_propio_usuario, |t| {
    three(&t).await;
    t.exec("UPDATE usuarios SET email='admin-hu06@ean.edu.co' WHERE id=1")
        .await;
    let admin = t.login("admin.demo").await;
    let (code, _) = t
        .req(
            &admin,
            "POST",
            "/api/reportes/enviar",
            Some(json!({"reporte":"retos","formato":"PDF","periodo":"todo"})),
        )
        .await;
    assert_eq!(code, 200);
    assert!(!mails_to("admin-hu06@ean.edu.co").is_empty());
});
parity!(test_correo_invalido_no_genera_nada, |t| {
    three(&t).await;
    let (code, _) = send(&t, "no-es-correo").await;
    assert_eq!(code, 400);
    assert_eq!(t.i64("SELECT count(*) FROM reportes_generados").await, 0);
    assert!(mails_to("no-es-correo").is_empty());
});
parity!(test_fallo_de_envio_conserva_el_reporte_y_lo_marca, |t| {
    three(&t).await;
    send(&t, "jefe@smtp-caido.invalid").await;
    let (sent, error): (bool, String) =
        sqlx::query_as("SELECT enviado_por_email,filtros->>'error_envio' FROM reportes_generados")
            .fetch_one(&t.pool)
            .await
            .unwrap();
    assert!(!sent);
    assert!(error.contains("SMTP caido"), "{error}");
    let history = t
        .ok(&t.admin, "GET", "/api/records/reportes_generados", None)
        .await;
    assert!(history.to_string().contains("SMTP caido"));
});

// ReportesDelProfesorTests (F3): a professor's participacion/evaluacion reports
// only cover retos with their approved/published integration; admins see all.
async fn two_retos(t: &Ctx) -> (i64, i64, Session) {
    let own = t.reto(COMPANY_USER, "aprobado", "reto").await;
    t.exec(&format!(
        "UPDATE retos SET titulo='Reto del profesor' WHERE id={own}"
    ))
    .await;
    let other = t.reto(COMPANY_USER, "aprobado", "reto").await;
    t.exec(&format!(
        "UPDATE retos SET titulo='Reto ajeno' WHERE id={other}"
    ))
    .await;
    let teacher = t.user("prof-scope", "PROFESOR").await;
    t.integrate(own, teacher).await;
    // A second integration still in review must not count (F2 states).
    let draft = t.integrate(other, teacher).await;
    t.exec(&format!(
        "UPDATE integraciones_academicas SET estado='en_revision' WHERE id={draft}"
    ))
    .await;
    t.accept(own, STUDENT).await;
    t.accept(other, STUDENT).await;
    t.deliverable(own, STUDENT).await;
    t.deliverable(other, STUDENT).await;
    (own, other, t.login("prof-scope").await)
}
parity!(
    test_profesor_solo_reporta_los_retos_de_sus_integraciones,
    |t| {
        let (_, _, session) = two_retos(&t).await;
        let data = t
            .ok(
                &session,
                "POST",
                "/api/reportes/vista-previa",
                Some(json!({"reporte":"participacion","periodo":"todo"})),
            )
            .await;
        assert_eq!(data["total"], 1, "{data}");
        assert!(data.to_string().contains("Reto del profesor"));
        assert!(!data.to_string().contains("Reto ajeno"));
        let data = t
            .ok(
                &session,
                "POST",
                "/api/reportes/vista-previa",
                Some(json!({"reporte":"evaluacion","periodo":"todo"})),
            )
            .await;
        assert_eq!(data["total"], 1, "{data}");
        assert!(!data.to_string().contains("Reto ajeno"), "{data}");
        // The admin keeps the global view.
        let data = preview(&t, "participacion", json!([])).await;
        assert!(data["total"].as_i64().unwrap() >= 2, "{data}");
    }
);
parity!(test_profesor_genera_y_envia_el_reporte_acotado, |t| {
    let (_, _, session) = two_retos(&t).await;
    let (code, bytes) = post_bytes(
        &t.app,
        &session,
        "/api/reportes/generar",
        json!({"reporte":"participacion","formato":"EXCEL","periodo":"todo"}),
    )
    .await;
    assert_eq!(code, 200);
    assert!(bytes.starts_with(b"PK"));
    let (user, total): (i64, i64) = sqlx::query_as("SELECT usuario_id,(filtros->>'total_registros')::bigint FROM reportes_generados ORDER BY id DESC LIMIT 1")
        .fetch_one(&t.pool)
        .await
        .unwrap();
    let teacher = t
        .i64("SELECT id FROM usuarios WHERE username='prof-scope'")
        .await;
    assert_eq!((user, total), (teacher, 1));
    let (code, body) = t
        .req(
            &session,
            "POST",
            "/api/reportes/enviar",
            Some(json!({"reporte":"evaluacion","formato":"PDF","periodo":"todo"})),
        )
        .await;
    assert_eq!(code, 200, "{body}");
});

// LimiteDeFilasExcelTests
/// True if any cell of the first worksheet of an xlsx contains `needle`.
fn xlsx_has(bytes: Vec<u8>, needle: &str) -> bool {
    let mut book: calamine::Xlsx<_> =
        calamine::open_workbook_from_rs(std::io::Cursor::new(bytes)).unwrap();
    let sheet = book.sheet_names()[0].clone();
    book.worksheet_range(&sheet)
        .unwrap()
        .cells()
        .any(|(_, _, cell)| matches!(cell, Data::String(s) if s.contains(needle)))
}
parity!(test_excel_trunca_y_avisa_cuando_supera_el_tope, |t| {
    // 10010 retos: the Excel export must stop at the cap and flag the cut.
    t.exec("TRUNCATE retos CASCADE").await;
    t.exec(&format!(
        "INSERT INTO retos(empresa_id,titulo) SELECT {COMPANY_USER},'Reto '||g FROM generate_series(1,10010) g"
    ))
    .await;
    let (code, bytes) = generate(&t, &t.admin, "retos", "EXCEL").await;
    assert_eq!(code, 200);
    assert!(bytes.starts_with(b"PK"));
    let total: i64 = t
        .i64("SELECT (filtros->>'total_registros')::bigint FROM reportes_generados ORDER BY id DESC LIMIT 1")
        .await;
    assert_eq!(total, 10000);
    assert!(xlsx_has(bytes, "truncado a 10000"));
});

// FiltrosDelReporteDeEmpresasTests
parity!(test_respeta_el_periodo_por_fecha_de_registro, |t| {
    t.company_user("emp-filt", "9002").await; // fecha_creacion = today
    t.exec("INSERT INTO empresas(nit,razon_social,sector_industrial,usuario_id,estado_validacion,estado_listas_restrictivas,estado_convenio,renuncio_a_convenio,participacion_bloqueada,motivo_bloqueo,tipo_convocatoria,area_responsable,sitio_web) VALUES('9003','Beta','',NULL,'PENDIENTE','PENDIENTE','NO_RADICADO',false,false,'','RETO','','')")
        .await;
    t.exec("UPDATE empresas SET fecha_creacion='2020-01-01' WHERE nit='9003'")
        .await;
    let data = t
        .ok(
            &t.admin,
            "POST",
            "/api/reportes/vista-previa",
            Some(json!({"reporte":"empresas","periodo":"custom","desde":"2025-01-01"})),
        )
        .await;
    assert!(data.to_string().contains("emp-filt S.A.S."), "{data}");
    assert!(!data.to_string().contains("Beta"), "{data}");
});
parity!(test_respeta_facultad_a_traves_de_sus_retos, |t| {
    let (user, _) = t.company_user("emp-filt2", "9004").await;
    let other = t.user("emp-filt2-b", "EMPRESA").await;
    t.exec(&format!(
        "INSERT INTO empresas(nit,razon_social,sector_industrial,usuario_id,estado_validacion,estado_listas_restrictivas,estado_convenio,renuncio_a_convenio,participacion_bloqueada,motivo_bloqueo,tipo_convocatoria,area_responsable,sitio_web) VALUES('9005','Beta','',{other},'PENDIENTE','PENDIENTE','NO_RADICADO',false,false,'','RETO','','')"
    ))
    .await;
    // The company's reto belongs to FACULTY; Beta has none, so it drops out.
    let reto = t.reto(user, "aprobado", "reto").await;
    t.exec(&format!(
        "UPDATE retos SET facultad_id={FACULTY} WHERE id={reto}"
    ))
    .await;
    let data = t
        .ok(
            &t.admin,
            "POST",
            "/api/reportes/vista-previa",
            Some(json!({"reporte":"empresas","periodo":"todo","facultad":FACULTY})),
        )
        .await;
    assert!(data.to_string().contains("emp-filt2 S.A.S."), "{data}");
    assert!(!data.to_string().contains("Beta"), "{data}");
});
