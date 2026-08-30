from django.shortcuts import render, redirect, get_object_or_404
from django.contrib.auth import login, logout, get_user_model
from django.contrib import messages
from django.utils.http import url_has_allowed_host_and_scheme
from .forms import LoginForm, RegistroEmpresaForm, RegistroAcademicoForm, EditarPerfilForm
from django.contrib.auth.decorators import login_required
from django.core.paginator import Paginator
from django.db.models import Q
from django.http import HttpResponse
from django.views.decorators.http import require_POST
from .importacion import COLUMNAS, COLUMNAS_OBLIGATORIAS, importar, leer_archivo, plantilla_csv
from .models import LogActividad, Usuario
from apps.empresas.models import Empresa
from apps.retos.models import Reto
from .decorators import solo_admin, solo_empresa, solo_estudiante, solo_profesor
from .roles import es_admin, url_dashboard_para


# ── Rutas de autenticación y perfil ──────────────────────────────

def _url_para_usuario(user):
    """Alias historico: otras apps lo importan. La logica vive en roles.py."""
    return url_dashboard_para(user)


def _redirigir_despues_login(request, user):
    """Redirige a `?next=` solo si es una URL segura del mismo host."""
    next_url = request.POST.get('next') or request.GET.get('next') or ''
    if next_url and url_has_allowed_host_and_scheme(next_url, allowed_hosts={request.get_host()}):
        return redirect(next_url)
    return redirect(_url_para_usuario(user))


def _mensaje_cuenta_inactiva(usuario):
    """Texto para una cuenta desactivada.

    Solo se acusa de listas restrictivas (Clinton/OFAC) cuando realmente es una
    empresa que aparece en ellas; el resto de cuentas desactivadas por el admin
    reciben un mensaje generico.
    """
    empresa = getattr(usuario, 'empresa_perfil', None)
    if usuario.rol == 'EMPRESA' and empresa is not None and empresa.estado_listas_restrictivas == 'RECHAZADO':
        return 'Tu cuenta se encuentra inhabilitada debido a que la empresa aparece en listas restrictivas (Clinton/OFAC).'
    return 'Tu cuenta se encuentra inhabilitada. Contacta con el administrador de la plataforma.'
    
def vista_login(request):
    next_url = request.POST.get('next') or request.GET.get('next') or ''

    if request.user.is_authenticated:
        return _redirigir_despues_login(request, request.user)

    if request.method == 'POST':
        # Capturamos el identificador ingresado (suele llamarse 'username' o 'email' en el form)
        username_ingresado = request.POST.get('username') or request.POST.get('email')
        
        # Verificamos si el usuario existe y está inactivo
        if username_ingresado:
            usuario_db = Usuario.objects.filter(username=username_ingresado).first() or Usuario.objects.filter(email=username_ingresado).first()
            if usuario_db and not usuario_db.is_active:
                messages.error(request, _mensaje_cuenta_inactiva(usuario_db))
                form = LoginForm(request, data=request.POST)
                return render(request, 'usuarios/login.html', {'form': form, 'next': next_url})

        # Flujo normal si la cuenta está activa o los datos son incorrectos
        form = LoginForm(request, data=request.POST)
        if form.is_valid():
            user = form.get_user()
            login(request, user)
            return _redirigir_despues_login(request, user)
        else:
            messages.error(request, 'Usuario o contraseña incorrectos.')
    else:
        form = LoginForm()

    return render(request, 'usuarios/login.html', {'form': form, 'next': next_url})


# cerramos la sesión del usuario si lo requiere
@require_POST
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
        'rol_usuario': 'Administrador' if es_admin(usuario) else usuario.get_rol_display().capitalize(),
    }

    return render(request, 'usuarios/perfil.html', context)


@login_required(login_url='usuarios:login')
def vista_editar_perfil(request):
    form = EditarPerfilForm(request.POST or None, instance=request.user)
    if request.method == 'POST' and form.is_valid():
        form.save()
        messages.success(request, 'Perfil actualizado correctamente.')
        return redirect('usuarios:perfil')
    return render(request, 'usuarios/editar_perfil.html', {'form': form})




# Los paneles por rol viven ahora en apps.dashboard (HU05), que centraliza
# indicadores, filtros de periodo y graficas. Estas vistas se conservan como
# redirects porque sus nombres de URL estan cableados en LOGIN_REDIRECT_URL,
# en roles.py y en todo el menu lateral.

@solo_admin
def dashboard_admin(request):
    return redirect('dashboard:panel')


@solo_empresa
def dashboard_empresa(request):
    return redirect('dashboard:panel')


@solo_profesor
def dashboard_profesor(request):
    return redirect('dashboard:panel')


@solo_estudiante
def dashboard_estudiante(request):
    return redirect('dashboard:panel')


# ── Vistas de Administración ──────────────────────────────────

@solo_admin
def lista_usuarios(request):
    """Gestión y listado de todos los usuarios del sistema."""
    usuarios = Usuario.objects.all()

    q = request.GET.get('q', '').strip()
    if q:
        usuarios = usuarios.filter(
            Q(username__icontains=q) | Q(email__icontains=q)
            | Q(first_name__icontains=q) | Q(last_name__icontains=q)
        )
    rol = request.GET.get('rol', '')
    if rol:
        usuarios = usuarios.filter(rol=rol)
    estado = request.GET.get('estado', '')
    if estado == 'activos':
        usuarios = usuarios.filter(is_active=True)
    elif estado == 'inactivos':
        usuarios = usuarios.filter(is_active=False)

    paginator = Paginator(usuarios.order_by('-date_joined'), 30)
    return render(request, 'usuarios/admin/lista_usuarios.html', {
        'usuarios': paginator.get_page(request.GET.get('page')),
        'titulo': 'Gestión de Usuarios',
        'q': q,
        'rol': rol,
        'estado': estado,
        'roles': Usuario.ROL_CHOICES,
        'total': usuarios.count(),
    })


@require_POST
@solo_admin
def cambiar_rol_usuario(request, pk):
    """Cambia el rol de una cuenta y lo deja registrado en la bitacora."""
    usuario = get_object_or_404(Usuario, pk=pk)
    nuevo_rol = request.POST.get('rol', '')
    if nuevo_rol not in dict(Usuario.ROL_CHOICES):
        messages.error(request, 'Rol no valido.')
        return redirect('usuarios:lista_usuarios')
    if usuario.pk == request.user.pk:
        messages.error(request, 'No puedes cambiar tu propio rol.')
        return redirect('usuarios:lista_usuarios')

    anterior = usuario.rol
    if anterior != nuevo_rol:
        usuario.rol = nuevo_rol
        usuario.save(update_fields=['rol'])
        LogActividad.objects.create(
            usuario=usuario, identificador=usuario.username, accion='CAMBIO_ROL',
            detalle=f'Rol cambiado de {anterior or "sin rol"} a {nuevo_rol}.',
            realizado_por=request.user,
        )
        messages.success(request, f'{usuario.username} ahora tiene el rol {nuevo_rol}.')
    return redirect('usuarios:lista_usuarios')


@require_POST
@solo_admin
def alternar_estado_usuario(request, pk):
    """Activa o desactiva una cuenta."""
    usuario = get_object_or_404(Usuario, pk=pk)
    if usuario.pk == request.user.pk:
        messages.error(request, 'No puedes desactivar tu propia cuenta.')
        return redirect('usuarios:lista_usuarios')

    usuario.is_active = not usuario.is_active
    usuario.activo = usuario.is_active
    usuario.save(update_fields=['is_active', 'activo'])
    LogActividad.objects.create(
        usuario=usuario, identificador=usuario.username, accion='CAMBIO_ESTADO',
        detalle='Cuenta activada.' if usuario.is_active else 'Cuenta desactivada.',
        realizado_por=request.user,
    )
    estado = 'activada' if usuario.is_active else 'desactivada'
    messages.success(request, f'Cuenta de {usuario.username} {estado}.')
    return redirect('usuarios:lista_usuarios')


@solo_admin
def importar_usuarios(request):
    """Carga masiva de usuarios desde CSV o Excel, con previsualizacion (HU13)."""
    lectura = None

    if request.method == 'POST' and request.FILES.get('archivo'):
        lectura = leer_archivo(request.FILES['archivo'])

        if not lectura.error_global and request.POST.get('accion') == 'confirmar':
            creados = importar(lectura.validas, request.user)
            messages.success(request, f'{len(creados)} usuario(s) creado(s) correctamente.')
            if lectura.invalidas:
                messages.warning(
                    request,
                    f'{len(lectura.invalidas)} fila(s) se omitieron por errores de validacion.'
                )
            return redirect('usuarios:lista_usuarios')

    return render(request, 'usuarios/admin/importar_usuarios.html', {
        'titulo': 'Importación Masiva de Usuarios',
        'lectura': lectura,
        'columnas': COLUMNAS,
        'columnas_obligatorias': COLUMNAS_OBLIGATORIAS,
    })


@solo_admin
def plantilla_importacion(request):
    """Descarga una plantilla CSV de ejemplo."""
    respuesta = HttpResponse(plantilla_csv(), content_type='text/csv; charset=utf-8')
    respuesta['Content-Disposition'] = 'attachment; filename="plantilla-usuarios.csv"'
    return respuesta


@solo_admin
def log_actividad(request):
    """Bitacora de accesos y cambios sobre las cuentas (HU13)."""
    registros = LogActividad.objects.select_related('usuario', 'realizado_por')
    accion = request.GET.get('accion', '')
    if accion:
        registros = registros.filter(accion=accion)

    paginator = Paginator(registros, 50)
    return render(request, 'usuarios/admin/log_actividad.html', {
        'titulo': 'Registro de Actividad',
        'registros': paginator.get_page(request.GET.get('page')),
        'accion': accion,
        'acciones': LogActividad.ACCIONES,
    })


@solo_admin
def reportes_admin(request):
    """Alias historico: la generacion de informes vive en apps.reportes (HU06)."""
    return redirect('reportes:constructor')

