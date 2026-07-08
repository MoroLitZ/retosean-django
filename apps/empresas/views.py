from django.shortcuts import render, redirect, get_object_or_404
from django.contrib.auth.decorators import login_required
from django.contrib import messages

from .models import Empresa, DocumentoEmpresa
from apps.usuarios.forms import CargarDocumentoForm
from apps.retos.models import Reto
from apps.participaciones.models import Postulacion as PostulacionReto
from apps.evaluacion.models import Entregable
from apps.usuarios.views import _url_para_usuario


@login_required(login_url='usuarios:login')
def panel_documentos_empresa(request):
    """Panel de carga y gestión de documentos de la empresa"""
    # verificamos que el usuario que entra sea una empresa
    if (request.user.rol != 'EMPRESA' or not request.user.empresa_perfil) and not request.user.is_staff:
        messages.error(request, "Acceso denegado. Esta sección es exclusiva para empresas con perfil completo.")
        return redirect(_url_para_usuario(request.user))

    # asignamos la empresa
    try:
        empresa = request.user.empresa_perfil
    except Empresa.DoesNotExist:
        messages.error(request, "Tu empresa aún no tiene perfil completo.")
        return redirect(_url_para_usuario(request.user))
    
    if request.method == 'POST':
        form = CargarDocumentoForm(request.POST, request.FILES)
        if form.is_valid():
            documento = form.save(commit=False)
            documento.empresa = empresa
            documento.estado = 'CARGADO'
            
            try:
                # si el documento ya existía, se actualiza en lugar de duplicarlo
                doc_existente = DocumentoEmpresa.objects.get(empresa=empresa, tipo_documento=documento.tipo_documento)
                doc_existente.archivo = documento.archivo
                doc_existente.fecha_expedicion = documento.fecha_expedicion
                doc_existente.estado = 'CARGADO'
                doc_existente.motivo_rechazo = None
                doc_existente.save()
            except DocumentoEmpresa.DoesNotExist:
                # si el documento no existía aún, se crea el registro desde cero
                documento.save()
                
            messages.success(request, f"El documento {form.get_tipo_documento_display if hasattr(form, 'get_tipo_documento_display') else form.cleaned_data['tipo_documento']} se cargó correctamente.")
            return redirect('empresas:documentos')
    else:
        form = CargarDocumentoForm()
    
    # consultamos el estado actual de todos sus documentos para ponerlos en la tabla
    documentos_raw = DocumentoEmpresa.objects.filter(empresa=empresa) if empresa else []
    documentos_procesados = []

    ESTADOS_MAP = {
        'PENDIENTE': {'texto': 'Pendiente', 'clase': 'bg-warning text-dark'},
        'CARGADO': {'texto': 'En Revisión', 'clase': 'bg-info text-dark'},
        'VERIFICADO': {'texto': 'Aprobado', 'clase': 'bg-success text-white'},
        'RECHAZADO': {'texto': 'Rechazado', 'clase': 'bg-danger text-white'},
    }

    for doc in documentos_raw:
        info_estado = ESTADOS_MAP.get(doc.estado, {'texto': doc.estado, 'clase': 'bg-secondary'})
        
        documentos_procesados.append({
            'tipo': doc.get_tipo_documento_display(),
            'estado_texto': info_estado['texto'],
            'estado_clase': info_estado['clase'],
            'motivo_rechazo': doc.motivo_rechazo,
            'fecha_expedicion': doc.fecha_expedicion.strftime('%d/%m/%Y') if doc.fecha_expedicion else '--',
            'url_archivo': doc.archivo.url if doc.archivo else None
        })

    context = {
        'form': form,
        'documentos': documentos_procesados,
        'tiene_documentos': len(documentos_procesados) > 0,
        'perfil': empresa,
    }
    return render(request, 'empresas/panel_documentos.html', context)


@login_required(login_url='usuarios:login')
def postulaciones_empresa(request):
    """Gestión de postulaciones y entregables recibidos por la empresa"""
    if request.user.rol != 'EMPRESA':
        return redirect(_url_para_usuario(request.user))

    try:
        request.user.empresa_perfil
    except Empresa.DoesNotExist:
        messages.error(request, "Primero debes completar el perfil de tu empresa.")
        return redirect('usuarios:empresa_dashboard')

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
    if request.user.rol != 'EMPRESA':
        return redirect(_url_para_usuario(request.user))

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
            messages.success(request, f'Postulación de {postulacion.estudiante.get_full_name() or postulacion.estudiante.username} aceptada.')
        elif accion == 'rechazar':
            postulacion.estado = 'RECHAZADA'
            postulacion.save()
            messages.warning(request, f'Postulación de {postulacion.estudiante.get_full_name() or postulacion.estudiante.username} rechazada.')

    return redirect('empresas:postulaciones')


@login_required(login_url='usuarios:login')
def indicadores_empresa(request):
    """Indicadores y estadísticas de gestión de la empresa"""
    if request.user.rol != 'EMPRESA':
        return redirect(_url_para_usuario(request.user))

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
        'retos_recientes': retos.order_by('-creado_en')[:5],
    }
    return render(request, 'empresas/indicadores.html', context)


@login_required(login_url='usuarios:login')
def publicar_reto(request):
    """Redirección a crear reto en retos app"""
    return redirect('retos:crear')


@login_required(login_url='usuarios:login')
def mis_retos_empresa(request):
    """Redirección a mis retos en retos app"""
    return redirect('retos:mis_retos')


@login_required
def admin_revisar_documentacion(request):
    """
    Vista donde el Admin ve todos los documentos en estado 'CARGADO'
    """
    if not request.user.is_superuser:
        messages.error(request, "Acceso denegado.")
        return redirect('usuarios:perfil')
    
    pendientes = DocumentoEmpresa.objects.filter(estado='CARGADO')
    return render(request, 'empresas/admin/revisar_documentos.html', {'pendientes': pendientes})


@login_required
def procesar_aprobacion(request, documento_id):
    """
    Vista procesa el formulario de aprobación/rechazo enviado por el Admin
    """
    if not request.user.is_superuser:
        return redirect('usuarios:perfil')
        
    doc = get_object_or_404(DocumentoEmpresa, pk=documento_id)
    
    if request.method == 'POST':
        nuevo_estado = request.POST.get('nuevo_estado')
        doc.estado = nuevo_estado
        doc.motivo_rechazo = request.POST.get('motivo', '')
        doc.save()
        
        estado_str = "aprobado" if nuevo_estado == 'VERIFICADO' else "rechazado"
        messages.success(request, f"Documento {doc.get_tipo_documento_display()} {estado_str} exitosamente.")
        
    return redirect('empresas:admin_revisar_documentacion')
