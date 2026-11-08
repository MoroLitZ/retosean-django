//! `import-django <db> <media_dir>`: copy an existing Django database and its
//! media into an empty Rust database. Both share the Django-generated schema, so
//! rows move as JSON per table; file fields get Rust's flat uuid keys.
use sqlx::{Connection, PgConnection, PgPool, Postgres, Transaction};
use std::path::{Path, PathBuf};

type Error = Box<dyn std::error::Error>;

/// Download keys Rust serves (media::safe_key); other files are reported.
const EXTENSIONS: &[&str] = &["pdf", "png", "jpg", "txt", "xlsx", "zip"];

/// The QA-package-F integrity rules (CHECKs, UNIQUEs and the document-type FK)
/// are defined VALID inline in schema.sql. A legacy Django database may contain
/// rows that violate them, so import-django drops them before copying and
/// re-adds them NOT VALID (with dedup) afterwards — the same reconciliation the
/// `migrate` command performs on a pre-existing database. Constraints are left
/// off while files are copied because the file `UPDATE`s would otherwise
/// re-validate grandfathered rows.
const RELAX_INTEGRITY: &str = r#"
ALTER TABLE "presupuestos" DROP CONSTRAINT IF EXISTS "presupuestos_monto_total_nonnegative";
ALTER TABLE "gastos" DROP CONSTRAINT IF EXISTS "gastos_monto_nonnegative";
ALTER TABLE "entregables" DROP CONSTRAINT IF EXISTS "entregables_puntaje_maximo_nonnegative";
ALTER TABLE "entregables" DROP CONSTRAINT IF EXISTS "entregables_nota_range";
ALTER TABLE "evaluaciones" DROP CONSTRAINT IF EXISTS "evaluaciones_nota_range";
ALTER TABLE "evaluacion_criteriorubrica" DROP CONSTRAINT IF EXISTS "evaluacion_criteriorubrica_puntaje_maximo_nonnegative";
ALTER TABLE "evaluacion_evaluacioncriterio" DROP CONSTRAINT IF EXISTS "evaluacion_evaluacioncriterio_puntaje_nonnegative";
ALTER TABLE "votaciones_hackaton" DROP CONSTRAINT IF EXISTS "votaciones_hackaton_puntaje_range";
ALTER TABLE "cierre_encuestasatisfaccion" DROP CONSTRAINT IF EXISTS "cierre_encuestasatisfaccion_calificacion_range";
ALTER TABLE "seguimientos_reto" DROP CONSTRAINT IF EXISTS "seguimientos_reto_porcentaje_avance_max";
ALTER TABLE "retos" DROP CONSTRAINT IF EXISTS "retos_fechas_tentativas_order";
ALTER TABLE "etapas_hackaton" DROP CONSTRAINT IF EXISTS "etapas_hackaton_fechas_order";
ALTER TABLE "documentos_empresas" DROP CONSTRAINT IF EXISTS "documentos_empresas_fechas_order";
ALTER TABLE "retos" DROP CONSTRAINT IF EXISTS "retos_estado_check";
ALTER TABLE "integraciones_academicas" DROP CONSTRAINT IF EXISTS "integraciones_academicas_estado_check";
ALTER TABLE "hackathons" DROP CONSTRAINT IF EXISTS "hackathons_estado_check";
ALTER TABLE "postulaciones" DROP CONSTRAINT IF EXISTS "postulaciones_estado_check";
ALTER TABLE "entregables" DROP CONSTRAINT IF EXISTS "entregables_estado_check";
ALTER TABLE "gastos" DROP CONSTRAINT IF EXISTS "gastos_estado_check";
ALTER TABLE "documentos_empresas" DROP CONSTRAINT IF EXISTS "documentos_empresas_estado_check";
ALTER TABLE "cierre_solicitudlogistica" DROP CONSTRAINT IF EXISTS "cierre_solicitudlogistica_estado_check";
ALTER TABLE "inscripciones_hackaton" DROP CONSTRAINT IF EXISTS "inscripciones_hackaton_estado_check";
DROP INDEX IF EXISTS "usuarios_email_ci_uniq";
DROP INDEX IF EXISTS "jurados_hackaton_usuario_uniq";
DROP INDEX IF EXISTS "jurados_hackaton_email_ci_uniq";
ALTER TABLE "equipos" DROP CONSTRAINT IF EXISTS "equipos_reto_nombre_unique";
ALTER TABLE "equipos_reto_academico" DROP CONSTRAINT IF EXISTS "equipos_reto_academico_reto_nombre_equipo_unique";
ALTER TABLE "documentos_empresas" DROP CONSTRAINT IF EXISTS "documentos_empresas_tipo_documento_fk_tipos_documento_legal";
-- QA package S: cross-table integrity triggers (N20, N51, N58, N59, N60) must not
-- fire while legacy rows are copied; they are re-created after the copy.
DROP TRIGGER IF EXISTS "trg_votaciones_hackaton_integridad" ON "votaciones_hackaton";
DROP TRIGGER IF EXISTS "trg_evaluacioncriterio_puntaje_maximo" ON "evaluacion_evaluacioncriterio";
DROP TRIGGER IF EXISTS "trg_inscripciones_hackaton_reto" ON "inscripciones_hackaton";
DROP TRIGGER IF EXISTS "trg_entregables_equipo_reto" ON "entregables";
DROP TRIGGER IF EXISTS "trg_estudiantes_usuario_rol" ON "estudiantes";
DROP TRIGGER IF EXISTS "trg_profesores_usuario_rol" ON "profesores";
DROP TRIGGER IF EXISTS "trg_postulaciones_estudiante_rol" ON "postulaciones";
DROP TRIGGER IF EXISTS "trg_entregables_estudiante_rol" ON "entregables";
DROP TRIGGER IF EXISTS "trg_miembros_equipo_estudiante_rol" ON "miembros_equipo";
DROP TRIGGER IF EXISTS "trg_equipos_academicos_estudiante_rol" ON "equipos_reto_academico_estudiantes";
DROP TRIGGER IF EXISTS "trg_integraciones_profesor_rol" ON "integraciones_academicas";
DROP TRIGGER IF EXISTS "trg_evaluaciones_profesor_rol" ON "evaluaciones";
DROP TRIGGER IF EXISTS "trg_rubrica_creador_rol" ON "evaluacion_rubrica";
DROP TRIGGER IF EXISTS "trg_equipos_academicos_profesor_rol" ON "equipos_reto_academico_profesores";
"#;

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
    // Relax the QA-package-F integrity rules before copying: legacy rows need not
    // satisfy them, and they are reconciled (dedup + NOT VALID) after the copy.
    sqlx::raw_sql(RELAX_INTEGRITY).execute(&mut *tx).await?;
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
        // OPS-13: stream each table through a server-side cursor in chunks so
        // a large Django database never fully lands in RAM.
        let mut source_tx = source.begin().await?;
        sqlx::query(&format!(
            "DECLARE retosean_import CURSOR FOR SELECT to_jsonb(t) FROM \"{table}\" t"
        ))
        .execute(&mut *source_tx)
        .await?;
        loop {
            let chunk: Vec<serde_json::Value> =
                sqlx::query_scalar("FETCH 500 FROM retosean_import")
                    .fetch_all(&mut *source_tx)
                    .await?;
            let count = chunk.len() as i64;
            if count == 0 {
                break;
            }
            sqlx::query(&format!("INSERT INTO \"{table}\" ({list}) SELECT {list} FROM jsonb_populate_recordset(NULL::\"{table}\", $1)"))
                .bind(serde_json::Value::Array(chunk))
                .execute(&mut *tx)
                .await?;
            summary.rows += count;
        }
        // Read-only source transaction: the rollback also closes the cursor.
        source_tx.rollback().await?;
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
    // Reconcile the grandfathered rows exactly like `migrate` on an existing DB:
    // dedupe the unique keys, then add the CHECK/FK rules back NOT VALID so new
    // writes are still constrained.
    let restored = match copied {
        Ok(()) => {
            sqlx::raw_sql(include_str!(
                "../sql/migrations/0002_estado_constraints.sql"
            ))
            .execute(&mut *tx)
            .await?;
            sqlx::raw_sql(include_str!("../sql/migrations/0003_unique_and_fk.sql"))
                .execute(&mut *tx)
                .await?;
            sqlx::raw_sql(include_str!(
                "../sql/migrations/0005_integridad_cruzada.sql"
            ))
            .execute(&mut *tx)
            .await?;
            Ok(())
        }
        Err(error) => Err(error),
    };
    let result = match restored {
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

/// seed-demo: every required document of the company verified and in force,
/// each with a placeholder PDF. Idempotent: existing documents are kept.
pub async fn demo_documents(
    tx: &mut Transaction<'_, Postgres>,
    media_root: &Path,
    company: i64,
) -> Result<Vec<PathBuf>, Error> {
    let kinds: Vec<(String, Option<i16>)> = sqlx::query_as("SELECT t.codigo,t.vigencia_dias FROM tipos_documento_legal t WHERE t.activo AND t.obligatorio AND NOT EXISTS(SELECT 1 FROM documentos_empresas d WHERE d.empresa_id=$1 AND d.tipo_documento=t.codigo) ORDER BY t.orden")
        .bind(company)
        .fetch_all(&mut **tx)
        .await?;
    std::fs::create_dir_all(media_root)?;
    let placeholder = crate::reports::pdf_lines(
        "Documento de demostración",
        &["Archivo de ejemplo generado por seed-demo.".into()],
    );
    let mut written = Vec::new();
    for (kind, days) in kinds {
        let key = format!("{}.pdf", uuid::Uuid::new_v4().simple());
        let path = media_root.join(&key);
        if let Err(error) = std::fs::write(&path, &placeholder) {
            for path in written {
                let _ = std::fs::remove_file(path);
            }
            return Err(error.into());
        }
        written.push(path);
        sqlx::query("INSERT INTO documentos_empresas(empresa_id,tipo_documento,archivo,estado,fecha_expedicion,fecha_vencimiento,fecha_carga,motivo_rechazo) VALUES($1,$2,$3,'VERIFICADO',CURRENT_DATE,CURRENT_DATE+$4::int,now(),'')")
            .bind(company).bind(&kind).bind(&key).bind(days.map(i32::from)).execute(&mut **tx).await?;
    }
    Ok(written)
}
