"""Expone el rol efectivo del usuario a todas las plantillas.

Evita que base.html tenga que repetir `user.is_superuser or user.rol == 'ADMIN'`
en cada bloque, que es de donde venia el bug de que un ADMIN no superusuario
se quedaba sin menu lateral.
"""

from .roles import (
    ROL_EMPRESA,
    ROL_ESTUDIANTE,
    ROL_PROFESOR,
    es_admin,
    rol_de,
    tiene_rol,
)


def rol_flags(request):
    user = getattr(request, 'user', None)
    if user is None or not user.is_authenticated:
        return {
            'rol_actual': None,
            'es_admin': False,
            'es_empresa': False,
            'es_profesor': False,
            'es_estudiante': False,
        }
    return {
        'rol_actual': rol_de(user),
        'es_admin': es_admin(user),
        'es_empresa': tiene_rol(user, ROL_EMPRESA),
        'es_profesor': tiene_rol(user, ROL_PROFESOR),
        'es_estudiante': tiene_rol(user, ROL_ESTUDIANTE),
    }
