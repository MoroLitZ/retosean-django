from django.http import HttpResponse
from django.shortcuts import get_object_or_404, redirect, render
from django.contrib.auth.decorators import login_required
from django.core.paginator import Paginator
from django.contrib import messages

from apps.retos.models import Reto
from apps.participaciones.models import Postulacion as PostulacionReto
from apps.academico.models import Certificado, Facultad, Programa
from apps.academico.services import generar_pdf_portafolio, obtener_pdf_certificado
from apps.usuarios.decorators import solo_estudiante
from apps.usuarios.roles import es_admin, url_dashboard_para


@login_required(login_url='usuarios:login')
def explorar_retos(request):
    """Vista para que estudiantes exploren retos disponibles con filtros"""
    # El admin tambien puede mirar el catalogo; el resto va a su propio panel.
    if not es_admin(request.user) and getattr(request.user, 'rol', None) != 'ESTUDIANTE':
        return redirect(url_dashboard_para(request.user))

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
        retos_qs = retos_qs.filter(area=area)
    if nivel:
        retos_qs = retos_qs.filter(nivel_academico=nivel)

    retos_qs = retos_qs.distinct().order_by('-creado_en')

    # Favoritos del estudiante (para marcar en las tarjetas)
    favoritos_ids = []
    if request.user.is_authenticated:
        favoritos_ids = list(
            request.user.favoritos.values_list('reto_id', flat=True)
        )

    paginator = Paginator(retos_qs, 12)
    page_obj = paginator.get_page(request.GET.get('page'))

    # Opciones para los selectores de filtro, tomadas del catalogo controlado del modelo.
    areas = Reto.AREA_CHOICES
    niveles = Reto.NIVEL_ACADEMICO_CHOICES

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


@solo_estudiante
def mis_postulaciones(request):
    """Ver todas las postulaciones del estudiante"""
    postulaciones_usuario = PostulacionReto.objects.filter(estudiante=request.user).select_related('reto')
    
    return render(request, 'estudiante/mis_postulaciones.html', {
        'postulaciones': postulaciones_usuario,
        'titulo': 'Mis Inscripciones a Retos'
    })


@solo_estudiante
def certificados(request):
    """Certificados obtenidos por el estudiante en retos finalizados (HU16)."""
    mis_certificados = (
        Certificado.objects
        .filter(usuario=request.user)
        .select_related('reto', 'reto__empresa')
    )
    return render(request, 'estudiante/certificados.html', {
        'titulo': 'Mis Certificados Obtenidos',
        'certificados': mis_certificados,
    })


@login_required(login_url='usuarios:login')
def descargar_certificado(request, codigo):
    """Descarga el PDF del certificado. Solo su titular o un administrador."""
    certificado = get_object_or_404(
        Certificado.objects.select_related('reto', 'reto__empresa', 'usuario'),
        codigo_verificacion=codigo,
    )
    if certificado.usuario_id != request.user.pk and not es_admin(request.user):
        messages.error(request, 'Ese certificado no te pertenece.')
        return redirect('academico:certificados')

    contenido = obtener_pdf_certificado(certificado)
    respuesta = HttpResponse(contenido, content_type='application/pdf')
    respuesta['Content-Disposition'] = (
        f'attachment; filename="certificado-{certificado.codigo_corto}.pdf"'
    )
    return respuesta


def verificar_certificado(request, codigo):
    """Verificacion publica de un certificado. No requiere iniciar sesion."""
    certificado = Certificado.objects.select_related(
        'reto', 'reto__empresa', 'usuario'
    ).filter(codigo_verificacion=codigo).first()
    return render(request, 'estudiante/verificar_certificado.html', {
        'titulo': 'Verificacion de Certificado',
        'certificado': certificado,
        'codigo': codigo,
    })


@solo_estudiante
def portafolio(request):
    """Exporta el portafolio del estudiante en PDF con formato institucional."""
    from apps.evaluacion.models import Entregable

    mis_certificados = list(
        Certificado.objects.filter(usuario=request.user)
        .select_related('reto', 'reto__empresa')
    )
    entregables = list(
        Entregable.objects.filter(estudiante=request.user)
        .select_related('reto').order_by('-fecha_entrega')
    )
    contenido = generar_pdf_portafolio(request.user, mis_certificados, entregables)
    respuesta = HttpResponse(contenido, content_type='application/pdf')
    nombre = (request.user.get_full_name() or request.user.username).replace(' ', '-').lower()
    respuesta['Content-Disposition'] = f'attachment; filename="portafolio-{nombre}.pdf"'
    return respuesta

