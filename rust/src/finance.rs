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
use rust_decimal::{Decimal, prelude::FromPrimitive};
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
    let gasto:i64=sqlx::query_scalar("INSERT INTO gastos(presupuesto_id,categoria,monto,descripcion,registrado_por_id,creado_en) VALUES($1,$2,$3,$4,$5,now()) RETURNING id").bind(id).bind(&input.categoria).bind(amount).bind(&input.descripcion).bind(user.id).fetch_one(&mut *tx).await?;
    check_threshold(&mut tx, id).await?;
    tx.commit().await?;
    Ok(Json(json!({"id":gasto,"monto":amount.to_string()})))
}
async fn check_threshold(tx: &mut Transaction<'_, Postgres>, id: i64) -> Result<()> {
    let (total,alert,company):(Decimal,bool,i64)=sqlx::query_as("SELECT b.monto_total,b.alerta_80_enviada,r.empresa_id FROM presupuestos b JOIN retos r ON r.id=b.reto_id WHERE b.id=$1 FOR UPDATE OF b").bind(id).fetch_one(&mut **tx).await?;
    let spent: Decimal =
        sqlx::query_scalar("SELECT COALESCE(sum(monto),0) FROM gastos WHERE presupuesto_id=$1")
            .bind(id)
            .fetch_one(&mut **tx)
            .await?;
    let exceeded = total > Decimal::ZERO && spent * Decimal::from(100) >= total * Decimal::from(80);
    if exceeded && !alert {
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
                &format!(
                    "Presupuesto {id}: ejecutado {spent} de {total}. Disponible: {}",
                    total - spent
                ),
                &format!("presupuesto:{id}:80"),
            )
            .await?;
        }
    }
    sqlx::query("UPDATE presupuestos SET alerta_80_enviada=$2,actualizado_en=now() WHERE id=$1")
        .bind(id)
        .bind(exceeded)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
pub async fn summary(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, false).await?;
    crate::resources::visible_record(&state, &user, "presupuestos", id).await?;
    let (total,spent):(Decimal,Decimal)=sqlx::query_as("SELECT b.monto_total,COALESCE(sum(g.monto),0) FROM presupuestos b LEFT JOIN gastos g ON g.presupuesto_id=b.id WHERE b.id=$1 GROUP BY b.id").bind(id).fetch_one(&state.pool).await?;
    let percent = if total == Decimal::ZERO {
        Decimal::ZERO
    } else {
        spent / total * Decimal::from_u32(100).unwrap()
    };
    Ok(Json(
        json!({"total":total.to_string(),"ejecutado":spent.to_string(),"disponible":(total-spent).to_string(),"porcentaje":percent.round_dp(2).to_string(),"alerta":percent>=Decimal::from(80)}),
    ))
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
