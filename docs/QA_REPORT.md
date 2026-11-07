# Informe de QA — RetosEAN

> **Nota:** este informe auditó la versión Django, ya retirada del repositorio.
> Sus hallazgos se portaron a la aplicación Rust (`rust/`) con pruebas de regresión
> en `rust/tests/`.

- **Fecha:** 2026-11-06
- **Alcance:** app Django `retosean-django` (14 apps + config)
- **Fuera de alcance:** port Rust (solo se incluye como puerta de CI sugerida)

## 1. Resumen ejecutivo

RetosEAN es una plataforma Django 6 + PostgreSQL que conecta empresas, profesores, estudiantes y expertos alrededor de retos. La auditoría cubrió 14 aplicaciones y la configuración con revisión por módulo y verificación cruzada contra el código real. Se confirmaron **79 hallazgos no descartados**: 0 críticos, 11 altos, 31 medios y 37 bajos (aparte quedan 5 hallazgos de `usuarios` pendientes de verificación, sección 3.1). No se confirmó ningún hallazgo crítico después de la verificación, pero los altos evidencian problemas serios de autorización, integridad académica y exposición de documentos legales. La suite actual de 271 pruebas pasa, pero no cubre las rutas negativas, condiciones de carrera ni autorización por objeto que originan la mayoría de los hallazgos. El CI actual no incluye cobertura, lint, escaneo de seguridad ni pruebas Rust.

| Severidad | Cantidad |
|---|---|
| Crítico | 0 |
| Alto | 11 |
| Medio | 31 |
| Bajo | 37 |
| **Total** | **79** |

### Top 5 a corregir primero

1. **Registro académico crea profesores activos e inicia sesión de inmediato** — cualquier persona con correo `@universidadean.edu.co` puede auto-registrarse como `PROFESOR`, quedar activa y autenticada sin aprobación; esto encadena con `_puede_gestionar` global en unidades de estudio y con creación de hackatones sobre retos ajenos.
2. **Documentos legales descargables sin control de autorización (IDOR vía media)** — rutas predecibles permiten a una empresa descargar PDFs legales de otra sin autenticación ni permiso.
3. **Un estudiante puede reemplazar un entregable final ya calificado/publicado** — `get_or_create` por `(reto, estudiante, titulo)` sobrescribe el archivo y conserva nota/publicación inconsistentes.
4. **Eliminar un jurado reescribe silenciosamente resultados publicados** — `on_delete=CASCADE` destruye votos y puede cambiar ganador después de notificar resultados.
5. **Finalizar un reto no bloquea evaluaciones pendientes** — permite cerrar, emitir certificados y congelar expediente con entregables sin evaluación.

---

## 2. Línea base

- `manage.py check` sin problemas.
- `makemigrations --check` sin migraciones pendientes.
- 271 pruebas, todas pasan (237 s, PostgreSQL).
- CI: GitHub Actions corre `check`, migraciones y pruebas sólo con `config.settings.development`.
- No hay cobertura, lint, escaneo de seguridad, `check --deploy` ni `cargo test`.

---

## 3. Hallazgos verificados

### 3.1 Usuarios

| ID | Severidad | Hallazgo | Ubicación | Veredicto |
|---|---|---|---|---|
| USR-01 | Medio | Cabecera `X-Forwarded-For` inválida provoca error 500 y permite falsear la IP almacenada en `LogActividad` | `apps/usuarios/signals.py :: _ip() / registrar_login()` | Confirmado |
| USR-02 | Alto | Registro académico crea profesores activos e inicia sesión de inmediato | `apps/usuarios/views.py :: vista_registro()`; `apps/usuarios/forms.py :: RegistroAcademicoForm` | Confirmado |
| USR-03 | Medio | Mensaje de cuenta inactiva revela estado OFAC/Clinton y existencia de cuenta | `apps/usuarios/views.py :: _mensaje_cuenta_inactiva() / vista_login()` | Confirmado |
| USR-04 | Medio | Sin limitación de intentos de login / fuerza bruta; tampoco hay `AUTH_PASSWORD_VALIDATORS` | `apps/usuarios/views.py :: vista_login()` | Confirmado |

#### USR-02 — Registro académico crea profesores activos e inicia sesión de inmediato

- **Escenario:** un visitante con correo institucional elige rol `PROFESOR`; `RegistroAcademicoForm.save()` no desactiva la cuenta y `vista_registro()` llama `login()` inmediatamente. El nuevo profesor accede al panel sin aprobación. Encadena con gestión global de unidades de estudio y creación de hackatones sobre retos ajenos.
- **Corrección sugerida:** si los profesores requieren aprobación, crear la cuenta con `is_active=False`/estado pendiente, no iniciar sesión y redirigir a una pantalla de revisión. Los estudiantes pueden mantenerse activos si la regla lo permite.
- **Prueba sugerida:** `test_registro_profesor_no_inicia_sesion_hasta_aprobacion`: POST válido `tipo=academico` y rol `PROFESOR`; comprobar `usuario.is_active is False` y que la sesión no contiene `_auth_user_id`.

#### Pendientes de verificación

| ID | Severidad original | Hallazgo | Ubicación |
|---|---|---|---|
| USR-05 | Medio | Importación masiva no crea los perfiles por rol | `apps/usuarios/importacion.py :: importar()` |
| USR-06 | Medio | Carrera entre previsualización y confirmación de importación produce `IntegrityError` 500 | `apps/usuarios/importacion.py :: importar()` |
| USR-07 | Medio | `LoginForm.clean_username` permite 500 si existen nombres de usuario que difieren solo por mayúsculas | `apps/usuarios/forms.py :: LoginForm.clean_username()` |
| USR-08 | Medio | Guardado de registro/edición de perfil sin transacción atómica | `apps/usuarios/forms.py :: RegistroAcademicoForm.save(), RegistroEmpresaForm.save(), EditarPerfilForm.save()` |
| USR-09 | Bajo | Importación CSV lee todo el archivo en memoria sin límite de tamaño real | `apps/usuarios/importacion.py :: _leer_csv()` |

---

### 3.2 Retos

| ID | Severidad | Hallazgo | Ubicación | Veredicto |
|---|---|---|---|---|
| RET-01 | Alto | La validación de estado obsoleto puede sobrescribir un estado terminal | `apps/retos/services.py:89` | Confirmado |
| RET-02 | Medio | Una integración borrador o rechazada otorga acceso de lectura/escritura a seguimiento | `apps/retos/views.py:90` | Confirmado |
| RET-03 | Alto | Cualquier integración de profesor autoriza acciones destructivas sobre todos los equipos del reto | `apps/retos/views.py:733-737` | Confirmado |
| RET-04 | Medio | La pertenencia a un equipo omite las reglas de publicación y visibilidad del estudiante | `apps/retos/views.py:79` | Confirmado |
| RET-05 | Medio | Una integración puede publicarse después de que su reto alcance un estado terminal | `apps/retos/views.py:533` | Confirmado |
| RET-06 | Medio | Los puntos de conexión directos de equipo y sesión omiten las restricciones de edición de la integración | `apps/retos/views.py:668,713` | Confirmado |
| RET-07 | Bajo | La creación de retos y los flujos de múltiples escrituras relacionados no son atómicos | `apps/retos/views.py:127-129` | Confirmado |
| RET-08 | Medio | La validación de carga acepta contenido de archivo arbitrario | `apps/retos/forms.py:26-34` | Confirmado |
| RET-09 | Bajo | Las fechas del reto no tienen validación cronológica | `apps/retos/forms.py:50-92` | Confirmado |
| RET-10 | Bajo | La validación de facultad/programa acepta inconsistencias y bloquea cambios válidos de facultad | `apps/retos/forms.py:99-103` | Confirmado |

#### RET-01 — La validación de estado obsoleto puede sobrescribir un estado terminal

- **Escenario:** publicación y cancelación cargan el mismo reto `aprobado`; cancelación confirma `aprobado → cancelado` y publicación guarda después una instancia obsoleta como `abierto`, resucitando el reto.
- **Corrección sugerida:** releer y bloquear con `select_for_update()` dentro de `transaction.atomic()`, validar contra el estado bloqueado y aplicar la misma disciplina a `publicar_integracion`.
- **Prueba sugerida:** `test_publicacion_no_resucita_un_reto_cancelado`: usar `TransactionTestCase`, dos conexiones y barrera para forzar la intercalación; verificar estado final `cancelado` y que no se registre `cancelado → abierto`.

#### RET-03 — Cualquier integración de profesor autoriza acciones destructivas sobre todos los equipos del reto

- **Escenario:** el profesor B crea solo una integración borrador sobre el mismo reto y puede eliminar/editar el equipo académico del profesor A, porque el queryset busca por `pk` + `reto__integraciones_seguimiento__profesor=request.user`.
- **Corrección sugerida:** asociar cada `EquipoRetoAcademico` a su `IntegracionAcademica` propietaria y autorizar a través de esa relación; como mínimo, exigir asignación aprobada activa y definir si otros profesores administran equipos compartidos.
- **Prueba sugerida:** `test_profesor_con_borrador_no_elimina_equipo_de_otro_profesor`: crear equipo de A y borrador de B; verificar 403/404 y que no cambien el equipo ni sus espejos operativos.

---

### 3.3 Empresas

| ID | Severidad | Hallazgo | Ubicación | Veredicto |
|---|---|---|---|---|
| EMP-01 | Alto | Documentos legales descargables sin control de autorización (IDOR vía media) | `apps/empresas/views.py:128`; `apps/empresas/models.py:6-10` | Confirmado |
| EMP-02 | Medio | Validación de archivo legal basada solo en extensión | `apps/empresas/forms.py:39-45` | Confirmado |
| EMP-03 | Medio | Flujo de listas restrictivas modifica múltiples entidades sin atomicidad | `apps/empresas/views.py:511-580` | Confirmado |
| EMP-04 | Medio | Evidencia de listas restrictivas se sube sin ninguna validación | `apps/empresas/views.py:528-529` | Confirmado |
| EMP-05 | Bajo | No hay límite de tamaño explícito para documentos legales | `apps/empresas/forms.py:17-20` | Confirmado |
| EMP-06 | Bajo | Condición de carrera en creación concurrente de documentos | `apps/empresas/services.py:160-171` | Confirmado |
| EMP-07 | Bajo | Elegir “sí convenio” no revierte `renuncio_a_convenio` | `apps/empresas/views.py:592-593` | Confirmado |
| EMP-08 | Bajo | Borrado de archivo anterior antes de confirmar el guardado | `apps/empresas/models.py:199-204` | Confirmado |
| EMP-09 | Bajo | El personal sin perfil de empresa puede acceder al panel de documentos | `apps/empresas/views.py:41` | Confirmado |
| EMP-10 | Bajo | El administrador de “pendientes” oculta documentos VERIFICADOS vencidos | `apps/empresas/views.py:345` | Parcial |

#### EMP-01 — Documentos legales descargables sin control de autorización (IDOR vía media)

- **Escenario:** la empresa A conoce o adivina el NIT de la empresa B y accede a `/media/documentos_empresas/<NIT_B>/<NIT_B>_RUT.pdf`, descargando documentos legales ajenos sin ser dueña ni admin.
- **Corrección sugerida:** servir documentos por vista autenticada que valide `request.user.empresa_perfil == documento.empresa` o permiso admin; usar almacenamiento privado o URLs firmadas.
- **Prueba sugerida:** `test_documento_no_descargable_entre_empresas`: crear dos empresas con documentos, autenticarse como A y solicitar el documento de B; esperar 403/404 sin contenido.

---

### 3.4 Participaciones y evaluación

| ID | Severidad | Hallazgo | Ubicación | Veredicto |
|---|---|---|---|---|
| PAR-01 | Medio | El total de la rúbrica puede exceder la escala del entregable / calificación fuera de 0-5 | `apps/evaluacion/views.py:76-85` | Confirmado |
| PAR-02 | Alto | Las integraciones académicas inactivas o revocadas aún permiten calificar/publicar | `apps/evaluacion/views.py:23-24` | Confirmado |
| PAR-03 | Alto | Un estudiante puede sobrescribir un entregable final/publicado | `apps/participaciones/views.py:301-306` | Confirmado |
| PAR-04 | Bajo | Una condición de carrera por postulación duplicada causa un 500 no controlado | `apps/participaciones/views.py:49-62` | Confirmado |
| PAR-05 | Bajo | Redirección abierta en vistas de evaluación | `apps/evaluacion/views.py:59,159` | Confirmado |
| PAR-06 | Bajo | No se verifica el estado no finalizado de la fecha límite al cargar entregables | `apps/participaciones/views.py:283` | Parcial |
| PAR-07 | Bajo | El estado de un entregable pendiente cambia en GET | `apps/evaluacion/views.py:32-34` | Confirmado |
| PAR-08 | Bajo | El flujo de carga de entregables carece de `transaction.atomic` | `apps/participaciones/views.py:287-307` | Confirmado |

#### PAR-02 — Las integraciones académicas inactivas o revocadas aún permiten calificar/publicar

- **Escenario:** un profesor con integración `rechazada`, `retirada` o en borrador sigue viendo el reto en `panel_profesor`, `calificar`, `publicar_calificaciones` y `comentario` porque `_retos_profesor()` filtra únicamente por `profesor=usuario`.
- **Corrección sugerida:** filtrar solo estados activos/aprobados, p. ej. `IntegracionAcademica.ESTADOS_ACTIVAS`.
- **Prueba sugerida:** `test_profesor_con_integracion_rechazada_no_puede_calificar`: crear integración rechazada, autenticar al profesor, POST a `calificar`/`publicar_calificaciones` y verificar 403/404 sin cambios.

#### PAR-03 — Un estudiante puede sobrescribir un entregable final/publicado

- **Escenario:** un `Entregable` `es_final=True`, `CALIFICADO` y con calificación publicada se reemplaza subiendo el mismo `titulo`; el archivo se sobrescribe, el estado vuelve a `ENVIADO` y la nota/publicación anterior quedan inconsistentes.
- **Corrección sugerida:** bloquear edición cuando `entregable.es_final` o `entregable.calificacion_publicada` sea verdadero; o exigir reset explícito del profesor y limpiar nota, comentarios y publicación.
- **Prueba sugerida:** `test_estudiante_no_puede_reemplazar_entregable_final_publicado`: crear entregable final publicado, POSTear el mismo título y verificar que archivo, nota y publicación no cambian.

---

### 3.5 Hackatón

| ID | Severidad | Hallazgo | Ubicación | Veredicto |
|---|---|---|---|---|
| HCK-01 | Medio | El avance por calendario no es atómico y puede duplicar o sobrescribir transiciones | `apps/hackaton/services.py:161-176` | Confirmado |
| HCK-02 | Medio | Un equipo rechazado permanece en el ranking y aún puede declararse ganador | `apps/hackaton/services.py:52-62` | Confirmado |
| HCK-03 | Alto | Eliminar un jurado reescribe silenciosamente resultados publicados | `apps/hackaton/views.py:212` | Confirmado |
| HCK-04 | Medio | Los hackatones finalizados pueden reabrirse y luego cambiar los resultados públicos | `apps/hackaton/forms.py:15` | Confirmado |
| HCK-05 | Medio | La votación falla de forma permisiva cuando la etapa de presentación no existe o no está programada | `apps/hackaton/services.py:38-39` | Confirmado |
| HCK-06 | Bajo | Un empate total se convierte en un ganador único por nombre de equipo | `apps/hackaton/services.py:61` | Parcial |
| HCK-07 | Medio | Cualquier profesor puede crear un hackatón sobre un reto no relacionado | `apps/hackaton/views.py:110-122` | Confirmado |
| HCK-08 | Bajo | Un estudiante puede registrar varios equipos en el mismo hackatón | `apps/hackaton/forms.py:113-117` | Confirmado |
| HCK-09 | Medio | El acceso directo a objetos omite el alcance por profesor/empresa de la lista | `apps/hackaton/views.py:52-54` | Confirmado |
| HCK-10 | Bajo | Los equipos aceptados sin votos desaparecen del ranking y de los reportes | `apps/hackaton/services.py:52-62` | Confirmado |
| HCK-11 | Bajo | La inscripción duplicada concurrente pasa la validación del formulario y termina en un 500 | `apps/hackaton/views.py:254-258` | Confirmado |
| HCK-12 | Bajo | El formulario de asignación de jurado no puede asignar el rol soportado EXPERTO | `apps/hackaton/forms.py:72-74` | Confirmado |

#### HCK-03 — Eliminar un jurado reescribe silenciosamente resultados publicados

- **Escenario:** un jurado ha votado, los resultados ya están publicados y un manager lo elimina; `VotacionHackaton.jurado` usa `on_delete=CASCADE`, destruye sus votos y el ganador publicado puede cambiar sin auditoría.
- **Corrección sugerida:** impedir eliminación si existen votos o resultados publicados; preferir desactivación lógica que conserve las papeletas y definir un recálculo auditado si el jurado debe invalidarse.
- **Prueba sugerida:** `test_eliminar_jurado_no_altera_resultados_publicados`: publicar ranking dependiente de un jurado, POST a `eliminar_jurado` y verificar rechazo o borrado lógico, con votos/ganador inmutables.

---

### 3.6 Cierre, seguimiento, académico y unidades de estudio

| ID | Severidad | Hallazgo | Ubicación | Veredicto |
|---|---|---|---|---|
| CIE-01 | Medio | Certificados se emiten a estudiantes sin validar aprobación/completitud | `apps/academico/services.py:42-50` | Parcial |
| CIE-02 | Alto | Finalizar un reto no bloquea evaluaciones pendientes | `apps/cierre/views.py:234-266` | Confirmado |
| CIE-03 | Alto | Cualquier profesor puede administrar unidades de estudio de cualquier programa | `apps/unidades_estudio/views.py:9-10` | Confirmado |
| CIE-04 | Bajo | Encuesta de satisfacción puede responderse/modificarse múltiples veces | `apps/cierre/views.py:303-311` | Parcial |
| CIE-05 | Bajo | GET de `gestionar` ejecuta emisión de certificados como efecto colateral | `apps/cierre/views.py:70-86` | Confirmado |
| CIE-06 | Medio | Retos ya finalizados pueden recibir entregables/reconocimientos nuevos | `apps/cierre/views.py:109-126` | Confirmado |
| CIE-07 | Medio | Cambio de estado guarda objeto e historial sin atomicidad | `apps/seguimiento/services.py:87-94` | Confirmado |
| CIE-08 | Medio | Importación CSV de unidades no es transaccional | `apps/unidades_estudio/views.py:270-304` | Confirmado |
| CIE-09 | Medio | CSV resuelve programa por nombre y puede asignar programa equivocado | `apps/unidades_estudio/views.py:226-230` | Confirmado |

#### CIE-02 — Finalizar un reto no bloquea evaluaciones pendientes

- **Escenario:** hay entregables pendientes de evaluación; el cierre solo exige que exista algún entregable final, auto-vincula entregables con archivo sin revisar y finaliza/emite certificados.
- **Corrección sugerida:** antes de finalizar, validar que no existan entregables/evaluaciones incompletas y que todas las evaluaciones requeridas estén completas.
- **Prueba sugerida:** `test_finalizar_bloquea_evaluaciones_pendientes`: crear reto con entregable pendiente, POST a `finalizar`; verificar que no quede `finalizado` y no se emitan certificados.

#### CIE-03 — Cualquier profesor puede administrar unidades de estudio de cualquier programa

- **Escenario:** `_puede_gestionar` autoriza a todo `PROFESOR`; edita o desactiva unidades por `pk` sin validar facultad ni programa.
- **Corrección sugerida:** restringir a `ADMIN` o a profesores autorizados por facultad/programa/unidad; usar permiso por objeto.
- **Prueba sugerida:** `test_profesor_externo_no_gestiona_unidades`: crear profesor sin permiso, POST a `eliminar_unidad`; verificar denegación y unidad activa.

---

### 3.7 Presupuesto, reportes y dashboard

| ID | Severidad | Hallazgo | Ubicación | Veredicto |
|---|---|---|---|---|
| PRE-01 | Alto | Los gastos pueden exceder el presupuesto asignado, generando disponible negativo | `apps/presupuesto/forms.py:40`; `apps/presupuesto/views.py:123` | Confirmado |
| PRE-02 | Medio | Los identificadores de filtro del dashboard inválidos causan un 500 no controlado | `apps/dashboard/views.py:33`; `apps/dashboard/services.py:35` | Confirmado |
| PRE-03 | Medio | El dashboard de estudiante/profesor ignora el rango de fechas seleccionado para entregables | `apps/dashboard/services.py:248,194` | Confirmado |
| PRE-04 | Bajo | La exportación Excel del presupuesto convierte montos Decimal a float | `apps/presupuesto/services.py:147,153,163` | Parcial |
| PRE-05 | Medio | La generación de reportes Excel no tiene límite de filas y puede agotar la memoria | `apps/reportes/services.py:105` | Confirmado |
| PRE-06 | Bajo | El reporte de presupuesto recalcula todos los gastos para cada columna | `apps/reportes/catalogo.py:262` | Confirmado |
| PRE-07 | Medio | El reporte de “Empresas aliadas” ignora la fecha seleccionada y otros filtros | `apps/reportes/catalogo.py:291` | Confirmado |
| PRE-08 | Bajo | `revisar_umbral` ejecuta consultas agregadas redundantes en cada acceso a la propiedad | `apps/presupuesto/services.py:51`; `apps/presupuesto/models.py:45` | Confirmado |

#### PRE-01 — Los gastos pueden exceder el presupuesto asignado, generando disponible negativo

- **Escenario:** un admin define presupuesto de $100 y registra un gasto ejecutado de $150; el formulario solo valida `monto > 0` y `registrar_gasto` lo guarda sin validar disponible, generando disponible negativo.
- **Corrección sugerida:** en `agregar_gasto`, comparar el monto con `presupuesto.monto_disponible` y rechazar si lo excede; considerar también `COMPROMETIDO`.
- **Prueba sugerida:** `test_rechaza_gasto_mayor_a_disponible`: presupuesto total 100, POST de gasto 150; verificar que no se crea gasto y disponible sigue en 100.

---

### 3.8 Notificaciones

| ID | Severidad | Hallazgo | Ubicación | Veredicto |
|---|---|---|---|---|
| NOT-01 | Medio | Los profesores desactivados aún reciben notificaciones y correos de recordatorio | `apps/notificaciones/management/commands/enviar_recordatorios.py:153,251` | Confirmado |
| NOT-02 | Medio | Las fechas límite reprogramadas pierden permanentemente los recordatorios posteriores | `apps/notificaciones/management/commands/enviar_recordatorios.py:126,150,158,204,231` | Confirmado |
| NOT-03 | Bajo | Los errores de integridad por falta de deduplicación se descartan silenciosamente | `apps/notificaciones/services.py:52,55` | Confirmado |
| NOT-04 | Bajo | La ejecución simulada reporta destinatarios que una ejecución real omitiría | `apps/notificaciones/management/commands/enviar_recordatorios.py:94` | Confirmado |
| NOT-05 | Bajo | `--dias` cambia inesperadamente el umbral de entregables sin calificar | `apps/notificaciones/management/commands/enviar_recordatorios.py:50` | Confirmado |
| NOT-06 | Bajo | Los recordatorios semanales de entregables sin calificar pueden duplicarse alrededor de Año Nuevo | `apps/notificaciones/management/commands/enviar_recordatorios.py:249` | Confirmado |
| NOT-07 | Bajo | Los filtros de fecha inválidos del administrador producen un error de servidor no controlado | `apps/notificaciones/views.py:127,129` | Confirmado |

---

### 3.9 Configuración y despliegue

| ID | Severidad | Hallazgo | Ubicación | Veredicto |
|---|---|---|---|---|
| CFG-01 | Alto | Valor por defecto inseguro de `SECRET_KEY` | `config/settings/base.py:23` | Confirmado |
| CFG-02 | Medio | No hay validadores de contraseña configurados | `config/settings/base.py:100-101` | Confirmado |
| CFG-03 | Bajo | La cabecera SSL del proxy se confía incondicionalmente | `config/settings/production.py:13` | Confirmado |
| CFG-04 | Medio | Los archivos media privados pueden exponerse adivinando la URL cuando `DEBUG` está habilitado | `config/urls.py:42-43` | Confirmado |
| CFG-05 | Bajo | El contenedor se ejecuta como root e inicia el servidor de desarrollo de Django con migraciones automáticas | `Dockerfile:16` | Confirmado |
| CFG-06 | Bajo | Los límites de tamaño de carga están incompletos y pueden causar abuso de memoria/disco | `config/settings/base.py:126-127` | Parcial |
| CFG-07 | Bajo | Compose contiene secretos por defecto débiles y expone interfaces de BD/correo | `docker-compose.yml:13,17,27,33` | Confirmado |
| CFG-08 | Bajo | `CSRF_TRUSTED_ORIGINS` de producción está vacío por defecto | `config/settings/production.py:9` | Confirmado |
| CFG-09 | Bajo | CI solo ejecuta pruebas con la configuración de desarrollo y carece de puertas de seguridad/calidad | `.github/workflows/tests.yml:27,57` | Confirmado |
| CFG-10 | Bajo | El registro no tiene filtrado de datos sensibles ni foco en eventos de seguridad | `config/settings/base.py:150-162` | Confirmado |
| CFG-11 | Bajo | El administrador de Django está expuesto en una ruta conocida | `config/urls.py:25` | Confirmado |

#### CFG-01 — Valor por defecto inseguro de `SECRET_KEY`

- **Escenario:** si la variable de entorno `SECRET_KEY` no existe, Django arranca con `change-me-in-env`, permitiendo falsificar sesiones y CSRF.
- **Corrección sugerida:** eliminar el default y fallar rápido si no está definida; cargar desde gestor de secretos y rotar.
- **Prueba sugerida:** arranque con `SECRET_KEY` ausente debe fallar inmediatamente sin levantar la app.

---

## 4. Brechas de cobertura de pruebas

### Usuarios

- Ramas de usuario inactivo, `next` seguro, cierre de sesión y bitácora `LOGOUT`.
- Registro académico por rol con validación de correo institucional y redirección.
- Importación CSV/Excel: perfiles por rol, concurrencia entre previsualización/confirmación, plantilla.
- Edición de perfil por rol y creación de perfil faltante en usuarios importados.
- `LoginForm.clean_username` con nombres de usuario sin distinción de mayúsculas y `MultipleObjectsReturned`.
- Funciones auxiliares de roles y `context_processors.rol_flags`.

### Retos

- Ciclo de vida completo y matriz de estados para editar/borrar por POST.
- Matriz de autorización por estado de integración para seguimiento, equipos y sesiones.
- Concurrencia entre aprobar/rechazar, cancelar/publicar y publicación repetida.
- Fallos inyectados y reversión de creación/archivos/historial.
- Validación de fechas, catálogo facultad/programa y contenido de archivos.
- Visibilidad de estudiantes por estado y equipos preliminares.

### Empresas

- Descarga/acceso directo a documentos legales entre empresas.
- Validación de contenido real de archivos, tamaño y evidencia de listas.
- Atomicidad de listas restrictivas y `Empresa.usuario=None`.
- Carrera en creación del primer documento y borrado de archivo antiguo.
- Personal sin empresa y documentos vencidos en la cola del administrador.
- Reversión de `renuncio_a_convenio`.

### Participaciones y evaluación

- Rúbricas que superen `puntaje_maximo`.
- Integraciones inactivas/rechazadas en calificación/publicación.
- Bloqueo de reemplazo de entregable final calificado.
- Doble postulación concurrente y manejo de `IntegrityError`.
- Redirección abierta en vistas de evaluación.
- Mutación por GET en panel profesor y atomicidad de subida.

### Hackatón

- Concurrencia de avance por calendario y GET sin efectos colaterales.
- Votación con fallo cerrado sin etapa de presentación fechada.
- Ranking con rechazo posterior, empate total, equipos sin votos y jurado eliminado.
- Autorización negativa: profesor sin integración y acceso directo a borradores.
- Inmutabilidad de resultados publicados tras edición/re-publicación.
- Solapamiento de estudiantes entre equipos y doble inscripción concurrente.

### Cierre, seguimiento, académico y unidades de estudio

- Emisión de certificados sujeta a aprobación/completitud.
- Bloqueo de cierre con evaluaciones pendientes.
- Inmutabilidad de encuestas ya respondidas.
- Autorización fina por programa/facultad en unidades de estudio.
- Efectos colaterales en GET y modificaciones post-cierre.
- Atomicidad de cambios de estado + historial.

### Presupuesto, reportes y dashboard

- Regla de gasto contra disponible y puntos de conexión solo POST.
- Validación de parámetros no enteros del dashboard y rangos de fecha.
- Respeto del periodo en los KPI de estudiante/profesor.
- Precisión Decimal en Excel y límite de filas.
- Filtros aplicables por reporte, N+1 y descarga autorizada.
- Inyección de fórmulas en exportaciones.

### Notificaciones

- Exclusión de profesores inactivos y cambio de fecha en recordatorios.
- Integridad de deduplicación y reversión del envío de correo.
- Ejecución simulada exacta, preferencias de usuario y cambio de año ISO.
- `--dias` sin alterar umbral de calificación.
- Fechas malformadas en el registro del administrador.
- Concurrencia en deduplicación y CSRF/IDOR en lectura.

### Configuración y despliegue

- No hay una puerta automatizada para `SECRET_KEY` insegura, media privada en DEBUG o `check --deploy`.
- No hay lint, cobertura, `bandit`, `pip-audit` ni `cargo test` en CI.
- No hay validación automática de `ALLOWED_HOSTS`, `CSRF_TRUSTED_ORIGINS` ni límites de subida.
- No hay control automatizado de filtrado de registros/PII.

---

## 5. Plan de pruebas manuales

Contraseñas de demo en `GUION_DEMO.md`.

### 5.1 Usuarios

| ID | Precondición | Pasos | Resultado esperado |
|---|---|---|---|
| MAN-USR-01 | Usuario activo `estudiante.demo` | Abrir `/usuarios/login/`, ingresar credenciales correctas | Redirige al panel del rol y bitácora registra `LOGIN` |
| MAN-USR-02 | Usuario activo | Ir a `/usuarios/login/?next=http://evil.com` con `Host` válido de `ALLOWED_HOSTS` | No debe redirigir fuera del sitio |
| MAN-USR-03 | Correo `@universidadean.edu.co` | Registrarse en `/usuarios/registro/?tipo=academico` con rol `PROFESOR` | La cuenta no debe quedar activa ni iniciar sesión sin aprobación; si ocurre, consultar hallazgo USR-02 |
| MAN-USR-04 | Usuario inactivo | Intentar login con credenciales correctas | Mensaje genérico, no revela OFAC/Clinton ni existencia |
| MAN-USR-05 | `admin.demo` | Importar CSV válido, primero previsualizar y luego confirmar | Previsualización no crea usuarios; confirmación crea solo filas válidas |
| MAN-USR-06 | `admin.demo` | Cambiar rol y desactivar/reactivar un usuario | Bitácora registra `CAMBIO_ROL`/`CAMBIO_ESTADO`; usuario inactivo no navega |

### 5.2 Retos

| ID | Precondición | Pasos | Resultado esperado |
|---|---|---|---|
| MAN-RET-01 | `empresa.verificada`, `admin.demo`, `profesor.demo` | Crear reto, aprobar, integrar y publicar | Transiciones válidas sin duplicar historial |
| MAN-RET-02 | `empresa.sinconvenio`, `admin.demo` | Intentar crear y someter reto sin documentos válidos, luego revocar documentos de otra empresa y aprobar | Se bloquea sin cambio de estado y aprobación muestra explicación |
| MAN-RET-03 | `empresa.revision`, `empresa.verificada` | Como B, abrir detalle y POST editar/enviar/eliminar retos de A | Solo accede a sus propios objetos |
| MAN-RET-04 | Retos en `en_revision`, `aprobado`, `abierto`, `en_curso`, `finalizado`, `cancelado` | Abrir URL de edición y POST en cada estado | Denegado con respuesta controlada y sin cambios |
| MAN-RET-05 | Profesor B con integración borrador en reto de A | Abrir seguimiento y enviar entrada/adjunto | GET/POST denegados, sin datos creados |
| MAN-RET-06 | Profesor A con equipo; profesor B con borrador | B intenta eliminar/editar equipo de A | B no administra equipos ajenos; integridad intacta |
| MAN-RET-07 | Reto con fechas y archivo HTML/PDF | Enviar fecha fin anterior a inicio, archivo HTML y facultad/programa inconsistente | Errores de validación; no guarda datos inválidos |

### 5.3 Empresas

| ID | Precondición | Pasos | Resultado esperado |
|---|---|---|---|
| MAN-EMP-01 | `empresa.verificada` | Subir PDF legal válido | Documento `CARGADO`, admin notificado |
| MAN-EMP-02 | `empresa.verificada` | Subir `documento.docx` o `.txt` | Error “Formato no permitido” sin guardar |
| MAN-EMP-03 | `empresa.verificada` | Subir `exploit.pdf` con bytes HTML/EXE | Debe rechazar por contenido; si guarda, documentar defecto |
| MAN-EMP-04 | `empresa.verificada` y `empresa.sinconvenio` con documentos | Como A, abrir URL directa del documento de B | 403/404 o descarga denegada |
| MAN-EMP-05 | `admin.demo`, empresa con documentos | Marcar `RECHAZADO` en listas restrictivas | Documentos rechazados, cuenta inactiva y estado consistente |
| MAN-EMP-06 | `empresa.sinconvenio` | Elegir `si_convenio` después de haber renunciado | `renuncio_a_convenio=False` y se exigen documentos |
| MAN-EMP-07 | Documento `VERIFICADO` vencido | Revisar cola admin de pendientes | El documento vencido debe aparecer disponible para acción |

### 5.4 Participaciones y evaluación

| ID | Precondición | Pasos | Resultado esperado |
|---|---|---|---|
| MAN-PAR-01 | `estudiante.demo` en reto abierto | Doble clic en “Postular” | Una sola `Postulacion`, sin 500 |
| MAN-PAR-02 | Profesor con rúbrica que suma > 5 | Calificar con criterios al máximo | No guarda nota mayor a `puntaje_maximo` |
| MAN-PAR-03 | Estudiante con entregable final publicado | Re-subir el mismo título con archivo nuevo | Bloqueado; archivo, nota y publicación intactos |
| MAN-PAR-04 | Profesor con integración rechazada | Abrir `panel_profesor`, `calificar`, `publicar_calificaciones` | Acceso denegado; sin cambios |
| MAN-PAR-05 | Profesor con entregables `ENVIADO` | Abrir `panel_profesor` por GET | No muta estados en GET |
| MAN-PAR-06 | Profesor en `calificar` | POST con `next=https://evil.example.com` | Redirección interna, no externa |

### 5.5 Hackatón

| ID | Precondición | Pasos | Resultado esperado |
|---|---|---|---|
| MAN-HCK-01 | Publicado sin etapa de presentación fechada | Votar | “La etapa de presentación no está abierta”; sin voto |
| MAN-HCK-02 | Dos equipos con votos; equipo mayor rechazable | Rechazar el mayor y ver ranking | Bloqueado o excluido consistentemente; no gana |
| MAN-HCK-03 | Resultados publicados dependientes de un jurado | Eliminar ese jurado | Bloqueado o borrado lógico; ganador no cambia |
| MAN-HCK-04 | Resultados publicados | Editar estado a `en_curso` e intentar votar | Reapertura denegada; votación cerrada |
| MAN-HCK-05 | Mismo equipo registrado en dos navegadores | Enviar simultáneamente | Una inscripción y sin 500 para el perdedor |
| MAN-HCK-06 | `profesor.ajeno` sin integración | POST a crear hackatón sobre reto de otra empresa | Denegado; no crea la fila OneToOne |
| MAN-HCK-07 | Empresa/profesor ajenos con ID de borrador | Abrir detalle/ranking del borrador | Denegado; no expone datos sensibles |
| MAN-HCK-08 | `EXPERTO` activo y `profesor.demo` | Asignar EXPERTO como jurado; seleccionar profesor con `nombre` vacío | EXPERTO disponible; nombre se completa sin error |

### 5.6 Cierre, seguimiento, académico y unidades

| ID | Precondición | Pasos | Resultado esperado |
|---|---|---|---|
| MAN-CIE-01 | Reto con entregable `ENVIADO` pendiente de evaluación | Finalizar reto | Bloqueado con mensaje de evaluaciones pendientes |
| MAN-CIE-02 | `admin.demo` | Abrir `gestionar` de un reto finalizado por GET | No se emiten certificados ni notificaciones |
| MAN-CIE-03 | Profesor sin relación con una unidad | POST `eliminar_unidad` | 403/redirect y unidad permanece activa |
| MAN-CIE-04 | Usuario con encuesta ya respondida | Enviar segundo POST con otra calificación | No se modifica o se bloquea |
| MAN-CIE-05 | Reto finalizado | POST a `agregar_entregable` | Acción bloqueada; no crea registro |
| MAN-CIE-06 | Dos programas con el mismo nombre en facultades distintas | Importar CSV que use ese nombre | Se rechaza o selecciona el programa correcto, sin aleatoriedad |

### 5.7 Presupuesto, reportes y dashboard

| ID | Precondición | Pasos | Resultado esperado |
|---|---|---|---|
| MAN-PRE-01 | Reto con presupuesto de $100 | Agregar gasto de $150 | Error; no se crea gasto y disponible queda en $100 |
| MAN-PRE-02 | `admin.demo` | Abrir `/dashboard/?facultad=abc` | La página no debe devolver 500 |
| MAN-PRE-03 | Estudiante con entregables viejos y recientes | Filtrar dashboard por últimos 30 días | Solo cuenta entregables recientes |
| MAN-PRE-04 | Presupuesto con `Decimal("0.10")` | Exportar Excel | Montos permanecen `0.10` sin artefactos float |
| MAN-PRE-05 | Reporte “Empresas aliadas” con filtros de fecha/facultad | Generar exportación | Filtros se aplican o no se ofrecen |
| MAN-PRE-06 | `empresa.revision` y `admin.demo` | GET a URLs POST-only: definir, gasto, eliminar, ejecutar | 405 y sin cambios |
| MAN-PRE-07 | Reto con título `=cmd|...` | Exportar Excel | Celda almacenada como texto, no fórmula |
| MAN-PRE-08 | Usuario B y reporte generado por A | B intenta descargar reporte de A | Denegado/redirigido |

### 5.8 Notificaciones

| ID | Precondición | Pasos | Resultado esperado |
|---|---|---|---|
| MAN-NOT-01 | Usuario A y B | B hace POST a la URL `leer/` de notificación de A | 404; A sigue sin leer |
| MAN-NOT-02 | Usuario autenticado | GET a URL de lectura; POST sin CSRF | 405/rechazo CSRF; la notificación sigue sin leer |
| MAN-NOT-03 | Recordatorio elegible, con inactivo y usuario opt-out | Ejecutar comando dos veces | Una notificación por usuario activo; inactivo/opt-out sin envío |
| MAN-NOT-04 | Recordatorio de siete días ya generado | Cambiar fecha límite y ejecutar comando antes de la nueva fecha | Se genera un nuevo recordatorio para la nueva fecha |
| MAN-NOT-05 | Ya ejecutado el comando real | Ejecutar `--dry-run` | No escribe nada; solo candidatos que el real generaría |
| MAN-NOT-06 | `admin.demo` | Abrir `registro_envios?desde=31/12/2025` | Respuesta controlada, nunca 500 |
| MAN-NOT-07 | SMTP configurado para fallar | Disparar notificación | La app no revienta; notificación en campana y registro `Fallido` |

### 5.9 Configuración y despliegue

| ID | Precondición | Pasos | Resultado esperado |
|---|---|---|---|
| MAN-CFG-01 | Entorno de despliegue | Verificar `DJANGO_SETTINGS_MODULE=config.settings.production` y `DEBUG=False` | Settings de producción y DEBUG falso |
| MAN-CFG-02 | Entorno sin `SECRET_KEY` | Arrancar la app | Debe fallar rápido, sin default |
| MAN-CFG-03 | Producción | Revisar `ALLOWED_HOSTS` y `CSRF_TRUSTED_ORIGINS` | Solo hosts y orígenes HTTPS reales |
| MAN-CFG-04 | Edge proxy | Enviar `X-Forwarded-Proto` manipulado | El proxy lo reescribe; no se toma encabezado directo |
| MAN-CFG-05 | PDF legal de otra empresa | Acceder a `/media/...` sin autorización | Denegado |
| MAN-CFG-06 | PDF mayor al límite | POST de carga | Rechazado por Django y proxy |
| MAN-CFG-07 | Imagen de producción | Inspeccionar `Dockerfile` | Usuario no root, WSGI/ASGI, sin `runserver` ni migraciones automáticas |
| MAN-CFG-08 | Logs | Revisar registros tras login fallido y acceso denegado | No contienen contraseñas, tokens ni PII |

---

## 6. Puertas de calidad recomendadas para CI

```yaml
name: Calidad y seguridad

on:
  push:
  pull_request:

permissions:
  contents: read

jobs:
  qa-security-gates:
    runs-on: ubuntu-latest
    timeout-minutes: 30

    services:
      postgres:
        image: postgres:18
        env:
          POSTGRES_USER: retosean
          POSTGRES_PASSWORD: retosean-ci
          POSTGRES_DB: retosean_django
        ports:
          - 5432:5432
        options: >-
          --health-cmd "pg_isready -U retosean"
          --health-interval 5s
          --health-timeout 5s
          --health-retries 10

    env:
      DJANGO_SETTINGS_MODULE: config.settings.development
      SECRET_KEY: ci-only-not-a-secret
      DEBUG: "False"
      ALLOWED_HOSTS: 127.0.0.1,localhost,testserver
      DB_NAME: retosean_django
      DB_USER: retosean
      DB_PASSWORD: retosean-ci
      DB_HOST: localhost
      DB_PORT: "5432"

    steps:
      - uses: actions/checkout@v4

      - uses: actions/setup-python@v5
        with:
          python-version: "3.14"
          cache: pip

      - name: Instalar app y herramientas QA
        run: |
          python -m pip install --upgrade pip
          pip install -r requirements.txt coverage ruff bandit pip-audit

      - name: Ruff lint
        run: |
          ruff check . --output-format=github
          ruff format --check .

      - name: Bandit security lint
        run: |
          bandit -r . -x .git,.venv,staticfiles,media,htmlcov -ll

      - name: pip-audit dependency vulnerabilities
        run: |
          pip-audit -r requirements.txt --strict

      - name: Django migration check
        run: |
          python manage.py makemigrations --check --dry-run
          python manage.py migrate --noinput

      - name: Production-like Django deploy check
        env:
          DJANGO_SETTINGS_MODULE: config.settings.production
          ALLOWED_HOSTS: testserver
          CSRF_TRUSTED_ORIGINS: https://testserver
        run: |
          python manage.py check --deploy

      - name: Coverage gate
        run: |
          coverage run manage.py test --noinput
          coverage report --fail-under=80

      - name: Setup Rust
        if: hashFiles('rust/Cargo.toml') != ''
        uses: dtolnay/rust-toolchain@stable

      - name: Cargo tests
        if: hashFiles('rust/Cargo.toml') != ''
        working-directory: rust
        run: |
          cargo test --all-features
```

---

## 7. Checklist previo a producción

1. Usar `DJANGO_SETTINGS_MODULE=config.settings.production` y `DEBUG=False`.
2. `SECRET_KEY` cargada desde gestor de secretos, sin fallback y rotada.
3. `ALLOWED_HOSTS` contiene solo los hostnames reales de producción.
4. `CSRF_TRUSTED_ORIGINS` contiene solo orígenes HTTPS explícitos y sin wildcards.
5. TLS terminado correctamente: el edge proxy define `X-Forwarded-Proto`, elimina cabeceras del cliente y redirige HTTP a HTTPS.
6. HSTS, `SESSION_COOKIE_SECURE`, `CSRF_COOKIE_SECURE` y cookies HttpOnly activados.
7. `/media/` no es navegable públicamente y los PDF confidenciales exigen autorización, URLs firmadas o almacenamiento privado.
8. Límites de carga activados en Django y reverse proxy; PDFs sobredimensionados rechazados.
9. Validadores de contraseña activos y rate limiting/MFA en login y admin.
10. Contenedor de producción no root, usa WSGI/ASGI y no ejecuta `runserver` ni migraciones automáticas en `CMD`.
11. SMTP autenticado por TLS, no consola/Mailpit, y `SITE_URL` HTTPS para enlaces de restablecimiento y certificados.
12. Logs sin contraseñas, tokens ni PII; `USE_TZ=True` y fechas UTC renderizadas para `America/Bogota`.

---

## Apéndice A — Hallazgos descartados (REFUTADOS)

| Hallazgo | Motivo del descarte |
|---|---|
| Redirección abierta vía `next` usando `request.get_host()` | `get_host()` ya está validado contra `ALLOWED_HOSTS`. |
| Construcción de ruta de carga poco segura para NIT/extensión | Django 6.0.6 `FileSystemStorage` rechaza `..` con `SuspiciousFileOperation`; el NIT no escapa de `MEDIA_ROOT` y `unique_together` previene colisión real. |
| Notificación de revisión asume `empresa.usuario` no nulo | `notificar()` retorna `None` si `usuario is None`, sin causar 500. |
| `finalizar` puede lanzar `AttributeError` si un entregable no tiene estudiante | `Entregable.estudiante` es FK no nula con `on_delete=CASCADE`; el escenario no es persistible. |
| N+1 probable en listado académico del profesor | La plantilla no accede a `integ.reto.empresa`; los campos usados están cubiertos por `select_related("reto")`. |
| Portafolio PDF puede fallar si un entregable no tiene fecha de entrega | `fecha_entrega` es `DateTimeField(auto_now_add=True)` no nula. |
| Los reportes de retos generan N+1 mediante `.count()` sobre relaciones prefetch | Django 6.0.6 usa el cache de prefetch y no emite `COUNT(*)` adicional. |
| Todo el contexto de compilación se copia en la imagen | Ya existe `.dockerignore` con `.env`, `media/`, `staticfiles/`, `.git/`, `venv/` y `*.sqlite3`. |

---

## Apéndice B — Metodología

Auditoría por módulo con 9 revisores en paralelo (modelos DeepSeek V4 Pro, Kimi K3.1 Pro, Qwen 3.8 Max). Verificación cruzada de cada hallazgo por un modelo distinto con acceso de lectura al repositorio (`opencode`). Revisión final manual de los hallazgos altos. Para `usuarios`, los veredictos provienen de revisión senior, con hallazgos no verificados listados separadamente.