use crate::{
    AppState,
    error::{AppError, Result},
};
use axum::{
    Json,
    extract::{ConnectInfo, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool};
use std::net::SocketAddr;
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
    sqlx::raw_sql("CREATE TABLE IF NOT EXISTS rust_sessions (token_hash TEXT PRIMARY KEY, user_id BIGINT NOT NULL REFERENCES usuarios(id) ON DELETE CASCADE, csrf TEXT NOT NULL, expires_at TIMESTAMPTZ NOT NULL); CREATE INDEX IF NOT EXISTS rust_sessions_expiry ON rust_sessions(expires_at); CREATE TABLE IF NOT EXISTS rust_login_attempts (subject TEXT NOT NULL, attempted_at TIMESTAMPTZ NOT NULL DEFAULT now()); CREATE INDEX IF NOT EXISTS rust_login_attempts_subject ON rust_login_attempts(subject, attempted_at); CREATE TABLE IF NOT EXISTS rust_password_resets (token_hash TEXT PRIMARY KEY, user_id BIGINT NOT NULL REFERENCES usuarios(id) ON DELETE CASCADE, expires_at TIMESTAMPTZ NOT NULL); CREATE TABLE IF NOT EXISTS rust_email_verifications (token_hash TEXT PRIMARY KEY, user_id BIGINT NOT NULL REFERENCES usuarios(id) ON DELETE CASCADE, expires_at TIMESTAMPTZ NOT NULL);").execute(pool).await?;
    Ok(())
}
pub fn digest(value: &str) -> String {
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
/// The client address for audit and rate limiting. X-Forwarded-For is only
/// trusted when the app is explicitly configured to run behind a trusted proxy;
/// otherwise the TCP peer address is used (fail closed on the spoofable header).
pub(crate) fn client_ip(
    headers: &HeaderMap,
    trust_proxy: bool,
    peer: Option<SocketAddr>,
) -> Option<String> {
    if trust_proxy
        && let Some(ip) = headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next())
            .map(|v| v.trim().to_string())
            .filter(|v| v.parse::<std::net::IpAddr>().is_ok())
    {
        return Some(ip);
    }
    peer.map(|p| p.ip().to_string())
}
/// usuarios/signals.py: LOGIN, LOGIN_FALLIDO and LOGOUT entries.
async fn log_access(
    state: &AppState,
    user: Option<i64>,
    identifier: &str,
    action: &str,
    ip: Option<&str>,
) -> Result<()> {
    let identifier: String = identifier.chars().take(150).collect();
    sqlx::query("INSERT INTO log_actividad(usuario_id,identificador,accion,detalle,ip,creado_en) VALUES($1,$2,$3,'',$4::inet,now())")
        .bind(user).bind(identifier).bind(action).bind(ip).execute(&state.pool).await?;
    Ok(())
}
/// The core rate-limit check shared by login, registration and password reset:
/// purge stale rows, take an advisory lock, count, reject over the cap, else
/// record. A 10-minute rolling window is kept by the cleanup in each path.
pub(crate) async fn check_subject(state: &AppState, subject: &str, limit: i64) -> Result<()> {
    sqlx::query("DELETE FROM rust_login_attempts WHERE attempted_at < now()-interval '10 minutes'")
        .execute(&state.pool)
        .await?;
    let mut tx = state.pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(subject)
        .execute(&mut *tx)
        .await?;
    let attempts: i64 =
        sqlx::query_scalar("SELECT count(*) FROM rust_login_attempts WHERE subject=$1")
            .bind(subject)
            .fetch_one(&mut *tx)
            .await?;
    if attempts >= limit {
        return Err(AppError(
            StatusCode::TOO_MANY_REQUESTS,
            "Demasiadas solicitudes. Intenta de nuevo más tarde".into(),
        ));
    }
    sqlx::query("INSERT INTO rust_login_attempts(subject) VALUES($1)")
        .bind(subject)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
/// A failure cap per client IP, so a distributed brute-force across many
/// usernames from one address is still throttled (the per-account limiter alone
/// would let an attacker walk the whole namespace).
const IP_FAILURE_LIMIT: i64 = 100;
/// Failures per account from all addresses together (distributed guessing).
const ACCOUNT_FAILURE_LIMIT: i64 = 50;
async fn record_ip_failure(state: &AppState, ip: Option<&str>) -> Result<()> {
    // Unknown client IP (e.g. a test harness without a peer) skips this limiter.
    let Some(ip) = ip else { return Ok(()) };
    check_subject(state, &digest(&format!("ip:{ip}")), IP_FAILURE_LIMIT).await
}
/// Waits briefly for a password-hashing slot instead of failing at once.
pub(crate) async fn hash_slot(
    slots: &std::sync::Arc<tokio::sync::Semaphore>,
) -> Result<tokio::sync::OwnedSemaphorePermit> {
    match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        slots.clone().acquire_owned(),
    )
    .await
    {
        Ok(Ok(permit)) => Ok(permit),
        _ => Err(AppError(
            StatusCode::TOO_MANY_REQUESTS,
            "Servidor ocupado. Intenta de nuevo".into(),
        )),
    }
}
pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    peer: Option<axum::Extension<ConnectInfo<SocketAddr>>>,
    Json(input): Json<Credentials>,
) -> Result<Response> {
    // 254 characters covers the longest valid email; usernames are capped at 150
    // characters (not bytes) when they are created.
    if input.username.chars().count() > 254
        || input.password.len() > 1024
        || input.username.is_empty()
    {
        return Err(AppError::bad("Credenciales inválidas"));
    }
    // Resolve the account first: username and email then share one counter.
    // Inactive accounts are resolved too, so a correct password on one gets an
    // explicit message instead of "wrong password" (_mensaje_cuenta_inactiva).
    let record: Option<(i64, String, String, Option<String>)> =
        sqlx::query_as::<_, (i64, String, String, Option<String>)>(
            "SELECT u.id,u.username,u.password,CASE \
             WHEN u.rol='EMPRESA' AND EXISTS(SELECT 1 FROM empresas e WHERE e.usuario_id=u.id AND e.estado_listas_restrictivas='RECHAZADO') THEN 'LISTAS' \
             WHEN u.is_active AND u.activo AND u.correo_verificado THEN NULL \
             WHEN u.is_active AND u.activo AND NOT u.correo_verificado THEN 'SIN_VERIFICAR' \
             WHEN u.rol='PROFESOR' AND u.last_login IS NULL THEN 'PENDIENTE' ELSE 'INHABILITADA' END \
             FROM usuarios u WHERE u.username=$1 OR (strpos($1,'@')>0 AND lower(u.email)=lower($1) AND (SELECT count(*) FROM usuarios WHERE lower(email)=lower($1))=1) ORDER BY u.username=$1 DESC LIMIT 1",
        )
        .bind(&input.username)
        .fetch_optional(&state.pool)
        .await?
        ;
    let account_key = match &record {
        Some((id, ..)) => format!("cuenta:{id}"),
        None => input.username.to_lowercase(),
    };
    let ip = client_ip(
        &headers,
        state.config.trust_proxy,
        peer.map(|axum::Extension(ConnectInfo(addr))| addr),
    );
    // AUTHK-1: the 10-failure lock is per account AND client address, so an
    // attacker elsewhere cannot lock the owner out; an account-wide cap still
    // stops distributed guessing. Without a known address it is per account.
    let account = digest(&account_key);
    let subject = match ip.as_deref() {
        Some(ip) => digest(&format!("{account_key}|ip:{ip}")),
        None => account.clone(),
    };
    // A server too busy to hash rejects without recording an attempt; no DB
    // connection is held while waiting for a slot.
    let permit = hash_slot(&state.login_slots).await?;
    sqlx::query("DELETE FROM rust_login_attempts WHERE attempted_at < now()-interval '10 minutes'")
        .execute(&state.pool)
        .await?;
    let mut tx = state.pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(&account)
        .execute(&mut *tx)
        .await?;
    let attempts: i64 =
        sqlx::query_scalar("SELECT count(*) FROM rust_login_attempts WHERE subject=$1")
            .bind(&subject)
            .fetch_one(&mut *tx)
            .await?;
    let account_attempts: i64 = if subject == account {
        attempts
    } else {
        sqlx::query_scalar("SELECT count(*) FROM rust_login_attempts WHERE subject=$1")
            .bind(&account)
            .fetch_one(&mut *tx)
            .await?
    };
    // AUTH-4: an address over its failure cap is refused before any password check.
    let ip_failures: i64 = match ip.as_deref() {
        Some(ip) => {
            sqlx::query_scalar("SELECT count(*) FROM rust_login_attempts WHERE subject=$1")
                .bind(digest(&format!("ip:{ip}")))
                .fetch_one(&mut *tx)
                .await?
        }
        None => 0,
    };
    if attempts >= 10
        || account_attempts >= ACCOUNT_FAILURE_LIMIT
        || ip_failures >= IP_FAILURE_LIMIT
    {
        return Err(AppError(
            StatusCode::TOO_MANY_REQUESTS,
            "Demasiados intentos. Espera 10 minutos".into(),
        ));
    }
    // Recorded before verifying, so parallel guesses cannot all pass the check
    // above; a successful login clears the counter, so only failures remain.
    sqlx::query("INSERT INTO rust_login_attempts(subject) SELECT DISTINCT unnest(ARRAY[$1,$2])")
        .bind(&subject)
        .bind(&account)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    let (id, account_name, hash, inactive) = record.unwrap_or((
        0,
        String::new(),
        "pbkdf2_sha256$1000000$dummy$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".into(),
        None,
    ));
    let username = input.username.clone();
    let valid = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        verify_password(&input.password, &hash)
    })
    .await
    .map_err(|_| AppError::bad("No se pudo validar la contraseña"))?;
    if !valid || id == 0 {
        log_access(&state, None, &username, "LOGIN_FALLIDO", ip.as_deref()).await?;
        record_ip_failure(&state, ip.as_deref()).await?;
        return Err(AppError(
            StatusCode::UNAUTHORIZED,
            "Usuario o contraseña incorrectos".into(),
        ));
    }
    if let Some(reason) = inactive {
        // Right password, but the account cannot sign in: no session is opened.
        let message = match reason.as_str() {
            "LISTAS" => {
                "Tu cuenta se encuentra inhabilitada debido a que la empresa aparece en listas restrictivas (Clinton/OFAC)."
            }
            "PENDIENTE" => "Tu cuenta está pendiente de aprobación por un administrador.",
            "SIN_VERIFICAR" => "Confirma tu correo antes de iniciar sesión",
            _ => {
                "Tu cuenta se encuentra inhabilitada. Contacta con el administrador de la plataforma."
            }
        };
        tracing::warn!(
            user_id = id,
            reason = message,
            "login rejected: inactive account"
        );
        log_access(
            &state,
            Some(id),
            &account_name,
            "LOGIN_FALLIDO",
            ip.as_deref(),
        )
        .await?;
        // Only the email-verification reason is shown to the user (they must
        // know to check their inbox); every other inactive reason stays generic.
        let shown = if reason == "SIN_VERIFICAR" {
            message
        } else {
            "Usuario o contraseña incorrectos"
        };
        return Err(AppError(StatusCode::UNAUTHORIZED, shown.into()));
    }
    sqlx::query("DELETE FROM rust_login_attempts WHERE subject=$1 OR subject=$2")
        .bind(&subject)
        .bind(&account)
        .execute(&state.pool)
        .await?;
    log_access(&state, Some(id), &account_name, "LOGIN", ip.as_deref()).await?;
    start_session(&state, id).await
}
/// Opens an 8-hour session for `id` and returns the cookie response (login, registration).
pub async fn start_session(state: &AppState, id: i64) -> Result<Response> {
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
    let mut permisos = Vec::new();
    for (code, ..) in crate::permissions::PERMISOS_MODULO {
        if crate::permissions::has_perm(&state.pool, &user, code).await? {
            permisos.push(*code);
        }
    }
    // CIE-03: only admins and professors with a faculty manage study units, so
    // the SPA offers "Agregar registro" only when the server would accept it.
    let gestiona_unidades = user.admin()
        || (user.rol == "PROFESOR"
            && sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM profesores WHERE usuario_id=$1 AND facultad_id IS NOT NULL)",
            )
            .bind(user.id)
            .fetch_one(&state.pool)
            .await?);
    Ok(Json(
        json!({"user":user,"csrf":csrf,"permisos":permisos,"gestiona_unidades":gestiona_unidades}),
    ))
}
pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
    peer: Option<axum::Extension<ConnectInfo<SocketAddr>>>,
) -> Result<Response> {
    let user = current(&state, &headers, true).await?;
    let ip = client_ip(
        &headers,
        state.config.trust_proxy,
        peer.map(|axum::Extension(ConnectInfo(addr))| addr),
    );
    log_access(
        &state,
        Some(user.id),
        &user.username,
        "LOGOUT",
        ip.as_deref(),
    )
    .await?;
    sqlx::query("DELETE FROM rust_sessions WHERE token_hash=$1")
        .bind(digest(session_token(&headers).unwrap_or("")))
        .execute(&state.pool)
        .await?;
    let mut response = Json(json!({"ok":true})).into_response();
    // Mirror the session cookie's Secure attribute so the clearing header
    // matches and older non-secure cookies are not left behind.
    let secure = if state.config.secure_cookie {
        "; Secure"
    } else {
        ""
    };
    response.headers_mut().insert(
        "set-cookie",
        HeaderValue::from_str(&format!(
            "retosean_session=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0{secure}"
        ))
        .map_err(|_| AppError::bad("Cookie inválida"))?,
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
