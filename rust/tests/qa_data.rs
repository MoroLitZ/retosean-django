//! Regression tests for the QA fix package B: uploads, import, report builder,
//! reports, resources, dashboard.
mod common;
use common::*;
use serde_json::{Value, json};
use tower::ServiceExt;

/// Build a multipart body with text fields and a file.
fn multipart_body(fields: &[(&str, &str)], files: &[(&str, &str, &[u8])]) -> Vec<u8> {
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend(
            format!("--FRM\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n")
                .into_bytes(),
        );
    }
    for (name, filename, bytes) in files {
        body.extend(format!("--FRM\r\nContent-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\nContent-Type: application/octet-stream\r\n\r\n").into_bytes());
        body.extend(*bytes);
        body.extend(b"\r\n");
    }
    body.extend(b"--FRM--\r\n");
    body
}

/// POST multipart to a path and return (status, json).
async fn post_multipart(
    app: &axum::Router,
    session: &Session,
    path: &str,
    body: Vec<u8>,
) -> (u16, Value) {
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("cookie", &session.cookie)
                .header("x-csrf-token", &session.csrf)
                .header("content-type", "multipart/form-data; boundary=FRM")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status.as_u16(),
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

// MED-2: bulk import cannot create ADMIN rows unless the importer is superuser.
parity!(test_import_admin_requiere_superusuario, |t| {
    // admin.demo is a superuser, so use a plain (non-superuser) ADMIN account.
    t.user("admin-import", "ADMIN").await;
    let admin = t.login("admin-import").await;
    let body = multipart_body(
        &[("accion", "previsualizar")],
        &[(
            "archivo",
            "admins.csv",
            b"username,email,rol\nadmin1,a@x.co,ADMIN\n",
        )],
    );
    let (status, result) = post_multipart(&t.app, &admin, "/api/usuarios/importar", body).await;
    assert_eq!(status, 200, "{result}");
    let invalidas = result["invalidas"].as_array().expect("invalidas present");
    assert!(
        invalidas[0]["errores"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e.as_str().unwrap().contains("superusuario")),
        "non-superuser admin must not import ADMIN: {result}"
    );
    let username = invalidas[0]["datos"]["username"].as_str().unwrap();
    assert_eq!(username, "admin1");
});

// MED-1: xlsx import with more than MAX_FILAS rows is rejected early.
parity!(test_import_xlsx_rechaza_exceso_de_filas, |t| {
    use rust_xlsxwriter::Workbook;
    let mut book = Workbook::new();
    let sheet = book.add_worksheet();
    sheet.write_string(0, 0, "username").unwrap();
    sheet.write_string(0, 1, "email").unwrap();
    sheet.write_string(0, 2, "rol").unwrap();
    for i in 1..=1002 {
        sheet.write_string(i as u32, 0, format!("u{i}")).unwrap();
        sheet
            .write_string(i as u32, 1, format!("u{i}@x.co"))
            .unwrap();
        sheet.write_string(i as u32, 2, "ESTUDIANTE").unwrap();
    }
    let bytes = book.save_to_buffer().unwrap();
    let body = multipart_body(
        &[("accion", "previsualizar")],
        &[("archivo", "muchos.xlsx", &bytes)],
    );
    let (status, result) = post_multipart(&t.app, &t.admin, "/api/usuarios/importar", body).await;
    assert_eq!(status, 200, "{result}");
    assert!(
        result["error_global"]
            .as_str()
            .unwrap_or("")
            .contains("1002 filas"),
        "{result}"
    );
});

// MED-3: per-file upload over the 50 MB limit is rejected while streaming.
parity!(test_adjunto_reto_rechaza_archivo_muy_grande, |t| {
    let reto = t.reto(COMPANY_USER, "borrador", "reto").await;
    let big = vec![b'x'; 51 * 1024 * 1024];
    let body = multipart_body(&[], &[("archivos", "grande.txt", &big)]);
    let (status, result) = post_multipart(
        &t.app,
        &t.company,
        &format!("/api/retos/{reto}/archivos"),
        body,
    )
    .await;
    assert!(!(200..300).contains(&status), "{result}");
});

// REP-2: a report request with only invalid columns is rejected without panic.
parity!(test_reporte_rechaza_columnas_inexistentes, |t| {
    t.exec("TRUNCATE retos CASCADE").await;
    t.reto(COMPANY_USER, "aprobado", "reto").await;
    let (status, result) = t
        .req(
            &t.admin,
            "POST",
            "/api/reportes/vista-previa",
            Some(json!({"reporte":"retos","periodo":"todo","columnas":["columna_inexistente"]})),
        )
        .await;
    assert_eq!(status, 400, "{result}");
    assert!(result.to_string().contains("columna") || result.to_string().contains("Columna"));
});

// REP-1/REP-4: PDF generation uses a 2000 row limit and export caps rows.
parity!(test_reporte_pdf_respeta_limite_de_filas, |t| {
    t.exec("TRUNCATE retos CASCADE").await;
    t.exec("INSERT INTO retos(empresa_id,titulo,estado) SELECT 2,'Reto '||g,'borrador' FROM generate_series(1,2500) g")
        .await;
    let (status, bytes) = post_bytes(
        &t.app,
        &t.admin,
        "/api/reportes/generar",
        json!({"reporte":"retos","formato":"PDF","periodo":"todo"}),
    )
    .await;
    assert_eq!(status.as_u16(), 200, "PDF generation failed");
    assert!(bytes.starts_with(b"%PDF"));
    // The logged total_registros should be at most 2000.
    let total: i64 = sqlx::query_scalar("SELECT (filtros->>'total_registros')::bigint FROM reportes_generados ORDER BY id DESC LIMIT 1")
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert!(
        total <= 2000,
        "PDF should cap rows before rendering: {total}"
    );
});

// REP-3: professor report catalog only shows reachable companies.
parity!(test_catalogo_reportes_profesor_ve_empresas_alcance, |t| {
    t.exec("TRUNCATE retos CASCADE").await;
    let (company_a, _) = t.company_user("emp-rep-a", "900300200").await;
    let (company_b, _) = t.company_user("emp-rep-b", "900300300").await;
    let reto_a = t.reto(company_a, "aprobado", "reto").await;
    let _reto_b = t.reto(company_b, "aprobado", "reto").await;
    let teacher = t.user("prof-rep", "PROFESOR").await;
    t.integrate(reto_a, teacher).await;
    let session = t.login("prof-rep").await;
    let catalog = t.ok(&session, "GET", "/api/reportes/catalogo", None).await;
    let empresas = catalog["opciones"]["empresas"].as_array().unwrap();
    let razones: Vec<String> = empresas
        .iter()
        .filter_map(|e| e[1].as_str().map(String::from))
        .collect();
    assert!(
        razones.iter().any(|r| r.contains("emp-rep-a S.A.S.")),
        "{razones:?}"
    );
    assert!(
        !razones.iter().any(|r| r.contains("emp-rep-b S.A.S.")),
        "{razones:?}"
    );
});

// REP-5: budget "ejecutado" only counts actually executed expenses (not compromised).
parity!(test_reporte_presupuesto_ejecutado_solo_pagado, |t| {
    t.exec("TRUNCATE retos CASCADE").await;
    let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
    let budget = t
        .i64(&format!(
            "INSERT INTO presupuestos(reto_id,monto_total,alerta_80_enviada,creado_en,actualizado_en) VALUES({reto},1000,false,now(),now()) RETURNING id"
        ))
        .await;
    t.exec(&format!(
        "INSERT INTO gastos(presupuesto_id,categoria,monto,estado,descripcion,creado_en) VALUES({budget},'MATERIALES',100,'EJECUTADO','',now()),({budget},'OTROS',50,'COMPROMETIDO','',now())"
    )).await;
    let data = t
        .ok(
            &t.admin,
            "POST",
            "/api/reportes/vista-previa",
            Some(json!({"reporte":"presupuesto","periodo":"todo","columnas":["reto","ejecutado","gastos"]})),
        )
        .await;
    let headers: Vec<&str> = data["encabezados"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h.as_str().unwrap())
        .collect();
    let idx = headers.iter().position(|&h| h == "Ejecutado").unwrap();
    let n_idx = headers.iter().position(|&h| h == "N.º de gastos").unwrap();
    let row = data["filas"].as_array().unwrap()[0].as_array().unwrap();
    assert_eq!(
        row[idx], "100.00",
        "COMPROMETIDO must not count as executed"
    );
    assert_eq!(
        row[n_idx], "2",
        "executed and committed expenses both count as gastos"
    );
});

// SQLQ-2: professor dashboard only counts approved/published integrations.
parity!(test_dashboard_profesor_solo_integraciones_aprobadas, |t| {
    t.exec("TRUNCATE retos CASCADE").await;
    let (company_a, _) = t.company_user("emp-dash-a", "900300400").await;
    let reto_a = t.reto(company_a, "aprobado", "reto").await;
    let reto_b = t.reto(company_a, "aprobado", "reto").await;
    let teacher = t.user("prof-dash", "PROFESOR").await;
    t.integrate(reto_a, teacher).await;
    let draft = t.integrate(reto_b, teacher).await;
    sqlx::query("UPDATE integraciones_academicas SET estado='en_revision' WHERE id=$1")
        .bind(draft)
        .execute(&t.pool)
        .await
        .unwrap();
    let session = t.login("prof-dash").await;
    let panel = t.ok(&session, "GET", "/api/dashboard", None).await;
    assert_eq!(panel["kpis"]["retos_asignados"], 1, "{panel}");
});

// ACA-8: detail authorization runs before advance_before_read side effects.
parity!(test_detalle_no_avanza_sin_autorizacion, |t| {
    // An unauthorized user cannot trigger advance_before_read for a hackathon.
    let (status, _) = t
        .req(&t.anon, "GET", "/api/records/hackathons/1", None)
        .await;
    assert!(!status.is_success());
});

// ACA-9: only unit managers (admins, professors) list inactive study units.
parity!(test_unidades_inactivas_solo_para_gestores, |t| {
    let inactive = t
        .i64("INSERT INTO unidades_estudio(codigo,nombre,programa_id,periodo,ciclo,activo,creado_en,actualizado_en) VALUES('INV-001','Inactiva',1,'2026-1','1',false,now(),now()) RETURNING id")
        .await;
    let admin_list = t
        .ok(
            &t.admin,
            "GET",
            "/api/records/unidades_estudio?activo=0",
            None,
        )
        .await;
    assert!(
        admin_list["records"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["id"] == inactive)
    );
    t.user("est-unidad", "ESTUDIANTE").await;
    let session = t.login("est-unidad").await;
    let list = t
        .ok(
            &session,
            "GET",
            "/api/records/unidades_estudio?activo=0",
            None,
        )
        .await;
    assert!(
        !list["records"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["id"] == inactive)
    );
});

// ACA-11: dangerous URL schemes are stripped from exports.
parity!(test_export_limpia_urls_peligrosas, |t| {
    let company = t.user("emp-url", "EMPRESA").await;
    t.exec(&format!("INSERT INTO empresas(nit,razon_social,sector_industrial,usuario_id,estado_validacion,estado_listas_restrictivas,estado_convenio,sitio_web) VALUES('900400500','URL SAS','',{},'PENDIENTE','PENDIENTE','NO_RADICADO','javascript:alert(1)')", company)).await;
    let (status, bytes) = download(&t.app, &t.admin, "/api/export/empresas").await;
    assert!(status.is_success(), "export status {status}");
    let csv = String::from_utf8_lossy(&bytes);
    assert!(
        csv.contains("URL SAS"),
        "the company must be exported: {csv}"
    );
    assert!(
        !csv.contains("javascript:"),
        "dangerous URL must be sanitized in export: {csv}"
    );
});

// ACA-12/MED-4: HTML/SVG content is rejected by media::extension.
parity!(test_extension_rechaza_html_y_svg, |_t| {
    assert!(retosean::media::extension(b"<html><body>X</body></html>").is_err());
    assert!(
        retosean::media::extension(b"<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>").is_err()
    );
    assert!(retosean::media::extension(b"%PDF-1.4").is_ok());
});

// MED-4/BIZ-9: media::download and budget summary enforce visible_record scope.
parity!(test_no_descarga_archivo_ajeno, |t| {
    // Company A creates a reto and uploads a file; company B cannot download it.
    let (company_a, _) = t.company_user("emp-media-a", "900300500").await;
    let reto = t.reto(company_a, "borrador", "reto").await;
    let body = multipart_body(&[], &[("archivos", "doc.pdf", b"%PDF-1.4 doc")]);
    post_multipart(
        &t.app,
        &t.login("emp-media-a").await,
        &format!("/api/retos/{reto}/archivos"),
        body,
    )
    .await;
    let row: i64 = sqlx::query_scalar("SELECT id FROM retos_retoarchivo WHERE reto_id=$1")
        .bind(reto)
        .fetch_one(&t.pool)
        .await
        .unwrap();
    t.company_user("emp-media-b", "900300600").await;
    let b = t.login("emp-media-b").await;
    let (status, _) = download(
        &t.app,
        &b,
        &format!("/api/archivo/retos_retoarchivo/{row}/archivo"),
    )
    .await;
    assert!(!status.is_success());
});

parity!(test_no_lee_resumen_presupuesto_ajeno, |t| {
    let reto = t.reto(COMPANY_USER, "aprobado", "reto").await;
    let budget = t
        .i64(&format!("INSERT INTO presupuestos(reto_id,monto_total,alerta_80_enviada,creado_en,actualizado_en) VALUES({reto},500,false,now(),now()) RETURNING id"))
        .await;
    t.company_user("emp-pres-b", "900300700").await;
    let b = t.login("emp-pres-b").await;
    let (status, _) = t
        .req(
            &b,
            "GET",
            &format!("/api/presupuestos/{budget}/resumen"),
            None,
        )
        .await;
    assert!(!status.is_success());
});
