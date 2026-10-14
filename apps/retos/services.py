from django.db import transaction
from django.urls import reverse
from django.utils import timezone

from apps.notificaciones.services import notificar, notificar_muchos
from apps.seguimiento.services import MaquinaEstados, TransicionInvalida

from .models import HistorialEstadoReto, Reto


# Ciclo de vida unificado del reto/hackathon (brief, seccion 3).
MAQUINA_RETO = MaquinaEstados(
    'el reto',
    {
        'borrador': {'en_revision', 'cancelado'},
        'en_revision': {'aprobado', 'rechazado', 'cancelado'},
        'rechazado': {'borrador', 'cancelado'},
        # Aprobado -> Abierto ocurre al publicar una integracion academica aprobada.
        'aprobado': {'abierto', 'cancelado'},
        'abierto': {'en_curso', 'cancelado'},
        'en_curso': {'pausado', 'finalizado', 'cancelado'},
        'pausado': {'en_curso', 'cancelado'},
        'finalizado': set(),
        'cancelado': set(),
    },
    dict(Reto.ESTADO_CHOICES),
)

# Estados que el administrador fija a mano desde "Cambiar estado".
ESTADOS_CAMBIO_MANUAL = {'en_curso', 'pausado', 'cancelado'}


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
    if estado_nuevo == 'aprobado' and reto.codigo:
        mensaje += f'\nNumero de reto asignado: {reto.codigo}.'

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


def cambiar_estado_reto(reto, estado_nuevo, usuario, comentario='', notificar_empresa=True):
    """Unico punto de cambio de estado de un reto.

    Rechaza con `TransicionInvalida` lo que no permita `MAQUINA_RETO`, aplica
    los efectos de cada estado (fecha de envio, consecutivo, comentario de
    revision), deja el cambio en el historial y avisa a la empresa.
    `notificar_empresa=False` es para los cambios que hace la propia empresa.
    """
    estado_anterior = reto.estado
    MAQUINA_RETO.validar(estado_anterior, estado_nuevo)
    with transaction.atomic():
        reto.estado = estado_nuevo
        if estado_nuevo == 'en_revision':
            reto.fecha_envio_revision = timezone.now()
        if estado_nuevo == 'aprobado':
            reto.fecha_aprobacion = timezone.now()
            reto.asignar_consecutivo_si_falta()
        if estado_nuevo in {'aprobado', 'rechazado'}:
            reto.comentarios_revision = comentario
        elif estado_nuevo == 'borrador':
            reto.comentarios_revision = ''
        reto.save()
        registrar_cambio_estado(reto, estado_anterior, estado_nuevo, usuario, comentario)
    if notificar_empresa:
        _notificar_cambio_estado(reto, estado_anterior, estado_nuevo, comentario)


def estados_manuales_disponibles(reto):
    """Estados a los que el administrador puede mover el reto desde "Cambiar estado".

    Aprobar/rechazar, abrir (publicar integracion) y finalizar (cierre formal)
    tienen su propio flujo y no se ofrecen aqui.
    """
    return [
        (valor, etiqueta)
        for valor, etiqueta in Reto.ESTADO_CHOICES
        if valor in ESTADOS_CAMBIO_MANUAL and MAQUINA_RETO.puede(reto.estado, valor)
    ]


def eliminar_reto_borrador(reto, usuario):
    """Borrado logico de un reto en borrador; deja constancia en el historial."""
    if not reto.puede_eliminar_empresa:
        raise TransicionInvalida(
            f'Solo se pueden eliminar retos en borrador; este esta en "{reto.get_estado_display()}".'
        )
    reto.eliminado_en = timezone.now()
    reto.save(update_fields=['eliminado_en', 'actualizado_en'])
    registrar_cambio_estado(reto, reto.estado, reto.estado, usuario, 'Reto eliminado por la empresa.')
