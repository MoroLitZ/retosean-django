# Decisiones de diseño

Registro de las decisiones tomadas donde el brief del proyecto era ambiguo, se
contradecía o donde nos apartamos de él. Cada entrada dice qué se decidió y por qué.

## Plataforma

**Django con plantillas del servidor, no React + Express + Prisma.**
El brief propone ese stack "salvo que haya una razón para cambiarlo". Cuando se
revisó el brief, las 17 historias de usuario ya estaban construidas en Django
(14 apps, más de 100 rutas). Reescribir en otro stack habría significado empezar
de cero sin ganar funcionalidad. Se mantiene Django.

**El código está en español.** El brief pide identificadores en inglés. El código
existente usa español en modelos, vistas y plantillas; renombrar todo rompería
migraciones y el trabajo en curso de varias ramas. Se acepta español en el código;
los mensajes de commit nuevos van en inglés.

**PostgreSQL es la única base de datos.** No hay configuración SQLite. La base del
proyecto es `retosean_django`; el rol necesita `CREATEDB` para que
`manage.py test` cree la base temporal. La integración continua
(`.github/workflows/tests.yml`) corre contra `postgres:18`.

## Ciclo de vida (brief, sección 3)

Todas las transiciones pasan por una máquina de estados por entidad
(`apps/seguimiento/services.py`, `MaquinaEstados`); una transición inválida lanza
`TransicionInvalida` y no guarda nada.

**Reto / hackathon**

| Desde | Hacia |
|---|---|
| Borrador | En revisión, Cancelado |
| En revisión | Aprobado, Rechazado, Cancelado |
| Rechazado | Borrador (al editar), Cancelado |
| Aprobado | Abierto, Cancelado |
| Abierto | En curso, Cancelado |
| En curso | Pausado, Finalizado, Cancelado |
| Pausado | En curso, Cancelado |

- **Abierto** se alcanza cuando el profesor publica una integración académica ya
  aprobada (el brief: "el profesor lo configuró y el admin aprobó la
  configuración"). Solo los retos abiertos aparecen en el explorador y reciben
  postulaciones.
- **Finalizado** solo desde En curso y solo por el cierre formal (HU08), que emite
  encuestas y certificados. El cambio manual de estado del administrador ofrece
  únicamente En curso, Pausado y Cancelado.
- **Consecutivo** separado por tipo y año: `RETO-2026-001`, `HACK-2026-001`, con
  restricción única en base de datos. Al migrar se renumeraron los retos que ya
  tenían número (antes era una secuencia global).
- **Eliminar** un reto es borrado lógico (`eliminado_en`) y solo en Borrador
  (ambigüedad 3 del brief).

**Documento de empresa:** Pendiente → Cargado → Verificado | Rechazado; Rechazado →
Cargado al recargar. Se agregó **Vencido** (lo marca el comando de recordatorios) y
se permite recargar un documento verificado o vencido para actualizarlo.

**Convenio:** No radicado → Radicado → En Jurídica → Firmado, con fecha de
vencimiento obligatoria al firmar. Extras: No radicado → Firmado (la empresa ya llega
con convenio vigente: el "Sí" del brief), En Jurídica → Radicado (Jurídica devuelve
el trámite) y Firmado → Radicado (renovación).

**Postulación:** el brief dice Enviada → En revisión → Aceptada | Rechazada. Se usa
Pendiente (= Enviada) → En revisión → Aceptada | Rechazada, más **Retirada** (el
estudiante desiste mientras no hay respuesta) y Retirada → Pendiente al reenviar.
Pasa a En revisión cuando la empresa abre su lista de postulaciones.

**Entregable:** el brief dice Pendiente → Entregado → Evaluado. Se usa Enviado →
En revisión (el profesor lo abre) → Calificado. "Pendiente" (aún no entregado) no
existe como fila: un entregable se crea al subirlo. Reemplazar el archivo lo
devuelve a Enviado.

**Inscripción a hackathon:** Pendiente → Aceptada | Rechazada; el gestor puede
corregir su decisión (Aceptada ↔ Rechazada).

**Auditoría:** cada cambio de estado de documentos, convenio, bloqueo de empresa,
integraciones, postulaciones, entregables, inscripciones y cuentas (activa/inactiva,
rol) queda en `seguimiento.HistorialEstado` (quién, cuándo, de, a, comentario). Los
retos conservan su tabla propia `retos.HistorialEstadoReto`, que ya muestra el
detalle del reto; unificar ambas queda pendiente.

## HU00 — Documentación de la empresa

- **Los 8 documentos del brief** (ambigüedad 1) forman un catálogo administrable
  (`TipoDocumentoLegal`, menú "Tipos de documento"): el administrador decide cuáles
  son obligatorios, su vigencia y si requieren permiso especial.
- **Los documentos son por empresa, no por reto.** El brief dice "por
  empresa/reto"; la ficha del reto, los términos de referencia y el formato de
  compromiso podrían pedirse por reto más adelante.
- **"Participar sin convenio" ya no omite la verificación.** Antes una empresa que
  elegía esa opción podía publicar sin ningún documento. Ahora solo deja de
  exigírsele lo propio del convenio (minuta y convenio firmado); el resto es
  obligatorio, como pide el brief ("solo se aprueba la participación con todos los
  documentos verificados").
- **Listas Informa** la carga la universidad, no la empresa, y solo un usuario con
  el permiso `empresas.cargar_listas_informa`.
- Solo se aceptan **PDF**. La vigencia del tipo (Cámara de Comercio: 30 días) valida
  la fecha de expedición y fija el vencimiento.
- El administrador puede **bloquear** la participación de una empresa con un
  motivo; queda en el historial y se notifica a la empresa.
- El administrador **no puede aprobar** un reto de una empresa que no esté
  habilitada.

## HU13 — Usuarios

- Solo se **inactiva**, no se elimina (ambigüedad 4). Django rechaza en cada
  petición a un usuario inactivo, así que sus sesiones abiertas dejan de funcionar
  de inmediato.
- Los **permisos por módulo** son permisos de Django que el administrador asigna
  desde "Permisos especiales"; solo se pueden asignar los registrados en
  `apps/usuarios/permisos.py`.
- **Correo de bienvenida** al registrarse y en la importación masiva. Las cuentas
  importadas sin contraseña reciben un enlace firmado para definirla, porque
  "olvidé mi contraseña" ignora las cuentas sin contraseña utilizable.

## HU11 / HU12

- La hoja de vida es opcional (PDF, máximo 5 MB); aceptar términos es obligatorio.
- Una nota no la ve el estudiante hasta que el profesor **publica** las
  calificaciones del reto; recalificar exige confirmación y deja la nota sin
  publicar otra vez. Las notas existentes al migrar quedaron publicadas.

## Otros puntos del brief

- **Presupuesto (ambigüedad 5):** se agregó el monto asignado por reto.
- **Certificados (ambigüedad 6):** se implementan certificados; el portafolio PDF
  existe como extra. El certificado de empresa muestra su razón social.
- **Ecosistema (ambigüedad 7):** catálogo administrado, como facultades y programas.
