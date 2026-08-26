from apps.notificaciones.models import Notificacion

def crear_notificacion(usuario, titulo, mensaje, tipo="INFO", link=""):
    """
    Función centralizada para crear una notificación in-app para cualquier usuario.
    """
    if not usuario:
        return None
        
    notificacion = Notificacion.objects.create(
        usuario=usuario,
        tipo=tipo,
        titulo=titulo,
        mensaje=mensaje,
        link=link
    )
    return notificacion