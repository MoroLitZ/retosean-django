//! Incremental schema changes after `init-db`. Add a file under sql/migrations/
//! and list it here; `migrate` applies each pending one once, in order.
use sqlx::PgPool;

pub const MIGRATIONS: &[(&str, &str)] = &[
    (
        "0001_equipo_reto_academico_integracion",
        include_str!("../sql/migrations/0001_equipo_reto_academico_integracion.sql"),
    ),
    (
        "0002_estado_constraints",
        include_str!("../sql/migrations/0002_estado_constraints.sql"),
    ),
    (
        "0003_unique_and_fk",
        include_str!("../sql/migrations/0003_unique_and_fk.sql"),
    ),
    (
        "0004_hot_indexes",
        include_str!("../sql/migrations/0004_hot_indexes.sql"),
    ),
];

const TABLE: &str = "CREATE TABLE IF NOT EXISTS rust_schema_migrations(name text PRIMARY KEY, applied_at timestamptz NOT NULL DEFAULT now())";

/// Serializes the one-time creation of the `rust_schema_migrations` catalog so
/// two concurrent `migrate` runs cannot race the `CREATE TABLE IF NOT EXISTS`
/// (OPS-11). Once the table exists, per-migration `LOCK TABLE` still serializes.
const BOOTSTRAP_LOCK: i64 = i64::from_be_bytes(*b"RETSEAN1");

/// init-db already holds every listed change: record them as applied.
pub async fn baseline(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>) -> sqlx::Result<()> {
    sqlx::query(TABLE).execute(&mut **tx).await?;
    for (name, _) in MIGRATIONS {
        sqlx::query("INSERT INTO rust_schema_migrations(name) VALUES($1) ON CONFLICT DO NOTHING")
            .bind(name)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

/// Applies pending migrations, each in its own transaction; returns their names.
pub async fn run(pool: &PgPool) -> sqlx::Result<Vec<&'static str>> {
    // Take the advisory lock before creating the bookkeeping table (OPS-11);
    // it releases on commit, so the whole run below still serializes per step.
    {
        let mut tx = pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(BOOTSTRAP_LOCK)
            .execute(&mut *tx)
            .await?;
        sqlx::query(TABLE).execute(&mut *tx).await?;
        tx.commit().await?;
    }
    let mut applied = Vec::new();
    for (name, sql) in MIGRATIONS {
        let mut tx = pool.begin().await?;
        // Serialize concurrent runs; the second one sees the row and skips.
        sqlx::query("LOCK TABLE rust_schema_migrations IN EXCLUSIVE MODE")
            .execute(&mut *tx)
            .await?;
        let done: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM rust_schema_migrations WHERE name=$1)")
                .bind(name)
                .fetch_one(&mut *tx)
                .await?;
        if done {
            continue;
        }
        sqlx::raw_sql(sql).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO rust_schema_migrations(name) VALUES($1)")
            .bind(name)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        applied.push(*name);
    }
    Ok(applied)
}

#[cfg(test)]
mod tests {
    #[test]
    fn migrations_are_ordered_and_unique() {
        let names: Vec<&str> = super::MIGRATIONS.iter().map(|m| m.0).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(names, sorted, "list migrations once, in name order");
    }
}
