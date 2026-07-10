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
    "apps.usuarios.apps.UsuariosConfig",
    "apps.empresas.apps.EmpresasConfig",
    "apps.retos.apps.RetosConfig",
    "apps.estudiantes.apps.EstudiantesConfig",
    "apps.evaluacion.apps.EvaluacionConfig",
    "apps.seguimiento.apps.SeguimientoConfig",
    "apps.notificaciones.apps.NotificacionesConfig",
    "apps.reportes.apps.ReportesConfig",
    "apps.dashboard.apps.DashboardConfig",
    "apps.cierre.apps.CierreConfig",
    "apps.hackathon.apps.HackathonConfig",
]

INSTALLED_APPS = DJANGO_APPS + LOCAL_APPS

MIDDLEWARE = [
    "django.middleware.security.SecurityMiddleware",
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
    }
}

AUTH_USER_MODEL = "usuarios.Usuario"

LANGUAGE_CODE = "es-co"
TIME_ZONE = "America/Bogota"
USE_I18N = True
USE_TZ = True

STATIC_URL = "/static/"
STATICFILES_DIRS = [BASE_DIR / "static"]
STATIC_ROOT = BASE_DIR / "staticfiles"

MEDIA_URL = "/media/"
MEDIA_ROOT = os.path.join(BASE_DIR, "media")
MAX_UPLOAD_SIZE = 50 * 1024 * 1024
FILE_UPLOAD_MAX_MEMORY_SIZE = MAX_UPLOAD_SIZE

DEFAULT_AUTO_FIELD = "django.db.models.BigAutoField"

LOGIN_URL = "/usuarios/login/"
LOGIN_REDIRECT_URL = "/"
LOGOUT_REDIRECT_URL = "/usuarios/login/"
