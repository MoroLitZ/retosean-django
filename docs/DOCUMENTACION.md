# RetosEAN — Documentación oficial

Plataforma web de la Universidad EAN para gestionar el ciclo de vida de los retos académicos entre
empresas, profesores, estudiantes, expertos y administradores. Versión actual: servidor nativo en
Rust (Axum, SQLx, PostgreSQL 18) con la interfaz web embebida en el mismo binario.

Este documento tiene dos partes:

- **Parte I. Manual de usuario** (secciones 1 a 10): cómo usar la plataforma según el rol, sin
  conocimientos técnicos.
- **Parte II. Guía para desarrolladores y operación** (secciones 11 a 24): arquitectura,
  configuración, API, modelo de dominio, seguridad, pruebas, despliegue y rendimiento.

Las cifras técnicas (rutas, pruebas, límites, mediciones) se verificaron contra el código del
commit en el que se publicó este documento; si cambian, actualice la sección correspondiente y
`docs/QA_SEGURIDAD_RUST.md` / `docs/MEMORIA_RUST.md`.

# Parte I. Manual de usuario

## 1. Qué es RetosEAN

RetosEAN es la plataforma de la Universidad EAN para conectar a la comunidad académica con empresas y organizaciones mediante retos, integraciones académicas y hackathones. Los participantes pueden tener uno de estos roles:

- **Administrador**: gestiona usuarios, empresas, documentos legales, retos, catálogos, presupuestos, reportes, cierres, hackathones y notificaciones.
- **Empresa**: propone retos, envía la documentación legal, revisa postulaciones, hace seguimiento y participa en los cierres.
- **Profesor**: integra retos a sus cursos, conforma equipos, configura cronogramas, evalúa entregables, crea rúbricas y gestiona cierres académicos.
- **Estudiante**: explora retos, se postula, entrega archivos, consulta su equipo y cronograma, responde encuestas y descarga certificados.
- **Experto (jurado)**: participa como jurado en hackathones, emite votos y consulta resultados.

### Ciclo de vida de un reto

Un reto nace como una idea de la empresa, se revisa, se abre a la comunidad, se ejecuta y se cierra formalmente. Durante ese recorrido puede pasar por nueve estados:

| Estado | Significado en lenguaje sencillo |
|---|---|
| **Borrador** | La empresa está construyendo o ajustando la información del reto. Aún no se envía a revisión. |
| **En revisión** | La empresa ya envió el reto y el administrador debe aprobarlo o rechazarlo. |
| **Aprobado** | El administrador validó el reto. Está listo para continuar su apertura y ejecución. |
| **Rechazado** | El administrador devolvió el reto a la empresa para que lo corrija. |
| **Abierto** | El reto está publicado y se reciben postulaciones. |
| **En curso** | El reto ya tiene participantes y se está ejecutando. |
| **Pausado** | El reto está temporalmente suspendido. |
| **Finalizado** | El reto terminó y se realizó su cierre formal. |
| **Cancelado** | El reto fue anulado y ya no continuará. |

## 2. Acceso a la plataforma

### 2.1. Iniciar sesión

1. Abra la plataforma. Si no hay sesión activa, verá el título **Bienvenido de vuelta** y el subtítulo **Ingresa tus credenciales para acceder al sistema**.
2. Diligencie los siguientes campos:

| Campo | Obligatorio | Notas |
|---|---|---|
| **Usuario o correo** | Sí | Máximo 150 caracteres. |
| **Contraseña** | Sí | Máximo 1024 caracteres. Puede mostrarse u ocultarse con el botón correspondiente. |

3. Pulse **Ingresar al Sistema**.
4. Si deja un campo obligatorio vacío, el sistema mostrará **Este campo es obligatorio.**.
5. Si los datos no son correctos, verá el mensaje devuelto por el servidor. Si hay un error general, puede mostrarse **No se pudo completar la operación**.
6. Si no hay conexión con el servidor, verá **No se pudo conectar con el servidor. Revisa tu conexión e intenta de nuevo.**

En la misma pantalla encontrará:

- **¿Olvidaste tu contraseña?**, para recuperar el acceso.
- **¿No tienes una cuenta?** y el enlace **Regístrate aquí**, para crear una cuenta.

### 2.2. Registro público

1. Use el enlace **Regístrate aquí**.
2. El sistema muestra el título **Crea tu cuenta** y el subtítulo **Elige cómo vas a participar en RetosEAN**.
3. Seleccione una de estas opciones:

| Opción | Descripción mostrada |
|---|---|
| **Comunidad EAN** | *Si eres Estudiante o Profesor de la Universidad EAN, usa esta opción para validar tu correo institucional.* |
| **Empresas / Organizaciones** | *Si eres un Aliado Comercial externo y deseas proponer retos o validar documentación legal, crea tu perfil corporativo aquí.* |

Si ya tiene cuenta, pulse **Inicia sesión aquí**.

#### 2.2.1. Registro de la Comunidad EAN

1. La pantalla se titula **Registro Comunidad EAN** y muestra el subtítulo **Usa tu correo institucional @universidadean.edu.co**.
2. Complete los campos:

| Campo | Obligatorio | Notas |
|---|---|---|
| **Rol** | Sí | Opciones: **Estudiante** o **Profesor**. |
| **Nombre** | Sí | — |
| **Apellido** | Sí | — |
| **Programa** | Sí para **Estudiante** | Se muestra con la opción inicial **Seleccione un programa**. Las opciones se cargan dinámicamente. |
| **Semestre** | No | Visible para **Estudiante**; opciones **1** a **10**. |
| **Facultad** | Sí para **Profesor** | Se muestra con la opción inicial **Seleccione una facultad**. |
| **Especialidad** | Sí para **Profesor** | Se muestra con la opción inicial **Seleccione una especialidad**. |
| **Usuario** | Sí | — |
| **Correo electrónico** | Sí | Si el formato no es válido: **Ingresa un correo válido.** |
| **Contraseña** | Sí | La pista indica **Mínimo 12 caracteres.** |
| **Confirmar contraseña** | Sí | Si no coincide: **Las contraseñas no coinciden.** |

3. Pulse **Crear cuenta**.

Después del envío puede aparecer:

- **Cuenta pendiente de aprobación**, cuando la cuenta queda en espera de aprobación.
- **Revisa tu correo**, cuando es necesario confirmar el correo electrónico.

#### 2.2.2. Registro de Empresa

1. La pantalla se titula **Registro de Empresa** y muestra el subtítulo **Crea el perfil corporativo de tu organización**.
2. Complete los campos:

| Campo | Obligatorio | Notas |
|---|---|---|
| **NIT de la Empresa** | Sí | — |
| **Razón social** | Sí | — |
| **Sector industrial** | No | — |
| **Tipo de convocatoria** | No | Opciones dinámicas. |
| **Área responsable** | No | — |
| **Sitio web** | No | Debe ser una URL. |
| **Usuario Corporativo** | Sí | — |
| **Correo electrónico corporativo** | Sí | Si el formato no es válido: **Ingresa un correo válido.** |
| **Contraseña** | Sí | La pista indica **Mínimo 12 caracteres.** |
| **Confirmar contraseña** | Sí | Si no coincide: **Las contraseñas no coinciden.** |

3. Pulse **Crear cuenta**.

Después del envío puede aparecer **Cuenta pendiente de aprobación** o **Revisa tu correo**.

### 2.3. Verificar el correo

1. Abra el enlace de verificación que llegó a su correo. El enlace es de un solo uso y tiene una vigencia de 3 días.
2. Si la verificación es exitosa, verá:

- Título: **Cuenta verificada**.
- Mensaje: **Tu correo fue confirmado. Ya puedes iniciar sesión en RetosEAN.**
- Enlace de retorno: **Inicia sesión aquí**.

3. Si el enlace no es válido o ya fue usado, verá:

- Título: **Enlace no válido**.
- Mensaje: **El enlace de verificación no es válido o ya fue usado.**, o el mensaje devuelto por el servidor.
- Enlace de retorno: **Inicia sesión aquí**.

### 2.4. Recuperar la contraseña

1. En el inicio de sesión pulse **¿Olvidaste tu contraseña?**.
2. La pantalla mostrará el título **Recuperar contraseña** y el subtítulo **Te enviaremos un enlace para elegir una nueva contraseña**.
3. Escriba un **Correo electrónico** válido.
4. Pulse **Enviar enlace**.
5. Si todo es correcto, verá **Revisa tu correo** con el mensaje **Si existe una cuenta con ese correo, te enviamos un enlace para restablecer la contraseña. El enlace vence en 3 días.**

### 2.5. Restablecer la contraseña

1. Abra el enlace de restablecimiento recibido por correo.
2. Verá el título **Elige una nueva contraseña** y el subtítulo **Usa al menos 12 caracteres**.
3. Diligencie:

| Campo | Obligatorio | Notas |
|---|---|---|
| **Nueva contraseña** | Sí | Debe tener al menos 12 caracteres. Si no: **La contraseña debe tener al menos 12 caracteres.** |
| **Confirmar contraseña** | Sí | Si no coincide: **Las contraseñas no coinciden.** |

4. Pulse **Cambiar contraseña**.
5. Si fue exitoso, verá **Contraseña actualizada** y el mensaje **Ya puedes iniciar sesión con tu nueva contraseña.**

### 2.6. Cerrar sesión

Puede cerrar sesión de dos formas:

- Desde la barra superior, con el botón **Salir**.
- Desde el menú lateral, en la sección **Cuenta**, con el botón **Cerrar Sesión**.

Al cerrar sesión, la plataforma regresa al inicio de sesión y limpia los datos visibles de la sesión anterior.

### 2.7. Sesión expirada

Las sesiones tienen una duración de 8 horas. Si la sesión deja de ser válida, la plataforma regresa automáticamente al inicio de sesión, oculta la aplicación y limpia los datos de la sesión anterior.

### 2.8. Mensajes frecuentes de acceso

| Situación | Mensaje |
|---|---|
| Campo obligatorio vacío | **Este campo es obligatorio.** |
| Correo con formato inválido | **Ingresa un correo válido.** |
| Contraseña de menos de 12 caracteres al restablecer | **La contraseña debe tener al menos 12 caracteres.** |
| Confirmación de contraseña distinta | **Las contraseñas no coinciden.** |
| No hay conexión con el servidor | **No se pudo conectar con el servidor. Revisa tu conexión e intenta de nuevo.** |
| Error general de operación | **No se pudo completar la operación.** |

## 3. Elementos comunes

### 3.1. Menú lateral por rol

El menú lateral cambia según el rol. Solo se muestran las opciones disponibles para cada tipo de usuario.

Al final de todos los menús aparece la sección **Cuenta** con **Mi Perfil** y **Cerrar Sesión**. Si están disponibles, también se agregan **Hackathones** y **Notificaciones**, esta última con contador de no leídas.

#### Administrador

| Sección | Opciones |
|---|---|
| **Administración** | **Dashboard**, **Usuarios**, **Empresas**, **Tipos de documento**, **Registro de notificaciones**, **Retos**, **Vinculaciones**, **Cierre de Retos**, **Reportes**, **Presupuesto** |
| **Gestión Académica** | **Unidades de Estudio** |

#### Empresa

| Sección | Opciones |
|---|---|
| **Mi Empresa** | **Dashboard**, **Publicar Reto**, **Mis Retos**, **Vinculaciones**, **Postulaciones**, **Documentos Legales**, **Seguimiento**, **Cierre de Retos**, **Mis Encuestas** |

#### Profesor

| Sección | Opciones |
|---|---|
| **Docencia** | **Dashboard**, **Mis Cursos**, **Integrar Reto**, **Mis Estudiantes**, **Entregables**, **Evaluaciones**, **Cierre de Retos**, **Mis Encuestas**, **Unidades de Estudio** |

#### Estudiante

| Sección | Opciones |
|---|---|
| **Mi Espacio** | **Dashboard**, **Explorar Retos**, **Mis Postulaciones**, **Mis Favoritos**, **Mis Entregables**, **Mis Encuestas**, **Certificados**, **Mis Equipos y Retos** |

#### Experto (jurado)

| Sección | Opciones |
|---|---|
| **Jurado** | **Dashboard** |

### 3.2. Campana de notificaciones

1. En la barra superior, pulse el botón de notificaciones. Su etiqueta accesible es **Notificaciones** si no hay no leídas, o **Notificaciones: X sin leer** cuando hay notificaciones sin leer.
2. El menú muestra el título **Notificaciones** y hasta 8 notificaciones propias.
3. Cada notificación incluye título, mensaje y fecha en formato `dd/mm/yyyy hh:mm`.
4. Si no tiene notificaciones, verá **Sin notificaciones**.
5. Use **Ver todas** para abrir el listado completo.

### 3.3. Búsqueda y paginación

En los listados de registros:

1. Escriba en el campo de búsqueda, con texto de ejemplo **Buscar…**.
2. Pulse **Buscar**.
3. El total se muestra como **N registro** o **N registros**, según corresponda.
4. Use **Anterior** y **Siguiente** para navegar. La página actual se muestra como **Página X de Y**. Cada página incluye 20 registros.

Si una búsqueda no produce resultados, verá:

- **Sin resultados**
- **Ningún registro coincide con tu búsqueda.**
- Botón **Limpiar búsqueda**.

Si una sección no tiene registros, verá:

- **No hay registros**
- **Aún no hay registros en esta sección.**

### 3.4. Ventana de detalle

1. En cada fila de un listado, pulse **Ver detalle**.
2. El cuadro de detalle muestra **Cargando…** mientras consulta la información.
3. Al abrirse, muestra los campos del registro. Los estados aparecen como insignias.
4. Los valores vacíos se muestran como **—**.
5. Los valores de sí/no se muestran como **Sí** o **No**.
6. Los campos de archivo permiten usar **Descargar archivo**.
7. Cierre el detalle con el botón de cierre ( **Cerrar** ) o con la tecla Escape.

### 3.5. Exportar a CSV, PDF o Excel

En los listados, los botones **CSV**, **PDF** y **Excel** permiten exportar la información. Están disponibles para el administrador.

Las exportaciones tienen un máximo de 5.000 registros.

### 3.6. Mi perfil y cambio de contraseña

1. Abra **Mi Perfil** desde el menú lateral.
2. Actualice los datos según su rol.

Campos comunes:

| Etiqueta | Aplica a |
|---|---|
| **Nombre** | Todos |
| **Apellido** | Todos |
| **Correo** | Todos |
| **Teléfono** | Todos |

Campos por rol:

| Etiqueta | Aplica a |
|---|---|
| **Carrera / Programa** | **Estudiante** |
| **Semestre** | **Estudiante** |
| **Facultad** | **Profesor** |
| **Especialidad** | **Profesor** |
| **Razón Social** | **Empresa** |
| **Sector Industrial** | **Empresa** |
| **Tipo de convocatoria** | **Empresa** |
| **Área responsable** | **Empresa** |
| **Sitio web** | **Empresa** |

Para **Tipo de convocatoria**, las opciones son **Reto**, **Hackathon** y **Reto y hackathon**.

#### Cambio de contraseña

1. Dentro de **Mi Perfil**, pulse **Cambiar contraseña**.
2. Verá el mensaje **Deberás iniciar sesión nuevamente después del cambio.**
3. Complete:

| Campo | Obligatorio | Notas |
|---|---|---|
| **Contraseña actual** | Sí | — |
| **Contraseña nueva (mínimo 12 caracteres)** | Sí | Debe tener al menos 12 caracteres. |

4. Guarde el cambio. Después deberá iniciar sesión de nuevo.

### 3.7. Preferencias de notificación

1. En su registro de preferencias, use la acción **Editar preferencias**.
2. Diligencie:

| Campo | Opciones |
|---|---|
| **Recibir correo** | **Sí** / **No** |
| **Recibir recordatorios** | **Sí** / **No** |
| **Eventos silenciados (separados por coma)** | Texto libre |

### 3.8. Avisos y mensajes del sistema

Las operaciones exitosas o fallidas muestran avisos emergentes:

- **Cambios guardados**, cuando la operación se guardó correctamente.
- **Elige un archivo para continuar.**, cuando falta un archivo requerido.
- **El contenido enviado es demasiado grande.**, cuando la solicitud supera el límite permitido.
- **Respuesta inesperada del servidor.**, cuando no se puede leer una respuesta.
- **No se pudo conectar con el servidor. Revisa tu conexión e intenta de nuevo.**, cuando no hay conexión.
- **No se pudo completar la operación**, cuando ocurre un error general.

## 4. Guía para la empresa

### 4.1. Dashboard de la empresa

En **Dashboard**, la empresa puede consultar indicadores como:

- Mis retos
- Activos
- En revisión
- Finalizados
- Postulaciones
- Pendientes
- Aceptadas
- Entregables

También puede ver las gráficas **Mis retos por estado** y **Retos publicados por mes**.

### 4.2. Publicar un reto

1. En el menú lateral, pulse **Publicar Reto**.
2. Diligencie:

| Campo | Obligatorio | Notas |
|---|---|---|
| **Título** | Sí | Máximo 180 caracteres. |
| **Tipo** | Solo al crear | Opciones: **Reto** o **Hackathon**. |
| **Área** | Sí | Máximo 120 caracteres. Valor por defecto: **Tecnología**. |
| **Descripción** | Sí | — |
| **Nivel académico** | No | Valor por defecto: **Pregrado**. |
| **Inicio tentativo** | Sí | Fecha. |
| **Fin tentativo** | Sí | Fecha. |
| **Límite de postulación** | Sí | Fecha. |

3. Pulse **Guardar borrador**.

El reto queda en estado **Borrador**.

### 4.3. Editar, adjuntar soporte y eliminar un borrador

Abra el reto desde **Mis Retos**.

- Si el reto está en **Borrador** o **Rechazado**, puede usar **Editar reto**.
- Si está en **Borrador**, puede usar **Enviar a revisión**.
- Si es la empresa propietaria y el reto está en **Borrador** o **Rechazado**, puede usar **Adjuntar archivos de soporte**. Seleccione uno o varios archivos y pulse **Cargar archivos**.
- Si es la empresa propietaria y el reto está en **Borrador**, puede usar **Eliminar reto**. Aparecerá la confirmación **¿Eliminar este reto en borrador?** con el botón **Eliminar**.

### 4.4. Enviar un reto a revisión

1. Abra el reto en **Borrador** desde **Mis Retos**.
2. Use la acción **Enviar a revisión**.
3. Si se solicita, escriba un **Motivo / comentario** y pulse **Confirmar**.

El reto pasa a **En revisión**.

### 4.5. Cargar documentos legales

1. En el menú lateral, pulse **Documentos Legales**.
2. Use la acción **Cargar documento**.
3. Diligencie:

| Campo | Obligatorio | Notas |
|---|---|---|
| **Fecha de expedición** | Sí | — |
| **Documento PDF** | Sí | Máximo 9 MB. Solo PDF. |

4. Pulse **Cargar documento**.

Si el envío es exitoso, verá **Documento enviado a revisión**.

Posibles validaciones:

- **Selecciona un archivo.**
- **Formato no admitido. Usa: .pdf**
- **El archivo supera el límite de 9 MB.**

### 4.6. Revisar postulaciones

1. En el menú lateral, pulse **Postulaciones**.
2. Abra una postulación con **Ver detalle**.
3. Si la postulación está **Pendiente**, puede usar **Aceptar** o **Rechazar**.

### 4.7. Hacer seguimiento y consultar avance

1. En el menú lateral, pulse **Seguimiento**.
2. Abra el reto correspondiente.
3. Use **Registrar seguimiento**.
4. Diligencie:

| Campo | Obligatorio | Notas |
|---|---|---|
| **Tipo de sesión** | No | **Seguimiento**, **Inicio**, **Preselección**, **Evaluación**, **Reconocimiento**, **Otro**. |
| **Fecha** | No | — |
| **Avance (%)** | No | Valor entre 0 y 100. |
| **Avances** | No | — |
| **Observaciones** | No | — |
| **Acuerdos** | No | — |
| **Archivos** | No | Múltiples. Máximo 50 MB por archivo. Formatos admitidos: `.pdf`, `.png`, `.jpg`, `.jpeg`, `.txt`, `.xlsx`, `.zip`, `.docx`, `.pptx`, `.csv`. |

5. Pulse **Registrar seguimiento**.

Si la operación es exitosa, verá **Seguimiento registrado.**

Para consultar el avance, use **Ver avance**. Allí puede ver el porcentaje del último seguimiento, las sesiones y la lista de entregables. Para comentar un entregable, use **Comentario para el equipo** y pulse **Comentar**.

### 4.8. Gestionar el cierre

En un reto autorizado, use **Gestionar cierre**. Allí podrá registrar:

- Agenda y acta del cierre.
- Entregables finales.
- Reconocimientos.
- Solicitudes de logística.

Consulte los campos de estas secciones en la guía del profesor, sección **Cierre del reto**.

### 4.9. Encuestas de satisfacción

1. En el menú lateral, pulse **Mis Encuestas**.
2. Abra la encuesta pendiente.
3. Use **Responder encuesta**.

### 4.10. Participar sin convenio

Si la empresa decide no firmar convenio, puede usar la acción **Participar sin convenio**. El sistema mostrará:

- **Has seleccionado participar sin convenio.**
- **Participarás sin convenio: no necesitas la minuta ni el convenio firmado. Carga el resto de documentos para poder publicar retos.**

## 5. Guía para el profesor

### 5.1. Dashboard del profesor

En **Dashboard**, el profesor puede consultar:

- Cursos activos
- Retos asignados
- Mis estudiantes
- Por calificar
- Entregables recibidos
- Integraciones en revisión
- Nota promedio

También puede ver las gráficas **Entregables por estado** y **Entregables por reto**.

### 5.2. Integrar un reto a un curso

1. En el menú lateral, pulse **Integrar Reto**.
2. Abra un reto disponible.
3. Si el reto está en **Aprobado**, **Abierto**, **En curso** o **Pausado**, use **Integrar a mi curso**.
4. Si necesita conformar equipo, use **Crear equipo**.

En la integración académica puede diligenciar:

| Campo | Obligatorio | Notas |
|---|---|---|
| **Facultad** | No | Catálogo. |
| **Nivel de formación** | No | Catálogo. |
| **Unidad de estudio** | No | — |
| **Ecosistema** | No | Catálogo. |
| **Descripción** | No | — |
| **Alcance** | No | — |
| **Entregable esperado** | No | Máximo 180 caracteres. |
| **Espacio físico** | No | Máximo 180 caracteres. |
| **Notas del cronograma** | No | — |
| **Requerimientos a la empresa** | No | — |
| **Requerimientos internos** | No | — |

Puede guardar con:

- **Guardar borrador**
- **Guardar y enviar a revisión**

Mensajes posibles:

- **Integración académica guardada como borrador.**
- **Integración enviada a aprobación del administrador.**

En **Mis Cursos**, el profesor propietario puede:

- **Editar integración**
- **Editar equipo y cronograma**
- **Enviar a revisión**, si está en **Borrador**
- **Publicar integración**, si está **Aprobada**

### 5.3. Configurar equipo y cronograma

1. Abra la acción **Editar equipo y cronograma** o el formulario **Equipo y cronograma**.
2. Diligencie los integrantes.

Campos del equipo:

| Campo | Notas |
|---|---|
| **Rol** | **Profesor**, **Estudiante**, **Experto invitado**. |
| **Nombre** | — |
| **Cédula / documento** | — |
| **Correo** | — |
| **Facultad u organización** | — |
| **Programa** | — |

Campos del cronograma:

| Campo | Notas |
|---|---|
| **Tipo** | **Inicio**, **Seguimiento**, **Preselección**, **Evaluación**. |
| **Fecha y hora** | — |
| **Enlace** | URL. |
| **Observaciones** | — |

3. Pulse **Guardar equipo y cronograma**.

### 5.4. Estudiantes, entregables y evaluaciones

#### Mis estudiantes y entregables

- **Mis Estudiantes** muestra el listado de estudiantes.
- **Entregables** muestra los entregables recibidos.

#### Calificar un entregable

1. Abra el entregable con **Ver detalle**.
2. Si no está calificado, use **Calificar**.
3. Si ya está calificado, use **Volver a calificar**.
4. Diligencie el formulario **Evaluar entregable**:

| Campo | Obligatorio | Notas |
|---|---|---|
| **Forma de calificar** | Sí | **Puntaje libre** o una rúbrica. |
| **Nota** | Sí, si eligió puntaje libre | De 0 al puntaje máximo del entregable. |
| **Criterios** | Sí, si eligió rúbrica | Cada criterio con su puntaje máximo. |
| **Retroalimentación** | No | — |
| **Confirmar recalificación** | No | Solo si el entregable ya estaba calificado. |

5. Pulse **Guardar evaluación**.

Mientras no publique notas, el sistema muestra: **La nota queda sin publicar hasta que publiques las calificaciones del reto.**

Para publicar, use **Publicar calificaciones del reto** cuando el entregable esté calificado y aún no publicado.

### 5.5. Crear rúbricas

1. En el reto autorizado, use **Crear rúbrica**.
2. En el formulario **Nueva rúbrica**, complete:

| Campo | Obligatorio | Notas |
|---|---|---|
| **Nombre** | Sí | — |
| **Descripción** | No | — |
| **Criterios** | No | Filas editables con **Nombre** y **Puntaje máximo**. |

3. Pulse **Crear rúbrica**.

### 5.6. Seguimiento y avance

El profesor puede usar **Registrar seguimiento** y **Ver avance**.

El panel **Avance del reto** muestra:

- El porcentaje del último seguimiento.
- Tabla de sesiones.
- Entregables con estado y comentarios.
- Campo **Comentario para el equipo** y botón **Comentar**.

Al comentar, el mensaje de éxito es **Comentario agregado al historial.**

Los campos de seguimiento son los mismos indicados en la guía de la empresa, sección **Hacer seguimiento y consultar avance**.

### 5.7. Cierre del reto

En un reto que lo permita, el profesor puede usar:

- **Gestionar cierre**
- **Cerrar y emitir certificados**, cuando el reto está **En curso**

#### Gestionar cierre

Se abre el diálogo **Gestión del cierre**, con estas secciones:

##### Agenda y acta

| Campo | Obligatorio | Notas |
|---|---|---|
| **Fecha y hora** | No | — |
| **Espacio** | No | — |
| **Recursos** | No | — |
| **Invitados** | No | — |
| **Agenda** | No | — |
| **Notas** | No | — |
| **¿La empresa aplicó la solución?** | No | **Sí**, **No**, **En evaluación**, **Sin registrar**. |
| **Acta de cierre (PDF)** | No | Máximo 9 MB. |

Botón: **Guardar documentación y agenda**.

Si ya existe un acta, puede usar **Descargar acta actual**.

##### Entregables finales

| Campo | Obligatorio | Notas |
|---|---|---|
| **Nombre** | Sí | — |
| **Archivo (PDF, PNG o JPG)** | Sí | Máximo 9 MB. |

Botón: **Cargar entregable final**.

##### Reconocimientos

| Campo | Obligatorio | Notas |
|---|---|---|
| **Estudiante** | No | Selección opcional. |
| **Destinatario** | Sí | — |
| **Descripción** | Sí | — |

Botón: **Registrar reconocimiento**.

##### Solicitudes de logística

| Campo | Obligatorio | Notas |
|---|---|---|
| **Evento** | No | **Evento de inicio** u **Evento de cierre**. |
| **Tipo de solicitud** | No | **Espacio físico**, **Apoyo del área de comunicaciones**, **Refrigerios**, **Equipos audiovisuales**, **Invitados**, **Otro**. |
| **Detalle** | No | — |

Botón: **Solicitar logística**.

El administrador puede **Confirmar**, **Descartar** o **Reabrir** una solicitud.

#### Cerrar y emitir certificados

En un reto en **En curso**, use **Cerrar y emitir certificados**. El formulario **Cierre formal** incluye:

| Campo | Obligatorio | Notas |
|---|---|---|
| **Notas del cierre** | No | Texto. |
| **Horas certificadas** | No | Número. |

### 5.8. Encuestas

En **Mis Encuestas**, el profesor puede abrir una encuesta pendiente y usar **Responder encuesta**.

## 6. Guía para el estudiante

### 6.1. Dashboard del estudiante

En **Dashboard**, el estudiante puede consultar:

- Retos disponibles
- Mis postulaciones
- Aceptadas
- Favoritos
- Entregables
- Sin calificar
- Nota promedio

También puede ver las gráficas **Mis postulaciones por estado** y **Mis notas recientes**.

### 6.2. Explorar retos y favoritos

1. En el menú lateral, pulse **Explorar Retos**.
2. El listado muestra columnas: **Título**, **Empresa**, **Área**, **Estado**, **Fecha límite**.
3. Abra un reto con **Ver detalle**.
4. Use **Alternar favorito** para guardarlo o quitarlo de **Mis Favoritos**.

En **Mis Favoritos**, el listado muestra **Reto** y **Agregado**.

### 6.3. Postularse a un reto

1. Abra un reto en estado **Abierto**.
2. Use **Postularme**.
3. Complete:

| Campo | Obligatorio | Notas |
|---|---|---|
| **¿Por qué quieres participar?** | Sí | — |
| **Habilidades** | No | — |
| **Programa académico** | No | Catálogo. |
| **Semestre** | Sí | Opciones de **1** a **12**. |
| **Hoja de vida o portafolio (PDF, opcional)** | No | Máximo 5 MB. |
| **Acepto los términos y condiciones...** | Sí | Casilla de verificación. |

4. Pulse **Enviar postulación**.

Si es exitoso, verá: **¡Tu postulación fue enviada! La empresa fue notificada y revisará tu solicitud.**

### 6.4. Mis postulaciones

En **Mis Postulaciones**, el listado muestra **Reto**, **Fecha de Postulación** y **Estado**.

Si una postulación propia está **Pendiente**, puede usar **Retirar postulación**.

### 6.5. Entregar archivos

1. Abra un reto en estado **Abierto** o **En curso**.
2. Use **Entregar archivo**.
3. Complete el formulario **Entregar un archivo**:

| Campo | Obligatorio | Notas |
|---|---|---|
| **Título del entregable** | Sí | Máximo 180 caracteres. |
| **Comentario** | No | — |
| **Archivo** | Sí | PDF, PNG, JPG o TXT. Máximo 9 MB. |

4. Pulse **Enviar entregable**.

Si es exitoso, verá **Archivo entregado**.

En **Mis Entregables**, el listado muestra **Título**, **Reto**, **Fecha entrega**, **Estado** y **Nota**.

### 6.6. Cronograma de sesiones

Consulte el panel **Cronograma de sesiones** para ver la programación. La tabla muestra:

- **Reto**
- **Sesión**
- **Fecha y hora**
- **Enlace**

### 6.7. Equipos y retos

En **Mis Equipos y Retos**, el listado muestra **Equipo** y **Reto**.

### 6.8. Certificados

1. En el menú lateral, pulse **Certificados**.
2. El listado muestra **Reto**, **Participante**, **Rol**, **Horas** y **Emitido**.
3. Para un certificado puede usar **Verificar certificado** y **Descargar certificado PDF**.
4. También puede usar **Descargar mi portafolio (PDF)**.

Al verificar un certificado válido, verá un mensaje con el formato **Certificado válido: …**.

### 6.9. Encuestas

En **Mis Encuestas**, el listado muestra **Participante**, **Calificación** y **Respondida**. Si la encuesta está pendiente, use **Responder encuesta**.

### 6.10. Participar en hackathones

Si un hackathon está publicado o en curso, el estudiante puede usar **Inscribir mi equipo** desde el detalle del hackathon.

## 7. Guía para el experto (jurado)

### 7.1. Panel del jurado

El menú del experto incluye **Jurado** y su opción principal **Dashboard**.

En el panel puede ver:

- Hackathones asignados
- En curso
- Equipos por evaluar
- Votos emitidos
- Puntaje promedio
- Equipos académicos

También puede ver la gráfica **Votos por hackathon**.

### 7.2. Votar como jurado

1. Ingrese al detalle del hackathon asignado.
2. Si el hackathon está **Publicado** o **En curso**, use **Votar como jurado**.
3. Registre su voto según la evaluación del evento.

### 7.3. Consultar resultados

Cuando los resultados están publicados, o si tiene acceso como administrador, puede usar **Ver ranking**. El ranking se muestra con el título **Ranking del hackathon**.

## 8. Guía para el administrador

### 8.1. Dashboard e indicadores

1. Pulse **Dashboard**. El título de la página es **Dashboard de Indicadores**.
2. Seleccione el **Periodo**: **Últimos 30 días**, **Últimos 90 días**, **Último año**, **Histórico completo** o **Rango personalizado**.
3. Si elige **Rango personalizado**, indique **Desde** y **Hasta**.
4. Como administrador, puede filtrar por **Facultad**, **Programa** y **Ecosistema**.
5. Pulse **Aplicar**.
6. El sistema muestra **Mostrando:** seguido del periodo seleccionado.

Indicadores visibles para administrador:

- Retos totales
- Retos activos
- Finalizados
- En revisión
- Empresas
- Estudiantes
- Docentes
- Postulaciones
- Solución aplicada
- No aplicada
- Impacto en evaluación

Gráficas:

- Retos por estado
- Retos por facultad
- Retos creados por mes
- Integraciones académicas por estado
- Impacto

### 8.2. Gestión de usuarios

1. En el menú lateral, pulse **Usuarios**.
2. El listado muestra **Usuario**, **Nombre**, **Apellido**, **Correo**, **Rol**, **Registro** y **Activo**.
3. Use **Crear usuario** para crear una cuenta manualmente.

#### Crear usuario

| Campo | Obligatorio | Notas |
|---|---|---|
| **Usuario** | Sí | Texto. |
| **Contraseña inicial (mínimo 12 caracteres)** | No | Debe tener al menos 12 caracteres. |
| **Correo** | No | Formato de correo. |
| **Nombre** | No | — |
| **Apellido** | No | — |
| **Rol** | No | **ESTUDIANTE**, **PROFESOR**, **EMPRESA**, **EXPERTO**. |
| **NIT (solo empresa)** | No | Visible para rol **EMPRESA**. |
| **Razón social (solo empresa)** | No | Visible para rol **EMPRESA**. |

#### Activar, desactivar, cambiar rol y permisos

En el detalle de un usuario:

- Use **Desactivar usuario** o **Activar usuario** para inhabilitar o habilitar una cuenta.
- Use **Cambiar rol**; en el formulario seleccione el **Nuevo rol**:
  - **ADMIN**
  - **EMPRESA**
  - **PROFESOR**
  - **ESTUDIANTE**
  - **EXPERTO**
- Use **Permisos especiales** para abrir el formulario de permisos. Marque o desmarque permisos y pulse **Guardar permisos**.

#### Importar usuarios

1. En **Usuarios**, pulse **Importar usuarios**.
2. Pulse **Descargar plantilla CSV** para obtener el formato.
3. Seleccione un archivo `.csv`, `.xlsx` o `.xlsm`.
4. Pulse **Previsualizar**.
5. Revise la previsualización.
6. Pulse **Confirmar importación**.

Las columnas de la plantilla son: `username`, `email`, `first_name`, `last_name`, `rol`, `telefono`, `password`.

Obligatorias: `username`, `email` y `rol`.

Si no se incluye contraseña, el sistema envía al usuario un enlace para definirla.

Mensaje de éxito esperado: **X usuario(s) creado(s) correctamente. Y fila(s) se omitieron por errores de validación.**

### 8.3. Gestión de empresas

1. En el menú lateral, pulse **Empresas**.
2. El listado muestra **Razón social**, **NIT**, **Sector**, **Validación** y **Convenio**.

En el detalle de una empresa, el administrador puede:

- **Revisar empresa**.
- **Tramitar convenio**.
- **Historial documental**.
- **Bloquear participación** o **Habilitar participación**.
- **Cargar Listas Informa**, si tiene el permiso correspondiente.

#### Revisar empresa

| Campo | Opciones |
|---|---|
| **Validación** | **PENDIENTE**, **EN_REVISION**, **VERIFICADA**, **RECHAZADA** |
| **Listas restrictivas** | **PENDIENTE**, **APROBADO**, **RECHAZADO** |
| **Observaciones** | Texto |

#### Tramitar convenio

| Campo | Opciones |
|---|---|
| **Nuevo estado** | **RADICADO**, **EN_JURIDICA**, **FIRMADO** |
| **Fecha de vencimiento** | Fecha |
| **Comentario** | Texto |

#### Bloquear participación

Al bloquear una empresa, complete el **Motivo del bloqueo**.

### 8.4. Documentos legales y tipos de documento

#### Tipos de documento

1. En el menú lateral, pulse **Tipos de documento**.
2. El listado muestra **Orden**, **Documento**, **Obligatorio**, **Vigencia (días)** y **Activo**.
3. Puede usar **Nuevo tipo de documento** o, en el detalle, **Editar tipo de documento**.

Formulario:

| Campo | Obligatorio | Notas |
|---|---|---|
| **Código (p. ej. POLIZA_RESPONSABILIDAD)** | Sí al crear | — |
| **Nombre** | Sí | — |
| **Descripción** | No | — |
| **Vigencia máxima (días desde la expedición)** | No | Numérico. |
| **Orden** | No | Numérico. Valor por defecto: 0. |
| **Obligatorio** | No | Casilla. |
| **Se exige también a empresas sin convenio** | No | Casilla. |
| **Solo lo carga un usuario con permiso especial** | No | Casilla. |
| **Activo** | No | Casilla. |

Pulse **Guardar**.

#### Revisar documento de empresa

En **Documentos Legales**, abra el documento y use **Revisar documento**.

#### Historial documental

El diálogo **Historial documental** permite consultar los cambios de documentos. La tabla incluye **Fecha**, **Documento**, **De**, **A**, **Por** y **Comentario**.

### 8.5. Aprobación y gestión de retos

1. En el menú lateral, pulse **Retos**.
2. El listado muestra **Título**, **Empresa**, **Estado**, **Consecutivo** y **Actualizado**.
3. Abra un reto con **Ver detalle**.

Acciones según el estado:

| Estado o condición | Acciones disponibles |
|---|---|
| **En revisión** | **Aprobar**, **Rechazar**. |
| **Abierto** o **Pausado** | **Iniciar / reanudar**. |
| **En curso** | **Pausar**. |
| No está **Finalizado** ni **Cancelado** | **Cancelar reto**. |
| Tipo **Hackathon** | **Crear hackathon**. |
| **Borrador** o **Rechazado** | **Adjuntar archivos de soporte**. |
| No está **Borrador** ni **Cancelado** | **Definir presupuesto**. |
| **Aprobado**, **Abierto**, **En curso**, **Pausado** o **Finalizado** | **Gestionar cierre**. |
| **En curso** | **Cerrar y emitir certificados**. |

#### Aprobar o rechazar retos

Al aprobar o rechazar, puede solicitarse un **Motivo / comentario**. Pulse **Confirmar**.

#### Definir presupuesto

| Campo | Obligatorio | Notas |
|---|---|---|
| **Monto total (COP)** | Sí | Número. |

#### Cierre formal

| Campo | Obligatorio | Notas |
|---|---|---|
| **Notas del cierre** | No | Texto. |
| **Horas certificadas** | No | Número. |

### 8.6. Vinculaciones e integraciones académicas

1. En el menú lateral, pulse **Vinculaciones**.
2. El listado muestra **Profesor**, **Reto**, **Programa**, **Estado** y **Enviada**.
3. Abra una integración en estado **En revisión**.
4. Use **Aprobar integración** o **Rechazar integración**.

- **Aprobar integración** solicita un **Comentario**.
- **Rechazar integración** solicita un **Motivo**.

### 8.7. Gestión de catálogos

El administrador puede gestionar **Facultades**, **Programas**, **Ecosistemas** y **Unidades de Estudio**.

En las vistas **Facultades**, **Programas**, **Ecosistemas** y **Unidades de Estudio**, el botón principal es **Agregar registro**.

#### Campos por catálogo

| Catálogo | Campos |
|---|---|
| **Facultades** | **Nombre**, **Código** |
| **Programas** | **Nombre**, **Facultad** |
| **Ecosistemas** | **Nombre**, **Descripción** |

#### Unidades de Estudio

Creación o edición:

| Campo | Obligatorio | Notas |
|---|---|---|
| **Código** | Sí | — |
| **Nombre de la unidad** | Sí | — |
| **Programa académico** | No | Incluye opción **Seleccione**. |
| **Periodo (ej: 2026-2)** | No | — |
| **Ciclo / Semestre** | No | — |
| **Archivo (PDF con info completa)** | No | En edición: **Reemplazar archivo (PDF)**. |

Botón: **Guardar**.

Si falta un archivo requerido, verá **Elige un archivo para continuar.**

#### Carga masiva CSV de unidades de estudio

1. En la ventana de creación, use la sección **Carga masiva (CSV)**.
2. El texto de ayuda indica: **Columnas: codigo, nombre, programa (nombre exacto del catálogo), periodo y ciclo opcionales.**
3. Seleccione el **Archivo CSV** con extensión `.csv`.
4. Pulse **Cargar CSV**.

#### Administración de unidades

- **Desactivar unidad** para una unidad activa.
- **Activar unidad** para una unidad inactiva.
- **Editar unidad** para modificar.
- **Mostrar unidades inactivas** para incluir las inactivas en el listado.

### 8.8. Constructor de reportes

1. En el menú lateral, pulse **Reportes**.
2. El título visible es **Generación de Informes y Reportes**.
3. Diligencie:

| Campo | Obligatorio | Notas |
|---|---|---|
| **Reporte** | Sí | Catálogo de reportes. |
| **Formato** | Sí | **PDF** o **Excel**. |
| **Periodo** | Sí | **Últimos 30 días**, **Últimos 90 días**, **Último año**, **Histórico completo**, **Rango personalizado**. |
| **Desde** / **Hasta** | Solo en **Rango personalizado** | Fechas. |
| **Filtros** | No | Facultad, Programa, Empresa, Estado del reto, Estado de postulación, Estado de la empresa. |
| **Variables a incluir** | No | Si no marca ninguna, se incluyen todas. |
| **Enviar a (correos separados por coma)** | No | Por defecto, se envía al correo del usuario. |

4. Use uno de los botones:

- **Vista previa**
- **Generar y descargar**
- **Generar y enviar por correo**

Validaciones posibles:

- **Indica la fecha de inicio.**
- **Indica la fecha final.**
- **La fecha final no puede ser anterior a la inicial.**
- **Indica al menos un correo.**
- **«X» no es un correo válido.**

Las exportaciones de reportes y listados tienen un máximo de 5.000 registros.

### 8.9. Presupuesto y gastos

1. En el menú lateral, pulse **Presupuesto**.
2. El listado muestra **Reto**, **Asignado** y **Actualizado**.
3. Abra un presupuesto con **Ver detalle**.

En el detalle se muestra un resumen:

**Comprometido: … · Ejecutado: … · Disponible: … · X% asignado**

Si hay alerta presupuestal, se agrega **· Umbral de alerta alcanzado**.

La tabla de gastos muestra:

- **Fecha**
- **Categoría**
- **Estado**
- **Descripción**
- **Monto**

Si no hay gastos, verá **Aún no se registran gastos.**

Acciones disponibles:

- **Registrar gasto**
- **Marcar ejecutado**, para gastos comprometidos
- **Eliminar**, con confirmación **¿Eliminar este gasto?** y botón **Eliminar**
- **Exportar gastos (Excel)**
- **Exportar gastos (PDF)**

### 8.10. Cierre de retos

1. En el menú lateral, pulse **Cierre de Retos**.
2. El listado muestra **Reto**, **Cerrado por**, **Fecha de cierre** y **Encuesta enviada**.
3. Abra un cierre con **Ver detalle**.

La gestión principal del cierre también se realiza desde el detalle del reto, con **Gestionar cierre** y **Cerrar y emitir certificados**.

### 8.11. Gestión de hackathones

1. En el detalle de un reto tipo **Hackathon**, use **Crear hackathon**.
2. El hackathon aparece en el menú **Hackathones**, si está disponible.

Acciones de gestión:

- **Logística e invitados**
- **Asignar jurado**
- **Configurar etapa**
- **Informe final PDF**
- **Informe final Excel**
- **Ver ranking**, para el administrador o cuando los resultados están publicados

#### Logística e invitados

Formulario:

| Campo | Obligatorio | Notas |
|---|---|---|
| **Modalidad** | No | **Presencial**, **Virtual**, **Híbrida**. |
| **Espacio físico** | No | — |
| **Enlace de transmisión** | No | — |
| **Notas de logística** | No | — |
| **Invitados** | No | Tipo (**Invitado**, **Mentor**, **Ponente**, **Patrocinador**), **Nombre**, **Organización**, **Correo**, **Confirmado**. |

Botón: **Guardar logística**.

#### Detalle del hackathon

El detalle muestra:

- Bloque **Logística del evento**.
- Línea **Modalidad: … · Espacio: …**. Si no hay espacio definido, se muestra **Por definir**.
- Si existe cierre programado y los resultados no están publicados: **Los resultados se publican automáticamente el …**.

Tabla de invitados:

- **Invitado**
- **Tipo**
- **Organización**
- **Confirmado**

Los valores de confirmación visibles son **Sí** y **Pendiente**.

Si no hay invitados: **Sin invitados.**

El ranking se abre con el título **Ranking del hackathon**. Si no hay votos:

- **Sin votos**
- **Todavía no se han registrado votos para este hackathon.**

### 8.12. Registro de notificaciones y bitácora

#### Registro de notificaciones

1. En el menú lateral, pulse **Registro de notificaciones**.
2. El listado muestra **Título**, **Tipo**, **Leída** y **Fecha**.
3. Si la notificación es propia y no está leída, puede usar **Marcar como leída**.

#### Registro de envíos

En la sección de notificaciones, el administrador puede usar **Registro de envíos**.

El diálogo incluye:

- Filtro **Estado**: **Todos**, **Enviado por correo**, **Fallido**, **Leído**, **No leído**, **Solo en la plataforma**.
- Resumen: Total, Enviadas por correo, Fallidas, Leídas.
- Tabla: **Fecha**, **Usuario**, **Evento**, **Título**, **Correo**, **Leída**.

#### Registro de actividad

Para consultar la bitácora:

1. Use **Registro de actividad** si está disponible.
2. Filtre por **Acción**, **Usuario**, **Desde** y **Hasta**.
3. Pulse **Filtrar**.

La tabla muestra **Fecha**, **Acción**, **Identificador**, **Usuario**, **Realizado por**, **IP** y **Detalle**.


## 9. Preguntas frecuentes y solución de problemas

### 9.1 No puedo iniciar sesión

**Respuesta:** El sistema rechaza el ingreso cuando el usuario o la contraseña no son correctos. Además, por seguridad, después de 10 intentos fallidos desde una misma cuenta y conexión, el acceso queda bloqueado temporalmente.

**Qué hacer:** Verifica que escribes bien tu usuario o correo y tu contraseña. Si ya tuviste varios intentos fallidos, espera 10 minutos sin intentarlo y vuelve a probar. Si olvidaste la contraseña, usa la opción de recuperación.

### 9.2 No me llegó el correo de verificación o de recuperación

**Respuesta:** Los correos pueden tardar unos minutos y, a veces, llegan a la carpeta de correo no deseado o spam. Los enlaces de verificación y de recuperación valen 3 días y son de un solo uso.

**Qué hacer:** Revisa la carpeta de spam o correo no deseado y confirma que escribiste bien tu correo. Si aun así no llega, solicita un nuevo enlace. Recuerda que el enlace vence a los 3 días y se invalida después de usarse.

### 9.3 La contraseña no es aceptada

**Respuesta:** Toda contraseña debe tener al menos 12 caracteres. Además, si debes confirmarla, debe coincidir exactamente con la primera vez que la escribiste.

**Qué hacer:** Usa una contraseña de 12 o más caracteres, sin espacios accidentales al copiarla, y escribe la misma contraseña en el campo de confirmación.

### 9.4 El enlace de verificación dice que no es válido

**Respuesta:** Ese mensaje aparece cuando el enlace ya venció, ya fue usado o está incompleto. Los enlaces valen 3 días y solo se pueden usar una vez.

**Qué hacer:** Solicita un nuevo enlace de verificación o contacta al administrador. No intentes abrir varias veces el mismo enlace.

### 9.5 No puedo publicar un reto

**Respuesta:** Las empresas solo pueden publicar retos cuando sus documentos legales están vigentes y aprobados. El sistema no permite publicar si hay documentos vencidos, rechazados o si la empresa está bloqueada. La Cámara de Comercio vale 30 días desde su expedición.

**Qué hacer:** Revisa el estado de tus documentos legales. Renueva y vuelve a cargar los que estén vencidos o rechazados. Si todo está al día y aun así no puedes publicar, contacta al administrador.

### 9.6 ¿Cuáles son los límites de archivos?

**Respuesta:** En general, cada archivo puede pesar hasta 50 MB. Puedes adjuntar hasta 10 archivos por envío, con un máximo total de 100 MB por envío. Algunos documentos, como documentos legales o entregables finales, solo aceptan PDF, PNG o JPG y tienen un límite de 9 MB.

**Qué hacer:** Revisa el límite indicado en cada formulario. Si un archivo pesa demasiado, redúcelo o conviértelo al formato indicado. Si el total es muy grande, reparte los adjuntos en varios envíos.

### 9.7 Rechazaron mi archivo aunque la extensión es correcta

**Respuesta:** El sistema revisa el contenido real del archivo, no solo su terminación. Por ejemplo, un archivo HTML que fue renombrado para terminar en .pdf no se acepta como PDF.

**Qué hacer:** Genera o exporta el archivo en el formato correcto desde el programa de origen y vuelve a intentarlo. No cambies la terminación del archivo de forma manual.

### 9.8 La exportación o el reporte responde que el servidor está ocupado

**Respuesta:** El sistema atiende pocas generaciones de exportaciones y reportes al mismo tiempo para no sobrecargarse. El mensaje indica que no hay un espacio disponible en ese momento.

**Qué hacer:** Espera unos segundos y vuelve a intentarlo. Si el mensaje continúa, espera un poco más y prueba de nuevo antes de contactar al administrador.

### 9.9 No veo un menú o un botón

**Respuesta:** La plataforma muestra menús y botones según el rol de cada persona y según el estado de cada reto o trámite. Por eso una opción puede no estar visible para todos.

**Qué hacer:** Revisa si tu rol debería tener esa opción y si el reto está en la etapa correspondiente. Si crees que deberías verla y no aparece, contacta al administrador.

### 9.10 Mi sesión se cerró sola

**Respuesta:** Por seguridad, cada sesión dura 8 horas. Después de ese tiempo, el sistema la cierra y pide iniciar sesión de nuevo.

**Qué hacer:** Vuelve a ingresar con tu usuario o correo y tu contraseña. Si necesitas conservar información, guarda o registra los datos importantes con frecuencia.

### 9.11 Las exportaciones muestran como máximo 5 000 registros

**Respuesta:** Es un límite del sistema para que las exportaciones se generen de forma estable. No significa que la información esté incompleta fuera de ese límite.

**Qué hacer:** Usa filtros o un periodo más corto para que el resultado tenga menos de 5 000 registros y no se corte.

### 9.12 Los valores que empiezan por = + - @ aparecen con un apóstrofo en Excel

**Respuesta:** Es una protección contra fórmulas. La plataforma agrega un apóstrofo al inicio de estos valores para evitar que Excel los ejecute como fórmulas.

**Qué hacer:** No es un error. Deja el valor como está. Si necesitas editarlo, hazlo con cuidado y solo si estás seguro de su contenido.

## 10. Glosario

| Término | Significado |
|---|---|
| Reto | Desafío o problema real propuesto por una empresa u organización para que la comunidad académica de la Universidad EAN proponga una solución. |
| Hackathon | Modalidad de reto de trabajo intensivo y colaborativo, normalmente por equipos, con evaluación de expertos y resultados al final. |
| Vinculación / integración académica | Articulación de un reto con una unidad de estudio o curso para que estudiantes y profesores trabajen en él como parte de su actividad académica. |
| Postulación | Solicitud de un estudiante o equipo para participar en un reto. |
| Equipo | Grupo de personas que participa o colabora en un reto o integración académica; puede incluir estudiantes, profesores y expertos invitados. |
| Entregable | Archivo o producto que un participante o equipo entrega como avance o como resultado final. |
| Rúbrica | Guía de evaluación con criterios y puntajes máximos para calificar un entregable. |
| Seguimiento | Registro de sesiones, porcentaje de avance, observaciones y acuerdos durante el desarrollo de un reto o integración académica. |
| Cierre | Etapa final de un reto en la que se documentan la agenda, el acta, los entregables finales, los reconocimientos y la logística. |
| Acta | Documento que deja constancia de lo realizado y de los resultados del cierre de un reto. |
| Reconocimiento | Mención o registro de agradecimiento para una persona o equipo que participó o aportó al reto. |
| Certificado | Constancia que se entrega a un participante cuando cumple los requisitos del reto o del proceso. |
| Portafolio | Conjunto de evidencias o trabajos de un estudiante; puede usarse como hoja de vida o muestra de logros. |
| Consecutivo (RETO-AAAA-NNN) | Código único que identifica cada reto, formado por la palabra RETO, el año y un número consecutivo. |
| Documento legal | Documento de una empresa que acredita el cumplimiento de requisitos legales; el sistema controla su vigencia y estado. |
| Cámara de Comercio | Documento que acredita la existencia y la matrícula de una empresa; en la plataforma tiene una validez de 30 días desde su expedición. |
| RUT | Registro Único Tributario, documento que identifica tributariamente a una empresa o persona en Colombia. |
| Listas Informa | Consulta en listas de control o de riesgo para verificar información, antecedentes o señales de alerta de una empresa o persona. |
| Convenio | Acuerdo entre la Universidad EAN y una empresa u organización para establecer las condiciones de colaboración en retos y actividades académicas. |
| Jurado / experto | Persona con conocimiento especializado que evalúa propuestas, equipos o entregables; en la plataforma corresponde al rol Experto, especialmente en hackathones. |
| Unidad de estudio | Curso, asignatura o unidad académica en la que se desarrolla una integración académica. |
| Ecosistema | Agrupación o categoría temática que organiza retos e integraciones académicas dentro de la institución. |
| Presupuesto | Recursos o fondos previstos para el desarrollo de un reto, integración académica o evento. |
| Gasto | Desembolso o uso de recursos registrado contra un presupuesto. |
| Notificación | Mensaje o aviso que recibe un usuario en la plataforma o por correo electrónico sobre eventos de su interés. |

# Parte II. Guía para desarrolladores y operación

## 11. Visión técnica

### 11.1 Stack

| Componente | Tecnología |
|---|---|
| Lenguaje | Rust, edición 2024 |
| Framework HTTP | Axum 0.8 |
| Runtime asíncrono | Tokio |
| Acceso a datos | SQLx 0.8 |
| Base de datos | PostgreSQL 18 |
| Frontend | SPA en HTML, CSS y JavaScript sin frameworks |
| Gráficas | Chart.js vendorizado en `web/chart.umd.min.js`, servido como `/assets/chart.js` |
| Binario | Único, con recursos frontend y SQL embebidos |

El frontend no requiere paso de compilación ni CDN. Todo se sirve desde la propia aplicación. La política CSP es:

```text
default-src 'self'; script-src 'self'; style-src 'self'; font-src 'self'; img-src 'self' data:; frame-ancestors 'none'; base-uri 'none'; form-action 'self'
```

### 11.2 Arquitectura de binario único

El paquete Cargo se llama `retosean` y genera el binario `retosean`. El binario incluye mediante `include_str!`:

- `../web/index.html` para `/`.
- Archivos CSS y JS de `../web/css` y `../web/js`.
- `../web/chart.umd.min.js` como `chart.js`.
- `../sql/schema.sql` para `init-db`.
- `../sql/demo.sql` para `seed-demo`.
- Migraciones SQL en `src/schema_migrations.rs`.

Perfil `release` definido en `Cargo.toml`:

```toml
lto = "fat"
codegen-units = 1
strip = "debuginfo"
```

`panic` no se declara, así que conserva el valor por defecto `unwind`: una petición que falla no detiene el servidor. No debe cambiarse a `abort`.

### 11.3 Diagrama textual de capas

```text
Cliente HTTP
   │
   ▼
security_headers
   │  (nosniff, DENY, referrer-policy, no-store, CSP)
   ▼
same_origin
   │  (rechaza escrituras cross-site, Origin nulo o inconsistente)
   ▼
DefaultBodyLimit global: 64 KB
   │  (límites mayores por ruta cuando se declaran explícitamente)
   ▼
Router Axum + handlers por área
   │
   ├── AppState
   │     ├── PgPool SQLx: 10 conexiones
   │     ├── Config
   │     ├── login_slots: semáforo de 4 permisos
   │     ├── password_slots: semáforo de 4 permisos
   │     └── heavy_slots: semáforo de 4 permisos
   │
   ├── PostgreSQL 18
   ├── Directorio de medios privado (`RUST_MEDIA_DIR`)
   └── Correo / outbox de notificaciones
```

### 11.4 Decisiones clave

| Decisión | Motivo |
|---|---|
| Binario único con SPA embebida | Sin build frontend, sin CDN, despliegue simple |
| Axum + Tokio + SQLx | Servicio HTTP asíncrono, pool PostgreSQL y consultas tipadas |
| PostgreSQL 18 como única base | Integridad relacional, transacciones, restricciones de estado y auditoría |
| Semáforos de concurrencia | Limitar hashing, login y operaciones pesadas para proteger CPU y memoria |
| Subidas por fragmentos a temporales `.part` | Evitar cargar cuerpos completos en memoria |
| Descargas por streaming | Reducir memoria en archivos grandes |
| Migraciones versionadas propias | `init-db` crea esquema completo; `migrate` aplica cambios incrementales |
| Paridad con la versión Django | Las suites `tests/parity_*.rs` reproducen el comportamiento anterior |
| Máquinas de estado explícitas | Transiciones auditadas y registradas en historial |
| Cabeceras de seguridad estrictas y CSP | Todo el contenido proviene del propio servidor |

---

## 12. Puesta en marcha local

### 12.1 Requisitos

| Requisito | Uso |
|---|---|
| Rust con soporte de edición 2024 | Compilar y ejecutar |
| PostgreSQL 18 | Base de datos |
| Node | Solo para `node --check` sobre `web/js/*.js`; no es necesario en tiempo de ejecución |

### 12.2 Archivo `.env`

El proceso busca `.env` en este orden:

1. `./.env`
2. `../.env`

Si el archivo no existe o falla su lectura, el proceso continúa. Ejemplo mínimo local:

```env
DB_HOST=localhost
DB_PORT=5432
DB_USER=retosean
DB_PASSWORD=contraseña-local
RUST_DB_NAME=retosean_rust
RUST_BIND=127.0.0.1:8080
RUST_LOG=retosean=info
```

Para Docker Compose es obligatorio definir además:

```env
RUST_POSTGRES_PASSWORD=contraseña-para-compose
```

### 12.3 Comandos básicos

```bash
# Crear base si no existe y esquema solo si public no tiene tablas
retosean init-db

# Aplicar migraciones pendientes
retosean migrate

# Crear datos demo solo si la tabla usuarios está vacía
retosean seed-demo

# Servidor HTTP; es el comando por defecto
retosean serve
```

Si se ejecuta desde el árbol de fuentes:

```bash
cargo run --locked -- init-db
cargo run --locked -- migrate
cargo run --locked -- seed-demo
cargo run --locked -- serve
```

### 12.4 `Start-Retos.ps1`

Script PowerShell para Windows:

```powershell
.\Start-Retos.ps1
.\Start-Retos.ps1 -Release
.\Start-Retos.ps1 -Initialize
.\Start-Retos.ps1 -Initialize -Release
```

Comportamiento:

- Busca `cargo.exe` en `~/.cargo/bin` o en el `PATH`.
- Cambia al directorio del script.
- Con `-Initialize`, ejecuta `init-db` y luego `seed-demo`.
- Luego ejecuta `serve`.
- Con `-Release`, usa compilación `--release`.
- Muestra:

```text
RetosEAN Rust: http://127.0.0.1:8080 (Ctrl+C to stop)
```

### 12.5 Cuentas demo

`seed-demo` solo se ejecuta si la tabla `usuarios` está vacía. Crea cuentas exclusivamente para demostración.

| Usuario | Rol |
|---|---|
| `admin.demo` | ADMIN |
| `empresa.verificada` | EMPRESA |
| `profesor.demo` | PROFESOR |
| `estudiante.demo` | ESTUDIANTE |
| `experto.demo` | EXPERTO |

Contraseña común:

```text
RetosEAN2026
```

---

## 13. Configuración

### 13.1 Variables de entorno principales

| Variable | Valor por defecto | Notas |
|---|---:|---|
| `RUST_DB_NAME` | `retosean_rust` | Debe tener 1–63 caracteres y contener solo letras ASCII, dígitos y guion bajo. Si es vacío o inválido, la carga falla. |
| `DB_HOST` | `localhost` | Host PostgreSQL. En Docker Compose se sobreescribe a `db`. |
| `DB_PORT` | `5432` | Puerto PostgreSQL. Debe ser válido. |
| `DB_USER` | `retosean` | Usuario PostgreSQL. |
| `DB_PASSWORD` | vacío | Contraseña PostgreSQL si no está definida `DB_PASSWORD_FILE`. Puede quedar visible en el entorno; preferir `DB_PASSWORD_FILE`. |
| `DB_PASSWORD_FILE` | sin valor por defecto | Ruta a un archivo con la contraseña. Tiene prioridad sobre `DB_PASSWORD`. Se recortan finales `\r` y `\n`. El archivo debe tener permisos restringidos. |
| `RUST_BIND` | `127.0.0.1:8080` | Dirección de escucha. La imagen Docker define `0.0.0.0:8080`. Exponer públicamente requiere proxy y endurecimiento adecuados. |
| `RUST_SECURE_COOKIE` | `true` | Si es `true`, las cookies de sesión llevan `Secure`. Si es distinto de `true`, queda `false` y se registra una advertencia. Solo debería desactivarse en desarrollo local HTTP plano. |
| `RUST_TRUST_PROXY` | `false` | Si es `true`, se asume un reverse proxy confiable. Puede usar cabeceras reenviadas, por ejemplo `X-Forwarded-Host`. Activar solo con proxy confiable; en caso contrario hay riesgo de spoofing. |
| `RUST_MEDIA_DIR` | `.rust-media` | Directorio privado de medios. En Docker se usa `/data/media`. Debe ser escribible para el proceso. |
| `RUST_SITE_URL` | `http://<RUST_BIND>` | URL base para enlaces de verificación y recuperación. Sin barra final. |
| `RUST_LOG` | `retosean=info` | Filtro de logs. `main.rs` usa `EnvFilter::try_from_default_env()` y, si falla, `retosean=info`. |

### 13.2 Variables de correo

| Variable | Comportamiento |
|---|---|
| `EMAIL_BACKEND` | Si contiene `console` o `locmem`, usa backend de consola. |
| `EMAIL_HOST` | Si falta, usa consola. |
| `EMAIL_PORT` | Por defecto `587`. |
| `EMAIL_HOST_USER` | Credencial SMTP si existe. |
| `EMAIL_HOST_PASSWORD` | Credencial SMTP si existe. |
| `EMAIL_USE_TLS` | Verdadero con `True`, `true` o `1`; por defecto `true`. |
| `DEFAULT_FROM_EMAIL` | Remitente; por defecto `RetosEAN <no-responder@universidadean.edu.co>`. |

Si SMTP no está configurado, se usa un backend de consola que registra los correos con tokens redactados.

### 13.3 Variable específica de Docker Compose

| Variable | Requisito |
|---|---|
| `RUST_POSTGRES_PASSWORD` | Obligatoria para Docker Compose. No tiene valor por defecto. Se entrega como Docker secret `db_password`. |

---

## 14. Comandos del binario

Uso general:

```bash
retosean [comando]
```

Si no se pasa comando, se ejecuta `serve`. Mensaje para comando desconocido:

```text
Commands: serve | init-db | migrate | seed-demo | import-django <db> <media_dir> | advance-hackathons | send-reminders [days...]
```

| Comando | Sintaxis | Efecto |
|---|---|---|
| `serve` | `retosean serve` o `retosean` | Servidor HTTP. Es el comando por defecto. |
| `init-db` | `retosean init-db` | Crea la base de datos si no existe. Crea el esquema solo si `public` no tiene tablas. Registra las migraciones como aplicadas mediante baseline. |
| `migrate` | `retosean migrate` | Aplica migraciones pendientes una sola vez y en orden. |
| `seed-demo` | `retosean seed-demo` | Crea datos demo solo si la tabla `usuarios` está vacía. |
| `import-django` | `retosean import-django <django_db_name> <django_media_dir>` | Importa una base Django existente y su `MEDIA_ROOT` hacia la base Rust. |
| `advance-hackathons` | `retosean advance-hackathons` | Avanza hackathons según calendario. Programar de forma horaria. |
| `send-reminders` | `retosean send-reminders [días...]` | Genera recordatorios. Si no se indican días, usa `7 3 1`. Programar diariamente. |

Detalles relevantes de `serve`:

1. Inicializa logging con filtro de entorno o `retosean=info`.
2. Carga configuración.
3. Crea pool PostgreSQL con 10 conexiones.
4. Inicializa autenticación.
5. Elimina temporales de subida huérfanos.
6. Instala correo y el worker de outbox.
7. Hace bind en `RUST_BIND`.
8. Construye `AppState` con semáforos de 4 permisos.
9. Sirve con Axum.
10. Espera apagado ordenado con Ctrl-C o `SIGTERM`.

---

## 15. Mapa del código

### 15.1 Módulos Rust en `src/`

| Archivo | Líneas | Responsabilidad |
|---|---:|---|
| `src/academic.rs` | 528 | Integraciones académicas: creación, edición, envío, revisión y publicación. |
| `src/accounts.rs` | 336 | Perfil, actividad y handlers de cuentas asociados a rutas `accounts::*`. |
| `src/auth.rs` | 499 | Autenticación, sesiones, contraseñas, CSRF y límites de login. |
| `src/builder.rs` | 984 | Constructor de reportes: preview, generación PDF/Excel, registro y envío por correo. |
| `src/catalogs.rs` | 68 | Creación de catálogos. |
| `src/closure.rs` | 421 | Cierre de retos: agenda, acta, entregables finales, reconocimientos y solicitudes de logística. |
| `src/companies.rs` | 563 | Empresas, documentos legales, revisión, historial documental y panel de avance. |
| `src/config.rs` | 78 | Carga de configuración desde entorno y `.env`. |
| `src/dashboard.rs` | 348 | Dashboard por rol, KPIs, series y filtro por periodo. |
| `src/domain.rs` | 318 | Validaciones de dominio, incluyendo política de contraseñas y reglas de campos. |
| `src/error.rs` | 61 | Tipos y respuestas de error. |
| `src/event.rs` | 534 | Avance programado de hackathons, logística de eventos e informe final exportable. |
| `src/finance.rs` | 614 | Presupuestos, gastos, ejecución, eliminación y exportaciones financieras. |
| `src/grading.rs` | 326 | Calificación libre o por rúbrica, recalificación confirmada y publicación de notas. |
| `src/hackathon.rs` | 891 | Hackathons, equipos, inscripciones, etapas, jurados, votos y ranking. |
| `src/import.rs` | 470 | Importación masiva de usuarios desde CSV o Excel con vista previa sin escritura. |
| `src/lib.rs` | 420 | Router Axum, assets embebidos, middlewares y límites de cuerpo. |
| `src/lifecycle.rs` | 406 | Seguimiento, cierre, encuestas y evolución del ciclo de vida de retos. |
| `src/mail.rs` | 500 | Correo de notificaciones, mailer por `EMAIL_*`, outbox y redacción de logs. |
| `src/main.rs` | 199 | CLI, comando `serve`, apagado ordenado y arranque de subsistemas. |
| `src/media.rs` | 903 | Subidas, temporales `.part`, validación por bytes mágicos, descargas y limpieza. |
| `src/migrate.rs` | 288 | `import-django`: copia de base y medios desde una instalación Django. |
| `src/models.rs` | 606 | Soporte de modelos y datos. |
| `src/permissions.rs` | 393 | Roles, permisos por módulo, bloqueo de participación y gate operativo de empresas. |
| `src/registration.rs` | 486 | Autorregistro, verificación de correo y recuperación de contraseña. |
| `src/reminders.rs` | 189 | Recordatorios diarios con deduplicación. |
| `src/reports.rs` | 858 | Exportaciones, certificados y portafolio. |
| `src/resources.rs` | 411 | Recursos genéricos, listado de registros, detalle, exportaciones y verificación de certificados. |
| `src/schema_migrations.rs` | 100 | Migraciones versionadas, baseline y ejecución serializada. |
| `src/study_units.rs` | 432 | Unidades de estudio: creación, edición, activación/desactivación y carga CSV. |
| `src/team.rs` | 311 | Equipo y cronograma de integraciones académicas. |
| `src/workflows.rs` | 998 | Retos, postulaciones, favoritos, estados, notificaciones y preferencias. |

### 15.2 Frontend embebido: `web/js`

| Archivo | Líneas |
|---|---:|
| `web/js/auth.js` | 424 |
| `web/js/builder.js` | 320 |
| `web/js/core.js` | 556 |
| `web/js/dashboard.js` | 409 |
| `web/js/detail.js` | 666 |
| `web/js/evaluation.js` | 90 |
| `web/js/forms.js` | 1195 |
| `web/js/icons.js` | 107 |
| `web/js/main.js` | 356 |
| `web/js/records.js` | 332 |
| `web/js/ui.js` | 414 |
| `web/js/workflows.js` | 694 |

### 15.3 Frontend embebido: `web/css`

| Archivo | Líneas |
|---|---:|
| `web/css/auth.css` | 105 |
| `web/css/base.css` | 405 |
| `web/css/builder.css` | 94 |
| `web/css/components.css` | 1235 |
| `web/css/dashboard.css` | 185 |
| `web/css/detail.css` | 59 |
| `web/css/forms.css` | 55 |
| `web/css/records.css` | 39 |
| `web/css/shell.css` | 678 |
| `web/css/tokens.css` | 202 |
| `web/css/workflows.css` | 155 |

Además, el binario sirve `web/index.html` y `web/chart.umd.min.js` como `/assets/chart.js`.

---

## 16. API HTTP

### 16.1 Convenciones

- La API responde JSON, salvo `/`, `/assets/{file}` y descargas de archivos.
- Los errores usan la forma:

```json
{"error": "mensaje"}
```

- La sesión viaja en la cookie `retosean_session`.
- Las operaciones de escritura requieren el token CSRF en la cabecera:

```http
X-CSRF-Token: <token>
```

- El middleware `same_origin` rechaza escrituras desde otro sitio, `Origin` nulo o `Sec-Fetch-Site` incompatible.
- `src/lib.rs` declara **102 rutas** (`.route()`), que suman **108 combinaciones método + ruta** porque seis paths aceptan dos métodos (`/api/perfil`, `/api/usuarios/{id}/permisos`, `/api/integraciones/{id}/equipo`, `/api/retos/{id}/cierre`, `/api/equipos-academicos/{id}` y `/api/hackathons/{id}/logistica`).

### 16.2 Códigos de estado

| Código | Uso |
|---:|---|
| 400 | Validación de entrada, formato inválido o regla de negocio. |
| 401 | Sin sesión válida o sesión expirada. |
| 403 | CSRF inválido, origen cruzado, permiso insuficiente o acción prohibida. |
| 404 | Ruta no encontrada o recurso no visible. |
| 409 | Conflicto de unicidad o estado incompatible. |
| 413 | Cuerpo de petición excede el límite. |
| 429 | Límite de intentos superado o servidor ocupado en hashing/login. |
| 503 | Operación pesada sin permiso disponible dentro de la espera máxima. |

Ejemplo de fallback:

```json
{"error":"Ruta no encontrada"}
```

Ejemplo de rechazo de origen:

```json
{"error":"Solicitud de otro sitio rechazada"}
```

### 16.3 Límites de cuerpo

| Contexto | Límite |
|---|---:|
| Límite global por defecto | 64 KB, es decir 65,536 bytes |
| `/api/usuarios/importar` | 6 MB |
| `/api/retos/{id}/postular` | 6 MB |
| Mayoría de cargas, por ejemplo unidades, documentos, entregables y cierre | 10 MB |
| `/api/retos/{id}/seguimiento` | 110 MB |
| `/api/retos/{id}/archivos` | 110 MB |

### 16.4 Exportaciones

Las exportaciones CSV, PDF y Excel se generan por rol y tienen un máximo de **5,000 registros**. En CSV/XLSX se neutralizan fórmulas. Los PDF usan fuentes Helvetica con codificación Windows-1252.

### 16.5 Tabla de rutas por área

Convenciones de la columna `Auth`:

- `público`: no requiere sesión según el propósito de la ruta.
- `sesión`: requiere sesión de forma evidente.
- `ver handler`: la autorización se resuelve dentro del handler.

Cuando un path admite varios métodos, se indican juntos.

#### Sistema y assets

| Método(s) | Ruta | Handler | Límite | Auth |
|---|---|---|---:|---|
| GET | `/` | closure del router | 64 KB | público |
| GET | `/assets/{file}` | `asset` | 64 KB | público |
| GET | `/health` | `health` | 64 KB | público |

#### Sesión, registro y recuperación

| Método(s) | Ruta | Handler | Límite | Auth |
|---|---|---|---:|---|
| POST | `/api/login` | `auth::login` | 64 KB | público |
| POST | `/api/logout` | `auth::logout` | 64 KB | sesión |
| POST | `/api/registro` | `registration::register` | 64 KB | público |
| GET | `/api/registro/opciones` | `registration::options` | 64 KB | público |
| POST | `/api/registro/verificar` | `registration::verify` | 64 KB | público |
| POST | `/api/password/reset` | `registration::reset_request` | 64 KB | público |
| POST | `/api/password/reset/confirm` | `registration::reset_confirm` | 64 KB | público |
| GET | `/api/me` | `auth::me` | 64 KB | sesión |

#### Usuarios, perfil y administración

| Método(s) | Ruta | Handler | Límite | Auth |
|---|---|---|---:|---|
| POST | `/api/usuarios` | `accounts::create` | 64 KB | ver handler |
| POST | `/api/usuarios/importar` | `import::import` | 6 MB | ver handler |
| GET | `/api/usuarios/plantilla` | `import::template` | 64 KB | ver handler |
| GET | `/api/log-actividad` | `accounts::activity_log` | 64 KB | ver handler |
| GET, POST | `/api/perfil` | `accounts::show_profile` / `accounts::profile` | 64 KB | ver handler |
| POST | `/api/password` | `accounts::password` | 64 KB | ver handler |
| POST | `/api/usuarios/{id}/estado` | `workflows::user_status` | 64 KB | ver handler |
| POST | `/api/usuarios/{id}/rol` | `permissions::role` | 64 KB | ver handler |
| GET, POST | `/api/usuarios/{id}/permisos` | `permissions::show_permissions` / `permissions::permissions` | 64 KB | ver handler |

#### Catálogos, registros y exportaciones

| Método(s) | Ruta | Handler | Límite | Auth |
|---|---|---|---:|---|
| POST | `/api/catalogos/{resource}` | `catalogs::create` | 64 KB | ver handler |
| GET | `/api/resources` | `resources::catalog` | 64 KB | ver handler |
| GET | `/api/records/{resource}` | `resources::list` | 64 KB | ver handler |
| GET | `/api/records/{resource}/{id}` | `resources::detail` | 64 KB | ver handler |
| GET | `/api/export/{resource}` | `resources::export` | 64 KB | ver handler |
| GET | `/api/export/{resource}/{format}` | `reports::export` | 64 KB | ver handler |
| GET | `/api/certificados/{id}/pdf` | `reports::certificate` | 64 KB | ver handler |
| GET | `/api/portafolio` | `reports::portfolio` | 64 KB | ver handler |
| GET | `/api/verificar/{code}` | `resources::verify_certificate` | 64 KB | público |

#### Dashboard

| Método(s) | Ruta | Handler | Límite | Auth |
|---|---|---|---:|---|
| GET | `/api/dashboard` | `dashboard::panel` | 64 KB | ver handler |

#### Unidades de estudio

| Método(s) | Ruta | Handler | Límite | Auth |
|---|---|---|---:|---|
| POST | `/api/unidades` | `study_units::create` | 10 MB | ver handler |
| POST | `/api/unidades/{id}` | `study_units::edit` | 10 MB | ver handler |
| POST | `/api/unidades/{id}/activo` | `study_units::set_active` | 64 KB | ver handler |

#### Retos, postulaciones y notificaciones

| Método(s) | Ruta | Handler | Límite | Auth |
|---|---|---|---:|---|
| POST | `/api/retos` | `workflows::create_challenge` | 64 KB | ver handler |
| POST | `/api/retos/{id}` | `workflows::edit_challenge` | 64 KB | ver handler |
| POST | `/api/retos/{id}/estado` | `workflows::challenge_state` | 64 KB | ver handler |
| POST | `/api/retos/{id}/favorito` | `workflows::favorite` | 64 KB | ver handler |
| POST | `/api/retos/{id}/postular` | `workflows::apply` | 6 MB | ver handler |
| POST | `/api/postulaciones/{id}/estado` | `workflows::application_state` | 64 KB | ver handler |
| POST | `/api/notificaciones/{id}/leer` | `workflows::read_notification` | 64 KB | ver handler |
| POST | `/api/preferencias` | `workflows::preferences` | 64 KB | ver handler |
| GET | `/api/notificaciones/registro` | `workflows::delivery_log` | 64 KB | ver handler |
| GET | `/api/notificaciones/mias` | `workflows::my_notifications` | 64 KB | ver handler |
| POST | `/api/retos/{id}/eliminar` | `workflows::delete_challenge` | 64 KB | ver handler |

#### Empresas, documentos y convenios

| Método(s) | Ruta | Handler | Límite | Auth |
|---|---|---|---:|---|
| POST | `/api/empresas/{id}/bloqueo` | `permissions::block` | 64 KB | ver handler |
| GET | `/api/empresas/{id}/historial-documental` | `companies::document_history` | 64 KB | ver handler |
| POST | `/api/tipos-documento` | `companies::create_document_type` | 64 KB | ver handler |
| POST | `/api/tipos-documento/{id}` | `companies::edit_document_type` | 64 KB | ver handler |
| GET | `/api/retos/{id}/avance` | `companies::progress_panel` | 64 KB | ver handler |
| POST | `/api/entregables/{id}/comentario` | `companies::comment` | 64 KB | ver handler |
| POST | `/api/empresa/modo-convenio` | `permissions::agreement_choice` | 64 KB | ver handler |
| POST | `/api/empresa/documentos/{kind}/cargar` | `media::own_document` | 10 MB | ver handler |
| POST | `/api/empresas/{id}/documentos/{kind}/cargar` | `media::special_document` | 10 MB | ver handler |
| POST | `/api/empresas/{id}/revisar` | `companies::review` | 64 KB | ver handler |
| POST | `/api/empresas/{id}/convenio` | `companies::agreement` | 64 KB | ver handler |
| POST | `/api/documentos/{id}/revisar` | `companies::document` | 64 KB | ver handler |
| POST | `/api/documentos/{id}/cargar` | `media::company_document` | 10 MB | ver handler |

#### Presupuesto y gastos

| Método(s) | Ruta | Handler | Límite | Auth |
|---|---|---|---:|---|
| POST | `/api/retos/{id}/presupuesto` | `finance::set_budget` | 64 KB | ver handler |
| POST | `/api/presupuestos/{id}/gastos` | `finance::expense` | 64 KB | ver handler |
| GET | `/api/presupuestos/{id}/resumen` | `finance::summary` | 64 KB | ver handler |
| POST | `/api/gastos/{id}/ejecutar` | `finance::execute` | 64 KB | ver handler |
| POST | `/api/gastos/{id}/eliminar` | `finance::delete` | 64 KB | ver handler |
| GET | `/api/retos/{id}/presupuesto/{format}` | `finance::export` | 64 KB | ver handler |

#### Integración académica, equipos y calificaciones

| Método(s) | Ruta | Handler | Límite | Auth |
|---|---|---|---:|---|
| POST | `/api/retos/{id}/integrar` | `academic::integrate` | 64 KB | ver handler |
| GET | `/api/integraciones/opciones` | `academic::options` | 64 KB | ver handler |
| POST | `/api/integraciones/{id}` | `academic::edit` | 64 KB | ver handler |
| POST | `/api/integraciones/{id}/estado` | `academic::integration_state` | 64 KB | ver handler |
| GET, POST | `/api/integraciones/{id}/equipo` | `team::show` / `team::update` | 64 KB | ver handler |
| POST | `/api/entregables/{id}/calificar` | `grading::grade` | 64 KB | ver handler |
| POST | `/api/retos/{id}/entregar` | `media::submit` | 10 MB | ver handler |
| POST | `/api/retos/{id}/rubrica` | `grading::create_rubric` | 64 KB | ver handler |
| GET | `/api/entregables/{id}/rubricas` | `grading::rubrics` | 64 KB | ver handler |
| POST | `/api/retos/{id}/publicar-calificaciones` | `grading::publish` | 64 KB | ver handler |

#### Archivos, seguimiento y cierre de ciclo

| Método(s) | Ruta | Handler | Límite | Auth |
|---|---|---|---:|---|
| GET | `/api/archivo/{resource}/{id}/{field}` | `media::download` | 64 KB | ver handler |
| POST | `/api/retos/{id}/seguimiento` | `lifecycle::progress` | 110 MB | ver handler |
| POST | `/api/retos/{id}/archivos` | `media::reto_files` | 110 MB | ver handler |
| POST | `/api/retos/{id}/cerrar` | `lifecycle::close` | 64 KB | ver handler |
| POST | `/api/encuestas/{id}` | `lifecycle::survey` | 64 KB | ver handler |

#### Cierre formal

| Método(s) | Ruta | Handler | Límite | Auth |
|---|---|---|---:|---|
| GET, POST | `/api/retos/{id}/cierre` | `closure::show` / `closure::save` | 10 MB | ver handler |
| POST | `/api/retos/{id}/cierre/entregables` | `closure::artifact` | 10 MB | ver handler |
| POST | `/api/retos/{id}/cierre/reconocimientos` | `closure::recognition` | 64 KB | ver handler |
| POST | `/api/retos/{id}/solicitudes-logistica` | `closure::request` | 64 KB | ver handler |
| POST | `/api/solicitudes-logistica/{id}/estado` | `closure::request_state` | 64 KB | ver handler |

#### Hackathons y eventos

| Método(s) | Ruta | Handler | Límite | Auth |
|---|---|---|---:|---|
| POST | `/api/retos/{id}/equipo` | `hackathon::team` | 64 KB | ver handler |
| GET, POST | `/api/equipos-academicos/{id}` | `hackathon::show_team` / `hackathon::edit_team` | 64 KB | ver handler |
| POST | `/api/equipos-academicos/{id}/eliminar` | `hackathon::delete_team` | 64 KB | ver handler |
| GET | `/api/retos/{id}/opciones` | `hackathon::challenge_options` | 64 KB | ver handler |
| GET | `/api/hackathons/{id}/opciones` | `hackathon::options` | 64 KB | ver handler |
| POST | `/api/retos/{id}/hackathon` | `hackathon::create` | 64 KB | ver handler |
| POST | `/api/hackathons/{id}/estado` | `hackathon::state` | 64 KB | ver handler |
| POST | `/api/hackathons/{id}/etapa` | `hackathon::stage` | 64 KB | ver handler |
| POST | `/api/hackathons/{id}/jurado` | `hackathon::juror` | 64 KB | ver handler |
| POST | `/api/hackathons/{id}/inscribir` | `hackathon::register` | 64 KB | ver handler |
| POST | `/api/inscripciones/{id}/estado` | `hackathon::registration_state` | 64 KB | ver handler |
| POST | `/api/hackathons/{id}/votar` | `hackathon::vote` | 64 KB | ver handler |
| GET | `/api/hackathons/{id}/ranking` | `hackathon::ranking` | 64 KB | ver handler |
| GET, POST | `/api/hackathons/{id}/logistica` | `event::show_logistics` / `event::logistics` | 64 KB | ver handler |
| GET | `/api/hackathons/{id}/informe/{format}` | `event::report` | 64 KB | ver handler |

#### Constructor de reportes

| Método(s) | Ruta | Handler | Límite | Auth |
|---|---|---|---:|---|
| GET | `/api/reportes/catalogo` | `builder::catalog` | 64 KB | ver handler |
| POST | `/api/reportes/vista-previa` | `builder::preview` | 64 KB | ver handler |
| POST | `/api/reportes/generar` | `builder::download` | 64 KB | ver handler |
| POST | `/api/reportes/enviar` | `builder::send` | 64 KB | ver handler |

---

## 17. Modelo de dominio

### 17.1 Máquinas de estado

Hay **8 máquinas de estado**.

#### MAQUINA_RETO — «el reto»

| Estado | Etiqueta | Transiciones permitidas |
|---|---|---|
| `borrador` | Borrador | `cancelado`, `en_revision` |
| `en_revision` | En revisión | `aprobado`, `cancelado`, `rechazado` |
| `rechazado` | Rechazado | `borrador`, `cancelado` |
| `aprobado` | Aprobado | `abierto`, `cancelado` |
| `abierto` | Abierto | `cancelado`, `en_curso` |
| `en_curso` | En curso | `cancelado`, `finalizado`, `pausado` |
| `pausado` | Pausado | `cancelado`, `en_curso` |
| `finalizado` | Finalizado | — terminal |
| `cancelado` | Cancelado | — terminal |

#### MAQUINA_DOCUMENTO — «el documento»

| Estado | Etiqueta | Transiciones permitidas |
|---|---|---|
| `PENDIENTE` | Pendiente | `CARGADO`, `RECHAZADO` |
| `CARGADO` | En revisión | `CARGADO`, `RECHAZADO`, `VENCIDO`, `VERIFICADO` |
| `VERIFICADO` | Aprobado | `CARGADO`, `VENCIDO` |
| `RECHAZADO` | Rechazado | `CARGADO` |
| `VENCIDO` | Vencido | `CARGADO`, `RECHAZADO` |

#### MAQUINA_CONVENIO — «el convenio»

| Estado | Etiqueta | Transiciones permitidas |
|---|---|---|
| `NO_RADICADO` | No radicado | `FIRMADO`, `RADICADO` |
| `RADICADO` | Radicado | `EN_JURIDICA` |
| `EN_JURIDICA` | En proceso en Jurídica | `FIRMADO`, `RADICADO` |
| `FIRMADO` | Firmado por las partes | `RADICADO` |

#### MAQUINA_ENTREGABLE — «el entregable»

| Estado | Etiqueta | Transiciones permitidas |
|---|---|---|
| `ENVIADO` | Enviado | `CALIFICADO`, `ENVIADO`, `EN_REVISION` |
| `EN_REVISION` | En revisión | `CALIFICADO`, `ENVIADO` |
| `CALIFICADO` | Calificado | `CALIFICADO`, `ENVIADO` |

#### MAQUINA_INSCRIPCION — «la inscripción»

| Estado | Etiqueta | Transiciones permitidas |
|---|---|---|
| `PENDIENTE` | Pendiente | `ACEPTADA`, `RECHAZADA` |
| `ACEPTADA` | Aceptada | `RECHAZADA` |
| `RECHAZADA` | Rechazada | `ACEPTADA` |

#### MAQUINA_POSTULACION — «la postulación»

| Estado | Etiqueta | Transiciones permitidas |
|---|---|---|
| `PENDIENTE` | Pendiente de aprobación | `ACEPTADA`, `EN_REVISION`, `RECHAZADA`, `RETIRADA` |
| `EN_REVISION` | En revisión | `ACEPTADA`, `RECHAZADA`, `RETIRADA` |
| `RETIRADA` | Retirada | `PENDIENTE` |
| `ACEPTADA` | Aceptada | — terminal |
| `RECHAZADA` | Rechazada | — terminal |

#### MAQUINA_INTEGRACION — «la integración académica»

| Estado | Etiqueta | Transiciones permitidas |
|---|---|---|
| `borrador` | Borrador | `en_revision` |
| `en_revision` | En revisión | `aprobada`, `rechazada` |
| `rechazada` | Rechazada | `borrador` |
| `aprobada` | Aprobada | `publicada` |
| `publicada` | Publicada | — terminal |

#### MAQUINA_SOLICITUD_LOGISTICA — «la solicitud»

| Estado | Etiqueta | Transiciones permitidas |
|---|---|---|
| `SOLICITADA` | Solicitada | `CONFIRMADA`, `DESCARTADA` |
| `CONFIRMADA` | Confirmada | `SOLICITADA` |
| `DESCARTADA` | Descartada | `SOLICITADA` |

### 17.2 Eventos de notificación

Todos los eventos tienen `permite_email: true`.

| Código | Título | Tipo |
|---|---|---|
| `CERTIFICADO_DISPONIBLE` | Tu certificado está disponible | EXITO |
| `COMENTARIO_EMPRESA` | Comentario de la empresa a un entregable | INFO |
| `CONVENIO_ACTUALIZADO` | Avance en el trámite de tu convenio | INFO |
| `DOCUMENTO_CARGADO_ADMIN` | Documento legal cargado por una empresa | INFO |
| `DOCUMENTO_REVISADO` | Documentación revisada | INFO |
| `EMPRESA_LISTAS_RESTRICTIVAS` | Verificación en listas restrictivas | ERROR |
| `EMPRESA_PARTICIPACION` | Estado de participación de tu empresa | INFO |
| `ENCUESTA_PENDIENTE` | Encuesta de satisfacción | INFO |
| `ENTREGABLE_CALIFICADO` | Tu entregable fue calificado | EXITO |
| `ENTREGABLE_RECIBIDO` | Nuevo entregable recibido | INFO |
| `EQUIPO_ASIGNADO` | Fuiste asignado a un equipo | EXITO |
| `HACKATON_PUBLICADO` | Hackathon publicado | INFO |
| `HACKATON_RESULTADOS` | Resultados del hackathon | EXITO |
| `INTEGRACION_ENVIADA_REVISION` | Nueva postulación docente a un reto | INFO |
| `INTEGRACION_REVISADA` | Integración académica revisada | INFO |
| `LOGISTICA_SOLICITADA` | Nueva solicitud de logística | INFO |
| `POSTULACION_GESTIONADA` | Respuesta a tu postulación | INFO |
| `POSTULACION_NUEVA` | Nueva postulación | INFO |
| `POSTULACION_RECIBIDA` | Recibimos tu postulación | EXITO |
| `PRESUPUESTO_80` | Presupuesto por encima del 80 % | ADVERTENCIA |
| `RECORDATORIO_FECHA_LIMITE` | Recordatorio de fecha límite | ADVERTENCIA |
| `RETO_ENVIADO_REVISION` | Reto enviado a revisión | INFO |
| `RETO_ESTADO_CAMBIADO` | Cambio de estado de tu reto | INFO |
| `RETO_FINALIZADO` | Reto finalizado | EXITO |
| `RETO_SIN_ACTUALIZAR` | Reto vencido sin actualizaciones | ADVERTENCIA |

### 17.3 Roles y permisos

Roles oficiales:

| Rol | Notas |
|---|---|
| `ADMIN` | El rol ADMIN solo puede otorgarse o cambiarse por superusuario. Un administrador no puede cambiar su propio rol. |
| `EMPRESA` | Requerido explícitamente para elegir modo de convenio. |
| `PROFESOR` | Rol académico. |
| `ESTUDIANTE` | Rol académico. |
| `EXPERTO` | Rol de experto. |

Reglas de cambio de rol:

- Solo administrador o superusuario puede usar el endpoint de cambio de rol.
- El rol recibido debe ser uno de `ADMIN`, `EMPRESA`, `PROFESOR`, `ESTUDIANTE`, `EXPERTO`.
- Si el rol origen es `ADMIN` o el usuario destino es superusuario, quien ejecuta debe ser superusuario.
- No se permite cambiar el propio rol.
- Al cambiar el rol se registra `log_actividad` con acción `CAMBIO_ROL`, se guarda historial y se borran las sesiones activas en `rust_sessions`.

Permiso de módulo documentado:

| Código | Nombre | Descripción | Otorgado por |
|---|---|---|---|
| `empresas.cargar_listas_informa` | Cargar Listas Informa | Carga el documento Listas Informa de las empresas. Normalmente un líder de facultad designado por el líder de retos. | Administrador o superusuario mediante asignación explícita. |

Reglas de permisos:

- Los superusuarios tienen todos los permisos de forma implícita.
- Los demás usuarios solo tienen permisos por otorgamiento explícito.
- `asignar_permisos_modulo` deja al usuario exactamente con los permisos solicitados.
- Los cambios se registran en `log_actividad` con acción `CAMBIO_PERMISOS`.

### 17.4 Reglas de validación

#### Contraseñas

| Regla | Valor |
|---|---:|
| Longitud mínima | 12 caracteres |
| Longitud máxima | 1024 bytes |
| Completamente numérica | Rechazada |
| Contraseñas comunes | Rechazadas |
| Similitud con usuario o correo | Rechazada si el atributo tiene 4 o más caracteres y hay contención en cualquier dirección |

Lista exacta de contraseñas comunes rechazadas:

```text
123456789012
1234567890123
12345678901234
1q2w3e4r5t6y
aaaaaaaaaaaa
abcdefghijkl
changeme123456
clave123456789
contrasena1234
contrasena123456
letmein12345
password1234
password12345
password123456
qwerty123456
qwertyqwerty
qwertyuiopas
welcome123456
```

#### Registro público

| Campo o regla | Validación |
|---|---|
| Tipos de registro | `academico` o `empresa` |
| Usuario | Máximo 150 caracteres; alfanuméricos más `. + - _`; sin `@` |
| Email | Máximo 254 caracteres; un solo `@`; local y dominio no vacíos; punto en dominio; sin espacios ni `<` o `>` |
| Contraseñas | Deben coincidir y pasar la política de contraseñas |
| Nombre y apellido | Máximo 150 caracteres |
| Roles académicos | `ESTUDIANTE` o `PROFESOR` |
| Correo académico | Debe terminar en `@universidadean.edu.co` |
| Semestre | `1..=10` |
| Especialidades | Lista cerrada de 10 valores |
| NIT | Máximo 20 caracteres |
| Razón social | Máximo 150 caracteres |
| Sector industrial | Máximo 100 caracteres |
| Área responsable | Máximo 150 caracteres |
| Tipo de convocatoria | `RETO`, `HACKATHON` o `AMBOS` |
| Sitio web | Opcional; máximo 200 caracteres; debe iniciar con `http://` o `https://` |

Especialidades válidas:

```text
Ingeniería de Software
Sistemas de Información
Inteligencia Artificial
Gestión de Operaciones
Finanzas Corporativas
Marketing Digital
Innovación y Emprendimiento
Sostenibilidad y Ambiente
Diseño y Creatividad
Otra
```

#### Perfil

| Campo | Regla |
|---|---|
| Teléfono | Máximo 20 caracteres |
| Nombre y apellido | Máximo 150 caracteres |
| Semestre | `1..=10` |
| Especialidad | Debe estar en la lista cerrada |
| Sitio web de empresa | Máximo 200 caracteres y prefijo `http://` o `https://` |
| Correo | Debe ser único en minúsculas |
| Estudiantes y profesores | Deben usar correo institucional `@universidadean.edu.co` |

---

## 18. Seguridad

### 18.1 Autenticación y contraseñas

- Algoritmo: PBKDF2-HMAC-SHA256.
- Iteraciones al crear hash: `1,000,000`.
- Formato almacenado:

```text
pbkdf2_sha256$1000000$<salt>$<base64>
```

- Compatible con hashes de Django `pbkdf2_sha256$<iter>$<salt>$<base64>`.
- Iteraciones aceptadas en verificación: `1..=2,000,000`.
- Comparación de hash en tiempo constante.
- Contraseña mínima: 12 caracteres.
- Registro público con verificación de correo por enlace de un solo uso:

```text
#verificar/<token>
```

- Token de verificación con hash SHA-256 y caducidad de 3 días.
- Recuperación de contraseña por enlace de un solo uso de 3 días:

```text
#restablecer/<token>
```

### 18.2 Sesiones

| Elemento | Valor |
|---|---|
| Cookie | `retosean_session` |
| Longitud de token | 64 caracteres |
| Hash de token | SHA-256 hexadecimal |
| Tabla | `rust_sessions` |
| Expiración | 8 horas |
| Max-Age | 28,800 segundos |
| Banderas | `Path=/; HttpOnly; SameSite=Strict; Max-Age=28800` |
| `Secure` | Solo si `RUST_SECURE_COOKIE` es verdadero; si es falso, se registra advertencia |

El usuario de sesión debe cumplir `is_active AND activo`.

### 18.3 CSRF y origen

- El token CSRF se guarda en `rust_sessions.csrf`.
- Se envía en JSON y se exige en la cabecera `X-CSRF-Token` para operaciones de escritura.
- La comparación de CSRF usa tiempo constante.
- El middleware `same_origin` rechaza escrituras con:
  - `Origin` ajeno al host efectivo.
  - `Origin` nulo.
  - `Sec-Fetch-Site` `cross-site` o `same-site`.
- Clientes sin cabeceras `Origin` o `Sec-Fetch-Site`, como curl o pruebas, igualmente necesitan CSRF en rutas autenticadas.

### 18.4 Limitación de intentos

| Alcance | Límite | Ventana | Respuesta |
|---|---:|---:|---|
| Login: cuenta + IP | 10 | 10 minutos | 429 |
| Login: cuenta global | 50 | 10 minutos | 429 |
| Login: IP global | 100 | 10 minutos | 429 |
| Registro público por IP | 10 | 10 minutos | 429 |
| Solicitud de restablecimiento por IP | 20 | 10 minutos | Respuesta uniforme `{"ok": true}` |
| Solicitud de restablecimiento por correo | 5 | 10 minutos | Respuesta uniforme `{"ok": true}` |

La tabla de intentos es `rust_login_attempts`. Los intentos antiguos se limpian tras 10 minutos.

### 18.5 Archivos

| Concepto | Límite |
|---|---:|
| Archivo individual en adjuntos múltiples | 50 MB |
| Máximo de archivos por solicitud | 10 |
| Total por solicitud | 100 MB |
| Campo de texto multipart | 1 MB |
| Entregable o documento legal | 9 MB |

Reglas de almacenamiento:

- Los adjuntos se escriben por fragmentos en temporales `.<uuid>.part`.
- La validación se hace por bytes mágicos:
  - PDF: `%PDF-`
  - PNG: `\x89PNG\r\n\x1a\n`
  - JPEG: `\xff\xd8\xff`
  - ZIP/OOXML: `PK\x03\x04`
  - Texto UTF-8 válido sin bytes NUL.
- En entregables y documentos (validación `extension()`), HTML, SVG y XML se rechazan aunque se disfracen. En los adjuntos múltiples (`save_attachments`) un `.txt` o `.csv` se acepta si es UTF-8 válido sin NUL, aunque su contenido sea HTML; no es explotable porque todo archivo se entrega solo como descarga (`attachment`, `application/octet-stream`, `nosniff`) y nunca se interpreta en el navegador.
- Los documentos legales deben ser PDF.
- Las descargas se realizan por streaming, con `Content-Length`, `Content-Disposition: attachment`, `Content-Type: application/octet-stream`, `nosniff`, y solo para usuarios autorizados.
- Al arrancar `serve`, se eliminan temporales huérfanos.

### 18.6 Correo

- Sin configuración SMTP, se usa backend de consola.
- Si `EMAIL_USE_TLS` es falso y hay credenciales SMTP, se rechaza autenticación en texto plano y se degrada a consola.
- Timeout SMTP: 10 segundos.
- El outbox de notificaciones se procesa cada 20 segundos.
- Los logs redactan claves y tokens, incluyendo:

```text
password
passwd
contrasena
contraseña
token
secret
authorization
#restablecer/
#verificar/
```

### 18.7 Cabeceras de seguridad

| Cabecera | Valor |
|---|---|
| `x-content-type-options` | `nosniff` |
| `x-frame-options` | `DENY` |
| `referrer-policy` | `same-origin` |
| `cache-control` | `no-store` |
| `content-security-policy` | `default-src 'self'; script-src 'self'; style-src 'self'; font-src 'self'; img-src 'self' data:; frame-ancestors 'none'; base-uri 'none'; form-action 'self'` |

### 18.8 Auditoría

Tabla: `log_actividad`.

| Acción | Origen |
|---|---|
| `LOGIN` | Login exitoso |
| `LOGIN_FALLIDO` | Credenciales inválidas o cuenta inhabilitada con contraseña correcta |
| `LOGOUT` | Cierre de sesión |
| `CREACION` | Registro público o alta por administrador |
| `CAMBIO_ESTADO` | Cambio de contraseña |
| `CAMBIO_ROL` | Cambio de rol |
| `CAMBIO_PERMISOS` | Cambio de permisos de módulo |

La consulta de bitácora es solo para administradores. La IP se guarda como tipo `inet`.

---

## 19. Base de datos y migraciones

### 19.1 Esquema

- El esquema contiene **60 tablas**, incluyendo tablas de autenticación heredadas de Django.
- Hay **49 tablas de dominio** con lectura paginada y política por rol.
- La tabla `historial_estados` es polimórfica.
- El esquema completo está en `sql/schema.sql`, embebido en el binario.

### 19.2 `init-db`

`retosean init-db`:

1. Conecta a la base `postgres`.
2. Crea la base objetivo si no existe.
3. Verifica que no haya tablas en `public`.
4. Si hay tablas, falla sin modificar datos:

```text
init-db requires an empty database; no existing tables were changed
```

5. En una transacción ejecuta `schema.sql`.
6. Registra las migraciones conocidas como aplicadas mediante baseline.

### 19.3 Migraciones versionadas

Las migraciones viven en `sql/migrations` y se registran en `src/schema_migrations.rs`.

| Nombre | Archivo |
|---|---|
| `0001_equipo_reto_academico_integracion` | `sql/migrations/0001_equipo_reto_academico_integracion.sql` |
| `0002_estado_constraints` | `sql/migrations/0002_estado_constraints.sql` |
| `0003_unique_and_fk` | `sql/migrations/0003_unique_and_fk.sql` |
| `0004_hot_indexes` | `sql/migrations/0004_hot_indexes.sql` |
| `0005_integridad_cruzada` | `sql/migrations/0005_integridad_cruzada.sql` |
| `0006_verificacion_correo` | `sql/migrations/0006_verificacion_correo.sql` |

Tabla de control:

```sql
CREATE TABLE IF NOT EXISTS rust_schema_migrations(
  name text PRIMARY KEY,
  applied_at timestamptz NOT NULL DEFAULT now()
)
```

`retosean migrate`:

- Toma un advisory lock para serializar el arranque.
- Usa `LOCK TABLE rust_schema_migrations IN EXCLUSIVE MODE` por migración.
- Aplica cada migración pendiente una sola vez.
- Imprime las migraciones aplicadas.

### 19.4 Cómo añadir una migración

1. Crear el archivo SQL bajo `sql/migrations/`.
2. Añadir la entrada en `MIGRATIONS` en `src/schema_migrations.rs`:

```rust
(
    "<nombre>",
    include_str!("../sql/migrations/<archivo>.sql"),
),
```

3. Respetar orden y unicidad de nombres.
4. Ejecutar:

```bash
retosean migrate
```

Si el cambio también debe estar en bases nuevas creadas con `init-db`, debe quedar reflejado en `sql/schema.sql` para que el baseline lo registre como aplicado.

### 19.5 `import-django`

```bash
retosean import-django <django_db_name> <django_media_dir>
```

- `<django_db_name>`: base Django existente en el mismo servidor.
- `<django_media_dir>`: ruta del `MEDIA_ROOT` de Django.

El proceso copia filas hacia una base Rust vacía y mueve archivos. Los campos de archivo quedan con claves planas UUID. Al final imprime resumen de filas, tablas y archivos, y lista hasta 20 elementos faltantes o tipos no soportados si los hay.

---

## 20. Pruebas y calidad

### 20.1 Conteo total

| Capa | Pruebas |
|---|---:|
| Paridad con Django (`tests/parity_*.rs`) | 339 |
| QA y seguridad (`tests/qa_*.rs`) | 135 |
| Extremo a extremo (`tests/end_to_end.rs`) | 5 |
| Unitarias en `src/` | 29 |
| **Total** | **508** |

### 20.2 Suites de paridad

| Archivo | Pruebas |
|---|---:|
| `tests/parity_academico.rs` | 16 |
| `tests/parity_cierre.rs` | 11 |
| `tests/parity_dashboard.rs` | 6 |
| `tests/parity_empresas.rs` | 44 |
| `tests/parity_evaluacion.rs` | 11 |
| `tests/parity_hackaton.rs` | 45 |
| `tests/parity_notificaciones.rs` | 22 |
| `tests/parity_participaciones.rs` | 19 |
| `tests/parity_presupuesto.rs` | 26 |
| `tests/parity_reportes.rs` | 24 |
| `tests/parity_retos.rs` | 46 |
| `tests/parity_seguimiento.rs` | 10 |
| `tests/parity_unidades_estudio.rs` | 16 |
| `tests/parity_usuarios.rs` | 43 |
| **Subtotal** | **339** |

### 20.3 Suites QA y de seguridad

| Archivo | Pruebas |
|---|---:|
| `tests/qa_academic.rs` | 12 |
| `tests/qa_ataques.rs` | 11 |
| `tests/qa_auth.rs` | 15 |
| `tests/qa_business.rs` | 12 |
| `tests/qa_data.rs` | 14 |
| `tests/qa_hackathon.rs` | 7 |
| `tests/qa_integridad.rs` | 9 |
| `tests/qa_memoria.rs` | 7 |
| `tests/qa_memoria_export.rs` | 2 |
| `tests/qa_origen.rs` | 4 |
| `tests/qa_robustez.rs` | 4 |
| `tests/qa_ronda3.rs` | 12 |
| `tests/qa_schema.rs` | 20 |
| `tests/qa_verificacion.rs` | 6 |
| **Subtotal** | **135** |

### 20.4 Extremo a extremo

`tests/end_to_end.rs` ejecuta handlers HTTP reales contra PostgreSQL, aislando cada prueba en un esquema desechable.

Ejecución explícita:

```bash
cargo test --test end_to_end -- --ignored --nocapture
```

### 20.5 Cómo ejecutar pruebas

Requisito: PostgreSQL disponible. Cada prueba crea su propio esquema.

Todo:

```bash
cargo test --locked -- --include-ignored
```

Un archivo:

```bash
cargo test --locked --test parity_retos -- --include-ignored
```

Una prueba por nombre:

```bash
cargo test --locked --test qa_business -- --include-ignored nombre_de_prueba
```

### 20.6 Calidad estática y auditoría

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
node --check web/js/*.js
cargo audit
```

`cargo audit` mantiene un aviso residual `RUSTSEC-2023-0071` del crate `rsa`, presente solo en `Cargo.lock` y no compilado.

### 20.7 CI

La integración continua está definida en:

```text
.github/workflows/rust.yml
```

---

## 21. Despliegue y operación

### 21.1 Build release

```bash
cargo build --locked --release
```

El binario resultante es:

```text
target/release/retosean
```

### 21.2 Docker Compose

El compose está en `rust/compose.yml` o en el directorio `rust` del proyecto, según la referencia verificada.

Requisito obligatorio:

```env
RUST_POSTGRES_PASSWORD=<contraseña>
```

Características del servicio `db`:

- Imagen `postgres:18` fijada por digest.
- Base: `retosean_rust`.
- Usuario: `retosean`.
- Contraseña entregada vía Docker secret.
- Volumen `rust_pgdata`.
- Healthcheck con `pg_isready`.

Características del servicio `web`:

- Binario `retosean`.
- Comando por defecto `serve`.
- `cap_drop: [ALL]`.
- `security_opt: ["no-new-privileges:true"]`.
- Sistema de archivos raíz de solo lectura.
- `tmpfs /tmp`.
- `mem_limit: 1g`.
- Healthcheck contra `/health`.
- Puerto publicado solo en loopback:

```yaml
ports:
  - "127.0.0.1:8080:8080"
```

- Volumen `rust_media` montado en `/data`.

El Dockerfile define estas variables:

```dockerfile
ENV RUST_BIND=0.0.0.0:8080
ENV RUST_MEDIA_DIR=/data/media
ENV RUST_LOG=retosean=info
```

Por tanto, es incorrecto afirmar que Compose no fija `RUST_BIND` ni `RUST_MEDIA_DIR`: el Dockerfile sí los define.

### 21.3 Proxy inverso

Recomendaciones operativas:

- Terminación TLS en el proxy.
- Mantener `RUST_SECURE_COOKIE=true`.
- Publicar el backend solo en loopback o red interna.
- Permitir cuerpos de al menos 110 MB para:
  - `/api/retos/{id}/seguimiento`
  - `/api/retos/{id}/archivos`
- Permitir 10 MB para cargas normales y 6 MB para importación/postulación.
- Solo activar `RUST_TRUST_PROXY=true` si el proxy es confiable.
- El proxy debe reenviar `Host` o `X-Forwarded-Host` de forma consistente para la validación de origen.

### 21.4 Copias de seguridad

Debe respaldarse:

1. La base de datos PostgreSQL.
2. El directorio de medios indicado por `RUST_MEDIA_DIR`.

En Docker, los volúmenes relevantes son `rust_pgdata` y `rust_media`.

### 21.5 Tareas programadas

| Tarea | Comando | Frecuencia |
|---|---|---|
| Avance de hackathons | `retosean advance-hackathons` | Horaria |
| Recordatorios | `retosean send-reminders [días...]` | Diaria |

Si no se indican días, `send-reminders` usa:

```text
7 3 1
```

### 21.6 Logs

La variable `RUST_LOG` controla el nivel. Por defecto:

```text
retosean=info
```

El backend de correo por consola registra correos con tokens redactados.

### 21.7 Salud

`GET /health` ejecuta `SELECT 1` contra el pool y devuelve:

```json
{"status":"ok","runtime":"rust","database":"postgresql"}
```

El healthcheck de Docker usa este endpoint.

### 21.8 Apagado ordenado

El servidor se detiene de forma ordenada con:

- Ctrl-C / `SIGINT`
- `SIGTERM`

Esto permite que `docker stop` o systemd drenen peticiones en curso.

### 21.9 Limpieza de temporales

- `serve` elimina temporales huérfanos al arrancar.
- Los temporales usan el patrón `.<uuid>.part`.
- `SpooledFile` elimina el temporal en `Drop` si no fue persistido.
- Si falla una transacción después de escribir un archivo final, el archivo se elimina.
- Al reemplazar un entregable, el archivo anterior almacenado se elimina para no dejar huérfanos.

---

## 22. Rendimiento y memoria

### 22.1 Semáforos y pool

| Recurso | Configuración |
|---|---|
| Pool PostgreSQL | 10 conexiones |
| `login_slots` | 4 permisos |
| `password_slots` | 4 permisos |
| `heavy_slots` | 4 permisos |

`heavy_slots` (vía `heavy_permit`) espera como máximo 30 segundos y responde `503`; `login_slots` y `password_slots` (vía `auth::hash_slot`) esperan como máximo 5 segundos y responden `429`.

`heavy_permit` devuelve `503` con el mensaje:

```text
El servidor está ocupado generando otros archivos; intenta de nuevo en un momento.
```

### 22.2 Resultados de carga

Medidos en equipo de 24 núcleos con build release:

| Métrica | Resultado |
|---|---:|
| Saturación aproximada | 6,500–8,500 solicitudes por segundo |
| Memoria en reposo | Aproximadamente 15 MB |
| Cargas: 2×45 MB ×4 | Pico de 83 MB |
| Descargas: 8×45 MB | Pico de 49 MB |
| 30 exportaciones | Pico de 146 MB |
| Retención tras carga | Aproximadamente 39 MB |

Referencia detallada: `docs/MEMORIA_RUST.md`.

### 22.3 Generador de carga

Hay un generador de prueba en `examples/stress.rs`:

```bash
cargo run --release --example stress -- [host:port] [segundos_por_nivel] [concurrencia,...]
```

Ejemplo:

```bash
cargo run --release --example stress -- 127.0.0.1:8082 10 1,10,50,100,250,500
```

Valores por defecto:

| Parámetro | Default |
|---|---:|
| `host:port` | `127.0.0.1:8082` |
| segundos por nivel | `10` |
| niveles de concurrencia | `1,10,50,100,250,500` |

Usa las cuentas demo y una mezcla de rutas de lectura. No escribe datos, salvo los logins de las cinco cuentas demo.

---

## 23. Cómo contribuir

### 23.1 Flujo general

1. Crear una rama desde el estado actual.
2. Implementar el cambio.
3. Añadir pruebas de regresión por cada corrección.
4. Ejecutar:

```bash
cargo test --locked -- --include-ignored
cargo fmt --check
cargo clippy --all-targets -- -D warnings
node --check web/js/*.js
```

5. No debilitar límites de cuerpo, límites de archivos, validaciones de estado o controles CSRF/origen.
6. Si el cambio toca seguridad, añadir o actualizar pruebas en `tests/qa_*.rs`.

### 23.2 Añadir una ruta

1. Crear o modificar el handler en el módulo correspondiente.
2. Declarar la ruta en `router()` dentro de `src/lib.rs`.
3. Mantener el límite global de 64 KB salvo necesidad justificada.
4. Si requiere un límite mayor, añadir una capa explícita:

```rust
.layer(axum::extract::DefaultBodyLimit::max(...))
```

5. Verificar que las escrituras exijan CSRF.
6. Mantener la política de roles y visibilidad del recurso.
7. Añadir pruebas de paridad o QA según el área.

### 23.3 Añadir una tabla con política

1. Crear migración SQL en `sql/migrations/`.
2. Registrarla en `src/schema_migrations.rs`.
3. Actualizar `sql/schema.sql` si la tabla debe existir en instalaciones nuevas con `init-db`.
4. Aplicar restricciones, claves foráneas y checks de estado.
5. Si es una tabla de dominio, incorporar:
   - lectura paginada,
   - política por rol,
   - auditoría cuando corresponda.
6. Añadir pruebas en `tests/qa_schema.rs` o en la suite del área.

### 23.4 Añadir una máquina de estado

1. Definir estados y transiciones permitidas.
2. Reflejar restricciones de estado en SQL.
3. Validar transiciones en el handler antes de actualizar.
4. Registrar el cambio en `historial_estados`.
5. Emitir notificación si el evento está contemplado en el catálogo.
6. Añadir pruebas de transición válida e inválida.

### 23.5 Añadir una notificación

1. Incorporar el evento al catálogo de eventos.
2. Definir si permite correo.
3. Crear la notificación en el flujo correspondiente.
4. Respetar preferencias del usuario y el outbox.
5. Registrar entrega o fallo si corresponde.
6. Añadir pruebas en `tests/parity_notificaciones.rs` o `tests/qa_*.rs`.

---

## 24. Documentos relacionados

| Documento | Contenido |
|---|---|
| `docs/QA_SEGURIDAD_RUST.md` | Informe de seguridad: 128 hallazgos, 119 corregidos, 9 rechazados, ronda 3. |
| `docs/MEMORIA_RUST.md` | Detalle de optimización de memoria y comportamiento bajo carga. |
| `docs/decisions.md` | Decisiones de diseño. |
| `rust/DEPLOY.md` | Material de despliegue del servidor Rust. |
| `.github/workflows/rust.yml` | Definición de CI. |