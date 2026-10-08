"""Punto unico de creacion de notificaciones.

Antes cada vista construia sus `Notificacion.objects.create(...)` a mano, con
titulos distintos para el mismo hecho, sin deduplicacion y sin correo. Todo el
codigo de dominio deberia entrar por `notificar()` / `notificar_muchos()`.
"""

import logging

from django.conf import settings
from django.core.mail import send_mail
from django.db import IntegrityError, transaction
from django.template.loader import render_to_string

from .eventos import obtener
from .models import Notificacion, PreferenciaNotificacion

logger = logging.getLogger(__name__)


def _preferencias(usuario):
    preferencia, _ = PreferenciaNotificacion.objects.get_or_create(usuario=usuario)
    return preferencia


def notificar(usuario, evento, *, mensaje, titulo=None, link="", tipo=None,
              clave_dedupe="", enviar_email=None):
    """Crea una notificacion in-app y, si procede, la envia por correo.

    Devuelve la Notificacion creada, o None si el usuario silencio el evento o
    si ya existia una con la misma `clave_dedupe`.
    """
    if usuario is None or not getattr(usuario, "pk", None):
        return None

    definicion = obtener(evento)
    preferencia = _preferencias(usuario)
    if not preferencia.acepta(definicion.slug):
        return None

    try:
        with transaction.atomic():
            notificacion = Notificacion.objects.create(
                usuario=usuario,
                evento=definicion.slug,
                tipo=tipo or definicion.tipo,
                titulo=titulo or definicion.titulo,
                mensaje=mensaje,
                link=link,
                clave_dedupe=clave_dedupe,
            )
    except IntegrityError:
        # Ya existia una notificacion con esta clave: es el caso normal de los
        # recordatorios diarios, no un error.
        return None

    debe_enviar = enviar_email if enviar_email is not None else definicion.permite_email
    if debe_enviar and preferencia.recibir_email and usuario.email:
        # on_commit para no mandar correo si la vista que nos llamo hace rollback.
        transaction.on_commit(lambda: enviar_por_correo(notificacion))

    return notificacion


def notificar_muchos(usuarios, evento, *, mensaje, **kwargs):
    """Notifica a varios usuarios el mismo evento, saltando duplicados."""
    creadas = []
    vistos = set()
    for usuario in usuarios:
        if usuario is None or usuario.pk in vistos:
            continue
        vistos.add(usuario.pk)
        notificacion = notificar(usuario, evento, mensaje=mensaje, **kwargs)
        if notificacion:
            creadas.append(notificacion)
    return creadas


def notificar_admins(evento, *, mensaje, excluir=None, **kwargs):
    """Notifica a los administradores del sistema.

    Incluye tanto superusuarios como usuarios con rol ADMIN: filtrar solo por
    `is_superuser` dejaba fuera a los administradores creados desde el registro.
    """
    from django.db.models import Q

    from apps.usuarios.models import Usuario

    admins = Usuario.objects.filter(
        Q(is_superuser=True) | Q(rol="ADMIN"), is_active=True
    )
    if excluir is not None:
        admins = admins.exclude(pk=getattr(excluir, "pk", excluir))
    return notificar_muchos(admins, evento, mensaje=mensaje, **kwargs)


def enviar_por_correo(notificacion):
    """Envia la notificacion por correo. Nunca propaga excepciones."""
    base_url = getattr(settings, "SITE_URL", "").rstrip("/")
    contexto = {
        "notificacion": notificacion,
        "usuario": notificacion.usuario,
        "url_absoluta": f"{base_url}{notificacion.link}" if notificacion.link else "",
    }
    try:
        cuerpo_txt = render_to_string("notificaciones/email/notificacion.txt", contexto)
        cuerpo_html = render_to_string("notificaciones/email/notificacion.html", contexto)
        send_mail(
            subject=f"[RetosEAN] {notificacion.titulo}",
            message=cuerpo_txt,
            from_email=settings.DEFAULT_FROM_EMAIL,
            recipient_list=[notificacion.usuario.email],
            html_message=cuerpo_html,
            fail_silently=False,
        )
    except Exception as exc:  # noqa: BLE001 - un fallo de correo no debe tumbar la vista
        logger.warning("No se pudo enviar la notificacion %s: %s", notificacion.pk, exc)
        Notificacion.objects.filter(pk=notificacion.pk).update(error_envio=str(exc)[:1000])
        return False

    Notificacion.objects.filter(pk=notificacion.pk).update(
        enviada_por_correo=True, error_envio=""
    )
    return True
