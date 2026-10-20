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
        && ["pdf", "png", "jpg", "txt"].contains(&ext)
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
    let accepted:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM postulaciones WHERE reto_id=$1 AND estudiante_id=$2 AND estado='ACEPTADA')").bind(reto).bind(user.id).fetch_one(&mut *tx).await?;
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
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=retosean-adjunto",
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
    mut multipart: Multipart,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    if user.rol != "EMPRESA" {
        return Err(AppError::forbidden());
    }
    let mut data = None;
    let mut issued = None;
    let mut expires = None;
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
            "fecha_vencimiento" => {
                let text = field
                    .text()
                    .await
                    .map_err(|_| AppError::bad("Fecha inválida"))?;
                if !text.is_empty() {
                    expires = Some(
                        text.parse::<chrono::NaiveDate>()
                            .map_err(|_| AppError::bad("Fecha de vencimiento inválida"))?,
                    );
                }
            }
            _ => return Err(AppError::bad("Campo desconocido")),
        }
    }
    let data = data.ok_or_else(|| AppError::bad("Adjunta un archivo"))?;
    let ext = extension(&data)?;
    // DocumentoEmpresa.EXTENSIONES_PERMITIDAS: legal documents are PDF only.
    if ext != "pdf" {
        return Err(AppError::bad("Los documentos legales deben ser PDF"));
    }
    let mut tx = state.pool.begin().await?;
    let row:Option<(String,String,i64,Option<i16>,Option<String>)>=sqlx::query_as("SELECT d.estado,d.tipo_documento,e.id,t.vigencia_dias,t.nombre FROM documentos_empresas d JOIN empresas e ON e.id=d.empresa_id LEFT JOIN tipos_documento_legal t ON t.codigo=d.tipo_documento WHERE d.id=$1 AND e.usuario_id=$2 FOR UPDATE OF d").bind(id).bind(user.id).fetch_optional(&mut *tx).await?;
    let (from, kind, company, validity, name) = row.ok_or_else(AppError::not_found)?;
    // Catalog validity (e.g. Cámara de Comercio, 30 days): the issue date is
    // required, must be recent enough, and fixes the expiry (fecha_vencimiento_para).
    if let Some(days) = validity.filter(|d| *d > 0) {
        let name = name.unwrap_or_default();
        let issued_on = issued
            .ok_or_else(|| AppError::bad(format!("{name}: indica la fecha de expedición")))?;
        let today = (chrono::Utc::now() - chrono::Duration::hours(5)).date_naive(); // Bogota, no DST
        if issued_on > today || (today - issued_on).num_days() > days as i64 {
            return Err(AppError::bad(format!(
                "{name}: debe tener menos de {days} días de expedida"
            )));
        }
        expires = Some(issued_on + chrono::Duration::days(days as i64));
    }
    if issued.zip(expires).is_some_and(|(a, b)| a > b) {
        return Err(AppError::bad(
            "La expedición debe ser anterior al vencimiento",
        ));
    }
    domain::transition("MAQUINA_DOCUMENTO", &from, "CARGADO")?;
    let key = format!("{}.{}", uuid::Uuid::new_v4().simple(), ext);
    sqlx::query("UPDATE documentos_empresas SET archivo=$2,estado='CARGADO',fecha_expedicion=$3,fecha_vencimiento=$4,motivo_rechazo='',fecha_carga=now() WHERE id=$1").bind(id).bind(&key).bind(issued).bind(expires).execute(&mut *tx).await?;
    history(
        &mut tx,
        ("empresas", "documentoempresa"),
        id,
        &from,
        "CARGADO",
        user.id,
        "Documento legal cargado",
    )
    .await?;
    crate::companies::recompute_validation(&mut tx, company).await?;
    let admins: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM usuarios WHERE is_active AND (is_superuser OR rol='ADMIN')",
    )
    .fetch_all(&mut *tx)
    .await?;
    for admin in admins {
        notify(
            &mut tx,
            admin,
            "DOCUMENTO_CARGADO_ADMIN",
            &format!("Documento {kind} pendiente de revisión"),
            &format!("documento:{id}:{}", uuid::Uuid::new_v4().simple()),
        )
        .await?;
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
    Ok(Json(json!({"estado":"CARGADO"})))
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
