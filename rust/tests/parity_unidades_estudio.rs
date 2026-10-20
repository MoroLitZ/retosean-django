//! apps/unidades_estudio/tests.py: CSV bulk load and single creation.
mod common;
use common::*;
use serde_json::Value;

async fn program(t: &Ctx) -> i64 {
    t.i64("INSERT INTO programas(nombre,facultad_id,nivel) VALUES('Ingenieria de Sistemas',1,'') RETURNING id").await
}
async fn load(t: &Ctx, session: &Session, header: &str, rows: &[&str]) -> (u16, Value) {
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
const HEADER: &str = "codigo,nombre,programa,periodo,ciclo";
async fn units(t: &Ctx) -> i64 {
    t.i64("SELECT count(*) FROM unidades_estudio").await
}

// CargaMasivaCsvTests
parity!(test_crea_varias_unidades, |t| {
    let program = program(&t).await;
    load(
        &t,
        &t.admin,
        HEADER,
        &[
            "ING-101,Introduccion a la Ingenieria,Ingenieria de Sistemas,2026-2,1",
            "MAT-201,Calculo Diferencial,Ingenieria de Sistemas,2026-2,2",
        ],
    )
    .await;
    assert_eq!(units(&t).await, 2);
    assert_eq!(
        t.i64("SELECT programa_id FROM unidades_estudio WHERE codigo='ING-101'")
            .await,
        program
    );
});
parity!(test_reutiliza_el_programa_sin_distinguir_mayusculas, |t| {
    program(&t).await;
    load(
        &t,
        &t.admin,
        HEADER,
        &["ING-102,Algoritmos,INGENIERIA DE SISTEMAS,2026-2,2"],
    )
    .await;
    assert_eq!(
        t.i64("SELECT count(*) FROM programas WHERE lower(nombre)='ingenieria de sistemas'")
            .await,
        1
    );
    assert_eq!(units(&t).await, 1);
});
parity!(test_omite_programa_inexistente, |t| {
    program(&t).await;
    load(
        &t,
        &t.admin,
        HEADER,
        &["ADM-101,Fundamentos,Administracion de Empresas,2026-2,1"],
    )
    .await;
    assert_eq!(
        t.i64("SELECT count(*) FROM programas WHERE lower(nombre)='administracion de empresas'")
            .await,
        0
    );
    assert_eq!(units(&t).await, 0);
});
parity!(test_omite_codigos_duplicados, |t| {
    program(&t).await;
    load(
        &t,
        &t.admin,
        HEADER,
        &[
            "DUP-100,Una,Ingenieria de Sistemas,2026-2,1",
            "DUP-100,Otra,Ingenieria de Sistemas,2026-2,1",
        ],
    )
    .await;
    assert_eq!(
        t.i64("SELECT count(*) FROM unidades_estudio WHERE codigo='DUP-100'")
            .await,
        1
    );
});
parity!(test_rechaza_csv_sin_columnas_obligatorias, |t| {
    program(&t).await;
    let (code, body) = load(&t, &t.admin, "periodo,ciclo", &["2026-2,1"]).await;
    assert_eq!(units(&t).await, 0);
    assert_eq!(code, 400);
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("columnas obligatorias"),
        "{body}"
    );
});
parity!(test_acepta_punto_y_coma_como_separador, |t| {
    program(&t).await;
    load(
        &t,
        &t.admin,
        "codigo;nombre;programa;periodo;ciclo",
        &["SEP-100;Con punto y coma;Ingenieria de Sistemas;2026-2;1"],
    )
    .await;
    assert_eq!(
        t.i64("SELECT count(*) FROM unidades_estudio WHERE codigo='SEP-100'")
            .await,
        1
    );
});
parity!(test_estudiante_no_puede_cargar, |t| {
    program(&t).await;
    let (code, _) = load(
        &t,
        &t.student,
        HEADER,
        &["NO-100,No,Ingenieria de Sistemas,2026-2,1"],
    )
    .await;
    assert_eq!(code, 403);
    assert_eq!(units(&t).await, 0);
});

// CreacionIndividualTests
parity!(test_crea_una_unidad_con_programa_existente, |t| {
    let program = program(&t).await;
    let id = program.to_string();
    let (code, body) = multipart(
        &t.app,
        &t.admin,
        "/api/unidades",
        &[
            ("codigo", "UNI-100"),
            ("nombre", "Unidad de prueba"),
            ("programa", id.as_str()),
            ("periodo", "2026-2"),
            ("ciclo", "3"),
        ],
        &[],
    )
    .await;
    assert_eq!(code, 200, "{body}");
    assert_eq!(
        t.i64("SELECT programa_id FROM unidades_estudio WHERE codigo='UNI-100'")
            .await,
        program
    );
});
