from django.db import models
from django.conf import settings

class Notificacion(models.Model):
    TIPOS = [
        ("INFO", "Informacion"),
        ("EXITO", "Exito"),
        ("ADVERTENCIA", "Advertencia"),
        ("ERROR", "Error"),
    ]
    usuario = models.ForeignKey("usuarios.Usuario", on_delete=models.CASCADE, related_name="notificaciones")
    tipo = models.CharField(max_length=20, choices=TIPOS, default="INFO")
    titulo = models.CharField(max_length=200)
    mensaje = models.TextField()
    leida = models.BooleanField(default=False)
    link = models.CharField(max_length=300, blank=True)
    creada_en = models.DateTimeField(auto_now_add=True)

    class Meta:
        db_table = "notificaciones"
        verbose_name = "Notificacion"
        verbose_name_plural = "Notificaciones"
        ordering = ["-creada_en"]

    def __str__(self):
        return f"{self.usuario.username} - {self.titulo}"