from django.shortcuts import render, redirect
from django.contrib.auth import login, logout
from django.contrib import messages
from django.contrib.auth.decorators import login_required
from .forms import LoginForm, RegistroUsuarioForm, RegistroAcademicoForm, RegistroEmpresaForm, CargarDocumentoForm
from .models import DocumentoEmpresa, PerfilEmpresa


def _url_para_usuario(user):
    if user.is_superuser:
        return 'usuarios:admin_dashboard'
    return {
        'EMPRESA':    'usuarios:empresa_dashboard',
        'PROFESOR':   'usuarios:profesor_dashboard',
        'ESTUDIANTE': 'usuarios:estudiante_dashboard',
    }.get(user.rol, 'usuarios:perfil')


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


def vista_logout(request):
    logout(request)
    messages.success(request, 'Sesión cerrada correctamente.')
    return redirect('usuarios:login')


def vista_registro(request):
    tipo = request.GET.get('tipo')

    if tipo == 'academico':
        form = RegistroAcademicoForm(request.POST or None)
        if request.method == 'POST' and form.is_valid():
            user = form.save()
            login(request, user)
            messages.success(request, f'¡Bienvenido a RetosEAN, {user.first_name or user.username}!')
            return redirect(_url_para_usuario(user))
        return render(request, 'usuarios/form_academico.html', {'form': form})

    if tipo == 'empresa':
        form = RegistroEmpresaForm(request.POST or None)
        if request.method == 'POST' and form.is_valid():
            user = form.save()
            login(request, user)
            messages.success(request, '¡Organización registrada! Bienvenido a RetosEAN.')
            return redirect(_url_para_usuario(user))
        return render(request, 'usuarios/form_empresa.html', {'form': form})

    return render(request, 'usuarios/registro.html')


@login_required(login_url='usuarios:login')
def vista_perfil(request):
    usuario = request.user
    nombre_completo = usuario.get_full_name().strip() or 'Sin nombre registrado'
    context = {
        'nombre_completo': nombre_completo,
        'rol_usuario': usuario.get_rol_display() if not usuario.is_superuser else 'Administrador',
    }
    return render(request, 'usuarios/perfil.html', context)


# ── Dashboards por rol ──────────────────────────────────────────────────────

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


@login_required(login_url='usuarios:login')
def panel_documentos_empresa(request):
    if request.user.rol != 'EMPRESA' and not request.user.is_superuser:
        messages.error(request, "Acceso denegado.")
        return redirect(_url_para_usuario(request.user))

    try:
        perfil = request.user.perfil_empresa
    except PerfilEmpresa.DoesNotExist:
        messages.error(request, "Tu empresa aún no tiene perfil completo.")
        return redirect(_url_para_usuario(request.user))

    if request.method == 'POST':
        form = CargarDocumentoForm(request.POST, request.FILES)
        if form.is_valid():
            doc = form.save(commit=False)
            doc.perfil_empresa = perfil
            doc.estado = 'CARGADO'
            try:
                existente = DocumentoEmpresa.objects.get(perfil_empresa=perfil, tipo_documento=doc.tipo_documento)
                existente.archivo = doc.archivo
                existente.fecha_expedicion = doc.fecha_expedicion
                existente.estado = 'CARGADO'
                existente.motivo_rechazo = None
                existente.save()
            except DocumentoEmpresa.DoesNotExist:
                doc.save()
            messages.success(request, "Documento cargado correctamente.")
            return redirect('usuarios:documentos_empresa')
    else:
        form = CargarDocumentoForm()

    ESTADOS_MAP = {
        'PENDIENTE': ('Pendiente',   'bg-warning text-dark'),
        'CARGADO':   ('En Revisión', 'bg-info text-dark'),
        'VERIFICADO':('Aprobado',    'bg-success text-white'),
        'RECHAZADO': ('Rechazado',   'bg-danger text-white'),
    }

    documentos = []
    for doc in DocumentoEmpresa.objects.filter(perfil_empresa=perfil):
        texto, clase = ESTADOS_MAP.get(doc.estado, (doc.estado, 'bg-secondary'))
        documentos.append({
            'tipo':            doc.get_tipo_documento_display(),
            'estado_texto':    texto,
            'estado_clase':    clase,
            'motivo_rechazo':  doc.motivo_rechazo,
            'fecha_expedicion': doc.fecha_expedicion.strftime('%d/%m/%Y') if doc.fecha_expedicion else '—',
            'url_archivo':     doc.archivo.url if doc.archivo else None,
        })

    return render(request, 'usuarios/panel_documentos.html', {
        'form':            form,
        'documentos':      documentos,
        'tiene_documentos': bool(documentos),
        'perfil':          perfil,
    })


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
