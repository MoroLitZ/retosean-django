-- RET-03: each academic team belongs to the integration of its owning professor,
-- so a different professor on the same reto can no longer edit or delete it.
ALTER TABLE "equipos_reto_academico" ADD COLUMN "integracion_id" bigint NULL;

ALTER TABLE "equipos_reto_academico"
    ADD CONSTRAINT "equipos_reto_academico_integracion_id_fk_integraciones_academicas"
    FOREIGN KEY ("integracion_id") REFERENCES "integraciones_academicas" ("id")
    DEFERRABLE INITIALLY DEFERRED;

CREATE INDEX "equipos_reto_academico_integracion_id"
    ON "equipos_reto_academico" ("integracion_id");

-- Django's 0018 `enlazar_equipos`: existing teams keep the oldest integration of
-- their reto as the historical owner.
UPDATE "equipos_reto_academico" q
   SET "integracion_id" = (
       SELECT i.id
         FROM "integraciones_academicas" i
        WHERE i.reto_id = q.reto_id
        ORDER BY i.id
        LIMIT 1
   )
 WHERE q.integracion_id IS NULL;