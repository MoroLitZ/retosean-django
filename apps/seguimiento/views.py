from django.shortcuts import render, redirect
from django.contrib.auth.decorators import login_required
from django.contrib import messages
from django.core.paginator import Paginator

from apps.retos.models import Reto
from apps.participaciones.models import Postulacion as PostulacionReto
from apps.evaluacion.models import Entregable, Rubrica
from .models import IntegracionAcademica
from apps.usuarios.decorators import solo_profesor


@solo_profesor
def entregables_profesor(request):
    """Panel de control académico para profesores - ver entregables de sus retos integrados"""

    retos_integrados = IntegracionAcademica.objects.filter(
        profesor=request.user
    ).values_list('reto_id', flat=True)

    entregables_academia = Entregable.objects.filter(
        reto_id__in=retos_integrados
    ).select_related('reto', 'estudiante', 'equipo').prefetch_related("historial_comentarios__autor").order_by('-id')

    todos_los_retos = Reto.objects.filter(
        id__in=retos_integrados
    ).select_related('empresa').order_by('-id')

    rubricas = Rubrica.objects.filter(
        reto_id__in=retos_integrados, activa=True
    ).prefetch_related("criterios")

    paginator = Paginator(entregables_academia, 25)
    page_obj = paginator.get_page(request.GET.get('page'))

    return render(request, 'profesor/entregables.html', {
        'titulo': 'Panel de Control Académico',
        'entregables': page_obj,
        'page_obj': page_obj,
        'retos': todos_los_retos,
        'rubricas': rubricas,
    })


@solo_profesor
def mis_cursos(request):
    """Gestión de cursos y retos vinculados para profesores"""

    integraciones = IntegracionAcademica.objects.filter(
        profesor=request.user
    ).select_related('reto').order_by('-creado_en')

    paginator = Paginator(integraciones, 25)
    page_obj = paginator.get_page(request.GET.get('page'))

    return render(request, 'profesor/mis_cursos.html', {
        'titulo': 'Mis Cursos y Retos Vinculados',
        'integraciones': page_obj,
        'page_obj': page_obj,
    })


@login_required(login_url='usuarios:login')
def retos_vinculados(request):
    """Redirección a mis integraciones en retos app"""
    return redirect('retos:mis_integraciones')


@solo_profesor
def mis_estudiantes(request):
    """Gestión de estudiantes vinculados a los retos del profesor"""

    retos_integrados = IntegracionAcademica.objects.filter(
        profesor=request.user
    ).values_list('reto_id', flat=True)

    postulaciones_aceptadas = PostulacionReto.objects.filter(
        reto_id__in=retos_integrados,
        estado='ACEPTADA',
    ).select_related('estudiante', 'reto').order_by('reto', 'estudiante__last_name')

    paginator = Paginator(postulaciones_aceptadas, 25)
    page_obj = paginator.get_page(request.GET.get('page'))

    return render(request, 'profesor/mis_estudiantes.html', {
        'titulo': 'Mis Estudiantes',
        'postulaciones': page_obj,
        'page_obj': page_obj,
    })


@solo_profesor
def evaluaciones(request):
    """Alias historico.

    Existia un segundo flujo de calificacion aqui que escribia la nota
    directamente sobre el Entregable, sin validar el rango, sin registrar
    Evaluacion y sin historial. Se retiro para dejar un unico camino:
    evaluacion:panel_profesor.
    """
    return redirect('evaluacion:panel_profesor')
