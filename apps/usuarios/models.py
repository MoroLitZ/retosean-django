from django.contrib.auth.models import AbstractUser
from django.db import models


class Usuario(AbstractUser):
    ROL_CHOICES = [
        ("ADMIN", "Administrador"),
        ("EMPRESA", "Empresa"),
        ("PROFESOR", "Profesor"),
        ("ESTUDIANTE", "Estudiante"),
        ("EXPERTO", "Experto"),
    ]
    rol = models.CharField(max_length=20, choices=ROL_CHOICES)
    telefono = models.CharField(max_length=20, blank=True)
    activo = models.BooleanField(default=True)
    fecha_creacion = models.DateTimeField(auto_now_add=True)

    class Meta:
        db_table = "usuarios"
        verbose_name = "Usuario"
        verbose_name_plural = "Usuarios"

    def __str__(self):
        return f"{self.username} ({self.rol})"


class LogActividad(models.Model):
    """Bitacora de accesos y cambios sobre usuarios (HU13).

    Se alimenta de las senales de autenticacion y de las vistas de
    administracion que modifican rol o estado de una cuenta.
    """

    ACCIONES = [
        ("LOGIN", "Inicio de sesion"),
        ("LOGIN_FALLIDO", "Intento fallido de inicio de sesion"),
        ("LOGOUT", "Cierre de sesion"),
        ("CREACION", "Creacion de usuario"),
        ("CAMBIO_ROL", "Cambio de rol"),
        ("CAMBIO_ESTADO", "Activacion o desactivacion"),
        ("IMPORTACION", "Importacion masiva"),
    ]
    usuario = models.ForeignKey(
        "usuarios.Usuario", on_delete=models.SET_NULL, null=True, blank=True,
        related_name="actividad",
    )
    # Se guarda aparte porque en un login fallido puede no existir el usuario.
    identificador = models.CharField(max_length=150, blank=True)
    accion = models.CharField(max_length=20, choices=ACCIONES, db_index=True)
    detalle = models.TextField(blank=True)
    realizado_por = models.ForeignKey(
        "usuarios.Usuario", on_delete=models.SET_NULL, null=True, blank=True,
        related_name="actividad_registrada",
    )
    ip = models.GenericIPAddressField(null=True, blank=True)
    creado_en = models.DateTimeField(auto_now_add=True, db_index=True)

    class Meta:
        db_table = "log_actividad"
        verbose_name = "Registro de actividad"
        verbose_name_plural = "Registros de actividad"
        ordering = ["-creado_en"]

    def __str__(self):
        quien = self.usuario.username if self.usuario else (self.identificador or "anonimo")
        return f"{quien} - {self.get_accion_display()} ({self.creado_en:%Y-%m-%d %H:%M})"
