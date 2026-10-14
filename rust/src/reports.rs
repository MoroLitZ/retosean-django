use crate::{
    AppState, auth,
    error::{AppError, Result},
    resources::{self, Filters},
};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str};
use serde_json::Value;

fn text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        v => v.to_string(),
    }
}
fn winansi(text: &str) -> Vec<u8> {
    encoding_rs::WINDOWS_1252.encode(text).0.into_owned()
}
/// Each page uses a fixed-width base font and bounded lines; no HTML is interpreted.
pub fn pdf_lines(title: &str, lines: &[String]) -> Vec<u8> {
    let mut pdf = Pdf::new();
    let pages = Ref::new(2);
    let font = Ref::new(3);
    let bold = Ref::new(4);
    let chunks: Vec<_> = if lines.is_empty() {
        vec![&[][..]]
    } else {
        lines.chunks(47).collect()
    };
    let ids: Vec<_> = (0..chunks.len())
        .map(|i| Ref::new(5 + (i as i32) * 2))
        .collect();
    pdf.catalog(Ref::new(1)).pages(pages);
    pdf.pages(pages)
        .kids(ids.iter().copied())
        .count(ids.len() as i32);
    pdf.type1_font(font)
        .base_font(Name(b"Courier"))
        .encoding_predefined(Name(b"WinAnsiEncoding"));
    pdf.type1_font(bold)
        .base_font(Name(b"Helvetica-Bold"))
        .encoding_predefined(Name(b"WinAnsiEncoding"));
    for (i, chunk) in chunks.iter().enumerate() {
        let contents = Ref::new(6 + (i as i32) * 2);
        let mut page = pdf.page(ids[i]);
        page.parent(pages)
            .media_box(Rect::new(0., 0., 595., 842.))
            .contents(contents);
        page.resources()
            .fonts()
            .pair(Name(b"F1"), font)
            .pair(Name(b"F2"), bold);
        page.finish();
        let mut content = Content::new();
        content.set_fill_rgb(0.10, 0.23, 0.17);
        content.rect(0., 812., 595., 30.);
        content.fill_nonzero();
        content
            .begin_text()
            .set_font(Name(b"F2"), 19.)
            .next_line(40., 777.)
            .show(Str(&winansi("RetosEAN · Universidad EAN")))
            .end_text();
        content
            .begin_text()
            .set_font(Name(b"F1"), 10.)
            .next_line(40., 750.)
            .show(Str(&winansi(&title.chars().take(85).collect::<String>())))
            .end_text();
        for (n, line) in chunk.iter().enumerate() {
            content
                .begin_text()
                .set_font(Name(b"F1"), 9.)
                .next_line(40., 720. - n as f32 * 13.)
                .show(Str(&winansi(&line.chars().take(95).collect::<String>())))
                .end_text();
        }
        content
            .begin_text()
            .set_font(Name(b"F1"), 8.)
            .next_line(40., 42.)
            .show(Str(&winansi(&format!(
                "{} | Página {} de {}",
                chrono::Utc::now()
                    .with_timezone(&chrono::FixedOffset::west_opt(5 * 3600).unwrap())
                    .format("%Y-%m-%d"),
                i + 1,
                chunks.len()
            ))))
            .end_text();
        pdf.stream(contents, &content.finish());
    }
    pdf.finish()
}
pub async fn export(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((name, format)): Path<(String, String)>,
    Query(filters): Query<Filters>,
) -> Result<Response> {
    let user = auth::current(&state, &headers, false).await?;
    if !["pdf", "xlsx"].contains(&format.as_str()) {
        return Err(AppError::not_found());
    }
    let data = resources::rows(&state, &user, &name, &filters, true).await?;
    let rows = data["records"].as_array().unwrap();
    let fields: Vec<String> = resources::resource(&name)?["fields"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|f| f["name"].as_str())
        .filter(|f| !["password", "is_staff", "is_superuser"].contains(f))
        .map(str::to_owned)
        .collect();
    let title = format!(
        "{} | {} de {} registros",
        data["resource"].as_str().unwrap_or("Reporte"),
        rows.len(),
        data["total"]
    );
    let (content_type, disposition, bytes) = if format == "pdf" {
        let mut lines = vec![title.clone(), String::new()];
        for row in rows {
            for field in &fields {
                let line = format!("{field}: {}", text(&row[field]));
                let chars: Vec<_> = line.chars().collect();
                for chunk in chars.chunks(95) {
                    lines.push(chunk.iter().collect());
                }
            }
            lines.push("—".repeat(60));
        }
        (
            "application/pdf",
            "attachment; filename=retosean.pdf",
            pdf_lines(&title, &lines),
        )
    } else {
        let mut book = rust_xlsxwriter::Workbook::new();
        let sheet = book.add_worksheet();
        let error = |e: rust_xlsxwriter::XlsxError| {
            tracing::error!(%e,"spreadsheet export failed");
            AppError(
                StatusCode::INTERNAL_SERVER_ERROR,
                "No se pudo generar el archivo Excel".into(),
            )
        };
        let heading = rust_xlsxwriter::Format::new()
            .set_bold()
            .set_background_color("A8D43A");
        for (col, field) in fields.iter().enumerate() {
            sheet
                .write_string_with_format(0, col as u16, field, &heading)
                .map_err(error)?;
            sheet.set_column_width(col as u16, 24).map_err(error)?;
        }
        for (r, row) in rows.iter().enumerate() {
            for (c, field) in fields.iter().enumerate() {
                let value = &row[field];
                if let Some(boolean) = value.as_bool() {
                    sheet
                        .write_boolean(r as u32 + 1, c as u16, boolean)
                        .map_err(error)?;
                } else {
                    // Strings preserve identifiers, decimal precision and formula safety.
                    sheet
                        .write_string(r as u32 + 1, c as u16, text(value))
                        .map_err(error)?;
                }
            }
        }
        sheet.set_freeze_panes(1, 0).map_err(error)?;
        let bytes = book.save_to_buffer().map_err(error)?;
        (
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "attachment; filename=retosean.xlsx",
            bytes,
        )
    };
    Ok((
        [
            (header::CONTENT_TYPE, content_type),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        bytes,
    )
        .into_response())
}
pub async fn certificate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Response> {
    let user = auth::current(&state, &headers, false).await?;
    let certificate = resources::visible_record(&state, &user, "certificados", id).await?;
    let person: i64 = certificate["usuario_id"]
        .as_i64()
        .ok_or_else(AppError::not_found)?;
    let name: String =
        sqlx::query_scalar("SELECT trim(first_name||' '||last_name) FROM usuarios WHERE id=$1")
            .bind(person)
            .fetch_one(&state.pool)
            .await?;
    let reto: i64 = certificate["reto_id"]
        .as_i64()
        .ok_or_else(AppError::not_found)?;
    let title: String = sqlx::query_scalar("SELECT titulo FROM retos WHERE id=$1")
        .bind(reto)
        .fetch_one(&state.pool)
        .await?;
    let mut lines = vec![
        String::new(),
        "CERTIFICADO DE PARTICIPACIÓN".into(),
        String::new(),
        "Se certifica que".into(),
        String::new(),
        name,
        String::new(),
        format!(
            "participó en calidad de {}",
            text(&certificate["rol_participacion"])
        ),
        String::new(),
    ];
    let chars: Vec<_> = title.chars().collect();
    for chunk in chars.chunks(90) {
        lines.push(chunk.iter().collect());
    }
    lines.extend([
        String::new(),
        format!("Horas: {}", text(&certificate["horas"])),
        format!("Fecha de emisión: {}", text(&certificate["emitido_en"])),
        String::new(),
        "Código de verificación:".into(),
        text(&certificate["codigo_verificacion"]),
        String::new(),
        format!(
            "Verificar: /api/verificar/{}",
            text(&certificate["codigo_verificacion"])
        ),
    ]);
    Ok((
        [
            (header::CONTENT_TYPE, "application/pdf"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=certificado-retosean.pdf",
            ),
        ],
        pdf_lines("Certificado de participación", &lines),
    )
        .into_response())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pdf_has_pages_and_cross_reference() {
        let bytes = pdf_lines(
            "Participación",
            &(0..100).map(|n| format!("Línea {n}")).collect::<Vec<_>>(),
        );
        assert!(bytes.starts_with(b"%PDF-"));
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("/Count 3"));
        assert!(text.contains("startxref"));
        assert!(text.contains("%%EOF"));
    }
}
