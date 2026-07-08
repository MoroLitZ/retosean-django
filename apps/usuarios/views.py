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
    return render(request, 'usuarios/dashboard_admin.html')


@login_required(login_url='usuarios:login')
def dashboard_empresa(request):
    if request.user.rol != 'EMPRESA':
        return redirect(_url_para_usuario(request.user))
    return render(request, 'empresas/dashboard.html')


@login_required(login_url='usuarios:login')
def dashboard_profesor(request):
    if request.user.rol != 'PROFESOR':
        return redirect(_url_para_usuario(request.user))
    return render(request, 'profesor/dashboard.html')


@login_required(login_url='usuarios:login')
def dashboard_estudiante(request):
    if request.user.rol != 'ESTUDIANTE':
        return redirect(_url_para_usuario(request.user))
    return render(request, 'estudiante/dashboard.html')




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
def lista_empresas(request):
    """Gestión y listado de empresas aliadas"""
    if not request.user.is_superuser:
        return redirect(_url_para_usuario(request.user))
    
    empresas = Empresa.objects.all().order_by('razon_social')
    return render(request, 'usuarios/admin/lista_empresas.html', {
        'empresas': empresas,
        'titulo': 'Empresas Aliadas'
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

