"""Registra accesos en LogActividad (HU13)."""

import ipaddress

from django.contrib.auth.signals import (
    user_logged_in,
    user_logged_out,
    user_login_failed,
)
from django.dispatch import receiver

from .models import LogActividad


def extraer_ip(request):
    """IP del cliente, saneada para evitar un 500 en la bitacora.

    Prefiere el X-Forwarded-For (detras de un proxy) y cae a REMOTE_ADDR; solo
    se acepta un valor parseable como IPv4/IPv6, de modo que una cabecera
    invalida ya no revienta al guardar `LogActividad.ip` (GenericIPAddressField).
    """
    if request is None:
        return None
    reenviada = (request.META.get("HTTP_X_FORWARDED_FOR") or "").split(",")[0].strip()
    for candidato in (reenviada, request.META.get("REMOTE_ADDR")):
        if not candidato:
            continue
        try:
            return str(ipaddress.ip_address(candidato))
        except ValueError:
            continue
    return None


@receiver(user_logged_in)
def registrar_login(sender, request, user, **kwargs):
    LogActividad.objects.create(
        usuario=user, identificador=user.username, accion="LOGIN", ip=extraer_ip(request)
    )


@receiver(user_logged_out)
def registrar_logout(sender, request, user, **kwargs):
    if user is None:
        return
    LogActividad.objects.create(
        usuario=user, identificador=user.username, accion="LOGOUT", ip=extraer_ip(request)
    )


@receiver(user_login_failed)
def registrar_login_fallido(sender, credentials, request=None, **kwargs):
    LogActividad.objects.create(
        identificador=(credentials or {}).get("username", "")[:150],
        accion="LOGIN_FALLIDO",
        ip=extraer_ip(request),
    )
