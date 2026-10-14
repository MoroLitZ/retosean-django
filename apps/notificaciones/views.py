from django.contrib import messages
from django.contrib.auth.decorators import login_required
from django.core.paginator import Paginator
from django.db.models import Count, Q
from django.shortcuts import get_object_or_404, redirect, render
from django.utils import timezone
from django.utils.http import url_has_allowed_host_and_scheme
from django.views.decorators.http import require_POST

from apps.usuarios.decorators import solo_admin

from .models import Notificacion, PreferenciaNotificacion
from .eventos import EVENTOS


def _destino_seguro(request, fallback='notificaciones:bandeja'):
    destino = request.POST.get('next') or request.META.get('HTTP_REFERER', '')
    if destino and url_has_allowed_host_and_scheme(
        destino, allowed_hosts={request.get_host()}, require_https=request.is_secure()
    ):
        return destino
    return fallback


@login_required(login_url='usuarios:login')
def bandeja(request):
    """Listado completo de notificaciones del usuario."""
    filtro = request.GET.get('filtro', '')
    notificaciones = Notificacion.objects.filter(usuario=request.user)
    if filtro == 'no_leidas':
        notificaciones = notificaciones.filter(leida=False)
    elif filtro == 'leidas':
        notificaciones = notificaciones.filter(leida=True)

    paginator = Paginator(notificaciones, 20)
    return render(request, 'notificaciones/bandeja.html', {
        'titulo': 'Mis Notificaciones',
        'notificaciones': paginator.get_page(request.GET.get('page')),
        'filtro': filtro,
        'no_leidas': Notificacion.objects.filter(usuario=request.user, leida=False).count(),
    })


@require_POST
@login_required(login_url='usuarios:login')
def marcar_leida(request, pk):
    notificacion = get_object_or_404(Notificacion, pk=pk, usuario=request.user)
    notificacion.marcar_leida()
    if notificacion.link:
        return redirect(notificacion.link)
    return redirect(_destino_seguro(request))


@require_POST
@login_required(login_url='usuarios:login')
def marcar_todas_leidas(request):
    actualizadas = Notificacion.objects.filter(usuario=request.user, leida=False).update(
        leida=True, leida_en=timezone.now()
    )
    if actualizadas:
        messages.success(request, f'{actualizadas} notificaciones marcadas como leidas.')
    return redirect(_destino_seguro(request))


@login_required(login_url='usuarios:login')
def preferencias(request):
    """Permite al usuario elegir que avisos recibe y cuales silencia."""
    preferencia, _ = PreferenciaNotificacion.objects.get_or_create(usuario=request.user)

    if request.method == 'POST':
        preferencia.recibir_email = bool(request.POST.get('recibir_email'))
        preferencia.recibir_recordatorios = bool(request.POST.get('recibir_recordatorios'))
        # Llega la lista de eventos ACTIVOS; guardamos el complemento.
        activos = set(request.POST.getlist('eventos_activos'))
        preferencia.eventos_silenciados = [
            slug for slug in EVENTOS if slug not in activos
        ]
        preferencia.save()
        messages.success(request, 'Preferencias de notificacion actualizadas.')
        return redirect('notificaciones:preferencias')

    silenciados = set(preferencia.eventos_silenciados or [])
    return render(request, 'notificaciones/preferencias.html', {
        'titulo': 'Preferencias de Notificacion',
        'preferencia': preferencia,
        'eventos': [
            {'slug': slug, 'titulo': evento.titulo, 'activo': slug not in silenciados}
            for slug, evento in EVENTOS.items()
        ],
    })


ESTADOS_ENVIO = [
    ('enviado', 'Enviado por correo'),
    ('fallido', 'Fallido'),
    ('leido', 'Leído'),
    ('no_leido', 'No leído'),
    ('solo_app', 'Solo en la plataforma'),
]


@solo_admin
def registro_envios(request):
    """Registro de notificaciones para el administrador (HU15): estado de cada envio."""
    registros = Notificacion.objects.select_related('usuario').order_by('-creada_en')
    estado = request.GET.get('estado', '')
    evento = request.GET.get('evento', '')
    usuario_q = request.GET.get('usuario', '').strip()
    desde = request.GET.get('desde', '')
    hasta = request.GET.get('hasta', '')

    if estado == 'enviado':
        registros = registros.filter(enviada_por_correo=True)
    elif estado == 'fallido':
        registros = registros.exclude(error_envio='')
    elif estado == 'leido':
        registros = registros.filter(leida=True)
    elif estado == 'no_leido':
        registros = registros.filter(leida=False)
    elif estado == 'solo_app':
        registros = registros.filter(enviada_por_correo=False, error_envio='')
    if evento:
        registros = registros.filter(evento=evento)
    if usuario_q:
        registros = registros.filter(usuario__username__icontains=usuario_q)
    if desde:
        registros = registros.filter(creada_en__date__gte=desde)
    if hasta:
        registros = registros.filter(creada_en__date__lte=hasta)

    resumen = Notificacion.objects.aggregate(
        total=Count('id'),
        enviadas=Count('id', filter=Q(enviada_por_correo=True)),
        fallidas=Count('id', filter=~Q(error_envio='')),
        leidas=Count('id', filter=Q(leida=True)),
    )
    filtros = request.GET.copy()
    filtros.pop('page', None)
    return render(request, 'notificaciones/registro_envios.html', {
        'registros': Paginator(registros, 50).get_page(request.GET.get('page')),
        'resumen': resumen,
        'estados': ESTADOS_ENVIO,
        'eventos': sorted((slug, e.titulo) for slug, e in EVENTOS.items()),
        'filtros': {'estado': estado, 'evento': evento, 'usuario': usuario_q, 'desde': desde, 'hasta': hasta},
        'querystring': filtros.urlencode(),
    })
