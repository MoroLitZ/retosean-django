from django.shortcuts import render, redirect, get_object_or_404
from django.contrib.auth import login, logout, get_user_model
from django.contrib import messages
from .forms import LoginForm, RegistroEmpresaForm, RegistroAcademicoForm
from django.contrib.auth.decorators import login_required
from .models import Usuario
from apps.empresas.models import Empresa
from apps.retos.models import Reto


# ── Rutas de autenticación y perfil ──────────────────────────────

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

    if request.method == 'POST':
        # Capturamos el identificador ingresado (suele llamarse 'username' o 'email' en el form)
        username_ingresado = request.POST.get('username') or request.POST.get('email')
        
        # Verificamos si el usuario existe y está inactivo
        if username_ingresado:
            usuario_db = Usuario.objects.filter(username=username_ingresado).first() or Usuario.objects.filter(email=username_ingresado).first()
            if usuario_db and not usuario_db.is_active:
                messages.error(request, 'Tu cuenta se encuentra inhabilitada debido a que la empresa aparece en listas restrictivas (Clinton/OFAC).')
                form = LoginForm(request, data=request.POST)
                return render(request, 'usuarios/login.html', {'form': form})

        # Flujo normal si la cuenta está activa o los datos son incorrectos
        form = LoginForm(request, data=request.POST)
        if form.is_valid():
            user = form.get_user()
            login(request, user)
            return redirect(_url_para_usuario(user))
        else:
            messages.error(request, 'Usuario o contraseña incorrectos.')
    else:
        form = LoginForm()

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
        return render(request, 'estudiante/form_academico.html', {'form': form})

    elif tipo_registro == 'empresa':
        form = RegistroEmpresaForm(request.POST or None)
        if request.method == 'POST' and form.is_valid():
            user = form.save()
            login(request, user)
            messages.success(request, '¡Organización registrada! Bienvenido a RetosEAN.')
            return redirect(_url_para_usuario(user))
        return render(request, 'empresas/form_empresa.html', {'form': form})    
    
    return render(request, 'usuarios/registro.html')


@login_required(login_url='usuarios:login')
def vista_perfil(request):
    usuario = request.user
    nombre_completo = usuario.get_full_name().strip() or 'Sin nombre registrado'
    context = {
        'nombre_completo': nombre_completo,
        'rol_usuario': usuario.get_rol_display().capitalize() if not usuario.is_superuser else 'Administrador',
    }

    return render(request, 'usuarios/perfil.html', context)




@login_required(login_url='usuarios:login')
def dashboard_admin(request):
    if not request.user.is_superuser:
        return redirect(_url_para_usuario(request.user))
    from apps.empresas.models import Empresa
    from apps.retos.models import Reto
    total_usuarios = Usuario.objects.count()
    total_empresas = Empresa.objects.count()
    total_retos = Reto.objects.count()
    total_estudiantes = Usuario.objects.filter(rol='ESTUDIANTE').count()
    return render(request, 'usuarios/dashboard_admin.html', {
        'total_usuarios': total_usuarios,
        'total_empresas': total_empresas,
        'total_retos': total_retos,
        'total_estudiantes': total_estudiantes,
    })


@login_required(login_url='usuarios:login')
def dashboard_empresa(request):
    if request.user.rol != 'EMPRESA':
        return redirect(_url_para_usuario(request.user))
    from apps.retos.models import Reto
    from apps.seguimiento.models import IntegracionAcademica
    retos = Reto.objects.filter(empresa=request.user)
    mis_retos = retos.count()
    postulaciones = IntegracionAcademica.objects.filter(reto__empresa=request.user).count()
    en_evaluacion = IntegracionAcademica.objects.filter(reto__empresa=request.user, estado='en_revision').count()
    retos_cerrados = retos.filter(estado='finalizado').count()
    return render(request, 'empresas/dashboard.html', {
        'total_retos': mis_retos,
        'total_postulaciones': postulaciones,
        'en_evaluacion': en_evaluacion,
        'retos_cerrados': retos_cerrados,
    })


@login_required(login_url='usuarios:login')
def dashboard_profesor(request):
    if request.user.rol != 'PROFESOR':
        return redirect(_url_para_usuario(request.user))
    from apps.seguimiento.models import IntegracionAcademica
    integraciones = IntegracionAcademica.objects.filter(profesor=request.user)
    cursos_activos = integraciones.filter(estado='aprobada').count()
    retos_asignados = integraciones.count()
    estudiantes = 0  # TODO: conectar con modelo de estudiantes por profesor
    evaluaciones_pendientes = integraciones.filter(estado='aprobada').count()
    return render(request, 'profesor/dashboard.html', {
        'cursos_activos': cursos_activos,
        'retos_asignados': retos_asignados,
        'total_estudiantes': estudiantes,
        'evaluaciones_pendientes': evaluaciones_pendientes,
    })


@login_required(login_url='usuarios:login')
def dashboard_estudiante(request):
    if request.user.rol != 'ESTUDIANTE':
        return redirect(_url_para_usuario(request.user))
    from apps.retos.models import Reto
    from apps.participaciones.models import Postulacion
    from apps.evaluacion.models import Entregable
    retos_disponibles = Reto.objects.filter(estado__in=['aprobado', 'en_curso']).count()
    mis_postulaciones = Postulacion.objects.filter(estudiante=request.user).count()
    entregables_pendientes = Entregable.objects.filter(
        estudiante=request.user, estado__in=['ENVIADO', 'EN_REVISION']
    ).count()
    certificados = 0
    retos_finalizados = Reto.objects.filter(estado='finalizado')
    if retos_finalizados.exists():
        certificados = Postulacion.objects.filter(
            estudiante=request.user, estado='ACEPTADA', reto__in=retos_finalizados
        ).count()
    return render(request, 'estudiante/dashboard.html', {
        'retos_disponibles': retos_disponibles,
        'mis_postulaciones': mis_postulaciones,
        'entregables_pendientes': entregables_pendientes,
        'certificados': certificados,
    })




# ── Vistas de Administración ──────────────────────────────────

@login_required(login_url='usuarios:login')
def lista_usuarios(request):
    """Gestión y listado de todos los usuarios del sistema"""
    if not request.user.is_superuser:
        return redirect(_url_para_usuario(request.user))
    
    # Consulta real a la tabla de usuarios de Postgres
    usuarios = Usuario.objects.all().order_by('-date_joined')
    return render(request, 'usuarios/admin/lista_usuarios.html', {
        'usuarios': usuarios,
        'titulo': 'Gestión de Usuarios'
    })

    

@login_required(login_url='usuarios:login')
def reportes_admin(request):
    """Reportes y estadísticas generales del sistema"""
    if not request.user.is_superuser:
        return redirect(_url_para_usuario(request.user))
        
    context = {
        'titulo': 'Reportes y Estadísticas',
        'total_usuarios': Usuario.objects.count(),
        'total_empresas': Empresa.objects.count(),
        'total_retos': Reto.objects.count(),
    }
    return render(request, 'usuarios/admin/reportes.html', context)

