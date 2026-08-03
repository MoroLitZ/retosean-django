from django.shortcuts import render, redirect
from django.contrib.auth.decorators import login_required
from django.core.paginator import Paginator
from django.contrib import messages

from apps.retos.models import Reto
from apps.participaciones.models import Postulacion as PostulacionReto
from apps.academico.models import Facultad, Programa
from apps.usuarios.views import _url_para_usuario


@login_required(login_url='usuarios:login')
def explorar_retos(request):
    """Vista para que estudiantes exploren retos disponibles con filtros"""
    if hasattr(request.user, 'rol') and request.user.rol.upper() != 'ESTUDIANTE' and not request.user.is_superuser:
        return redirect(_url_para_usuario(request.user))

    retos_qs = Reto.objects.filter(estado__in=['aprobado', 'en_curso']).select_related('empresa', 'facultad', 'programa')

    # Buscador por texto
    q = request.GET.get('q', '').strip()
    if q:
        retos_qs = retos_qs.filter(titulo__icontains=q)

    # Filtros
    facultad = request.GET.get('facultad', '')
    programa = request.GET.get('programa', '')
    tipo = request.GET.get('tipo', '')
    area = request.GET.get('area', '')
    nivel = request.GET.get('nivel', '')

    if facultad:
        retos_qs = retos_qs.filter(facultad_id=facultad)
    if programa:
        retos_qs = retos_qs.filter(programa_id=programa)
    if tipo:
        retos_qs = retos_qs.filter(tipo=tipo)
    if area:
        retos_qs = retos_qs.filter(area__icontains=area)
    if nivel:
        retos_qs = retos_qs.filter(nivel_academico__icontains=nivel)

    retos_qs = retos_qs.distinct().order_by('-creado_en')

    # Favoritos del estudiante (para marcar en las tarjetas)
    favoritos_ids = []
    if request.user.is_authenticated:
        favoritos_ids = list(
            request.user.favoritos.values_list('reto_id', flat=True)
        )

    paginator = Paginator(retos_qs, 12)
    page_obj = paginator.get_page(request.GET.get('page'))

    # Opciones para los selectores de filtro
    areas = Reto.objects.exclude(area='').values_list('area', flat=True).distinct().order_by('area')
    niveles = Reto.objects.exclude(nivel_academico='').values_list('nivel_academico', flat=True).distinct().order_by('nivel_academico')

    return render(request, 'estudiante/explorar_retos.html', {
        'retos': page_obj,
        'titulo': 'Explorar Retos Disponibles',
        'facultades': Facultad.objects.all().order_by('nombre'),
        'programas': Programa.objects.select_related('facultad').all().order_by('nombre'),
        'tipos': Reto.TIPO_CHOICES,
        'areas': areas,
        'niveles': niveles,
        'favoritos_ids': favoritos_ids,
        'filtros': {
            'q': q, 'facultad': facultad, 'programa': programa,
            'tipo': tipo, 'area': area, 'nivel': nivel,
        },
        'querystring': request.GET.urlencode(),
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

