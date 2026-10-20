use crate::{
    AppState, auth, domain,
    error::{AppError, Result},
    workflows::{history, notify},
};
use axum::{
    Json,
    extract::{Multipart, Path, State},
    http::{HeaderMap, header},
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};

pub fn extension(data: &[u8]) -> Result<&'static str> {
    if data.starts_with(b"%PDF-") {
        Ok("pdf")
    } else if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        Ok("png")
    } else if data.starts_with(b"\xff\xd8\xff") {
        Ok("jpg")
    } else if std::str::from_utf8(data).is_ok() && !data.contains(&0) {
        Ok("txt")
    } else {
        Err(AppError::bad(
            "Solo se admiten archivos PDF, PNG, JPG o texto UTF-8",
        ))
    }
}
pub fn safe_key(key: &str) -> bool {
    let Some((name, ext)) = key.split_once('.') else {
        return false;
    };
    name.len() == 32
        && name.bytes().all(|c| c.is_ascii_hexdigit())
        // xlsx: generated reports (HU06); zip, docx, pptx, csv: support files.
        && ["pdf", "png", "jpg", "txt", "xlsx", "zip", "docx", "pptx", "csv"].contains(&ext)
}
fn file_error(error: std::io::Error) -> AppError {
    tracing::error!(%error,"private storage failure");
    AppError(
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        "No se pudo acceder al archivo".into(),
    )
}
pub async fn submit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(reto): Path<i64>,
    mut multipart: Multipart,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    if user.rol != "ESTUDIANTE" {
        return Err(AppError::forbidden());
    }
    let mut title = String::new();
    let mut comment = String::new();
    let mut data = None;
    let mut final_file = false;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::bad("Carga multipart inválida"))?
    {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "archivo" => {
                if data.is_some() {
                    return Err(AppError::bad("Adjunta un solo archivo"));
                }
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|_| AppError::bad("Archivo demasiado grande"))?;
                if bytes.is_empty() || bytes.len() > 9 * 1024 * 1024 {
                    return Err(AppError::bad("El archivo debe tener entre 1 byte y 9 MB"));
                }
                data = Some(bytes);
            }
            "titulo" => {
                title = field
                    .text()
                    .await
                    .map_err(|_| AppError::bad("Título inválido"))?
            }
            "comentario" => {
                comment = field
                    .text()
                    .await
                    .map_err(|_| AppError::bad("Comentario inválido"))?
            }
            "es_final" => final_file = field.text().await.unwrap_or_default() == "true",
            _ => return Err(AppError::bad("Campo de carga desconocido")),
        }
    }
    domain::validate_text(&title, 180, "Título")?;
    if comment.len() > 10000 {
        return Err(AppError::bad("Comentario demasiado largo"));
    }
    let data = data.ok_or_else(|| AppError::bad("Adjunta un archivo"))?;
    let key = format!("{}.{}", uuid::Uuid::new_v4().simple(), extension(&data)?);
    let mut tx = state.pool.begin().await?;
    let eligible:Option<i64>=sqlx::query_scalar("SELECT id FROM retos WHERE id=$1 AND eliminado_en IS NULL AND estado IN ('abierto','en_curso') FOR UPDATE").bind(reto).fetch_optional(&mut *tx).await?;
    if eligible.is_none() {
        return Err(AppError::bad("El reto no admite entregas"));
    }
    // _estudiante_participa_en: accepted, on an academic team, or already submitted.
    let accepted:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM postulaciones WHERE reto_id=$1 AND estudiante_id=$2 AND estado='ACEPTADA') OR EXISTS(SELECT 1 FROM equipos_reto_academico_estudiantes m JOIN equipos_reto_academico q ON q.id=m.equiporetoacademico_id WHERE q.reto_id=$1 AND m.usuario_id=$2) OR EXISTS(SELECT 1 FROM entregables WHERE reto_id=$1 AND estudiante_id=$2)").bind(reto).bind(user.id).fetch_one(&mut *tx).await?;
    if !accepted {
        return Err(AppError::forbidden());
    }
    let previous:Option<(i64,String)>=sqlx::query_as("SELECT id,estado FROM entregables WHERE reto_id=$1 AND estudiante_id=$2 AND titulo=$3 FOR UPDATE").bind(reto).bind(user.id).bind(&title).fetch_optional(&mut *tx).await?;
    let id:i64=sqlx::query_scalar("INSERT INTO entregables(reto_id,estudiante_id,titulo,es_final,puntaje_maximo,archivo,comentario_estudiante,fecha_entrega,actualizado_en,estado) VALUES($1,$2,$3,$4,5.00,$5,$6,now(),now(),'ENVIADO') ON CONFLICT(reto_id,estudiante_id,titulo) DO UPDATE SET archivo=excluded.archivo,comentario_estudiante=excluded.comentario_estudiante,es_final=excluded.es_final,estado='ENVIADO',nota=NULL,comentario_profesor=NULL,actualizado_en=now() RETURNING id")
        .bind(reto).bind(user.id).bind(&title).bind(final_file).bind(&key).bind(&comment).fetch_one(&mut *tx).await?;
    // A replacement invalidates old scores; history remains in the audit trail.
    if previous.is_some() {
        sqlx::query("DELETE FROM evaluacion_evaluacioncriterio WHERE evaluacion_id IN (SELECT id FROM evaluaciones WHERE entregable_id=$1)").bind(id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM evaluaciones WHERE entregable_id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    history(
        &mut tx,
        ("evaluacion", "entregable"),
        id,
        previous.as_ref().map(|(_, s)| s.as_str()).unwrap_or(""),
        "ENVIADO",
        user.id,
        "Archivo entregado desde Rust",
    )
    .await?;
    let teachers:Vec<i64>=sqlx::query_scalar("SELECT DISTINCT profesor_id FROM integraciones_academicas WHERE reto_id=$1 AND estado='publicada'").bind(reto).fetch_all(&mut *tx).await?;
    for teacher in teachers {
        notify(
            &mut tx,
            teacher,
            "ENTREGABLE_RECIBIDO",
            &format!("{} entregó {title}", user.username),
            &format!("entregable:{id}:{}", uuid::Uuid::new_v4().simple()),
        )
        .await?;
    }
    tokio::fs::create_dir_all(&state.config.media_root)
        .await
        .map_err(file_error)?;
    let path = state.config.media_root.join(&key);
    tokio::fs::write(&path, &data).await.map_err(file_error)?;
    if let Err(error) = tx.commit().await {
        let _ = tokio::fs::remove_file(path).await;
        return Err(error.into());
    }
    Ok(Json(json!({"id":id,"estado":"ENVIADO"})))
}
pub async fn download(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((name, id, field)): Path<(String, i64, String)>,
) -> Result<Response> {
    let user = auth::current(&state, &headers, false).await?;
    let meta = crate::resources::resource(&name)?;
    let allowed = meta["fields"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["name"] == field && f["type"] == "FileField");
    if !allowed {
        return Err(AppError::not_found());
    }
    let row = crate::resources::visible_record(&state, &user, &name, id).await?;
    let key = row[&field].as_str().ok_or_else(AppError::not_found)?;
    if !safe_key(key) {
        return Err(AppError::not_found());
    }
    let data = tokio::fs::read(state.config.media_root.join(key))
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                AppError::not_found()
            } else {
                file_error(e)
            }
        })?;
    let extension = key.rsplit('.').next().unwrap_or("bin");
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=retosean-adjunto.{extension}"),
            ),
        ],
        data,
    )
        .into_response())
}
pub async fn company_document(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    multipart: Multipart,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    if user.rol != "EMPRESA" {
        return Err(AppError::forbidden());
    }
    let upload = read_document(multipart).await?;
    let mut tx = state.pool.begin().await?;
    let row:Option<(i64,String,bool)>=sqlx::query_as("SELECT e.id,d.tipo_documento,COALESCE(t.requiere_permiso_especial,false) FROM documentos_empresas d JOIN empresas e ON e.id=d.empresa_id LEFT JOIN tipos_documento_legal t ON t.codigo=d.tipo_documento WHERE d.id=$1 AND e.usuario_id=$2 FOR UPDATE OF d").bind(id).bind(user.id).fetch_optional(&mut *tx).await?;
    let (company, kind, special) = row.ok_or_else(AppError::not_found)?;
    // Listas Informa and other special types are uploaded by the university (HU13).
    if special {
        return Err(AppError::forbidden());
    }
    store_document(&state, tx, &user, company, &kind, upload).await
}
/// The company's own upload by catalog type (empresas:documentos): created on the
/// first upload, reloaded afterwards; special types stay with the university.
pub async fn own_document(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(kind): Path<String>,
    multipart: Multipart,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    if user.rol != "EMPRESA" {
        return Err(AppError::forbidden());
    }
    let upload = read_document(multipart).await?;
    let tx = state.pool.begin().await?;
    let row: Option<(i64, bool)> = sqlx::query_as("SELECT e.id,t.requiere_permiso_especial FROM empresas e JOIN tipos_documento_legal t ON t.codigo=$2 AND t.activo WHERE e.usuario_id=$1")
        .bind(user.id).bind(&kind).fetch_optional(&state.pool).await?;
    let (company, special) = row.ok_or_else(AppError::not_found)?;
    if special {
        return Err(AppError::forbidden());
    }
    store_document(&state, tx, &user, company, &kind, upload).await
}
/// cargar_documento_especial: holders of empresas.cargar_listas_informa upload
/// special-permission documents for any company.
pub async fn special_document(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((company, kind)): Path<(i64, String)>,
    multipart: Multipart,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    if !crate::permissions::has_perm(&state.pool, &user, "empresas.cargar_listas_informa").await? {
        return Err(AppError::forbidden());
    }
    let upload = read_document(multipart).await?;
    let tx = state.pool.begin().await?;
    let special: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tipos_documento_legal WHERE codigo=$1 AND activo AND requiere_permiso_especial) AND EXISTS(SELECT 1 FROM empresas WHERE id=$2)").bind(&kind).bind(company).fetch_one(&state.pool).await?;
    if !special {
        return Err(AppError::not_found());
    }
    store_document(&state, tx, &user, company, &kind, upload).await
}
type Upload = (axum::body::Bytes, Option<chrono::NaiveDate>);
async fn read_document(mut multipart: Multipart) -> Result<Upload> {
    let mut data = None;
    let mut issued = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::bad("Carga inválida"))?
    {
        match field.name().unwrap_or("") {
            "archivo" => {
                if data.is_some() {
                    return Err(AppError::bad("Adjunta un solo archivo"));
                }
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|_| AppError::bad("Archivo demasiado grande"))?;
                if bytes.is_empty() || bytes.len() > 9 * 1024 * 1024 {
                    return Err(AppError::bad("Archivo de máximo 9 MB"));
                }
                data = Some(bytes);
            }
            "fecha_expedicion" => {
                let text = field
                    .text()
                    .await
                    .map_err(|_| AppError::bad("Fecha inválida"))?;
                if !text.is_empty() {
                    issued = Some(
                        text.parse::<chrono::NaiveDate>()
                            .map_err(|_| AppError::bad("Fecha de expedición inválida"))?,
                    );
                }
            }
            // The expiry comes from the catalog (fecha_vencimiento_para), never the client.
            "fecha_vencimiento" => {}
            _ => return Err(AppError::bad("Campo desconocido")),
        }
    }
    Ok((
        data.ok_or_else(|| AppError::bad("Adjunta un archivo"))?,
        issued,
    ))
}
/// cargar_documento + CargarDocumentoForm: create or reload the document in CARGADO.
async fn store_document(
    state: &AppState,
    mut tx: sqlx::Transaction<'_, sqlx::Postgres>,
    user: &auth::User,
    company: i64,
    kind: &str,
    (data, issued): Upload,
) -> Result<Json<Value>> {
    let ext = extension(&data)?;
    // DocumentoEmpresa.EXTENSIONES_PERMITIDAS: legal documents are PDF only.
    if ext != "pdf" {
        return Err(AppError::bad("Los documentos legales deben ser PDF"));
    }
    let catalog: Option<(Option<i16>, String)> =
        sqlx::query_as("SELECT vigencia_dias,nombre FROM tipos_documento_legal WHERE codigo=$1")
            .bind(kind)
            .fetch_optional(&mut *tx)
            .await?;
    let (validity, name) = catalog.unwrap_or((None, kind.to_string()));
    let issued =
        issued.ok_or_else(|| AppError::bad("Indica la fecha de expedición del documento."))?;
    let today = (chrono::Utc::now() - chrono::Duration::hours(5)).date_naive(); // Bogota, no DST
    if issued > today {
        return Err(AppError::bad("La fecha de expedición no puede ser futura."));
    }
    let mut expires = None;
    if let Some(days) = validity.filter(|d| *d > 0) {
        let age = (today - issued).num_days();
        if age > days as i64 {
            return Err(AppError::bad(format!(
                "{name} tiene {age} dias de expedida y el maximo es {days}."
            )));
        }
        expires = Some(issued + chrono::Duration::days(days as i64));
    }
    let key = format!("{}.{}", uuid::Uuid::new_v4().simple(), ext);
    let existing: Option<(i64, String)> = sqlx::query_as(
        "SELECT id,estado FROM documentos_empresas WHERE empresa_id=$1 AND tipo_documento=$2 FOR UPDATE",
    )
    .bind(company)
    .bind(kind)
    .fetch_optional(&mut *tx)
    .await?;
    let (id, from, comment) = match existing {
        Some((id, from)) => {
            domain::transition("MAQUINA_DOCUMENTO", &from, "CARGADO")?;
            sqlx::query("UPDATE documentos_empresas SET archivo=$2,estado='CARGADO',fecha_expedicion=$3,fecha_vencimiento=$4,motivo_rechazo='',fecha_carga=now() WHERE id=$1").bind(id).bind(&key).bind(issued).bind(expires).execute(&mut *tx).await?;
            (id, from, "Documento recargado.")
        }
        None => {
            let id: i64 = sqlx::query_scalar("INSERT INTO documentos_empresas(empresa_id,tipo_documento,archivo,estado,fecha_expedicion,fecha_vencimiento,fecha_carga,motivo_rechazo) VALUES($1,$2,$3,'CARGADO',$4,$5,now(),'') RETURNING id").bind(company).bind(kind).bind(&key).bind(issued).bind(expires).fetch_one(&mut *tx).await?;
            (id, String::new(), "Documento cargado.")
        }
    };
    history(
        &mut tx,
        ("empresas", "documentoempresa"),
        id,
        &from,
        "CARGADO",
        user.id,
        comment,
    )
    .await?;
    crate::companies::recompute_validation(&mut tx, company).await?;
    // The company's own upload tells the admins (empresas:documentos); the
    // special upload by the university does not.
    if user.rol == "EMPRESA" {
        let company_name: String =
            sqlx::query_scalar("SELECT razon_social FROM empresas WHERE id=$1")
                .bind(company)
                .fetch_one(&mut *tx)
                .await?;
        let message = format!(
            "La empresa {company_name} {} el documento \"{name}\" para revision.",
            if from.is_empty() {
                "cargo"
            } else {
                "actualizo"
            }
        );
        let admins: Vec<i64> = sqlx::query_scalar(
            "SELECT id FROM usuarios WHERE is_active AND (is_superuser OR rol='ADMIN') AND id<>$1",
        )
        .bind(user.id)
        .fetch_all(&mut *tx)
        .await?;
        for admin in admins {
            notify(&mut tx, admin, "DOCUMENTO_CARGADO_ADMIN", &message, "").await?;
        }
    }
    tokio::fs::create_dir_all(&state.config.media_root)
        .await
        .map_err(file_error)?;
    let path = state.config.media_root.join(key);
    tokio::fs::write(&path, &data).await.map_err(file_error)?;
    if let Err(error) = tx.commit().await {
        let _ = tokio::fs::remove_file(path).await;
        return Err(error.into());
    }
    Ok(Json(json!({"id":id,"estado":"CARGADO"})))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_cannot_escape_private_storage() {
        for key in [
            "../secret.pdf",
            "C:\\secret.pdf",
            "a/b.pdf",
            "../../.env",
            "",
        ] {
            assert!(!safe_key(key));
        }
        assert!(safe_key("09df6e9134944ef89f4af98922ae8322.pdf"));
    }
    #[test]
    fn formats_are_detected_from_content() {
        assert_eq!(extension(b"%PDF-1.4").unwrap(), "pdf");
        assert_eq!(extension(b"text").unwrap(), "txt");
        assert!(extension(b"\0\xff\0").is_err());
    }
}

/// MultipleFileField: up to 50 MB per file (retos and follow-ups).
pub const ATTACHMENT_LIMIT: usize = 50 * 1024 * 1024;
/// Support files keep a served extension; Django accepts any file, Rust only
/// the types its download endpoint serves.
pub fn attachment_extension(name: &str) -> Result<&'static str> {
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_lowercase())
        .unwrap_or_default();
    Ok(match ext.as_str() {
        "pdf" => "pdf",
        "png" => "png",
        "jpg" | "jpeg" => "jpg",
        "txt" => "txt",
        "xlsx" => "xlsx",
        "zip" => "zip",
        "docx" => "docx",
        "pptx" => "pptx",
        "csv" => "csv",
        _ => {
            return Err(AppError::bad(format!(
                "{name}: tipo de archivo no permitido (PDF, imagen, texto, Excel, Word, PowerPoint, CSV o ZIP)."
            )));
        }
    })
}
/// Writes validated attachments; returns (key, original name, size, path).
pub async fn save_attachments(
    state: &AppState,
    files: &[(String, axum::body::Bytes)],
) -> Result<Vec<(String, String, i64, std::path::PathBuf)>> {
    for (name, data) in files {
        if data.len() > ATTACHMENT_LIMIT {
            return Err(AppError::bad(format!("{name} supera el limite de 50 MB.")));
        }
        attachment_extension(name)?;
    }
    tokio::fs::create_dir_all(&state.config.media_root)
        .await
        .map_err(file_error)?;
    let mut saved = Vec::new();
    for (name, data) in files {
        let key = format!(
            "{}.{}",
            uuid::Uuid::new_v4().simple(),
            attachment_extension(name)?
        );
        let path = state.config.media_root.join(&key);
        if let Err(error) = tokio::fs::write(&path, data).await {
            for (_, _, _, written) in &saved {
                let _ = tokio::fs::remove_file(written).await;
            }
            return Err(file_error(error));
        }
        let original: String = name.chars().take(255).collect();
        saved.push((key, original, data.len() as i64, path));
    }
    Ok(saved)
}
/// Text fields plus every file under `file_field` (retos and follow-ups).
pub async fn read_with_files(
    mut multipart: Multipart,
    file_field: &str,
) -> Result<(
    std::collections::HashMap<String, String>,
    Vec<(String, axum::body::Bytes)>,
)> {
    let mut fields = std::collections::HashMap::new();
    let mut files = Vec::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::bad("Carga inválida"))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == file_field {
            let original = field.file_name().unwrap_or("archivo").to_string();
            let bytes = field
                .bytes()
                .await
                .map_err(|_| AppError::bad(format!("{original} supera el limite de 50 MB.")))?;
            if !bytes.is_empty() {
                files.push((original, bytes));
            }
        } else {
            let text = field
                .text()
                .await
                .map_err(|_| AppError::bad("Campo inválido"))?;
            fields.insert(name, text);
        }
    }
    Ok((fields, files))
}
/// RetoForm.archivos: the owning company while it can edit the reto, or an admin.
pub async fn reto_files(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(reto): Path<i64>,
    multipart: Multipart,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    let (_, files) = read_with_files(multipart, "archivos").await?;
    if files.is_empty() {
        return Err(AppError::bad("Adjunta al menos un archivo"));
    }
    let mut tx = state.pool.begin().await?;
    let row: Option<(i64, String)> = sqlx::query_as(
        "SELECT empresa_id,estado FROM retos WHERE id=$1 AND eliminado_en IS NULL FOR UPDATE",
    )
    .bind(reto)
    .fetch_optional(&mut *tx)
    .await?;
    let (owner, estado) = row.ok_or_else(AppError::not_found)?;
    let editable = user.rol == "EMPRESA"
        && owner == user.id
        && ["borrador", "rechazado"].contains(&estado.as_str());
    if !user.admin() && !editable {
        return Err(AppError::forbidden());
    }
    let saved = save_attachments(&state, &files).await?;
    for (key, original, size, _) in &saved {
        sqlx::query("INSERT INTO retos_retoarchivo(reto_id,archivo,nombre_original,tamano,creado_en) VALUES($1,$2,$3,$4,now())")
            .bind(reto).bind(key).bind(original).bind(size).execute(&mut *tx).await?;
    }
    if let Err(error) = tx.commit().await {
        for (_, _, _, path) in saved {
            let _ = tokio::fs::remove_file(path).await;
        }
        return Err(error.into());
    }
    Ok(Json(json!({"archivos": saved.len()})))
}
