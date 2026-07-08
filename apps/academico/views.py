from django.shortcuts import render, redirect
from django.contrib.auth.decorators import login_required
from django.contrib import messages

from apps.retos.models import Reto
from apps.participaciones.models import Postulacion as PostulacionReto
from apps.usuarios.views import _url_para_usuario


@login_required(login_url='usuarios:login')
def explorar_retos(request):
    """Vista para que estudiantes exploren retos disponibles"""
    if hasattr(request.user, 'rol') and request.user.rol.upper() != 'ESTUDIANTE' and not request.user.is_superuser:
        return redirect(_url_para_usuario(request.user))
        
    # Cambiamos '-fecha_creacion' por '-creado_en'
    retos_disponibles = Reto.objects.all().order_by('-creado_en') if 'Reto' in globals() else []
    return render(request, 'estudiante/explorar_retos.html', {
        'retos': retos_disponibles, 'titulo': 'Explorar Retos Disponibles'
    })


@login_required(login_url='usuarios:login')
def mis_postulaciones(request):
    """Ver todas las postulaciones del estudiante"""
    if request.user.rol != 'ESTUDIANTE':
        return redirect(_url_para_usuario(request.user))
        
    postulaciones_usuario = PostulacionReto.objects.filter(estudiante=request.user).select_related('reto')
    
    return render(request, 'estudiante/mis_postulaciones.html', {
        'postulaciones': postulaciones_usuario,
        'titulo': 'Mis Inscripciones a Retos'
    })


@login_required(login_url='usuarios:login')
def certificados(request):
    """Vista de certificados del estudiante"""
    if request.user.rol != 'ESTUDIANTE':
        return redirect(_url_para_usuario(request.user))
        
    return render(request, 'estudiante/certificados.html', {
        'titulo': 'Mis Certificados Obtenidos'
    })
