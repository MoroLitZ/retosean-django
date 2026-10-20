use crate::{
    AppState, auth, domain,
    error::{AppError, Result},
};
use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
pub struct CatalogInput {
    nombre: String,
    #[serde(default)]
    codigo: String,
    #[serde(default)]
    descripcion: String,
    facultad_id: Option<i64>,
}
pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(name): Path<String>,
    Json(input): Json<CatalogInput>,
) -> Result<Json<Value>> {
    let user = auth::current(&state, &headers, true).await?;
    if !user.admin() {
        return Err(AppError::forbidden());
    }
    domain::validate_text(&input.nombre, 150, "Nombre")?;
    if input.codigo.chars().count() > 20 || input.descripcion.len() > 10000 {
        return Err(AppError::bad("Datos de catálogo demasiado largos"));
    }
    let id: i64 = match name.as_str() {
        "facultades" => {
            domain::validate_text(&input.codigo, 20, "Código")?;
            sqlx::query_scalar("INSERT INTO facultades(nombre,codigo) VALUES($1,$2) RETURNING id")
                .bind(&input.nombre)
                .bind(&input.codigo)
                .fetch_one(&state.pool)
                .await?
        }
        "programas" => {
            let faculty = input
                .facultad_id
                .ok_or_else(|| AppError::bad("Selecciona la facultad"))?;
            sqlx::query_scalar(
                "INSERT INTO programas(nombre,facultad_id,nivel) VALUES($1,$2,'') RETURNING id",
            )
            .bind(&input.nombre)
            .bind(faculty)
            .fetch_one(&state.pool)
            .await?
        }
        "ecosistemas" => {
            sqlx::query_scalar(
                "INSERT INTO ecosistemas(nombre,descripcion) VALUES($1,$2) RETURNING id",
            )
            .bind(&input.nombre)
            .bind(&input.descripcion)
            .fetch_one(&state.pool)
            .await?
        }
        _ => return Err(AppError::not_found()),
    };
    Ok(Json(json!({"id":id})))
}
