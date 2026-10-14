use retosean::{AppState, config::Config, router};
use sqlx::{Connection, Executor, PgConnection, postgres::PgPoolOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "retosean=info".into()),
        )
        .init();
    let config = Config::load()?;
    let command = std::env::args().nth(1).unwrap_or_else(|| "serve".into());
    if command == "init-db" {
        let mut admin =
            PgConnection::connect_with(&config.database.clone().database("postgres")).await?;
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname=$1)")
                .bind(&config.database_name)
                .fetch_one(&mut admin)
                .await?;
        if !exists {
            admin
                .execute(format!("CREATE DATABASE \"{}\"", config.database_name).as_str())
                .await?;
        }
        let mut db = PgConnection::connect_with(&config.database).await?;
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM information_schema.tables WHERE table_schema='public'",
        )
        .fetch_one(&mut db)
        .await?;
        if count != 0 {
            return Err(
                "init-db requires an empty database; no existing tables were changed".into(),
            );
        }
        let mut tx = db.begin().await?;
        sqlx::raw_sql(include_str!("../sql/schema.sql"))
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        println!("Created Rust schema in {}", config.database_name);
        return Ok(());
    }
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect_with(config.database.clone())
        .await?;
    if command == "seed-demo" {
        let mut tx = pool.begin().await?;
        sqlx::query("LOCK TABLE usuarios IN EXCLUSIVE MODE")
            .execute(&mut *tx)
            .await?;
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM usuarios")
            .fetch_one(&mut *tx)
            .await?;
        if count != 0 {
            return Err(
                "seed-demo requires an empty usuarios table; existing accounts were preserved"
                    .into(),
            );
        }
        let password = retosean::auth::hash_password("RetosEAN2026");
        sqlx::raw_sql(
            &include_str!("../sql/demo.sql").replace("__DEMO_PASSWORD_HASH__", &password),
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        println!(
            "Demo created. Accounts: admin.demo, empresa.verificada, profesor.demo, estudiante.demo, experto.demo. Password: RetosEAN2026"
        );
        return Ok(());
    }
    if command != "serve" {
        return Err("Commands: serve | init-db | seed-demo".into());
    }
    retosean::auth::init(&pool).await?;
    let listener = tokio::net::TcpListener::bind(config.bind).await?;
    tracing::info!(address=%config.bind, database=%config.database_name, "RetosEAN Rust ready");
    axum::serve(
        listener,
        router(AppState {
            pool,
            config,
            password_slots: std::sync::Arc::new(tokio::sync::Semaphore::new(4)),
        }),
    )
    .with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await?;
    Ok(())
}
