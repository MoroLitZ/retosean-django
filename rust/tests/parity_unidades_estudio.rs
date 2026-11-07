//! apps/unidades_estudio/tests.py: CSV bulk load and single creation.
mod common;
use common::*;
use serde_json::{Value, json};

async fn program(t: &Ctx) -> i64 {
    t.i64("INSERT INTO programas(nombre,facultad_id,nivel) VALUES('Ingenieria de Sistemas',1,'') RETURNING id").await
}
async fn load(t: &Ctx, session: &Session, header: &str, rows: &[&str]) -> (u16, Value) {
    let csv = format!("{header}\n{}", rows.join("\n"));
    let (code, body) = multipart(
        &t.app,
        session,
        "/api/unidades",
        &[("accion", "cargar_csv")],
        &[("archivo_csv", "unidades.csv", csv.as_bytes())],
    )
    .await;
    (code.as_u16(), body)
}
const HEADER: &str = "codigo,nombre,programa,periodo,ciclo";
async fn units(t: &Ctx) -> i64 {
    t.i64("SELECT count(*) FROM unidades_estudio").await
}

// CargaMasivaCsvTests
parity!(test_crea_varias_unidades, |t| {
    let program = program(&t).await;
    load(
        &t,
        &t.admin,
        HEADER,
        &[
            "ING-101,Introduccion a la Ingenieria,Ingenieria de Sistemas,2026-2,1",
            "MAT-201,Calculo Diferencial,Ingenieria de Sistemas,2026-2,2",
        ],
    )
    .await;
    assert_eq!(units(&t).await, 2);
    assert_eq!(
        t.i64("SELECT programa_id FROM unidades_estudio WHERE codigo='ING-101'")
            .await,
        program
    );
});
parity!(test_reutiliza_el_programa_sin_distinguir_mayusculas, |t| {
    program(&t).await;
    load(
        &t,
        &t.admin,
        HEADER,
        &["ING-102,Algoritmos,INGENIERIA DE SISTEMAS,2026-2,2"],
    )
    .await;
    assert_eq!(
        t.i64("SELECT count(*) FROM programas WHERE lower(nombre)='ingenieria de sistemas'")
            .await,
        1
    );
    assert_eq!(units(&t).await, 1);
});
parity!(test_omite_programa_inexistente, |t| {
    program(&t).await;
    load(
        &t,
        &t.admin,
        HEADER,
        &["ADM-101,Fundamentos,Administracion de Empresas,2026-2,1"],
    )
    .await;
    assert_eq!(
        t.i64("SELECT count(*) FROM programas WHERE lower(nombre)='administracion de empresas'")
            .await,
        0
    );
    assert_eq!(units(&t).await, 0);
});
parity!(test_omite_codigos_duplicados, |t| {
    program(&t).await;
    load(
        &t,
        &t.admin,
        HEADER,
        &[
            "DUP-100,Una,Ingenieria de Sistemas,2026-2,1",
            "DUP-100,Otra,Ingenieria de Sistemas,2026-2,1",
        ],
    )
    .await;
    assert_eq!(
        t.i64("SELECT count(*) FROM unidades_estudio WHERE codigo='DUP-100'")
            .await,
        1
    );
});
parity!(test_rechaza_csv_sin_columnas_obligatorias, |t| {
    program(&t).await;
    let (code, body) = load(&t, &t.admin, "periodo,ciclo", &["2026-2,1"]).await;
    assert_eq!(units(&t).await, 0);
    assert_eq!(code, 400);
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("columnas obligatorias"),
        "{body}"
    );
});
parity!(test_acepta_punto_y_coma_como_separador, |t| {
    program(&t).await;
    load(
        &t,
        &t.admin,
        "codigo;nombre;programa;periodo;ciclo",
        &["SEP-100;Con punto y coma;Ingenieria de Sistemas;2026-2;1"],
    )
    .await;
    assert_eq!(
        t.i64("SELECT count(*) FROM unidades_estudio WHERE codigo='SEP-100'")
            .await,
        1
    );
});
parity!(test_estudiante_no_puede_cargar, |t| {
    program(&t).await;
    let (code, _) = load(
        &t,
        &t.student,
        HEADER,
        &["NO-100,No,Ingenieria de Sistemas,2026-2,1"],
    )
    .await;
    assert_eq!(code, 403);
    assert_eq!(units(&t).await, 0);
});

// CreacionIndividualTests
parity!(test_crea_una_unidad_con_programa_existente, |t| {
    let program = program(&t).await;
    let id = program.to_string();
    let (code, body) = multipart(
        &t.app,
        &t.admin,
        "/api/unidades",
        &[
            ("codigo", "UNI-100"),
            ("nombre", "Unidad de prueba"),
            ("programa", id.as_str()),
            ("periodo", "2026-2"),
            ("ciclo", "3"),
        ],
        &[],
    )
    .await;
    assert_eq!(code, 200, "{body}");
    assert_eq!(
        t.i64("SELECT programa_id FROM unidades_estudio WHERE codigo='UNI-100'")
            .await,
        program
    );
});

// AutorizacionProfesorTests (CIE-03): un profesor solo gestiona unidades de su facultad.
/// Una unidad propia (facultad 1), una ajena (Negocios) y la facultad ajena.
async fn scope_setup(t: &Ctx) -> (i64, i64, i64) {
    let negocios = t
        .i64("INSERT INTO facultades(nombre,codigo) VALUES('Negocios','NEG') RETURNING id")
        .await;
    let propia = program(t).await;
    let ajeno = t.i64(&format!("INSERT INTO programas(nombre,facultad_id,nivel) VALUES('Administracion de Empresas',{negocios},'') RETURNING id")).await;
    let unidad = t.i64(&format!("INSERT INTO unidades_estudio(codigo,nombre,programa_id,periodo,ciclo,activo,creado_en,actualizado_en) VALUES('UNI-SIS','Unidad propia',{propia},'2026-2','1',true,now(),now()) RETURNING id")).await;
    let ajena = t.i64(&format!("INSERT INTO unidades_estudio(codigo,nombre,programa_id,periodo,ciclo,activo,creado_en,actualizado_en) VALUES('UNI-ADM','Unidad ajena',{ajeno},'2026-2','2',true,now(),now()) RETURNING id")).await;
    (unidad, ajena, negocios)
}
parity!(
    test_profesor_puede_desactivar_unidad_de_su_facultad,
    |t| {
        let (unidad, ..) = scope_setup(&t).await;
        t.user("profe-ing", "PROFESOR").await;
        let session = t.login("profe-ing").await;
        t.ok(
            &session,
            "POST",
            &format!("/api/unidades/{unidad}/activo"),
            Some(json!({"activo":false})),
        )
        .await;
        assert!(
            !t.bool(&format!(
                "SELECT activo FROM unidades_estudio WHERE id={unidad}"
            ))
            .await
        );
    }
);
parity!(
    test_profesor_no_desactiva_unidad_de_otra_facultad,
    |t| {
        let (_, ajena, _) = scope_setup(&t).await;
        t.user("profe-ing", "PROFESOR").await;
        let session = t.login("profe-ing").await;
        assert_eq!(
            t.status(
                &session,
                "POST",
                &format!("/api/unidades/{ajena}/activo"),
                Some(json!({"activo":false}))
            )
            .await,
            403
        );
        assert!(
            t.bool(&format!(
                "SELECT activo FROM unidades_estudio WHERE id={ajena}"
            ))
            .await
        );
    }
);
parity!(test_profesor_no_crea_unidad_en_otra_facultad, |t| {
    let (_, _, negocios) = scope_setup(&t).await;
    let ajeno = t
        .i64(&format!(
            "SELECT id FROM programas WHERE facultad_id={negocios}"
        ))
        .await
        .to_string();
    t.user("profe-ing", "PROFESOR").await;
    let session = t.login("profe-ing").await;
    let (code, body) = multipart(
        &t.app,
        &session,
        "/api/unidades",
        &[
            ("codigo", "UNI-NUEVA"),
            ("nombre", "Unidad nueva"),
            ("programa", ajeno.as_str()),
            ("periodo", "2026-2"),
            ("ciclo", "1"),
        ],
        &[],
    )
    .await;
    assert_eq!(code, 400, "{body}");
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("No tienes permiso para crear unidades"),
        "{body}"
    );
    assert_eq!(
        t.i64("SELECT count(*) FROM unidades_estudio WHERE codigo='UNI-NUEVA'")
            .await,
        0
    );
});
parity!(
    test_profesor_no_editar_unidad_de_otra_facultad,
    |t| {
        let (_, ajena, negocios) = scope_setup(&t).await;
        let ajeno = t
            .i64(&format!(
                "SELECT id FROM programas WHERE facultad_id={negocios}"
            ))
            .await
            .to_string();
        t.user("profe-ing", "PROFESOR").await;
        let session = t.login("profe-ing").await;
        let (code, _) = multipart(
            &t.app,
            &session,
            &format!("/api/unidades/{ajena}"),
            &[
                ("codigo", "UNI-ADM-MOD"),
                ("nombre", "Unidad modificada"),
                ("programa", ajeno.as_str()),
                ("periodo", "2026-2"),
                ("ciclo", "2"),
            ],
            &[],
        )
        .await;
        assert_eq!(code, 403);
        assert_eq!(
            t.text(&format!(
                "SELECT codigo FROM unidades_estudio WHERE id={ajena}"
            ))
            .await,
            "UNI-ADM"
        );
    }
);
parity!(test_profesor_sin_facultad_no_gestiona, |t| {
    let (unidad, ..) = scope_setup(&t).await;
    let profe = t.user("profe-sf", "PROFESOR").await;
    t.exec(&format!(
        "UPDATE profesores SET facultad_id=NULL WHERE usuario_id={profe}"
    ))
    .await;
    let session = t.login("profe-sf").await;
    let propia = t
        .i64(&format!(
            "SELECT programa_id FROM unidades_estudio WHERE id={unidad}"
        ))
        .await
        .to_string();
    let (code, _) = multipart(
        &t.app,
        &session,
        "/api/unidades",
        &[
            ("codigo", "UNI-SF"),
            ("nombre", "Sin facultad"),
            ("programa", propia.as_str()),
            ("periodo", "2026-2"),
            ("ciclo", "1"),
        ],
        &[],
    )
    .await;
    assert_eq!(code, 403);
    assert_eq!(
        t.i64("SELECT count(*) FROM unidades_estudio WHERE codigo='UNI-SF'")
            .await,
        0
    );
    assert_eq!(
        t.status(
            &session,
            "POST",
            &format!("/api/unidades/{unidad}/activo"),
            Some(json!({"activo":false}))
        )
        .await,
        403
    );
    assert!(
        t.bool(&format!(
            "SELECT activo FROM unidades_estudio WHERE id={unidad}"
        ))
        .await
    );
});
parity!(test_admin_puede_gestionar_cualquier_unidad, |t| {
    let (_, ajena, _) = scope_setup(&t).await;
    t.ok(
        &t.admin,
        "POST",
        &format!("/api/unidades/{ajena}/activo"),
        Some(json!({"activo":false})),
    )
    .await;
    assert!(
        !t.bool(&format!(
            "SELECT activo FROM unidades_estudio WHERE id={ajena}"
        ))
        .await
    );
});

// CsvTransaccionalTests (CIE-09): resolucion de programas sin ambigüedad.
/// Dos programas con el mismo nombre en facultades distintas; devuelve la ajena.
async fn ambiguous(t: &Ctx) -> i64 {
    let negocios = t
        .i64("INSERT INTO facultades(nombre,codigo) VALUES('Negocios','NEG') RETURNING id")
        .await;
    t.exec("INSERT INTO programas(nombre,facultad_id,nivel) VALUES('Sistemas de Informacion',1,'')")
        .await;
    t.exec(&format!("INSERT INTO programas(nombre,facultad_id,nivel) VALUES('Sistemas de Informacion',{negocios},'')")).await;
    negocios
}
parity!(test_csv_rechaza_programa_ambiguo, |t| {
    ambiguous(&t).await;
    let (code, body) = load(
        &t,
        &t.admin,
        HEADER,
        &["AMB-100,Unidad,Sistemas de Informacion,2026-2,1"],
    )
    .await;
    assert_eq!(code, 200, "{body}");
    assert_eq!(units(&t).await, 0);
    assert!(
        body["mensaje"]
            .as_str()
            .unwrap()
            .contains("existe en varias facultades"),
        "{body}"
    );
});
parity!(
    test_csv_resuelve_programa_ambiguo_con_columna_facultad,
    |t| {
        let negocios = ambiguous(&t).await;
        let (code, body) = load(
            &t,
            &t.admin,
            "codigo,nombre,programa,facultad,periodo,ciclo",
            &["AMB-100,Unidad,Sistemas de Informacion,Negocios,2026-2,1"],
        )
        .await;
        assert_eq!(code, 200, "{body}");
        assert_eq!(
            t.i64(&format!("SELECT count(*) FROM unidades_estudio u JOIN programas p ON p.id=u.programa_id WHERE u.codigo='AMB-100' AND p.facultad_id={negocios}")).await,
            1
        );
    }
);
