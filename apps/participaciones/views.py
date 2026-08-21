from django.shortcuts import render, redirect, get_object_or_404
from django.contrib.auth.decorators import login_required
from django.contrib import messages

from apps.retos.models import Reto
from apps.retos.views import rol_requerido
from apps.participaciones.models import Postulacion as PostulacionReto
from apps.participaciones.models import RetoFavorito
from apps.participaciones.forms import PostulacionForm
from apps.evaluacion.models import Entregable
from apps.usuarios.forms import EntregableForm
from apps.usuarios.views import _url_para_usuario


@rol_requerido('ESTUDIANTE')
def postular_a_reto(request, reto_id):
    """Formulario de postulaci\u00f3n del estudiante a un reto"""
    reto = get_object_or_404(Reto, pk=reto_id, estado='aprobado')

    postulacion, creada = PostulacionReto.objects.get_or_create(
        reto=reto,
        estudiante=request.user,
        defaults={'estado': 'PENDIENTE'},
    )

    if not creada and postulacion.estado == 'ACEPTADA':
        messages.info(request, 'Ya fuiste aceptado en este reto. No es necesario volver a postularte.')
        return redirect('retos:detalle', pk=reto.id)

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
            'Un docente evaluar\u00e1 tu perfil y la empresa ser\u00e1 notificada.'
        )
        return redirect('academico:mis_postulaciones')

    return render(request, 'participaciones/postulacion_form.html', {
        'form': form,
        'reto': reto,
        'titulo': 'Postularse a: ' + reto.titulo,
        'ya_postulado': (not creada) and postulacion.estado == 'PENDIENTE',
    })


def _notificar_nueva_postulacion(reto, estudiante):
    """Notifica a la empresa del reto y a los administradores sobre una nueva postulaci\u00f3n."""
    from apps.notificaciones.models import Notificacion
    from apps.usuarios.models import Usuario

    destino_empresa = reto.empresa
    if destino_empresa and destino_empresa != estudiante:
        Notificacion.objects.create(
            usuario=destino_empresa,
            tipo='EXITO',
            titulo='Nueva postulaci\u00f3n recibida',
            mensaje=(estudiante.get_full_name() or estudiante.username)
                    + ' se ha postulado a tu reto "' + reto.titulo + '".',
            link='/retos/' + str(reto.id) + '/',
        )

    for admin in Usuario.objects.filter(is_superuser=True).exclude(pk=estudiante.pk):
        Notificacion.objects.create(
            usuario=admin,
            tipo='INFO',
            titulo='Nueva postulaci\u00f3n de estudiante',
            mensaje=(estudiante.get_full_name() or estudiante.username)
                    + ' se postul\u00f3 al reto "' + reto.titulo + '".',
            link='/retos/' + str(reto.id) + '/',
        )


@rol_requerido('ESTUDIANTE')
def toggle_favorito(request, reto_id):
    """Marca o desmarca un reto como favorito del estudiante."""
    reto = get_object_or_404(Reto, pk=reto_id)
    favorito, creado = RetoFavorito.objects.get_or_create(usuario=request.user, reto=reto)
    if creado:
        messages.success(request, 'Reto agregado a favoritos.')
    else:
        favorito.delete()
        messages.success(request, 'Reto eliminado de favoritos.')
    referer = request.META.get('HTTP_REFERER', '')
    return redirect(referer or 'academico:explorar_retos')


@rol_requerido('ESTUDIANTE')
def panel_entregables(request):
    """Panel rÃ¡pido de postulaciones aceptadas"""
    postulaciones_aceptadas = PostulacionReto.objects.filter(
        estudiante=request.user,
        estado='ACEPTADA'
    ).select_related('reto')

    return render(request, 'usuarios/mis_entregables.html', {
        'postulaciones': postulaciones_aceptadas
    })


@login_required(login_url='usuarios:login')
def mis_entregables(request, reto_id=None):
    if request.user.rol != 'ESTUDIANTE':
        return redirect(_url_para_usuario(request.user))
    
    reto = None
    entregables_subidos = []
    form = None
    todas_mis_postulaciones = PostulacionReto.objects.filter(estudiante=request.user).select_related('reto')

    if reto_id:
        reto = get_object_or_404(Reto, pk=reto_id, estado='aprobado')
        # ... (tu lÃ³gica de validaciÃ³n de acceso se mantiene igual) ...
        
        if request.method == 'POST':
            form = EntregableForm(request.POST, request.FILES)
            if form.is_valid():
                # Usa titulo del formulario (o default del modelo) para respetar
                # el unique_together (reto, estudiante, titulo).
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
                messages.success(request, 'Entregable guardado con exito!')
                return redirect('participaciones:mis_entregables_reto', reto_id=reto.id)
        else:
            form = EntregableForm()
            
        entregables_subidos = Entregable.objects.filter(reto=reto, estudiante=request.user).order_by('-fecha_entrega')

    return render(request, 'estudiante/mis_entregables.html', {
        'titulo': 'Mis Entregables',
        'reto': reto,
        'form': form,
        'entregables_subidos': entregables_subidos,
        'postulaciones': todas_mis_postulaciones
    })
