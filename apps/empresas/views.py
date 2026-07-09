from django.shortcuts import render, redirect, get_object_or_404
from django.contrib.auth.decorators import login_required
from django.contrib import messages

from .models import Empresa, DocumentoEmpresa
from .forms import CargarDocumentoForm
from apps.retos.models import Reto
from apps.participaciones.models import Postulacion as PostulacionReto
from apps.evaluacion.models import Entregable
from apps.usuarios.views import _url_para_usuario
from .services import puede_la_empresa_operar


@login_required(login_url='usuarios:login')
def panel_documentos_empresa(request):
    empresa = getattr(request.user, 'empresa_perfil', None)
    
    if not empresa and not request.user.is_staff:
        messages.error(request, "Acceso denegado.")
        return redirect('usuarios:login')

    if request.method == 'POST':
        form = CargarDocumentoForm(request.POST, request.FILES)

        if form.is_valid():
            documento = form.save(commit=False)
            documento.empresa = empresa
            documento.estado = 'CARGADO'
            documento.save()
            
            messages.success(request, "Documento cargado correctamente.")
            return redirect('empresas:documentos')
        else:
            messages.error(request, "Error al cargar el documento. Revisa los datos.")
    else:
        form = CargarDocumentoForm()

    documentos = DocumentoEmpresa.objects.filter(empresa=empresa) if empresa else []
    
    estados_config = {
        'CARGADO': ('En Revisión', 'bg-info text-dark'),
        'VERIFICADO': ('Aprobado', 'bg-success text-white'),
        'RECHAZADO': ('Rechazado', 'bg-danger text-white'),
        'PENDIENTE': ('Pendiente', 'bg-warning text-dark')
    }

    context = {
        'form': form,
        'documentos': [
            {
                'tipo': doc.get_tipo_documento_display(),
                'estado_texto': estados_config.get(doc.estado, (doc.estado, 'bg-secondary'))[0],
                'estado_clase': estados_config.get(doc.estado, (doc.estado, 'bg-secondary'))[1],
                'fecha_expedicion': doc.fecha_expedicion,
                'url_archivo': doc.archivo.url if doc.archivo else None,
                'motivo_rechazo': doc.motivo_rechazo
            }
            for doc in documentos
        ],
        'tiene_documentos': documentos.exists(),
        'perfil': empresa
    }
    
    return render(request, 'empresas/panel_documentos.html', context)


@login_required(login_url='usuarios:login')
def postulaciones_empresa(request):
    """Gestión de postulaciones y entregables recibidos por la empresa"""
    if request.user.rol != 'EMPRESA' or not hasattr(request.user, 'empresa_perfil'):
        messages.error(request, "Primero debes completar el perfil de tu empresa.")
        return redirect(_url_para_usuario(request.user))

    empresa = request.user.empresa_perfil
    retos_empresa = Reto.objects.filter(empresa=request.user)
    estado_filtro = request.GET.get('estado', 'PENDIENTE')
    estados_validos = {'PENDIENTE', 'ACEPTADA', 'RECHAZADA'}

    postulaciones = PostulacionReto.objects.filter(
        reto__in=retos_empresa
    ).select_related('reto', 'estudiante').order_by('-fecha_postulacion')

    if estado_filtro in estados_validos:
        postulaciones = postulaciones.filter(estado=estado_filtro)

    entregables_recibidos = Entregable.objects.filter(
        reto__in=retos_empresa
    ).select_related('reto', 'estudiante').order_by('-id')

    return render(request, 'empresas/postulaciones.html', {
        'titulo': 'Gestión de Postulaciones y Entregables',
        'postulaciones': postulaciones,
        'entregables': entregables_recibidos,
        'estado_filtro': estado_filtro,
        'estados': PostulacionReto.ESTADOS,
    })


@login_required(login_url='usuarios:login')
def gestionar_postulacion(request, postulacion_id):
    """Aceptar o rechazar una postulación de estudiante"""
    if request.user.rol != 'EMPRESA' or not hasattr(request.user, 'empresa_perfil'):
        return redirect(_url_para_usuario(request.user))

    # CORRECCIÓN: Filtrar por empresa_perfil
    postulacion = get_object_or_404(
        PostulacionReto,
        pk=postulacion_id,
        reto__empresa=request.user,
    )

    if request.method == 'POST':
        accion = request.POST.get('accion')
        if accion == 'aceptar':
            postulacion.estado = 'ACEPTADA'
            postulacion.save()
            messages.success(request, 'Postulación aceptada.')
        elif accion == 'rechazar':
            postulacion.estado = 'RECHAZADA'
            postulacion.save()
            messages.warning(request, 'Postulación rechazada.')

    return redirect('empresas:postulaciones')


@login_required(login_url='usuarios:login')
def indicadores_empresa(request):
    """Indicadores y estadísticas de gestión de la empresa"""
    if request.user.rol != 'EMPRESA' or not hasattr(request.user, 'empresa_perfil'):
        return redirect(_url_para_usuario(request.user))

    # CORRECCIÓN: Filtrar por empresa_perfil
    retos = Reto.objects.filter(empresa=request.user)
    context = {
        'titulo': 'Indicadores de Gestión',
        'total_retos': retos.count(),
        'retos_activos': retos.filter(estado__in=['aprobado', 'en_curso']).count(),
        'retos_finalizados': retos.filter(estado='finalizado').count(),
        'retos_en_revision': retos.filter(estado='en_revision').count(),
        'total_postulaciones': PostulacionReto.objects.filter(reto__in=retos).count(),
        'postulaciones_pendientes': PostulacionReto.objects.filter(reto__in=retos, estado='PENDIENTE').count(),
        'postulaciones_aceptadas': PostulacionReto.objects.filter(reto__in=retos, estado='ACEPTADA').count(),
        'total_entregables': Entregable.objects.filter(reto__in=retos).count(),
        'entregables_pendientes': Entregable.objects.filter(reto__in=retos, estado='ENVIADO').count(),
        'retos_recientes': retos.order_by('-id')[:5],
    }
    return render(request, 'empresas/indicadores.html', context)


@login_required(login_url='usuarios:login')
def publicar_reto(request):
    return redirect('retos:crear')


@login_required(login_url='usuarios:login')
def mis_retos_empresa(request):
    return redirect('retos:mis_retos')


@login_required
def admin_revisar_documentacion(request):
    if not request.user.is_superuser:
        messages.error(request, "Acceso denegado.")
        return redirect('usuarios:perfil')
    
    pendientes = DocumentoEmpresa.objects.filter(estado='CARGADO').exclude(archivo='')
    return render(request, 'empresas/admin/revisar_documentos.html', {'pendientes': pendientes})


@login_required
def procesar_aprobacion(request, documento_id):
    if not request.user.is_superuser:
        return redirect('usuarios:perfil')
        
    doc = get_object_or_404(DocumentoEmpresa, pk=documento_id)
    
    if request.method == 'POST':
        nuevo_estado = request.POST.get('nuevo_estado')
        doc.estado = nuevo_estado
        doc.motivo_rechazo = request.POST.get('motivo', '')
        doc.save()
        
        # Lógica de cierre HU01: Validar si al aprobar un documento, la empresa queda VERIFICADA
        if puede_la_empresa_operar(doc.empresa):
            doc.empresa.estado_validacion = 'VERIFICADA'
            doc.empresa.save()
        
        estado_str = "aprobado" if nuevo_estado == 'VERIFICADO' else "rechazado"
        messages.success(request, f"Documento {doc.get_tipo_documento_display()} {estado_str} exitosamente.")
        
    return redirect('empresas:admin_revisar_documentacion')


@login_required(login_url='usuarios:login')
def lista_empresas(request):
    """Gestión y listado de empresas aliadas"""
    if not request.user.is_superuser:
        return redirect(_url_para_usuario(request.user))
    
    empresas = Empresa.objects.all().order_by('razon_social')
    return render(request, 'usuarios/admin/lista_empresas.html', {
        'empresas': empresas,
        'titulo': 'Empresas Aliadas'
    })