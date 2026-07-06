from django.shortcuts import render, redirect, get_object_or_404
from django.contrib.auth import login, logout, get_user_model
from django.contrib import messages
from .forms import LoginForm, RegistroEmpresaForm, RegistroAcademicoForm, CargarDocumentoForm, EntregableForm
from django.contrib.auth.decorators import login_required
from .models import Usuario
from apps.empresas.models import Empresa, DocumentoEmpresa
from apps.participaciones.models import Postulacion as PostulacionReto
from apps.evaluacion.models import Entregable
from apps.retos.models import Reto
from apps.seguimiento.models import IntegracionAcademica
from apps.retos.views import rol_requerido


def _url_para_usuario(user):
    if user.is_superuser:
        return 'usuarios:admin_dashboard'
    return {
        'EMPRESA':    'usuarios:empresa_dashboard',
        'PROFESOR':   'usuarios:profesor_dashboard',
        'ESTUDIANTE': 'usuarios:estudiante_dashboard',
    }.get(user.rol, 'usuarios:perfil')

# se manda al user a su perfil si ya tiene registro en la app
def vista_login(request):
    if request.user.is_authenticated:
        return redirect(_url_para_usuario(request.user))

    form = LoginForm(request, data=request.POST or None)
    if request.method == 'POST':
        if form.is_valid():
            user = form.get_user()
            login(request, user)
            return redirect(_url_para_usuario(user))
        else:
            messages.error(request, 'Usuario o contraseña incorrectos.')
    return render(request, 'usuarios/login.html', {'form': form})


# cerramos la sesión del usuario si lo requiere
def vista_logout(request):
    logout(request)
    messages.success(request, 'Sesión cerrada correctamente.')
    return redirect('usuarios:login')

# zona en la cual los usuarios nuevos podrán registrarse y colocar su rol
def vista_registro(request):
    tipo_registro = request.GET.get('tipo')

    if tipo_registro == 'academico':
        form = RegistroAcademicoForm(request.POST or None)
        if request.method == 'POST' and form.is_valid():
            user = form.save()
            login(request, user)
            messages.success(request, f'¡Bienvenido a RetosEAN, {user.first_name or user.username}!')
            return redirect(_url_para_usuario(user))
        return render(request, 'usuarios/form_academico.html', {'form': form})

    elif tipo_registro == 'empresa':
        form = RegistroEmpresaForm(request.POST or None)
        if request.method == 'POST' and form.is_valid():
            user = form.save()
            login(request, user)
            messages.success(request, '¡Organización registrada! Bienvenido a RetosEAN.')
            return redirect(_url_para_usuario(user))
        return render(request, 'usuarios/form_empresa.html', {'form': form})    
    
    return render(request, 'usuarios/registro.html')


@login_required(login_url='usuarios:login') # Se protege la vista para que únicamente los usuarios logeados puedan entrar
# la vista donde el usuario va a ir directamente despues del login o el registro
def vista_perfil(request):
    usuario = request.user
    nombre_completo = usuario.get_full_name().strip() or 'Sin nombre registrado'
    context = {
        'nombre_completo': nombre_completo,
        'rol_usuario': usuario.get_rol_display().capitalize() if not usuario.is_superuser else 'Administrador',
    }

    return render(request, 'usuarios/perfil.html', context)

@login_required
def panel_documentos_empresa(request):

    # verificamos que el usuario que entra sea una empresa
    if (request.user.rol != 'EMPRESA' or not request.user.empresa) and not request.user.is_staff:
        messages.error(request, "Acceso denegado. Esta sección es exclusiva para empresas con perfil completo.")
        return redirect(_url_para_usuario(request.user))

    # asignamos la empresa
    try:
        empresa = request.user.empresa
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
            #return redirect('/usuarios/empresa/documentos/')
            return redirect('usuarios:documentos_empresa')
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
        'perfil': empresa, # si no funciona, quitamos la linea
    }
    return render(request, 'usuarios/panel_documentos.html', context)


@rol_requerido('ESTUDIANTE')
def postular_a_reto(request, reto_id):
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
    postulaciones_aceptadas = PostulacionReto.objects.filter(
        estudiante=request.user,
        estado='ACEPTADA'
    ).select_related('reto')

    return render(request, 'usuarios/mis_entregables.html', {
        'postulaciones': postulaciones_aceptadas
    })


@login_required(login_url='usuarios:login')
def dashboard_admin(request):
    if not request.user.is_superuser:
        return redirect(_url_para_usuario(request.user))
    return render(request, 'usuarios/dashboard_admin.html')


@login_required(login_url='usuarios:login')
def dashboard_empresa(request):
    if request.user.rol != 'EMPRESA':
        return redirect(_url_para_usuario(request.user))
    return render(request, 'usuarios/dashboard_empresa.html')


@login_required(login_url='usuarios:login')
def dashboard_profesor(request):
    if request.user.rol != 'PROFESOR':
        return redirect(_url_para_usuario(request.user))
    return render(request, 'usuarios/dashboard_profesor.html')


@login_required(login_url='usuarios:login')
def dashboard_estudiante(request):
    if request.user.rol != 'ESTUDIANTE':
        return redirect(_url_para_usuario(request.user))
    return render(request, 'usuarios/dashboard_estudiante.html')



## Se can añadiendo funciones para que los accesos de la sidebar vayan tomando forma

# admin
@login_required(login_url='usuarios:login')
def lista_usuarios(request):
    if not request.user.is_superuser:
        return redirect(_url_para_usuario(request.user))
    
    # Consulta real a la tabla de usuarios de Postgres
    usuarios = Usuario.objects.all().order_by('-date_joined')
    return render(request, 'usuarios/admin/lista_usuarios.html', {
        'usuarios': usuarios,
        'titulo': 'Gestión de Usuarios'
    })


@login_required(login_url='usuarios:login')
def lista_empresas(request):
    if not request.user.is_superuser:
        return redirect(_url_para_usuario(request.user))
    
    empresas = Empresa.objects.all().order_by('razon_social')
    return render(request, 'usuarios/admin/lista_empresas.html', {
        'empresas': empresas,
        'titulo': 'Empresas Aliadas'
    })
    

@login_required(login_url='usuarios:login')
def reportes_admin(request):
    if not request.user.is_superuser:
        return redirect(_url_para_usuario(request.user))
        
    context = {
        'titulo': 'Reportes y Estadísticas',
        'total_usuarios': Usuario.objects.count(),
        'total_empresas': Empresa.objects.count(),
        'total_retos': Reto.objects.count() if 'Reto' in globals() else 0,
    }
    return render(request, 'usuarios/admin/reportes.html', context)


# estudiantes
@login_required(login_url='usuarios:login')
def explorar_retos(request):
    if hasattr(request.user, 'rol') and request.user.rol.upper() != 'ESTUDIANTE' and not request.user.is_superuser:
        return redirect(_url_para_usuario(request.user))
        
    # Cambiamos '-fecha_creacion' por '-creado_en'
    retos_disponibles = Reto.objects.all().order_by('-creado_en') if 'Reto' in globals() else []
    return render(request, 'usuarios/estudiante/explorar_retos.html', {
        'retos': retos_disponibles, 'titulo': 'Explorar Retos Disponibles'
    })


@login_required(login_url='usuarios:login')
def mis_postulaciones(request):
    if request.user.rol != 'ESTUDIANTE':
        return redirect(_url_para_usuario(request.user))
        
    postulaciones_usuario = PostulacionReto.objects.filter(estudiante=request.user).select_related('reto')
    
    return render(request, 'usuarios/estudiante/mis_postulaciones.html', {
        'postulaciones': postulaciones_usuario,
        'titulo': 'Mis Inscripciones a Retos'
    })


@login_required(login_url='usuarios:login')
def mis_entregables(request, reto_id=None):
    if request.user.rol != 'ESTUDIANTE':
        return redirect(_url_para_usuario(request.user))
    
    if reto_id:
        reto = get_object_or_404(Reto, pk=reto_id, estado='aprobado')
        tiene_acceso = PostulacionReto.objects.filter(reto=reto, estudiante=request.user).exists()
        
        if not tiene_acceso:
            messages.error(request, 'No puedes gestionar entregables si no te has postulado a este reto.')
            return redirect('retos:detalle', pk=reto.id)
            
        entregables_subidos = Entregable.objects.filter(reto=reto, estudiante=request.user).order_by('-fecha_entrega') # o el campo de fecha que tengas
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
            return redirect('usuarios:mis_entregables', reto_id=reto.id)
    else:
        form = EntregableForm() if reto else None
        
    todas_mis_postulaciones = PostulacionReto.objects.filter(estudiante=request.user).select_related('reto')
        
    return render(request, 'usuarios/estudiante/mis_entregables.html', {
        'titulo': 'Mis Entregables de Proyecto',
        'reto': reto,
        'form': form,
        'entregables_subidos': entregables_subidos,
        'postulaciones': todas_mis_postulaciones
    })


@login_required(login_url='usuarios:login')
def certificados(request):
    if request.user.rol != 'ESTUDIANTE':
        return redirect(_url_para_usuario(request.user))
        
    return render(request, 'usuarios/estudiante/certificados.html', {
        'titulo': 'Mis Certificados Obtenidos'
    })
    

# empresas
@login_required(login_url='usuarios:login')
def postulaciones_empresa(request):
    if request.user.rol != 'EMPRESA':
        return redirect(_url_para_usuario(request.user))

    try:
        request.user.empresa
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

    return render(request, 'usuarios/empresa/postulaciones.html', {
        'titulo': 'Gestión de Postulaciones y Entregables',
        'postulaciones': postulaciones,
        'entregables': entregables_recibidos,
        'estado_filtro': estado_filtro,
        'estados': PostulacionReto.ESTADOS_POSTULACION,
    })


@login_required(login_url='usuarios:login')
def gestionar_postulacion(request, postulacion_id):
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

    return redirect('usuarios:postulaciones_empresa')


# profesores
@login_required(login_url='usuarios:login')
def entregables_profesor(request):
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

    return render(request, 'usuarios/profesor/entregables.html', {
        'titulo': 'Panel de Control Académico',
        'entregables': entregables_academia,
        'retos': todos_los_retos
    })
# ── Vistas stub (en construcción) ──────────────────────────────────

# Empresa — stubs que apuntan a vistas reales
@login_required(login_url='usuarios:login')
def publicar_reto(request):
    return redirect('retos:crear')


@login_required(login_url='usuarios:login')
def mis_retos_empresa(request):
    return redirect('retos:mis_retos')


@login_required(login_url='usuarios:login')
def indicadores_empresa(request):
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
    return render(request, 'usuarios/empresa/indicadores.html', context)


# Profesor
@login_required(login_url='usuarios:login')
def mis_cursos(request):
    if request.user.rol != 'PROFESOR':
        return redirect(_url_para_usuario(request.user))

    integraciones = IntegracionAcademica.objects.filter(
        profesor=request.user
    ).select_related('reto').order_by('-creado_en')

    return render(request, 'usuarios/profesor/mis_cursos.html', {
        'titulo': 'Mis Cursos y Retos Vinculados',
        'integraciones': integraciones,
    })


@login_required(login_url='usuarios:login')
def retos_vinculados(request):
    return redirect('retos:mis_integraciones')


@login_required(login_url='usuarios:login')
def mis_estudiantes(request):
    if request.user.rol != 'PROFESOR':
        return redirect(_url_para_usuario(request.user))

    retos_integrados = IntegracionAcademica.objects.filter(
        profesor=request.user
    ).values_list('reto_id', flat=True)

    postulaciones_aceptadas = PostulacionReto.objects.filter(
        reto_id__in=retos_integrados,
        estado='ACEPTADA',
    ).select_related('estudiante', 'reto').order_by('reto', 'estudiante__last_name')

    return render(request, 'usuarios/profesor/mis_estudiantes.html', {
        'titulo': 'Mis Estudiantes',
        'postulaciones': postulaciones_aceptadas,
    })


@login_required(login_url='usuarios:login')
def evaluaciones(request):
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
        return redirect('usuarios:evaluaciones')

    return render(request, 'usuarios/profesor/evaluaciones.html', {
        'titulo': 'Evaluaciones y Calificaciones',
        'entregables': entregables,
    })

