"""Registra accesos en LogActividad (HU13)."""

from django.contrib.auth.signals import (
    user_logged_in,
    user_logged_out,
    user_login_failed,
)
from django.dispatch import receiver

from .models import LogActividad


def _ip(request):
    if request is None:
        return None
    reenviada = request.META.get("HTTP_X_FORWARDED_FOR", "")
    if reenviada:
        return reenviada.split(",")[0].strip()
    return request.META.get("REMOTE_ADDR")


@receiver(user_logged_in)
def registrar_login(sender, request, user, **kwargs):
    LogActividad.objects.create(
        usuario=user, identificador=user.username, accion="LOGIN", ip=_ip(request)
    )


@receiver(user_logged_out)
def registrar_logout(sender, request, user, **kwargs):
    if user is None:
        return
    LogActividad.objects.create(
        usuario=user, identificador=user.username, accion="LOGOUT", ip=_ip(request)
    )


@receiver(user_login_failed)
def registrar_login_fallido(sender, credentials, request=None, **kwargs):
    LogActividad.objects.create(
        identificador=(credentials or {}).get("username", "")[:150],
        accion="LOGIN_FALLIDO",
        ip=_ip(request),
    )
