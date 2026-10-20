//! apps/participaciones/tests.py: deliverable access, favorites, applications
//! (program catalog, terms, CV, notices) and the company's review.
mod common;
use common::*;
use serde_json::json;

const PDF: &[u8] = b"%PDF-1.4 contenido";

async fn upload(t: &Ctx, session: &Session, reto: i64) -> u16 {
    multipart(
        &t.app,
        session,
        &format!("/api/retos/{reto}/entregar"),
        &[("titulo", "Avance")],
        &[("archivo", "avance.pdf", PDF)],
    )
    .await
    .0
    .as_u16()
}
async fn deliverables(t: &Ctx, reto: i64, student: i64) -> i64 {
    t.i64(&format!(
        "SELECT count(*) FROM entregables WHERE reto_id={reto} AND estudiante_id={student}"
    ))
    .await
}

// AccesoAEntregablesTests
async fn access(t: &Ctx) -> i64 {
    let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
    let ok = t.user("est-ok", "ESTUDIANTE").await;
    let intruder = t.user("est-intruso", "ESTUDIANTE").await;
    t.accept(reto, ok).await;
    let rejected = t.accept(reto, intruder).await;
    t.exec(&format!(
        "UPDATE postulaciones SET estado='RECHAZADA' WHERE id={rejected}"
    ))
    .await;
    reto
}
parity!(test_estudiante_aceptado_puede_subir, |t| {
    let reto = access(&t).await;
    let session = t.login("est-ok").await;
    assert_eq!(upload(&t, &session, reto).await, 200);
    let student = t
        .i64("SELECT id FROM usuarios WHERE username='est-ok'")
        .await;
    assert_eq!(deliverables(&t, reto, student).await, 1);
});
parity!(
    test_estudiante_sin_postulacion_aceptada_no_puede_subir,
    |t| {
        let reto = access(&t).await;
        let session = t.login("est-intruso").await;
        assert_eq!(upload(&t, &session, reto).await, 403);
        let student = t
            .i64("SELECT id FROM usuarios WHERE username='est-intruso'")
            .await;
        assert_eq!(deliverables(&t, reto, student).await, 0);
    }
);
parity!(test_estudiante_de_equipo_academico_puede_subir, |t| {
    let reto = access(&t).await;
    let member = t.user("est-equipo", "ESTUDIANTE").await;
    let team = t.i64(&format!("INSERT INTO equipos_reto_academico(reto_id,nombre_equipo) VALUES({reto},'Equipo A') RETURNING id")).await;
    t.exec(&format!("INSERT INTO equipos_reto_academico_estudiantes(equiporetoacademico_id,usuario_id) VALUES({team},{member})")).await;
    let session = t.login("est-equipo").await;
    assert_eq!(upload(&t, &session, reto).await, 200);
    assert_eq!(deliverables(&t, reto, member).await, 1);
});
parity!(
    test_listado_sin_reto_elegido_muestra_los_retos_participando,
    |t| {
        let reto = access(&t).await;
        let session = t.login("est-ok").await;
        let list = t
            .ok(&session, "GET", "/api/records/retos?limit=100", None)
            .await;
        assert!(
            list["records"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["id"] == reto),
            "{list}"
        );
    }
);

// FavoritosTests
async fn favorite(t: &Ctx) -> (i64, Session) {
    // Django's fixture uses an "aprobado" reto; Rust only lets students favorite
    // retos they can see (from "abierto"), so hidden ids cannot be probed.
    let reto = t.reto(COMPANY_USER, "abierto", "reto").await;
    t.user("est-fav", "ESTUDIANTE").await;
    (reto, t.login("est-fav").await)
}
async fn favorites(t: &Ctx, reto: i64) -> i64 {
    t.i64(&format!("SELECT count(*) FROM retos_favoritos f JOIN usuarios u ON u.id=f.usuario_id WHERE u.username='est-fav' AND f.reto_id={reto}")).await
}
parity!(test_get_no_modifica_favoritos, |t| {
    let (reto, session) = favorite(&t).await;
    assert_eq!(
        t.status(
            &session,
            "GET",
            &format!("/api/retos/{reto}/favorito"),
            None
        )
        .await,
        405
    );
    assert_eq!(favorites(&t, reto).await, 0);
});
parity!(test_post_alterna_favorito, |t| {
    let (reto, session) = favorite(&t).await;
    t.ok(
        &session,
        "POST",
        &format!("/api/retos/{reto}/favorito"),
        Some(json!({})),
    )
    .await;
    assert_eq!(favorites(&t, reto).await, 1);
    t.ok(
        &session,
        "POST",
        &format!("/api/retos/{reto}/favorito"),
        Some(json!({})),
    )
    .await;
    assert_eq!(favorites(&t, reto).await, 0);
});
parity!(test_no_redirige_a_dominio_externo, |t| {
    let (reto, session) = favorite(&t).await;
    // The API never redirects: an external "next" is simply ignored.
    let (code, body, _) = request(
        &t.app,
        &session,
        "POST",
        &format!("/api/retos/{reto}/favorito"),
        Some(json!({"next":"https://evil.example.com/robo"})),
    )
    .await;
    assert_eq!(code, 200, "{body}");
    assert!(!body.to_string().contains("evil.example.com"));
});

// PostulacionProgramaDropdownTests
async fn open(t: &Ctx) -> (i64, Session) {
    let reto = t.reto(COMPANY_USER, "abierto", "reto").await;
    t.user("est-pf", "ESTUDIANTE").await;
    (reto, t.login("est-pf").await)
}
parity!(test_formulario_programa_se_renderiza_como_select, |t| {
    let options = t.ok(&t.anon, "GET", "/api/registro/opciones", None).await;
    let program = options["programas"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["nombre"] == "Ingeniería de Sistemas")
        .unwrap();
    assert_eq!(program["label"], "Ingeniería de Sistemas (Ingeniería)");
});
parity!(test_formulario_rechaza_programa_fuera_de_catalogo, |t| {
    let (reto, session) = open(&t).await;
    let (code, body) = t.req(&session, "POST", &format!("/api/retos/{reto}/postular"), Some(json!({"motivacion":"Quiero participar","habilidades":"Python","semestre":6,"programa":"Programa inventado","acepta_terminos":true}))).await;
    assert_eq!(code, 400, "{body}");
    assert_eq!(
        t.i64(&format!(
            "SELECT count(*) FROM postulaciones WHERE reto_id={reto}"
        ))
        .await,
        0
    );
});
parity!(test_postular_con_programa_de_catalogo_guarda_valor, |t| {
    let (reto, session) = open(&t).await;
    t.ok(&session, "POST", &format!("/api/retos/{reto}/postular"), Some(json!({"motivacion":"Me interesa el reto","habilidades":"Analisis de datos","semestre":7,"programa":"Ingeniería de Sistemas","acepta_terminos":true}))).await;
    assert_eq!(
        t.text(&format!(
            "SELECT programa FROM postulaciones WHERE reto_id={reto}"
        ))
        .await,
        "Ingeniería de Sistemas"
    );
});

// FlujoNotificacionesYRevisionPostulacionTests
async fn apply(t: &Ctx) -> (i64, i64, i64) {
    let (company, _) = t.company_user("empresa-revision", "900555111").await;
    let admin = t.user("admin-revision", "ADMIN").await;
    t.user("est-revision", "ESTUDIANTE").await;
    let reto = t.reto(company, "abierto", "reto").await;
    let session = t.login("est-revision").await;
    t.ok(&session, "POST", &format!("/api/retos/{reto}/postular"), Some(json!({"motivacion":"Quiero participar porque me interesa aplicar mis conocimientos.","habilidades":"Python, analitica y trabajo en equipo","semestre":6,"programa":"Ingeniería de Sistemas","acepta_terminos":true}))).await;
    (reto, company, admin)
}
parity!(test_postulacion_notifica_a_empresa_y_no_a_admin, |t| {
    let (_, company, admin) = apply(&t).await;
    assert_eq!(t.notices(company, "POSTULACION_NUEVA").await, 1);
    assert_eq!(t.notices(admin, "POSTULACION_NUEVA").await, 0);
});
parity!(
    test_empresa_ve_motivacion_habilidades_y_datos_en_tabla,
    |t| {
        apply(&t).await;
        let session = t.login("empresa-revision").await;
        let list = t
            .ok(&session, "GET", "/api/records/postulaciones", None)
            .await
            .to_string();
        for text in [
            "Quiero participar porque me interesa",
            "Python, analitica y trabajo en equipo",
            "Ingeniería de Sistemas",
        ] {
            assert!(list.contains(text), "{text}");
        }
    }
);

// ListaMisEntregablesTests
parity!(test_lista_de_retos_del_estudiante_se_muestra, |t| {
    let (company, _) = t.company_user("empresa-lista", "900555222").await;
    let student = t.user("est-lista", "ESTUDIANTE").await;
    let reto = t.reto(company, "en_curso", "reto").await;
    t.exec(&format!(
        "UPDATE retos SET titulo='Reto con entregables' WHERE id={reto}"
    ))
    .await;
    t.accept(reto, student).await;
    let session = t.login("est-lista").await;
    let list = t
        .ok(&session, "GET", "/api/records/retos?limit=100", None)
        .await
        .to_string();
    assert!(list.contains("Reto con entregables"));
});

// PostulacionCompletaTests
async fn complete(t: &Ctx) -> (i64, i64, i64, Session) {
    let (company, _) = t.company_user("empresa-hu11", "900123123").await;
    let student = t.user("est-hu11", "ESTUDIANTE").await;
    let ecosystem = t.i64("INSERT INTO ecosistemas(nombre,descripcion) VALUES('Innovacion social','') RETURNING id").await;
    let reto = t.reto(company, "abierto", "reto").await;
    t.exec(&format!(
        "UPDATE retos SET titulo='Reto HU11',ecosistema_id={ecosystem} WHERE id={reto}"
    ))
    .await;
    let other = t.reto(company, "abierto", "reto").await;
    t.exec(&format!(
        "UPDATE retos SET titulo='Reto sin ecosistema',ecosistema_id=NULL WHERE id={other}"
    ))
    .await;
    (reto, ecosystem, student, t.login("est-hu11").await)
}
fn form(extra: &[(&'static str, &'static str)]) -> Vec<(&'static str, &'static str)> {
    let mut fields = vec![
        ("motivacion", "Me interesa"),
        ("habilidades", "Python"),
        ("semestre", "5"),
        ("programa", "Ingeniería de Sistemas"),
    ];
    fields.extend_from_slice(extra);
    fields
}
parity!(test_explorador_filtra_por_ecosistema, |t| {
    let (reto, ecosystem, _, session) = complete(&t).await;
    let list = t
        .ok(
            &session,
            "GET",
            &format!("/api/records/retos?ecosistema={ecosystem}"),
            None,
        )
        .await;
    let ids: Vec<i64> = list["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_i64().unwrap())
        .collect();
    assert_eq!(ids, [reto]);
});
parity!(test_sin_aceptar_terminos_no_se_postula, |t| {
    let (reto, _, _, session) = complete(&t).await;
    let (code, _) = multipart(
        &t.app,
        &session,
        &format!("/api/retos/{reto}/postular"),
        &form(&[]),
        &[],
    )
    .await;
    assert_eq!(code, 400);
    assert_eq!(
        t.i64(&format!(
            "SELECT count(*) FROM postulaciones WHERE reto_id={reto}"
        ))
        .await,
        0
    );
});
parity!(
    test_postulacion_con_hoja_de_vida_y_confirmacion_al_estudiante,
    |t| {
        let (reto, _, student, session) = complete(&t).await;
        let (code, body) = multipart(
            &t.app,
            &session,
            &format!("/api/retos/{reto}/postular"),
            &form(&[("acepta_terminos", "on")]),
            &[("hoja_de_vida", "cv.pdf", b"%PDF-1.4")],
        )
        .await;
        assert_eq!(code, 200, "{body}");
        let (terms, cv): (bool, String) = sqlx::query_as(&format!("SELECT acepta_terminos,COALESCE(hoja_de_vida,'') FROM postulaciones WHERE reto_id={reto}")).fetch_one(&t.pool).await.unwrap();
        assert!(terms && !cv.is_empty());
        assert_eq!(t.notices(student, "POSTULACION_RECIBIDA").await, 1);
    }
);
parity!(test_hoja_de_vida_que_no_es_pdf_se_rechaza, |t| {
    let (reto, _, _, session) = complete(&t).await;
    let (code, _) = multipart(
        &t.app,
        &session,
        &format!("/api/retos/{reto}/postular"),
        &form(&[("acepta_terminos", "on")]),
        &[("hoja_de_vida", "cv.exe", b"MZ")],
    )
    .await;
    assert_eq!(code, 400);
    assert_eq!(
        t.i64(&format!(
            "SELECT count(*) FROM postulaciones WHERE reto_id={reto}"
        ))
        .await,
        0
    );
});
parity!(
    test_la_empresa_al_abrir_la_lista_deja_la_postulacion_en_revision,
    |t| {
        let (reto, _, student, _) = complete(&t).await;
        let application = t.i64(&format!("INSERT INTO postulaciones(reto_id,estudiante_id,estado,motivacion,semestre,programa,fecha_postulacion,habilidades,acepta_terminos) VALUES({reto},{student},'PENDIENTE','x',5,'',now(),'',true) RETURNING id")).await;
        let company = t.login("empresa-hu11").await;
        t.ok(&company, "GET", "/api/records/postulaciones", None)
            .await;
        assert_eq!(
            t.text(&format!(
                "SELECT estado FROM postulaciones WHERE id={application}"
            ))
            .await,
            "EN_REVISION"
        );
        t.ok(
            &company,
            "POST",
            &format!("/api/postulaciones/{application}/estado"),
            Some(json!({"estado":"ACEPTADA"})),
        )
        .await;
        assert_eq!(
            t.text(&format!(
                "SELECT estado FROM postulaciones WHERE id={application}"
            ))
            .await,
            "ACEPTADA"
        );
    }
);
parity!(test_el_estudiante_retira_una_postulacion_en_revision, |t| {
    let (reto, _, student, session) = complete(&t).await;
    let application = t.i64(&format!("INSERT INTO postulaciones(reto_id,estudiante_id,estado,motivacion,semestre,programa,fecha_postulacion,habilidades,acepta_terminos) VALUES({reto},{student},'EN_REVISION','x',5,'',now(),'',true) RETURNING id")).await;
    t.ok(
        &session,
        "POST",
        &format!("/api/postulaciones/{application}/estado"),
        Some(json!({"estado":"RETIRADA"})),
    )
    .await;
    assert_eq!(
        t.text(&format!(
            "SELECT estado FROM postulaciones WHERE id={application}"
        ))
        .await,
        "RETIRADA"
    );
});
