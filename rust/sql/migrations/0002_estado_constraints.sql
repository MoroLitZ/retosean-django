-- CHECK constraints on money, score, date ordering and state columns.
-- Added NOT VALID so legacy rows are grandfathered: pre-existing data that does
-- not satisfy a rule is left as-is, while every new write is constrained. The
-- fresh-database path (schema.sql) defines these same rules inline as VALID.

-- DB-6 / DB-7: money amounts are never negative.
ALTER TABLE "presupuestos" ADD CONSTRAINT "presupuestos_monto_total_nonnegative" CHECK ("monto_total" >= 0) NOT VALID;
ALTER TABLE "gastos" ADD CONSTRAINT "gastos_monto_nonnegative" CHECK ("monto" >= 0) NOT VALID;

-- DB-8: deliverable maximum and grade sanity.
ALTER TABLE "entregables" ADD CONSTRAINT "entregables_puntaje_maximo_nonnegative" CHECK ("puntaje_maximo" >= 0) NOT VALID;
ALTER TABLE "entregables" ADD CONSTRAINT "entregables_nota_range" CHECK ("nota" IS NULL OR ("nota" >= 0 AND "nota" <= "puntaje_maximo")) NOT VALID;

-- DB-9: professor evaluation grades 0-5.
ALTER TABLE "evaluaciones" ADD CONSTRAINT "evaluaciones_nota_range" CHECK ("nota" >= 0 AND "nota" <= 5) NOT VALID;

-- DB-10 / DB-11: rubric maxima and criterion scores never negative.
ALTER TABLE "evaluacion_criteriorubrica" ADD CONSTRAINT "evaluacion_criteriorubrica_puntaje_maximo_nonnegative" CHECK ("puntaje_maximo" >= 0) NOT VALID;
ALTER TABLE "evaluacion_evaluacioncriterio" ADD CONSTRAINT "evaluacion_evaluacioncriterio_puntaje_nonnegative" CHECK ("puntaje" >= 0) NOT VALID;

-- DB-12 / HCK-03: hackathon votes 0-5.
ALTER TABLE "votaciones_hackaton" ADD CONSTRAINT "votaciones_hackaton_puntaje_range" CHECK ("puntaje" >= 0 AND "puntaje" <= 5) NOT VALID;

-- DB-13: satisfaction ratings 0-5 or unanswered.
ALTER TABLE "cierre_encuestasatisfaccion" ADD CONSTRAINT "cierre_encuestasatisfaccion_calificacion_range" CHECK ("calificacion" IS NULL OR ("calificacion" >= 0 AND "calificacion" <= 5)) NOT VALID;

-- DB-14: progress never exceeds 100%.
ALTER TABLE "seguimientos_reto" ADD CONSTRAINT "seguimientos_reto_porcentaje_avance_max" CHECK ("porcentaje_avance" <= 100) NOT VALID;

-- DB-15 / DB-16 / DB-17: end dates never precede start dates.
ALTER TABLE "retos" ADD CONSTRAINT "retos_fechas_tentativas_order" CHECK ("fecha_inicio_tentativa" IS NULL OR "fecha_fin_tentativa" IS NULL OR "fecha_fin_tentativa" >= "fecha_inicio_tentativa") NOT VALID;
ALTER TABLE "etapas_hackaton" ADD CONSTRAINT "etapas_hackaton_fechas_order" CHECK ("inicia_en" IS NULL OR "termina_en" IS NULL OR "termina_en" >= "inicia_en") NOT VALID;
ALTER TABLE "documentos_empresas" ADD CONSTRAINT "documentos_empresas_fechas_order" CHECK ("fecha_expedicion" IS NULL OR "fecha_vencimiento" IS NULL OR "fecha_vencimiento" >= "fecha_expedicion") NOT VALID;

-- DB-19: state columns restricted to their state machines
-- (states.json / states_meta.json; all values written by the application).
ALTER TABLE "retos" ADD CONSTRAINT "retos_estado_check" CHECK ("estado" IN ('borrador','en_revision','rechazado','aprobado','abierto','en_curso','pausado','finalizado','cancelado')) NOT VALID;
ALTER TABLE "integraciones_academicas" ADD CONSTRAINT "integraciones_academicas_estado_check" CHECK ("estado" IN ('borrador','en_revision','rechazada','aprobada','publicada')) NOT VALID;
ALTER TABLE "hackathons" ADD CONSTRAINT "hackathons_estado_check" CHECK ("estado" IN ('borrador','publicado','en_curso','finalizado','cancelado')) NOT VALID;
ALTER TABLE "postulaciones" ADD CONSTRAINT "postulaciones_estado_check" CHECK ("estado" IN ('PENDIENTE','EN_REVISION','ACEPTADA','RECHAZADA','RETIRADA')) NOT VALID;
ALTER TABLE "entregables" ADD CONSTRAINT "entregables_estado_check" CHECK ("estado" IN ('ENVIADO','EN_REVISION','CALIFICADO')) NOT VALID;
ALTER TABLE "gastos" ADD CONSTRAINT "gastos_estado_check" CHECK ("estado" IN ('COMPROMETIDO','EJECUTADO')) NOT VALID;
ALTER TABLE "documentos_empresas" ADD CONSTRAINT "documentos_empresas_estado_check" CHECK ("estado" IN ('PENDIENTE','CARGADO','VERIFICADO','RECHAZADO','VENCIDO')) NOT VALID;
ALTER TABLE "cierre_solicitudlogistica" ADD CONSTRAINT "cierre_solicitudlogistica_estado_check" CHECK ("estado" IN ('SOLICITADA','CONFIRMADA','DESCARTADA')) NOT VALID;
ALTER TABLE "inscripciones_hackaton" ADD CONSTRAINT "inscripciones_hackaton_estado_check" CHECK ("estado" IN ('PENDIENTE','ACEPTADA','RECHAZADA')) NOT VALID;

-- DB-27: read timestamp must agree with the read flag.
