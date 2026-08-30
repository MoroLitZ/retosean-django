# RetosEAN — Sistema de Control de Retos Universidad–Empresa

Plataforma Django + PostgreSQL que conecta empresas, docentes y estudiantes de la
Universidad EAN alrededor de retos empresariales: la empresa publica el reto, el
administrador lo aprueba, un docente lo integra a su curso, los estudiantes se
postulan y entregan, el docente evalúa y el administrador cierra el ciclo emitiendo
certificados.

## Estado por historia de usuario

| Sprint | HU | Módulo | Estado |
|---|---|---|---|
| 1 | HU13 Usuarios y roles | `usuarios` | Registro multirol, login, decoradores compartidos, importación masiva CSV/Excel, bitácora de actividad |
| 2 | HU01 / HU00 Empresas y documentación | `empresas` | Registro, carga de documentos con validación de vigencia, verificación por admin, listas restrictivas |
| 3 | HU02 / HU04 Ciclo del reto | `retos` | Borrador → revisión → aprobación con consecutivo e historial de estados |
| 4 | HU03 Integración académica | `retos` + `seguimiento` | Integración por docente, equipos, sesiones y flujo de publicación |
| 5 | HU10 / HU11 Participación | `academico` + `participaciones` | Explorador filtrable, favoritos, postulación con validaciones |
| 6 | HU12 / HU14 Evaluación y seguimiento | `evaluacion` + `seguimiento` | Rúbricas o puntaje libre, historial de comentarios, avance visible para la empresa |
| 7 | HU08 Cierre | `cierre` | Agenda, acta, entregables finales, reconocimientos y encuesta |
| 8 | HU15 Notificaciones | `notificaciones` | Motor central, campana in-app, correo, preferencias y recordatorios |
| 9 | HU05 / HU06 / HU07 | `dashboard`, `reportes`, `presupuesto` | KPIs con Chart.js, reportes PDF/Excel y control presupuestal con alerta al 80% |
| 10 | HU09 / HU16 | `hackaton`, `academico` | Hackathon completo (etapas, inscripción, jurados, ranking) y certificados con verificación pública |

## Puesta en marcha

```bash
python -m venv venv
venv\Scripts\activate            # Windows
pip install -r requirements.txt
copy .env.example .env           # y edita las credenciales
python manage.py migrate
python manage.py createsuperuser
python manage.py runserver
```

Requiere PostgreSQL. Todas las credenciales salen de `.env`; nunca se comitean.

### Variables de entorno

| Variable | Uso |
|---|---|
| `SECRET_KEY`, `DEBUG`, `ALLOWED_HOSTS` | Configuración base de Django |
| `DB_NAME`, `DB_USER`, `DB_PASSWORD`, `DB_HOST`, `DB_PORT` | Conexión a PostgreSQL |
| `SITE_URL` | URL pública, usada para los enlaces absolutos de los correos |
| `EMAIL_HOST`, `EMAIL_PORT`, `EMAIL_HOST_USER`, `EMAIL_HOST_PASSWORD`, `EMAIL_USE_TLS`, `DEFAULT_FROM_EMAIL` | Envío de correo (HU15) |
| `CSRF_TRUSTED_ORIGINS`, `SECURE_SSL_REDIRECT` | Solo producción |

En desarrollo los correos se imprimen en la consola: no hace falta configurar SMTP.

## Arquitectura

```
apps/
  usuarios/        roles.py, decorators.py, context_processors.py, importacion.py, signals.py
  empresas/        onboarding y documentación legal
  retos/           ciclo de vida del reto e integración académica
  academico/       facultades, programas, exploración y certificados
  participaciones/ postulaciones, equipos operativos y entregables
  evaluacion/      rúbricas, calificación y comentarios
  seguimiento/     IntegracionAcademica y avances del reto
  cierre/          agenda, acta, reconocimientos y encuestas
  notificaciones/  motor de eventos, bandeja, preferencias y recordatorios
  dashboard/       KPIs, series para Chart.js y filtros de periodo
  reportes/        catálogo de reportes y exportación PDF/Excel
  presupuesto/     presupuesto y gastos por reto
  hackaton/        modalidad hackathon
  unidades_estudio/ unidades por programa académico
```

Convenciones que conviene respetar (ver `constitution.md`):

- **Roles**: usa `apps.usuarios.roles.es_admin` y los decoradores de
  `apps.usuarios.decorators`. En plantillas, los flags `es_admin`, `es_empresa`,
  `es_profesor`, `es_estudiante` los inyecta un context processor.
- **Notificaciones**: siempre a través de
  `apps.notificaciones.services.notificar` / `notificar_muchos` / `notificar_admins`.
  El servicio aplica las preferencias del usuario, deduplica por `clave_dedupe` y
  envía el correo en `transaction.on_commit`.
- **Cross-app**: nunca importes de otro `views.py`; usa su `services.py`,
  `models.py` o `decorators.py`.

## Recordatorios automáticos

No se usa Celery. Los recordatorios (7, 3 y 1 día antes de las fechas clave) son un
comando de gestión idempotente que se programa una vez al día:

```bash
python manage.py enviar_recordatorios --dias 7 3 1
python manage.py enviar_recordatorios --dry-run     # ver qué se enviaría
```

Cubre: cierre de postulaciones, entrega final pendiente, entregables sin calificar,
etapas de hackathon y vencimiento de documentos de empresa.

**Programador de tareas de Windows**: crea una tarea diaria que ejecute
`C:\ruta\al\proyecto\venv\Scripts\python.exe manage.py enviar_recordatorios`
con el directorio de trabajo en la raíz del proyecto. En Linux, la línea de cron
equivalente es `0 7 * * *`.

## Rutas principales

| Ruta | Descripción |
|---|---|
| `/dashboard/` | Panel de indicadores según el rol |
| `/retos/mis-retos/`, `/retos/crear/` | Gestión de retos por la empresa |
| `/retos/admin/panel/` | Revisión y aprobación de retos |
| `/retos/integraciones/` | Integraciones académicas del docente |
| `/academico/explorar-retos/` | Explorador de retos del estudiante |
| `/academico/certificados/`, `/academico/portafolio/` | Certificados y portafolio |
| `/academico/verificar/<codigo>/` | Verificación pública de un certificado (sin login) |
| `/participaciones/entregables/` | Entregables del estudiante |
| `/evaluacion/` | Panel de calificación del docente |
| `/cierre/` | Cierre formal de retos |
| `/notificaciones/` | Bandeja y preferencias |
| `/reportes/` | Constructor de informes PDF/Excel |
| `/presupuesto/` | Presupuesto y gastos por reto |
| `/hackatones/` | Hackathones, inscripción, votación y ranking |

## Verificación

```bash
python manage.py check
python manage.py makemigrations --check --dry-run      # debe decir "No changes"
python manage.py migrate --plan
python manage.py test
python manage.py check --deploy --settings=config.settings.production
```

Toda la suite debe pasar antes de mergear. Los cambios de modelo van siempre con su
migración en el mismo commit.

## Settings

- Desarrollo: `config.settings.development` (correo a consola, `DEBUG=True`)
- Producción: `config.settings.production` (HSTS, cookies seguras, redirección HTTPS,
  `ALLOWED_HOSTS` obligatorio desde entorno)

## Despliegue a producción

```bash
pip install -r requirements.txt
copy .env.example .env                # edita SECRET_KEY, DEBUG=False, ALLOWED_HOSTS y la BD
python manage.py migrate
python manage.py sembrar_catalogo     # siembra facultades/programas/ecosistemas (idempotente)
python manage.py sembrar_catalogo --demo   # opcional: usuarios por rol + un reto de ejemplo
python manage.py collectstatic --noinput
python manage.py check --deploy --settings=config.settings.production
gunicorn config.wsgi:application      # wsgi.py usa config.settings.production
```

- **Estáticos**: Whitenoise sirve `/static/` directamente desde el proceso Django
  (no requiere nginx para los estáticos). Ejecuta `collectstatic` en cada despliegue.
- **Archivos subidos (`/media/`)**: no los sirve Django en producción. Expón la
  carpeta `media/` a través de nginx, S3 o un almacenamiento compatible; los
  `FileField` siguen escribiendo en `MEDIA_ROOT`.
- **Correo**: en producción define `EMAIL_*` con un SMTP real. La recuperación de
  contraseña (`/usuarios/recuperar-contrasena/`) y las notificaciones dependen de él.
- **Correo de respaldo del admin**: crea el superusuario antes de arrancar
  (`python manage.py createsuperuser`).

### Ajustes de seguridad recomendados

| Check | Cómo se cubre |
|---|---|
| `SECURE_SSL_REDIRECT` | `SECURE_SSL_REDIRECT=1` en `.env` |
| `SECURE_HSTS_SECONDS` | Ya configurado en `production.py` |
| `CSRF_TRUSTED_ORIGINS` | Lista de orígenes separados por coma en `.env` |
| `SESSION_COOKIE_SECURE` / `CSRF_COOKIE_SECURE` | Ya configurados en `production.py` |
