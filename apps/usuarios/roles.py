"""Fuente unica de verdad sobre los roles del sistema.

Este modulo no importa vistas ni modelos de otras apps a proposito: lo consumen
los decoradores, los context processors y varias vistas, y cualquier import
hacia arriba crearia ciclos.
"""

ROL_ADMIN = 'ADMIN'
ROL_EMPRESA = 'EMPRESA'
ROL_PROFESOR = 'PROFESOR'
ROL_ESTUDIANTE = 'ESTUDIANTE'

DASHBOARD_POR_ROL = {
    ROL_ADMIN: 'usuarios:admin_dashboard',
    ROL_EMPRESA: 'usuarios:empresa_dashboard',
    ROL_PROFESOR: 'usuarios:profesor_dashboard',
    ROL_ESTUDIANTE: 'usuarios:estudiante_dashboard',
}


def es_admin(user):
    """Un superusuario de Django y un usuario con rol ADMIN son equivalentes.

    Antes cada app decidia por su cuenta: las vistas de `usuarios` miraban solo
    `is_superuser` y las de `retos` aceptaban ambos, asi que un ADMIN creado
    desde el registro se quedaba sin acceso a media plataforma.
    """
    return bool(
        getattr(user, 'is_authenticated', False)
        and (user.is_superuser or getattr(user, 'rol', None) == ROL_ADMIN)
    )


def rol_de(user):
    """Rol efectivo del usuario, normalizando al superusuario como ADMIN."""
    if not getattr(user, 'is_authenticated', False):
        return None
    if es_admin(user):
        return ROL_ADMIN
    return getattr(user, 'rol', None)


def tiene_rol(user, *roles):
    return rol_de(user) in roles


def url_dashboard_para(user):
    """Nombre de URL del panel que corresponde al usuario."""
    return DASHBOARD_POR_ROL.get(rol_de(user), 'usuarios:perfil')
