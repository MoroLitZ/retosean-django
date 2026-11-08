//! Notification emails (Django apps/notificaciones): event catalog, mailer
//! configured from the same EMAIL_* settings, and an outbox worker that
//! emails pending notifications and records sent/failed like Django.
use crate::error::Result;
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor, message::MultiPart,
    transport::smtp::authentication::Credentials,
};
use serde_json::Value;
use sqlx::PgPool;
use std::sync::{Arc, Mutex, OnceLock};

fn catalog() -> &'static Value {
    static EVENTS: OnceLock<Value> = OnceLock::new();
    EVENTS.get_or_init(|| parse_catalog(include_str!("../events.json")))
}

/// #93: parsing the bundled catalog cannot panic the server at runtime. A
/// broken events.json is logged and degrades to an empty catalog: every event
/// then uses Django's generic title (see `event`) and no email goes out until
/// the file is fixed (`email_events` returns nothing).
pub fn parse_catalog(text: &str) -> Value {
    match serde_json::from_str(text) {
        Ok(catalog) => catalog,
        Err(error) => {
            tracing::error!(%error, "invalid events.json; falling back to an empty catalog");
            Value::Object(serde_json::Map::new())
        }
    }
}

/// (tipo, titulo, permite_email) of an event, with Django's generic fallback.
pub fn event(slug: &str) -> (String, String, bool) {
    match catalog().get(slug) {
        Some(e) => (
            e["tipo"].as_str().unwrap_or("INFO").into(),
            e["titulo"].as_str().unwrap_or(slug).into(),
            e["permite_email"].as_bool().unwrap_or(true),
        ),
        None => {
            let text = slug.replace('_', " ").to_lowercase();
            let mut chars = text.chars();
            let title = chars
                .next()
                .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default();
            ("INFO".into(), title, true)
        }
    }
}

/// Events whose notifications may go out by email.
pub fn email_events() -> Vec<String> {
    catalog()
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(_, e)| e["permite_email"].as_bool().unwrap_or(true))
        .map(|(slug, _)| slug.clone())
        .collect()
}

#[derive(Clone)]
pub enum Mailer {
    /// Django's console backend: emails are logged (redacted, CFG-10), not
    /// sent (development default).
    Console,
    Smtp {
        transport: AsyncSmtpTransport<Tokio1Executor>,
        from: String,
    },
    /// Collected in memory, for tests.
    Memory(Arc<Mutex<Vec<(String, String, String)>>>),
}

impl Mailer {
    /// Same settings as config/settings: EMAIL_BACKEND, EMAIL_HOST, EMAIL_PORT,
    /// EMAIL_HOST_USER, EMAIL_HOST_PASSWORD, EMAIL_USE_TLS, DEFAULT_FROM_EMAIL.
    pub fn from_env() -> Self {
        let get = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let backend = get("EMAIL_BACKEND").unwrap_or_default();
        let Some(host) = get("EMAIL_HOST") else {
            return Mailer::Console;
        };
        if backend.contains("console") || backend.contains("locmem") {
            return Mailer::Console;
        }
        let port: u16 = get("EMAIL_PORT")
            .and_then(|p| p.parse().ok())
            .unwrap_or(587);
        let user = get("EMAIL_HOST_USER");
        let password = get("EMAIL_HOST_PASSWORD");
        let tls = get("EMAIL_USE_TLS")
            .map(|v| ["True", "true", "1"].contains(&v.as_str()))
            .unwrap_or(true);
        // Never send SMTP credentials over plaintext (WF-5): with TLS disabled and
        // credentials configured the mailer fails back to the console backend.
        if !tls && (user.is_some() || password.is_some()) {
            tracing::error!(
                "EMAIL_USE_TLS is false but SMTP credentials are set; refusing plaintext auth, emails will only be logged"
            );
            return Mailer::Console;
        }
        let builder = if tls {
            match AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&host) {
                Ok(b) => b,
                Err(error) => {
                    tracing::error!(%error, "invalid EMAIL_HOST; emails will be logged");
                    return Mailer::Console;
                }
            }
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&host)
        };
        let mut builder = builder
            .port(port)
            .timeout(Some(std::time::Duration::from_secs(10)));
        if let (Some(user), Some(password)) = (user, password) {
            builder = builder.credentials(Credentials::new(user, password));
        }
        Mailer::Smtp {
            transport: builder.build(),
            from: get("DEFAULT_FROM_EMAIL")
                .unwrap_or_else(|| "RetosEAN <no-responder@universidadean.edu.co>".into()),
        }
    }

    pub async fn send(
        &self,
        to: &str,
        subject: &str,
        text: &str,
        html: &str,
    ) -> std::result::Result<(), String> {
        match self {
            Mailer::Console => {
                // CFG-10: the body is logged redacted, so recovery tokens and
                // key=value secrets never reach the logs.
                tracing::info!(to, subject, "email (console backend)\n{}", redact(text));
                Ok(())
            }
            Mailer::Memory(sent) => {
                // #93: a poisoned mutex (a panic while holding the lock) must
                // not take down the caller; the buffer is still usable.
                sent.lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push((to.into(), subject.into(), text.into()));
                Ok(())
            }
            Mailer::Smtp { transport, from } => {
                let message = Message::builder()
                    .from(
                        from.parse()
                            .map_err(|e| format!("DEFAULT_FROM_EMAIL: {e}"))?,
                    )
                    .to(to.parse().map_err(|e| format!("destinatario: {e}"))?)
                    .subject(subject)
                    .multipart(MultiPart::alternative_plain_html(
                        text.to_string(),
                        html.to_string(),
                    ))
                    .map_err(|e| e.to_string())?;
                transport
                    .send(message)
                    .await
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
        }
    }
}

impl Mailer {
    /// Plain message with one attachment (reports sent by email, HU06).
    pub async fn send_attachment(
        &self,
        to: &[String],
        subject: &str,
        text: &str,
        filename: &str,
        bytes: Vec<u8>,
        mime: &str,
    ) -> std::result::Result<(), String> {
        match self {
            Mailer::Console => {
                // CFG-10: same redaction as send().
                tracing::info!(
                    ?to,
                    subject,
                    filename,
                    "email with attachment (console backend)
{}",
                    redact(text)
                );
                Ok(())
            }
            Mailer::Memory(sent) => {
                // Like an SMTP server refusing it: the reserved .invalid TLD (RFC 2606) fails.
                if let Some(bad) = to.iter().find(|a| a.ends_with(".invalid")) {
                    return Err(format!("SMTP caido: {bad}"));
                }
                for address in to {
                    // #93: same poisoned-mutex recovery as send().
                    sent.lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .push((
                            address.clone(),
                            subject.into(),
                            format!(
                                "{text}
[adjunto: {filename}]"
                            ),
                        ));
                }
                Ok(())
            }
            Mailer::Smtp { transport, from } => {
                use lettre::message::{Attachment, SinglePart, header::ContentType};
                let mut builder = Message::builder()
                    .from(
                        from.parse()
                            .map_err(|e| format!("DEFAULT_FROM_EMAIL: {e}"))?,
                    )
                    .subject(subject);
                for address in to {
                    builder =
                        builder.to(address.parse().map_err(|e| format!("destinatario: {e}"))?);
                }
                let kind = ContentType::parse(mime).map_err(|e| e.to_string())?;
                let message = builder
                    .multipart(
                        MultiPart::mixed()
                            .singlepart(SinglePart::plain(text.to_string()))
                            .singlepart(Attachment::new(filename.to_string()).body(bytes, kind)),
                    )
                    .map_err(|e| e.to_string())?;
                transport
                    .send(message)
                    .await
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
        }
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn starts_with_ci(chars: &[char], at: usize, needle: &str) -> bool {
    needle.chars().enumerate().all(|(i, c)| {
        chars
            .get(at + i)
            .is_some_and(|x| x.to_lowercase().eq(c.to_lowercase()))
    })
}

/// CFG-10: redaction for whatever reaches the logs, like Django's
/// FormateadorSeguro (config/registro.py): `clave=valor` and `clave: valor`
/// pairs of sensitive keys become `clave=***`, and the token of a password
/// recovery link is replaced so the console backend never leaks it.
pub fn redact(text: &str) -> String {
    const KEYS: [&str; 7] = [
        "password",
        "passwd",
        "contrasena",
        "contraseña",
        "token",
        "secret",
        "authorization",
    ];
    // Recovery and email-verification links carry single-use tokens.
    const LINKS: [&str; 2] = ["#restablecer/", "#verificar/"];
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        // Token links: the token (64 hex characters) is replaced by ***.
        if let Some(link) = LINKS.iter().find(|link| starts_with_ci(&chars, i, link)) {
            out.extend(&chars[i..i + link.chars().count()]);
            i += link.chars().count();
            let start = i;
            while i < chars.len() && chars[i].is_alphanumeric() {
                i += 1;
            }
            if i - start >= 20 {
                out.push_str("***");
            } else {
                out.extend(&chars[start..i]);
            }
            continue;
        }
        // Django's pattern: key, optional quote, spaces, ':' or '=' and a value.
        if let Some(key) = KEYS.iter().find(|key| starts_with_ci(&chars, i, key)) {
            let mut j = i + key.chars().count();
            if matches!(chars.get(j), Some('\'') | Some('"')) {
                j += 1;
            }
            while matches!(chars.get(j), Some(c) if c.is_whitespace()) {
                j += 1;
            }
            if matches!(chars.get(j), Some(':') | Some('=')) {
                j += 1;
                while matches!(chars.get(j), Some(c) if c.is_whitespace()) {
                    j += 1;
                }
                let value = j;
                while matches!(chars.get(j), Some(c) if !c.is_whitespace()) {
                    j += 1;
                }
                if j > value {
                    out.extend(&chars[i..value]);
                    out.push_str("***");
                    i = j;
                    continue;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Bodies of templates/notificaciones/email/notificacion.{txt,html}.
pub fn bodies(name: &str, title: &str, message: &str, url: &str) -> (String, String) {
    let mut text = format!("Hola {name},\n\n{title}\n\n{message}\n");
    if !url.is_empty() {
        text.push_str(&format!("\nVer en la plataforma: {url}\n"));
    }
    text.push_str("\n--\nRetosEAN - Universidad EAN\nPuedes ajustar qué avisos recibes desde tu perfil de notificaciones.");
    let button = if url.is_empty() {
        String::new()
    } else {
        format!(
            r#"<a href="{}" style="display:inline-block;background:#2563eb;color:#ffffff;text-decoration:none;padding:10px 18px;border-radius:6px;font-size:14px;font-weight:600;">Ver en la plataforma</a>"#,
            escape(url)
        )
    };
    let html = format!(
        r#"<!doctype html><html lang="es"><body style="margin:0;padding:24px;background:#f3f4f6;font-family:Segoe UI,Arial,sans-serif;color:#111827;"><table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="max-width:560px;margin:0 auto;background:#ffffff;border-radius:10px;overflow:hidden;"><tr><td style="background:#111827;padding:18px 24px;color:#ffffff;font-size:18px;font-weight:700;">retos<span style="color:#f59e0b;">ean</span></td></tr><tr><td style="padding:24px;"><p style="margin:0 0 12px;font-size:15px;">Hola {},</p><h2 style="margin:0 0 12px;font-size:18px;color:#111827;">{}</h2><p style="margin:0 0 20px;font-size:14px;line-height:1.6;color:#4b5563;">{}</p>{button}</td></tr><tr><td style="padding:16px 24px;background:#f9fafb;font-size:12px;color:#6b7280;">RetosEAN &middot; Universidad EAN. Puedes ajustar qué avisos recibes desde tu perfil de notificaciones.</td></tr></table></body></html>"#,
        escape(name),
        escape(title),
        escape(message).replace('\n', "<br>")
    );
    (text, html)
}

/// Emails pending notifications (recent, email-enabled event, user with an email
/// who did not opt out) and records enviada_por_correo / error_envio like Django.
/// Rows stay locked while sending, so concurrent workers never send twice.
pub async fn deliver_pending(pool: &PgPool, mailer: &Mailer, site_url: &str) -> Result<usize> {
    // Rows are claimed with a timestamped sentinel ("ENVIANDO:<epoch>") while
    // sending, without holding a transaction or row locks during SMTP I/O
    // (WF-6). Only claims older than 10 minutes (a crashed worker) are
    // recovered, so another instance can never re-send a row being sent now.
    sqlx::query(
        "UPDATE notificaciones SET error_envio='' WHERE (error_envio='ENVIANDO' AND creada_en<now()-interval '10 minutes') OR CASE WHEN error_envio ~ '^ENVIANDO:[0-9]{1,18}$' THEN split_part(error_envio,':',2)::bigint < extract(epoch FROM now() - interval '10 minutes')::bigint ELSE false END",
    )
    .execute(pool)
    .await?;
    let rows: Vec<(i64, String, String, String, String, String)> = {
        let mut tx = pool.begin().await?;
        let rows: Vec<(i64, String, String, String, String, String)> = sqlx::query_as(
            "SELECT n.id,u.email,coalesce(nullif(trim(u.first_name||' '||u.last_name),''),u.username),n.titulo,n.mensaje,n.link FROM notificaciones n JOIN usuarios u ON u.id=n.usuario_id LEFT JOIN preferencias_notificacion p ON p.usuario_id=u.id WHERE NOT n.enviada_por_correo AND n.error_envio='' AND n.creada_en>now()-interval '1 day' AND n.evento=ANY($1) AND u.email<>'' AND coalesce(p.recibir_email,true) ORDER BY n.id LIMIT 50 FOR UPDATE OF n SKIP LOCKED",
        )
        .bind(email_events())
        .fetch_all(&mut *tx)
        .await?;
        let ids: Vec<i64> = rows.iter().map(|(id, ..)| *id).collect();
        sqlx::query("UPDATE notificaciones SET error_envio='ENVIANDO:'||extract(epoch FROM now())::bigint WHERE id=ANY($1)")
            .bind(ids.as_slice())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        rows
    };
    let mut sent = 0;
    for (id, email, name, title, message, link) in rows {
        let url = if link.is_empty() || link == "/" {
            String::new()
        } else {
            format!("{}{link}", site_url.trim_end_matches('/'))
        };
        let (text, html) = bodies(&name, &title, &message, &url);
        match mailer
            .send(&email, &format!("[RetosEAN] {title}"), &text, &html)
            .await
        {
            Ok(()) => {
                sqlx::query(
                    "UPDATE notificaciones SET enviada_por_correo=true,error_envio='' WHERE id=$1",
                )
                .bind(id)
                .execute(pool)
                .await?;
                sent += 1;
            }
            Err(error) => {
                tracing::warn!(id, %error, "notification email failed");
                let error: String = error.chars().take(1000).collect();
                sqlx::query("UPDATE notificaciones SET error_envio=$2 WHERE id=$1")
                    .bind(id)
                    .bind(error)
                    .execute(pool)
                    .await?;
            }
        }
    }
    Ok(sent)
}

static GLOBAL: OnceLock<Mailer> = OnceLock::new();

/// Process-wide mailer (set by `serve`; tests may install a Memory mailer).
pub fn install(mailer: Mailer) -> &'static Mailer {
    GLOBAL.get_or_init(|| mailer)
}
pub fn global() -> &'static Mailer {
    GLOBAL.get_or_init(|| Mailer::Console)
}

/// Sends a plain message after the request, never failing it (Django fail_silently=True).
pub fn send_later(to: String, subject: String, text: String) {
    if to.is_empty() {
        return;
    }
    tokio::spawn(async move {
        let html = format!(
            "<pre style=\"font-family:Segoe UI,Arial,sans-serif\">{}</pre>",
            escape(&text)
        );
        if let Err(error) = global().send(&to, &subject, &text, &html).await {
            tracing::warn!(%error, "email failed");
        }
    });
}

/// Background outbox: every 20 seconds, well inside the brief's 2 minutes.
pub fn spawn_outbox(pool: PgPool, mailer: Mailer, site_url: String) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(20));
        loop {
            tick.tick().await;
            if let Err(error) = deliver_pending(&pool, &mailer, &site_url).await {
                tracing::warn!(error = %error.1, "notification outbox failed");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogo_y_plantilla_como_django() {
        let (tipo, titulo, email) = event("POSTULACION_GESTIONADA");
        assert_eq!(
            (tipo.as_str(), titulo.as_str(), email),
            ("INFO", "Respuesta a tu postulación", true)
        );
        assert_eq!(event("SEGUIMIENTO_REGISTRADO").1, "Seguimiento registrado");
        let (text, html) = bodies("Ana <x>", "Título", "línea 1\nlínea 2", "http://h/r/1");
        assert!(text.contains("Ver en la plataforma: http://h/r/1"));
        assert!(html.contains("Ana &lt;x&gt;") && html.contains("línea 1<br>línea 2"));
    }

    #[test]
    fn test_el_formateador_seguro_redacta_datos_sensibles() {
        // Same case as Django's RegistroSeguroTests (CFG-10 / MAN-CFG-08).
        let salida = redact("POST login password=hunter2 csrfmiddlewaretoken=abc123token");
        assert!(!salida.contains("hunter2"));
        assert!(!salida.contains("abc123token"));
        assert!(salida.contains("password=***"));
        assert!(salida.contains("token=***"));
        let salida = redact("contraseña: mi-clave-larga\ntoken = \"secreto\"");
        assert!(!salida.contains("mi-clave-larga"));
        assert!(!salida.contains("secreto"));
        // Ordinary prose that merely mentions the words is left untouched.
        let body = "Para elegir una nueva contraseña, abre el siguiente enlace: /";
        assert_eq!(redact(body), body);
    }

    #[test]
    fn test_consola_no_registra_el_token_de_restablecimiento() {
        let token = "a".repeat(64);
        let body = format!("Hola Ana:\n\nAbre:\n\nhttp://sitio/#restablecer/{token}\n\nGracias");
        let salida = redact(&body);
        assert!(!salida.contains(&token), "the token would leak into logs");
        assert!(salida.contains("#restablecer/***"));
        assert!(salida.contains("Hola Ana:"), "the body stays readable");
        let verify = format!("Confirma: http://sitio/#verificar/{token}");
        assert_eq!(redact(&verify), "Confirma: http://sitio/#verificar/***");
    }
}
