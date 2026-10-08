from django.shortcuts import render, redirect, get_object_or_404
from django.contrib.auth.decorators import login_required
from django.contrib import messages

from apps.retos.models import Reto
from apps.retos.views import rol_requerido
from apps.participaciones.models import MiembroEquipo, Postulacion as PostulacionReto
from apps.evaluacion.models import Entregable
from apps.usuarios.forms import EntregableForm
from apps.usuarios.views import _url_para_usuario


@rol_requerido('ESTUDIANTE')
def postular_a_reto(request, reto_id):
    """Crear postulación de estudiante a un reto"""
    reto = get_object_or_404(Reto, pk=reto_id, estado='aprobado')

    postulacion, creada = PostulacionReto.objects.get_or_create(
        reto=reto,
        estudiante=request.user,
        defaults={'estado': 'PENDIENTE'},
    )

    if creada:
        messages.success(request, f'¡Tu postulación al reto "{reto.titulo}" fue enviada! Espera la aprobación de la empresa.')
    else:
        messages.info(request, 'Ya tienes una postulación enviada para este reto.')

    return redirect('retos:detalle', pk=reto.id)


@rol_requerido('ESTUDIANTE')
def panel_entregables(request):
    """Panel rápido de postulaciones aceptadas"""
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
        reto = get_object_or_404(Reto, pk=reto_id, estado__in=['aprobado', 'en_curso', 'pausado'])
        if not PostulacionReto.objects.filter(reto=reto, estudiante=request.user, estado='ACEPTADA').exists():
            messages.error(request, 'Solo los estudiantes aceptados pueden cargar entregables.')
            return redirect('participaciones:mis_entregables')
        # ... (tu lógica de validación de acceso se mantiene igual) ...
        
        if request.method == 'POST':
            form = EntregableForm(request.POST, request.FILES)
            if form.is_valid():
                # CORRECCIÓN: Actualizar o Crear para evitar IntegrityError
                miembro = MiembroEquipo.objects.filter(
                    equipo__reto=reto, estudiante=request.user
                ).select_related('equipo').first()
                entregable, created = Entregable.objects.update_or_create(
                    reto=reto,
                    estudiante=request.user,
                    titulo=form.cleaned_data['titulo'],
                    defaults={
                        'archivo': form.cleaned_data['archivo'],
                        'equipo': miembro.equipo if miembro else None,
                        'es_final': form.cleaned_data['es_final'],
                        'comentario_estudiante': form.cleaned_data['comentario_estudiante'],
                        'nota': None,
                        'comentario_profesor': '',
                        'estado': 'ENVIADO',
                    }
                )
                if form.cleaned_data['comentario_estudiante']:
                    entregable.historial_comentarios.create(
                        autor=request.user, comentario=form.cleaned_data['comentario_estudiante']
                    )
                messages.success(request, '¡Entregable guardado con éxito!')
                return redirect('participaciones:mis_entregables_reto', reto_id=reto.id)
        else:
            form = EntregableForm()
            
        entregables_subidos = Entregable.objects.filter(reto=reto, estudiante=request.user).prefetch_related(
            'historial_comentarios__autor'
        ).order_by('-fecha_entrega')

    return render(request, 'estudiante/mis_entregables.html', {
        'titulo': 'Mis Entregables',
        'reto': reto,
        'form': form,
        'entregables_subidos': entregables_subidos,
        'postulaciones': todas_mis_postulaciones
    })
