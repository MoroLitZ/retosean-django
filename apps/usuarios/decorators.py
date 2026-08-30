"""Decoradores de control de acceso por rol, compartidos por todas las apps."""

from functools import wraps

from django.contrib import messages
from django.contrib.auth.decorators import login_required
from django.core.exceptions import PermissionDenied
from django.shortcuts import redirect

from .roles import (
    ROL_ADMIN,
    ROL_EMPRESA,
    ROL_ESTUDIANTE,
    ROL_PROFESOR,
    tiene_rol,
    url_dashboard_para,
)


def rol_requerido(*roles_permitidos, raise_exception=False):
    """Restringe una vista a los roles indicados.

    Por defecto redirige al panel del usuario con un mensaje, que es como se
    comportaban las vistas de `academico`, `usuarios` y `seguimiento`. Con
    `raise_exception=True` lanza PermissionDenied (403), que es lo que esperan
    las vistas de `retos` y sus tests.
    """
    def decorador(view_func):
        @wraps(view_func)
        @login_required(login_url='usuarios:login')
        def vista_envuelta(request, *args, **kwargs):
            if tiene_rol(request.user, *roles_permitidos):
                return view_func(request, *args, **kwargs)
            if raise_exception:
                raise PermissionDenied('No tienes acceso a esta seccion.')
            messages.error(request, 'No tienes acceso a esta seccion.')
            return redirect(url_dashboard_para(request.user))
        return vista_envuelta
    return decorador


def solo_admin(view_func):
    return rol_requerido(ROL_ADMIN)(view_func)


def solo_empresa(view_func):
    return rol_requerido(ROL_EMPRESA)(view_func)


def solo_profesor(view_func):
    return rol_requerido(ROL_PROFESOR)(view_func)


def solo_estudiante(view_func):
    return rol_requerido(ROL_ESTUDIANTE)(view_func)


def profesor_o_admin(view_func):
    return rol_requerido(ROL_PROFESOR, ROL_ADMIN)(view_func)
