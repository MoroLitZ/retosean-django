"""Utilidades de registro: redaccion de datos sensibles en los logs (CFG-10)."""
import logging
import re


# Pares clave=valor tipicos de volcados de formularios, sesiones o peticiones.
PATRON_SENSIBLE = re.compile(
    r"(password|passwd|contrasena|token|secret|authorization)"
    r"(['\"]?\s*[:=]\s*)"
    r"(\S+)",
    re.IGNORECASE,
)


class FormateadorSeguro(logging.Formatter):
    """Formatter que sustituye por *** los valores sensibles del mensaje."""

    def __init__(self, format=None, datefmt=None, style="%", validate=True):
        super().__init__(fmt=format, datefmt=datefmt, style=style, validate=validate)

    def format(self, record):
        mensaje = super().format(record)
        return PATRON_SENSIBLE.sub(lambda m: m.group(1) + m.group(2) + "***", mensaje)
