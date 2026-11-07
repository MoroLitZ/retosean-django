from .base import *


DEBUG = False

# CFG-01: en produccion SECRET_KEY es obligatoria. Sin default ni fallback:
# si la variable no esta definida, Django falla de inmediato al arrancar
# (UndefinedValueError) en lugar de usar una clave insegura conocida.
SECRET_KEY = config("SECRET_KEY")

# En produccion ALLOWED_HOSTS es obligatorio: sin dominio real, Django rechaza
# todas las peticiones. No heredamos el default de localhost de base.py.
ALLOWED_HOSTS = config("ALLOWED_HOSTS", cast=Csv())
# CFG-08: solo origenes HTTPS explicitos definidos por el despliegue; no hay
# default vacio, arrancar sin la variable debe fallar rapido igual que ALLOWED_HOSTS.
CSRF_TRUSTED_ORIGINS = config("CSRF_TRUSTED_ORIGINS", cast=Csv())

# --- Seguridad de transporte ---
SECURE_SSL_REDIRECT = env_bool("SECURE_SSL_REDIRECT", default=True)
# CFG-03: X-Forwarded-Proto solo es fiable si el proxy perimetral la reescribe
# y borra la que envie el cliente (MAN-CFG-04). Por defecto no se confia en la
# cabecera; el despliegue lo activa con PROXY_SSL_HEADER_CONFIABLE=True cuando
# garantiza ese proxy.
if env_bool("PROXY_SSL_HEADER_CONFIABLE", default=False):
    SECURE_PROXY_SSL_HEADER = ("HTTP_X_FORWARDED_PROTO", "https")
else:
    SECURE_PROXY_SSL_HEADER = None
SECURE_HSTS_SECONDS = config("SECURE_HSTS_SECONDS", default=31536000, cast=int)
SECURE_HSTS_INCLUDE_SUBDOMAINS = True
SECURE_HSTS_PRELOAD = True

# --- Cookies ---
SESSION_COOKIE_SECURE = True
CSRF_COOKIE_SECURE = True
SESSION_COOKIE_HTTPONLY = True

# --- Cabeceras ---
SECURE_CONTENT_TYPE_NOSNIFF = True
X_FRAME_OPTIONS = "DENY"
SECURE_REFERRER_POLICY = "same-origin"
