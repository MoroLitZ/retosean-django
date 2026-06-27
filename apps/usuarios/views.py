from django.shortcuts import render, redirect
from django.contrib.auth import login, logout
from django.contrib import messages
from .forms import LoginForm, RegistroEmpresaForm, RegistroAcademicoForm, CargarDocumentoForm
from django.contrib.auth.decorators import login_required
from .models import DocumentoEmpresa, Empresa


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
            return redirect(_url_para_usuario.get(user))
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





# ── Vistas stub (en construcción) ──────────────────────────────────

def _stub(titulo, icono, rol=None):
    def view(request):
        if rol == 'SUPERUSER' and not request.user.is_superuser:
            return redirect(_url_para_usuario(request.user))
        if rol and rol != 'SUPERUSER' and request.user.rol != rol:
            return redirect(_url_para_usuario(request.user))
        return render(request, 'usuarios/en_construccion.html', {
            'titulo': titulo, 'icono': icono
        })
    view.__name__ = titulo.lower().replace(' ', '_')
    return login_required(view, login_url='usuarios:login')


# Admin
lista_usuarios    = _stub('Usuarios',         'bi-people',           rol='SUPERUSER')
lista_empresas    = _stub('Empresas',         'bi-building',         rol='SUPERUSER')
lista_retos_admin = _stub('Retos',            'bi-trophy',           rol='SUPERUSER')
reportes_admin    = _stub('Reportes',         'bi-bar-chart-line',   rol='SUPERUSER')

# Empresa
publicar_reto         = _stub('Publicar Reto',     'bi-plus-circle',       rol='EMPRESA')
mis_retos_empresa     = _stub('Mis Retos',         'bi-trophy',            rol='EMPRESA')
postulaciones_empresa = _stub('Postulaciones',     'bi-person-check',      rol='EMPRESA')
indicadores_empresa   = _stub('Indicadores',       'bi-bar-chart',         rol='EMPRESA')

# Profesor
mis_cursos           = _stub('Mis Cursos',       'bi-journal-text', rol='PROFESOR')
retos_vinculados     = _stub('Retos Vinculados', 'bi-trophy',       rol='PROFESOR')
mis_estudiantes      = _stub('Mis Estudiantes',  'bi-people',       rol='PROFESOR')
evaluaciones         = _stub('Evaluaciones',     'bi-star-half',    rol='PROFESOR')
entregables_profesor = _stub('Entregables',      'bi-folder-check', rol='PROFESOR')

# Estudiante
explorar_retos    = _stub('Explorar Retos',    'bi-search',       rol='ESTUDIANTE')
mis_postulaciones = _stub('Mis Postulaciones', 'bi-send',         rol='ESTUDIANTE')
mis_entregables   = _stub('Mis Entregables',   'bi-folder2-open', rol='ESTUDIANTE')
certificados      = _stub('Certificados',      'bi-award',        rol='ESTUDIANTE')

