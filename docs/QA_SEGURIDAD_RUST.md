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
- Corregidos: **119** (incluye **23** corregidos en esquema)
- Rechazados: **9**
- Diferidos: **0**
- Pruebas automáticas: **370 → 484** pruebas que pasan
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

## 5. Ronda 2: cierre de diferidos y pruebas adversariales

La Ronda 2, autoritativa en `round2-notes.md`, cierra los 13 hallazgos que permanecían diferidos en el commit `194826f30`.

- **Verificación de correo (#1):** el autorregistro de ESTUDIANTE/EMPRESA crea `correo_verificado=false` (migración 0006; cuentas existentes por defecto `true`). No se inicia sesión al registrar; el enlace de verificación es de un solo uso (`POST /api/registro/verificar`). El login rechaza cuentas sin verificar con «Confirma tu correo antes de iniciar sesión» solo tras contraseña correcta, para no permitir enumeración.
- **CSRF y origen (#77):** el middleware `same_origin` en `lib.rs` rechaza con 403 antes del handler cualquier escritura POST/PUT/PATCH/DELETE con `Origin` de host distinto, `Origin: null` o `Sec-Fetch-Site` cross-site/same-site. Cubre login CSRF y aporta defensa en profundidad para todas las escrituras.
- **Integridad cruzada (#20, #51, #58, #59):** triggers PL/pgSQL en la migración `0005_integridad_cruzada`, reflejados en `schema.sql` y relajados/restaurados por `import-django`. Las violaciones usan `SQLSTATE 23514` y se exponen como HTTP 400.
- **Integridad de roles (#60, acotado):** triggers de rol sobre `estudiantes`, `profesores`, `postulaciones.estudiante_id`, `entregables.estudiante_id` y otras columnas estrictamente de estudiante/profesor. Las columnas de rol mixto (jurados, expertos invitados, certificados, autores de comentarios) están documentadas y excluidas. El trigger solo revisa referencias nuevas o modificadas, por lo que un cambio de rol posterior no bloquea filas existentes.
- **Gastos (#98):** borrado lógico con `gastos.eliminado_en` y auditoría en `historial_estados`; todas las lecturas de gastos excluyen filas eliminadas.
- **Decimales en Excel (#96):** las celdas monetarias XLSX se redondean a dos decimales con formato `"#,##0.00"`, manteniéndose numéricas.
- **Correo robusto (#93):** `mail.rs` sin `expect`/`unwrap`; el fallo de catálogo se registra y degrada; el mutex envenenado se recupera.
- **Importación por cursor (#126):** `import-django` copia con cursor server-side en lotes de 500, con memoria acotada.
- **Secreto de BD (#119):** `DB_PASSWORD_FILE` (Docker secret) se lee en `config.rs`; `compose.yml` pasa el secreto a ambos contenedores mediante `POSTGRES_PASSWORD_FILE` / `DB_PASSWORD_FILE` y sigue rechazando arrancar sin `RUST_POSTGRES_PASSWORD`.
- **Tokio/PBKDF2 (#125):** features de `tokio` reducidas de `full` a `rt-multi-thread, macros, net, fs, io-util, signal, sync, time`; PBKDF2-SHA256 con 1,000,000 iteraciones confirmado.

### Suite adversarial y verificación en vivo

- `tests/qa_ataques.rs` agrega 8 pruebas: barrido CSRF sobre las 68 rutas protegidas de escritura (token ausente o incorrecto nunca devuelve 2xx y los checksums de tablas no cambian; el parser exige al menos 65 rutas), endpoints públicos rechazados en cross-site y carreras de concurrencia con 20 peticiones paralelas.
- Las carreras cubren: límite de login evaluado exactamente en 10, token de reset consumido una sola vez, votos concurrentes de un jurado producen una fila, postulaciones concurrentes una fila, gastos concurrentes nunca superan el presupuesto y aprobaciones concurrentes una fila de historial.
- Verificación de la ronda 2: **478** pruebas pasaban (0 fallos) al cierre de esa ronda (482 tras la revisión independiente, ver 5b); `cargo fmt --check`, `clippy --all-targets -D warnings`, `node --check` sobre todo el JS y `cargo audit` limpios, salvo `rsa` RUSTSEC-2023-0071, que no se compila.
- Release build en vivo de `194826f30` con BD demo nueva: comprobaciones de Origin/CSRF/verificación descritas arriba; recorrido Playwright por el formulario real de login para admin, empresa, profesor, estudiante, experto y un estudiante recién verificado. Cada ruta de navegación (51/11/10/10/3/10) cargó sin banners de error; solo se observó el 401 esperado en `/api/me` antes del login.

## 5b. Revisión independiente y verificación final

Para no depender del veredicto de un solo agente, se repitió la verificación con modelos distintos sobre el código final:

- **Hallazgos rechazados re-verificados:** Qwen 3.8 Max (#3, #13, #37) y DeepSeek V4 Pro (#5, #25, #38, #47) revisaron de nuevo cada rechazo con evidencia del código. Coincidieron en todos salvo #25, que se revocó y se corrigió (bloqueo por cuenta e IP + tope global). #108 y #128 se rechazaron con evidencia directa (409/409 campos con `choices`; datos legados sin marca de lectura).
- **Revisión del código de la ronda 2** (dos agentes de la ronda 2 terminaron antes de tiempo), por DeepSeek V4 Pro y Qwen 3.8 Max. Hallazgos nuevos corregidos:
  - enlaces `#verificar/<token>` redactados en los logs de correo (como los de recuperación);
  - `same_origin` solo confía en `X-Forwarded-Host` con `RUST_TRUST_PROXY=true`;
  - bloqueo consultivo también por correo en el registro (carrera de correos duplicados);
  - recuperación de reclamos del buzón robusta ante valores malformados (`CASE`) y reclamos con marca de tiempo (`ENVIANDO:<epoch>`), de modo que varias instancias nunca reenvían (`qa_ataques::test_dos_buzones_concurrentes_envian_una_vez`);
  - listas restrictivas evaluadas antes que el estado de la cuenta en el login;
  - el alcance de profesores en hackathones/equipos excluye retos eliminados (`qa_ataques::test_profesor_no_gestiona_equipos_de_reto_eliminado`).
  - Se revisaron y se mantienen por diseño: `same-site` tratado como ajeno (protege contra subdominios), peticiones sin `Origin` ni `Sec-Fetch-Site` permitidas (clientes no navegador; siguen necesitando el token CSRF), y triggers de rol que solo validan referencias nuevas o cambiadas.
- **Ataque en vivo completo** sobre el binario release del commit final con BD demo nueva (script Python independiente que lee la tabla de rutas de `lib.rs`): 15/15 PASS — cabeceras; cookie; 97 rutas no públicas sin sesión; CSRF y origen cruzado en las 72 rutas de escritura sin cambios de datos; CSRF de login; matriz de roles; IDOR en 10 tablas para 4 roles; path traversal; inyección; cuerpos malformados; sesión y fijación; asignación masiva; flujo de verificación de correo; carreras de login (exactamente 10 evaluados); fuerza bruta aislada por cuenta+IP. Carrera de 20 postulaciones simultáneas: una sola fila en la BD (el endpoint actualiza la postulación pendiente).
- **Navegador (Playwright)** sobre el mismo binario, iniciando sesión por el formulario real (ejercita el control de origen): admin 51 rutas/18 detalles, empresa 11/6, profesor 10/3, estudiante 10/4, experto 3/0; sin banners de error; solo el 401 esperado de `/api/me` antes del login.
- **Cierre:** 483 pruebas pasan en una ejecución completa del árbol final (482 antes de la prueba de auditoría de gastos); `cargo fmt --check`, `clippy --all-targets -D warnings`, `node --check` limpios.

### Cierre de pendientes de la revisión

- **Rechazos #108 y #128, re-verificados con datos:** `schema.json` tiene 409 campos y los 409 traen `choices` como arreglo (ninguno sin la clave), así que `field.choices.length` no puede fallar (#108). La única escritura que marca una notificación como leída fija `leida_en` en la misma sentencia (`workflows.rs`) y las inserciones nacen sin leer, por lo que la aplicación no crea filas inconsistentes; solo pueden venir de datos legados y ningún código decide nada con `leida_en` (#128).
- **Puntos restantes de los revisores, resueltos uno a uno:**
  - Triggers con `NULL`: `puntaje` y `criterio_id` son `NOT NULL` y `criterio_id` tiene FK, así que el caso no puede ocurrir.
  - Tokens de verificación: se guardan como hash SHA-256, caducan a los 3 días y se consumen con `DELETE … RETURNING` (un solo uso).
  - Actualizaciones de gastos sin predicado de borrado: `ejecutar` y `eliminar` bloquean antes la fila con `eliminado_en IS NULL FOR UPDATE`, así que un gasto eliminado no se puede tocar.
  - Auditoría de gastos: **corregido**, ejecutar un gasto ahora deja la transición `COMPROMETIDO → EJECUTADO` en `historial_estados` (`qa_integridad::qa_ejecutar_gasto_queda_en_historial`).
  - Postulaciones concurrentes: además de la prueba de carrera, la BD tiene `UNIQUE (reto_id, estudiante_id)`, así que una segunda fila es imposible a cualquier concurrencia; en vivo, 20 peticiones simultáneas dejaron una sola fila (el endpoint actualiza la postulación pendiente).
- **Flujo de verificación de correo en el navegador, de punta a punta** (binario release final): registro por el formulario real → mensaje "Revisa tu correo" y sin cookie de sesión; login → "Confirma tu correo antes de iniciar sesión"; enlace falso → "Enlace no válido"; enlace válido → "Cuenta verificada"; reutilizarlo → "Enlace no válido"; login posterior correcto.
  - Este recorrido encontró un **error real, corregido**: el arranque de la SPA enrutaba dos veces y enviaba el token dos veces; la primera llamada verificaba la cuenta y la segunda reemplazaba el éxito por "Enlace no válido". Ahora cada token se envía una sola vez por carga (comprobado: una llamada 200 y "Cuenta verificada").

- **Flujo de escritura del profesor en el navegador** (binario release): el profesor creó una unidad de estudio desde el formulario real (POST 200 y visible en la lista). Este recorrido encontró y corrigió dos errores de interfaz: el formulario de unidad exigía un PDF aunque el servidor lo trata como opcional (una edición obligaba a volver a subirlo), y la SPA ofrecía gestionar unidades a cualquier profesor aunque el servidor exige facultad asignada; ahora `/api/me` expone `gestiona_unidades` y la interfaz lo respeta (`qa_academic::test_me_indica_si_gestiona_unidades`).

## 6. Tabla completa de los 128 hallazgos

| N | Severidad | Área | Hallazgo (breve) | Veredicto | Nota o prueba de regresión |
|---:|---|---|---|---|---|
| 1 | Alta | Autenticación/sesiones | El autorregistro activa la cuenta e inicia sesión sin verificar el correo | Corregido | El autorregistro de ESTUDIANTE/EMPRESA crea `correo_verificado=false` (migración 0006; cuentas existentes por defecto `true`); no inicia sesión; enlace de verificación de un solo uso (`POST /api/registro/verificar`); el login rechaza cuentas no verificadas solo tras contraseña correcta; tests/qa_verificacion.rs (6) y verificación en vivo |
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
| 20 | Alta | Votaciones/BD | Jurado y equipo no validados contra el mismo hackathon/reto | Corregido | Trigger PL/pgSQL de integridad cruzada en migración `0005_integridad_cruzada`; SQLSTATE 23514 → HTTP 400; test en `tests/qa_integridad.rs` |
| 21 | Media | Configuración/sesiones | `RUST_SECURE_COOKIE` por defecto omitía `Secure` | Corregido | Ahora por defecto `true` con advertencia; `test_cookie_de_sesion_y_logout_llevan_secure` |
| 22 | Media | Recuperación | Límite de solicitudes de reinicio sin candado atómico | Corregido | Candado consultivo y transacción |
| 23 | Media | Recuperación | Token de reinicio reutilizable por carrera | Corregido | `DELETE ... RETURNING` en la transacción; `test_reset_confirm_solo_una_vez` |
| 24 | Media | Registro | Registro público sin límite por IP/CAPTCHA | Corregido | Límite por IP de 10/10 min; `test_registro_limitado_por_ip` |
| 25 | Media | Autenticación | Los 10 fallos bloquean incluso credenciales válidas | Corregido | Revocado el rechazo tras la segunda opinión: el bloqueo de 10 fallos es por cuenta e IP y hay un tope global de 50 por cuenta; `qa_auth::test_bloqueo_por_cuenta_e_ip` |
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
| 51 | Media | Evaluaciones/BD | Nota de criterio sin validación cruzada contra máximo | Corregido | Trigger PL/pgSQL de integridad cruzada valida la nota contra el `puntaje_maximo`; migración `0005_integridad_cruzada`; test en `tests/qa_integridad.rs` |
| 52 | Media | Encuesta/BD | Calificación de satisfacción sin techo | Corregido (esquema) | `CHECK` 0–5 |
| 53 | Media | Seguimientos/BD | Porcentaje de avance puede superar 100 | Corregido (esquema) | `CHECK (porcentaje_avance <= 100)` |
| 54 | Media | Retos/BD | Fechas del reto sin orden cronológico | Corregido (esquema) | `CHECK` de orden de fechas |
| 55 | Media | Etapas/BD | Etapa puede terminar antes de iniciar | Corregido (esquema) | `CHECK` de orden de fechas |
| 56 | Media | Documentos/BD | `fecha_vencimiento` anterior a expedición | Corregido (esquema) | `CHECK`; pruebas de documentos vencidos ajustan `fecha_expedicion` |
| 57 | Media | Estados/BD | Columnas de estado sin restricción de dominio | Corregido (esquema) | Restricciones de estado por columna |
| 58 | Media | Inscripciones hackathon/BD | Equipo no validado contra el reto del hackathon | Corregido | Trigger PL/pgSQL de integridad cruzada valida el equipo contra el reto del hackathon; migración `0005_integridad_cruzada`; test en `tests/qa_integridad.rs` |
| 59 | Media | Entregables/BD | `entregables.reto_id` puede diferir del reto del equipo | Corregido | Trigger PL/pgSQL de integridad cruzada valida `entregables.reto_id` contra el reto del equipo; migración `0005_integridad_cruzada`; test en `tests/qa_integridad.rs` |
| 60 | Media | Integridad de roles/BD | FKs a usuarios en vez de perfiles específicos | Corregido | Corregido de forma acotada: triggers de rol en `estudiantes`, `profesores`, `postulaciones.estudiante_id`, `entregables.estudiante_id` y otras columnas estrictamente de estudiante/profesor; columnas de rol mixto (jurados, expertos invitados, certificados, autores de comentarios) documentadas y excluidas; solo valida referencias nuevas o modificadas; test `qa_cambio_de_rol_no_bloquea_filas_existentes` |
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
| 77 | Baja | Autenticación | Login CSRF | Corregido | Middleware `same_origin` en `lib.rs` rechaza con 403 escrituras POST/PUT/PATCH/DELETE con `Origin` de host distinto, `Origin: null` o `Sec-Fetch-Site` cross-site/same-site; cubre login CSRF y refuerza todas las escrituras; tests/qa_origen.rs (4) y qa_ataques |
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
| 93 | Baja | Correo | `expect`/`unwrap` en `Mailer::Memory` | Corregido | `mail.rs` sin `expect`/`unwrap`: el fallo de catálogo se registra y degrada; mutex envenenado recuperado; tests/qa_robustez.rs |
| 94 | Baja | Hackathones | Logística sin bloqueo ni validación de estado | Corregido | `test_logistica_rechazada_en_hackathon_cerrado` |
| 95 | Baja | Finanzas | Gasto permitido en retos cancelados/borrador | Corregido | `test_reto_cancelado_no_admite_gastos_ni_ejecucion` |
| 96 | Baja | Finanzas | Export Excel convierte `Decimal` a `f64` | Corregido | Celdas monetarias XLSX redondeadas a 2 decimales con formato `"#,##0.00"`; se mantienen numéricas |
| 97 | Baja | Calificación | Rúbrica mayor que escala impide nota completa | Corregido | `test_rubrica_que_supera_la_escala_se_rechaza` |
| 98 | Baja | Finanzas | Gasto con borrado físico sin auditoría | Corregido | Borrado lógico `gastos.eliminado_en` con `historial_estados` de auditoría; todas las lecturas de gastos excluyen filas eliminadas |
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
| 119 | Baja | Contenedores | Clave de BD como variable de entorno visible | Corregido | `DB_PASSWORD_FILE` (Docker secret) leído por `config.rs`; `compose.yml` pasa la contraseña como `db_password` a ambos contenedores (`POSTGRES_PASSWORD_FILE` / `DB_PASSWORD_FILE`) y sigue exigiendo `RUST_POSTGRES_PASSWORD`; test unitario `config::tests` |
| 120 | Baja | Contenedores | Contenedor sin endurecimiento ni límites | Corregido | `cap_drop`, `read_only`, `security_opt`, límites |
| 121 | Baja | CI | Supresión indefinida de `cargo audit` | Corregido | Verificación condicionada del `rsa` en CI |
| 122 | Baja | Configuración | `.env.example` sin `RUST_POSTGRES_PASSWORD` | Corregido | Documentada/agregada |
| 123 | Baja | Servidor | Apagado no espera `SIGTERM` | Corregido | `SIGTERM` incluido |
| 124 | Baja | Migraciones/BD | Migraciones concurrentes sin candado previo | Corregido (esquema) | Candado consultivo previo |
| 125 | Baja | Cargo/configuración | PBKDF2 no verificado y `tokio/full` amplio | Corregido | PBKDF2-SHA256 con 1,000,000 iteraciones confirmado; features de `tokio` reducidas a `rt-multi-thread, macros, net, fs, io-util, signal, sync, time` |
| 126 | Baja | Migración | `import_django` usa `fetch_all` para tablas completas | Corregido | `import-django` copia mediante cursor server-side en lotes de 500; test en `tests/qa_robustez.rs` |
| 127 | Baja | Índices/BD | Faltan índices en filtros calientes | Corregido (esquema) | Índices compuestos agregados |
| 128 | Baja | Notificaciones/BD | Inconsistencia `leida`/`leida_en` | Rechazado | Revertido por revisor principal; datos legacy y bajo valor |

## 7. Decisiones del revisor principal

Las siguientes decisiones de `lead-notes.md` son autoritativas:

1. **Dependencias:** `calamine`, `rust_xlsxwriter`/`zip` y `rustls-webpki` actualizados. `rsa` permanece sin fix y fuera del grafo compilado; la CI lo verifica.
2. **Hallazgo #47 rechazado y revertido:** exigir postulación aceptada para certificados rompe el flujo de equipos académicos, que no postulan.
3. **Hallazgo #101 matizado:** administradores **y** profesores gestores de unidad pueden listar unidades inactivas con `activo=0`; los estudiantes solo ven activas.
4. **Hallazgo #128 rechazado:** la restricción de consistencia `leida`/`leida_en` se retiró por riesgo de importación y bajo valor; existen filas legacy/test.
5. **`CHECK` de orden de fechas de documentos se mantiene:** las pruebas que simulaban documentos vencidos ahora respaldan `fecha_expedicion`.
6. **Hallazgo #78 reforzado:** se añadió un pre-check por IP: más de 100 fallos/10 min devuelve 429 antes de verificar contraseña.
7. **Corrección de compilación:** `Option<ConnectInfo<SocketAddr>>` no es extractor válido en axum 0.8; se usa `Option<Extension<ConnectInfo<SocketAddr>>>`.
8. **Hallazgos #74/#117:** acciones de GitHub fijadas a SHA e imágenes Docker fijadas a digests.
9. **Correo multi-instancia (cerrado):** los reclamos del buzón llevan marca de tiempo (`ENVIANDO:<epoch>`) y solo se recuperan los de más de 10 minutos, por lo que varias instancias nunca reenvían una notificación en curso (prueba `qa_ataques::test_dos_buzones_concurrentes_envian_una_vez`).
10. **Verificación dinámica final:** todos los controles de caja negra PASS; recorrido Playwright sin errores de consola.
11. **Cierre de calidad:** 484 pruebas pasan; `fmt`, `clippy -D warnings` y `node --check` limpios.

## 8. Riesgos residuales y trabajo diferido

| Ítem | Justificación breve |
|---|---|
| `rsa` RUSTSEC-2023-0071 | Sin corrección upstream. Solo figura en `Cargo.lock` porque `sqlx` declara su controlador MySQL como dependencia opcional (Cargo registra las opcionales); `cargo tree` confirma que no se compila y la CI falla si llegara a compilarse. No es código que ejecute la aplicación. |

## 9. Cómo reproducir

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
- `tests/qa_verificacion.rs`
- `tests/qa_origen.rs`
- `tests/qa_integridad.rs`
- `tests/qa_ataques.rs`
- `tests/qa_robustez.rs`

Los archivos de referencia para futuros auditores son:

- `lead-notes.md` — decisiones autoritativas del revisor principal.
- `round2-notes.md` — cierre autoritativo de la Ronda 2.
- `00-consolidated.md` — matriz original de los 128 hallazgos.
- `report-*.md` — reportes por paquete con veredictos y evidencia.