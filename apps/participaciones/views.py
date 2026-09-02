from django.shortcuts import render, redirect, get_object_or_404
from django.contrib.auth.decorators import login_required
from django.contrib import messages
from django.utils import timezone
from django.utils.http import url_has_allowed_host_and_scheme
from django.views.decorators.http import require_POST

from apps.retos.models import Reto
from apps.participaciones.models import Postulacion as PostulacionReto
from apps.participaciones.models import RetoFavorito
from apps.participaciones.forms import PostulacionForm
from apps.evaluacion.models import Entregable
from apps.usuarios.forms import EntregableForm
from apps.usuarios.decorators import solo_estudiante


@solo_estudiante
def postular_a_reto(request, reto_id):
    """Formulario de postulacion del estudiante a un reto.

    Se aceptan tambien los retos 'en_curso': son los que el explorador lista
    junto a los aprobados, y exigir solo 'aprobado' daba un 404 desde ahi.
    """
    reto = get_object_or_404(Reto, pk=reto_id, estado__in=['aprobado', 'en_curso'])

    if reto.fecha_limite_postulacion and reto.fecha_limite_postulacion < timezone.localdate():
        messages.error(
            request,
            'La fecha limite de postulacion de este reto fue el '
            + reto.fecha_limite_postulacion.strftime('%d/%m/%Y') + '.'
        )
        return redirect('retos:detalle', pk=reto.id)

    postulacion = PostulacionReto.objects.filter(reto=reto, estudiante=request.user).first()

    if postulacion and postulacion.estado == 'ACEPTADA':
        messages.info(request, 'Ya fuiste aceptado en este reto. No es necesario volver a postularte.')
        return redirect('retos:detalle', pk=reto.id)

    # Una postulacion ya resuelta no se reabre editando el formulario:
    # antes form.save() la devolvia a PENDIENTE sin ningun control.
    if postulacion and postulacion.estado == 'RECHAZADA':
        messages.error(request, 'Tu postulacion a este reto fue rechazada y no puede reenviarse.')
        return redirect('academico:mis_postulaciones')

    creada = postulacion is None
    if creada:
        postulacion = PostulacionReto(reto=reto, estudiante=request.user, estado='PENDIENTE')

    inicial = {}
    perfil = getattr(request.user, 'perfil_estudiante', None)
    if perfil:
        inicial['semestre'] = perfil.semestre
        if perfil.programa:
            inicial['programa'] = perfil.programa.nombre

    form = PostulacionForm(request.POST or None, instance=postulacion, initial=inicial)
    if request.method == 'POST' and form.is_valid():
        postulacion = form.save(commit=False)
        postulacion.estado = 'PENDIENTE'
        postulacion.save()
        _notificar_nueva_postulacion(reto, request.user)
        messages.success(
            request,
            '\u00a1Tu postulaci\u00f3n al reto "' + reto.titulo + '" fue enviada! '
            'La empresa fue notificada y revisara tu solicitud.'
        )
        return redirect('academico:mis_postulaciones')

    return render(request, 'participaciones/postulacion_form.html', {
        'form': form,
        'reto': reto,
        'titulo': 'Postularse a: ' + reto.titulo,
        'ya_postulado': (not creada) and postulacion.estado == 'PENDIENTE',
    })


def _notificar_nueva_postulacion(reto, estudiante):
    """Avisa a la empresa dueña del reto sobre una postulacion nueva."""
    from django.urls import reverse

    from apps.notificaciones.services import notificar

    nombre = estudiante.get_full_name() or estudiante.username
    link = reverse('retos:detalle', kwargs={'pk': reto.pk})
    clave = f'postulacion-nueva:{reto.pk}:{estudiante.pk}'

    notificar(
        reto.empresa, 'POSTULACION_NUEVA',
        titulo='Nueva postulacion recibida',
        mensaje=f'{nombre} se ha postulado a tu reto "{reto.titulo}".',
        tipo='EXITO', link=link, clave_dedupe=clave,
    )


@require_POST
@solo_estudiante
def toggle_favorito(request, reto_id):
    """Marca o desmarca un reto como favorito del estudiante."""
    reto = get_object_or_404(Reto, pk=reto_id)
    favorito, creado = RetoFavorito.objects.get_or_create(usuario=request.user, reto=reto)
    if creado:
        messages.success(request, 'Reto agregado a favoritos.')
    else:
        favorito.delete()
        messages.success(request, 'Reto eliminado de favoritos.')

    # Solo volvemos al origen si es una URL interna: un HTTP_REFERER externo
    # convertiria esta vista en un open redirect.
    destino = request.POST.get('next') or request.META.get('HTTP_REFERER', '')
    if destino and url_has_allowed_host_and_scheme(
        destino, allowed_hosts={request.get_host()}, require_https=request.is_secure()
    ):
        return redirect(destino)
    return redirect('academico:explorar_retos')


@solo_estudiante
def mis_favoritos(request):
    """Retos que el estudiante marco con la estrella (HU11).

    La estrella existia desde el Sprint 5 y el dashboard contaba los favoritos,
    pero no habia ninguna pantalla donde verlos.
    """
    favoritos = (
        RetoFavorito.objects
        .filter(usuario=request.user)
        .select_related('reto', 'reto__empresa', 'reto__facultad', 'reto__programa')
        .order_by('-fecha')
    )
    postulados = set(
        PostulacionReto.objects
        .filter(estudiante=request.user)
        .values_list('reto_id', flat=True)
    )
    return render(request, 'participaciones/mis_favoritos.html', {
        'titulo': 'Mis Retos Favoritos',
        'favoritos': favoritos,
        'postulados': postulados,
    })


@require_POST
@solo_estudiante
def retirar_postulacion(request, reto_id):
    """El estudiante retira una postulacion que aun no ha sido resuelta."""
    postulacion = get_object_or_404(
        PostulacionReto, reto_id=reto_id, estudiante=request.user
    )
    if postulacion.estado != 'PENDIENTE':
        messages.error(
            request,
            'Solo puedes retirar una postulacion que siga pendiente de respuesta.'
        )
        return redirect('academico:mis_postulaciones')

    postulacion.estado = 'RETIRADA'
    postulacion.save(update_fields=['estado'])
    _notificar_postulacion_retirada(postulacion)
    messages.success(
        request,
        f'Retiraste tu postulacion al reto "{postulacion.reto.titulo}".'
    )
    return redirect('academico:mis_postulaciones')


def _notificar_postulacion_retirada(postulacion):
    from django.urls import reverse

    from apps.notificaciones.services import notificar

    estudiante = postulacion.estudiante
    nombre = estudiante.get_full_name() or estudiante.username
    notificar(
        postulacion.reto.empresa, 'POSTULACION_GESTIONADA',
        titulo='Postulacion retirada',
        mensaje=f'{nombre} retiro su postulacion al reto "{postulacion.reto.titulo}".',
        tipo='ADVERTENCIA',
        link=reverse('empresas:postulaciones'),
        clave_dedupe=f'postulacion:{postulacion.pk}:RETIRADA',
    )


@solo_estudiante
def panel_entregables(request):
    """Indice de retos donde el estudiante fue aceptado, para entrar a entregar.

    Apuntaba a 'usuarios/mis_entregables.html', plantilla que no existe: devolvia
    un 500 a quien llegara por URL. La plantilla del estudiante ya resuelve este
    caso cuando no se le pasa un reto concreto, asi que se reutiliza en lugar de
    duplicarla.
    """
    postulaciones_aceptadas = (
        PostulacionReto.objects
        .filter(estudiante=request.user, estado='ACEPTADA')
        .select_related('reto', 'reto__empresa')
        .order_by('-fecha_postulacion')
    )

    return render(request, 'estudiante/mis_entregables.html', {
        'titulo': 'Mis Entregables',
        'postulaciones': postulaciones_aceptadas,
    })


def _estudiante_participa_en(usuario, reto):
    """El estudiante participa si fue aceptado, asignado a un equipo o ya envio un entregable."""
    from apps.evaluacion.models import Entregable
    from apps.retos.models import EquipoRetoAcademico

    if PostulacionReto.objects.filter(
        reto=reto, estudiante=usuario, estado='ACEPTADA'
    ).exists():
        return True
    if EquipoRetoAcademico.objects.filter(reto=reto, estudiantes=usuario).exists():
        return True
    return Entregable.objects.filter(reto=reto, estudiante=usuario).exists()


@solo_estudiante
def mis_entregables(request, reto_id=None):
    from apps.retos.models import EquipoRetoAcademico

    reto = None
    entregables_subidos = []
    form = None

    retos_aceptados_ids = PostulacionReto.objects.filter(
        estudiante=request.user, estado='ACEPTADA'
    ).values_list('reto_id', flat=True)
    retos_equipo_ids = EquipoRetoAcademico.objects.filter(
        estudiantes=request.user
    ).values_list('reto_id', flat=True)
    retos_entregable_ids = Entregable.objects.filter(
        estudiante=request.user
    ).values_list('reto_id', flat=True)
    todos_mis_retos_ids = set(retos_aceptados_ids) | set(retos_equipo_ids) | set(retos_entregable_ids)

    retos_participando = Reto.objects.filter(
        id__in=todos_mis_retos_ids
    ).select_related('empresa').order_by('-creado_en')

    if not reto_id and request.method == 'POST':
        messages.error(request, 'Elige primero el reto al que quieres subir el entregable.')
        return redirect('participaciones:mis_entregables')

    if reto_id:
        reto = get_object_or_404(Reto, pk=reto_id, estado__in=['aprobado', 'en_curso', 'finalizado'])
        if not _estudiante_participa_en(request.user, reto):
            messages.error(
                request,
                'Solo puedes ver entregables de retos en los que participas.'
            )
            return redirect('participaciones:mis_entregables')

        if request.method == 'POST':
            if reto.estado == 'finalizado':
                messages.error(request, 'Este reto ya fue cerrado formalmente y no recibe mas entregables.')
                return redirect('participaciones:mis_entregables_reto', reto_id=reto.id)

            form = EntregableForm(request.POST, request.FILES)
            if form.is_valid():
                titulo = form.cleaned_data.get('titulo') or 'Entregable del reto'
                entregable, created = Entregable.objects.get_or_create(
                    reto=reto,
                    estudiante=request.user,
                    titulo=titulo,
                    defaults={
                        'archivo': form.cleaned_data['archivo'],
                        'es_final': form.cleaned_data.get('es_final', False),
                        'comentario_estudiante': form.cleaned_data.get('comentario_estudiante', ''),
                        'estado': 'ENVIADO',
                    }
                )
                if not created:
                    entregable.archivo = form.cleaned_data['archivo']
                    entregable.es_final = form.cleaned_data.get('es_final', False)
                    entregable.comentario_estudiante = form.cleaned_data.get('comentario_estudiante', '')
                    entregable.estado = 'ENVIADO'
                    entregable.save()
                _notificar_entregable_recibido(reto, request.user, entregable)
                messages.success(request, 'Entregable guardado con exito!')
                return redirect('participaciones:mis_entregables_reto', reto_id=reto.id)
        else:
            if reto.estado != 'finalizado':
                form = EntregableForm()

        entregables_subidos = (
            Entregable.objects
            .filter(reto=reto, estudiante=request.user)
            .prefetch_related('historial_comentarios__autor')
            .order_by('-fecha_entrega')
        )

    return render(request, 'estudiante/mis_entregables.html', {
        'titulo': 'Mis Entregables',
        'reto': reto,
        'form': form,
        'entregables_subidos': entregables_subidos,
        'postulaciones': retos_participando,
        'retos_participando': retos_participando,
    })


def _notificar_entregable_recibido(reto, estudiante, entregable):
    """Avisa a los profesores que integraron el reto que hay algo por calificar."""
    from django.urls import reverse

    from apps.notificaciones.services import notificar_muchos
    from apps.usuarios.models import Usuario

    profesores = Usuario.objects.filter(integraciones__reto=reto).distinct()
    nombre = estudiante.get_full_name() or estudiante.username
    notificar_muchos(
        profesores, 'ENTREGABLE_RECIBIDO',
        mensaje=f'{nombre} subio el entregable "{entregable.titulo}" del reto "{reto.titulo}".',
        link=reverse('evaluacion:panel_profesor'),
        clave_dedupe=f'entregable-recibido:{entregable.pk}:{entregable.actualizado_en:%Y%m%d%H%M}',
    )
