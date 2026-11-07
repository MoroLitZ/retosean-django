from pathlib import Path
import os

from decouple import Csv, config


BASE_DIR = Path(__file__).resolve().parent.parent.parent


def env_bool(name, default=False):
    raw_value = config(name, default=default)
    if isinstance(raw_value, bool):
        return raw_value

    normalized = str(raw_value).strip().lower()
    if normalized in {"1", "true", "yes", "on"}:
        return True
    if normalized in {"0", "false", "no", "off"}:
        return False
    return default


SECRET_KEY = config("SECRET_KEY", default="change-me-in-env")
DEBUG = env_bool("DEBUG", default=False)
ALLOWED_HOSTS = config("ALLOWED_HOSTS", default="127.0.0.1,localhost", cast=Csv())

DJANGO_APPS = [
    "django.contrib.admin",
    "django.contrib.auth",
    "django.contrib.contenttypes",
    "django.contrib.sessions",
    "django.contrib.messages",
    "django.contrib.staticfiles",
]

LOCAL_APPS = [
    "apps.academico.apps.AcademicoConfig",
    "apps.cierre.apps.CierreConfig",
    "apps.dashboard.apps.DashboardConfig",
    "apps.empresas.apps.EmpresasConfig",
    "apps.evaluacion.apps.EvaluacionConfig",
    "apps.hackaton.apps.HackatonConfig",
    "apps.notificaciones.apps.NotificacionesConfig",
    "apps.participaciones.apps.ParticipacionesConfig",
    "apps.presupuesto.apps.PresupuestoConfig",
    "apps.reportes.apps.ReportesConfig",
    "apps.retos.apps.RetosConfig",
    "apps.seguimiento.apps.SeguimientoConfig",
    "apps.unidades_estudio.apps.UnidadesEstudioConfig",
    "apps.usuarios.apps.UsuariosConfig",
]

INSTALLED_APPS = DJANGO_APPS + LOCAL_APPS

MIDDLEWARE = [
    "django.middleware.security.SecurityMiddleware",
    "whitenoise.middleware.WhiteNoiseMiddleware",
    "django.contrib.sessions.middleware.SessionMiddleware",
    "django.middleware.common.CommonMiddleware",
    "django.middleware.csrf.CsrfViewMiddleware",
    "django.contrib.auth.middleware.AuthenticationMiddleware",
    "django.contrib.messages.middleware.MessageMiddleware",
    "django.middleware.clickjacking.XFrameOptionsMiddleware",
]

ROOT_URLCONF = "config.urls"

TEMPLATES = [
    {
        "BACKEND": "django.template.backends.django.DjangoTemplates",
        "DIRS": [BASE_DIR / "templates"],
        "APP_DIRS": True,
        "OPTIONS": {
            "context_processors": [
                "django.template.context_processors.debug",
                "django.template.context_processors.request",
                "django.contrib.auth.context_processors.auth",
                "django.contrib.messages.context_processors.messages",
                "apps.usuarios.context_processors.rol_flags",
                "apps.notificaciones.context_processors.notificaciones_topbar",
            ],
        },
    }
]

WSGI_APPLICATION = "config.wsgi.application"
ASGI_APPLICATION = "config.asgi.application"

DATABASES = {
    "default": {
        "ENGINE": "django.db.backends.postgresql",
        "NAME": config("DB_NAME"),
        "USER": config("DB_USER"),
        "PASSWORD": config("DB_PASSWORD"),
        "HOST": config("DB_HOST", default="localhost"),
        "PORT": config("DB_PORT", default="5432"),
        # Permite correr varias suites en paralelo, cada una con su BD de prueba.
        "TEST": {"NAME": config("DB_TEST_NAME", default=None)},
    }
}

AUTH_USER_MODEL = "usuarios.Usuario"
AUTHENTICATION_BACKENDS = ["apps.usuarios.backends.UsuarioOCorreoBackend"]

# CFG-02: validadores de contrasena por defecto de Django (similitud con los
# datos del usuario, longitud minima, contrasenas comunes y solo numeros).
AUTH_PASSWORD_VALIDATORS = [
    {"NAME": "django.contrib.auth.password_validation.UserAttributeSimilarityValidator"},
    {"NAME": "django.contrib.auth.password_validation.MinimumLengthValidator"},
    {"NAME": "django.contrib.auth.password_validation.CommonPasswordValidator"},
    {"NAME": "django.contrib.auth.password_validation.NumericPasswordValidator"},
]

LANGUAGE_CODE = "es-co"
TIME_ZONE = "America/Bogota"
USE_I18N = True
USE_TZ = True

STATIC_URL = "/static/"
STATICFILES_DIRS = [BASE_DIR / "static"]
STATIC_ROOT = BASE_DIR / "staticfiles"

# Whitenoise sirve /static/ directamente desde el proceso (sin nginx) cuando
# DEBUG=False. CompressedStaticFilesStorage añade gzip/brotli sin el hashing
# de nombres que romperia las referencias a archivos puntuales del proyecto.
STORAGES = {
    "default": {
        "BACKEND": "django.core.files.storage.FileSystemStorage",
    },
    "staticfiles": {
        "BACKEND": "whitenoise.storage.CompressedStaticFilesStorage",
    },
}

MEDIA_URL = "/media/"
MEDIA_ROOT = os.path.join(BASE_DIR, "media")
MAX_UPLOAD_SIZE = 50 * 1024 * 1024
# CFG-06: los archivos por encima de este umbral se vuelcan a un temporal en
# disco en vez de RAM; con el umbral anterior (MAX_UPLOAD_SIZE) varias subidas
# concurrentes de decenas de MB podian agotar la memoria del proceso.
FILE_UPLOAD_MAX_MEMORY_SIZE = 5 * 1024 * 1024
# Acota por separado los datos de formulario sin archivo en cada peticion.
DATA_UPLOAD_MAX_MEMORY_SIZE = 5 * 1024 * 1024

DEFAULT_AUTO_FIELD = "django.db.models.BigAutoField"

LOGIN_URL = "/usuarios/login/"
LOGIN_REDIRECT_URL = "/"
LOGOUT_REDIRECT_URL = "/usuarios/login/"

# CFG-11: la ruta del administrador se define por entorno; en produccion se
# cambia para no dejar el panel expuesto en la ruta conocida "admin/".
ADMIN_URL = config("ADMIN_URL", default="admin/")

# --- Correo (HU15) --------------------------------------------------------
# En desarrollo se sobreescribe por el backend de consola.
EMAIL_BACKEND = config("EMAIL_BACKEND", default="django.core.mail.backends.smtp.EmailBackend")
EMAIL_HOST = config("EMAIL_HOST", default="localhost")
EMAIL_PORT = config("EMAIL_PORT", default=587, cast=int)
EMAIL_HOST_USER = config("EMAIL_HOST_USER", default="")
EMAIL_HOST_PASSWORD = config("EMAIL_HOST_PASSWORD", default="")
EMAIL_USE_TLS = env_bool("EMAIL_USE_TLS", default=True)
EMAIL_TIMEOUT = config("EMAIL_TIMEOUT", default=10, cast=int)
DEFAULT_FROM_EMAIL = config("DEFAULT_FROM_EMAIL", default="RetosEAN <no-responder@universidadean.edu.co>")
SERVER_EMAIL = DEFAULT_FROM_EMAIL

# URL publica del sitio: se usa para construir enlaces absolutos en los correos.
SITE_URL = config("SITE_URL", default="http://127.0.0.1:8000")

LOGGING = {
    "version": 1,
    "disable_existing_loggers": False,
    "formatters": {
        "simple": {"format": "[{levelname}] {asctime} {name}: {message}", "style": "{"},
        # CFG-10: mismo formato, pero redacta contrasenas, tokens y secretos
        # que puedan colarse en los mensajes (MAN-CFG-08).
        "seguro": {
            "()": "config.registro.FormateadorSeguro",
            "format": "[{levelname}] {asctime} {name}: {message}",
            "style": "{",
        },
    },
    "handlers": {
        "console": {"class": "logging.StreamHandler", "formatter": "seguro"},
    },
    "root": {"handlers": ["console"], "level": config("LOG_LEVEL", default="INFO")},
    "loggers": {
        "django.request": {"handlers": ["console"], "level": "ERROR", "propagate": False},
        # CFG-10: eventos de seguridad (CSRF sospechoso, cabeceras falsificadas,
        # denegaciones) con logger propio para que destaquen en el registro.
        "django.security": {"handlers": ["console"], "level": "WARNING", "propagate": False},
    },
}
