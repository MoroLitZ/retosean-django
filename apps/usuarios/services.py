"""Cambios de estado de cuentas de usuario (HU13).

Activar/inactivar y cambiar de rol quedan en la bitacora de actividad
(`LogActividad`) y en el historial de estados comun (`seguimiento.HistorialEstado`).
Django rechaza en cada peticion a un usuario con `is_active=False`, asi que
inactivar una cuenta cierra de inmediato sus sesiones abiertas.
"""

from django.db import transaction

from apps.seguimiento.services import registrar_cambio_estado

from .models import LogActividad


def _etiqueta_activo(activo):
    return "ACTIVO" if activo else "INACTIVO"


@transaction.atomic
def cambiar_estado_cuenta(usuario, activo, realizado_por, comentario=""):
    """Activa o inactiva una cuenta. No hace nada si ya esta en ese estado."""
    if usuario.is_active == activo:
        return False
    anterior = _etiqueta_activo(usuario.is_active)
    usuario.is_active = activo
    usuario.activo = activo
    usuario.save(update_fields=["is_active", "activo"])
    detalle = comentario or ("Cuenta activada." if activo else "Cuenta desactivada.")
    LogActividad.objects.create(
        usuario=usuario, identificador=usuario.username, accion="CAMBIO_ESTADO",
        detalle=detalle, realizado_por=realizado_por,
    )
    registrar_cambio_estado(
        usuario, anterior, _etiqueta_activo(activo), realizado_por, detalle, campo="is_active",
    )
    return True


@transaction.atomic
def cambiar_rol(usuario, nuevo_rol, realizado_por):
    """Cambia el rol de la cuenta. No hace nada si ya tiene ese rol."""
    anterior = usuario.rol
    if anterior == nuevo_rol:
        return False
    usuario.rol = nuevo_rol
    usuario.save(update_fields=["rol"])
    detalle = f'Rol cambiado de {anterior or "sin rol"} a {nuevo_rol}.'
    LogActividad.objects.create(
        usuario=usuario, identificador=usuario.username, accion="CAMBIO_ROL",
        detalle=detalle, realizado_por=realizado_por,
    )
    registrar_cambio_estado(usuario, anterior, nuevo_rol, realizado_por, detalle, campo="rol")
    return True
