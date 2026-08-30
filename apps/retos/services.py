from django.urls import reverse
from django.utils import timezone

from apps.notificaciones.services import notificar, notificar_muchos

from .models import HistorialEstadoReto


def registrar_cambio_estado(reto, estado_anterior, estado_nuevo, usuario, comentario=''):
    HistorialEstadoReto.objects.create(
        reto=reto,
        estado_anterior=estado_anterior or '',
        estado_nuevo=estado_nuevo,
        realizado_por=usuario,
        comentario=comentario,
    )


def _notificar_cambio_estado(reto, estado_anterior, estado_nuevo, comentario):
    """Avisa a la empresa (y al cerrar, a los profesores) del cambio de estado.

    La aprobacion y el rechazo de un reto no notificaban a nadie: la empresa
    solo se enteraba si entraba al detalle del reto por su cuenta.
    """
    link = reverse('retos:detalle', kwargs={'pk': reto.pk})
    etiqueta = reto.get_estado_display()

    mensaje = f'Tu reto "{reto.titulo}" paso a estado: {etiqueta}.'
    if comentario:
        mensaje += f'\nComentario del administrador: {comentario}'
    if estado_nuevo == 'aprobado' and reto.consecutivo:
        mensaje += f'\nNumero de reto asignado: {reto.consecutivo}.'

    evento = 'RETO_FINALIZADO' if estado_nuevo == 'finalizado' else 'RETO_ESTADO_CAMBIADO'
    tipo = {
        'aprobado': 'EXITO',
        'finalizado': 'EXITO',
        'rechazado': 'ERROR',
        'cancelado': 'ADVERTENCIA',
    }.get(estado_nuevo)

    notificar(
        reto.empresa, evento,
        mensaje=mensaje, link=link, tipo=tipo,
        clave_dedupe=f'reto:{reto.pk}:{estado_anterior}->{estado_nuevo}',
    )

    if estado_nuevo in {'finalizado', 'cancelado'}:
        from apps.usuarios.models import Usuario

        profesores = Usuario.objects.filter(
            integraciones__reto=reto
        ).distinct()
        notificar_muchos(
            profesores, evento,
            mensaje=f'El reto "{reto.titulo}" fue marcado como {etiqueta}.',
            link=link,
            clave_dedupe=f'reto:{reto.pk}:{estado_nuevo}:profesor',
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
    if estado_anterior != estado_nuevo:
        _notificar_cambio_estado(reto, estado_anterior, estado_nuevo, comentario)
