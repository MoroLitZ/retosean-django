//! Study units (apps/unidades_estudio/views.py): admins and professors create,
//! edit, deactivate/activate and bulk-load them from CSV.
use crate::{
    AppState, auth,
    auth::User,
    error::{AppError, Result},
};
use axum::{
    Json,
    body::Bytes,
    extract::{Multipart, Path, State},
    http::{HeaderMap, StatusCode},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use std::collections::HashMap;

/// _facultad_profesor: la facultad del perfil de profesor, si existe.
async fn faculty(pool: &PgPool, user: i64) -> Result<Option<i64>> {
    let row: Option<Option<i64>> =
        sqlx::query_scalar("SELECT facultad_id FROM profesores WHERE usuario_id=$1")
            .bind(user)
            .fetch_optional(pool)
            .await?;
    Ok(row.flatten())
}
/// _puede_gestionar: admin siempre; profesor solo si tiene facultad asignada.
async fn manager(state: &AppState, headers: &HeaderMap) -> Result<User> {
    let user = auth::current(state, headers, true).await?;
    // CIE-03: un profesor sin facultad no gestiona unidades de estudio.
    if user.admin() || (user.rol == "PROFESOR" && faculty(&state.pool, user.id).await?.is_some()) {
        return Ok(user);
    }
    Err(AppError::forbidden())
}
/// _puede_administrar_unidad: un profesor solo administra unidades cuyo programa
/// pertenece a su facultad (CIE-03); se usa al editar y al activar/desactivar.
async fn ensure_scope(pool: &PgPool, user: &User, unit: i64, action: &str) -> Result<()> {
    if user.admin() {
        return Ok(());
    }
    let belongs: Option<i64> = sqlx::query_scalar(
        "SELECT p.facultad_id FROM unidades_estudio u JOIN programas p ON p.id=u.programa_id WHERE u.id=$1",
    )
    .bind(unit)
    .fetch_optional(pool)
    .await?;
    let belongs = belongs.ok_or_else(AppError::not_found)?;
    if faculty(pool, user.id).await? != Some(belongs) {
        return Err(AppError(
            StatusCode::FORBIDDEN,
            format!("No tienes permiso para {action} esta unidad de estudio."),
        ));
    }
    Ok(())
}
async fn read_form(mut multipart: Multipart) -> Result<(HashMap<String, String>, Option<Bytes>)> {
    let mut fields = HashMap::new();
    let mut file = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::bad("Carga inválida"))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "archivo" || name == "archivo_csv" {
            let bytes = field
                .bytes()
                .await
                .map_err(|_| AppError::bad("Archivo demasiado grande."))?;
            if !bytes.is_empty() {
                file = Some(bytes);
            }
        } else {
            let text = field
                .text()
                .await
                .map_err(|_| AppError::bad("Campo inválido"))?;
            fields.insert(name, text.trim().to_string());
        }
    }
    Ok((fields, file))
}
struct Unit {
    codigo: String,
    nombre: String,
    programa: i64,
    periodo: String,
    ciclo: String,
}
/// The view's checks, in its order; the first error is reported.
async fn validate(
    state: &AppState,
    user: &User,
    fields: &HashMap<String, String>,
    current: Option<i64>,
) -> Result<Unit> {
    let get = |name: &str| fields.get(name).cloned().unwrap_or_default();
    let (codigo, nombre, periodo, ciclo) =
        (get("codigo"), get("nombre"), get("periodo"), get("ciclo"));
    if codigo.is_empty() {
        return Err(AppError::bad("El código es obligatorio."));
    }
    if nombre.is_empty() {
        return Err(AppError::bad("El nombre es obligatorio."));
    }
    if codigo.chars().count() > 20
        || nombre.chars().count() > 200
        || periodo.chars().count() > 20
        || ciclo.chars().count() > 50
    {
        return Err(AppError::bad("Datos demasiado largos"));
    }
    let programa: Option<(i64, i64)> = match get("programa").parse::<i64>() {
        Ok(id) => {
            sqlx::query_as("SELECT id,facultad_id FROM programas WHERE id=$1")
                .bind(id)
                .fetch_optional(&state.pool)
                .await?
        }
        Err(_) => None,
    };
    let Some((programa, facultad)) = programa else {
        return Err(AppError::bad("El programa académico es obligatorio."));
    };
    // CIE-03: _pertenece_a_facultad; un profesor solo asigna programas de su facultad.
    if !user.admin() && faculty(&state.pool, user.id).await? != Some(facultad) {
        return Err(AppError::bad(if current.is_some() {
            "No tienes permiso para asignar esta unidad a ese programa."
        } else {
            "No tienes permiso para crear unidades en ese programa."
        }));
    }
    let taken: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM unidades_estudio WHERE codigo=$1 AND id IS DISTINCT FROM $2)",
    )
    .bind(&codigo)
    .bind(current)
    .fetch_one(&state.pool)
    .await?;
    if taken {
        return Err(AppError::bad(
            "Ese código ya está vinculado a otra materia.",
        ));
    }
    Ok(Unit {
        codigo,
        nombre,
        programa,
        periodo,
        ciclo,
    })
}
async fn store(state: &AppState, file: Option<Bytes>) -> Result<Option<String>> {
    let Some(data) = file else { return Ok(None) };
    if data.len() > 9 * 1024 * 1024 || crate::media::extension(&data)? != "pdf" {
        return Err(AppError::bad("El archivo debe ser un PDF de máximo 9 MB"));
    }
    let key = format!("{}.pdf", uuid::Uuid::new_v4().simple());
    tokio::fs::create_dir_all(&state.config.media_root)
        .await
        .map_err(|_| AppError::bad("No se pudo guardar el archivo"))?;
    tokio::fs::write(state.config.media_root.join(&key), &data)
        .await
        .map_err(|_| AppError::bad("No se pudo guardar el archivo"))?;
    Ok(Some(key))
}

pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    multipart: Multipart,
) -> Result<Json<Value>> {
    let user = manager(&state, &headers).await?;
    let (fields, file) = read_form(multipart).await?;
    if fields.get("accion").map(String::as_str) == Some("cargar_csv") {
        return load_csv(&state, file).await;
    }
    let unit = validate(&state, &user, &fields, None).await?;
    let key = store(&state, file).await?;
    let id: i64 = sqlx::query_scalar("INSERT INTO unidades_estudio(codigo,nombre,programa_id,periodo,ciclo,archivo,activo,creado_en,actualizado_en) VALUES($1,$2,$3,$4,$5,$6,true,now(),now()) RETURNING id")
        .bind(&unit.codigo).bind(&unit.nombre).bind(unit.programa).bind(&unit.periodo).bind(&unit.ciclo).bind(key).fetch_one(&state.pool).await?;
    Ok(Json(json!({
        "id": id,
        "mensaje": format!("Unidad de estudio '{} - {}' creada correctamente.", unit.codigo, unit.nombre),
    })))
}
pub async fn edit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    multipart: Multipart,
) -> Result<Json<Value>> {
    let user = manager(&state, &headers).await?;
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM unidades_estudio WHERE id=$1)")
            .bind(id)
            .fetch_one(&state.pool)
            .await?;
    if !exists {
        return Err(AppError::not_found());
    }
    ensure_scope(&state.pool, &user, id, "editar").await?;
    let (fields, file) = read_form(multipart).await?;
    let unit = validate(&state, &user, &fields, Some(id)).await?;
    let key = store(&state, file).await?;
    sqlx::query("UPDATE unidades_estudio SET codigo=$2,nombre=$3,programa_id=$4,periodo=$5,ciclo=$6,archivo=COALESCE($7,archivo),actualizado_en=now() WHERE id=$1")
        .bind(id).bind(&unit.codigo).bind(&unit.nombre).bind(unit.programa).bind(&unit.periodo).bind(&unit.ciclo).bind(key).execute(&state.pool).await?;
    Ok(Json(json!({
        "id": id,
        "mensaje": format!("Unidad de estudio '{}' actualizada correctamente.", unit.codigo),
    })))
}
#[derive(Deserialize)]
pub struct ActiveInput {
    activo: bool,
}
/// eliminar_unidad (logical delete) and activar_unidad.
pub async fn set_active(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<ActiveInput>,
) -> Result<Json<Value>> {
    let user = manager(&state, &headers).await?;
    let action = if input.activo { "activar" } else { "desactivar" };
    ensure_scope(&state.pool, &user, id, action).await?;
    let code: Option<String> = sqlx::query_scalar(
        "UPDATE unidades_estudio SET activo=$2,actualizado_en=now() WHERE id=$1 RETURNING codigo",
    )
    .bind(id)
    .bind(input.activo)
    .fetch_optional(&state.pool)
    .await?;
    let code = code.ok_or_else(AppError::not_found)?;
    Ok(Json(json!({
        "activo": input.activo,
        "mensaje": format!("Unidad de estudio '{code}' {} correctamente.", if input.activo { "activada" } else { "desactivada" }),
    })))
}

/// _procesar_csv: rows need codigo, nombre and an existing programa (by name).
async fn load_csv(state: &AppState, file: Option<Bytes>) -> Result<Json<Value>> {
    let data = file.ok_or_else(|| AppError::bad("Selecciona un archivo CSV."))?;
    if data.len() > 2 * 1024 * 1024 {
        return Err(AppError::bad("El CSV es demasiado grande."));
    }
    let text = match std::str::from_utf8(&data) {
        Ok(t) => t.trim_start_matches('\u{feff}').to_string(),
        Err(_) => encoding_rs::WINDOWS_1252.decode(&data).0.into_owned(),
    };
    let mut rows = crate::import::parse_csv(&text).into_iter();
    let headers: Vec<String> = rows
        .next()
        .unwrap_or_default()
        .iter()
        .map(|h| h.trim().to_lowercase())
        .collect();
    let missing: Vec<&str> = ["codigo", "nombre", "programa"]
        .into_iter()
        .filter(|c| !headers.iter().any(|h| h == c))
        .collect();
    if !missing.is_empty() {
        return Err(AppError::bad(format!(
            "Al CSV le faltan estas columnas obligatorias: {}.",
            missing.join(", ")
        )));
    }
    // csv.DictReader skips blank lines; keep the original line numbers for messages.
    let rows: Vec<(usize, Vec<String>)> = rows
        .enumerate()
        .map(|(index, row)| (index + 2, row))
        .filter(|(_, row)| !row.iter().all(|v| v.trim().is_empty()))
        .collect();
    if rows.len() > 5000 {
        return Err(AppError::bad("El CSV tiene más de 5000 filas."));
    }
    let mut tx = state.pool.begin().await?;
    let (mut created, mut problems) = (0, Vec::new());
    for (number, row) in rows {
        let get = |key: &str| {
            headers
                .iter()
                .position(|h| h == key)
                .and_then(|i| row.get(i))
                .map(|v| v.trim().to_string())
                .unwrap_or_default()
        };
        let (codigo, nombre, programa) = (get("codigo"), get("nombre"), get("programa"));
        if codigo.is_empty() || nombre.is_empty() || programa.is_empty() {
            problems.push(format!("fila {number}: faltan código, nombre o programa"));
            continue;
        }
        let (periodo, ciclo) = (get("periodo"), get("ciclo"));
        if codigo.chars().count() > 20
            || nombre.chars().count() > 200
            || periodo.chars().count() > 20
            || ciclo.chars().count() > 50
        {
            problems.push(format!("fila {number}: datos demasiado largos"));
            continue;
        }
        let taken: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM unidades_estudio WHERE codigo=$1)")
                .bind(&codigo)
                .fetch_one(&mut *tx)
                .await?;
        if taken {
            problems.push(format!("fila {number}: el código {codigo} ya existe"));
            continue;
        }
        // CIE-09: _resolver_programa_csv; un nombre repetido en varias facultades se
        // resuelve con la columna opcional `facultad` y sin ella se rechaza la fila.
        let facultad = get("facultad");
        let facultad = (!facultad.is_empty()).then_some(facultad.as_str());
        let matches: Vec<i64> = sqlx::query_scalar(
            "SELECT p.id FROM programas p JOIN facultades f ON f.id=p.facultad_id WHERE lower(p.nombre)=lower($1) AND ($2::text IS NULL OR lower(f.nombre)=lower($2::text))",
        )
        .bind(&programa)
        .bind(facultad)
        .fetch_all(&mut *tx)
        .await?;
        let program = match *matches.as_slice() {
            [only] => only,
            [] => {
                problems.push(format!(
                    "fila {number}: el programa '{programa}' no existe en el catálogo"
                ));
                continue;
            }
            _ => {
                let detail = match facultad {
                    Some(name) => {
                        format!("el programa '{programa}' no existe en la facultad '{name}'")
                    }
                    None => format!(
                        "el programa '{programa}' existe en varias facultades; indica la facultad en el CSV"
                    ),
                };
                problems.push(format!("fila {number}: {detail}"));
                continue;
            }
        };
        sqlx::query("INSERT INTO unidades_estudio(codigo,nombre,programa_id,periodo,ciclo,activo,creado_en,actualizado_en) VALUES($1,$2,$3,$4,$5,true,now(),now())")
            .bind(&codigo).bind(&nombre).bind(program).bind(&periodo).bind(&ciclo).execute(&mut *tx).await?;
        created += 1;
    }
    tx.commit().await?;
    let mut messages = Vec::new();
    if created > 0 {
        messages.push(format!(
            "{created} unidad(es) de estudio creada(s) desde el CSV."
        ));
    }
    if !problems.is_empty() {
        let mut detail = problems
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("; ");
        if problems.len() > 8 {
            detail.push_str("; ...");
        }
        messages.push(format!("{} fila(s) omitida(s): {detail}", problems.len()));
    }
    if created == 0 && problems.is_empty() {
        messages.push("El CSV no contenía filas de datos.".into());
    }
    Ok(Json(
        json!({"creadas":created,"omitidas":problems.len(),"mensaje":messages.join(" ")}),
    ))
}
