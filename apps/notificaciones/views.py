from django.contrib import messages
from django.contrib.auth.decorators import login_required
from django.core.paginator import Paginator
from django.shortcuts import get_object_or_404, redirect, render
from django.utils import timezone
from django.utils.http import url_has_allowed_host_and_scheme
from django.views.decorators.http import require_POST

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
