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

/// Allowed content-derived extensions. HTML/SVG/XML active content is rejected
/// even when disguised as plain text, so uploads cannot be served inline as a
/// browser-rendered page (ACA-12 / MED-4 follow-up).
/// Stream a multipart field up to `limit` bytes and abort as soon as the
/// per-file cap is exceeded, instead of buffering the whole field first.
async fn limited_bytes(
    field: &mut axum::extract::multipart::Field<'_>,
    limit: usize,
    name: &str,
) -> Result<axum::body::Bytes> {
    // Keep the received chunks and copy them once into an exact-size buffer,
    // instead of a Vec that doubles its capacity as it grows.
    let mut chunks: Vec<axum::body::Bytes> = Vec::new();
    let mut len = 0usize;
    while let Some(chunk) = field
        .chunk()
        .await
        .map_err(|_| AppError::bad(format!("{name}: error leyendo el archivo")))?
    {
        if len + chunk.len() > limit {
            return Err(AppError::bad(format!(
                "{name} supera el límite de {} MB",
                limit / 1024 / 1024
            )));
        }
        len += chunk.len();
        chunks.push(chunk);
    }
    if chunks.len() == 1 {
        return Ok(chunks.pop().unwrap_or_default());
    }
    let mut buf = Vec::with_capacity(len);
    for chunk in chunks {
        buf.extend_from_slice(&chunk);
    }
    Ok(axum::body::Bytes::from(buf))
}

pub fn extension(data: &[u8]) -> Result<&'static str> {
    if data.starts_with(b"%PDF-") {
        Ok("pdf")
    } else if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        Ok("png")
    } else if data.starts_with(b"\xff\xd8\xff") {
        Ok("jpg")
    } else if std::str::from_utf8(data).is_ok() && !data.contains(&0) {
        // Reject HTML/SVG/XML disguised as text (case-insensitive, allowing BOM
        // and leading whitespace). Keep plain text and CSV safe.
        let head = data
            .split(|&b| b == b'\n')
            .next()
            .unwrap_or(data)
            .split(|&b| b == b' ')
            .next()
            .unwrap_or(data);
        let lower = head.to_ascii_lowercase();
        if lower.starts_with(b"<!doctype html")
            || lower.starts_with(b"<html")
            || lower.starts_with(b"<svg")
            || lower.starts_with(b"<?xml")
        {
            return Err(AppError::bad(
                "Solo se admiten archivos PDF, PNG, JPG o texto UTF-8",
            ));
        }
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
    while let Some(mut field) = multipart
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
                let original = field.file_name().unwrap_or("archivo").to_string();
                let limit = 9 * 1024 * 1024;
                let bytes = limited_bytes(&mut field, limit, &original).await?;
                if bytes.is_empty() || bytes.len() > limit {
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
    domain::max_chars(&comment, 10000, "Comentario")?;
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
    let previous:Option<(i64,String,bool,bool,String)>=sqlx::query_as("SELECT id,estado,es_final,calificacion_publicada,archivo FROM entregables WHERE reto_id=$1 AND estudiante_id=$2 AND titulo=$3 FOR UPDATE").bind(reto).bind(user.id).bind(&title).fetch_optional(&mut *tx).await?;
    // PAR-03: un entregable final o con calificación publicada no se reemplaza.
    if let Some((_, _, es_final, publicada, _)) = &previous
        && (*es_final || *publicada)
    {
        return Err(AppError::bad(
            "No puedes reemplazar un entregable final o con calificación publicada.",
        ));
    }
    let old_file = previous
        .as_ref()
        .map(|(_, _, _, _, archivo)| archivo.clone());
    let id:i64=sqlx::query_scalar("INSERT INTO entregables(reto_id,estudiante_id,titulo,es_final,puntaje_maximo,archivo,comentario_estudiante,fecha_entrega,actualizado_en,estado) VALUES($1,$2,$3,$4,5.00,$5,$6,now(),now(),'ENVIADO') ON CONFLICT(reto_id,estudiante_id,titulo) DO UPDATE SET archivo=excluded.archivo,comentario_estudiante=excluded.comentario_estudiante,es_final=excluded.es_final,estado='ENVIADO',nota=NULL,comentario_profesor=NULL,calificacion_publicada=false,calificacion_publicada_en=NULL,actualizado_en=now() RETURNING id")
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
        previous
            .as_ref()
            .map(|(_, s, _, _, _)| s.as_str())
            .unwrap_or(""),
        "ENVIADO",
        user.id,
        "Entregable enviado.",
    )
    .await?;
    let teachers:Vec<i64>=sqlx::query_scalar("SELECT DISTINCT profesor_id FROM integraciones_academicas WHERE reto_id=$1 AND estado='publicada'").bind(reto).fetch_all(&mut *tx).await?;
    for teacher in teachers {
        notify(
            &mut tx,
            teacher,
            "ENTREGABLE_RECIBIDO",
            &format!("{} subió el entregable \"{title}\".", user.username),
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
    // A replaced deliverable must not orphan the previous stored file.
    if let Some(old) = old_file.filter(|o| !o.is_empty()) {
        let _ = tokio::fs::remove_file(state.config.media_root.join(old)).await;
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
    let file = tokio::fs::File::open(state.config.media_root.join(key))
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                AppError::not_found()
            } else {
                file_error(e)
            }
        })?;
    let length = file.metadata().await.map_err(file_error)?.len();
    let extension = key.rsplit('.').next().unwrap_or("bin");
    // Stream in 64 KiB chunks so a download never holds the whole file in memory.
    let body = axum::body::Body::from_stream(futures_util::stream::try_unfold(
        file,
        |mut file| async move {
            let mut buf = vec![0u8; 64 * 1024];
            let n = tokio::io::AsyncReadExt::read(&mut file, &mut buf).await?;
            if n == 0 {
                return Ok::<_, std::io::Error>(None);
            }
            buf.truncate(n);
            Ok(Some((axum::body::Bytes::from(buf), file)))
        },
    ));
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=retosean-adjunto.{extension}"),
            ),
            (header::CONTENT_LENGTH, length.to_string()),
        ],
        body,
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
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::bad("Carga inválida"))?
    {
        match field.name().unwrap_or("") {
            "archivo" => {
                if data.is_some() {
                    return Err(AppError::bad("Adjunta un solo archivo"));
                }
                let original = field.file_name().unwrap_or("archivo").to_string();
                let limit = 9 * 1024 * 1024;
                let bytes = limited_bytes(&mut field, limit, &original).await?;
                if bytes.is_empty() || bytes.len() > limit {
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
                    let date = text
                        .parse::<chrono::NaiveDate>()
                        .map_err(|_| AppError::bad("Fecha de expedición inválida"))?;
                    domain::validate_date(Some(date), "Fecha de expedición")?;
                    issued = Some(date);
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
                "{name} tiene {age} días de expedida y el máximo es {days}."
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
            // EMP-06: dos primeras cargas concurrentes de un tipo aun no creado
            // pueden chocar en el unique_together; la perdedora recarga y usa la
            // fila ganadora en lugar de reventar con una violacion de unicidad.
            let created: Option<i64> = sqlx::query_scalar("INSERT INTO documentos_empresas(empresa_id,tipo_documento,archivo,estado,fecha_expedicion,fecha_vencimiento,fecha_carga,motivo_rechazo) VALUES($1,$2,$3,'CARGADO',$4,$5,now(),'') ON CONFLICT (empresa_id,tipo_documento) DO NOTHING RETURNING id").bind(company).bind(kind).bind(&key).bind(issued).bind(expires).fetch_optional(&mut *tx).await?;
            if let Some(id) = created {
                (id, String::new(), "Documento cargado.")
            } else {
                let (id, from): (i64, String) = sqlx::query_as(
                    "SELECT id,estado FROM documentos_empresas WHERE empresa_id=$1 AND tipo_documento=$2 FOR UPDATE",
                )
                .bind(company)
                .bind(kind)
                .fetch_one(&mut *tx)
                .await?;
                domain::transition("MAQUINA_DOCUMENTO", &from, "CARGADO")?;
                sqlx::query("UPDATE documentos_empresas SET archivo=$2,estado='CARGADO',fecha_expedicion=$3,fecha_vencimiento=$4,motivo_rechazo='',fecha_carga=now() WHERE id=$1").bind(id).bind(&key).bind(issued).bind(expires).execute(&mut *tx).await?;
                (id, from, "Documento recargado.")
            }
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
            "La empresa {company_name} {} el documento \"{name}\" para revisión.",
            if from.is_empty() {
                "cargó"
            } else {
                "actualizó"
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

/// MultipleFileField: up to 50 MB per file (retos and follow-ups).
pub const ATTACHMENT_LIMIT: usize = 50 * 1024 * 1024;
/// Cap on the number of files a single request may carry, and on the aggregate
/// bytes held in memory at once (under the route's 110 MB body limit).
pub const MAX_FILES_PER_REQUEST: usize = 10;
pub const ATTACHMENT_TOTAL_LIMIT: usize = 100 * 1024 * 1024;
/// A multipart text field is a form value, not a file; bound it so it cannot
/// balloon the request memory alongside the attachments.
const TEXT_FIELD_LIMIT: usize = 1024 * 1024;
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
/// Magic-byte class of a support file: the image/PDF signatures are exact,
/// the office and archive formats share the ZIP container, and plain text
/// covers txt and csv. Anything else is treated as binary/executable.
fn content_class(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(b"%PDF-") {
        Some("pdf")
    } else if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if data.starts_with(b"\xff\xd8\xff") {
        Some("jpg")
    } else if data.starts_with(b"PK\x03\x04") {
        Some("zip")
    } else if std::str::from_utf8(data).is_ok() && !data.contains(&0) {
        Some("text")
    } else {
        None
    }
}
/// Magic bytes and text-ness of an upload, tracked while it streams so the
/// whole file never has to be in memory. Equivalent to `content_class` on the
/// complete data.
#[derive(Default)]
struct Sniffer {
    head: Vec<u8>,
    carry: Vec<u8>,
    not_text: bool,
}
impl Sniffer {
    fn feed(&mut self, chunk: &[u8]) {
        if self.head.len() < 8 {
            let take = (8 - self.head.len()).min(chunk.len());
            self.head.extend_from_slice(&chunk[..take]);
        }
        if self.not_text {
            return;
        }
        if chunk.contains(&0) {
            self.not_text = true;
            return;
        }
        // A multibyte character may be split across chunks: validate the
        // carried prefix together with the new bytes.
        let joined;
        let data: &[u8] = if self.carry.is_empty() {
            chunk
        } else {
            let mut buf = std::mem::take(&mut self.carry);
            buf.extend_from_slice(chunk);
            joined = buf;
            &joined
        };
        if let Err(error) = std::str::from_utf8(data) {
            if error.error_len().is_some() {
                self.not_text = true;
            } else {
                self.carry = data[error.valid_up_to()..].to_vec();
            }
        }
    }
    fn class(&self) -> Option<&'static str> {
        let text = !self.not_text && self.carry.is_empty();
        match content_class(&self.head) {
            // The head alone may look like text; the verdict needs every byte.
            Some("text") | None => text.then_some("text"),
            magic => magic,
        }
    }
}
/// An attachment spooled to a private temporary file (`.<uuid>.part` in the
/// media root, never named after user input). The file is deleted when the
/// value is dropped unless `save_attachments` moved it to its final key.
pub struct SpooledFile {
    pub name: String,
    pub size: usize,
    class: Option<&'static str>,
    path: std::path::PathBuf,
    persisted: bool,
}
impl Drop for SpooledFile {
    fn drop(&mut self) {
        if !self.persisted {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}
/// Deletes `.<uuid>.part` spool files left behind by a crash or a killed
/// process; called once when the server starts.
pub fn remove_stale_spool_files(root: &std::path::Path) -> usize {
    let Ok(entries) = std::fs::read_dir(root) else {
        return 0;
    };
    entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            name.starts_with('.') && name.ends_with(".part")
        })
        .filter(|e| std::fs::remove_file(e.path()).is_ok())
        .count()
}
/// Validates the spooled attachments and moves them to their final keys;
/// returns (key, original name, size, path).
pub async fn save_attachments(
    state: &AppState,
    files: &mut [SpooledFile],
) -> Result<Vec<(String, String, i64, std::path::PathBuf)>> {
    for file in files.iter() {
        let name = &file.name;
        if file.size > ATTACHMENT_LIMIT {
            return Err(AppError::bad(format!("{name} supera el límite de 50 MB.")));
        }
        let declared = attachment_extension(name)?;
        // RET-08: se valida el contenido, no solo la extensión, para que un
        // HTML o un ejecutable renombrado (.pdf) no pase como archivo soporte.
        let coherent = matches!(
            (declared, file.class.unwrap_or("binario")),
            ("pdf", "pdf")
                | ("png", "png")
                | ("jpg", "jpg")
                | ("txt" | "csv", "text")
                | ("xlsx" | "zip" | "docx" | "pptx", "zip")
        );
        if !coherent {
            return Err(AppError::bad(format!(
                "{name}: archivo no permitido (el contenido no coincide con su extensión)."
            )));
        }
    }
    let mut saved: Vec<(String, String, i64, std::path::PathBuf)> = Vec::new();
    for file in files.iter_mut() {
        let key = format!(
            "{}.{}",
            uuid::Uuid::new_v4().simple(),
            attachment_extension(&file.name)?
        );
        let path = state.config.media_root.join(&key);
        if let Err(error) = tokio::fs::rename(&file.path, &path).await {
            for (_, _, _, written) in &saved {
                let _ = tokio::fs::remove_file(written).await;
            }
            return Err(file_error(error));
        }
        file.persisted = true;
        let original: String = file.name.chars().take(255).collect();
        saved.push((key, original, file.size as i64, path));
    }
    Ok(saved)
}
/// Streams one file part into a temporary file, enforcing the per-file and
/// per-request limits as the bytes arrive.
async fn spool(
    root: &std::path::Path,
    field: &mut axum::extract::multipart::Field<'_>,
    name: String,
    total: &mut usize,
) -> Result<SpooledFile> {
    use tokio::io::AsyncWriteExt;
    let mut upload = SpooledFile {
        path: root.join(format!(".{}.part", uuid::Uuid::new_v4().simple())),
        name,
        size: 0,
        class: None,
        persisted: false,
    };
    // Created on the first non-empty chunk, so empty parts never touch the disk.
    let mut out: Option<tokio::fs::File> = None;
    let mut sniffer = Sniffer::default();
    while let Some(chunk) = field
        .chunk()
        .await
        .map_err(|_| AppError::bad(format!("{}: error leyendo el archivo", upload.name)))?
    {
        if upload.size + chunk.len() > ATTACHMENT_LIMIT {
            return Err(AppError::bad(format!(
                "{} supera el límite de {} MB",
                upload.name,
                ATTACHMENT_LIMIT / 1024 / 1024
            )));
        }
        if *total + chunk.len() > ATTACHMENT_TOTAL_LIMIT {
            return Err(AppError::bad(
                "El tamaño total de los archivos supera el límite permitido",
            ));
        }
        upload.size += chunk.len();
        *total += chunk.len();
        sniffer.feed(&chunk);
        if out.is_none() && !chunk.is_empty() {
            tokio::fs::create_dir_all(root).await.map_err(file_error)?;
            out = Some(
                tokio::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&upload.path)
                    .await
                    .map_err(file_error)?,
            );
        }
        if let Some(file) = out.as_mut() {
            file.write_all(&chunk).await.map_err(file_error)?;
        }
    }
    if let Some(mut file) = out {
        file.flush().await.map_err(file_error)?;
    }
    upload.class = sniffer.class();
    Ok(upload)
}
/// Text fields plus every file under `file_field` (retos and follow-ups).
/// Files are spooled under `root` instead of being held in memory.
pub async fn read_with_files(
    root: &std::path::Path,
    mut multipart: Multipart,
    file_field: &str,
) -> Result<(std::collections::HashMap<String, String>, Vec<SpooledFile>)> {
    let mut fields = std::collections::HashMap::new();
    let mut files: Vec<SpooledFile> = Vec::new();
    let mut total = 0usize;
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::bad("Carga inválida"))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == file_field {
            if files.len() >= MAX_FILES_PER_REQUEST {
                return Err(AppError::bad(format!(
                    "Máximo {MAX_FILES_PER_REQUEST} archivos por solicitud"
                )));
            }
            let original = field.file_name().unwrap_or("archivo").to_string();
            let upload = spool(root, &mut field, original, &mut total).await?;
            // Empty parts are ignored (the temporary file is removed on drop).
            if upload.size > 0 {
                files.push(upload);
            }
        } else {
            let bytes = limited_bytes(&mut field, TEXT_FIELD_LIMIT, "El campo").await?;
            let text =
                String::from_utf8(bytes.to_vec()).map_err(|_| AppError::bad("Campo inválido"))?;
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
    // Authorize before reading the body: a rejected upload never reaches memory.
    let row: Option<(i64, String)> =
        sqlx::query_as("SELECT empresa_id,estado FROM retos WHERE id=$1 AND eliminado_en IS NULL")
            .bind(reto)
            .fetch_optional(&state.pool)
            .await?;
    let (owner, estado) = row.ok_or_else(AppError::not_found)?;
    let editable = |estado: &str| {
        user.rol == "EMPRESA" && owner == user.id && ["borrador", "rechazado"].contains(&estado)
    };
    if !user.admin() && !editable(&estado) {
        return Err(AppError::forbidden());
    }
    let (_, mut files) = read_with_files(&state.config.media_root, multipart, "archivos").await?;
    if files.is_empty() {
        return Err(AppError::bad("Adjunta al menos un archivo"));
    }
    let mut tx = state.pool.begin().await?;
    // Re-check under the row lock: the reto may have changed while reading.
    let row: Option<String> = sqlx::query_scalar(
        "SELECT estado FROM retos WHERE id=$1 AND eliminado_en IS NULL AND empresa_id=$2 FOR UPDATE",
    )
    .bind(reto)
    .bind(owner)
    .fetch_optional(&mut *tx)
    .await?;
    let current = row.ok_or_else(AppError::not_found)?;
    if !user.admin() && !editable(&current) {
        return Err(AppError::forbidden());
    }
    let saved = save_attachments(&state, &mut files).await?;
    let commit_result = async {
        for (key, original, size, _) in &saved {
            sqlx::query("INSERT INTO retos_retoarchivo(reto_id,archivo,nombre_original,tamano,creado_en) VALUES($1,$2,$3,$4,now())")
                .bind(reto).bind(key).bind(original).bind(size).execute(&mut *tx).await?;
        }
        tx.commit().await
    }.await;
    if let Err(error) = commit_result {
        for (_, _, _, path) in &saved {
            let _ = tokio::fs::remove_file(path).await;
        }
        return Err(error.into());
    }
    Ok(Json(json!({"archivos": saved.len()})))
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
    fn streamed_sniffing_matches_whole_file_classification() {
        let text = "año ñandú — 漢字 ok".as_bytes();
        let samples: [&[u8]; 6] = [
            text,
            b"%PDF-1.4 body",
            b"\x89PNG\r\n\x1a\n....",
            b"PK\x03\x04zip",
            b"abc\xff\xfe",
            b"abc\0def",
        ];
        for data in samples {
            // Every split point, including inside multibyte characters.
            for cut in 0..=data.len() {
                let mut sniffer = Sniffer::default();
                sniffer.feed(&data[..cut]);
                sniffer.feed(&data[cut..]);
                assert_eq!(
                    sniffer.class(),
                    content_class(data),
                    "{data:?} cut at {cut}"
                );
            }
            let mut bytewise = Sniffer::default();
            for b in data {
                bytewise.feed(std::slice::from_ref(b));
            }
            assert_eq!(
                bytewise.class(),
                content_class(data),
                "{data:?} byte by byte"
            );
        }
        // A file that ends inside a multibyte character is not text.
        let mut sniffer = Sniffer::default();
        sniffer.feed(&text[..2]);
        assert_eq!(sniffer.class(), None);
    }
    #[test]
    fn formats_are_detected_from_content() {
        assert_eq!(extension(b"%PDF-1.4").unwrap(), "pdf");
        assert_eq!(extension(b"text").unwrap(), "txt");
        assert!(extension(b"\0\xff\0").is_err());
    }
}
