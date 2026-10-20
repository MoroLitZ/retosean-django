//! Daily reminders (Django `enviar_recordatorios`, HU15): same rules, messages
//! and dedupe keys, so running it twice a day never duplicates a notice.
use crate::{error::Result, workflows::notify};
use sqlx::{PgPool, Postgres, Transaction};

const ADMINS: &str = "SELECT id FROM usuarios WHERE is_active AND (rol='ADMIN' OR is_superuser)";

/// Sends `event` to every user returned by `recipients` ($1 = reto/document id)
/// who keeps reminders on; returns how many notices were created.
async fn emit(
    tx: &mut Transaction<'_, Postgres>,
    event: &str,
    recipients: &str,
    id: i64,
    message: &str,
    key: &str,
) -> Result<usize> {
    let users: Vec<i64> = sqlx::query_scalar(&format!(
        "SELECT u FROM ({recipients}) r(u) WHERE NOT EXISTS(SELECT 1 FROM preferencias_notificacion p WHERE p.usuario_id=r.u AND NOT p.recibir_recordatorios)"
    ))
    .bind(id)
    .fetch_all(&mut **tx)
    .await?;
    let before: i64 =
        sqlx::query_scalar("SELECT count(*) FROM notificaciones WHERE clave_dedupe=$1")
            .bind(key)
            .fetch_one(&mut **tx)
            .await?;
    for user in &users {
        notify(tx, *user, event, message, key).await?;
    }
    let after: i64 =
        sqlx::query_scalar("SELECT count(*) FROM notificaciones WHERE clave_dedupe=$1")
            .bind(key)
            .fetch_one(&mut **tx)
            .await?;
    Ok((after - before) as usize)
}

pub async fn run(pool: &PgPool, days: &[i64]) -> Result<usize> {
    let mut tx = pool.begin().await?;
    let today = "(now() AT TIME ZONE 'America/Bogota')::date";
    let mut total = 0;

    // Documents past their expiry become VENCIDO and the company is told (marcar_documento_vencido).
    let expired: Vec<(i64, String, Option<i64>, chrono::NaiveDate, String)> = sqlx::query_as(&format!(
        "SELECT d.id,d.estado,e.usuario_id,d.fecha_vencimiento,coalesce(t.nombre,d.tipo_documento) FROM documentos_empresas d JOIN empresas e ON e.id=d.empresa_id LEFT JOIN tipos_documento_legal t ON t.codigo=d.tipo_documento WHERE d.fecha_vencimiento<{today} AND d.estado IN ('VERIFICADO','CARGADO') FOR UPDATE OF d"
    ))
    .fetch_all(&mut *tx)
    .await?;
    for (id, from, owner, due, name) in expired {
        let reason = format!("Documento vencido el {}.", due.format("%d/%m/%Y"));
        sqlx::query("UPDATE documentos_empresas SET estado='VENCIDO' WHERE id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        crate::workflows::history(
            &mut tx,
            ("empresas", "documentoempresa"),
            id,
            &from,
            "VENCIDO",
            0,
            &reason,
        )
        .await?;
        if let Some(owner) = owner {
            notify(&mut tx, owner, "DOCUMENTO_REVISADO",
                &format!("Tu documento \"{name}\" esta vencido. {reason} Cargalo de nuevo para seguir participando."),
                &format!("documento:{id}:vencido:{due}")).await?;
        }
        total += 1;
    }

    for &n in days {
        let target = format!("({today} + {n})");
        // Application deadline: students who favorited or applied (pending/in review).
        let retos: Vec<(i64, String)> = sqlx::query_as(&format!("SELECT id,titulo FROM retos WHERE eliminado_en IS NULL AND estado='abierto' AND fecha_limite_postulacion={target}")).fetch_all(&mut *tx).await?;
        for (reto, title) in retos {
            total += emit(&mut tx,"RECORDATORIO_FECHA_LIMITE",
                "SELECT DISTINCT u.id FROM usuarios u WHERE u.rol='ESTUDIANTE' AND u.is_active AND (EXISTS(SELECT 1 FROM retos_favoritos f WHERE f.usuario_id=u.id AND f.reto_id=$1) OR EXISTS(SELECT 1 FROM postulaciones p WHERE p.estudiante_id=u.id AND p.reto_id=$1 AND p.estado IN ('PENDIENTE','EN_REVISION')))",
                reto, &format!("Quedan {n} dia(s) para postularte al reto '{title}'."),
                &format!("recordatorio:postulacion:{reto}:{n}")).await?;
        }
        // Challenge end: accepted students without a final deliverable, and the professors.
        let retos: Vec<(i64, String)> = sqlx::query_as(&format!("SELECT id,titulo FROM retos WHERE eliminado_en IS NULL AND estado IN ('aprobado','abierto','en_curso','pausado') AND fecha_fin_tentativa={target}")).fetch_all(&mut *tx).await?;
        for (reto, title) in retos {
            total += emit(&mut tx,"RECORDATORIO_FECHA_LIMITE",
                "SELECT DISTINCT p.estudiante_id FROM postulaciones p JOIN usuarios u ON u.id=p.estudiante_id WHERE p.reto_id=$1 AND p.estado='ACEPTADA' AND u.is_active AND NOT EXISTS(SELECT 1 FROM entregables e WHERE e.reto_id=$1 AND e.es_final AND e.estudiante_id=p.estudiante_id)",
                reto, &format!("Quedan {n} dia(s) para la entrega final del reto '{title}'."),
                &format!("recordatorio:entrega-final:{reto}:{n}")).await?;
            total += emit(
                &mut tx,
                "RECORDATORIO_FECHA_LIMITE",
                "SELECT DISTINCT profesor_id FROM integraciones_academicas WHERE reto_id=$1",
                reto,
                &format!("El reto '{title}' finaliza en {n} dia(s)."),
                &format!("recordatorio:fin-reto:{reto}:{n}:profesor"),
            )
            .await?;
        }
        // Verified documents about to expire.
        let docs: Vec<(i64, String)> = sqlx::query_as(&format!("SELECT d.id,coalesce(t.nombre,d.tipo_documento) FROM documentos_empresas d LEFT JOIN tipos_documento_legal t ON t.codigo=d.tipo_documento WHERE d.estado='VERIFICADO' AND d.fecha_vencimiento={target}")).fetch_all(&mut *tx).await?;
        for (doc, name) in docs {
            total += emit(&mut tx,"RECORDATORIO_FECHA_LIMITE",
                "SELECT e.usuario_id FROM documentos_empresas d JOIN empresas e ON e.id=d.empresa_id WHERE d.id=$1 AND e.usuario_id IS NOT NULL",
                doc, &format!("Tu documento '{name}' vence en {n} dia(s)."),
                &format!("recordatorio:documento:{doc}:{n}")).await?;
        }
        // Hackathon stages starting soon.
        let stages: Vec<(i64, i64, String, String)> = sqlx::query_as(&format!("SELECT s.id,h.reto_id,s.titulo,r.titulo FROM etapas_hackaton s JOIN hackathons h ON h.id=s.hackathon_id JOIN retos r ON r.id=h.reto_id WHERE (s.inicia_en AT TIME ZONE 'America/Bogota')::date={target} AND h.estado IN ('publicado','en_curso')")).fetch_all(&mut *tx).await?;
        for (stage, reto, stage_title, title) in stages {
            total += emit(&mut tx,"RECORDATORIO_FECHA_LIMITE",
                "SELECT DISTINCT p.estudiante_id FROM postulaciones p JOIN usuarios u ON u.id=p.estudiante_id WHERE p.reto_id=$1 AND p.estado='ACEPTADA' AND u.is_active",
                reto, &format!("La etapa '{stage_title}' del hackathon '{title}' inicia en {n} dia(s)."),
                &format!("recordatorio:etapa:{stage}:{n}")).await?;
        }
    }

    // Deliverables waiting for a grade longer than the longest notice period.
    let wait = days.iter().copied().max().unwrap_or(7);
    let week: String = sqlx::query_scalar(&format!("SELECT to_char({today},'YYYY-WW')"))
        .fetch_one(&mut *tx)
        .await?;
    let pending: Vec<(i64, String, i64)> = sqlx::query_as(&format!("SELECT r.id,r.titulo,count(*) FROM entregables e JOIN retos r ON r.id=e.reto_id WHERE e.estado IN ('ENVIADO','EN_REVISION') AND e.fecha_entrega<now()-interval '{wait} days' GROUP BY r.id,r.titulo")).fetch_all(&mut *tx).await?;
    for (reto, title, count) in pending {
        total += emit(&mut tx,"RECORDATORIO_FECHA_LIMITE",
            "SELECT DISTINCT profesor_id FROM integraciones_academicas WHERE reto_id=$1",
            reto, &format!("Tienes {count} entregable(s) del reto '{title}' sin calificar desde hace mas de {wait} dias."),
            &format!("recordatorio:sin-calificar:{reto}:{week}")).await?;
    }

    // HU04: active challenges past their end date without follow-ups since then (weekly).
    let iso_week: String = sqlx::query_scalar(&format!("SELECT to_char({today},'IYYY-IW')"))
        .fetch_one(&mut *tx)
        .await?;
    let stale: Vec<(i64, String, String, i32)> = sqlx::query_as(&format!("SELECT r.id,r.titulo,r.estado,({today}-r.fecha_fin_tentativa) FROM retos r WHERE r.eliminado_en IS NULL AND r.estado IN ('aprobado','abierto','en_curso','pausado') AND r.fecha_fin_tentativa<{today} AND NOT EXISTS(SELECT 1 FROM seguimientos_reto s WHERE s.reto_id=r.id AND (s.creado_en AT TIME ZONE 'America/Bogota')::date>=r.fecha_fin_tentativa)")).fetch_all(&mut *tx).await?;
    for (reto, title, state, late) in stale {
        let label = crate::dashboard::etiqueta("retos", "estado", &state);
        total += emit(
            &mut tx,
            "RETO_SIN_ACTUALIZAR",
            &format!("{ADMINS} AND $1::bigint IS NOT NULL"),
            reto,
            &format!("El reto \"{title}\" ({label}) paso su fecha de fin tentativa hace {late} dia(s) y no tiene seguimientos registrados desde entonces."),
            &format!("reto-sin-actualizar:{reto}:{iso_week}"),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(total)
}
