from django.shortcuts import render, redirect
from django.contrib.auth import login, logout
from django.contrib import messages
from .forms import LoginForm, RegistroUsuarioForm
from django.contrib.auth.decorators import login_required

# Create your views here.

REDIRECCION_POR_ROL = {
    'ADMIN':'usuarios:perfil',
    'EMPRESA':'usuarios:perfil',
    'PROFESOR':'usuarios:perfil',
    'ESTUDIANTE':'usuarios:perfil',
}


# se manda al user a su perfil si ya tiene registro en la app
def vista_login(request):
    if request.user.is_authenticated:
        return redirect(REDIRECCION_POR_ROL.get(request.user.rol, 'usuarios:perfil'))

    form = LoginForm(request, data=request.POST or None)
    if request.method == 'POST':
        if form.is_valid():
            user = form.get_user()
            login(request, user)
            return redirect(REDIRECCION_POR_ROL.get(user.rol, 'usuarios:perfil'))
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
    form = RegistroUsuarioForm(request.POST or None)
    if request.method == 'POST':
        if form.is_valid():
            user = form.save()
            login(request, user)
            messages.success(request, f'¡Bienvenido, {user.first_name}!')
            return redirect(REDIRECCION_POR_ROL.get(user.rol, 'usuarios:perfil'))
    return render(request, 'usuarios/registro.html', {'form': form})

@login_required(login_url='usuarios:login') # Se protege la vista para que únicamente los usuarios logeados puedan entrar

# la vista donde el usuario va a ir directamente despues del login o el registro
def vista_perfil(request):
    usuario_actual = request.user
    nombre_completo = usuario_actual.get_full_name().strip()
    
    if not nombre_completo:
        nombre_completo = "No registrado"
    
    rol_formateado = usuario_actual.get_rol_display().capitalize()

    context = {'nombre_completo': nombre_completo, 'rol_usuario':rol_formateado}

    return render(request, 'usuarios/perfil.html', context)