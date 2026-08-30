from django.db import models


class Notificacion(models.Model):
    TIPOS = [
        ("INFO", "Informacion"),
        ("EXITO", "Exito"),
        ("ADVERTENCIA", "Advertencia"),
        ("ERROR", "Error"),
    ]
    usuario = models.ForeignKey("usuarios.Usuario", on_delete=models.CASCADE, related_name="notificaciones")
    tipo = models.CharField(max_length=20, choices=TIPOS, default="INFO")
    # Slug del evento de dominio que la origino (ver notificaciones.eventos).
    evento = models.CharField(max_length=60, blank=True, db_index=True)
    titulo = models.CharField(max_length=200)
    mensaje = models.TextField()
    leida = models.BooleanField(default=False)
    leida_en = models.DateTimeField(null=True, blank=True)
    link = models.CharField(max_length=300, blank=True)
    # Idempotencia: evita duplicar la misma notificacion (util en recordatorios
    # que se ejecutan a diario desde el programador de tareas).
    clave_dedupe = models.CharField(max_length=120, blank=True, db_index=True)
    enviada_por_correo = models.BooleanField(default=False)
    error_envio = models.TextField(blank=True)
    creada_en = models.DateTimeField(auto_now_add=True)

    class Meta:
        db_table = "notificaciones"
        verbose_name = "Notificacion"
        verbose_name_plural = "Notificaciones"
        ordering = ["-creada_en"]
        constraints = [
            models.UniqueConstraint(
                fields=["usuario", "clave_dedupe"],
                condition=~models.Q(clave_dedupe=""),
                name="uniq_notificacion_dedupe",
            )
        ]

    def __str__(self):
        return f"{self.usuario.username} - {self.titulo}"

    def marcar_leida(self):
        if not self.leida:
            from django.utils import timezone

            self.leida = True
            self.leida_en = timezone.now()
            self.save(update_fields=["leida", "leida_en"])


class PreferenciaNotificacion(models.Model):
    """Preferencias de notificacion por usuario.

    Se guarda un unico registro por usuario con la lista de eventos silenciados
    en vez de una columna por evento, para no migrar el esquema cada vez que se
    agrega un evento nuevo al catalogo.
    """

    usuario = models.OneToOneField(
        "usuarios.Usuario", on_delete=models.CASCADE, related_name="preferencias_notificacion"
    )
    recibir_email = models.BooleanField(default=True)
    recibir_recordatorios = models.BooleanField(default=True)
    eventos_silenciados = models.JSONField(default=list, blank=True)
    actualizado_en = models.DateTimeField(auto_now=True)

    class Meta:
        db_table = "preferencias_notificacion"
        verbose_name = "Preferencia de Notificacion"
        verbose_name_plural = "Preferencias de Notificacion"

    def __str__(self):
        return f"Preferencias de {self.usuario.username}"

    def acepta(self, evento):
        return evento not in (self.eventos_silenciados or [])
