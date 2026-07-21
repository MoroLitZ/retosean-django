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
            tipo = form.cleaned_data['tipo_documento']
            archivo = form.cleaned_data['archivo']
            fecha_expedicion = form.cleaned_data.get('fecha_expedicion')

            documento, created = DocumentoEmpresa.objects.update_or_create(
                empresa=empresa,
                tipo_documento=tipo,
                defaults={
                    'archivo': archivo,
                    'estado': 'CARGADO',
                    'motivo_rechazo': '',
                    'fecha_expedicion': fecha_expedicion
                }
            )
            
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
def admin_revisar_documentacion(request, empresa_id):
    empresa = get_object_or_404(Empresa, id=empresa_id)
    pendientes = DocumentoEmpresa.objects.filter(empresa=empresa).exclude(estado__in=['VERIFICADO', 'RECHAZADO'])

    return render(request, 'empresas/admin/revisar_documentos.html', {
        'empresa': empresa,
        'pendientes': pendientes
    })


@login_required
def procesar_aprobacion(request, documento_id):
    if request.method == 'POST':
        documento = get_object_or_404(DocumentoEmpresa, id=documento_id)
        
        nuevo_estado = request.POST.get('nuevo_estado')
        motivo = request.POST.get('motivo', '')
        
        if nuevo_estado == 'VERIFICADO':
            documento.estado = 'VERIFICADO'
            documento.motivo_rechazo = ''
        elif nuevo_estado == 'RECHAZADO':
            documento.estado = 'RECHAZADO'
            documento.motivo_rechazo = motivo
        
        documento.save()
        messages.success(request, f"Documento {documento.get_tipo_documento_display()} procesado correctamente.")
        
        return redirect('empresas:admin_revisar_documentacion', empresa_id=documento.empresa.id)
    
    return redirect('empresas:lista_empresas')


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
    
@login_required
def procesar_listas_restrictivas(request, empresa_id):
    """Permite al administrador registrar el resultado de listas restrictivas (Clinton/OFAC) y subir evidencia"""
    if not request.user.is_staff:
        messages.error(request, "Acceso denegado.")
        return redirect('usuarios:login')

    empresa = get_object_or_404(Empresa, id=empresa_id)

    if request.method == 'POST':
        estado_listas = request.POST.get('estado_listas_restrictivas')
        evidencia = request.FILES.get('evidencia_listas')

        if estado_listas in ['APROBADO', 'RECHAZADO', 'PENDIENTE']:
            empresa.estado_listas_restrictivas = estado_listas
            
            # Si se adjunta una nueva evidencia, la guardamos
            if evidencia:
                empresa.evidencia_listas = evidencia
            
            # REGLA DE NEGOCIO: Si aparece en listas, rechazo global y bloqueo de cuenta
            if estado_listas == 'RECHAZADO':
                empresa.estado_validacion = 'RECHAZADA'
                
                # Rechazar automáticamente todos los documentos pendientes
                DocumentoEmpresa.objects.filter(empresa=empresa).exclude(estado='VERIFICADO').update(
                    estado='RECHAZADO',
                    motivo_rechazo='Rechazado automáticamente por aparecer en listas restrictivas (Clinton/OFAC).'
                )

                # Inhabilitar la cuenta del usuario para que no pueda volver a iniciar sesión
                if empresa.usuario:
                    empresa.usuario.is_active = False
                    empresa.usuario.save()
                
                messages.warning(request, f"La empresa {empresa.razon_social} fue rechazada por listas restrictivas y su cuenta ha sido inhabilitada.")
            elif estado_listas == 'APROBADO':
                messages.success(request, f"Verificación de listas restrictivas aprobada para {empresa.razon_social}.")
                # Si por error estuvo rechazada antes y ahora se aprueba, podríamos reactivar el usuario opcionalmente:
                if empresa.usuario and not empresa.usuario.is_active:
                    empresa.usuario.is_active = True
                    empresa.usuario.save()

            empresa.save()
        else:
            messages.error(request, "Estado de listas no válido.")

    return redirect('empresas:admin_revisar_documentacion', empresa_id=empresa.id)