use crate::{
    AppState,
    error::{AppError, Result},
};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool};
use subtle::ConstantTimeEq;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub rol: String,
    pub is_superuser: bool,
}
impl User {
    pub fn admin(&self) -> bool {
        self.is_superuser || self.rol == "ADMIN"
    }
    pub fn require(&self, role: &str) -> Result<()> {
        if self.admin() || self.rol == role {
            Ok(())
        } else {
            Err(AppError::forbidden())
        }
    }
}

pub async fn init(pool: &PgPool) -> std::result::Result<(), sqlx::Error> {
    sqlx::raw_sql("CREATE TABLE IF NOT EXISTS rust_sessions (token_hash TEXT PRIMARY KEY, user_id BIGINT NOT NULL REFERENCES usuarios(id) ON DELETE CASCADE, csrf TEXT NOT NULL, expires_at TIMESTAMPTZ NOT NULL); CREATE INDEX IF NOT EXISTS rust_sessions_expiry ON rust_sessions(expires_at); CREATE TABLE IF NOT EXISTS rust_login_attempts (subject TEXT NOT NULL, attempted_at TIMESTAMPTZ NOT NULL DEFAULT now()); CREATE INDEX IF NOT EXISTS rust_login_attempts_subject ON rust_login_attempts(subject, attempted_at);").execute(pool).await?;
    Ok(())
}
fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
fn session_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("cookie")?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|item| item.trim().strip_prefix("retosean_session="))
}
pub async fn current(state: &AppState, headers: &HeaderMap, write: bool) -> Result<User> {
    let unauthorized = || {
        AppError(
            StatusCode::UNAUTHORIZED,
            "Inicia sesión para continuar".into(),
        )
    };
    let token = session_token(headers).ok_or_else(unauthorized)?;
    if token.len() != 64 {
        return Err(unauthorized());
    }
    let record: Option<(i64, String)> = sqlx::query_as(
        "SELECT user_id, csrf FROM rust_sessions WHERE token_hash=$1 AND expires_at>now()",
    )
    .bind(digest(token))
    .fetch_optional(&state.pool)
    .await?;
    let (id, csrf) = record.ok_or_else(unauthorized)?;
    if write {
        let supplied = headers
            .get("x-csrf-token")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if !bool::from(csrf.as_bytes().ct_eq(supplied.as_bytes())) {
            return Err(AppError::forbidden());
        }
    }
    sqlx::query_as::<_, User>("SELECT id, username, first_name, last_name, email, rol, is_superuser FROM usuarios WHERE id=$1 AND is_active AND activo")
        .bind(id).fetch_optional(&state.pool).await?.ok_or_else(unauthorized)
}

pub fn verify_password(password: &str, encoded: &str) -> bool {
    let parts: Vec<_> = encoded.split('$').collect();
    if parts.len() != 4 || parts[0] != "pbkdf2_sha256" {
        return false;
    }
    let iterations = match parts[1].parse::<u32>() {
        Ok(n) if (1..=2_000_000).contains(&n) => n,
        _ => return false,
    };
    let expected = match STANDARD.decode(parts[3]) {
        Ok(v) if v.len() == 32 => v,
        _ => return false,
    };
    let mut actual = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<Sha256>(
        password.as_bytes(),
        parts[2].as_bytes(),
        iterations,
        &mut actual,
    );
    bool::from(actual.as_slice().ct_eq(&expected))
}
pub fn hash_password(password: &str) -> String {
    let salt = uuid::Uuid::new_v4().simple().to_string();
    let mut output = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<Sha256>(password.as_bytes(), salt.as_bytes(), 1_000_000, &mut output);
    format!("pbkdf2_sha256$1000000${salt}${}", STANDARD.encode(output))
}
#[derive(Deserialize)]
pub struct Credentials {
    username: String,
    password: String,
}
pub async fn login(
    State(state): State<AppState>,
    Json(input): Json<Credentials>,
) -> Result<Response> {
    if input.username.len() > 150 || input.password.len() > 1024 || input.username.is_empty() {
        return Err(AppError::bad("Credenciales inválidas"));
    }
    let subject = digest(&input.username.to_lowercase());
    sqlx::query("DELETE FROM rust_login_attempts WHERE attempted_at < now()-interval '10 minutes'")
        .execute(&state.pool)
        .await?;
    let mut tx = state.pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(&subject)
        .execute(&mut *tx)
        .await?;
    let attempts: i64 =
        sqlx::query_scalar("SELECT count(*) FROM rust_login_attempts WHERE subject=$1")
            .bind(&subject)
            .fetch_one(&mut *tx)
            .await?;
    if attempts >= 10 {
        return Err(AppError(
            StatusCode::TOO_MANY_REQUESTS,
            "Demasiados intentos. Espera 10 minutos".into(),
        ));
    }
    sqlx::query("INSERT INTO rust_login_attempts(subject) VALUES($1)")
        .bind(&subject)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    let record: Option<(i64, String)> = sqlx::query_as(
        "SELECT id,password FROM usuarios WHERE username=$1 AND is_active AND activo",
    )
    .bind(&input.username)
    .fetch_optional(&state.pool)
    .await?;
    let (id, hash) = record.unwrap_or((
        0,
        "pbkdf2_sha256$1000000$dummy$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".into(),
    ));
    let permit = state
        .password_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| {
            AppError(
                StatusCode::TOO_MANY_REQUESTS,
                "Servidor ocupado. Intenta de nuevo".into(),
            )
        })?;
    let valid = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        verify_password(&input.password, &hash)
    })
    .await
    .map_err(|_| AppError::bad("No se pudo validar la contraseña"))?;
    if !valid || id == 0 {
        return Err(AppError(
            StatusCode::UNAUTHORIZED,
            "Usuario o contraseña incorrectos".into(),
        ));
    }
    let token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let csrf = uuid::Uuid::new_v4().simple().to_string();
    sqlx::query("DELETE FROM rust_sessions WHERE expires_at<=now()")
        .execute(&state.pool)
        .await?;
    sqlx::query("INSERT INTO rust_sessions(token_hash,user_id,csrf,expires_at) VALUES($1,$2,$3,now()+interval '8 hours')").bind(digest(&token)).bind(id).bind(&csrf).execute(&state.pool).await?;
    sqlx::query("UPDATE usuarios SET last_login=now() WHERE id=$1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    let mut response = Json(json!({"csrf":csrf})).into_response();
    let secure = if state.config.secure_cookie {
        "; Secure"
    } else {
        ""
    };
    response.headers_mut().insert(
        "set-cookie",
        HeaderValue::from_str(&format!(
            "retosean_session={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age=28800{secure}"
        ))
        .map_err(|_| AppError::bad("Cookie inválida"))?,
    );
    Ok(response)
}
pub async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>> {
    let user = current(&state, &headers, false).await?;
    let csrf: String = sqlx::query_scalar("SELECT csrf FROM rust_sessions WHERE token_hash=$1")
        .bind(digest(session_token(&headers).unwrap_or("")))
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(json!({"user":user,"csrf":csrf})))
}
pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Result<Response> {
    current(&state, &headers, true).await?;
    sqlx::query("DELETE FROM rust_sessions WHERE token_hash=$1")
        .bind(digest(session_token(&headers).unwrap_or("")))
        .execute(&state.pool)
        .await?;
    let mut response = Json(json!({"ok":true})).into_response();
    response.headers_mut().insert(
        "set-cookie",
        HeaderValue::from_static("retosean_session=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0"),
    );
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn django_password_compatibility() {
        // Produced by Python hashlib.pbkdf2_hmac('sha256', b'correct', b'salt', 1000).
        let hash = "pbkdf2_sha256$1000$salt$Gqh1a0dau93l9kbZqwSU16P8k+ICFm8gow9Jjw85w34=";
        assert!(verify_password("correct", hash));
        assert!(!verify_password("wrong", hash));
        let hash = hash_password("RetosEAN2026");
        assert!(verify_password("RetosEAN2026", &hash));
        assert!(!verify_password("wrong", &hash));
    }
    #[test]
    fn malformed_passwords_are_rejected() {
        for hash in [
            "",
            "!disabled",
            "pbkdf2_sha256$999999999$salt$AAA=",
            "pbkdf2_sha256$0$salt$AAA=",
            "argon2$invalid",
        ] {
            assert!(!verify_password("x", hash));
        }
    }
}
