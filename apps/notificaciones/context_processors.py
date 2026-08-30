"""Alimenta la campana de notificaciones del topbar."""

from .models import Notificacion

LIMITE_DESPLEGABLE = 5


def notificaciones_topbar(request):
    user = getattr(request, 'user', None)
    if user is None or not user.is_authenticated:
        return {'notificaciones_no_leidas': 0, 'notificaciones_recientes': []}

    recientes = list(
        Notificacion.objects
        .filter(usuario=user)
        .only('id', 'tipo', 'titulo', 'mensaje', 'link', 'leida', 'creada_en')[:LIMITE_DESPLEGABLE]
    )
    return {
        'notificaciones_no_leidas': Notificacion.objects.filter(usuario=user, leida=False).count(),
        'notificaciones_recientes': recientes,
    }
