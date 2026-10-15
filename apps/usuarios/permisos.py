"""Permisos especiales por modulo que el administrador asigna a cada usuario (HU13).

Son permisos de Django (`app_label.codename`); el rol sigue mandando sobre el
acceso general y estos permisos habilitan acciones puntuales adicionales.
Para agregar uno nuevo basta con declararlo en el `Meta.permissions` del
modelo de su app y listarlo aqui.
"""

PERMISOS_MODULO = [
    (
        "empresas.cargar_listas_informa",
        "Cargar Listas Informa",
        "Carga el documento Listas Informa de las empresas (HU00). "
        "Normalmente un líder de facultad designado por el líder de retos.",
    ),
]


def permiso_por_codigo(codigo):
    from django.contrib.auth.models import Permission

    app_label, codename = codigo.split(".", 1)
    return Permission.objects.get(content_type__app_label=app_label, codename=codename)
