//! apps/presupuesto/tests.py: amounts, committed vs executed, the 80% alert,
//! access and the HU07 expense report.
mod common;
use calamine::{Data, Reader};
use common::*;
use serde_json::{Value, json};

async fn budget(t: &Ctx, reto: i64, total: &str) -> i64 {
    t.i64(&format!("INSERT INTO presupuestos(reto_id,monto_total,alerta_80_enviada,creado_en,actualizado_en) VALUES({reto},{total},false,now(),now()) RETURNING id")).await
}
async fn expense(t: &Ctx, budget: i64, category: &str, amount: &str, estado: &str) -> i64 {
    t.i64(&format!("INSERT INTO gastos(presupuesto_id,categoria,monto,estado,descripcion,creado_en,ejecutado_en) VALUES({budget},'{category}',{amount},'{estado}','',now(),CASE WHEN '{estado}'='EJECUTADO' THEN now() END) RETURNING id")).await
}
async fn summary(t: &Ctx, budget: i64) -> Value {
    t.ok(
        &t.admin,
        "GET",
        &format!("/api/presupuestos/{budget}/resumen"),
        None,
    )
    .await
}
fn amount(value: &Value) -> f64 {
    value.as_str().unwrap().parse().unwrap()
}
async fn alert(t: &Ctx, budget: i64) -> bool {
    t.bool(&format!(
        "SELECT alerta_80_enviada FROM presupuestos WHERE id={budget}"
    ))
    .await
}
/// registrar_gasto through the admin endpoint.
async fn spend(t: &Ctx, budget: i64, amount: &str, estado: &str) {
    t.ok(
        &t.admin,
        "POST",
        &format!("/api/presupuestos/{budget}/gastos"),
        Some(json!({"categoria":"OTROS","monto":amount,"estado":estado})),
    )
    .await;
}
fn sheet(bytes: Vec<u8>) -> (Vec<Data>, calamine::Range<String>) {
    let mut book: calamine::Xlsx<_> =
        calamine::open_workbook_from_rs(std::io::Cursor::new(bytes)).unwrap();
    let values = book.worksheet_range("Gastos").unwrap();
    let formulas = book.worksheet_formula("Gastos").unwrap();
    (
        values.cells().map(|(_, _, v)| v.clone()).collect(),
        formulas,
    )
}

// PresupuestoModeloTests
parity!(test_sin_gastos_el_ejecutado_es_decimal_cero, |t| {
    let reto = t.reto(COMPANY_USER, "aprobado", "reto").await;
    let id = budget(&t, reto, "1000.00").await;
    assert_eq!(amount(&summary(&t, id).await["ejecutado"]), 0.0);
});
parity!(test_calcula_ejecutado_disponible_y_porcentaje, |t| {
    let reto = t.reto(COMPANY_USER, "aprobado", "reto").await;
    let id = budget(&t, reto, "1000.00").await;
    expense(&t, id, "MATERIALES", "250.00", "EJECUTADO").await;
    expense(&t, id, "REFRIGERIOS", "150.00", "EJECUTADO").await;
    let s = summary(&t, id).await;
    assert_eq!(
        (
            amount(&s["ejecutado"]),
            amount(&s["disponible"]),
            amount(&s["porcentaje"])
        ),
        (400.0, 600.0, 40.0)
    );
});
parity!(test_presupuesto_en_cero_no_divide_por_cero, |t| {
    let reto = t.reto(COMPANY_USER, "aprobado", "reto").await;
    let id = budget(&t, reto, "0.00").await;
    assert_eq!(amount(&summary(&t, id).await["porcentaje"]), 0.0);
});

// ComprometidoVsEjecutadoTests
parity!(test_gasto_sin_estado_explicito_cuenta_como_ejecutado, |t| {
    let reto = t.reto(COMPANY_USER, "aprobado", "reto").await;
    let id = budget(&t, reto, "1000.00").await;
    t.exec(&format!("INSERT INTO gastos(presupuesto_id,categoria,monto,descripcion,creado_en) VALUES({id},'OTROS',100.00,'',now())")).await;
    let s = summary(&t, id).await;
    assert_eq!(
        (amount(&s["ejecutado"]), amount(&s["comprometido"])),
        (100.0, 0.0)
    );
});
parity!(test_comprometido_y_ejecutado_se_suman_por_separado, |t| {
    let reto = t.reto(COMPANY_USER, "aprobado", "reto").await;
    let id = budget(&t, reto, "1000.00").await;
    expense(&t, id, "MATERIALES", "300.00", "COMPROMETIDO").await;
    expense(&t, id, "REFRIGERIOS", "200.00", "EJECUTADO").await;
    let s = summary(&t, id).await;
    assert_eq!(
        (amount(&s["comprometido"]), amount(&s["ejecutado"])),
        (300.0, 200.0)
    );
    assert_eq!(
        (amount(&s["disponible"]), amount(&s["porcentaje"])),
        (500.0, 50.0)
    );
});
parity!(
    test_ejecutar_gasto_lo_mueve_de_comprometido_a_ejecutado,
    |t| {
        let reto = t.reto(COMPANY_USER, "aprobado", "reto").await;
        let id = budget(&t, reto, "1000.00").await;
        let gasto = expense(&t, id, "OTROS", "400.00", "COMPROMETIDO").await;
        t.ok(
            &t.admin,
            "POST",
            &format!("/api/gastos/{gasto}/ejecutar"),
            Some(json!({})),
        )
        .await;
        assert_eq!(
            t.text(&format!("SELECT estado FROM gastos WHERE id={gasto}"))
                .await,
            "EJECUTADO"
        );
        assert!(
            t.bool(&format!(
                "SELECT ejecutado_en IS NOT NULL FROM gastos WHERE id={gasto}"
            ))
            .await
        );
        let s = summary(&t, id).await;
        assert_eq!(
            (amount(&s["comprometido"]), amount(&s["ejecutado"])),
            (0.0, 400.0)
        );
    }
);
parity!(test_ejecutar_gasto_ya_ejecutado_no_hace_nada, |t| {
    let reto = t.reto(COMPANY_USER, "aprobado", "reto").await;
    let id = budget(&t, reto, "1000.00").await;
    let gasto = expense(&t, id, "OTROS", "100.00", "EJECUTADO").await;
    let body = t
        .ok(
            &t.admin,
            "POST",
            &format!("/api/gastos/{gasto}/ejecutar"),
            Some(json!({})),
        )
        .await;
    assert_eq!(body["estado"], "EJECUTADO");
});
parity!(
    test_comprometido_por_si_solo_puede_disparar_la_alerta_del_80,
    |t| {
        let reto = t.reto(COMPANY_USER, "aprobado", "reto").await;
        let id = budget(&t, reto, "1000.00").await;
        spend(&t, id, "850.00", "COMPROMETIDO").await;
        assert!(alert(&t, id).await);
    }
);

// AlertaDelOchentaTests
async fn watched(t: &Ctx) -> i64 {
    let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
    budget(t, reto, "1000.00").await
}
async fn alerts(t: &Ctx) -> i64 {
    t.i64("SELECT count(*) FROM notificaciones WHERE evento='PRESUPUESTO_80' AND clave_dedupe<>'rust-demo:presupuesto'").await
}
parity!(test_por_debajo_del_umbral_no_alerta, |t| {
    let id = watched(&t).await;
    spend(&t, id, "500.00", "").await;
    assert!(!alert(&t, id).await);
    assert_eq!(alerts(&t).await, 0);
});
parity!(test_al_superar_el_80_notifica_a_admin_y_empresa, |t| {
    let id = watched(&t).await;
    spend(&t, id, "850.00", "").await;
    assert!(alert(&t, id).await);
    assert_eq!(t.notices(ADMIN, "PRESUPUESTO_80").await, 1);
    assert_eq!(t.notices(COMPANY_USER, "PRESUPUESTO_80").await, 1);
});
parity!(test_la_alerta_no_se_repite_en_cada_gasto, |t| {
    let id = watched(&t).await;
    for value in ["850.00", "50.00", "30.00"] {
        spend(&t, id, value, "").await;
    }
    assert_eq!(alerts(&t).await, 2);
});
parity!(test_ampliar_el_presupuesto_rearma_la_alerta, |t| {
    let id = watched(&t).await;
    spend(&t, id, "850.00", "").await;
    assert!(alert(&t, id).await);
    let reto = t
        .i64(&format!("SELECT reto_id FROM presupuestos WHERE id={id}"))
        .await;
    t.ok(
        &t.admin,
        "POST",
        &format!("/api/retos/{reto}/presupuesto"),
        Some(json!({"monto_total":"5000.00"})),
    )
    .await;
    assert!(!alert(&t, id).await);
});

// AccesoAPresupuestoTests
async fn own(t: &Ctx) -> (i64, i64, Session) {
    t.company_user("empresa-ajena", "900200300").await;
    let other = t.login("empresa-ajena").await;
    let reto = t.reto(COMPANY_USER, "aprobado", "reto").await;
    (reto, budget(t, reto, "500.00").await, other)
}
parity!(test_admin_ve_el_panel, |t| {
    own(&t).await;
    t.ok(&t.admin, "GET", "/api/records/presupuestos", None)
        .await;
});
parity!(test_empresa_ve_el_presupuesto_de_su_reto, |t| {
    let (_, id, _) = own(&t).await;
    t.ok(
        &t.company,
        "GET",
        &format!("/api/presupuestos/{id}/resumen"),
        None,
    )
    .await;
});
parity!(test_empresa_no_ve_el_presupuesto_ajeno, |t| {
    let (_, id, other) = own(&t).await;
    assert!(
        !t.status(
            &other,
            "GET",
            &format!("/api/presupuestos/{id}/resumen"),
            None
        )
        .await
        .is_success()
    );
});
parity!(test_empresa_no_puede_registrar_gastos, |t| {
    let (_, id, _) = own(&t).await;
    assert_eq!(
        t.status(
            &t.company,
            "POST",
            &format!("/api/presupuestos/{id}/gastos"),
            Some(json!({"categoria":"OTROS","monto":"100"}))
        )
        .await,
        403
    );
    assert_eq!(
        t.i64(&format!(
            "SELECT count(*) FROM gastos WHERE presupuesto_id={id}"
        ))
        .await,
        0
    );
});

// ExportacionPresupuestoTests
async fn exportable(t: &Ctx) -> i64 {
    let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
    let id = budget(t, reto, "1000.00").await;
    let gasto = expense(t, id, "MATERIALES", "250.00", "EJECUTADO").await;
    t.exec(&format!(
        "UPDATE gastos SET descripcion='Cartulinas' WHERE id={gasto}"
    ))
    .await;
    reto
}
parity!(test_excel_trae_resumen_y_detalle, |t| {
    let reto = exportable(&t).await;
    let (code, bytes) = download(
        &t.app,
        &t.admin,
        &format!("/api/retos/{reto}/presupuesto/xlsx"),
    )
    .await;
    assert_eq!(code, 200);
    let (values, _) = sheet(bytes);
    assert!(values.contains(&Data::String("Cartulinas".into())));
    assert!(values.contains(&Data::Float(250.0)));
    assert!(values.contains(&Data::Float(1000.0)));
});
parity!(test_pdf_se_genera, |t| {
    let reto = exportable(&t).await;
    let (code, bytes) = download(
        &t.app,
        &t.company,
        &format!("/api/retos/{reto}/presupuesto/pdf"),
    )
    .await;
    assert_eq!(code, 200);
    assert!(bytes.starts_with(b"%PDF"));
});
parity!(test_otra_empresa_no_exporta, |t| {
    let reto = exportable(&t).await;
    t.company_user("empresa-ajena", "900200400").await;
    let other = t.login("empresa-ajena").await;
    let (code, _) = download(
        &t.app,
        &other,
        &format!("/api/retos/{reto}/presupuesto/pdf"),
    )
    .await;
    assert_ne!(code, 200);
});
parity!(test_formato_desconocido_es_404, |t| {
    let reto = exportable(&t).await;
    let (code, _) = download(
        &t.app,
        &t.admin,
        &format!("/api/retos/{reto}/presupuesto/doc"),
    )
    .await;
    assert_eq!(code, 404);
});

// ExportacionConTextoHostilTests
async fn hostile(t: &Ctx) -> i64 {
    let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
    t.exec(&format!(
        "UPDATE retos SET titulo='Reto <b> & cia' WHERE id={reto}"
    ))
    .await;
    let id = budget(t, reto, "1000.00").await;
    for description in [
        "Tinta <b",
        "Pago <proveedor> & cia",
        "=1+1",
        "con\u{1}control",
    ] {
        sqlx::query("INSERT INTO gastos(presupuesto_id,categoria,monto,estado,descripcion,creado_en) VALUES($1,'OTROS',10.00,'EJECUTADO',$2,now())")
            .bind(id).bind(description).execute(&t.pool).await.unwrap();
    }
    reto
}
parity!(test_el_pdf_se_genera_con_simbolos_en_el_texto, |t| {
    let reto = hostile(&t).await;
    let (code, bytes) = download(
        &t.app,
        &t.admin,
        &format!("/api/retos/{reto}/presupuesto/pdf"),
    )
    .await;
    assert_eq!(code, 200);
    assert!(bytes.starts_with(b"%PDF"));
});
parity!(
    test_el_excel_no_convierte_texto_en_formula_ni_falla_con_caracteres_de_control,
    |t| {
        let reto = hostile(&t).await;
        let (code, bytes) = download(
            &t.app,
            &t.admin,
            &format!("/api/retos/{reto}/presupuesto/xlsx"),
        )
        .await;
        assert_eq!(code, 200);
        let (values, formulas) = sheet(bytes);
        assert!(values.contains(&Data::String("=1+1".into())));
        assert!(
            formulas.cells().all(|(_, _, f)| f.is_empty()),
            "no formulas"
        );
        assert!(values.contains(&Data::String("concontrol".into())));
    }
);
