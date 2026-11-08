//! QA package S: cross-table integrity (N20, N51, N58, N59, N60), expense soft
//! delete (N98) and exact money in the XLSX export (N96). Each test proves a
//! trigger/behaviour from rust/sql/schema.sql (mirrored by migration 0005).
mod common;
use calamine::{Data, Reader};
use common::*;
use serde_json::json;

/// Executes a statement and reports whether the database rejected it.
async fn rejects(t: &Ctx, sql: &str) -> bool {
    sqlx::raw_sql(sql).execute(&t.pool).await.is_err()
}

/// A hackathon reto plus its hackathon row; returns (reto_id, hackathon_id).
async fn hackathon(t: &Ctx, owner: i64) -> (i64, i64) {
    let reto = t.reto(owner, "en_curso", "hackathon").await;
    let hack = t
        .i64(&format!(
            "INSERT INTO hackathons(reto_id,reglas,estado,resultados_publicados,creado_en,actualizado_en) VALUES({reto},'','borrador',false,now(),now()) RETURNING id"
        ))
        .await;
    (reto, hack)
}

// N20: a vote's juror and team must belong to the same hackathon/reto.
parity!(qa_voto_jurado_y_equipo_del_mismo_hackathon, |t| {
    let (reto_a, hack_a) = hackathon(&t, COMPANY_USER).await;
    let (reto_b, hack_b) = hackathon(&t, COMPANY_USER).await;
    let expert = t.user("qa-exp-1", "EXPERTO").await;
    let juror_a = t
        .i64(&format!(
            "INSERT INTO jurados_hackaton(hackathon_id,usuario_id,nombre,email,especialidad) VALUES({hack_a},{expert},'Jurado','j@x.co','') RETURNING id"
        ))
        .await;
    let team_a = t
        .i64(&format!(
            "INSERT INTO equipos(reto_id,nombre,creado_en) VALUES({reto_a},'qa-team-a',now()) RETURNING id"
        ))
        .await;
    let team_b = t
        .i64(&format!(
            "INSERT INTO equipos(reto_id,nombre,creado_en) VALUES({reto_b},'qa-team-b',now()) RETURNING id"
        ))
        .await;
    assert!(
        rejects(
            &t,
            &format!(
                "INSERT INTO votaciones_hackaton(hackathon_id,jurado_id,equipo_id,puntaje,comentario,fecha) VALUES({hack_b},{juror_a},{team_b},5,'',now())"
            )
        )
        .await,
        "a juror voting on another hackathon must be rejected"
    );
    assert!(
        rejects(
            &t,
            &format!(
                "INSERT INTO votaciones_hackaton(hackathon_id,jurado_id,equipo_id,puntaje,comentario,fecha) VALUES({hack_a},{juror_a},{team_b},5,'',now())"
            )
        )
        .await,
        "a team of another reto must be rejected"
    );
    let id = t
        .i64(&format!(
            "INSERT INTO votaciones_hackaton(hackathon_id,jurado_id,equipo_id,puntaje,comentario,fecha) VALUES({hack_a},{juror_a},{team_a},5,'',now()) RETURNING id"
        ))
        .await;
    assert!(id > 0);
});

// N51: a per-criterion score cannot exceed its criterion's puntaje_maximo.
parity!(qa_evaluacioncriterio_puntaje_maximo, |t| {
    let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
    let student = t.user("qa-est-1", "ESTUDIANTE").await;
    let teacher = t.user("qa-prof-1", "PROFESOR").await;
    let deliverable = t.deliverable(reto, student).await;
    let rubric = t
        .i64(&format!(
            "INSERT INTO evaluacion_rubrica(reto_id,nombre,descripcion,activa,creada_por_id,creada_en) VALUES({reto},'qa','',true,{teacher},now()) RETURNING id"
        ))
        .await;
    let criterio = t
        .i64(&format!(
            "INSERT INTO evaluacion_criteriorubrica(rubrica_id,nombre,descripcion,puntaje_maximo,orden) VALUES({rubric},'criterio','',5,0) RETURNING id"
        ))
        .await;
    let evaluation = t
        .i64(&format!(
            "INSERT INTO evaluaciones(entregable_id,profesor_id,nota,comentario,rubrica_id,fecha,actualizado_en) VALUES({deliverable},{teacher},4,'',{rubric},now(),now()) RETURNING id"
        ))
        .await;
    assert!(
        rejects(
            &t,
            &format!(
                "INSERT INTO evaluacion_evaluacioncriterio(evaluacion_id,criterio_id,puntaje) VALUES({evaluation},{criterio},6)"
            )
        )
        .await,
        "a score above the criterion's maximum must be rejected"
    );
    t.exec(&format!(
        "INSERT INTO evaluacion_evaluacioncriterio(evaluacion_id,criterio_id,puntaje) VALUES({evaluation},{criterio},5)"
    ))
    .await;
});

// N58: a hackathon registration's team must belong to the hackathon's reto.
parity!(qa_inscripcion_equipo_del_reto, |t| {
    let (reto_a, hack_a) = hackathon(&t, COMPANY_USER).await;
    let (reto_b, _) = hackathon(&t, COMPANY_USER).await;
    let team_a = t
        .i64(&format!(
            "INSERT INTO equipos(reto_id,nombre,creado_en) VALUES({reto_a},'qa-team-a',now()) RETURNING id"
        ))
        .await;
    let team_b = t
        .i64(&format!(
            "INSERT INTO equipos(reto_id,nombre,creado_en) VALUES({reto_b},'qa-team-b',now()) RETURNING id"
        ))
        .await;
    assert!(
        rejects(
            &t,
            &format!(
                "INSERT INTO inscripciones_hackaton(hackathon_id,equipo_id,estado,creado_en) VALUES({hack_a},{team_b},'PENDIENTE',now())"
            )
        )
        .await,
        "a team of another reto must be rejected"
    );
    let id = t
        .i64(&format!(
            "INSERT INTO inscripciones_hackaton(hackathon_id,equipo_id,estado,creado_en) VALUES({hack_a},{team_a},'PENDIENTE',now()) RETURNING id"
        ))
        .await;
    assert!(id > 0);
});

// N59: a deliverable's reto must equal its team's reto when a team is set.
parity!(qa_entregable_equipo_del_reto, |t| {
    let reto_a = t.reto(COMPANY_USER, "en_curso", "reto").await;
    let reto_b = t.reto(COMPANY_USER, "en_curso", "reto").await;
    let student = t.user("qa-est-2", "ESTUDIANTE").await;
    let deliverable = t.deliverable(reto_a, student).await;
    let team_b = t
        .i64(&format!(
            "INSERT INTO equipos(reto_id,nombre,creado_en) VALUES({reto_b},'qa-team-b',now()) RETURNING id"
        ))
        .await;
    assert!(
        rejects(
            &t,
            &format!("UPDATE entregables SET equipo_id={team_b} WHERE id={deliverable}")
        )
        .await,
        "a deliverable tied to another reto's team must be rejected"
    );
    let team_a = t
        .i64(&format!(
            "INSERT INTO equipos(reto_id,nombre,creado_en) VALUES({reto_a},'qa-team-a',now()) RETURNING id"
        ))
        .await;
    t.exec(&format!(
        "UPDATE entregables SET equipo_id={team_a} WHERE id={deliverable}"
    ))
    .await;
});

// N60: role enforcement on student/professor profile columns.
parity!(qa_roles_de_estudiante_y_profesor_exigidos, |t| {
    let est = t.user("qa-est-r", "ESTUDIANTE").await;
    let prof = t.user("qa-prof-r", "PROFESOR").await;
    assert!(
        rejects(
            &t,
            &format!("INSERT INTO profesores(usuario_id,especialidad) VALUES({est},'')")
        )
        .await,
        "an ESTUDIANTE must not become a profesor profile"
    );
    assert!(
        rejects(
            &t,
            &format!("INSERT INTO estudiantes(usuario_id,semestre) VALUES({prof},5)")
        )
        .await,
        "a PROFESOR must not become a estudiante profile"
    );
    let reto = t.reto(COMPANY_USER, "abierto", "reto").await;
    assert!(
        rejects(
            &t,
            &format!("INSERT INTO postulaciones(reto_id,estudiante_id,estado,motivacion,semestre,programa,fecha_postulacion,habilidades,acepta_terminos) VALUES({reto},{prof},'PENDIENTE','',5,'',now(),'',true)")
        )
        .await,
        "a non-student must not postular"
    );
    let team = t
        .i64(&format!(
            "INSERT INTO equipos(reto_id,nombre,creado_en) VALUES({reto},'qa-team',now()) RETURNING id"
        ))
        .await;
    assert!(
        rejects(
            &t,
            &format!("INSERT INTO miembros_equipo(equipo_id,estudiante_id,rol,fecha_ingreso) VALUES({team},{prof},'MIEMBRO',now())")
        )
        .await,
        "a non-student must not join a team"
    );
});

// N98: expenses are soft-deleted, audited, and excluded from every read.
parity!(qa_gasto_borrado_logico_y_auditado, |t| {
    let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
    let budget = t
        .i64(&format!(
            "INSERT INTO presupuestos(reto_id,monto_total,alerta_80_enviada,creado_en,actualizado_en) VALUES({reto},1000,false,now(),now()) RETURNING id"
        ))
        .await;
    let gasto = t
        .i64(&format!(
            "INSERT INTO gastos(presupuesto_id,categoria,monto,estado,descripcion,creado_en) VALUES({budget},'OTROS',250.00,'EJECUTADO','',now()) RETURNING id"
        ))
        .await;
    let body = t
        .ok(
            &t.admin,
            "POST",
            &format!("/api/gastos/{gasto}/eliminar"),
            Some(json!({})),
        )
        .await;
    assert_eq!(body["ok"], true);
    assert!(
        t.bool(&format!(
            "SELECT eliminado_en IS NOT NULL FROM gastos WHERE id={gasto}"
        ))
        .await,
        "the expense stays with a delete timestamp"
    );
    let s = t
        .ok(
            &t.admin,
            "GET",
            &format!("/api/presupuestos/{budget}/resumen"),
            None,
        )
        .await;
    assert_eq!(s["ejecutado"], "0.00", "a deleted expense no longer counts");
    assert_eq!(s["gastos"], json!([]));
    assert_eq!(
        t.i64(&format!(
            "SELECT count(*) FROM historial_estados WHERE object_id={gasto} AND estado_nuevo='eliminado'"
        ))
        .await,
        1,
        "the deletion is recorded in the audit trail"
    );
});

// N96: money stays numeric in the Excel with two decimals.
parity!(qa_excel_montos_numericos_con_centavos, |t| {
    let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
    let budget = t
        .i64(&format!(
            "INSERT INTO presupuestos(reto_id,monto_total,alerta_80_enviada,creado_en,actualizado_en) VALUES({reto},1234.56,false,now(),now()) RETURNING id"
        ))
        .await;
    t.exec(&format!(
        "INSERT INTO gastos(presupuesto_id,categoria,monto,estado,descripcion,creado_en) VALUES({budget},'MATERIALES',1234.56,'EJECUTADO','',now())"
    ))
    .await;
    let (code, bytes) = download(
        &t.app,
        &t.admin,
        &format!("/api/retos/{reto}/presupuesto/xlsx"),
    )
    .await;
    assert_eq!(code, 200);
    let mut book: calamine::Xlsx<_> =
        calamine::open_workbook_from_rs(std::io::Cursor::new(bytes)).unwrap();
    let values = book.worksheet_range("Gastos").unwrap();
    let cells: Vec<Data> = values.cells().map(|(_, _, v)| v.clone()).collect();
    assert!(
        cells.contains(&Data::Float(1234.56)),
        "money stays a numeric cell with its cents"
    );
});

// N60: a later role change must not freeze the user's existing rows.
parity!(qa_cambio_de_rol_no_bloquea_filas_existentes, |t| {
    let est = t.user("qa-est-cambio", "ESTUDIANTE").await;
    let reto = t.reto(COMPANY_USER, "abierto", "reto").await;
    let postulacion = t.i64(&format!("INSERT INTO postulaciones(reto_id,estudiante_id,estado,motivacion,semestre,programa,fecha_postulacion,habilidades,acepta_terminos) VALUES({reto},{est},'PENDIENTE','',5,'',now(),'',true) RETURNING id")).await;
    t.exec(&format!(
        "UPDATE usuarios SET rol='PROFESOR' WHERE id={est}"
    ))
    .await;
    t.exec(&format!(
        "UPDATE postulaciones SET estado='RECHAZADA' WHERE id={postulacion}"
    ))
    .await;
    assert_eq!(
        t.text(&format!(
            "SELECT estado FROM postulaciones WHERE id={postulacion}"
        ))
        .await,
        "RECHAZADA"
    );
});
