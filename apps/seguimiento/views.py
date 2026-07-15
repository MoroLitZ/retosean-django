from django.shortcuts import render, redirect
from django.contrib.auth.decorators import login_required
from django.contrib import messages

from apps.retos.models import Reto
from apps.participaciones.models import Postulacion as PostulacionReto
from apps.evaluacion.models import Entregable
from .models import IntegracionAcademica
from apps.usuarios.views import _url_para_usuario


@login_required(login_url='usuarios:login')
def entregables_profesor(request):
    """Panel de control académico para profesores - ver entregables de sus retos integrados"""
    if request.user.rol != 'PROFESOR':
        return redirect(_url_para_usuario(request.user))

    retos_integrados = IntegracionAcademica.objects.filter(
        profesor=request.user
    ).values_list('reto_id', flat=True)

    entregables_academia = Entregable.objects.filter(
        reto_id__in=retos_integrados
    ).select_related('reto', 'estudiante').order_by('-id')

    todos_los_retos = Reto.objects.filter(
        id__in=retos_integrados
    ).select_related('empresa').order_by('-id')

    return render(request, 'profesor/entregables.html', {
        'titulo': 'Panel de Control Académico',
        'entregables': entregables_academia,
        'retos': todos_los_retos
    })


@login_required(login_url='usuarios:login')
def mis_cursos(request):
    """Gestión de cursos y retos vinculados para profesores"""
    if request.user.rol != 'PROFESOR':
        return redirect(_url_para_usuario(request.user))

    integraciones = IntegracionAcademica.objects.filter(
        profesor=request.user
    ).select_related('reto').order_by('-creado_en')

    return render(request, 'profesor/mis_cursos.html', {
        'titulo': 'Mis Cursos y Retos Vinculados',
        'integraciones': integraciones,
    })


@login_required(login_url='usuarios:login')
def retos_vinculados(request):
    """Redirección a mis integraciones en retos app"""
    return redirect('retos:mis_integraciones')


@login_required(login_url='usuarios:login')
def mis_estudiantes(request):
    """Gestión de estudiantes vinculados a los retos del profesor"""
    if request.user.rol != 'PROFESOR':
        return redirect(_url_para_usuario(request.user))

    retos_integrados = IntegracionAcademica.objects.filter(
        profesor=request.user
    ).values_list('reto_id', flat=True)

    postulaciones_aceptadas = PostulacionReto.objects.filter(
        reto_id__in=retos_integrados,
        estado='ACEPTADA',
    ).select_related('estudiante', 'reto').order_by('reto', 'estudiante__last_name')

    return render(request, 'profesor/mis_estudiantes.html', {
        'titulo': 'Mis Estudiantes',
        'postulaciones': postulaciones_aceptadas,
    })


@login_required(login_url='usuarios:login')
def evaluaciones(request):
    """Sistema de evaluación y calificación para profesores"""
    if request.user.rol != 'PROFESOR':
        return redirect(_url_para_usuario(request.user))

    retos_integrados = IntegracionAcademica.objects.filter(
        profesor=request.user
    ).values_list('reto_id', flat=True)

    entregables = Entregable.objects.filter(
        reto_id__in=retos_integrados
    ).select_related('reto', 'estudiante').order_by('estado', '-fecha_entrega')

    if request.method == 'POST':
        entregable_id = request.POST.get('entregable_id')
        nota = request.POST.get('nota')
        comentario = request.POST.get('comentario', '')

        if entregable_id and nota:
            try:
                entregable = Entregable.objects.get(pk=entregable_id, reto_id__in=retos_integrados)
                entregable.nota = nota
                entregable.comentario_profesor = comentario
                entregable.estado = 'CALIFICADO'
                entregable.save(update_fields=['nota', 'comentario_profesor', 'estado', 'actualizado_en'])
                messages.success(request, 'Calificación registrada exitosamente.')
            except Entregable.DoesNotExist:
                messages.error(request, 'No tienes permisos para calificar este entregable.')
        return redirect('seguimiento:evaluaciones')

    return render(request, 'profesor/evaluaciones.html', {
        'titulo': 'Evaluaciones y Calificaciones',
        'entregables': entregables,
    })
