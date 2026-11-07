-- Unique indexes (DB-4, DB-2, DB-3, DB-5), the document-type FK (DB-18) and
-- audit-actor SET NULL (DB-25). Unique indexes cannot be created NOT VALID, so
-- duplicates are deduped first and the choice is explained in each comment.

-- DB-4 / AUTHK-3: emails are unique case-insensitively. Keep the earliest
-- account per lower(email); later duplicates lose the shared address (which
-- already disabled email login, since login requires exactly one match).
UPDATE "usuarios" d
SET "email" = ''
FROM "usuarios" k
WHERE lower(d."email") = lower(k."email")
  AND d."email" <> ''
  AND k."email" <> ''
  AND k.id = (SELECT min("id") FROM "usuarios" WHERE lower("email") = lower(k."email") AND "email" <> '')
  AND d.id <> k.id;

CREATE UNIQUE INDEX "usuarios_email_ci_uniq" ON "usuarios" (lower("email")) WHERE "email" <> '';

-- DB-5: one juror per hackathon, by linked user or (when unlinked) by email.
-- Duplicates keep one row per identity; the survivors are demoted to manual
-- jurors with a unique email so no vote or audit row is lost.
UPDATE "jurados_hackaton" d
SET "usuario_id" = NULL,
    "email" = d."email" || '#' || d.id
WHERE d."usuario_id" IS NOT NULL
  AND EXISTS (
      SELECT 1 FROM "jurados_hackaton" k
      WHERE k."hackathon_id" = d."hackathon_id"
        AND k."usuario_id" = d."usuario_id"
        AND k.id < d.id
  );

UPDATE "jurados_hackaton" d
SET "email" = d."email" || '#' || d.id
WHERE d."email" <> ''
  AND EXISTS (
      SELECT 1 FROM "jurados_hackaton" k
      WHERE k."hackathon_id" = d."hackathon_id"
        AND lower(k."email") = lower(d."email")
        AND k.id < d.id
  );

CREATE UNIQUE INDEX "jurados_hackaton_usuario_uniq" ON "jurados_hackaton" ("hackathon_id", "usuario_id") WHERE "usuario_id" IS NOT NULL;
CREATE UNIQUE INDEX "jurados_hackaton_email_ci_uniq" ON "jurados_hackaton" ("hackathon_id", lower("email")) WHERE "email" <> '';

-- DB-2 / DB-3: team names unique within a challenge. Duplicated names get a
-- stable " #<id>" suffix (this cannot silently merge teams or their members).
UPDATE "equipos" d
SET "nombre" = d."nombre" || ' #' || d.id
WHERE EXISTS (
    SELECT 1 FROM "equipos" k
    WHERE k."reto_id" = d."reto_id" AND k."nombre" = d."nombre" AND k.id < d.id
);

ALTER TABLE "equipos" ADD CONSTRAINT "equipos_reto_nombre_unique" UNIQUE ("reto_id", "nombre");

UPDATE "equipos_reto_academico" d
SET "nombre_equipo" = d."nombre_equipo" || ' #' || d.id
WHERE EXISTS (
    SELECT 1 FROM "equipos_reto_academico" k
    WHERE k."reto_id" = d."reto_id" AND k."nombre_equipo" = d."nombre_equipo" AND k.id < d.id
);

ALTER TABLE "equipos_reto_academico" ADD CONSTRAINT "equipos_reto_academico_reto_nombre_equipo_unique" UNIQUE ("reto_id", "nombre_equipo");

-- DB-18: document types reference the catalog. NOT VALID grandfathers legacy
-- rows with codes that no longer exist; new/updated rows must reference a code.
ALTER TABLE "documentos_empresas" ADD CONSTRAINT "documentos_empresas_tipo_documento_fk_tipos_documento_legal" FOREIGN KEY ("tipo_documento") REFERENCES "tipos_documento_legal" ("codigo") DEFERRABLE INITIALLY DEFERRED NOT VALID;

-- DB-25: deleting a user keeps audit history (actor is set NULL instead of
-- blocking the delete).
ALTER TABLE "log_actividad" DROP CONSTRAINT "log_actividad_usuario_id_ab07d972_fk_usuarios_id";
ALTER TABLE "log_actividad" ADD CONSTRAINT "log_actividad_usuario_id_ab07d972_fk_usuarios_id" FOREIGN KEY ("usuario_id") REFERENCES "usuarios" ("id") ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED;

ALTER TABLE "log_actividad" DROP CONSTRAINT "log_actividad_realizado_por_id_84fa3425_fk_usuarios_id";
ALTER TABLE "log_actividad" ADD CONSTRAINT "log_actividad_realizado_por_id_84fa3425_fk_usuarios_id" FOREIGN KEY ("realizado_por_id") REFERENCES "usuarios" ("id") ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED;

ALTER TABLE "historial_estados" DROP CONSTRAINT "historial_estados_realizado_por_id_fc531431_fk_usuarios_id";
ALTER TABLE "historial_estados" ADD CONSTRAINT "historial_estados_realizado_por_id_fc531431_fk_usuarios_id" FOREIGN KEY ("realizado_por_id") REFERENCES "usuarios" ("id") ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED;