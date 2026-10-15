"""Cambios de estado de cuentas de usuario (HU13).

Activar/inactivar y cambiar de rol quedan en la bitacora de actividad
(`LogActividad`) y en el historial de estados comun (`seguimiento.HistorialEstado`).
Django rechaza en cada peticion a un usuario con `is_active=False`, asi que
inactivar una cuenta cierra de inmediato sus sesiones abiertas.
"""

from django.db import transaction

from apps.seguimiento.services import registrar_cambio_estado

from .models import LogActividad


def _etiqueta_activo(activo):
    return "ACTIVO" if activo else "INACTIVO"


@transaction.atomic
def cambiar_estado_cuenta(usuario, activo, realizado_por, comentario=""):
    """Activa o inactiva una cuenta. No hace nada si ya esta en ese estado."""
    if usuario.is_active == activo:
        return False
    anterior = _etiqueta_activo(usuario.is_active)
    usuario.is_active = activo
    usuario.activo = activo
    usuario.save(update_fields=["is_active", "activo"])
    detalle = comentario or ("Cuenta activada." if activo else "Cuenta desactivada.")
    LogActividad.objects.create(
        usuario=usuario, identificador=usuario.username, accion="CAMBIO_ESTADO",
        detalle=detalle, realizado_por=realizado_por,
    )
    registrar_cambio_estado(
        usuario, anterior, _etiqueta_activo(activo), realizado_por, detalle, campo="is_active",
    )
    return True


@transaction.atomic
def cambiar_rol(usuario, nuevo_rol, realizado_por):
    """Cambia el rol de la cuenta. No hace nada si ya tiene ese rol."""
    anterior = usuario.rol
    if anterior == nuevo_rol:
        return False
    usuario.rol = nuevo_rol
    usuario.save(update_fields=["rol"])
    detalle = f'Rol cambiado de {anterior or "sin rol"} a {nuevo_rol}.'
    LogActividad.objects.create(
        usuario=usuario, identificador=usuario.username, accion="CAMBIO_ROL",
        detalle=detalle, realizado_por=realizado_por,
    )
    registrar_cambio_estado(usuario, anterior, nuevo_rol, realizado_por, detalle, campo="rol")
    return True


@transaction.atomic
def asignar_permisos_modulo(usuario, codigos, realizado_por):
    """Deja al usuario exactamente con los permisos especiales `codigos`.

    Devuelve (agregados, quitados) y registra el cambio en la bitacora.
    """
    from .permisos import PERMISOS_MODULO, permiso_por_codigo

    validos = {codigo for codigo, _, _ in PERMISOS_MODULO}
    deseados = set(codigos) & validos
    # Solo se gestionan los permisos explicitos: has_perm de un superusuario es siempre True.
    explicitos = {
        f"{p.content_type.app_label}.{p.codename}"
        for p in usuario.user_permissions.select_related("content_type")
    } & validos
    agregados = deseados - explicitos
    quitados = explicitos - deseados
    for codigo in agregados:
        usuario.user_permissions.add(permiso_por_codigo(codigo))
    for codigo in quitados:
        usuario.user_permissions.remove(permiso_por_codigo(codigo))
    if agregados or quitados:
        partes = []
        if agregados:
            partes.append("otorgados: " + ", ".join(sorted(agregados)))
        if quitados:
            partes.append("retirados: " + ", ".join(sorted(quitados)))
        LogActividad.objects.create(
            usuario=usuario, identificador=usuario.username, accion="CAMBIO_PERMISOS",
            detalle="Permisos especiales " + "; ".join(partes) + ".", realizado_por=realizado_por,
        )
    return agregados, quitados


def enviar_bienvenida(usuario):
    """Correo de bienvenida al crear una cuenta (HU13).

    Se envia al confirmar la transaccion; si la cuenta no tiene contrasena
    utilizable (importacion masiva sin contrasena) incluye el enlace para
    definirla. Un fallo de correo no impide crear la cuenta.
    """
    from django.conf import settings
    from django.core.mail import send_mail
    from django.urls import reverse

    if not usuario.email:
        return

    nombre = usuario.get_full_name() or usuario.username
    base = settings.SITE_URL.rstrip("/")
    lineas = [
        f"Hola {nombre},",
        "",
        "Te damos la bienvenida a RetosEAN, la plataforma de retos Universidad-Empresa "
        "de la Universidad EAN.",
        "",
        f"Usuario: {usuario.username}",
        f"Ingresa en: {base}{reverse('usuarios:login')}",
    ]
    if not usuario.has_usable_password():
        # "Olvide mi contrasena" ignora las cuentas sin contrasena utilizable,
        # asi que se envia directamente el enlace firmado para definirla.
        from django.contrib.auth.tokens import default_token_generator
        from django.utils.encoding import force_bytes
        from django.utils.http import urlsafe_base64_encode

        enlace = reverse("usuarios:password_reset_confirm", kwargs={
            "uidb64": urlsafe_base64_encode(force_bytes(usuario.pk)),
            "token": default_token_generator.make_token(usuario),
        })
        lineas += ["", f"Tu cuenta aun no tiene contrasena. Definela aqui: {base}{enlace}"]
    lineas += ["", "Equipo RetosEAN"]

    def _enviar():
        send_mail(
            "Bienvenido a RetosEAN",
            "\n".join(lineas),
            settings.DEFAULT_FROM_EMAIL,
            [usuario.email],
            fail_silently=True,
        )

    transaction.on_commit(_enviar)
