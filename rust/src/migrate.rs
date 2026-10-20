//! `import-django <db> <media_dir>`: copy an existing Django database and its
//! media into an empty Rust database. Both share the Django-generated schema, so
//! rows move as JSON per table; file fields get Rust's flat uuid keys.
use sqlx::{Connection, PgConnection, PgPool, Postgres, Transaction};
use std::path::{Path, PathBuf};

type Error = Box<dyn std::error::Error>;

/// Download keys Rust serves (media::safe_key); other files are reported.
const EXTENSIONS: &[&str] = &["pdf", "png", "jpg", "txt", "xlsx", "zip"];

pub struct Summary {
    pub rows: i64,
    pub tables: usize,
    pub files: usize,
    pub missing: Vec<String>,
    pub skipped: Vec<String>,
}

pub async fn import_django(
    pool: &PgPool,
    config: &crate::config::Config,
    source_db: &str,
    media_dir: &Path,
) -> Result<Summary, Error> {
    let mut source =
        PgConnection::connect_with(&config.database.clone().database(source_db)).await?;
    let mut tx = pool.begin().await?;
    let tables: Vec<String> = sqlx::query_scalar("SELECT table_name::text FROM information_schema.tables WHERE table_schema='public' AND table_type='BASE TABLE' AND table_name NOT LIKE 'rust\\_%' AND table_name NOT IN ('django_session','django_migrations') ORDER BY 1")
        .fetch_all(&mut *tx).await?;
    // Never merge into live data: the target must hold only an empty schema.
    for table in &tables {
        let any: bool = sqlx::query_scalar(&format!("SELECT EXISTS(SELECT 1 FROM \"{table}\")"))
            .fetch_one(&mut *tx)
            .await?;
        if any {
            return Err(format!("import-django requires an empty database (run init-db with a new RUST_DB_NAME); \"{table}\" has rows").into());
        }
    }
    let mut summary = Summary {
        rows: 0,
        tables: 0,
        files: 0,
        missing: vec![],
        skipped: vec![],
    };
    for table in &tables {
        let columns: Vec<String> = sqlx::query_scalar("SELECT column_name::text FROM information_schema.columns WHERE table_schema='public' AND table_name=$1 ORDER BY ordinal_position")
            .bind(table).fetch_all(&mut *tx).await?;
        let present: Vec<String> = sqlx::query_scalar("SELECT column_name::text FROM information_schema.columns WHERE table_schema='public' AND table_name=$1")
            .bind(table).fetch_all(&mut source).await?;
        if present.is_empty() {
            continue; // Not in this Django database (older migrations).
        }
        // Columns the source lacks keep the Rust defaults.
        let shared: Vec<String> = columns
            .into_iter()
            .filter(|c| present.contains(c))
            .map(|c| format!("\"{c}\""))
            .collect();
        let list = shared.join(",");
        // shortcut: one table at a time in memory; fine for Django databases of
        // a few hundred MB, stream with a cursor if they grow beyond that.
        let rows: Vec<serde_json::Value> =
            sqlx::query_scalar(&format!("SELECT to_jsonb(t) FROM \"{table}\" t"))
                .fetch_all(&mut source)
                .await?;
        for chunk in rows.chunks(500) {
            sqlx::query(&format!("INSERT INTO \"{table}\" ({list}) SELECT {list} FROM jsonb_populate_recordset(NULL::\"{table}\", $1)"))
                .bind(serde_json::Value::Array(chunk.to_vec()))
                .execute(&mut *tx)
                .await?;
        }
        summary.rows += rows.len() as i64;
        summary.tables += 1;
        let has_id = shared.iter().any(|c| c == "\"id\"");
        if has_id {
            sqlx::query(&format!("SELECT setval(pg_get_serial_sequence('\"{table}\"','id'), COALESCE(MAX(id),1), MAX(id) IS NOT NULL) FROM \"{table}\" WHERE pg_get_serial_sequence('\"{table}\"','id') IS NOT NULL"))
                .execute(&mut *tx).await?;
        }
    }
    let mut written = Vec::new();
    let copied = copy_files(
        &mut tx,
        &config.media_root,
        media_dir,
        &mut summary,
        &mut written,
    )
    .await;
    let result = match copied {
        Ok(()) => tx.commit().await.map_err(Error::from),
        Err(error) => Err(error),
    };
    if let Err(error) = result {
        for path in written {
            let _ = std::fs::remove_file(path);
        }
        return Err(error);
    }
    Ok(summary)
}

/// Django stores "folder/name.ext" under MEDIA_ROOT; Rust stores "<uuid>.<ext>".
async fn copy_files(
    tx: &mut Transaction<'_, Postgres>,
    media_root: &Path,
    media_dir: &Path,
    summary: &mut Summary,
    written: &mut Vec<PathBuf>,
) -> Result<(), Error> {
    let base = media_dir.canonicalize()?;
    std::fs::create_dir_all(media_root)?;
    let schema: serde_json::Value = serde_json::from_str(include_str!("../schema.json"))?;
    for model in schema.as_array().into_iter().flatten() {
        let table = model["table"].as_str().unwrap_or_default();
        for field in model["fields"].as_array().into_iter().flatten() {
            if !matches!(field["type"].as_str(), Some("FileField" | "ImageField")) {
                continue;
            }
            let column = field["name"].as_str().unwrap_or_default();
            let rows: Vec<(i64, String)> = sqlx::query_as(&format!("SELECT id::bigint,\"{column}\"::text FROM \"{table}\" WHERE COALESCE(\"{column}\",'')<>''"))
                .fetch_all(&mut **tx).await?;
            for (id, name) in rows {
                let label = format!("{table}.{column} #{id}: {name}");
                let ext = name
                    .rsplit_once('.')
                    .map(|(_, e)| e.to_lowercase())
                    .unwrap_or_default();
                let ext = if ext == "jpeg" {
                    "jpg".to_string()
                } else {
                    ext
                };
                if !EXTENSIONS.contains(&ext.as_str()) {
                    summary.skipped.push(label);
                    continue;
                }
                // Stay inside MEDIA_ROOT whatever the stored name says.
                let Ok(path) = base.join(&name).canonicalize() else {
                    summary.missing.push(label);
                    continue;
                };
                if !path.starts_with(&base) || !path.is_file() {
                    summary.missing.push(label);
                    continue;
                }
                let key = format!("{}.{ext}", uuid::Uuid::new_v4().simple());
                let target = media_root.join(&key);
                std::fs::copy(&path, &target)?;
                written.push(target);
                sqlx::query(&format!(
                    "UPDATE \"{table}\" SET \"{column}\"=$2 WHERE id=$1"
                ))
                .bind(id)
                .bind(&key)
                .execute(&mut **tx)
                .await?;
                summary.files += 1;
            }
        }
    }
    Ok(())
}
