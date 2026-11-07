use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

#[derive(Debug)]
pub struct AppError(pub StatusCode, pub String);
pub type Result<T> = std::result::Result<T, AppError>;
impl AppError {
    pub fn bad(message: impl Into<String>) -> Self {
        Self(StatusCode::BAD_REQUEST, message.into())
    }
    pub fn forbidden() -> Self {
        Self(
            StatusCode::FORBIDDEN,
            "No tienes permiso para esta operación".into(),
        )
    }
    pub fn not_found() -> Self {
        Self(StatusCode::NOT_FOUND, "Registro no encontrado".into())
    }
}
impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        if let sqlx::Error::Database(db) = &error {
            if db.is_unique_violation() {
                return Self(StatusCode::CONFLICT, "El registro ya existe".into());
            }
            if db.is_foreign_key_violation() || db.is_check_violation() {
                return Self::bad("Datos relacionados inválidos");
            }
            // Bad client data reaching a column: NUL/invalid bytes (22021), numeric
            // out of range (22003), datetime overflow (22008), bad datetime (22007).
            if matches!(
                db.code().as_deref(),
                Some("22021") | Some("22003") | Some("22008") | Some("22007")
            ) {
                return Self::bad(
                    "Datos inválidos: un valor está fuera de rango o tiene caracteres no permitidos",
                );
            }
            if db.code().as_deref() == Some("22001") {
                return Self::bad(
                    "Uno de los textos supera la longitud máxima permitida. Acórtalo e inténtalo de nuevo.",
                );
            }
        }
        tracing::error!(%error, "database operation failed");
        Self(
            StatusCode::INTERNAL_SERVER_ERROR,
            "No se pudo completar la operación".into(),
        )
    }
}
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error": self.1}))).into_response()
    }
}
