//! Regression tests for QA round 2, package M: panics in mail.rs (#93) and the
//! bounded streaming copy of import-django (#126).
//! Run: cargo test --test qa_robustez -- --include-ignored
use retosean::config::Config;
use retosean::mail::{Mailer, event, parse_catalog};
use serde_json::json;
use sqlx::{Connection, PgConnection, postgres::PgPoolOptions};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// An `Arc<Mutex<Vec<..>>>` poisoned by a panic while holding the lock, like a
/// failing task sharing the memory mailer would leave it.
fn poisoned_outbox() -> Arc<Mutex<Vec<(String, String, String)>>> {
    let sent: Arc<Mutex<Vec<(String, String, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let holder = sent.clone();
    let _ = std::thread::spawn(move || {
        let _guard = holder.lock().unwrap();
        panic!("boom");
    })
    .join();
    assert!(sent.is_poisoned(), "the fixture must poison the mutex");
    sent
}

// #93: a poisoned Mailer::Memory mutex must not panic the request handler or
// the outbox worker; the buffer is recovered and keeps recording.
#[tokio::test]
async fn mailer_memory_recupera_un_mutex_envenenado() {
    let sent = poisoned_outbox();
    let mailer = Mailer::Memory(sent.clone());
    mailer
        .send("ana@example.com", "Asunto", "texto plano", "<p>html</p>")
        .await
        .expect("send must not fail on a poisoned mutex");
    mailer
        .send_attachment(
            &["beto@example.com".into()],
            "Informe",
            "cuerpo",
            "informe.pdf",
            vec![1, 2, 3],
            "application/pdf",
        )
        .await
        .expect("send_attachment must not fail on a poisoned mutex");
    let recorded = sent.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(recorded.len(), 2);
    assert_eq!(recorded[0].0, "ana@example.com");
    assert_eq!(recorded[0].2, "texto plano");
    assert_eq!(recorded[1].0, "beto@example.com");
    assert!(recorded[1].2.contains("[adjunto: informe.pdf]"));
}

// #93: the sane mailer path is untouched by the poison recovery.
#[tokio::test]
async fn mailer_memory_sano_sigue_registrando() {
    let sent: Arc<Mutex<Vec<(String, String, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let mailer = Mailer::Memory(sent.clone());
    mailer
        .send("ana@example.com", "Asunto", "texto", "<p>texto</p>")
        .await
        .unwrap();
    assert_eq!(sent.lock().unwrap().len(), 1);
}

// #93: a broken bundled catalog degrades to an empty catalog instead of
// panicking at runtime, and events then use Django's generic fallback.
#[test]
fn catalogo_invalido_degrada_sin_panico() {
    assert_eq!(parse_catalog("{ roto"), json!({}));
    assert_eq!(parse_catalog(""), json!({}));
    // An empty catalog leaves no email-enabled events (nothing goes out until
    // the file is fixed), and every event takes the generic branch: INFO and a
    // humanized title, exactly what the fallback must provide.
    assert_eq!(
        parse_catalog("{ roto").as_object().map(|c| c.len()),
        Some(0)
    );
    assert_eq!(
        event("EVENTO_NUEVO"),
        ("INFO".to_string(), "Evento nuevo".to_string(), true)
    );
    // The bundled catalog still parses as-is.
    let bundled = parse_catalog(include_str!("../events.json"));
    assert!(bundled["POSTULACION_GESTIONADA"]["titulo"].is_string());
}

// #126: import_django streams each table through a server-side cursor in
// FETCH 500 batches; a table larger than one batch imports with the same
// semantics and memory stays bounded.
#[tokio::test]
#[ignore = "requires local PostgreSQL with CREATEDB; creates throwaway databases"]
async fn import_django_copia_en_lotes_acotados() {
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let src = format!("rust_qa_src_{suffix}");
    let dst = format!("rust_qa_dst_{suffix}");
    let media = std::env::temp_dir().join(format!("rust_qa_media_{suffix}"));
    let config = Config::load().unwrap();
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(config.database.clone())
        .await
        .unwrap();
    // import_django reads a second database on the same server, so the fake
    // Django source cannot be just a schema: CREATEDB is required here.
    sqlx::query(&format!("CREATE DATABASE \"{src}\""))
        .execute(&admin)
        .await
        .expect("the import test needs CREATE DATABASE on the dev server");
    sqlx::query(&format!("CREATE DATABASE \"{dst}\""))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;

    // Everything fallible lands in `outcome`; the throwaway databases and the
    // media directory are always dropped before the failure is reported.
    let outcome = import_and_verify(&config, &src, &dst, &media).await;

    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(config.database.clone())
        .await
        .unwrap();
    for database in [&src, &dst] {
        let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS \"{database}\""))
            .execute(&admin)
            .await;
    }
    admin.close().await;
    let _ = std::fs::remove_dir_all(&media);

    outcome.expect("import-django regression (#126)");
}

/// Prepares both throwaway databases and delegates to `run_checks`; always
/// closes the target pool so the caller can drop the databases.
async fn import_and_verify(
    config: &Config,
    src: &str,
    dst: &str,
    media: &Path,
) -> Result<(), String> {
    let dst_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect_with(config.database.clone().database(dst))
        .await
        .map_err(|e| e.to_string())?;
    let result = run_checks(config, src, dst, &dst_pool, media).await;
    dst_pool.close().await;
    result
}

/// The fake Django source, the import and the post-import checks. Errors are
/// strings so the caller cleans up before the failure is reported.
async fn run_checks(
    config: &Config,
    src: &str,
    dst: &str,
    dst_pool: &sqlx::PgPool,
    media: &Path,
) -> Result<(), String> {
    // A small fake Django database: two tables with a subset of Rust's columns
    // (as an older migration would leave them) and 1200 programs, so the copy
    // loop must cross several FETCH 500 batches (#126) with bounded memory.
    let mut source = PgConnection::connect_with(&config.database.clone().database(src))
        .await
        .map_err(|e| e.to_string())?;
    sqlx::raw_sql(
        "CREATE TABLE facultades(id bigint NOT NULL PRIMARY KEY GENERATED BY DEFAULT AS IDENTITY, codigo varchar(20) NOT NULL, nombre varchar(150) NOT NULL UNIQUE);\
         INSERT INTO facultades(id,nombre,codigo) VALUES(1,'Ingeniería','ING');\
         CREATE TABLE programas(id bigint NOT NULL PRIMARY KEY GENERATED BY DEFAULT AS IDENTITY, nombre varchar(150) NOT NULL, nivel varchar(50) NOT NULL, facultad_id bigint NOT NULL);\
         INSERT INTO programas(id,nombre,nivel,facultad_id) SELECT g,'Programa '||g,'Pregrado',1 FROM generate_series(1,1200) g;",
    )
    .execute(&mut source)
    .await
    .map_err(|e| e.to_string())?;
    source.close().await.map_err(|e| e.to_string())?;

    // The target: schema.sql on an empty database, exactly what init-db leaves
    // before a real import-django.
    sqlx::raw_sql(include_str!("../sql/schema.sql"))
        .execute(dst_pool)
        .await
        .map_err(|e| e.to_string())?;
    std::fs::create_dir_all(media.join("django")).map_err(|e| e.to_string())?;
    let mut import_config = config.clone();
    import_config.database = import_config.database.clone().database(dst);
    import_config.media_root = media.join("media_root");

    let summary =
        retosean::migrate::import_django(dst_pool, &import_config, src, &media.join("django"))
            .await
            .map_err(|e| e.to_string())?;
    if summary.tables != 2 {
        return Err(format!(
            "summary.tables = {}, want 2 (only tables present in the source copy)",
            summary.tables
        ));
    }
    if summary.rows != 1201 {
        return Err(format!("summary.rows = {}, want 1201", summary.rows));
    }
    if summary.files != 0 || !summary.missing.is_empty() || !summary.skipped.is_empty() {
        return Err(format!(
            "no files were expected: files={}, missing={:?}, skipped={:?}",
            summary.files, summary.missing, summary.skipped
        ));
    }

    // Rows land intact and the identity sequence moves past them.
    let programs: i64 = scalar(
        dst_pool,
        "SELECT count(*) FROM programas WHERE nombre LIKE 'Programa %' AND facultad_id=1 AND nivel='Pregrado'",
    )
    .await?;
    if programs != 1200 {
        return Err(format!("copied programs = {programs}, want 1200"));
    }
    let faculty: String = scalar(dst_pool, "SELECT nombre FROM facultades WHERE id=1").await?;
    if faculty != "Ingeniería" {
        return Err(format!("facultad 1 = {faculty:?}, want Ingeniería"));
    }
    let next_id: i64 = scalar(
        dst_pool,
        "SELECT nextval(pg_get_serial_sequence('programas','id'))",
    )
    .await?;
    if next_id != 1201 {
        return Err(format!(
            "nextval = {next_id}, want 1201 (setval above the rows)"
        ));
    }
    // The relax/restore integrity steps of the file still run: restored CHECKs
    // stay NOT VALID (grandfathered rows) and the unique indexes are back.
    let validated: bool = scalar(
        dst_pool,
        "SELECT convalidated FROM pg_constraint WHERE conname='gastos_monto_nonnegative'",
    )
    .await?;
    if validated {
        return Err("gastos_monto_nonnegative must be restored NOT VALID".into());
    }
    let unique: bool = scalar(
        dst_pool,
        "SELECT EXISTS(SELECT 1 FROM pg_indexes WHERE indexname='usuarios_email_ci_uniq')",
    )
    .await?;
    if !unique {
        return Err("usuarios_email_ci_uniq must be recreated after the copy".into());
    }
    Ok(())
}

/// One scalar of a query, as a String error for the cleanup-first flow.
async fn scalar<T>(pool: &sqlx::PgPool, sql: &str) -> Result<T, String>
where
    T: for<'r> sqlx::Decode<'r, sqlx::Postgres> + sqlx::Type<sqlx::Postgres> + Send + Unpin,
{
    sqlx::query_scalar(sql)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("{sql}: {e}"))
}
