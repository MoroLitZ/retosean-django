from django.utils import timezone

from .models import HistorialEstadoReto


def registrar_cambio_estado(reto, estado_anterior, estado_nuevo, usuario, comentario=''):
    HistorialEstadoReto.objects.create(
        reto=reto,
        estado_anterior=estado_anterior or '',
        estado_nuevo=estado_nuevo,
        realizado_por=usuario,
        comentario=comentario,
    )


def cambiar_estado_reto(reto, estado_nuevo, usuario, comentario=''):
    estado_anterior = reto.estado
    reto.estado = estado_nuevo
    if estado_nuevo == 'aprobado':
        reto.fecha_aprobacion = timezone.now()
        reto.asignar_consecutivo_si_falta()
    reto.comentarios_revision = comentario
    reto.save()
    registrar_cambio_estado(reto, estado_anterior, estado_nuevo, usuario, comentario)
