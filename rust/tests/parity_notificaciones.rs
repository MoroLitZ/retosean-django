//! apps/notificaciones/tests.py: the notification engine, domain notices, the
//! inbox, the overdue-reto alert (HU04) and the admin delivery log (HU15).
mod common;
use common::*;
use serde_json::json;

async fn notify(t: &Ctx, user: i64, event: &str, message: &str, key: &str) {
    let mut tx = t.pool.begin().await.unwrap();
    retosean::workflows::notify(&mut tx, user, event, message, key)
        .await
        .unwrap();
    tx.commit().await.unwrap();
}
/// Runs the outbox once with a private memory mailer; returns what it sent.
async fn deliver(t: &Ctx) -> Vec<(String, String, String)> {
    let sent = Outbox::default();
    let mailer = retosean::mail::Mailer::Memory(sent.clone());
    retosean::mail::deliver_pending(&t.pool, &mailer, "http://test")
        .await
        .unwrap();
    let mails = sent.lock().unwrap().clone();
    mails
}
async fn recipient(t: &Ctx) -> i64 {
    t.user("destinatario", "ESTUDIANTE").await
}

// MotorNotificacionesTests
parity!(test_notificar_crea_registro_y_envia_correo, |t| {
    let user = recipient(&t).await;
    notify(
        &t,
        user,
        "POSTULACION_GESTIONADA",
        "Fuiste aceptado.",
        "k-gestionada",
    )
    .await;
    let (event, read): (String, bool) = sqlx::query_as(&format!(
        "SELECT evento,leida FROM notificaciones WHERE usuario_id={user}"
    ))
    .fetch_one(&t.pool)
    .await
    .unwrap();
    assert_eq!((event.as_str(), read), ("POSTULACION_GESTIONADA", false));
    let mails = deliver(&t).await;
    let mine: Vec<_> = mails
        .iter()
        .filter(|m| m.0 == "destinatario@universidadean.edu.co")
        .collect();
    assert_eq!(mine.len(), 1);
    assert!(
        mine[0].1.contains("Respuesta a tu postulacion"),
        "{}",
        mine[0].1
    );
});
parity!(test_clave_dedupe_evita_duplicados, |t| {
    let user = recipient(&t).await;
    notify(&t, user, "RETO_ESTADO_CAMBIADO", "uno", "k1").await;
    notify(&t, user, "RETO_ESTADO_CAMBIADO", "dos", "k1").await;
    assert_eq!(
        t.i64(&format!(
            "SELECT count(*) FROM notificaciones WHERE usuario_id={user}"
        ))
        .await,
        1
    );
});
parity!(test_evento_silenciado_no_genera_notificacion, |t| {
    let user = recipient(&t).await;
    t.exec(&format!("INSERT INTO preferencias_notificacion(usuario_id,recibir_email,recibir_recordatorios,eventos_silenciados) VALUES({user},true,true,'[\"ENTREGABLE_CALIFICADO\"]')")).await;
    notify(&t, user, "ENTREGABLE_CALIFICADO", "4.5", "k-calificado").await;
    assert_eq!(
        t.i64(&format!(
            "SELECT count(*) FROM notificaciones WHERE usuario_id={user}"
        ))
        .await,
        0
    );
});
parity!(test_sin_email_activo_solo_queda_in_app, |t| {
    let user = recipient(&t).await;
    t.exec(&format!("INSERT INTO preferencias_notificacion(usuario_id,recibir_email,recibir_recordatorios,eventos_silenciados) VALUES({user},false,true,'[]')")).await;
    notify(&t, user, "POSTULACION_NUEVA", "hola", "k-nueva").await;
    assert_eq!(
        t.i64(&format!(
            "SELECT count(*) FROM notificaciones WHERE usuario_id={user}"
        ))
        .await,
        1
    );
    assert!(
        deliver(&t)
            .await
            .iter()
            .all(|m| m.0 != "destinatario@universidadean.edu.co")
    );
});
parity!(test_notificar_admins_incluye_admin_por_rol, |t| {
    let superuser = t.user("super", "ESTUDIANTE").await;
    t.exec(&format!(
        "UPDATE usuarios SET is_superuser=true WHERE id={superuser}"
    ))
    .await;
    let by_role = t.user("admin-rol", "ADMIN").await;
    // A logistics request notifies every administrator (notificar_admins).
    let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
    t.ok(
        &t.company,
        "POST",
        &format!("/api/retos/{reto}/solicitudes-logistica"),
        Some(json!({"evento":"CIERRE","tipo":"OTRO","descripcion":"nueva"})),
    )
    .await;
    assert_eq!(t.notices(superuser, "LOGISTICA_SOLICITADA").await, 1);
    assert_eq!(t.notices(by_role, "LOGISTICA_SOLICITADA").await, 1);
});

// NotificacionesDeDominioTests
async fn in_review(t: &Ctx) -> (i64, i64) {
    let (company, _) = t.company_user("empresa-notif", "900400100").await;
    (t.reto(company, "en_revision", "reto").await, company)
}
parity!(test_aprobar_reto_notifica_a_la_empresa, |t| {
    let (reto, company) = in_review(&t).await;
    t.ok(
        &t.admin,
        "POST",
        &format!("/api/retos/{reto}/estado"),
        Some(json!({"estado":"aprobado","comentario":"Todo correcto."})),
    )
    .await;
    let (event, message): (String, String) = sqlx::query_as(&format!(
        "SELECT evento,mensaje FROM notificaciones WHERE usuario_id={company}"
    ))
    .fetch_one(&t.pool)
    .await
    .unwrap();
    assert_eq!(event, "RETO_ESTADO_CAMBIADO");
    assert!(message.to_lowercase().contains("aprobado"), "{message}");
    assert!(message.contains("Todo correcto."), "{message}");
});
parity!(test_rechazar_reto_notifica_con_el_comentario, |t| {
    let (reto, company) = in_review(&t).await;
    t.ok(
        &t.admin,
        "POST",
        &format!("/api/retos/{reto}/estado"),
        Some(json!({"estado":"rechazado","comentario":"Falta el alcance."})),
    )
    .await;
    let (kind, message): (String, String) = sqlx::query_as(&format!(
        "SELECT tipo,mensaje FROM notificaciones WHERE usuario_id={company}"
    ))
    .fetch_one(&t.pool)
    .await
    .unwrap();
    assert_eq!(kind, "ERROR");
    assert!(message.contains("Falta el alcance."), "{message}");
});
parity!(test_estado_sin_cambio_se_rechaza_y_no_notifica, |t| {
    let (reto, company) = in_review(&t).await;
    assert_eq!(
        t.status(
            &t.admin,
            "POST",
            &format!("/api/retos/{reto}/estado"),
            Some(json!({"estado":"en_revision"}))
        )
        .await,
        400
    );
    assert_eq!(
        t.i64(&format!(
            "SELECT count(*) FROM notificaciones WHERE usuario_id={company}"
        ))
        .await,
        0
    );
});

// BandejaTests
async fn inbox(t: &Ctx) -> (i64, Session) {
    let user = t.user("lector", "ESTUDIANTE").await;
    let id = t.i64(&format!("INSERT INTO notificaciones(usuario_id,tipo,evento,titulo,mensaje,leida,link,clave_dedupe,enviada_por_correo,error_envio,creada_en) VALUES({user},'INFO','','Aviso','cuerpo',false,'','',false,'',now()) RETURNING id")).await;
    (id, t.login("lector").await)
}
parity!(test_bandeja_lista_las_notificaciones, |t| {
    let (_, session) = inbox(&t).await;
    assert!(
        t.ok(&session, "GET", "/api/records/notificaciones", None)
            .await
            .to_string()
            .contains("Aviso")
    );
});
parity!(test_marcar_leida_requiere_post, |t| {
    let (id, session) = inbox(&t).await;
    assert_eq!(
        t.status(
            &session,
            "GET",
            &format!("/api/notificaciones/{id}/leer"),
            None
        )
        .await,
        405
    );
});
parity!(test_marcar_leida_actualiza_el_registro, |t| {
    let (id, session) = inbox(&t).await;
    t.ok(
        &session,
        "POST",
        &format!("/api/notificaciones/{id}/leer"),
        Some(json!({})),
    )
    .await;
    assert!(
        t.bool(&format!(
            "SELECT leida AND leida_en IS NOT NULL FROM notificaciones WHERE id={id}"
        ))
        .await
    );
});
parity!(test_no_se_puede_leer_la_notificacion_de_otro, |t| {
    let (id, _) = inbox(&t).await;
    t.user("otro", "ESTUDIANTE").await;
    let other = t.login("otro").await;
    assert_eq!(
        t.status(
            &other,
            "POST",
            &format!("/api/notificaciones/{id}/leer"),
            Some(json!({}))
        )
        .await,
        404
    );
    assert!(
        !t.bool(&format!("SELECT leida FROM notificaciones WHERE id={id}"))
            .await
    );
});
parity!(test_el_badge_aparece_en_el_topbar, |t| {
    let (_, session) = inbox(&t).await;
    // The bell counts the unread notifications of the user's list.
    let list = t
        .ok(
            &session,
            "GET",
            "/api/records/notificaciones?limit=100",
            None,
        )
        .await;
    let unread = list["records"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| n["leida"] == false)
        .count();
    assert_eq!(unread, 1);
});

// RetoVencidoSinActualizarTests
async fn overdue(t: &Ctx) -> (i64, i64) {
    let admin = t.user("admin-venc", "ADMIN").await;
    let (company, _) = t.company_user("empresa-venc", "900400200").await;
    let late = t.reto(company, "en_curso", "reto").await;
    t.exec(&format!(
        "UPDATE retos SET titulo='Reto atrasado',fecha_fin_tentativa=CURRENT_DATE-5 WHERE id={late}"
    ))
    .await;
    let fine = t.reto(company, "en_curso", "reto").await;
    t.exec(&format!(
        "UPDATE retos SET titulo='Reto al dia',fecha_fin_tentativa=CURRENT_DATE+30 WHERE id={fine}"
    ))
    .await;
    (admin, late)
}
async fn remind(t: &Ctx, admin: i64) -> Vec<String> {
    retosean::reminders::run(&t.pool, &[7, 3, 1]).await.unwrap();
    t.texts(&format!("SELECT mensaje FROM notificaciones WHERE usuario_id={admin} AND evento='RETO_SIN_ACTUALIZAR'")).await
}
parity!(test_avisa_al_admin_del_reto_vencido, |t| {
    let (admin, _) = overdue(&t).await;
    let notices = remind(&t, admin).await;
    assert_eq!(notices.len(), 1, "{notices:?}");
    assert!(notices[0].contains("Reto atrasado"));
});
parity!(test_no_repite_el_aviso_la_misma_semana, |t| {
    let (admin, _) = overdue(&t).await;
    remind(&t, admin).await;
    assert_eq!(remind(&t, admin).await.len(), 1);
});
parity!(test_un_seguimiento_posterior_silencia_la_alerta, |t| {
    let (admin, late) = overdue(&t).await;
    t.exec(&format!("INSERT INTO seguimientos_reto(reto_id,tipo_sesion,fecha_sesion,porcentaje_avance,avances,observaciones,acuerdos,creado_en) VALUES({late},'seguimiento',CURRENT_DATE,90,'Casi listo','','',now())")).await;
    assert_eq!(remind(&t, admin).await.len(), 0);
});

// RegistroEnviosAdminTests
async fn log(t: &Ctx) {
    let user = t.user("destino-reg", "ESTUDIANTE").await;
    for (title, sent, error, read) in [
        ("Enviada", true, "", false),
        ("Con fallo", false, "SMTP caido", false),
        ("Leida", false, "", true),
    ] {
        sqlx::query("INSERT INTO notificaciones(usuario_id,tipo,evento,titulo,mensaje,leida,link,clave_dedupe,enviada_por_correo,error_envio,creada_en) VALUES($1,'INFO','',$2,'m',$3,'','',$4,$5,now())")
            .bind(user).bind(title).bind(read).bind(sent).bind(error).execute(&t.pool).await.unwrap();
    }
}
parity!(test_filtra_por_estado_de_envio, |t| {
    log(&t).await;
    let admin = {
        t.user("admin-reg", "ADMIN").await;
        t.login("admin-reg").await
    };
    let titles = |body: serde_json::Value| -> Vec<String> {
        body["registros"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["username"] == "destino-reg")
            .map(|r| r["titulo"].as_str().unwrap().to_string())
            .collect()
    };
    for (state, expected) in [
        ("fallido", "Con fallo"),
        ("enviado", "Enviada"),
        ("leido", "Leida"),
    ] {
        let body = t
            .ok(
                &admin,
                "GET",
                &format!("/api/notificaciones/registro?estado={state}"),
                None,
            )
            .await;
        assert_eq!(titles(body), [expected], "{state}");
    }
    let all = t
        .ok(&admin, "GET", "/api/notificaciones/registro", None)
        .await;
    assert_eq!(all["resumen"]["fallidas"], 1);
});
parity!(test_solo_el_admin_lo_ve, |t| {
    log(&t).await;
    let user = t.login("destino-reg").await;
    assert_ne!(
        t.status(&user, "GET", "/api/notificaciones/registro", None)
            .await,
        200
    );
});
