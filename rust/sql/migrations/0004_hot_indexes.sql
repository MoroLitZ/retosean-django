-- DB-26: indexes on common hot-path filters (read state, challenge/entity state).

CREATE INDEX "notificaciones_usuario_leida_idx" ON "notificaciones" ("usuario_id", "leida");
CREATE INDEX "retos_estado_idx" ON "retos" ("estado");
CREATE INDEX "postulaciones_reto_estado_idx" ON "postulaciones" ("reto_id", "estado");
CREATE INDEX "entregables_reto_estado_idx" ON "entregables" ("reto_id", "estado");
CREATE INDEX "gastos_presupuesto_estado_idx" ON "gastos" ("presupuesto_id", "estado");
CREATE INDEX "inscripciones_hackaton_hackathon_estado_idx" ON "inscripciones_hackaton" ("hackathon_id", "estado");
CREATE INDEX "hackathons_estado_idx" ON "hackathons" ("estado");