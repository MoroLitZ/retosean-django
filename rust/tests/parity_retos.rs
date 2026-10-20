//! apps/retos/tests.py: support files, follow-up access, professor access, the
//! academic integration flow and form, notices, the reto lifecycle, ISO dates,
//! the HU03 team and schedule, and the shared history.
mod common;
use common::*;
use serde_json::{Value, json};

fn sessions(kinds: &[&str]) -> Value {
    let base = chrono::Utc::now() + chrono::Duration::days(7);
    json!(kinds.iter().enumerate().map(|(i, k)| json!({"tipo":k,"fecha_hora":(base + chrono::Duration::days(14 * i as i64)).to_rfc3339()})).collect::<Vec<_>>())
}
/// datos_formsets(): a valid team (one professor) and the three required sessions.
fn team() -> Value {
    json!({
        "participantes":[{"rol":"PROFESOR","nombre":"Profesor Titular","documento":"1010","email":"titular@ean.edu.co","facultad":"Ingenieria"}],
        "sesiones": sessions(&["inicio","seguimiento","evaluacion"]),
    })
}
fn merge(mut a: Value, b: Value) -> Value {
    for (k, v) in b.as_object().unwrap() {
        a[k] = v.clone();
    }
    a
}
async fn estado(t: &Ctx, reto: i64) -> String {
    t.text(&format!("SELECT estado FROM retos WHERE id={reto}"))
        .await
}

// MultipleFileUploadTests
parity!(test_reto_form_accepts_multiple_files, |t| {
    let reto = t.reto(COMPANY_USER, "borrador", "reto").await;
    let (code, body) = multipart(
        &t.app,
        &t.company,
        &format!("/api/retos/{reto}/archivos"),
        &[],
        &[
            ("archivos", "soporte-1.pdf", b"uno"),
            ("archivos", "soporte-2.pdf", b"dos"),
        ],
    )
    .await;
    assert_eq!(code, 200, "{body}");
    assert_eq!(
        t.texts(&format!(
            "SELECT nombre_original FROM retos_retoarchivo WHERE reto_id={reto} ORDER BY id"
        ))
        .await,
        ["soporte-1.pdf", "soporte-2.pdf"]
    );
});
parity!(test_reto_form_rejects_a_file_larger_than_50_mb, |t| {
    let reto = t.reto(COMPANY_USER, "borrador", "reto").await;
    let big = vec![b'x'; 50 * 1024 * 1024 + 1];
    let (code, body) = multipart(
        &t.app,
        &t.company,
        &format!("/api/retos/{reto}/archivos"),
        &[],
        &[("archivos", "demasiado-grande.zip", &big)],
    )
    .await;
    assert_eq!(code, 400);
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("supera el limite de 50 MB"),
        "{body}"
    );
    assert_eq!(t.i64("SELECT count(*) FROM retos_retoarchivo").await, 0);
});
parity!(test_progress_form_has_optional_multiple_file_upload, |t| {
    let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
    let fields = [
        ("tipo_sesion", "seguimiento"),
        ("porcentaje_avance", "40"),
        ("avances", "Sin archivos"),
    ];
    let (code, body) = multipart(
        &t.app,
        &t.company,
        &format!("/api/retos/{reto}/seguimiento"),
        &fields,
        &[],
    )
    .await;
    assert_eq!((code.as_u16(), body["archivos"].as_i64()), (200, Some(0)));
    let (code, body) = multipart(
        &t.app,
        &t.company,
        &format!("/api/retos/{reto}/seguimiento"),
        &fields,
        &[
            ("archivos", "a.pdf", b"%PDF"),
            ("archivos", "b.txt", b"notas"),
        ],
    )
    .await;
    assert_eq!((code.as_u16(), body["archivos"].as_i64()), (200, Some(2)));
});

// EmpresaSeguimientoAccessTests
parity!(test_owner_sees_seguimiento_button_and_can_open_page, |t| {
    let reto = t.reto(COMPANY_USER, "borrador", "reto").await;
    t.ok(
        &t.company,
        "GET",
        &format!("/api/records/retos/{reto}"),
        None,
    )
    .await;
    t.ok(&t.company, "GET", "/api/records/seguimientos_reto", None)
        .await;
    t.ok(
        &t.company,
        "POST",
        &format!("/api/retos/{reto}/seguimiento"),
        Some(json!({"porcentaje_avance":10,"avances":"Inicio"})),
    )
    .await;
});
parity!(test_other_empresa_cannot_open_seguimiento, |t| {
    let reto = t.reto(COMPANY_USER, "borrador", "reto").await;
    t.company_user("otra_empresa", "900600100").await;
    let other = t.login("otra_empresa").await;
    assert_eq!(
        t.status(
            &other,
            "POST",
            &format!("/api/retos/{reto}/seguimiento"),
            Some(json!({"porcentaje_avance":10}))
        )
        .await,
        403
    );
});

// AccesoDelProfesorARetosTests
async fn professors(t: &Ctx) -> (Session, Session, i64) {
    t.user("profe-acceso", "PROFESOR").await;
    let linked = t.user("profe-vinculado", "PROFESOR").await;
    (
        t.login("profe-acceso").await,
        t.login("profe-vinculado").await,
        linked,
    )
}
parity!(test_profesor_abre_reto_no_aprobado_sin_error_500, |t| {
    let (teacher, _, _) = professors(&t).await;
    for state in ["borrador", "en_revision", "rechazado", "cancelado"] {
        let reto = t.reto(COMPANY_USER, state, "reto").await;
        let code = t
            .status(&teacher, "GET", &format!("/api/records/retos/{reto}"), None)
            .await;
        // No access without an integration: refused cleanly (404 hides the record).
        assert!(code == 403 || code == 404, "{state}: {code}");
    }
});
parity!(test_profesor_vinculado_si_ve_el_reto_en_borrador, |t| {
    let (_, linked, id) = professors(&t).await;
    let reto = t.reto(COMPANY_USER, "borrador", "reto").await;
    let integration = t.integrate(reto, id).await;
    t.exec(&format!(
        "UPDATE integraciones_academicas SET estado='aprobada' WHERE id={integration}"
    ))
    .await;
    t.ok(&linked, "GET", &format!("/api/records/retos/{reto}"), None)
        .await;
});
parity!(test_profesor_ve_cualquier_reto_aprobado, |t| {
    let (teacher, _, _) = professors(&t).await;
    let reto = t.reto(COMPANY_USER, "aprobado", "reto").await;
    t.ok(&teacher, "GET", &format!("/api/records/retos/{reto}"), None)
        .await;
});
parity!(
    test_boton_de_seguimiento_solo_para_el_profesor_vinculado,
    |t| {
        let (teacher, linked, id) = professors(&t).await;
        let reto = t.reto(COMPANY_USER, "aprobado", "reto").await;
        let integration = t.integrate(reto, id).await;
        t.exec(&format!(
            "UPDATE integraciones_academicas SET estado='aprobada' WHERE id={integration}"
        ))
        .await;
        t.ok(
            &linked,
            "POST",
            &format!("/api/retos/{reto}/seguimiento"),
            Some(json!({"porcentaje_avance":5})),
        )
        .await;
        assert_eq!(
            t.status(
                &teacher,
                "POST",
                &format!("/api/retos/{reto}/seguimiento"),
                Some(json!({"porcentaje_avance":5}))
            )
            .await,
            403
        );
    }
);

// FlujoIntegracionAcademicaTests
struct Flow {
    reto: i64,
    unit: i64,
    teacher: Session,
}
async fn flow(t: &Ctx) -> Flow {
    let reto = t.reto(COMPANY_USER, "aprobado", "reto").await;
    t.user("profe-flujo-int", "PROFESOR").await;
    let faculty = t.i64("INSERT INTO facultades(nombre,codigo) VALUES('Facultad de Ingenieria','FI') RETURNING id").await;
    let program = t.i64(&format!("INSERT INTO programas(nombre,facultad_id,nivel) VALUES('Ingenieria de Sistemas',{faculty},'') RETURNING id")).await;
    t.exec("INSERT INTO ecosistemas(nombre,descripcion) VALUES('Ecosistema de innovacion','')")
        .await;
    let unit = t.i64(&format!("INSERT INTO unidades_estudio(codigo,nombre,programa_id,periodo,ciclo,activo,creado_en,actualizado_en) VALUES('ISIS-101','Arquitectura de Software',{program},'2026-1','Quinto',true,now(),now()) RETURNING id")).await;
    Flow {
        reto,
        unit,
        teacher: t.login("profe-flujo-int").await,
    }
}
fn integration(unit: i64) -> Value {
    merge(
        json!({"facultad":"Facultad de Ingenieria","nivel_formacion":"Pregrado","unidad_estudio":unit,"ecosistema":"Ecosistema de innovacion","descripcion":"Integracion de prueba","alcance":"Dos unidades de estudio y una hackaton interna.","entregable_esperado":"Prototipo funcional","cronograma_sesiones":"Semana 1: kickoff; Semana 4: cierre.","requerimientos_empresa":"Datos de mercado","requerimientos_internos":"Sala de computo","espacio_fisico":"Auditorio principal","accion":"guardar"}),
        team(),
    )
}
parity!(test_crear_completar_enviar_aprobar, |t| {
    let f = flow(&t).await;
    let id = t
        .ok(
            &f.teacher,
            "POST",
            &format!("/api/retos/{}/integrar", f.reto),
            Some(integration(f.unit)),
        )
        .await["id"]
        .as_i64()
        .unwrap();
    let row: Value = sqlx::query_scalar(&format!(
        "SELECT to_jsonb(i) FROM integraciones_academicas i WHERE id={id}"
    ))
    .fetch_one(&t.pool)
    .await
    .unwrap();
    assert_eq!(row["estado"], "borrador");
    assert_eq!(row["programa_academico"], "Arquitectura de Software");
    assert_eq!(
        row["alcance"],
        "Dos unidades de estudio y una hackaton interna."
    );
    assert_eq!(
        row["cronograma_sesiones"],
        "Semana 1: kickoff; Semana 4: cierre."
    );
    assert_eq!(row["ecosistema"], "Ecosistema de innovacion");
    t.ok(
        &f.teacher,
        "POST",
        &format!("/api/integraciones/{id}/estado"),
        Some(json!({"estado":"en_revision"})),
    )
    .await;
    assert!(t.bool(&format!("SELECT estado='en_revision' AND fecha_envio_revision IS NOT NULL FROM integraciones_academicas WHERE id={id}")).await);
    t.user("admin-flujo-int", "ADMIN").await;
    let admin = t.login("admin-flujo-int").await;
    t.ok(
        &admin,
        "POST",
        &format!("/api/integraciones/{id}/estado"),
        Some(json!({"estado":"aprobada","comentario":"Todo en orden."})),
    )
    .await;
    let (state, comment): (String, String) = sqlx::query_as(&format!(
        "SELECT estado,comentarios_revision FROM integraciones_academicas WHERE id={id}"
    ))
    .fetch_one(&t.pool)
    .await
    .unwrap();
    assert_eq!(
        (state.as_str(), comment.as_str()),
        ("aprobada", "Todo en orden.")
    );
});
parity!(test_envio_sigue_bloqueado_si_falta_alcance, |t| {
    let f = flow(&t).await;
    let id = t
        .ok(
            &f.teacher,
            "POST",
            &format!("/api/retos/{}/integrar", f.reto),
            Some(integration(f.unit)),
        )
        .await["id"]
        .as_i64()
        .unwrap();
    t.exec(&format!(
        "UPDATE integraciones_academicas SET alcance='' WHERE id={id}"
    ))
    .await;
    assert_eq!(
        t.status(
            &f.teacher,
            "POST",
            &format!("/api/integraciones/{id}/estado"),
            Some(json!({"estado":"en_revision"}))
        )
        .await,
        400
    );
    assert_eq!(
        t.text(&format!(
            "SELECT estado FROM integraciones_academicas WHERE id={id}"
        ))
        .await,
        "borrador"
    );
});

// NotificacionEnvioRevisionRetoTests
parity!(test_enviar_reto_a_revision_notifica_al_admin, |t| {
    let (company, empresa) = t.company_user("empresa-envio-reto", "901111222").await;
    t.exec(&format!(
        "UPDATE empresas SET renuncio_a_convenio=true WHERE id={empresa}"
    ))
    .await;
    t.verify_documents(empresa).await;
    let reto = t.reto(company, "borrador", "reto").await;
    t.exec(&format!("UPDATE retos SET titulo='Reto para revision',fecha_inicio_tentativa=CURRENT_DATE+10,fecha_fin_tentativa=CURRENT_DATE+40,fecha_limite_postulacion=CURRENT_DATE+7 WHERE id={reto}")).await;
    let session = t.login("empresa-envio-reto").await;
    t.ok(
        &session,
        "POST",
        &format!("/api/retos/{reto}/estado"),
        Some(json!({"estado":"en_revision"})),
    )
    .await;
    assert_eq!(estado(&t, reto).await, "en_revision");
    let message = t.text(&format!("SELECT mensaje FROM notificaciones WHERE usuario_id={ADMIN} AND evento='RETO_ENVIADO_REVISION'")).await;
    assert!(message.contains("Reto para revision"), "{message}");
});

// IntegracionAcademicaSelectsTests
async fn selects(t: &Ctx) -> (i64, i64, i64, Session) {
    let reto = t.reto(COMPANY_USER, "aprobado", "reto").await;
    let teacher = t.user("profe-form-int", "PROFESOR").await;
    let ing = t.i64("INSERT INTO facultades(nombre,codigo) VALUES('Facultad de Ingenieria','FI') RETURNING id").await;
    let eco = t.i64("INSERT INTO facultades(nombre,codigo) VALUES('Facultad de Economia','FE') RETURNING id").await;
    t.exec("INSERT INTO ecosistemas(nombre,descripcion) VALUES('Ecosistema de innovacion','')")
        .await;
    let p_ing = t.i64(&format!("INSERT INTO programas(nombre,facultad_id,nivel) VALUES('Ingenieria de Sistemas',{ing},'') RETURNING id")).await;
    let p_eco = t.i64(&format!("INSERT INTO programas(nombre,facultad_id,nivel) VALUES('Economia',{eco},'') RETURNING id")).await;
    let unit = t.i64(&format!("INSERT INTO unidades_estudio(codigo,nombre,programa_id,periodo,ciclo,activo,creado_en,actualizado_en) VALUES('ING-201','Arquitectura de Software',{p_ing},'2026-2','Quinto',true,now(),now()) RETURNING id")).await;
    let inactive = t.i64(&format!("INSERT INTO unidades_estudio(codigo,nombre,programa_id,periodo,ciclo,activo,creado_en,actualizado_en) VALUES('ECO-101','Microeconomia',{p_eco},'2026-2','Primero',false,now(),now()) RETURNING id")).await;
    let _ = teacher;
    (reto, unit, inactive, t.login("profe-form-int").await)
}
fn unit_ids(body: &Value) -> Vec<i64> {
    body["unidades"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| u["value"].as_i64().unwrap())
        .collect()
}
parity!(test_rechaza_valores_fuera_del_catalogo_en_selects, |t| {
    let (reto, _, _, teacher) = selects(&t).await;
    let (code, body) = t.req(&teacher, "POST", &format!("/api/retos/{reto}/integrar"), Some(json!({"facultad":"Texto libre no permitido","nivel_formacion":"Nivel inventado","ecosistema":"Otro valor libre"}))).await;
    assert_eq!(code, 400);
    let error = body["error"].as_str().unwrap();
    for field in ["facultad", "nivel_formacion", "ecosistema"] {
        assert!(error.contains(field), "{error}");
    }
});
parity!(test_permite_editar_valores_legacy_existentes, |t| {
    let (reto, _, _, teacher) = selects(&t).await;
    let id = t.i64(&format!("INSERT INTO integraciones_academicas(reto_id,profesor_id,estado,facultad,programa_academico,nivel_formacion,ecosistema,descripcion,alcance,entregable_esperado,cronograma_sesiones) SELECT {reto},id,'borrador','Facultad legacy','','Nivel legacy','Ecosistema legacy','Desc','Alcance','Entregable','Cronograma' FROM usuarios WHERE username='profe-form-int' RETURNING id")).await;
    t.ok(&teacher, "POST", &format!("/api/integraciones/{id}"), Some(json!({"facultad":"Facultad legacy","nivel_formacion":"Nivel legacy","ecosistema":"Ecosistema legacy"}))).await;
});
parity!(
    test_unidad_estudio_lista_unidades_activas_disponibles,
    |t| {
        let (_, unit, inactive, teacher) = selects(&t).await;
        let ids = unit_ids(
            &t.ok(&teacher, "GET", "/api/integraciones/opciones", None)
                .await,
        );
        assert!(ids.contains(&unit) && !ids.contains(&inactive));
    }
);
parity!(test_unidad_estudio_se_filtra_por_facultad, |t| {
    let (_, unit, _, teacher) = selects(&t).await;
    let body = t
        .ok(
            &teacher,
            "GET",
            "/api/integraciones/opciones?facultad=Facultad%20de%20Ingenieria",
            None,
        )
        .await;
    assert_eq!(unit_ids(&body), [unit]);
});

// NotificacionPostulacionDocenteTests
parity!(
    test_enviar_postulacion_docente_notifica_a_empresa_y_admin,
    |t| {
        let (company, _) = t.company_user("empresa-post-doc", "900600200").await;
        let teacher = t.user("profe-post-doc", "PROFESOR").await;
        let admin = t.user("admin-post-doc", "ADMIN").await;
        let reto = t.reto(company, "aprobado", "reto").await;
        let session = t.login("profe-post-doc").await;
        let id = t.ok(&session, "POST", &format!("/api/retos/{reto}/integrar"), Some(merge(json!({"facultad":"Ingeniería","nivel_formacion":"Pregrado","programa_academico":"Ingenieria de Sistemas","descripcion":"Descripcion","alcance":"Alcance","entregable_esperado":"Entregable","cronograma_sesiones":"Cronograma"}), team()))).await["id"].as_i64().unwrap();
        t.ok(
            &session,
            "POST",
            &format!("/api/integraciones/{id}/estado"),
            Some(json!({"estado":"en_revision"})),
        )
        .await;
        assert_eq!(
            t.text(&format!(
                "SELECT estado FROM integraciones_academicas WHERE id={id}"
            ))
            .await,
            "en_revision"
        );
        assert_eq!(t.notices(company, "INTEGRACION_ENVIADA_REVISION").await, 1);
        assert_eq!(t.notices(admin, "INTEGRACION_ENVIADA_REVISION").await, 1);
        let _ = teacher;
    }
);

// CicloDeVidaRetoTests
async fn cycle(t: &Ctx) -> (i64, Session) {
    let (company, empresa) = t.company_user("empresa-ciclo", "900100200").await;
    t.verify_documents(empresa).await;
    (company, t.login("empresa-ciclo").await)
}
parity!(
    test_transicion_invalida_se_rechaza_sin_cambiar_estado,
    |t| {
        let (company, _) = cycle(&t).await;
        let reto = t.reto(company, "borrador", "reto").await;
        assert_eq!(
            t.status(
                &t.admin,
                "POST",
                &format!("/api/retos/{reto}/estado"),
                Some(json!({"estado":"aprobado","comentario":"x"}))
            )
            .await,
            400
        );
        assert_eq!(estado(&t, reto).await, "borrador");
        assert!(t.history("retos", "reto", reto, "estado").await.is_empty());
    }
);
#[test]
fn test_estados_finales_no_admiten_cambios() {
    for state in ["finalizado", "cancelado"] {
        assert!(
            retosean::domain::machines()["MAQUINA_RETO"][state].is_empty(),
            "{state}"
        );
    }
}
#[test]
fn test_cualquier_estado_no_final_puede_cancelarse() {
    for state in [
        "borrador",
        "en_revision",
        "aprobado",
        "abierto",
        "rechazado",
        "en_curso",
        "pausado",
    ] {
        assert!(
            retosean::domain::transition("MAQUINA_RETO", state, "cancelado").is_ok(),
            "{state}"
        );
    }
}
parity!(test_cambio_manual_solo_ofrece_transiciones_validas, |t| {
    let (company, _) = cycle(&t).await;
    let reto = t.reto(company, "en_curso", "reto").await;
    // estados_manuales_disponibles: the machine's destinations among the manual ones.
    let mut options: Vec<&str> = retosean::domain::machines()["MAQUINA_RETO"]["en_curso"]
        .iter()
        .map(String::as_str)
        .filter(|s| ["en_curso", "pausado", "cancelado"].contains(s))
        .collect();
    options.sort();
    assert_eq!(options, ["cancelado", "pausado"]);
    // "finalizado" goes through the formal close, not the manual change.
    assert_ne!(
        t.status(
            &t.admin,
            "POST",
            &format!("/api/retos/{reto}/estado"),
            Some(json!({"estado":"finalizado"}))
        )
        .await,
        200
    );
    assert_eq!(estado(&t, reto).await, "en_curso");
});
parity!(test_reto_rechazado_no_se_reenvia_sin_editar, |t| {
    let (company, session) = cycle(&t).await;
    let reto = t.reto(company, "rechazado", "reto").await;
    assert_eq!(
        t.status(
            &session,
            "POST",
            &format!("/api/retos/{reto}/estado"),
            Some(json!({"estado":"en_revision"}))
        )
        .await,
        400
    );
    assert_eq!(estado(&t, reto).await, "rechazado");
});
parity!(test_publicar_integracion_abre_el_reto_a_estudiantes, |t| {
    let (company, _) = cycle(&t).await;
    let reto = t.reto(company, "aprobado", "reto").await;
    let teacher = t.user("profe-ciclo", "PROFESOR").await;
    let integration = t.integrate(reto, teacher).await;
    t.exec(&format!(
        "UPDATE integraciones_academicas SET estado='aprobada' WHERE id={integration}"
    ))
    .await;
    let session = t.login("profe-ciclo").await;
    t.ok(
        &session,
        "POST",
        &format!("/api/integraciones/{integration}/estado"),
        Some(json!({"estado":"publicada"})),
    )
    .await;
    assert_eq!(
        t.text(&format!(
            "SELECT estado FROM integraciones_academicas WHERE id={integration}"
        ))
        .await,
        "publicada"
    );
    assert_eq!(estado(&t, reto).await, "abierto");
    assert!(
        t.history("retos", "reto", reto, "estado")
            .await
            .iter()
            .any(|s| s.0 == "aprobado" && s.1 == "abierto")
    );
});
parity!(test_explorador_solo_lista_retos_abiertos, |t| {
    let (company, _) = cycle(&t).await;
    let approved = t.reto(company, "aprobado", "reto").await;
    let open = t.reto(company, "abierto", "reto").await;
    let list = t
        .ok(&t.student, "GET", "/api/records/retos?limit=100", None)
        .await;
    let ids: Vec<i64> = list["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_i64().unwrap())
        .collect();
    assert!(ids.contains(&open) && !ids.contains(&approved));
});
parity!(
    test_no_se_puede_postular_a_un_reto_que_no_esta_abierto,
    |t| {
        let (company, _) = cycle(&t).await;
        let reto = t.reto(company, "aprobado", "reto").await;
        assert_eq!(
            t.status(
                &t.student,
                "POST",
                &format!("/api/retos/{reto}/postular"),
                Some(json!({"motivacion":"x","acepta_terminos":true}))
            )
            .await,
            400
        );
    }
);
async fn approve(t: &Ctx, reto: i64) -> String {
    t.ok(
        &t.admin,
        "POST",
        &format!("/api/retos/{reto}/estado"),
        Some(json!({"estado":"aprobado","comentario":"Ok"})),
    )
    .await;
    t.text(&format!("SELECT CASE WHEN tipo='hackathon' THEN 'HACK' ELSE 'RETO' END||'-'||anio_consecutivo||'-'||lpad(consecutivo::text,3,'0') FROM retos WHERE id={reto}")).await
}
parity!(test_consecutivos_separados_por_tipo_y_anio, |t| {
    let (company, _) = cycle(&t).await;
    t.exec("UPDATE retos SET consecutivo=NULL,anio_consecutivo=NULL")
        .await;
    let year = (chrono::Utc::now() - chrono::Duration::hours(5))
        .format("%Y")
        .to_string();
    let r1 = t.reto(company, "en_revision", "reto").await;
    let h1 = t.reto(company, "en_revision", "hackathon").await;
    let r2 = t.reto(company, "en_revision", "reto").await;
    assert_eq!(approve(&t, r1).await, format!("RETO-{year}-001"));
    assert_eq!(approve(&t, h1).await, format!("HACK-{year}-001"));
    assert_eq!(approve(&t, r2).await, format!("RETO-{year}-002"));
});
parity!(
    test_consecutivo_no_cambia_si_el_reto_vuelve_a_aprobarse,
    |t| {
        let (company, _) = cycle(&t).await;
        let reto = t.reto(company, "en_revision", "reto").await;
        let code = approve(&t, reto).await;
        t.exec(&format!(
            "UPDATE retos SET estado='en_revision' WHERE id={reto}"
        ))
        .await;
        assert_eq!(approve(&t, reto).await, code);
    }
);
parity!(
    test_eliminar_borrador_es_logico_y_queda_en_el_historial,
    |t| {
        let (company, session) = cycle(&t).await;
        let reto = t.reto(company, "borrador", "reto").await;
        t.ok(
            &session,
            "POST",
            &format!("/api/retos/{reto}/eliminar"),
            Some(json!({})),
        )
        .await;
        assert!(
            t.bool(&format!(
                "SELECT eliminado_en IS NOT NULL FROM retos WHERE id={reto}"
            ))
            .await
        );
        assert!(
            t.history("retos", "reto", reto, "estado")
                .await
                .iter()
                .any(|_| true)
        );
        assert!(t.bool(&format!("SELECT EXISTS(SELECT 1 FROM historial_estados h JOIN django_content_type c ON c.id=h.content_type_id WHERE c.model='reto' AND h.object_id={reto} AND h.comentario ILIKE '%eliminado%')")).await);
        assert_eq!(
            t.status(&session, "GET", &format!("/api/records/retos/{reto}"), None)
                .await,
            404
        );
    }
);
parity!(test_solo_se_eliminan_borradores, |t| {
    let (company, session) = cycle(&t).await;
    for state in ["en_revision", "rechazado", "cancelado"] {
        let reto = t.reto(company, state, "reto").await;
        t.status(
            &session,
            "POST",
            &format!("/api/retos/{reto}/eliminar"),
            Some(json!({})),
        )
        .await;
        assert!(
            t.bool(&format!(
                "SELECT eliminado_en IS NULL FROM retos WHERE id={reto}"
            ))
            .await,
            "{state}"
        );
    }
});
parity!(test_reto_eliminado_responde_404, |t| {
    let (company, session) = cycle(&t).await;
    let reto = t.reto(company, "borrador", "reto").await;
    t.exec(&format!(
        "UPDATE retos SET eliminado_en=now() WHERE id={reto}"
    ))
    .await;
    assert_eq!(
        t.status(&session, "GET", &format!("/api/records/retos/{reto}"), None)
            .await,
        404
    );
});
parity!(test_enviar_a_revision_no_acepta_get, |t| {
    let (company, session) = cycle(&t).await;
    let reto = t.reto(company, "borrador", "reto").await;
    assert_eq!(
        t.status(&session, "GET", &format!("/api/retos/{reto}/estado"), None)
            .await,
        405
    );
    assert_eq!(estado(&t, reto).await, "borrador");
});

// FechasEnFormulariosTests
parity!(test_editar_reto_muestra_las_fechas_en_iso, |t| {
    let reto = t.reto(COMPANY_USER, "borrador", "reto").await;
    t.exec(&format!(
        "UPDATE retos SET fecha_inicio_tentativa='2026-10-18' WHERE id={reto}"
    ))
    .await;
    let record = t
        .ok(
            &t.company,
            "GET",
            &format!("/api/records/retos/{reto}"),
            None,
        )
        .await;
    assert_eq!(record["fecha_inicio_tentativa"], "2026-10-18");
});
parity!(test_agenda_de_cierre_muestra_fecha_y_hora_en_iso, |t| {
    let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
    t.exec(&format!("INSERT INTO cierre_agendacierre(reto_id,fecha_hora,espacio,recursos,invitados,agenda,actualizado_en) VALUES({reto},'2026-10-09 08:01:00-05','','','','',now())")).await;
    let panel = t
        .ok(
            &t.company,
            "GET",
            &format!("/api/retos/{reto}/cierre"),
            None,
        )
        .await;
    assert_eq!(panel["agenda"]["fecha_hora"], "2026-10-09T08:01");
});

// EquipoYCronogramaHU03Tests
async fn hu03(t: &Ctx) -> (i64, i64, Session) {
    let reto = t.reto(COMPANY_USER, "aprobado", "reto").await;
    t.user("prof-hu03", "PROFESOR").await;
    let student = t.user("est-hu03", "ESTUDIANTE").await;
    t.exec(&format!(
        "UPDATE usuarios SET email='Est.HU03@ean.edu.co' WHERE id={student}"
    ))
    .await;
    (reto, student, t.login("prof-hu03").await)
}
fn base(accion: &str) -> Value {
    json!({"facultad":"Ingeniería","nivel_formacion":"Pregrado","programa_academico":"Unidad","descripcion":"d","alcance":"a","entregable_esperado":"Prototipo","accion":accion})
}
parity!(
    test_guarda_equipo_y_cronograma_y_enlaza_cuentas_por_correo,
    |t| {
        let (reto, student, teacher) = hu03(&t).await;
        let body = merge(
            base("guardar"),
            json!({"participantes":[
        {"rol":"PROFESOR","nombre":"Prof","documento":"1","email":"p@ean.edu.co","facultad":"Ing"},
        {"rol":"ESTUDIANTE","nombre":"Est","documento":"2","email":"est.hu03@ean.edu.co","programa":"Sis"},
        {"rol":"EXPERTO","nombre":"Mentor","documento":"3","email":"m@acme.co","facultad":"ACME"}],
        "sesiones": sessions(&["inicio","seguimiento","evaluacion"])}),
        );
        let id = t
            .ok(
                &teacher,
                "POST",
                &format!("/api/retos/{reto}/integrar"),
                Some(body),
            )
            .await["id"]
            .as_i64()
            .unwrap();
        assert_eq!(
            t.i64(&format!(
                "SELECT count(*) FROM participantes_integracion WHERE integracion_id={id}"
            ))
            .await,
            3
        );
        assert_eq!(t.i64(&format!("SELECT usuario_id FROM participantes_integracion WHERE integracion_id={id} AND rol='ESTUDIANTE'")).await, student);
        assert!(t.bool(&format!("SELECT usuario_id IS NULL FROM participantes_integracion WHERE integracion_id={id} AND rol='EXPERTO'")).await);
        assert_eq!(
            t.texts(&format!(
                "SELECT tipo FROM sesiones_reto_academico WHERE integracion_id={id} ORDER BY tipo"
            ))
            .await,
            ["evaluacion", "inicio", "seguimiento"]
        );
        assert!(t.bool(&format!("SELECT bool_and(reto_id={reto}) FROM sesiones_reto_academico WHERE integracion_id={id}")).await);
    }
);
parity!(test_cada_rol_exige_sus_datos, |t| {
    let (reto, _, teacher) = hu03(&t).await;
    let (code, body) = t
        .req(
            &teacher,
            "POST",
            &format!("/api/retos/{reto}/integrar"),
            Some(merge(
                base("guardar"),
                json!({"participantes":[{"rol":"ESTUDIANTE","nombre":"Sin datos"}]}),
            )),
        )
        .await;
    assert_eq!(code, 400, "{body}");
    assert_eq!(t.i64("SELECT count(*) FROM integraciones_academicas WHERE profesor_id=(SELECT id FROM usuarios WHERE username='prof-hu03')").await, 0);
});
parity!(
    test_no_se_envia_sin_profesor_ni_sesiones_obligatorias,
    |t| {
        let (reto, _, teacher) = hu03(&t).await;
        let body = t
            .ok(
                &teacher,
                "POST",
                &format!("/api/retos/{reto}/integrar"),
                Some(merge(
                    base("enviar"),
                    json!({"participantes":[],"sesiones":sessions(&["inicio"])}),
                )),
            )
            .await;
        assert_eq!(body["estado"], "borrador");
        let id = body["id"].as_i64().unwrap();
        let (_, error) = t
            .req(
                &teacher,
                "POST",
                &format!("/api/integraciones/{id}/estado"),
                Some(json!({"estado":"en_revision"})),
            )
            .await;
        let error = error["error"].as_str().unwrap();
        assert!(
            error.contains("al menos un profesor en el equipo"),
            "{error}"
        );
        assert!(
            error.contains("al menos una sesion de seguimiento"),
            "{error}"
        );
        assert!(error.contains("una sesion de evaluacion"), "{error}");
        assert!(!error.contains("una sesion de inicio"), "{error}");
    }
);
parity!(test_con_equipo_y_cronograma_completos_se_envia, |t| {
    let (reto, _, teacher) = hu03(&t).await;
    let body = t
        .ok(
            &teacher,
            "POST",
            &format!("/api/retos/{reto}/integrar"),
            Some(merge(base("enviar"), team())),
        )
        .await;
    assert_eq!(body["estado"], "en_revision");
});
parity!(
    test_publicar_notifica_una_vez_a_los_estudiantes_y_muestra_el_cronograma,
    |t| {
        let (reto, student, teacher) = hu03(&t).await;
        let body = merge(
            base("guardar"),
            json!({"participantes":[
        {"rol":"PROFESOR","nombre":"Profesor Titular","documento":"1010","email":"titular@ean.edu.co","facultad":"Ingenieria"},
        {"rol":"ESTUDIANTE","nombre":"Est","documento":"2","email":"est.hu03@ean.edu.co","programa":"Sis"}],
        "sesiones": sessions(&["inicio","seguimiento","evaluacion"])}),
        );
        let id = t
            .ok(
                &teacher,
                "POST",
                &format!("/api/retos/{reto}/integrar"),
                Some(body.clone()),
            )
            .await["id"]
            .as_i64()
            .unwrap();
        t.exec(&format!(
            "UPDATE integraciones_academicas SET estado='aprobada' WHERE id={id}"
        ))
        .await;
        t.ok(
            &teacher,
            "POST",
            &format!("/api/integraciones/{id}/estado"),
            Some(json!({"estado":"publicada"})),
        )
        .await;
        assert_eq!(
            t.text(&format!(
                "SELECT estado FROM integraciones_academicas WHERE id={id}"
            ))
            .await,
            "publicada"
        );
        assert_eq!(t.notices(student, "EQUIPO_ASIGNADO").await, 1);
        // Editing the team again does not notify the same student twice.
        t.ok(
            &teacher,
            "POST",
            &format!("/api/integraciones/{id}/equipo"),
            Some(json!({"participantes":body["participantes"],"sesiones":body["sesiones"]})),
        )
        .await;
        assert_eq!(t.notices(student, "EQUIPO_ASIGNADO").await, 1);
        let session = t.login("est-hu03").await;
        let schedule = t
            .ok(
                &session,
                "GET",
                "/api/records/sesiones_reto_academico?limit=100",
                None,
            )
            .await;
        assert_eq!(schedule["total"], 3);
    }
);

// BitacoraUnicaTests
parity!(
    test_el_cambio_de_estado_del_reto_queda_en_historial_estados,
    |t| {
        let (company, empresa) = t.company_user("emp-bitacora", "900600300").await;
        t.verify_documents(empresa).await;
        let reto = t.reto(company, "en_revision", "reto").await;
        t.ok(
            &t.admin,
            "POST",
            &format!("/api/retos/{reto}/estado"),
            Some(json!({"estado":"aprobado","comentario":"Listo"})),
        )
        .await;
        let steps = t.history("retos", "reto", reto, "estado").await;
        assert_eq!(steps.len(), 1);
        assert_eq!(
            (steps[0].0.as_str(), steps[0].1.as_str()),
            ("en_revision", "aprobado")
        );
        assert_eq!(
            retosean::dashboard::etiqueta("retos", "estado", "aprobado"),
            "Aprobado"
        );
    }
);
