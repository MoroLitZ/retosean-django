//! Incremental schema changes after `init-db`. Add a file under sql/migrations/
//! and list it here; `migrate` applies each pending one once, in order.
use sqlx::PgPool;

pub const MIGRATIONS: &[(&str, &str)] = &[
    // ("0001_short_name", include_str!("../sql/migrations/0001_short_name.sql")),
];

const TABLE: &str = "CREATE TABLE IF NOT EXISTS rust_schema_migrations(name text PRIMARY KEY, applied_at timestamptz NOT NULL DEFAULT now())";

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
    sqlx::query(TABLE).execute(pool).await?;
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
