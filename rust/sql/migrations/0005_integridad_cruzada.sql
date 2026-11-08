-- QA package S: cross-table integrity (N20, N51, N58, N59, N60) as PL/pgSQL
-- triggers, plus the gastos soft-delete column (N98). Mirrored inline in
-- schema.sql for fresh databases; existing databases converge via `migrate`.
-- Triggers raise with SQLSTATE 23514 so the app maps them to HTTP 400.

-- N98: logical delete for expenses (gastos.eliminado_en).
ALTER TABLE "gastos" ADD COLUMN IF NOT EXISTS "eliminado_en" timestamp with time zone NULL;

-- N20: a vote's juror must belong to the same hackathon and its team to the
-- hackathon's own reto.
CREATE OR REPLACE FUNCTION fn_votaciones_hackaton_integridad() RETURNS trigger AS $$
BEGIN
    IF NEW.jurado_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM "jurados_hackaton" j
         WHERE j.id = NEW.jurado_id AND j.hackathon_id = NEW.hackathon_id
    ) THEN
        RAISE EXCEPTION 'El jurado no pertenece al hackathon de la votación' USING ERRCODE = '23514';
    END IF;
    IF NOT EXISTS (
        SELECT 1
          FROM "hackathons" h
          JOIN "equipos" e ON e.id = NEW.equipo_id
         WHERE h.id = NEW.hackathon_id AND e.reto_id = h.reto_id
    ) THEN
        RAISE EXCEPTION 'El equipo no pertenece al reto del hackathon' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS "trg_votaciones_hackaton_integridad" ON "votaciones_hackaton";
CREATE TRIGGER "trg_votaciones_hackaton_integridad"
    BEFORE INSERT OR UPDATE ON "votaciones_hackaton"
    FOR EACH ROW EXECUTE FUNCTION fn_votaciones_hackaton_integridad();

-- N51: a per-criterion score cannot exceed its criterion's maximum.
CREATE OR REPLACE FUNCTION fn_evaluacioncriterio_puntaje_maximo() RETURNS trigger AS $$
BEGIN
    IF NEW.puntaje > (
        SELECT c.puntaje_maximo FROM "evaluacion_criteriorubrica" c WHERE c.id = NEW.criterio_id
    ) THEN
        RAISE EXCEPTION 'El puntaje supera el máximo del criterio' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS "trg_evaluacioncriterio_puntaje_maximo" ON "evaluacion_evaluacioncriterio";
CREATE TRIGGER "trg_evaluacioncriterio_puntaje_maximo"
    BEFORE INSERT OR UPDATE ON "evaluacion_evaluacioncriterio"
    FOR EACH ROW EXECUTE FUNCTION fn_evaluacioncriterio_puntaje_maximo();

-- N58: a hackathon registration's team must belong to the hackathon's reto.
CREATE OR REPLACE FUNCTION fn_inscripciones_hackaton_reto() RETURNS trigger AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
          FROM "hackathons" h
          JOIN "equipos" e ON e.id = NEW.equipo_id
         WHERE h.id = NEW.hackathon_id AND e.reto_id = h.reto_id
    ) THEN
        RAISE EXCEPTION 'El equipo no pertenece al reto del hackathon' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS "trg_inscripciones_hackaton_reto" ON "inscripciones_hackaton";
CREATE TRIGGER "trg_inscripciones_hackaton_reto"
    BEFORE INSERT OR UPDATE ON "inscripciones_hackaton"
    FOR EACH ROW EXECUTE FUNCTION fn_inscripciones_hackaton_reto();

-- N59: a deliverable's reto must equal its team's reto (when a team is set).
CREATE OR REPLACE FUNCTION fn_entregables_equipo_reto() RETURNS trigger AS $$
BEGIN
    IF NEW.equipo_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM "equipos" e WHERE e.id = NEW.equipo_id AND e.reto_id = NEW.reto_id
    ) THEN
        RAISE EXCEPTION 'El entregable no coincide con el reto de su equipo' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS "trg_entregables_equipo_reto" ON "entregables";
CREATE TRIGGER "trg_entregables_equipo_reto"
    BEFORE INSERT OR UPDATE ON "entregables"
    FOR EACH ROW EXECUTE FUNCTION fn_entregables_equipo_reto();

-- N60: role enforcement for columns that strictly reference students or
-- professors. Columns that legitimately accept other roles (jurados, expertos
-- invitados, certificados, actores de auditoría, empresas, etc.) are documented
-- in the report and left unconstrained. Trigger arguments:
--   [0] column, [1] required role, [2] Spanish error message.
CREATE OR REPLACE FUNCTION fn_usuario_rol_exigido() RETURNS trigger AS $$
DECLARE
    columna      text := TG_ARGV[0];
    rol_esperado text := TG_ARGV[1];
    mensaje      text := TG_ARGV[2];
    uid          bigint;
    previo       bigint;
BEGIN
    EXECUTE format('SELECT ($1).%I', columna) INTO uid USING NEW;
    -- Only a new or changed reference is checked: a later role change of the
    -- user must not block state updates on their existing rows.
    IF TG_OP = 'UPDATE' THEN
        EXECUTE format('SELECT ($1).%I', columna) INTO previo USING OLD;
        IF previo IS NOT DISTINCT FROM uid THEN
            RETURN NEW;
        END IF;
    END IF;
    IF uid IS NOT NULL AND EXISTS (
        SELECT 1 FROM "usuarios" u WHERE u.id = uid AND u.rol <> rol_esperado
    ) THEN
        RAISE EXCEPTION '%', mensaje USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS "trg_estudiantes_usuario_rol" ON "estudiantes";
CREATE TRIGGER "trg_estudiantes_usuario_rol"
    BEFORE INSERT OR UPDATE ON "estudiantes"
    FOR EACH ROW EXECUTE FUNCTION fn_usuario_rol_exigido('usuario_id', 'ESTUDIANTE', 'El usuario debe tener rol ESTUDIANTE.');

DROP TRIGGER IF EXISTS "trg_profesores_usuario_rol" ON "profesores";
CREATE TRIGGER "trg_profesores_usuario_rol"
    BEFORE INSERT OR UPDATE ON "profesores"
    FOR EACH ROW EXECUTE FUNCTION fn_usuario_rol_exigido('usuario_id', 'PROFESOR', 'El usuario debe tener rol PROFESOR.');

DROP TRIGGER IF EXISTS "trg_postulaciones_estudiante_rol" ON "postulaciones";
CREATE TRIGGER "trg_postulaciones_estudiante_rol"
    BEFORE INSERT OR UPDATE ON "postulaciones"
    FOR EACH ROW EXECUTE FUNCTION fn_usuario_rol_exigido('estudiante_id', 'ESTUDIANTE', 'El estudiante de la postulación debe tener rol ESTUDIANTE.');

DROP TRIGGER IF EXISTS "trg_entregables_estudiante_rol" ON "entregables";
CREATE TRIGGER "trg_entregables_estudiante_rol"
    BEFORE INSERT OR UPDATE ON "entregables"
    FOR EACH ROW EXECUTE FUNCTION fn_usuario_rol_exigido('estudiante_id', 'ESTUDIANTE', 'El estudiante del entregable debe tener rol ESTUDIANTE.');

DROP TRIGGER IF EXISTS "trg_miembros_equipo_estudiante_rol" ON "miembros_equipo";
CREATE TRIGGER "trg_miembros_equipo_estudiante_rol"
    BEFORE INSERT OR UPDATE ON "miembros_equipo"
    FOR EACH ROW EXECUTE FUNCTION fn_usuario_rol_exigido('estudiante_id', 'ESTUDIANTE', 'El estudiante del equipo debe tener rol ESTUDIANTE.');

DROP TRIGGER IF EXISTS "trg_equipos_academicos_estudiante_rol" ON "equipos_reto_academico_estudiantes";
CREATE TRIGGER "trg_equipos_academicos_estudiante_rol"
    BEFORE INSERT OR UPDATE ON "equipos_reto_academico_estudiantes"
    FOR EACH ROW EXECUTE FUNCTION fn_usuario_rol_exigido('usuario_id', 'ESTUDIANTE', 'El estudiante del equipo académico debe tener rol ESTUDIANTE.');

DROP TRIGGER IF EXISTS "trg_integraciones_profesor_rol" ON "integraciones_academicas";
CREATE TRIGGER "trg_integraciones_profesor_rol"
    BEFORE INSERT OR UPDATE ON "integraciones_academicas"
    FOR EACH ROW EXECUTE FUNCTION fn_usuario_rol_exigido('profesor_id', 'PROFESOR', 'El profesor de la integración debe tener rol PROFESOR.');

DROP TRIGGER IF EXISTS "trg_evaluaciones_profesor_rol" ON "evaluaciones";
CREATE TRIGGER "trg_evaluaciones_profesor_rol"
    BEFORE INSERT OR UPDATE ON "evaluaciones"
    FOR EACH ROW EXECUTE FUNCTION fn_usuario_rol_exigido('profesor_id', 'PROFESOR', 'El profesor de la evaluación debe tener rol PROFESOR.');

DROP TRIGGER IF EXISTS "trg_rubrica_creador_rol" ON "evaluacion_rubrica";
CREATE TRIGGER "trg_rubrica_creador_rol"
    BEFORE INSERT OR UPDATE ON "evaluacion_rubrica"
    FOR EACH ROW EXECUTE FUNCTION fn_usuario_rol_exigido('creada_por_id', 'PROFESOR', 'El creador de la rúbrica debe tener rol PROFESOR.');

DROP TRIGGER IF EXISTS "trg_equipos_academicos_profesor_rol" ON "equipos_reto_academico_profesores";
CREATE TRIGGER "trg_equipos_academicos_profesor_rol"
    BEFORE INSERT OR UPDATE ON "equipos_reto_academico_profesores"
    FOR EACH ROW EXECUTE FUNCTION fn_usuario_rol_exigido('usuario_id', 'PROFESOR', 'El profesor del equipo académico debe tener rol PROFESOR.');