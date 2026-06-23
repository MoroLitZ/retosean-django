from django.shortcuts import render, redirect
from django.contrib.auth import login, logout
from django.contrib import messages
from .forms import LoginForm, RegistroUsuarioForm

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

def vista_perfil(request):
    return render(request, 'usuarios/perfil.html')