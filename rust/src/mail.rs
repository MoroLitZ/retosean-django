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
    EVENTS.get_or_init(|| {
        serde_json::from_str(include_str!("../events.json")).expect("bundled events")
    })
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
    /// Django's console backend: emails are logged, not sent (development default).
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
        let tls = get("EMAIL_USE_TLS")
            .map(|v| ["True", "true", "1"].contains(&v.as_str()))
            .unwrap_or(true);
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
        if let (Some(user), Some(password)) = (get("EMAIL_HOST_USER"), get("EMAIL_HOST_PASSWORD")) {
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
                tracing::info!(to, subject, "email (console backend)\n{text}");
                Ok(())
            }
            Mailer::Memory(sent) => {
                sent.lock()
                    .unwrap()
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
                tracing::info!(
                    ?to,
                    subject,
                    filename,
                    "email with attachment (console backend)
{text}"
                );
                Ok(())
            }
            Mailer::Memory(sent) => {
                for address in to {
                    sent.lock().unwrap().push((
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

/// Bodies of templates/notificaciones/email/notificacion.{txt,html}.
pub fn bodies(name: &str, title: &str, message: &str, url: &str) -> (String, String) {
    let mut text = format!("Hola {name},\n\n{title}\n\n{message}\n");
    if !url.is_empty() {
        text.push_str(&format!("\nVer en la plataforma: {url}\n"));
    }
    text.push_str("\n--\nRetosEAN - Universidad EAN\nPuedes ajustar que avisos recibes desde tu perfil de notificaciones.");
    let button = if url.is_empty() {
        String::new()
    } else {
        format!(
            r#"<a href="{}" style="display:inline-block;background:#2563eb;color:#ffffff;text-decoration:none;padding:10px 18px;border-radius:6px;font-size:14px;font-weight:600;">Ver en la plataforma</a>"#,
            escape(url)
        )
    };
    let html = format!(
        r#"<!doctype html><html lang="es"><body style="margin:0;padding:24px;background:#f3f4f6;font-family:Segoe UI,Arial,sans-serif;color:#111827;"><table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="max-width:560px;margin:0 auto;background:#ffffff;border-radius:10px;overflow:hidden;"><tr><td style="background:#111827;padding:18px 24px;color:#ffffff;font-size:18px;font-weight:700;">retos<span style="color:#f59e0b;">ean</span></td></tr><tr><td style="padding:24px;"><p style="margin:0 0 12px;font-size:15px;">Hola {},</p><h2 style="margin:0 0 12px;font-size:18px;color:#111827;">{}</h2><p style="margin:0 0 20px;font-size:14px;line-height:1.6;color:#4b5563;">{}</p>{button}</td></tr><tr><td style="padding:16px 24px;background:#f9fafb;font-size:12px;color:#6b7280;">RetosEAN &middot; Universidad EAN. Puedes ajustar que avisos recibes desde tu perfil de notificaciones.</td></tr></table></body></html>"#,
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
    let mut tx = pool.begin().await?;
    let rows: Vec<(i64, String, String, String, String, String)> = sqlx::query_as(
        "SELECT n.id,u.email,coalesce(nullif(trim(u.first_name||' '||u.last_name),''),u.username),n.titulo,n.mensaje,n.link FROM notificaciones n JOIN usuarios u ON u.id=n.usuario_id LEFT JOIN preferencias_notificacion p ON p.usuario_id=u.id WHERE NOT n.enviada_por_correo AND n.error_envio='' AND n.creada_en>now()-interval '1 day' AND n.evento=ANY($1) AND u.email<>'' AND coalesce(p.recibir_email,true) ORDER BY n.id LIMIT 50 FOR UPDATE OF n SKIP LOCKED",
    )
    .bind(email_events())
    .fetch_all(&mut *tx)
    .await?;
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
                .execute(&mut *tx)
                .await?;
                sent += 1;
            }
            Err(error) => {
                tracing::warn!(id, %error, "notification email failed");
                let error: String = error.chars().take(1000).collect();
                sqlx::query("UPDATE notificaciones SET error_envio=$2 WHERE id=$1")
                    .bind(id)
                    .bind(error)
                    .execute(&mut *tx)
                    .await?;
            }
        }
    }
    tx.commit().await?;
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
            ("INFO", "Respuesta a tu postulacion", true)
        );
        assert_eq!(event("SEGUIMIENTO_REGISTRADO").1, "Seguimiento registrado");
        let (text, html) = bodies("Ana <x>", "Título", "línea 1\nlínea 2", "http://h/r/1");
        assert!(text.contains("Ver en la plataforma: http://h/r/1"));
        assert!(html.contains("Ana &lt;x&gt;") && html.contains("línea 1<br>línea 2"));
    }
}
