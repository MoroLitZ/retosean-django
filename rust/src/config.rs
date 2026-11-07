use sqlx::postgres::PgConnectOptions;
use std::net::SocketAddr;

#[derive(Clone)]
pub struct Config {
    pub database: PgConnectOptions,
    pub database_name: String,
    pub bind: SocketAddr,
    pub secure_cookie: bool,
    /// True when the app is behind a trusted reverse proxy: X-Forwarded-For may
    /// then be trusted for audit/rate-limiting IPs. Otherwise only the TCP peer.
    pub trust_proxy: bool,
    pub media_root: std::path::PathBuf,
}
impl Config {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let _ = dotenvy::from_path(".env");
        let _ = dotenvy::from_path("../.env");
        let get = |key: &str, default: &str| std::env::var(key).unwrap_or_else(|_| default.into());
        let database_name = get("RUST_DB_NAME", "retosean_rust");
        if database_name.is_empty()
            || database_name.len() > 63
            || !database_name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_')
        {
            return Err("RUST_DB_NAME must contain only letters, digits and underscores".into());
        }
        // CFG: Secure session cookies by default; only an explicit opt-out (local
        // development over plain HTTP) disables the attribute, and then loudly.
        let secure_cookie = get("RUST_SECURE_COOKIE", "true") == "true";
        if !secure_cookie {
            tracing::warn!(
                "RUST_SECURE_COOKIE is false; session cookies will not carry the Secure attribute"
            );
        }
        Ok(Self {
            database: PgConnectOptions::new()
                .host(&get("DB_HOST", "localhost"))
                .port(get("DB_PORT", "5432").parse()?)
                .username(&get("DB_USER", "retosean"))
                .password(&get("DB_PASSWORD", ""))
                .database(&database_name),
            database_name,
            bind: get("RUST_BIND", "127.0.0.1:8080").parse()?,
            secure_cookie,
            trust_proxy: get("RUST_TRUST_PROXY", "false") == "true",
            media_root: get("RUST_MEDIA_DIR", ".rust-media").into(),
        })
    }
}
