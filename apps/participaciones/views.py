from django.shortcuts import render, redirect, get_object_or_404
from django.contrib.auth.decorators import login_required
from django.contrib import messages

from apps.retos.models import Reto
from apps.retos.views import rol_requerido
from apps.participaciones.models import Postulacion as PostulacionReto
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
    """Gestión completa de entregables del estudiante"""
    if request.user.rol != 'ESTUDIANTE':
        return redirect(_url_para_usuario(request.user))
    
    if reto_id:
        reto = get_object_or_404(Reto, pk=reto_id, estado='aprobado')
        tiene_acceso = PostulacionReto.objects.filter(reto=reto, estudiante=request.user).exists()
        
        if not tiene_acceso:
            messages.error(request, 'No puedes gestionar entregables si no te has postulado a este reto.')
            return redirect('retos:detalle', pk=reto.id)
            
        entregables_subidos = Entregable.objects.filter(reto=reto, estudiante=request.user).order_by('-fecha_entrega')
    else:
        reto = None
        entregables_subidos = []

    if request.method == 'POST' and reto:
        form = EntregableForm(request.POST, request.FILES)
        if form.is_valid():
            nuevo_entregable = form.save(commit=False)
            nuevo_entregable.estudiante = request.user
            nuevo_entregable.reto = reto
            nuevo_entregable.estado = 'ENVIADO'
            nuevo_entregable.save()
            messages.success(request, '¡El entregable seleccionado ha sido cargado con éxito!')
            return redirect('participaciones:mis_entregables', reto_id=reto.id)
    else:
        form = EntregableForm() if reto else None
        
    todas_mis_postulaciones = PostulacionReto.objects.filter(estudiante=request.user).select_related('reto')
        
    return render(request, 'estudiante/mis_entregables.html', {
        'titulo': 'Mis Entregables de Proyecto',
        'reto': reto,
        'form': form,
        'entregables_subidos': entregables_subidos,
        'postulaciones': todas_mis_postulaciones
    })
