//! apps/usuarios/tests.py: routing, admin by role, bulk import, activity log,
//! module permissions and welcome email, company registration, login by email.
mod common;
use common::*;
use serde_json::{Value, json};

// RoutingSmokeTests
parity!(test_login_page_loads, |t| {
    let (code, page) = download(&t.app, &t.anon, "/").await;
    assert_eq!(code, 200);
    assert!(String::from_utf8_lossy(&page).contains("Ingresar al Sistema"));
});
parity!(test_root_redirects_to_profile_workflow, |t| {
    // The single-page app serves "/" and routes by session: without one, /api/me is 401.
    assert_eq!(download(&t.app, &t.anon, "/").await.0, 200);
    assert_eq!(t.status(&t.anon, "GET", "/api/me", None).await, 401);
});

// RolAdminTests
async fn by_role(t: &Ctx) -> Session {
    t.user("admin-rol", "ADMIN").await;
    t.login("admin-rol").await
}
parity!(test_admin_por_rol_accede_al_panel, |t| {
    let admin = by_role(&t).await;
    let panel = t.ok(&admin, "GET", "/api/dashboard", None).await;
    assert!(panel["kpis"].get("retos_total").is_some(), "{panel}");
});
parity!(test_superusuario_accede_al_panel, |t| {
    t.ok(&t.admin, "GET", "/api/dashboard", None).await;
});
parity!(test_admin_por_rol_ve_el_menu_de_administracion, |t| {
    let admin = by_role(&t).await;
    // The admin menu lists users: the catalog exposes "usuarios" to admins.
    let catalog = t
        .ok(&admin, "GET", "/api/resources", None)
        .await
        .to_string();
    assert!(catalog.contains("usuarios"));
});
parity!(test_admin_por_rol_gestiona_usuarios_y_empresas, |t| {
    let admin = by_role(&t).await;
    let users = t.ok(&admin, "GET", "/api/records/usuarios", None).await;
    assert!(users["total"].as_i64().unwrap() > 1);
    t.ok(&admin, "GET", "/api/records/empresas", None).await;
});
parity!(test_estudiante_no_accede_al_panel_de_admin, |t| {
    assert_eq!(
        t.status(&t.student, "GET", "/api/records/log_actividad", None)
            .await,
        403
    );
    assert_eq!(
        t.status(&t.student, "GET", "/api/log-actividad", None)
            .await,
        403
    );
});

// ImportacionMasivaTests
const HEADER: &str = "username,email,first_name,last_name,rol,telefono,password";
async fn import(
    t: &Ctx,
    session: &Session,
    rows: &[&str],
    header: &str,
    confirm: bool,
) -> (u16, Value) {
    let csv = format!("{header}\n{}", rows.join("\n"));
    let action = if confirm {
        "confirmar"
    } else {
        "previsualizar"
    };
    let (code, body) = multipart(
        &t.app,
        session,
        "/api/usuarios/importar",
        &[("accion", action)],
        &[("archivo", "usuarios.csv", csv.as_bytes())],
    )
    .await;
    (code.as_u16(), body)
}
async fn exists(t: &Ctx, username: &str) -> bool {
    t.bool(&format!(
        "SELECT EXISTS(SELECT 1 FROM usuarios WHERE username='{username}')"
    ))
    .await
}
fn count(body: &Value, key: &str) -> usize {
    body[key].as_array().map(Vec::len).unwrap_or(0)
}
parity!(test_previsualizacion_no_crea_usuarios, |t| {
    let (code, body) = import(
        &t,
        &t.admin,
        &["nuevo1,nuevo1@ean.edu.co,Ana,Diaz,ESTUDIANTE,,"],
        HEADER,
        false,
    )
    .await;
    assert_eq!(code, 200);
    assert!(!exists(&t, "nuevo1").await);
    assert_eq!(count(&body, "validas"), 1);
});
parity!(test_confirmar_crea_los_usuarios, |t| {
    import(
        &t,
        &t.admin,
        &[
            "nuevo2,nuevo2@ean.edu.co,Luis,Mora,ESTUDIANTE,,",
            "nuevo3,nuevo3@ean.edu.co,Sara,Ruiz,PROFESOR,,",
        ],
        HEADER,
        true,
    )
    .await;
    assert!(
        t.bool(
            "SELECT EXISTS(SELECT 1 FROM usuarios WHERE username='nuevo2' AND rol='ESTUDIANTE')"
        )
        .await
    );
    assert!(
        t.bool("SELECT EXISTS(SELECT 1 FROM usuarios WHERE username='nuevo3' AND rol='PROFESOR')")
            .await
    );
});
parity!(test_detecta_rol_invalido, |t| {
    let (_, body) = import(
        &t,
        &t.admin,
        &["malrol,malrol@ean.edu.co,X,Y,DIRECTOR,,"],
        HEADER,
        false,
    )
    .await;
    assert_eq!(count(&body, "invalidas"), 1);
    assert!(
        body["invalidas"][0]["errores"]
            .to_string()
            .contains("Rol no valido"),
        "{body}"
    );
});
parity!(test_detecta_correo_invalido_y_usuario_repetido, |t| {
    t.user("existente", "ESTUDIANTE").await;
    let (_, body) = import(
        &t,
        &t.admin,
        &[
            "existente,otro@ean.edu.co,A,B,ESTUDIANTE,,",
            "nuevo4,correo-malo,C,D,ESTUDIANTE,,",
        ],
        HEADER,
        false,
    )
    .await;
    assert_eq!(count(&body, "invalidas"), 2);
});
parity!(test_detecta_duplicados_dentro_del_archivo, |t| {
    let (_, body) = import(
        &t,
        &t.admin,
        &[
            "repetido,repetido@ean.edu.co,A,B,ESTUDIANTE,,",
            "repetido,otro@ean.edu.co,C,D,ESTUDIANTE,,",
        ],
        HEADER,
        false,
    )
    .await;
    assert_eq!(count(&body, "invalidas"), 1);
});
parity!(test_rechaza_archivo_sin_columnas_obligatorias, |t| {
    let (_, body) = import(&t, &t.admin, &["Ana,Diaz"], "first_name,last_name", false).await;
    assert!(
        body["error_global"]
            .as_str()
            .unwrap_or_default()
            .contains("columnas obligatorias"),
        "{body}"
    );
});
parity!(test_solo_se_crean_las_filas_validas, |t| {
    import(
        &t,
        &t.admin,
        &[
            "bueno,bueno@ean.edu.co,A,B,ESTUDIANTE,,",
            "malo,malo@ean.edu.co,C,D,ROL_INEXISTENTE,,",
        ],
        HEADER,
        true,
    )
    .await;
    assert!(exists(&t, "bueno").await);
    assert!(!exists(&t, "malo").await);
});
parity!(test_registra_la_importacion_en_la_bitacora, |t| {
    import(
        &t,
        &t.admin,
        &["log1,log1@ean.edu.co,A,B,ESTUDIANTE,,"],
        HEADER,
        true,
    )
    .await;
    assert!(
        t.bool("SELECT EXISTS(SELECT 1 FROM log_actividad WHERE accion='IMPORTACION')")
            .await
    );
});
parity!(test_estudiante_no_accede_a_la_importacion, |t| {
    let (code, _) = import(
        &t,
        &t.student,
        &["x,x@ean.edu.co,A,B,ESTUDIANTE,,"],
        HEADER,
        false,
    )
    .await;
    assert_eq!(code, 403);
});

// LogActividadTests
parity!(test_el_login_queda_registrado, |t| {
    t.user("log-user", "ESTUDIANTE").await;
    t.login("log-user").await;
    assert!(t.bool("SELECT EXISTS(SELECT 1 FROM log_actividad WHERE accion='LOGIN' AND identificador='log-user')").await);
});
parity!(test_el_login_fallido_queda_registrado, |t| {
    let (code, _) = login_with(&t.app, "inexistente", "mala").await;
    assert_eq!(code, 401);
    assert!(t.bool("SELECT EXISTS(SELECT 1 FROM log_actividad WHERE accion='LOGIN_FALLIDO' AND identificador='inexistente')").await);
});
parity!(test_cambiar_rol_queda_registrado, |t| {
    let target = t.user("objetivo", "ESTUDIANTE").await;
    t.ok(
        &t.admin,
        "POST",
        &format!("/api/usuarios/{target}/rol"),
        Some(json!({"rol":"PROFESOR"})),
    )
    .await;
    assert_eq!(
        t.text(&format!("SELECT rol FROM usuarios WHERE id={target}"))
            .await,
        "PROFESOR"
    );
    assert!(t.bool(&format!("SELECT EXISTS(SELECT 1 FROM log_actividad WHERE accion='CAMBIO_ROL' AND usuario_id={target})")).await);
});
parity!(test_no_puedo_desactivar_mi_propia_cuenta, |t| {
    assert_ne!(
        t.status(
            &t.admin,
            "POST",
            &format!("/api/usuarios/{ADMIN}/estado"),
            Some(json!({"activo":false}))
        )
        .await,
        200
    );
    assert!(
        t.bool(&format!("SELECT is_active FROM usuarios WHERE id={ADMIN}"))
            .await
    );
});

// PermisosYBienvenidaTests
async fn leader(t: &Ctx) -> (i64, Session) {
    t.user("admin-perm", "ADMIN").await;
    let leader = t.user("lider-perm", "ADMIN").await;
    (leader, t.login("admin-perm").await)
}
async fn has_informa(t: &Ctx, user: i64) -> bool {
    t.bool(&format!("SELECT EXISTS(SELECT 1 FROM usuarios_user_permissions up JOIN auth_permission p ON p.id=up.permission_id WHERE up.usuario_id={user} AND p.codename='cargar_listas_informa')")).await
}
parity!(
    test_admin_otorga_y_retira_el_permiso_de_listas_informa,
    |t| {
        let (leader, admin) = leader(&t).await;
        let url = format!("/api/usuarios/{leader}/permisos");
        t.ok(&admin, "GET", &url, None).await;
        t.ok(
            &admin,
            "POST",
            &url,
            Some(json!({"codigos":["empresas.cargar_listas_informa"]})),
        )
        .await;
        assert!(has_informa(&t, leader).await);
        t.ok(&admin, "POST", &url, Some(json!({}))).await;
        assert!(!has_informa(&t, leader).await);
        assert_eq!(t.i64(&format!("SELECT count(*) FROM log_actividad WHERE usuario_id={leader} AND accion='CAMBIO_PERMISOS'")).await, 2);
    }
);
parity!(test_permisos_desconocidos_se_ignoran, |t| {
    let (leader, admin) = leader(&t).await;
    t.ok(
        &admin,
        "POST",
        &format!("/api/usuarios/{leader}/permisos"),
        Some(json!({"codigos":["auth.delete_user"]})),
    )
    .await;
    assert_eq!(
        t.i64(&format!(
            "SELECT count(*) FROM usuarios_user_permissions WHERE usuario_id={leader}"
        ))
        .await,
        0
    );
});
parity!(test_solo_el_admin_gestiona_permisos, |t| {
    let (leader, _) = leader(&t).await;
    assert_ne!(
        t.status(
            &t.student,
            "POST",
            &format!("/api/usuarios/{leader}/permisos"),
            Some(json!({"codigos":["empresas.cargar_listas_informa"]}))
        )
        .await,
        200
    );
    assert!(!has_informa(&t, leader).await);
});
parity!(
    test_correo_de_bienvenida_con_enlace_para_definir_contrasena,
    |t| {
        // An imported account without a password gets the welcome mail with the set-password link.
        import(
            &t,
            &t.admin,
            &["nuevo-import,nuevo-bienvenida@ean.edu.co,Nuevo,Import,ESTUDIANTE,,"],
            HEADER,
            true,
        )
        .await;
        let mut mails = Vec::new();
        for _ in 0..50 {
            mails = mails_to("nuevo-bienvenida@ean.edu.co");
            if !mails.is_empty() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        assert_eq!(mails.len(), 1);
        assert!(mails[0].1.contains("Bienvenido a RetosEAN"));
        assert!(mails[0].2.contains("#restablecer/"), "{}", mails[0].2);
    }
);
parity!(test_bitacora_filtra_por_usuario_y_fecha, |t| {
    t.exec("INSERT INTO log_actividad(identificador,accion,detalle,creado_en) VALUES('buscado','LOGIN','',now()),('otro','LOGIN','',now())").await;
    let (_, admin) = leader(&t).await;
    let found = t
        .ok(&admin, "GET", "/api/log-actividad?usuario=buscado", None)
        .await;
    let ids: std::collections::BTreeSet<String> = found["registros"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["identificador"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(ids, ["buscado".to_string()].into());
    let old = t
        .ok(
            &admin,
            "GET",
            "/api/log-actividad?desde=2000-01-01&hasta=2000-01-02",
            None,
        )
        .await;
    assert_eq!(old["registros"].as_array().unwrap().len(), 0);
});

// RegistroEmpresaHU01Tests
fn company(extra: Value) -> Value {
    let mut data = json!({"tipo":"empresa","nit":"901222333","razon_social":"Soluciones Andinas","sector_industrial":"Tecnologia","tipo_convocatoria":"AMBOS","area_responsable":"Innovacion","sitio_web":"https://andinas.co","username":"andinas","email":"contacto@andinas.co","password":"Clave-Segura-2026","password2":"Clave-Segura-2026"});
    for (key, value) in extra.as_object().unwrap() {
        data[key] = value.clone();
    }
    data
}
parity!(test_registro_guarda_los_datos_de_la_organizacion, |t| {
    t.ok(&t.anon, "POST", "/api/registro", Some(company(json!({}))))
        .await;
    let (kind, area, web): (String, String, String) = sqlx::query_as(
        "SELECT tipo_convocatoria,area_responsable,sitio_web FROM empresas WHERE nit='901222333'",
    )
    .fetch_one(&t.pool)
    .await
    .unwrap();
    assert_eq!(
        (kind.as_str(), area.as_str(), web.as_str()),
        ("AMBOS", "Innovacion", "https://andinas.co")
    );
});
parity!(test_sitio_web_invalido_se_rechaza, |t| {
    assert_eq!(
        t.status(
            &t.anon,
            "POST",
            "/api/registro",
            Some(company(json!({"sitio_web":"no es una url"})))
        )
        .await,
        400
    );
    assert_eq!(
        t.i64("SELECT count(*) FROM empresas WHERE nit='901222333'")
            .await,
        0
    );
});
parity!(test_la_empresa_edita_sus_datos_despues, |t| {
    t.ok(&t.anon, "POST", "/api/registro", Some(company(json!({}))))
        .await;
    let (code, session) = login_with(&t.app, "andinas", "Clave-Segura-2026").await;
    assert_eq!(code, 200);
    t.ok(&session, "POST", "/api/perfil", Some(json!({"first_name":"Soluciones Andinas","last_name":"","email":"contacto@andinas.co","telefono":"","razon_social":"Soluciones Andinas","sector_industrial":"Tecnologia","tipo_convocatoria":"RETO","area_responsable":"Transformacion digital","sitio_web":""}))).await;
    let (area, kind): (String, String) = sqlx::query_as(
        "SELECT area_responsable,tipo_convocatoria FROM empresas WHERE nit='901222333'",
    )
    .fetch_one(&t.pool)
    .await
    .unwrap();
    assert_eq!(
        (area.as_str(), kind.as_str()),
        ("Transformacion digital", "RETO")
    );
});

// LoginConCorreoTests
async fn ana(t: &Ctx) {
    let id = t.user("ana", "ESTUDIANTE").await;
    t.exec(&format!(
        "UPDATE usuarios SET email='Ana@Example.com' WHERE id={id}"
    ))
    .await;
}
parity!(test_entra_con_el_usuario, |t| {
    ana(&t).await;
    assert_eq!(login_with(&t.app, "ana", "RetosEAN2026").await.0, 200);
});
parity!(test_entra_con_el_correo_sin_importar_mayusculas, |t| {
    ana(&t).await;
    assert_eq!(
        login_with(&t.app, "ana@example.com", "RetosEAN2026")
            .await
            .0,
        200
    );
});
parity!(test_correo_compartido_no_permite_entrar, |t| {
    ana(&t).await;
    let other = t.user("ana2", "ESTUDIANTE").await;
    t.exec(&format!(
        "UPDATE usuarios SET email='ana@example.com' WHERE id={other}"
    ))
    .await;
    assert_eq!(
        login_with(&t.app, "ana@example.com", "RetosEAN2026")
            .await
            .0,
        401
    );
});
parity!(test_correo_con_clave_incorrecta_falla, |t| {
    ana(&t).await;
    assert_eq!(login_with(&t.app, "ana@example.com", "otra").await.0, 401);
});
