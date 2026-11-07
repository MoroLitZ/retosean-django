//! QA package F: database schema integrity (constraints, unique indexes, FK,
//! state checks and the migration bootstrap lock). Each test proves one rule
//! enforced by rust/sql/schema.sql is really rejected/kept by PostgreSQL.
mod common;
use common::*;

/// Executes a statement and reports whether the database rejected it.
async fn rejects(t: &Ctx, sql: &str) -> bool {
    sqlx::raw_sql(sql).execute(&t.pool).await.is_err()
}

/// A hackathon reto with a jury; returns (hackathon_id, equipo_id, jurado_id).
async fn hackathon_jury(t: &Ctx, owner: i64) -> (i64, i64, i64) {
    let reto = t.reto(owner, "en_curso", "hackathon").await;
    let hack = t
        .i64(&format!(
            "INSERT INTO hackathons(reto_id,reglas,estado,resultados_publicados,creado_en,actualizado_en) VALUES({reto},'','borrador',false,now(),now()) RETURNING id"
        ))
        .await;
    let team = t
        .i64(&format!(
            "INSERT INTO equipos(reto_id,nombre,creado_en) VALUES({reto},'qa-equipo',now()) RETURNING id"
        ))
        .await;
    let expert = t.user("qa-experto", "EXPERTO").await;
    let juror = t
        .i64(&format!(
            "INSERT INTO jurados_hackaton(hackathon_id,usuario_id,nombre,email,especialidad) VALUES({hack},{expert},'Experto','qa@x.co','') RETURNING id"
        ))
        .await;
    (hack, team, juror)
}

// DB-4 / AUTHK-3: emails are unique case-insensitively.
parity!(qa_usuarios_email_ci_unique, |t| {
    t.exec(&format!("INSERT INTO usuarios(password,is_superuser,username,first_name,last_name,email,is_staff,is_active,date_joined,rol,telefono,activo) VALUES('{HASH}',false,'qa-email-1','A','B','Dup@Example.com',false,true,now(),'ADMIN','',true)")).await;
    assert!(
        rejects(&t, &format!("INSERT INTO usuarios(password,is_superuser,username,first_name,last_name,email,is_staff,is_active,date_joined,rol,telefono,activo) VALUES('{HASH}',false,'qa-email-2','A','B','dup@example.com',false,true,now(),'ADMIN','',true)")).await,
        "case-insensitive duplicate email must be rejected"
    );
});

// DB-6: budgets cannot be negative.
parity!(qa_presupuesto_monto_nonnegative, |t| {
    let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
    assert!(
        rejects(&t, &format!("INSERT INTO presupuestos(reto_id,monto_total,alerta_80_enviada,creado_en,actualizado_en) VALUES({reto},-1,false,now(),now())")).await,
        "negative budget must be rejected"
    );
    t.i64(&format!("INSERT INTO presupuestos(reto_id,monto_total,alerta_80_enviada,creado_en,actualizado_en) VALUES({reto},1000,false,now(),now()) RETURNING id")).await;
});

// DB-7: expenses cannot be negative.
parity!(qa_gasto_monto_nonnegative, |t| {
    let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
    let budget = t.i64(&format!("INSERT INTO presupuestos(reto_id,monto_total,alerta_80_enviada,creado_en,actualizado_en) VALUES({reto},1000,false,now(),now()) RETURNING id")).await;
    assert!(
        rejects(&t, &format!("INSERT INTO gastos(presupuesto_id,categoria,monto,estado,descripcion,creado_en) VALUES({budget},'OTROS',-5,'EJECUTADO','',now())")).await,
        "negative expense must be rejected"
    );
});

// DB-8: deliverable maximum and nota are never negative or above the maximum.
parity!(qa_entregable_nota_range, |t| {
    let reto = t.reto(COMPANY_USER, "abierto", "reto").await;
    let student = t.user("qa-est", "ESTUDIANTE").await;
    assert!(
        rejects(&t, &format!("INSERT INTO entregables(reto_id,estudiante_id,titulo,es_final,puntaje_maximo,archivo,comentario_estudiante,comentario_profesor,nota,calificacion_publicada,fecha_entrega,actualizado_en,estado) VALUES({reto},{student},'qa',false,5,'x.pdf','','',6,false,now(),now(),'ENVIADO')")).await,
        "nota above puntaje_maximo must be rejected"
    );
    assert!(
        rejects(&t, &format!("INSERT INTO entregables(reto_id,estudiante_id,titulo,es_final,puntaje_maximo,archivo,comentario_estudiante,comentario_profesor,nota,calificacion_publicada,fecha_entrega,actualizado_en,estado) VALUES({reto},{student},'qa',false,-1,'x.pdf','','',NULL,false,now(),now(),'ENVIADO')")).await,
        "negative puntaje_maximo must be rejected"
    );
});

// DB-9: professor evaluation grades 0-5.
parity!(qa_evaluacion_nota_range, |t| {
    let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
    let student = t.user("qa-est", "ESTUDIANTE").await;
    let teacher = t.user("qa-prof", "PROFESOR").await;
    let deliverable = t.deliverable(reto, student).await;
    assert!(
        rejects(&t, &format!("INSERT INTO evaluaciones(entregable_id,profesor_id,nota,comentario,fecha,actualizado_en) VALUES({deliverable},{teacher},5.01,'',now(),now())")).await,
        "evaluation nota above 5 must be rejected"
    );
});

// DB-10: rubric criterion maximum is never negative.
parity!(qa_criterio_puntaje_maximo_nonnegative, |t| {
    let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
    let rubric = t.i64(&format!("INSERT INTO evaluacion_rubrica(reto_id,nombre,descripcion,activa,creada_en) VALUES({reto},'qa','',true,now()) RETURNING id")).await;
    assert!(
        rejects(&t, &format!("INSERT INTO evaluacion_criteriorubrica(rubrica_id,nombre,descripcion,puntaje_maximo,orden) VALUES({rubric},'criterio','',-1,0)")).await,
        "negative criterion maximum must be rejected"
    );
});

// DB-11: per-criterion score is never negative.
parity!(qa_evaluacioncriterio_puntaje_nonnegative, |t| {
    let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
    let student = t.user("qa-est", "ESTUDIANTE").await;
    let teacher = t.user("qa-prof", "PROFESOR").await;
    let deliverable = t.deliverable(reto, student).await;
    let rubric = t.i64(&format!("INSERT INTO evaluacion_rubrica(reto_id,nombre,descripcion,activa,creada_en) VALUES({reto},'qa','',true,now()) RETURNING id")).await;
    let criterio = t.i64(&format!("INSERT INTO evaluacion_criteriorubrica(rubrica_id,nombre,descripcion,puntaje_maximo,orden) VALUES({rubric},'criterio','',5,0) RETURNING id")).await;
    let evaluation = t.i64(&format!("INSERT INTO evaluaciones(entregable_id,profesor_id,nota,comentario,rubrica_id,fecha,actualizado_en) VALUES({deliverable},{teacher},4,'',{rubric},now(),now()) RETURNING id")).await;
    assert!(
        rejects(&t, &format!("INSERT INTO evaluacion_evaluacioncriterio(evaluacion_id,criterio_id,puntaje) VALUES({evaluation},{criterio},-0.5)")).await,
        "negative criterion score must be rejected"
    );
});

// DB-12 / HCK-03: hackathon votes range 0-5.
parity!(qa_votacion_puntaje_range, |t| {
    let (hack, team, juror) = hackathon_jury(&t, COMPANY_USER).await;
    assert!(
        rejects(&t, &format!("INSERT INTO votaciones_hackaton(hackathon_id,jurado_id,equipo_id,puntaje,comentario,fecha) VALUES({hack},{juror},{team},-1,'',now())")).await,
        "negative vote must be rejected"
    );
    assert!(
        rejects(&t, &format!("INSERT INTO votaciones_hackaton(hackathon_id,jurado_id,equipo_id,puntaje,comentario,fecha) VALUES({hack},{juror},{team},5.01,'',now())")).await,
        "vote above 5 must be rejected"
    );
});

// DB-13: satisfaction ratings 0-5 or unanswered.
parity!(qa_cierre_calificacion_range, |t| {
    let reto = t.reto(COMPANY_USER, "finalizado", "reto").await;
    let cierre = t.i64(&format!("INSERT INTO cierres_reto(reto_id,notas,impacto,encuesta_enviada,actualizado_en) VALUES({reto},'','',false,now()) RETURNING id")).await;
    assert!(
        rejects(&t, &format!("INSERT INTO cierre_encuestasatisfaccion(cierre_id,participante_id,calificacion,comentario,creada_en) VALUES({cierre},{STUDENT},6,'',now())")).await,
        "rating above 5 must be rejected"
    );
});

// DB-14: progress never exceeds 100%.
parity!(qa_seguimiento_porcentaje_max, |t| {
    let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
    assert!(
        rejects(&t, &format!("INSERT INTO seguimientos_reto(reto_id,tipo_sesion,fecha_sesion,porcentaje_avance,avances,observaciones,acuerdos,creado_en) VALUES({reto},'seguimiento',now()::date,101,'','','',now())")).await,
        "progress above 100 must be rejected"
    );
});

// DB-15: tentative end is never before tentative start.
parity!(qa_retos_fechas_order, |t| {
    let reto = t.reto(COMPANY_USER, "borrador", "reto").await;
    assert!(
        rejects(&t, &format!("UPDATE retos SET fecha_inicio_tentativa='2026-06-01',fecha_fin_tentativa='2026-01-01' WHERE id={reto}")).await,
        "reversed tentative dates must be rejected"
    );
});

// DB-16: stage end is never before stage start.
parity!(qa_etapas_fechas_order, |t| {
    let (hack, _, _) = hackathon_jury(&t, COMPANY_USER).await;
    assert!(
        rejects(&t, &format!("INSERT INTO etapas_hackaton(hackathon_id,tipo,titulo,inicia_en,termina_en,orden) VALUES({hack},'presentacion','P',now(),now()-interval '1 day',1)")).await,
        "reversed stage dates must be rejected"
    );
});

// DB-17: document expiry is never before issue.
parity!(qa_documentos_fechas_order, |t| {
    let (_, company) = t.company_user("qa-emp", "900999001").await;
    assert!(
        rejects(&t, &format!("INSERT INTO documentos_empresas(empresa_id,tipo_documento,archivo,estado,fecha_expedicion,fecha_vencimiento,fecha_carga,motivo_rechazo) VALUES({company},'RUT','x.pdf','VERIFICADO','2026-06-01','2026-01-01',now(),'')")).await,
        "reversed document dates must be rejected"
    );
});

// DB-18: document type must exist in the catalog.
parity!(qa_documentos_tipo_fk, |t| {
    let (_, company) = t.company_user("qa-emp", "900999002").await;
    assert!(
        rejects(&t, &format!("INSERT INTO documentos_empresas(empresa_id,tipo_documento,archivo,estado,fecha_carga,motivo_rechazo) VALUES({company},'NO_EXISTE','x.pdf','PENDIENTE',now(),'')")).await,
        "unknown document type must be rejected"
    );
});

// DB-19: state columns are constrained.
parity!(qa_estado_checks, |t| {
    let reto = t.reto(COMPANY_USER, "borrador", "reto").await;
    assert!(
        rejects(
            &t,
            &format!("UPDATE retos SET estado='no_existe' WHERE id={reto}")
        )
        .await,
        "unknown reto state must be rejected"
    );
    let teacher = t.user("qa-prof", "PROFESOR").await;
    let integration = t.integrate(reto, teacher).await;
    assert!(
        rejects(
            &t,
            &format!(
                "UPDATE integraciones_academicas SET estado='no_existe' WHERE id={integration}"
            )
        )
        .await,
        "unknown integration state must be rejected"
    );
    let student = t.user("qa-est", "ESTUDIANTE").await;
    let application = t.accept(reto, student).await;
    assert!(
        rejects(
            &t,
            &format!("UPDATE postulaciones SET estado='no_existe' WHERE id={application}")
        )
        .await,
        "unknown postulation state must be rejected"
    );
    let deliverable = t.deliverable(reto, student).await;
    assert!(
        rejects(
            &t,
            &format!("UPDATE entregables SET estado='no_existe' WHERE id={deliverable}")
        )
        .await,
        "unknown deliverable state must be rejected"
    );
});

// DB-2: team names unique within a challenge.
parity!(qa_equipos_nombre_unique, |t| {
    let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
    t.i64(&format!("INSERT INTO equipos(reto_id,nombre,creado_en) VALUES({reto},'Duplicado',now()) RETURNING id")).await;
    assert!(
        rejects(
            &t,
            &format!(
                "INSERT INTO equipos(reto_id,nombre,creado_en) VALUES({reto},'Duplicado',now())"
            )
        )
        .await,
        "duplicate team name must be rejected"
    );
});

// DB-3: academic team names unique within a challenge.
parity!(qa_equipos_academico_nombre_unique, |t| {
    let reto = t.reto(COMPANY_USER, "en_curso", "reto").await;
    t.i64(&format!("INSERT INTO equipos_reto_academico(reto_id,nombre_equipo) VALUES({reto},'Duplicado') RETURNING id")).await;
    assert!(
        rejects(&t, &format!("INSERT INTO equipos_reto_academico(reto_id,nombre_equipo) VALUES({reto},'Duplicado')")).await,
        "duplicate academic team name must be rejected"
    );
});

// DB-5: a user may be jury of a hackathon only once.
parity!(qa_jurados_duplicado, |t| {
    let (hack, _, _) = hackathon_jury(&t, COMPANY_USER).await;
    let expert = t.user("qa-experto2", "EXPERTO").await;
    t.i64(&format!("INSERT INTO jurados_hackaton(hackathon_id,usuario_id,nombre,email,especialidad) VALUES({hack},{expert},'Exp','e2@x.co','') RETURNING id")).await;
    assert!(
        rejects(&t, &format!("INSERT INTO jurados_hackaton(hackathon_id,usuario_id,nombre,email,especialidad) VALUES({hack},{expert},'Exp','e3@x.co','')")).await,
        "duplicate juror user must be rejected"
    );
});

// DB-25: deleting a user nulls audit actor references instead of failing.
parity!(qa_audit_set_null, |t| {
    let actor = t.user("qa-audit", "ADMIN").await;
    let victim = t.user("qa-victim", "ADMIN").await;
    t.exec(&format!("INSERT INTO log_actividad(usuario_id,realizado_por_id,identificador,accion,detalle,creado_en) VALUES({victim},{actor},'qa','PRUEBA','',now())")).await;
    t.exec(&format!("INSERT INTO historial_estados(content_type_id,object_id,campo,estado_anterior,estado_nuevo,comentario,realizado_por_id,fecha) VALUES(1,1,'estado','','x','',{actor},now())")).await;
    t.exec(&format!("DELETE FROM usuarios WHERE id={actor}"))
        .await;
    assert_eq!(t.i64(&format!("SELECT count(*) FROM log_actividad WHERE realizado_por_id IS NULL AND usuario_id={victim}")).await, 1);
    assert_eq!(
        t.i64(
            "SELECT count(*) FROM historial_estados WHERE realizado_por_id IS NULL AND object_id=1"
        )
        .await,
        1
    );
});

// OPS-11: `migrate` is idempotent and skips already-applied migrations. Runs
// outside common::run because that harness `tokio::spawn`s the body, which trips
// sqlx's higher-ranked `Executor`/`Send` bounds for the borrowed transaction.
#[tokio::test]
#[ignore = "requires local PostgreSQL; each test uses its own schema"]
async fn qa_migrations_idempotent() {
    use sqlx::Executor;
    let config = retosean::config::Config::load().unwrap();
    let admin = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with(config.database.clone())
        .await
        .unwrap();
    let schema = format!("rust_test_{}", uuid::Uuid::new_v4().simple());
    admin
        .execute(format!("CREATE SCHEMA {schema}").as_str())
        .await
        .unwrap();
    let search_path = schema.clone();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .after_connect(move |conn, _| {
            let statement = format!("SET search_path TO {search_path}");
            Box::pin(async move {
                conn.execute(statement.as_str()).await?;
                Ok(())
            })
        })
        .connect_with(config.database.clone())
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("../sql/schema.sql"))
        .execute(&pool)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    retosean::schema_migrations::baseline(&mut tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let applied = retosean::schema_migrations::run(&pool).await.unwrap();
    assert!(
        applied.is_empty(),
        "already-applied migrations must be skipped"
    );
    pool.close().await;
    admin
        .execute(format!("DROP SCHEMA {schema} CASCADE").as_str())
        .await
        .unwrap();
    admin.close().await;
}
