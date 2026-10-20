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
/// Base-14 fonts only cover Windows-1252; other characters print as '?'
/// instead of encoding_rs's literal "&#NNNN;" references.
fn winansi(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    let mut buffer = [0u8; 4];
    for c in text.chars() {
        let (bytes, _, unmappable) = encoding_rs::WINDOWS_1252.encode(c.encode_utf8(&mut buffer));
        if unmappable {
            out.push(b'?');
        } else {
            out.extend_from_slice(&bytes);
        }
    }
    out
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
// Landscape A4 and the reportes/services.py layout (generar_pdf, encabezado_de_marca).
const MM: f32 = 2.8346;
const PAGE_W: f32 = 842.0;
const PAGE_H: f32 = 595.0;
const VERDE: (f32, f32, f32) = (0.231, 0.675, 0.322);
const AZUL: (f32, f32, f32) = (0.0, 0.369, 0.522);
const NEGRO: (f32, f32, f32) = (0.039, 0.039, 0.039);
const GRIS: (f32, f32, f32) = (0.420, 0.447, 0.502);
const SUAVE: (f32, f32, f32) = (0.953, 0.957, 0.965);
const BORDE: (f32, f32, f32) = (0.820, 0.835, 0.859);

/// Helvetica advance widths (Adobe AFM, 1/1000 em) for ASCII 32..=126.
const HELVETICA: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667,
    611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500,
    222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
];
fn glyph(c: char) -> f32 {
    // Accented Latin letters share their base letter's width.
    let base = match c {
        'á' | 'à' | 'ä' | 'â' => 'a',
        'é' | 'è' | 'ë' | 'ê' => 'e',
        'í' | 'ì' | 'ï' | 'î' => 'i',
        'ó' | 'ò' | 'ö' | 'ô' => 'o',
        'ú' | 'ù' | 'ü' | 'û' => 'u',
        'ñ' => 'n',
        'Á' | 'À' | 'Ä' => 'A',
        'É' | 'È' => 'E',
        'Í' => 'I',
        'Ó' | 'Ö' => 'O',
        'Ú' | 'Ü' => 'U',
        'Ñ' => 'N',
        '·' => '.',
        other => other,
    };
    match base as u32 {
        32..=126 => HELVETICA[base as usize - 32] as f32 / 1000.,
        _ => 0.556,
    }
}
fn width(text: &str, size: f32) -> f32 {
    text.chars().map(glyph).sum::<f32>() * size
}
/// Greedy word wrap; words longer than the column are broken by characters.
fn wrap(text: &str, max: f32, size: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let candidate = if line.is_empty() {
                word.to_string()
            } else {
                format!("{line} {word}")
            };
            if width(&candidate, size) <= max {
                line = candidate;
                continue;
            }
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            for c in word.chars() {
                if width(&line, size) + glyph(c) * size > max && !line.is_empty() {
                    lines.push(std::mem::take(&mut line));
                }
                line.push(c);
            }
        }
        lines.push(line);
    }
    lines
}
struct Writer {
    pages: Vec<Content>,
    y: f32,
}
impl Writer {
    fn text(&mut self, x: f32, y: f32, font: &[u8], size: f32, color: (f32, f32, f32), text: &str) {
        let page = self.pages.last_mut().unwrap();
        page.set_fill_rgb(color.0, color.1, color.2);
        page.begin_text()
            .set_font(Name(font), size)
            .next_line(x, y)
            .show(Str(&winansi(text)))
            .end_text();
    }
    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: (f32, f32, f32)) {
        let page = self.pages.last_mut().unwrap();
        page.set_fill_rgb(color.0, color.1, color.2);
        page.rect(x, y, w, h);
        page.fill_nonzero();
    }
    fn new_page(&mut self) {
        self.pages.push(Content::new());
        let number = self.pages.len();
        self.rect(0., PAGE_H - 3. * MM, PAGE_W, 3. * MM, VERDE);
        let top = PAGE_H - 10. * MM;
        self.text(15. * MM, top, b"F2", 10., NEGRO, "retos");
        self.text(
            15. * MM + width("retos", 10.),
            top,
            b"F2",
            10.,
            VERDE,
            "ean",
        );
        let right = PAGE_W - 15. * MM;
        self.text(
            right - width("Universidad EAN", 7.5),
            top,
            b"F1",
            7.5,
            GRIS,
            "Universidad EAN",
        );
        let page = self.pages.last_mut().unwrap();
        page.set_stroke_rgb(AZUL.0, AZUL.1, AZUL.2);
        page.set_line_width(0.6);
        page.move_to(15. * MM, PAGE_H - 13. * MM);
        page.line_to(right, PAGE_H - 13. * MM);
        page.stroke();
        self.text(
            15. * MM,
            10. * MM,
            b"F1",
            7.5,
            GRIS,
            "RetosEAN - Universidad EAN",
        );
        let label = format!("Pagina {number}");
        self.text(
            right - width(&label, 7.5),
            10. * MM,
            b"F1",
            7.5,
            GRIS,
            &label,
        );
        self.y = PAGE_H - 22. * MM;
    }
}
/// generar_pdf: title, description, generation date and filters, then the table.
pub fn pdf_table(
    title: &str,
    meta: &[String],
    columns: &[(&str, f32)],
    rows: &[Vec<String>],
    total: &str,
) -> Vec<u8> {
    let (left, right, bottom) = (15. * MM, PAGE_W - 15. * MM, 18. * MM);
    let mut w = Writer {
        pages: Vec::new(),
        y: 0.,
    };
    w.new_page();
    w.y -= 16.;
    w.text(left, w.y, b"F2", 16., NEGRO, title);
    w.y -= 8.;
    for line in meta {
        for part in wrap(line, right - left, 8.5) {
            w.y -= 11.;
            w.text(left, w.y, b"F1", 8.5, GRIS, &part);
        }
    }
    w.y -= 10.;
    if rows.is_empty() {
        w.y -= 12.;
        w.text(
            left,
            w.y,
            b"F1",
            10.,
            NEGRO,
            "No hay registros que cumplan los filtros seleccionados.",
        );
    } else {
        let sum: f32 = columns.iter().map(|c| c.1).sum::<f32>().max(1.);
        let widths: Vec<f32> = columns.iter().map(|c| (right - left) * c.1 / sum).collect();
        let cells = |values: Vec<&str>, size: f32| -> (Vec<Vec<String>>, f32, f32) {
            let lines: Vec<Vec<String>> = values
                .iter()
                .zip(&widths)
                .map(|(v, cw)| {
                    wrap(
                        v,
                        if size >= 8. {
                            (cw - 8.) / 1.06
                        } else {
                            cw - 8.
                        },
                        size,
                    )
                })
                .collect();
            let leading = if size >= 8. { 10. } else { 9.5 };
            let height = lines.iter().map(Vec::len).max().unwrap_or(1) as f32 * leading + 6.;
            (lines, height, leading)
        };
        let draw = |w: &mut Writer,
                    lines: &[Vec<String>],
                    height: f32,
                    leading: f32,
                    header: bool,
                    shade: bool| {
            let top = w.y;
            if header {
                w.rect(left, top - height, right - left, height, VERDE);
            } else if shade {
                w.rect(left, top - height, right - left, height, SUAVE);
            }
            let mut x = left;
            for (cell, cw) in lines.iter().zip(&widths) {
                for (n, line) in cell.iter().enumerate() {
                    let baseline = top - 3. - (n as f32 + 1.) * leading + 2.;
                    if header {
                        w.text(x + 4., baseline, b"F2", 8., (1., 1., 1.), line);
                    } else {
                        w.text(x + 4., baseline, b"F1", 7.5, NEGRO, line);
                    }
                }
                x += cw;
            }
            let page = w.pages.last_mut().unwrap();
            page.set_stroke_rgb(BORDE.0, BORDE.1, BORDE.2);
            page.set_line_width(0.4);
            page.rect(left, top - height, right - left, height);
            let mut x = left;
            for cw in &widths[..widths.len() - 1] {
                x += cw;
                page.move_to(x, top);
                page.line_to(x, top - height);
            }
            page.stroke();
            w.y = top - height;
        };
        let (head, head_h, head_lead) = cells(columns.iter().map(|c| c.0).collect(), 8.);
        draw(&mut w, &head, head_h, head_lead, true, false);
        for (i, row) in rows.iter().enumerate() {
            let (mut lines, mut height, leading) =
                cells(row.iter().map(String::as_str).collect(), 7.5);
            let room = PAGE_H - 22. * MM - head_h - bottom;
            if height > room {
                // A single row taller than a page is clipped, never lost silently.
                let keep = ((room - 6.) / leading).floor().max(1.) as usize;
                for cell in &mut lines {
                    if cell.len() > keep {
                        cell.truncate(keep);
                        if let Some(last) = cell.last_mut() {
                            last.push_str(" ...");
                        }
                    }
                }
                height = keep as f32 * leading + 6.;
            }
            if w.y - height < bottom {
                w.new_page();
                draw(&mut w, &head, head_h, head_lead, true, false);
            }
            draw(&mut w, &lines, height, leading, false, i % 2 == 1);
        }
    }
    if w.y - 20. < bottom {
        w.new_page();
    }
    w.y -= 16.;
    w.text(right - width(total, 8.5), w.y, b"F1", 8.5, GRIS, total);

    let mut pdf = Pdf::new();
    let pages = Ref::new(2);
    let (font, bold) = (Ref::new(3), Ref::new(4));
    let ids: Vec<Ref> = (0..w.pages.len())
        .map(|i| Ref::new(5 + i as i32 * 2))
        .collect();
    pdf.catalog(Ref::new(1)).pages(pages);
    pdf.pages(pages)
        .kids(ids.iter().copied())
        .count(ids.len() as i32);
    pdf.type1_font(font)
        .base_font(Name(b"Helvetica"))
        .encoding_predefined(Name(b"WinAnsiEncoding"));
    pdf.type1_font(bold)
        .base_font(Name(b"Helvetica-Bold"))
        .encoding_predefined(Name(b"WinAnsiEncoding"));
    for (i, content) in w.pages.into_iter().enumerate() {
        let contents = Ref::new(6 + i as i32 * 2);
        let mut page = pdf.page(ids[i]);
        page.parent(pages)
            .media_box(Rect::new(0., 0., PAGE_W, PAGE_H))
            .contents(contents);
        page.resources()
            .fonts()
            .pair(Name(b"F1"), font)
            .pair(Name(b"F2"), bold);
        page.finish();
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
            .set_font_color("FFFFFF")
            .set_background_color("3BAC52");
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
    fn unsupported_characters_print_as_question_marks() {
        assert_eq!(
            winansi("A\u{f1}o \u{2192} 5\u{20ac}"),
            b"A\xf1o ? 5\x80".to_vec()
        );
    }
    #[test]
    fn table_wraps_and_repeats_header_across_pages() {
        let lines = wrap("Universidad EAN supercalifragilisticoespialidoso", 60., 7.5);
        assert!(lines.len() > 2 && lines.iter().all(|l| width(l, 7.5) <= 60.));
        let rows: Vec<Vec<String>> = (0..120)
            .map(|i| vec![format!("Reto {i}"), "Descripción larga ".repeat(6)])
            .collect();
        let pdf = pdf_table(
            "Reporte",
            &["Generado".into()],
            &[("Reto", 20.), ("Detalle", 40.)],
            &rows,
            "Total de registros: 120",
        );
        let text = String::from_utf8_lossy(&pdf);
        assert!(
            text.matches("/MediaBox").count() >= 3,
            "120 wrapped rows span pages"
        );
        assert!(text.contains("842"), "landscape A4");
    }
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
