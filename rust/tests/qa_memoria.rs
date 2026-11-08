//! Memory optimisation regressions: attachments are spooled to disk while they
//! stream in and downloads are streamed back, with the same limits and content
//! checks as before and no temporary file left behind on any error.
mod common;
use common::*;

/// Temporary spool files (`.<uuid>.part`) still present in the media directory.
fn spool_files(t: &Ctx) -> usize {
    std::fs::read_dir(&t.media)
        .map(|dir| {
            dir.filter_map(|e| e.ok())
                .filter(|e| e.file_name().to_string_lossy().ends_with(".part"))
                .count()
        })
        .unwrap_or(0)
}

parity!(test_adjuntos_se_guardan_y_descargan_identicos, |t| {
    let reto = t.reto(COMPANY_USER, "borrador", "reto").await;
    // Bigger than one multipart chunk, with varied bytes.
    let pdf: Vec<u8> = b"%PDF-1.4\n"
        .iter()
        .copied()
        .chain((0..3_000_000u32).map(|i| (i % 251) as u8))
        .collect();
    let text = "línea con acentos: ñ, á, é, ü — fin\n".repeat(20_000);
    let (code, body) = multipart(
        &t.app,
        &t.company,
        &format!("/api/retos/{reto}/archivos"),
        &[],
        &[
            ("archivos", "grande.pdf", &pdf),
            ("archivos", "notas.txt", text.as_bytes()),
        ],
    )
    .await;
    assert_eq!(code, 200, "{body}");
    assert_eq!(body["archivos"], 2);
    let ids: Vec<i64> = sqlx::query_scalar(&format!(
        "SELECT id FROM retos_retoarchivo WHERE reto_id={reto} ORDER BY id"
    ))
    .fetch_all(&t.pool)
    .await
    .unwrap();
    let sizes: Vec<i64> = sqlx::query_scalar(&format!(
        "SELECT tamano FROM retos_retoarchivo WHERE reto_id={reto} ORDER BY id"
    ))
    .fetch_all(&t.pool)
    .await
    .unwrap();
    assert_eq!(sizes, [pdf.len() as i64, text.len() as i64]);
    for (id, expected) in ids.iter().zip([pdf.as_slice(), text.as_bytes()]) {
        let (status, bytes) = download(
            &t.app,
            &t.admin,
            &format!("/api/archivo/retos_retoarchivo/{id}/archivo"),
        )
        .await;
        assert_eq!(status, 200);
        assert!(bytes == expected, "downloaded bytes differ for {id}");
    }
    assert_eq!(spool_files(&t), 0);
});

parity!(test_descarga_informa_content_length, |t| {
    let reto = t.reto(COMPANY_USER, "borrador", "reto").await;
    let data = b"%PDF-1.4 contenido de prueba".to_vec();
    let (code, _) = multipart(
        &t.app,
        &t.company,
        &format!("/api/retos/{reto}/archivos"),
        &[],
        &[("archivos", "a.pdf", &data)],
    )
    .await;
    assert_eq!(code, 200);
    let id = t
        .i64(&format!(
            "SELECT id FROM retos_retoarchivo WHERE reto_id={reto}"
        ))
        .await;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;
    let response = t
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/archivo/retos_retoarchivo/{id}/archivo"))
                .header("cookie", &t.admin.cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.headers()["content-length"],
        data.len().to_string().as_str()
    );
    assert_eq!(
        response.headers()["content-type"],
        "application/octet-stream"
    );
    assert!(
        response.headers()["content-disposition"]
            .to_str()
            .unwrap()
            .starts_with("attachment;")
    );
});

parity!(
    test_texto_invalido_o_con_nul_se_rechaza_sin_temporales,
    |t| {
        let reto = t.reto(COMPANY_USER, "borrador", "reto").await;
        let mut invalid = "texto válido ".repeat(10_000).into_bytes();
        invalid.extend([0xff, 0xfe]);
        let mut with_nul = "texto válido ".repeat(10_000).into_bytes();
        with_nul.insert(5_000, 0);
        for (name, data) in [("malo.txt", invalid), ("nul.csv", with_nul)] {
            let (code, body) = multipart(
                &t.app,
                &t.company,
                &format!("/api/retos/{reto}/archivos"),
                &[],
                &[("archivos", name, &data)],
            )
            .await;
            assert_eq!(code, 400, "{name}: {body}");
            assert!(
                body["error"]
                    .as_str()
                    .unwrap()
                    .contains("el contenido no coincide con su extensión"),
                "{body}"
            );
        }
        assert_eq!(
            t.i64(&format!(
                "SELECT count(*) FROM retos_retoarchivo WHERE reto_id={reto}"
            ))
            .await,
            0
        );
        assert_eq!(spool_files(&t), 0);
    }
);

parity!(test_contenido_disfrazado_se_rechaza_sin_temporales, |t| {
    let reto = t.reto(COMPANY_USER, "borrador", "reto").await;
    // An HTML page renamed to .pdf, after a valid file in the same request.
    let (code, body) = multipart(
        &t.app,
        &t.company,
        &format!("/api/retos/{reto}/archivos"),
        &[],
        &[
            ("archivos", "bueno.pdf", b"%PDF-1.4 ok"),
            (
                "archivos",
                "malo.pdf",
                b"<html><script>alert(1)</script></html>",
            ),
        ],
    )
    .await;
    assert_eq!(code, 400, "{body}");
    assert_eq!(spool_files(&t), 0);
    let stored = std::fs::read_dir(&t.media).map(|d| d.count()).unwrap_or(0);
    assert_eq!(stored, 0, "no file may be stored when the request fails");
});

parity!(test_limites_de_tamano_no_dejan_temporales, |t| {
    let reto = t.reto(COMPANY_USER, "borrador", "reto").await;
    let big = vec![b'x'; 50 * 1024 * 1024 + 1];
    let (code, body) = multipart(
        &t.app,
        &t.company,
        &format!("/api/retos/{reto}/archivos"),
        &[],
        &[("archivos", "grande.txt", &big)],
    )
    .await;
    assert_eq!(code, 400);
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("supera el límite de 50 MB"),
        "{body}"
    );
    // 3 x 35 MB = 105 MB: under the 110 MB body limit, over the 100 MB total.
    let part = vec![b'y'; 35 * 1024 * 1024];
    let (code, body) = post_raw_files(&t, reto, &[&part, &part, &part]).await;
    assert_eq!(code, 400);
    assert!(
        body.contains("El tamaño total de los archivos supera el límite permitido"),
        "{body}"
    );
    assert_eq!(spool_files(&t), 0);
});

/// Three text parts; the route's 110 MB body limit allows building them.
async fn post_raw_files(t: &Ctx, reto: i64, parts: &[&[u8]]) -> (u16, String) {
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    let mut body = Vec::new();
    for (i, part) in parts.iter().enumerate() {
        body.extend(format!("--FRM\r\nContent-Disposition: form-data; name=\"archivos\"; filename=\"p{i}.txt\"\r\n\r\n").into_bytes());
        body.extend(*part);
        body.extend(b"\r\n");
    }
    body.extend(b"--FRM--\r\n");
    let response = t
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/retos/{reto}/archivos"))
                .header("cookie", &t.company.cookie)
                .header("x-csrf-token", &t.company.csrf)
                .header("content-type", "multipart/form-data; boundary=FRM")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status().as_u16();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}
