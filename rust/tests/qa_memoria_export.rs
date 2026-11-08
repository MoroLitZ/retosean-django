//! Memory-oriented QA for the export paths: byte-identical output and
//! semaphore-backed concurrency.
mod common;
use calamine::Reader;
use common::*;

parity!(test_exporta_csv_y_xlsx_identicos_a_los_datos, |t| {
    // Insert a uniquely identifiable faculty and export only that row.
    let nombre = "FacultadMemoria123";
    let codigo = "FMM123";
    let id: i64 =
        sqlx::query_scalar("INSERT INTO facultades(nombre,codigo) VALUES($1,$2) RETURNING id")
            .bind(nombre)
            .bind(codigo)
            .fetch_one(&t.pool)
            .await
            .unwrap();

    let path = format!("/api/export/facultades?q={nombre}");
    let (status, csv_bytes) = download(&t.app, &t.admin, &path).await;
    assert_eq!(status, 200);
    let csv = String::from_utf8(csv_bytes).unwrap();

    // Expected CSV computed directly from the data and schema labels.
    // Every cell is quoted, matching domain::csv_cell / csv_cell_str.
    let expected =
        format!("\u{feff}\"ID\",\"Nombre\",\"Código\"\r\n\"{id}\",\"{nombre}\",\"{codigo}\"\r\n");
    assert_eq!(
        csv, expected,
        "CSV must be byte-identical to the expected text"
    );

    // XLSX export must contain the same cell values.
    let (status, xlsx_bytes) = download(
        &t.app,
        &t.admin,
        &format!("/api/export/facultades/xlsx?q={nombre}"),
    )
    .await;
    assert_eq!(status, 200);
    let mut book: calamine::Xlsx<_> =
        calamine::open_workbook_from_rs(std::io::Cursor::new(&xlsx_bytes)).unwrap();
    let sheet = book.worksheet_range_at(0).unwrap().unwrap();
    assert_eq!(sheet.get_value((0, 0)).unwrap().to_string(), "ID");
    assert_eq!(sheet.get_value((0, 1)).unwrap().to_string(), "Nombre");
    assert_eq!(sheet.get_value((0, 2)).unwrap().to_string(), "Código");
    assert_eq!(sheet.get_value((1, 0)).unwrap().to_string(), id.to_string());
    assert_eq!(sheet.get_value((1, 1)).unwrap().to_string(), nombre);
    assert_eq!(sheet.get_value((1, 2)).unwrap().to_string(), codigo);
});

parity!(test_ocho_exportes_concurrentes_completan, |t| {
    // Ensure there is at least one row to export.
    sqlx::query("INSERT INTO facultades(nombre,codigo) VALUES('FC1','FC1')")
        .execute(&t.pool)
        .await
        .unwrap();
    let mut handles = Vec::new();
    for _ in 0..8 {
        let app = t.app.clone();
        let session = t.admin.clone();
        handles.push(tokio::spawn(async move {
            download(&app, &session, "/api/export/facultades").await.0
        }));
    }
    for handle in handles {
        assert_eq!(handle.await.unwrap(), 200, "concurrent export must succeed");
    }
});
