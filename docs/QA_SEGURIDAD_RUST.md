# Auditoría permanente de QA y seguridad — RetosEAN Rust

## 1. Resumen ejecutivo

**Alcance:** aplicación Rust (axum/SQLx), SPA en `rust/web`, esquema PostgreSQL, contenedores/CI y dependencias del repositorio RetosEAN.

**Metodología en cuatro fases:**

1. **Auditoría estática multi-modelo:** agentes de auditoría sobre backend, frontend, esquema, configuración y contenedores.
2. **Consolidación:** normalización de 128 hallazgos en `00-consolidated.md`.
3. **Corrección con pruebas de regresión:** cada corrección debía incorporar una prueba específica en `tests/qa_*.rs`.
4. **Verificación dinámica:** pruebas de caja negra antes y después de corregir, ataques dirigidos y recorrido de navegador.

**Cifras finales:**

- Hallazgos consolidados: **128**
- Corregidos: **105** (incluye **23** corregidos en esquema)
- Rechazados: **10**
- Diferidos: **13**
- Pruebas automáticas: **370 → 447** pruebas que pasan
- Herramientas finales limpias: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `node --check` sobre todo `rust/web/js/*.js`

Las decisiones del revisor principal están en `lead-notes.md` y son autoritativas; donde hay conflicto con un reporte de agente, prevalecen.

## 2. Metodología

La actividad se ejecutó con:

- **13 agentes de auditoría InferHub** sobre los modelos DeepSeek V4 Pro, Qwen 3.8 Max y GLM 5.3.
- Las llamadas de Kimi K3.1 Pro retornaron vacías y se reejecutaron en otros modelos.
- **1 agente de consolidación** produjo `00-consolidated.md`.
- **1 agente de sondeo dinámico** ejecutó pruebas de caja negra.
- **8 agentes de corrección opencode** más **6 agentes de seguimiento**.
- Los paquetes A, D, F y H agotaron el límite de 90 minutos; los seguimientos los finalizaron. El paquete H fue terminado por el revisor principal.

**Reglas aplicadas:**

- Verificar antes de corregir; no corregir un falso positivo sin evidencia.
- Respetar propiedad de archivos: cada agente solo podía modificar su paquete.
- Toda corrección de seguridad debía acompañarse de una prueba de regresión.
- Un hallazgo solo se marca “Corregido” si existe evidencia en código y prueba.
- Las decisiones de `lead-notes.md` tienen prioridad sobre cualquier reporte de agente.

## 3. Vulnerabilidades de dependencias (`cargo audit`) y resolución

| Advisory | Paquete/riesgo | Resolución |
|---|---|---|
| `RUSTSEC-2026-0436` | `calamine`: pánico/OOM al parsear hojas de cálculo subidas | Actualizado a `0.38` |
| `RUSTSEC-2026-0367/0368/0369/0429` | `zip` vía `rust_xlsxwriter` | `rust_xlsxwriter 0.96 → 0.102` |
| `RUSTSEC-2026-0382` | `rustls-webpki` | Actualizado a `0.103.16` |
| `RUSTSEC-2023-0071` | `rsa` | Sin corrección disponible; no está en el grafo compilado; la CI lo verifica |

Comando documentado para reproducir la auditoría:

```bash
cargo audit --ignore RUSTSEC-2023-0071
```

## 4. Pruebas dinámicas de caja negra

Sondeo Python estándar contra build release con base demo nueva, antes y después de corregir.

| Verificación | Resultado |
|---|---|
| Cabeceras de seguridad: CSP `default-src 'self'`, `frame-ancestors 'none'`, `nosniff`, `X-Frame-Options DENY`, `Referrer-Policy same-origin`, `no-store` | PASS |
| Cookie de sesión `HttpOnly` + `SameSite=Strict` | PASS |
| 107 rutas no públicas sin autenticación: ninguna devolvió 2xx | PASS |
| CSRF con token ausente o incorrecto | PASS — inicialmente el probe marcaba 422 porque el JSON se parsea antes del CSRF; no había cambio de estado; falso positivo |
| Matriz de roles: rutas solo-admin devuelven 403 para otros roles | PASS |
| Barrido IDOR con IDs 1..40 sobre recursos | PASS |
| Path traversal en `/assets` y `/api/archivo` | PASS |
| Cargas SQL injection en filtros y login | PASS |
| JSON malformado, sobredimensionado o profundo → 400/413/415/422, sin 500 | PASS |
| Fuerza bruta → 429 tras 10 intentos | PASS |
| Logout invalida sesión; sin fijación de sesión | PASS |
| Asignación masiva de `rol`/`is_superuser` | PASS |

### Ataques dirigidos poscorrección

| Ataque | Resultado |
|---|---|
| `X-Forwarded-For` suplantado no elude el bloqueo de cuenta | PASS |
| HTML o SVG disfrazado como PDF | Rechazado con 400 |
| PDF legítimo | Servido como attachment, `application/octet-stream`, `nosniff` |
| Estudiante/experto no puede descargar/ver/revisar documento de empresa | 403 |
| Recorrido Playwright de rutas de navegación para admin, empresa, estudiante y experto | 0 errores de consola |

## 5. Tabla completa de los 128 hallazgos

| N | Severidad | Área | Hallazgo (breve) | Veredicto | Nota o prueba de regresión |
|---:|---|---|---|---|---|
| 1 | Alta | Autenticación/sesiones | El autorregistro activa la cuenta e inicia sesión sin verificar el correo | Diferido | Decisión de producto; el registro de profesores ya queda inactivo/pendiente |
| 2 | Alta | Importación/roles | Importación masiva permite crear `rol=ADMIN` si el actor es admin | Corregido | `test_import_admin_requiere_superusuario` |
| 3 | Alta | Medios/finanzas | `download` y `resumen` confían en `visible_record` sin comprobación adicional | Rechazado | Falso positivo; delegación verificada con pruebas IDOR |
| 4 | Alta | Reportes | El PDF no aplica `LIMIT` SQL y materializa todo el resultado | Corregido | `test_reporte_pdf_respeta_limite_de_filas` |
| 5 | Alta | Flujos/empresas | `retos.empresa_id` se interpretaría como IDOR por comparar contra usuario | Rechazado | Falso positivo; la FK referencia `usuarios.id`; las comparaciones son correctas |
| 6 | Alta | Permisos/roles | Un admin puede asignar `ADMIN` o degradar cuentas privilegiadas | Corregido | `test_admin_no_superusuario_no_otorga_ni_degrade_admin` |
| 7 | Alta | Hackathones/eventos | `advance` puede finalizar sin fechas de presentación/premiación | Corregido | `test_no_cierra_sin_presentacion_ni_premiacion` |
| 8 | Alta | Hackathones/equipos | Eliminación de equipos sin validar hackathon finalizado | Corregido | `test_no_elimina_equipo_con_resultados_publicados` |
| 9 | Alta | Empresas/autoverificación | Admin puede verificar documentos/convenio de su propia empresa | Corregido | `test_admin_no_revisa_su_propia_empresa` |
| 10 | Alta | Unidades de estudio | Profesor puede crear unidades de programas de otra facultad | Corregido | `test_csv_rechaza_programa_de_otra_facultad` |
| 11 | Alta | Hackathones/BD | Mismo estudiante en varios equipos de un mismo reto | Corregido (esquema) | Restricción `UNIQUE(reto_id, estudiante_id)` y filtro aplicativo; `test_estudiante_no_queda_en_dos_equipos_del_mismo_reto` |
| 12 | Alta | Autenticación/BD | Correo electrónico sin unicidad insensible a mayúsculas | Corregido (esquema) | Índice único `lower(email)` y validación en registro/accounts; `test_email_registro_unico_sin_importar_mayusculas` |
| 13 | Alta | Hackathones/jurados/BD | Jurado duplicado por hackathon habilitaría múltiples votos | Rechazado | `juror()` ya valida existencia en transacción; no es brecha adicional |
| 14 | Alta | Presupuestos/BD | Presupuestos admiten `monto_total` negativo | Corregido (esquema) | `CHECK (monto_total >= 0)` |
| 15 | Alta | Gastos/BD | Gastos admiten `monto` negativo | Corregido (esquema) | `CHECK (monto >= 0)` |
| 16 | Alta | Entregables/BD | `puntaje_maximo`/`nota` sin rango válido | Corregido (esquema) | `CHECK` de máximo y rango de nota |
| 17 | Alta | Evaluaciones/BD | `nota` sin rango 0–5 | Corregido (esquema) | `CHECK (nota >= 0 AND nota <= 5)` |
| 18 | Alta | Votaciones/BD | Voto de hackathon acepta puntajes negativos | Corregido (esquema) | Validación aplicativa 0–5 y `CHECK` en BD; `test_voto_negativo_es_invalido` |
| 19 | Alta | Documentos/BD | `tipo_documento` no referencia `tipos_documento_legal` | Corregido (esquema) | FK a `tipos_documento_legal(codigo)` |
| 20 | Alta | Votaciones/BD | Jurado y equipo no validados contra el mismo hackathon/reto | Diferido | Requiere trigger entre tablas; no implementado |
| 21 | Media | Configuración/sesiones | `RUST_SECURE_COOKIE` por defecto omitía `Secure` | Corregido | Ahora por defecto `true` con advertencia; `test_cookie_de_sesion_y_logout_llevan_secure` |
| 22 | Media | Recuperación | Límite de solicitudes de reinicio sin candado atómico | Corregido | Candado consultivo y transacción |
| 23 | Media | Recuperación | Token de reinicio reutilizable por carrera | Corregido | `DELETE ... RETURNING` en la transacción; `test_reset_confirm_solo_una_vez` |
| 24 | Media | Registro | Registro público sin límite por IP/CAPTCHA | Corregido | Límite por IP de 10/10 min; `test_registro_limitado_por_ip` |
| 25 | Media | Autenticación | Los 10 fallos bloquean incluso credenciales válidas | Rechazado | Comportamiento intencional de paridad con Django; el límite por IP añade defensa |
| 26 | Media | Autenticación | Nombres de usuario con `@` colisionan con correos | Corregido | `@` rechazado en registro y alta; `test_registro_rechaza_arroba_en_usuario` |
| 27 | Media | Importación/medios | XLSX recogido completo antes de limitar filas | Corregido | Conteo temprano de filas; `test_import_xlsx_rechaza_exceso_de_filas` |
| 28 | Media | Medios/carga | Multipart usa `bytes()` antes de validar 50 MB | Corregido | Lectura por chunks; `test_adjunto_reto_rechaza_archivo_muy_grande` |
| 29 | Media | Reportes | Columnas vacías pueden provocar underflow en PDF | Corregido | Retorna 400; `test_reporte_rechaza_columnas_inexistentes` |
| 30 | Media | Reportes/export | Export sin límite visible de filas | Corregido | `EXPORT_LIMIT` y `resources::rows`; `test_reporte_pdf_respeta_limite_de_filas` |
| 31 | Media | Dashboard | Dashboard de profesor no filtra integraciones aprobadas/publicadas | Corregido | `test_dashboard_profesor_solo_integraciones_aprobadas` |
| 32 | Media | Flujos/revisión | Admin puede aprobar reto de su propia empresa | Corregido | `test_admin_no_aprueba_reto_de_su_propia_empresa` |
| 33 | Media | Flujos/postulaciones | `mark_applications_in_review` salta la máquina de estados | Corregido | `test_postulaciones_de_reto_finalizado_no_pasan_a_revision` |
| 34 | Media | Correo | SMTP con TLS desactivado y credenciales envía contraseñas en texto plano | Corregido | Fallback a consola |
| 35 | Media | Correo | Envío SMTP mantiene transacción abierta | Corregido | Reclama filas, envía fuera de transacción y luego marca resultado |
| 36 | Media | Hackathones/inscripción | Inscripción sin postulación aceptada | Corregido | `test_inscripcion_exige_postulacion_aceptada` |
| 37 | Media | Hackathones/votación | `ON CONFLICT` de voto sin alcance por hackathon | Rechazado | Falso positivo: `(jurado_id, equipo_id)` ya equivale a un único hackathon |
| 38 | Media | Hackathones/estado | Publicar resultados sin validar cierre de etapas | Rechazado | Publicación manual autorizada; el flujo automático quedó cubierto por N7 |
| 39 | Media | Hackathones/ventana | Sin etapa de inscripción la ventana queda abierta | Rechazado | Comportamiento documentado y cubierto por pruebas de paridad |
| 40 | Media | Finanzas/presupuesto | Presupuesto puede bajarse por debajo de lo comprometido/ejecutado | Corregido | `test_no_se_reduce_el_presupuesto_por_debajo_de_lo_asignado` |
| 41 | Media | Calificación | Recalificación ignora nota ya publicada | Corregido | `test_no_se_recalifica_una_nota_ya_publicada` |
| 42 | Media | Académico | Edición de integración expone retos eliminados | Corregido | `test_editar_integracion_reto_eliminado` |
| 43 | Media | Académico | Integración editable tras aprobación/publicación | Corregido | `test_integracion_aprobada_no_editable` |
| 44 | Media | Ciclo de vida | Avance permitido en retos finalizados/cancelados | Corregido | `test_avance_bloqueado_en_reto_finalizado` |
| 45 | Media | Cierre | Acta/agenda editable tras finalización | Corregido | `test_cierre_guardar_bloqueado_despues_de_finalizar` |
| 46 | Media | Recursos | `advance_before_read` ocurre antes de autorizar | Corregido | `test_detalle_no_avanza_sin_autorizacion` |
| 47 | Media | Certificados | Certificados sin exigir postulación aceptada | Rechazado | Revertido por revisor principal: equipos académicos no postulan |
| 48 | Media | Equipos/BD | Duplicidad de nombres de equipo por reto | Corregido (esquema) | `UNIQUE(reto_id, nombre)` |
| 49 | Media | Equipos académicos/BD | Duplicidad de nombres de equipo académico | Corregido (esquema) | `UNIQUE(reto_id, nombre_equipo)` |
| 50 | Media | Rúbricas/BD | `puntaje_maximo` negativo en criterios de rúbrica | Corregido (esquema) | `CHECK (puntaje_maximo >= 0)` |
| 51 | Media | Evaluaciones/BD | Nota de criterio sin validación cruzada contra máximo | Diferido | Requiere validación entre tablas; no implementado |
| 52 | Media | Encuesta/BD | Calificación de satisfacción sin techo | Corregido (esquema) | `CHECK` 0–5 |
| 53 | Media | Seguimientos/BD | Porcentaje de avance puede superar 100 | Corregido (esquema) | `CHECK (porcentaje_avance <= 100)` |
| 54 | Media | Retos/BD | Fechas del reto sin orden cronológico | Corregido (esquema) | `CHECK` de orden de fechas |
| 55 | Media | Etapas/BD | Etapa puede terminar antes de iniciar | Corregido (esquema) | `CHECK` de orden de fechas |
| 56 | Media | Documentos/BD | `fecha_vencimiento` anterior a expedición | Corregido (esquema) | `CHECK`; pruebas de documentos vencidos ajustan `fecha_expedicion` |
| 57 | Media | Estados/BD | Columnas de estado sin restricción de dominio | Corregido (esquema) | Restricciones de estado por columna |
| 58 | Media | Inscripciones hackathon/BD | Equipo no validado contra el reto del hackathon | Diferido | Trigger entre tablas no implementado |
| 59 | Media | Entregables/BD | `entregables.reto_id` puede diferir del reto del equipo | Diferido | Trigger entre tablas no implementado |
| 60 | Media | Integridad de roles/BD | FKs a usuarios en vez de perfiles específicos | Diferido | Migración de FKs a tablas de perfil no implementada |
| 61 | Media | Equipos académicos/BD | `integracion_id` nullable | Corregido (esquema) | Backfill y `NOT NULL` |
| 62 | Media | Auditoría/BD | Borrar usuario falla o pierde auditoría | Corregido (esquema) | `ON DELETE SET NULL` |
| 63 | Media | Votaciones/BD | Unicidad de voto sin alcance por etapa | Corregido (esquema) | Índice con alcance por etapa |
| 64 | Media | Frontend | `href` con `javascript:`/`data:` desde enlace de transmisión | Corregido | `safeHref()`; smoke-G |
| 65 | Media | Frontend | Condición de carrera tras `await` con `selected` global | Corregido | Captura de parámetros y token de secuencia |
| 66 | Media | Frontend | Diálogo abierto atrapa al usuario tras 401 | Corregido | Cierre de diálogo en login/ruta |
| 67 | Media | Frontend | Upload de unidades con `fetch` sin manejo 401/CSRF | Corregido | `apiForm()` |
| 68 | Media | Frontend | Fallos de opciones de registro invisibles | Corregido | Error visible y retry |
| 69 | Media | Frontend | Falla de llamadas secundarias deja diálogo parcial | Corregido | `detail_loadError()` |
| 70 | Media | Frontend accesibilidad | Campana sin Escape/click externo/foco | Corregido | Menú accesible |
| 71 | Media | Frontend accesibilidad | Sidebar `aria-expanded` inconsistente | Corregido | Sincronización ARIA y foco |
| 72 | Media | Frontend | Loader de ruta puede rechazar sin manejo | Corregido | `try/catch` en `route()` |
| 73 | Media | Frontend | Submit de permisos sin captura de error | Corregido | Captura y toast |
| 74 | Media | CI | Actions con tags mutables | Corregido | Pinned a SHA |
| 75 | Media | CI | Workflow sin `permissions` | Corregido | `permissions: contents: read` |
| 76 | Baja | Autenticación | Logout no incluye `Secure` | Corregido | `test_cookie_de_sesion_y_logout_llevan_secure` |
| 77 | Baja | Autenticación | Login CSRF | Diferido | Requiere doble token en SPA; no se implementó para no romper frontend |
| 78 | Baja | Autenticación | Límite solo por cuenta, no por IP | Corregido | Pre-check por IP: >100 fallos/10 min → 429 |
| 79 | Baja | Recuperación | Filas viejas de intentos sin limpiar en reset | Corregido | Purga en ruta de reset |
| 80 | Baja | Autenticación | Login de cuenta inactiva filtra motivo | Corregido | `test_login_cuenta_inactiva_respuesta_generica` |
| 81 | Baja | Registro | Respuestas distintas permiten enumeración | Corregido | Respuesta uniforme |
| 82 | Baja | Recuperación | Token permite cambiar contraseña de cuenta inactiva | Corregido | `test_reset_confirm_invalido_para_cuenta_inactiva` |
| 83 | Baja | Permisos | Convenio sin exigir rol `EMPRESA` | Corregido | `test_solo_empresa_elige_modo_convenio` |
| 84 | Baja | Recuperación | Límite de reset solo por correo electrónico | Corregido | Límite por IP |
| 85 | Baja | Auditoría | `X-Forwarded-For` confiable sin validar proxy | Corregido | Solo se usa si `RUST_TRUST_PROXY=true` |
| 86 | Baja | Registro | Unicidad de usuario/correo/NIT con carrera | Corregido | Candados y comprobación en transacción; índice en esquema |
| 87 | Baja | Medios | Archivos huérfanos si falla inserción de BD | Corregido | Limpieza ante cualquier error |
| 88 | Baja | Reportes | Catálogo expone empresas/facultades de más | Corregido | Catálogo limitado por alcance |
| 89 | Baja | Reportes | Ejecutado suma estados anulados/rechazados | Corregido | `test_reporte_presupuesto_ejecutado_solo_pagado` |
| 90 | Baja | Reportes | `LIMIT` dinámico con `format!` | Corregido | Parámetro bind |
| 91 | Baja | Flujos | Deduplicación de notificaciones al ciclar estado | Corregido | Clave incluye ID de auditoría |
| 92 | Baja | Recordatorios | Días sin validar pueden generar envío masivo | Corregido | Días descartados y barrido limitado |
| 93 | Baja | Correo | `expect`/`unwrap` en `Mailer::Memory` | Diferido | Sin evidencia de cambio en el reporte del paquete; pendiente |
| 94 | Baja | Hackathones | Logística sin bloqueo ni validación de estado | Corregido | `test_logistica_rechazada_en_hackathon_cerrado` |
| 95 | Baja | Finanzas | Gasto permitido en retos cancelados/borrador | Corregido | `test_reto_cancelado_no_admite_gastos_ni_ejecucion` |
| 96 | Baja | Finanzas | Export Excel convierte `Decimal` a `f64` | Diferido | Cosmético; cambiar a cadenas rompería prueba de paridad actual |
| 97 | Baja | Calificación | Rúbrica mayor que escala impide nota completa | Corregido | `test_rubrica_que_supera_la_escala_se_rechaza` |
| 98 | Baja | Finanzas | Gasto con borrado físico sin auditoría | Diferido | Requiere columna/ledger; no implementado |
| 99 | Baja | Ciclo de vida | Permiso de profesor no revalidado tras bloqueo | Corregido | Revalidación dentro de transacción |
| 100 | Baja | Cierre | Acciones de cierre permitidas en retos cancelados | Corregido | Estados permitidos explícitos |
| 101 | Baja | Unidades de estudio | Unidades inactivas listadas/editable indebidamente | Corregido | Decisión revisor: admin y profesores pueden listar `activo=0`; estudiantes solo activas |
| 102 | Baja | Unidades de estudio | Código de unidad con concurrencia | Corregido (esquema) | Candado consultivo y restricción única |
| 103 | Baja | Recursos | Enlaces con esquemas peligrosos | Corregido | Sanitización de esquemas en salida |
| 104 | Baja | Cierre | Archivo de cierre sin lista explícita de extensiones | Corregido | Solo PDF/PNG/JPG; `test_cierre_entregable_rechaza_txt` |
| 105 | Baja | Cierre | Comentario logístico sin `max_chars` | Corregido | `test_estado_solicitud_comentario_demasiado_largo` |
| 106 | Baja | Recursos | `export` con `unwrap()` sobre metadatos | Corregido | Error controlado de aplicación |
| 107 | Baja | Frontend accesibilidad | Tablist solo click, sin navegación por teclado | Corregido | Flechas, Home/End, tabindex móvil |
| 108 | Baja | Frontend | `field.choices` asume array | Rechazado | Falso positivo: 409/409 campos con `choices`; sin evidencia |
| 109 | Baja | Frontend | `tipo` de notificación puede resolver `constructor` | Corregido | Prototipo nulo |
| 110 | Baja | Frontend | Controles de log sin estado busy | Corregido | Controles deshabilitados durante carga |
| 111 | Baja | Frontend accesibilidad | Encabezados de nav `aria-hidden` | Corregido | `aria-hidden` retirado |
| 112 | Baja | Frontend | Subida de importación sin límite de tamaño local | Corregido | Validación local de 5 MB |
| 113 | Baja | Frontend | Inputs de archivo sin `accept` ni validación de tipo | Corregido | `accept` y validación de extensión/tipo |
| 114 | Baja | Frontend | Inputs de evaluación sin `max` | Corregido | `max` asignado desde metadatos |
| 115 | Baja | Frontend | Anchor de descarga no adjuntado al DOM | Corregido | Se agrega, click y remueve |
| 116 | Baja | Frontend | Upload exitoso muestra error si falla refresco | Corregido | Separación de errores de subida y refresco |
| 117 | Baja | CI/Contenedores | Tags de imágenes mutables en Docker/CI | Corregido | Imágenes fijadas a digest |
| 118 | Baja | Contenedores | Servicio web sin restart/healthcheck | Corregido | Restart y healthcheck |
| 119 | Baja | Contenedores | Clave de BD como variable de entorno visible | Diferido | Requiere Docker secrets/gestor de secretos |
| 120 | Baja | Contenedores | Contenedor sin endurecimiento ni límites | Corregido | `cap_drop`, `read_only`, `security_opt`, límites |
| 121 | Baja | CI | Supresión indefinida de `cargo audit` | Corregido | Verificación condicionada del `rsa` en CI |
| 122 | Baja | Configuración | `.env.example` sin `RUST_POSTGRES_PASSWORD` | Corregido | Documentada/agregada |
| 123 | Baja | Servidor | Apagado no espera `SIGTERM` | Corregido | `SIGTERM` incluido |
| 124 | Baja | Migraciones/BD | Migraciones concurrentes sin candado previo | Corregido (esquema) | Candado consultivo previo |
| 125 | Baja | Cargo/configuración | PBKDF2 no verificado y `tokio/full` amplio | Diferido | PBKDF2-SHA256 con 1,000,000 iteraciones ya es aceptable; recorte de `tokio` pendiente |
| 126 | Baja | Migración | `import_django` usa `fetch_all` para tablas completas | Diferido | Debe migrar a COPY/cursor |
| 127 | Baja | Índices/BD | Faltan índices en filtros calientes | Corregido (esquema) | Índices compuestos agregados |
| 128 | Baja | Notificaciones/BD | Inconsistencia `leida`/`leida_en` | Rechazado | Revertido por revisor principal; datos legacy y bajo valor |

## 6. Decisiones del revisor principal

Las siguientes decisiones de `lead-notes.md` son autoritativas:

1. **Dependencias:** `calamine`, `rust_xlsxwriter`/`zip` y `rustls-webpki` actualizados. `rsa` permanece sin fix y fuera del grafo compilado; la CI lo verifica.
2. **Hallazgo #47 rechazado y revertido:** exigir postulación aceptada para certificados rompe el flujo de equipos académicos, que no postulan.
3. **Hallazgo #101 matizado:** administradores **y** profesores gestores de unidad pueden listar unidades inactivas con `activo=0`; los estudiantes solo ven activas.
4. **Hallazgo #128 rechazado:** la restricción de consistencia `leida`/`leida_en` se retiró por riesgo de importación y bajo valor; existen filas legacy/test.
5. **`CHECK` de orden de fechas de documentos se mantiene:** las pruebas que simulaban documentos vencidos ahora respaldan `fecha_expedicion`.
6. **Hallazgo #78 reforzado:** se añadió un pre-check por IP: más de 100 fallos/10 min devuelve 429 antes de verificar contraseña.
7. **Corrección de compilación:** `Option<ConnectInfo<SocketAddr>>` no es extractor válido en axum 0.8; se usa `Option<Extension<ConnectInfo<SocketAddr>>>`.
8. **Hallazgos #74/#117:** acciones de GitHub fijadas a SHA e imágenes Docker fijadas a digests.
9. **Riesgo residual de correo:** la recuperación por correo usa `creada_en` con centinela `ENVIANDO`; con múltiples instancias una notificación antigua podría reenviarse. El despliegue documentado es de instancia única.
10. **Verificación dinámica final:** todos los controles de caja negra PASS; recorrido Playwright sin errores de consola.
11. **Cierre de calidad:** 447 pruebas pasan; `fmt`, `clippy -D warnings` y `node --check` limpios.

## 7. Riesgos residuales y trabajo diferido

| Ítem | Justificación breve |
|---|---|
| Verificación de correo al registrarse (#1) | Requiere token de confirmación y decisión de producto; no es cambio trivial |
| Triggers entre tablas (#20, #51, #58, #59, #60) | Validación de integridad entre filas/tablas; mitigación en capa de aplicación en algunos casos |
| Login CSRF (#77) | Requiere token previo o doble submit en SPA; cambio backend+frontend |
| Pánicos en `mail.rs` (#93) | `expect`/`unwrap` en `Mailer::Memory`; pendiente de convertir a errores controlados |
| Decimal en Excel (#96) | La conversión a `f64` puede generar artefactos; requiere cadenas decimales y actualizar pruebas |
| Borrado lógico de gastos (#98) | Requiere esquema/ledger nuevo; no se realizó |
| Secretos Docker (#119) | Clave de BD visible por inspección/proc; migrar a Docker secrets o gestor |
| PBKDF2/tokio (#125) | PBKDF2 actual es aceptable; queda el recorte de `tokio/full` |
| `import_django` con `fetch_all` (#126) | Tablas grandes pueden agotar RAM; migrar a COPY/cursor |
| Envío de correo multi-instancia | Centinela `ENVIANDO` basado en `creada_en`; despliegue documentado de instancia única |
| `rsa` RUSTSEC-2023-0071 | Sin corrección; no está en el grafo compilado; la CI lo verifica |

## 8. Cómo reproducir

Ejecutar desde `rust/`:

```bash
# Suite completa, incluidos tests ignorados
cargo test --locked --no-fail-fast -- --include-ignored

# Lint estricto
cargo clippy --all-targets -- -D warnings

# Auditoría de dependencias
cargo audit --ignore RUSTSEC-2023-0071

# Verificación sintáctica del frontend
node --check rust/web/js/*.js
```

Archivos nuevos de regresión:

- `tests/qa_auth.rs`
- `tests/qa_data.rs`
- `tests/qa_hackathon.rs`
- `tests/qa_business.rs`
- `tests/qa_academic.rs`
- `tests/qa_schema.rs`

Los archivos de referencia para futuros auditores son:

- `lead-notes.md` — decisiones autoritativas del revisor principal.
- `00-consolidated.md` — matriz original de los 128 hallazgos.
- `report-*.md` — reportes por paquete con veredictos y evidencia.