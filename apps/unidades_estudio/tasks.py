from celery import shared_task
from django.core.mail import send_mail
from django.contrib.auth import get_user_model
from apps.notificaciones.models import Notificacion  # <-- ¡Importamos de tu app notificaciones!

Usuario = get_user_model()

@shared_task
def enviar_correo_nueva_unidad(codigo, nombre):
    # Buscamos a los usuarios que deben enterarse (Profesores, Admins o Superusuarios)
    destinatarios = Usuario.objects.filter(rol__in=['PROFESOR', 'ADMIN']) | Usuario.objects.filter(is_superuser=True)
    
    for usuario in destinatarios:
        # 1. Creamos la notificación para la campanita usando tus campos exactos
        Notificacion.objects.create(
            usuario=usuario,
            tipo="EXITO",
            titulo="Nueva Unidad de Estudio",
            mensaje=f"Se ha creado la unidad de estudio '{nombre}' con código {codigo}.",
            leida=False,
            link="/unidades-estudio/"  # O la ruta que prefieras para ver el detalle
        )

        # 2. Enviamos el correo electrónico en segundo plano
        if usuario.email:
            send_mail(
                subject=f"Nueva Unidad de Estudio: {codigo}",
                message=f"Se ha registrado formalmente la unidad de estudio '{nombre}' ({codigo}) en RetosEAN.",
                from_email="no-reply@retosean.com",
                recipient_list=[usuario.email],
                fail_silently=True,
            )