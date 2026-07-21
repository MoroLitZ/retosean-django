# Guía para alinear otras ramas con la estructura de `Caicedo`

Esta guía describe la estructura técnica vigente en la rama `Caicedo` y el procedimiento recomendado para adaptar otra rama sin perder funcionalidad, romper migraciones ni devolver código a la estructura monolítica anterior.

> Rama de referencia al escribir esta guía: `Caicedo`, commit `f40207e` o posterior.

## 1. Principio principal

La estructura objetivo está organizada por dominios de negocio dentro de `apps/`. Cada dominio debe ser responsable de sus modelos, vistas, URLs, administración, migraciones, pruebas y plantillas.

No se deben volver a concentrar funcionalidades de empresas, estudiantes, profesores, entregables o seguimiento dentro de `apps/usuarios`.

```text
retosean-django/
├── apps/
│   ├── usuarios/
│   ├── empresas/
│   ├── academico/
│   ├── retos/
│   ├── participaciones/
│   ├── evaluacion/
│   ├── seguimiento/
│   ├── unidades_estudio/
│   ├── presupuesto/
│   ├── cierre/
│   ├── hackaton/
│   ├── notificaciones/
│   ├── reportes/
│   └── dashboard/
├── config/
│   ├── settings/
│   │   ├── __init__.py
│   │   ├── base.py
│   │   ├── development.py
│   │   └── production.py
│   ├── urls.py
│   ├── asgi.py
│   └── wsgi.py
├── templates/
│   ├── base.html
│   └── base_auth.html
├── manage.py
├── requirements.txt
└── .env.example
```

## 2. Responsabilidad de cada aplicación

### `apps.usuarios`

Debe contener exclusivamente autenticación, registro, perfil, modelo de usuario, selección de dashboard por rol y administración general de usuarios.

No debe volver a contener modelos o vistas de documentos empresariales, postulaciones, entregables o gestión académica.

### `apps.empresas`

Responsable de:

- Perfil y datos de la empresa.
- Documentación legal.
- Verificación de empresa.
- Panel empresarial.
- Postulaciones y entregables vistos desde la empresa.
- Indicadores empresariales.

Las plantillas deben vivir en:

```text
apps/empresas/templates/empresas/
```

### `apps.academico`

Responsable de facultades, programas, perfil académico y pantallas específicas de profesores y estudiantes.

Ubicación de plantillas:

```text
apps/academico/templates/profesor/
apps/academico/templates/estudiante/
```

### `apps.retos`

Responsable del ciclo de vida principal de retos:

- Crear y editar retos.
- Enviar a aprobación.
- Aprobar o rechazar.
- Cambiar estado.
- Mostrar el detalle.
- Gestionar archivos de soporte mediante `RetoArchivo`.
- Coordinar las vistas actuales de integraciones académicas.

El modelo `Reto` permanece en esta app. No se debe crear una segunda versión en otra aplicación.

### `apps.seguimiento`

Responsable de:

- `IntegracionAcademica`.
- `SesionReto`.
- `SeguimientoReto`.
- `SeguimientoArchivo`.
- Consultas de cursos, estudiantes, evaluaciones y entregables para seguimiento.

Aunque algunas vistas de integración todavía se coordinan desde `apps.retos`, el modelo canónico está en `apps.seguimiento.models`. No se debe duplicar en `apps.retos.models`.

### `apps.participaciones`

Responsable de postulaciones y participación del estudiante en retos, incluyendo la carga de entregables desde la experiencia estudiantil.

### `apps.evaluacion`

Responsable de `Entregable`, evaluaciones y criterios asociados. El modelo `Entregable` no debe regresar a `apps.usuarios`.

### `apps.unidades_estudio`

Responsable del CRUD de unidades de estudio y carga individual o masiva mediante CSV.

Las acciones que cambian estado, como activar o desactivar, deben usar `POST` y CSRF. Nunca deben implementarse como enlaces GET.

### Apps de soporte

- `presupuesto`: presupuesto y comprobantes.
- `cierre`: actas y cierre de retos.
- `notificaciones`: notificaciones internas.
- `reportes`: generación y almacenamiento de reportes.
- `dashboard`: punto reservado para coordinación de dashboards; no duplicar aquí los dashboards que ya pertenecen a un dominio.
- `hackaton`: implementación actual de modelos de hackatón.

Actualmente también existe `apps/hackathon` como estructura vacía de compatibilidad. No se debe agregar funcionalidad nueva allí. Hasta realizar una migración específica para normalizar el nombre, los modelos activos continúan en `apps.hackaton`.

## 3. Configuración Django

No debe existir un archivo monolítico `config/settings.py`. La configuración se divide así:

```text
config/settings/base.py
config/settings/development.py
config/settings/production.py
```

### `base.py`

Contiene configuración compartida:

- `INSTALLED_APPS`.
- Middleware.
- Templates.
- Base de datos PostgreSQL.
- Usuario personalizado.
- Internacionalización.
- Archivos estáticos y multimedia.
- Límite general de carga.

Las aplicaciones locales se registran con su clase `AppConfig`:

```python
LOCAL_APPS = [
    "apps.usuarios.apps.UsuariosConfig",
    "apps.empresas.apps.EmpresasConfig",
    "apps.academico.apps.AcademicoConfig",
    "apps.retos.apps.RetosConfig",
    "apps.participaciones.apps.ParticipacionesConfig",
    "apps.evaluacion.apps.EvaluacionConfig",
    "apps.seguimiento.apps.SeguimientoConfig",
    "apps.cierre.apps.CierreConfig",
    "apps.hackaton.apps.HackatonConfig",
    "apps.notificaciones.apps.NotificacionesConfig",
    "apps.reportes.apps.ReportesConfig",
    "apps.dashboard.apps.DashboardConfig",
    "apps.presupuesto.apps.PresupuestoConfig",
    "apps.unidades_estudio.apps.UnidadesEstudioConfig",
]
```

### Settings por entorno

`manage.py` utiliza por defecto:

```python
DJANGO_SETTINGS_MODULE = "config.settings.development"
```

Producción debe utilizar:

```text
DJANGO_SETTINGS_MODULE=config.settings.production
```

No se deben subir secretos, `.env`, bases SQLite locales, archivos de medios ni credenciales.

## 4. Enrutamiento

`config/urls.py` solo debe incluir rutas de nivel superior. La lógica concreta pertenece al `urls.py` de cada aplicación.

Ejemplo:

```python
path("retos/", include("apps.retos.urls")),
path("empresas/", include("apps.empresas.urls")),
path("academico/", include("apps.academico.urls")),
path("participaciones/", include("apps.participaciones.urls")),
path("seguimiento/", include("apps.seguimiento.urls")),
path("unidades-estudio/", include("apps.unidades_estudio.urls")),
```

Cada aplicación debe declarar un namespace:

```python
app_name = "retos"
```

Y las plantillas deben usar URLs con namespace:

```django
{% url 'retos:detalle' reto.pk %}
```

No se deben importar vistas de dominio directamente en `config/urls.py`.

## 5. Organización de plantillas

Solo las plantillas globales deben estar en `templates/`:

- `base.html`.
- `base_auth.html`.

Las demás deben estar dentro de su aplicación y con namespace de carpeta:

```text
apps/retos/templates/retos/detalle_reto.html
apps/empresas/templates/empresas/dashboard.html
apps/academico/templates/profesor/dashboard.html
```

La llamada desde una vista debe incluir el namespace:

```python
return render(request, "retos/detalle_reto.html", context)
```

No copiar una plantilla para resolver temporalmente un error de ruta. Se debe mover la plantilla, actualizar su vista y corregir todos los `{% url %}` relacionados.

## 6. Roles y permisos

Los roles vigentes son:

- `ADMIN`.
- `EMPRESA`.
- `PROFESOR`.
- `ESTUDIANTE`.

Las vistas deben validar permisos en servidor. Ocultar un enlace en `base.html` no constituye autorización.

En retos existen decoradores como:

```python
@solo_administrador
@solo_empresa
@solo_profesor
@profesor_o_admin
```

Al mover una vista entre apps, también se debe revisar:

1. Quién puede abrirla.
2. Qué objetos puede consultar ese usuario.
3. Si el `get_object_or_404` filtra propiedad cuando corresponde.
4. Si una acción destructiva exige `POST`.
5. Si el formulario incluye `{% csrf_token %}`.

## 7. Archivos subidos

Los retos y avances permiten múltiples archivos. No se debe volver al campo único en formularios.

Modelos canónicos:

- `RetoArchivo` para soportes de reto.
- `SeguimientoArchivo` para soportes de avance.

Reglas actuales:

- Máximo 50 MB por archivo en los formularios implementados.
- Validación en servidor, no solamente en HTML.
- Selección múltiple mediante `MultipleFileInput`.
- Formularios HTML con `enctype="multipart/form-data"`.
- Guardado individual de cada archivo relacionado.

Al crear otro flujo de carga se debe reutilizar o extraer la validación existente; no confiar únicamente en `FILE_UPLOAD_MAX_MEMORY_SIZE`, porque ese ajuste controla memoria y no reemplaza la validación del formulario.

## 8. Modelos y migraciones

Nunca se debe editar una migración ya compartida para acomodar una rama nueva, salvo que el equipo confirme que nunca fue aplicada en ningún entorno.

Procedimiento para cambios de modelo:

```powershell
python manage.py makemigrations
python manage.py makemigrations --check --dry-run
python manage.py migrate
python manage.py check
```

Antes de mover un modelo entre aplicaciones:

1. Identificar su tabla existente mediante `db_table`.
2. Revisar claves foráneas y dependencias de migraciones.
3. Crear una migración de estado/datos si la tabla debe conservarse.
4. No crear dos tablas canónicas para el mismo concepto.
5. Probar sobre una base limpia y sobre una copia de la base existente.

Los modelos canónicos que suelen causar conflictos son:

| Concepto | Ubicación canónica |
|---|---|
| Usuario | `apps.usuarios.models.Usuario` |
| Empresa | `apps.empresas.models.Empresa` |
| Documento empresarial | `apps.empresas.models.DocumentoEmpresa` |
| Reto | `apps.retos.models.Reto` |
| Archivo de reto | `apps.retos.models.RetoArchivo` |
| Integración académica | `apps.seguimiento.models.IntegracionAcademica` |
| Seguimiento | `apps.seguimiento.models.SeguimientoReto` |
| Archivo de seguimiento | `apps.seguimiento.models.SeguimientoArchivo` |
| Entregable | `apps.evaluacion.models.Entregable` |
| Postulación | `apps.participaciones.models.PostulacionReto` |
| Unidad de estudio | `apps.unidades_estudio.models.UnidadEstudio` |

## 9. Procedimiento para adaptar una rama

### Paso 1: preservar el trabajo

```powershell
git status --short
git add <archivos-de-la-funcionalidad>
git commit -m "wip: preserve work before structure alignment"
```

No incluir `.env`, bases de datos, medios subidos o archivos temporales.

### Paso 2: actualizar referencias

```powershell
git fetch origin --prune
git branch -a -vv
```

### Paso 3: crear una rama de integración

```powershell
git switch -c integration/<nombre-funcionalidad>
```

### Paso 4: incorporar `Caicedo`

La opción recomendada para una rama de funcionalidad corta es rebase:

```powershell
git rebase origin/Caicedo
```

Para una rama compartida donde no se debe reescribir historia:

```powershell
git merge --no-ff origin/Caicedo
```

No usar `git reset --hard` para resolver diferencias estructurales.

### Paso 5: resolver conflictos por dominio

Al resolver cada conflicto:

1. Mantener la ubicación del archivo usada por `Caicedo`.
2. Incorporar la lógica funcional nueva dentro de esa ubicación.
3. Actualizar imports de modelos.
4. Actualizar rutas con namespace.
5. Actualizar referencias de plantillas.
6. Conservar la configuración modular.
7. Conservar migraciones de ambas funcionalidades cuando sean compatibles.
8. No aceptar automáticamente una versión completa sin revisar la pérdida funcional.

Ejemplo: si otra rama modificó `apps/usuarios/views.py` para agregar comportamiento empresarial, no se debe restaurar esa vista monolítica. Se debe trasladar la lógica a `apps/empresas/views.py` y conectar sus URLs.

### Paso 6: buscar referencias antiguas

```powershell
rg -n "apps\.usuarios.*Entregable|apps\.retos.*IntegracionAcademica|config\.settings\.py" apps config templates
rg -n "<<<<<<<|=======|>>>>>>>" .
```

### Paso 7: validar

```powershell
python -m compileall -q apps config
python manage.py check
python manage.py makemigrations --check --dry-run
python manage.py migrate
python manage.py test
git diff --check
```

Se debe probar además manualmente:

- Inicio de sesión de cada rol.
- Navegación del sidebar.
- Creación y aprobación de retos.
- Creación y aprobación de integraciones.
- Documentos empresariales.
- Postulaciones y entregables.
- Archivos múltiples.
- Seguimientos con archivos.
- Unidades de estudio.

## 10. Reglas para código nuevo

1. Crear funcionalidad en la app dueña del dominio.
2. Evitar imports desde `views.py` de otra app; mover utilidades compartidas a `services.py`, `decorators.py` o un módulo común.
3. Usar servicios para transiciones de estado con efectos secundarios.
4. Mantener consultas de propiedad en servidor.
5. Usar `POST` para crear, aprobar, rechazar, activar, desactivar o eliminar.
6. Agregar pruebas reales; no dejar únicamente el archivo `tests.py` generado por Django.
7. Guardar archivos en UTF-8.
8. No crear una segunda app con una variante ortográfica del mismo dominio.
9. No agregar rutas vacías a `config/urls.py`.
10. No subir bases locales, respaldos ni archivos generados.

## 11. Checklist para pull requests

- [ ] La rama parte de una versión reciente de `Caicedo`.
- [ ] No reaparece `config/settings.py`.
- [ ] La app está registrada en `LOCAL_APPS` si corresponde.
- [ ] La URL raíz está incluida una sola vez.
- [ ] Todas las URLs usan namespace.
- [ ] Las plantillas están dentro de la app correcta.
- [ ] No se duplicaron modelos existentes.
- [ ] Se revisaron migraciones y dependencias.
- [ ] Las acciones que modifican datos usan POST y CSRF.
- [ ] Los permisos se validan en servidor.
- [ ] Los archivos tienen validación de tamaño y tipo cuando aplica.
- [ ] `python manage.py check` pasa.
- [ ] `makemigrations --check --dry-run` no detecta cambios olvidados.
- [ ] Las pruebas pasan.
- [ ] `git diff --check` no reporta errores.
- [ ] No hay marcadores de conflicto.
- [ ] No se incluyeron secretos, bases locales ni archivos multimedia.

## 12. Aspectos pendientes conocidos

La estructura actual es la referencia de integración, pero todavía tiene puntos que deben resolverse en cambios independientes y controlados:

- Normalizar definitivamente `hackaton` frente a `hackathon`.
- Retirar el campo legado `Reto.documento_soporte` después de migrar sus datos a `RetoArchivo`.
- Centralizar validadores de archivos para todos los módulos.
- Extraer decoradores de roles a un módulo compartido.
- Consolidar el flujo duplicado de revisión de integraciones/vinculaciones.
- Aumentar cobertura de pruebas.
- Corregir textos heredados con codificación dañada.

No se deben resolver estos puntos eliminando archivos o migraciones de forma unilateral dentro de una rama funcional. Cada consolidación necesita migración, pruebas y revisión del equipo.

## 13. Criterio de finalización

Una rama está alineada cuando conserva su funcionalidad, utiliza los módulos canónicos descritos aquí, no restaura la estructura anterior, aplica correctamente todas las migraciones y supera los comandos de validación sin errores.
