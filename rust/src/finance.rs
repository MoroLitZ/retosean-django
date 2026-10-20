use crate::{
    AppState, auth,
    error::{AppError, Result},
    workflows::notify,
};
use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};

pub fn money(text: &str) -> Result<Decimal> {
    let value = text
        .parse::<Decimal>()
        .map_err(|_| AppError::bad("Monto decimal inválido"))?;
    if value < Decimal::ZERO || value.scale() > 2 || value > Decimal::from(9_999_999_999i64) {
        return Err(AppError::bad(
            "El monto debe ser positivo, con máximo dos decimales y diez enteros",
        ));
    }
    Ok(value)
}
#[derive(Deserialize)]
pub struct BudgetInput {
    monto_total: String,
}
pub async fn set_budget(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(reto): Path<i64>,
    Json(input): Json<BudgetInput>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    if !user.admin() {
        return Err(AppError::forbidden());
    }
    let amount = money(&input.monto_total)?;
    let mut tx = state.pool.begin().await?;
    let eligible: bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM retos WHERE id=$1 AND eliminado_en IS NULL AND estado NOT IN ('borrador','cancelado'))").bind(reto).fetch_one(&mut *tx).await?;
    if !eligible {
        return Err(AppError::bad("El reto no admite presupuesto"));
    }
    let id: i64=sqlx::query_scalar("INSERT INTO presupuestos(reto_id,monto_total,alerta_80_enviada,creado_en,actualizado_en) VALUES($1,$2,false,now(),now()) ON CONFLICT(reto_id) DO UPDATE SET monto_total=excluded.monto_total,actualizado_en=now() RETURNING id").bind(reto).bind(amount).fetch_one(&mut *tx).await?;
    check_threshold(&mut tx, id).await?;
    tx.commit().await?;
    Ok(Json(json!({"id":id,"monto_total":amount.to_string()})))
}
#[derive(Deserialize)]
pub struct ExpenseInput {
    categoria: String,
    monto: String,
    #[serde(default)]
    descripcion: String,
    /// COMPROMETIDO (reserved) or EJECUTADO (paid, the default).
    #[serde(default)]
    estado: String,
}
pub async fn expense(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<ExpenseInput>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    if !user.admin() {
        return Err(AppError::forbidden());
    }
    let amount = money(&input.monto)?;
    if amount == Decimal::ZERO
        || ![
            "REFRIGERIOS",
            "MATERIALES",
            "IMPRESIONES",
            "ESPACIOS",
            "OTROS",
        ]
        .contains(&input.categoria.as_str())
        || input.descripcion.len() > 4000
        || !["", "COMPROMETIDO", "EJECUTADO"].contains(&input.estado.as_str())
    {
        return Err(AppError::bad("Gasto inválido"));
    }
    let mut tx = state.pool.begin().await?;
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT id FROM presupuestos WHERE id=$1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    if exists.is_none() {
        return Err(AppError::not_found());
    }
    let estado = if input.estado.is_empty() {
        "EJECUTADO"
    } else {
        input.estado.as_str()
    };
    let gasto: i64 = sqlx::query_scalar("INSERT INTO gastos(presupuesto_id,categoria,monto,estado,descripcion,registrado_por_id,creado_en,ejecutado_en) VALUES($1,$2,$3,$4,$5,$6,now(),CASE WHEN $4='EJECUTADO' THEN now() END) RETURNING id")
        .bind(id).bind(&input.categoria).bind(amount).bind(estado).bind(&input.descripcion).bind(user.id).fetch_one(&mut *tx).await?;
    check_threshold(&mut tx, id).await?;
    tx.commit().await?;
    Ok(Json(json!({"id":gasto,"monto":amount.to_string()})))
}
/// (total, comprometido, ejecutado) of a budget.
async fn amounts(
    tx: &mut Transaction<'_, Postgres>,
    id: i64,
) -> Result<(Decimal, Decimal, Decimal)> {
    Ok(sqlx::query_as("SELECT b.monto_total,COALESCE(sum(g.monto) FILTER (WHERE g.estado='COMPROMETIDO'),0),COALESCE(sum(g.monto) FILTER (WHERE g.estado<>'COMPROMETIDO'),0) FROM presupuestos b LEFT JOIN gastos g ON g.presupuesto_id=b.id WHERE b.id=$1 GROUP BY b.id")
        .bind(id).fetch_one(&mut **tx).await?)
}
fn percent(total: Decimal, assigned: Decimal) -> Decimal {
    if total == Decimal::ZERO {
        Decimal::ZERO
    } else {
        assigned / total * Decimal::from(100)
    }
}
/// revisar_umbral: notify once when committed + executed reaches 80%; below it the
/// alert re-arms (budget increased or an expense removed).
async fn check_threshold(tx: &mut Transaction<'_, Postgres>, id: i64) -> Result<()> {
    let (alert, company, title): (bool, i64, String) = sqlx::query_as("SELECT b.alerta_80_enviada,r.empresa_id,r.titulo FROM presupuestos b JOIN retos r ON r.id=b.reto_id WHERE b.id=$1 FOR UPDATE OF b")
        .bind(id).fetch_one(&mut **tx).await?;
    let (total, committed, executed) = amounts(tx, id).await?;
    let pct = percent(total, committed + executed);
    if pct < Decimal::from(80) {
        if alert {
            sqlx::query("UPDATE presupuestos SET alerta_80_enviada=false WHERE id=$1")
                .bind(id)
                .execute(&mut **tx)
                .await?;
        }
        return Ok(());
    }
    if alert {
        return Ok(());
    }
    let message = format!(
        "El presupuesto del reto '{title}' va en {:.1}% asignado (comprometido: {committed}, ejecutado: {executed}, de {total}). Disponible: {}.",
        pct.round_dp(1),
        total - committed - executed
    );
    let mut recipients: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM usuarios WHERE is_active AND (is_superuser OR rol='ADMIN')",
    )
    .fetch_all(&mut **tx)
    .await?;
    recipients.push(company);
    recipients.sort_unstable();
    recipients.dedup();
    for recipient in recipients {
        notify(
            tx,
            recipient,
            "PRESUPUESTO_80",
            &message,
            &format!("presupuesto:{id}:80"),
        )
        .await?;
    }
    sqlx::query("UPDATE presupuestos SET alerta_80_enviada=true WHERE id=$1")
        .bind(id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
/// marcar_ejecutado: a committed expense becomes executed (paid); others unchanged.
pub async fn execute(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(gasto): Path<i64>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    if !user.admin() {
        return Err(AppError::forbidden());
    }
    let mut tx = state.pool.begin().await?;
    let row: Option<(i64, String)> =
        sqlx::query_as("SELECT presupuesto_id,estado FROM gastos WHERE id=$1 FOR UPDATE")
            .bind(gasto)
            .fetch_optional(&mut *tx)
            .await?;
    let (budget, estado) = row.ok_or_else(AppError::not_found)?;
    if estado != "COMPROMETIDO" {
        return Ok(Json(
            json!({"estado":estado,"mensaje":"Ese gasto ya estaba ejecutado."}),
        ));
    }
    sqlx::query("UPDATE gastos SET estado='EJECUTADO',ejecutado_en=now() WHERE id=$1")
        .bind(gasto)
        .execute(&mut *tx)
        .await?;
    check_threshold(&mut tx, budget).await?;
    tx.commit().await?;
    Ok(Json(
        json!({"estado":"EJECUTADO","mensaje":"Gasto marcado como ejecutado."}),
    ))
}
/// eliminar_gasto: removing an expense can drop below the threshold again.
pub async fn delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(gasto): Path<i64>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    if !user.admin() {
        return Err(AppError::forbidden());
    }
    let mut tx = state.pool.begin().await?;
    let budget: Option<i64> =
        sqlx::query_scalar("DELETE FROM gastos WHERE id=$1 RETURNING presupuesto_id")
            .bind(gasto)
            .fetch_optional(&mut *tx)
            .await?;
    check_threshold(&mut tx, budget.ok_or_else(AppError::not_found)?).await?;
    tx.commit().await?;
    Ok(Json(json!({"ok":true,"mensaje":"Gasto eliminado."})))
}
pub async fn summary(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, false).await?;
    crate::resources::visible_record(&state, &user, "presupuestos", id).await?;
    let mut tx = state.pool.begin().await?;
    let (total, committed, executed) = amounts(&mut tx, id).await?;
    let pct = percent(total, committed + executed);
    let expenses: Value = sqlx::query_scalar("SELECT COALESCE(jsonb_agg(jsonb_build_object('id',id,'categoria',categoria,'estado',estado,'monto',monto::text,'descripcion',descripcion,'creado_en',creado_en) ORDER BY creado_en,id),'[]') FROM gastos WHERE presupuesto_id=$1")
        .bind(id).fetch_one(&mut *tx).await?;
    let reto: i64 = sqlx::query_scalar("SELECT reto_id FROM presupuestos WHERE id=$1")
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    Ok(Json(json!({
        "reto": reto,
        "gastos": expenses,
        "total": total.to_string(),
        "comprometido": committed.to_string(),
        "ejecutado": executed.to_string(),
        "asignado": (committed + executed).to_string(),
        "disponible": (total - committed - executed).to_string(),
        "porcentaje": pct.round_dp(2).to_string(),
        "alerta": pct >= Decimal::from(80),
    })))
}

/// presupuesto:exportar (HU07): admin or the owning company; xlsx or pdf.
pub async fn export(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((reto, format)): Path<(i64, String)>,
) -> Result<axum::response::Response> {
    use axum::response::IntoResponse;
    let user = auth::current(&state, &headers, false).await?;
    if !["xlsx", "pdf"].contains(&format.as_str()) {
        return Err(AppError::not_found());
    }
    if !user.admin() && user.rol != "EMPRESA" {
        return Err(AppError::forbidden());
    }
    let row: Option<(i64, String, i64)> = sqlx::query_as("SELECT b.id,r.titulo,r.empresa_id FROM presupuestos b JOIN retos r ON r.id=b.reto_id WHERE r.id=$1")
        .bind(reto)
        .fetch_optional(&state.pool)
        .await?;
    let (id, title, owner) = row.ok_or_else(AppError::not_found)?;
    if !user.admin() && owner != user.id {
        return Err(AppError::forbidden());
    }
    let mut tx = state.pool.begin().await?;
    let (total, committed, executed) = amounts(&mut tx, id).await?;
    let pct = percent(total, committed + executed).round_dp(2);
    let by_category: Vec<(String, Decimal, Decimal)> = sqlx::query_as("SELECT c,COALESCE(sum(g.monto) FILTER (WHERE g.estado='COMPROMETIDO'),0),COALESCE(sum(g.monto) FILTER (WHERE g.estado<>'COMPROMETIDO'),0) FROM unnest(ARRAY['REFRIGERIOS','MATERIALES','IMPRESIONES','ESPACIOS','OTROS']) WITH ORDINALITY k(c,n) LEFT JOIN gastos g ON g.presupuesto_id=$1 AND g.categoria=k.c GROUP BY k.c,k.n ORDER BY k.n")
        .bind(id).fetch_all(&mut *tx).await?;
    let expenses: Vec<(String, String, String, String, Decimal, String, bool)> = sqlx::query_as("SELECT to_char(g.creado_en AT TIME ZONE 'America/Bogota','DD/MM/YYYY'),g.categoria,g.estado,COALESCE(g.descripcion,''),g.monto,COALESCE(u.username,''),COALESCE(g.comprobante,'')<>'' FROM gastos g LEFT JOIN usuarios u ON u.id=g.registrado_por_id WHERE g.presupuesto_id=$1 ORDER BY g.creado_en,g.id")
        .bind(id).fetch_all(&mut *tx).await?;
    let label = |field: &str, value: &str| crate::dashboard::etiqueta("gastos", field, value);
    // agregar_fila: control characters never reach the file.
    let clean = |text: &str| {
        text.chars()
            .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
            .collect::<String>()
    };
    let generated = (chrono::Utc::now() - chrono::Duration::hours(5))
        .format("%d/%m/%Y %H:%M")
        .to_string();
    let summary = [
        ("Asignado", total),
        ("Comprometido", committed),
        ("Ejecutado", executed),
        ("Disponible", total - committed - executed),
        ("% asignado", pct),
    ];
    let filename = format!("presupuesto-reto-{reto}.{format}");
    if format == "xlsx" {
        let fail = |e: rust_xlsxwriter::XlsxError| {
            tracing::error!(%e, "budget export failed");
            AppError(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "No se pudo generar el archivo Excel".into(),
            )
        };
        let number = |d: Decimal| d.to_string().parse::<f64>().unwrap_or(0.);
        let mut book = rust_xlsxwriter::Workbook::new();
        let head = rust_xlsxwriter::Format::new()
            .set_bold()
            .set_font_color("FFFFFF")
            .set_background_color("3BAC52");
        let bold = rust_xlsxwriter::Format::new().set_bold();
        let sheet = book.add_worksheet().set_name("Gastos").map_err(fail)?;
        sheet
            .write_string_with_format(
                0,
                0,
                "retosean · Universidad EAN",
                &rust_xlsxwriter::Format::new()
                    .set_bold()
                    .set_font_color("3BAC52"),
            )
            .map_err(fail)?;
        sheet
            .write_string_with_format(
                1,
                0,
                clean(&format!("Presupuesto del reto: {title}")),
                &rust_xlsxwriter::Format::new().set_bold().set_font_size(14),
            )
            .map_err(fail)?;
        sheet
            .write_string(2, 0, format!("Generado el {generated}"))
            .map_err(fail)?;
        let mut row = 4u32;
        for (name, value) in summary {
            sheet
                .write_string_with_format(row, 0, name, &bold)
                .map_err(fail)?;
            sheet.write_number(row, 1, number(value)).map_err(fail)?;
            row += 1;
        }
        row += 1;
        for (c, name) in ["Categoria", "Comprometido", "Ejecutado", "Total"]
            .iter()
            .enumerate()
        {
            sheet
                .write_string_with_format(row, c as u16, *name, &head)
                .map_err(fail)?;
        }
        for (category, c, e) in &by_category {
            row += 1;
            sheet
                .write_string(row, 0, label("categoria", category))
                .map_err(fail)?;
            sheet.write_number(row, 1, number(*c)).map_err(fail)?;
            sheet.write_number(row, 2, number(*e)).map_err(fail)?;
            sheet.write_number(row, 3, number(*c + *e)).map_err(fail)?;
        }
        row += 2;
        for (c, name) in [
            "Fecha",
            "Categoria",
            "Estado",
            "Descripcion",
            "Monto",
            "Registrado por",
            "Comprobante",
        ]
        .iter()
        .enumerate()
        {
            sheet
                .write_string_with_format(row, c as u16, *name, &head)
                .map_err(fail)?;
        }
        for (date, category, estado, description, amount, by, receipt) in &expenses {
            row += 1;
            // Strings only for text: "=1+1" stays literal, never a formula.
            sheet.write_string(row, 0, date).map_err(fail)?;
            sheet
                .write_string(row, 1, label("categoria", category))
                .map_err(fail)?;
            sheet
                .write_string(row, 2, label("estado", estado))
                .map_err(fail)?;
            sheet
                .write_string(row, 3, clean(description))
                .map_err(fail)?;
            sheet.write_number(row, 4, number(*amount)).map_err(fail)?;
            sheet.write_string(row, 5, by).map_err(fail)?;
            sheet
                .write_string(row, 6, if *receipt { "Si" } else { "No" })
                .map_err(fail)?;
        }
        for (c, w) in [16, 18, 16, 40, 14, 18, 12].iter().enumerate() {
            sheet.set_column_width(c as u16, *w as f64).map_err(fail)?;
        }
        return Ok((
            [
                (
                    axum::http::header::CONTENT_TYPE,
                    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string(),
                ),
                (
                    axum::http::header::CONTENT_DISPOSITION,
                    format!("attachment; filename={filename}"),
                ),
            ],
            book.save_to_buffer().map_err(fail)?,
        )
            .into_response());
    }
    let pesos = |d: Decimal| format!("${}", d.round_dp(2));
    let summary_rows = vec![
        summary
            .iter()
            .map(|(name, value)| {
                if *name == "% asignado" {
                    format!("{:.1}%", value)
                } else {
                    pesos(*value)
                }
            })
            .collect::<Vec<_>>(),
    ];
    let summary_columns: Vec<(&str, f32)> = summary.iter().map(|(name, _)| (*name, 1.)).collect();
    let category_rows: Vec<Vec<String>> = by_category
        .iter()
        .map(|(category, c, e)| {
            vec![
                label("categoria", category),
                pesos(*c),
                pesos(*e),
                pesos(*c + *e),
            ]
        })
        .collect();
    let expense_rows: Vec<Vec<String>> = expenses
        .iter()
        .map(|(date, category, estado, description, amount, _, _)| {
            vec![
                date.clone(),
                label("categoria", category),
                label("estado", estado),
                clean(if description.is_empty() {
                    "-"
                } else {
                    description
                }),
                pesos(*amount),
            ]
        })
        .collect();
    let sections = [
        crate::reports::Section {
            heading: None,
            columns: &summary_columns,
            rows: &summary_rows,
            empty: "",
        },
        crate::reports::Section {
            heading: Some("Totales por categoria"),
            columns: &[
                ("Categoria", 1.),
                ("Comprometido", 1.),
                ("Ejecutado", 1.),
                ("Total", 1.),
            ],
            rows: &category_rows,
            empty: "",
        },
        crate::reports::Section {
            heading: Some("Detalle de gastos"),
            columns: &[
                ("Fecha", 22.),
                ("Categoria", 28.),
                ("Estado", 24.),
                ("Descripcion", 76.),
                ("Monto", 30.),
            ],
            rows: &expense_rows,
            empty: "Sin gastos registrados.",
        },
    ];
    let pdf = crate::reports::pdf_document(
        &format!("Presupuesto del reto: {title}"),
        &[format!("Generado el {generated}")],
        &sections,
        "",
    );
    Ok((
        [
            (
                axum::http::header::CONTENT_TYPE,
                "application/pdf".to_string(),
            ),
            (
                axum::http::header::CONTENT_DISPOSITION,
                format!("attachment; filename={filename}"),
            ),
        ],
        pdf,
    )
        .into_response())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn money_preserves_cents_and_rejects_invalid_amounts() {
        assert_eq!(money("100.25").unwrap().to_string(), "100.25");
        for v in ["-1", "1.001", "NaN", "10000000000"] {
            assert!(money(v).is_err());
        }
    }
}
