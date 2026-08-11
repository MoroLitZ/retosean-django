from django.db import models


class AgendaCierre(models.Model):
    reto = models.OneToOneField("retos.Reto", on_delete=models.CASCADE, related_name="agenda_cierre")
    fecha_hora = models.DateTimeField(null=True, blank=True)
    espacio = models.CharField(max_length=200, blank=True)
    recursos = models.TextField(blank=True)
    invitados = models.TextField(blank=True)
    agenda = models.TextField(blank=True)
    actualizado_en = models.DateTimeField(auto_now=True)

    class Meta:
        verbose_name = "Agenda de cierre"
        verbose_name_plural = "Agendas de cierre"

    def __str__(self):
        return f"Agenda de cierre - {self.reto}"


class CierreReto(models.Model):
    reto = models.OneToOneField("retos.Reto", on_delete=models.CASCADE, related_name="cierre")
    agenda = models.TextField(blank=True)
    acta_url = models.FileField(upload_to="actas_cierre/", null=True, blank=True)
    notas = models.TextField(blank=True)
    catering = models.TextField(blank=True)
    av_notas = models.TextField(blank=True)
    invitados = models.TextField(blank=True)
    espacio_notas = models.TextField(blank=True)
    encuesta_enviada = models.BooleanField(default=False)
    cerrado_por = models.ForeignKey("usuarios.Usuario", on_delete=models.SET_NULL, null=True, related_name="cierres")
    cerrado_en = models.DateTimeField(null=True, blank=True)
    actualizado_en = models.DateTimeField(auto_now=True)

    class Meta:
        db_table = "cierres_reto"
        verbose_name = "Cierre de Reto"
        verbose_name_plural = "Cierres de Reto"

    def __str__(self):
        return f"Cierre de {self.reto}"


class Reconocimiento(models.Model):
    cierre = models.ForeignKey(CierreReto, on_delete=models.CASCADE, related_name="reconocimientos")
    usuario = models.ForeignKey("usuarios.Usuario", on_delete=models.SET_NULL, null=True, blank=True, related_name="reconocimientos")
    nombre_destinatario = models.CharField(max_length=150)
    descripcion = models.TextField()
    creado_en = models.DateTimeField(auto_now_add=True)

    class Meta:
        db_table = "reconocimientos"
        verbose_name = "Reconocimiento"
        verbose_name_plural = "Reconocimientos"

    def __str__(self):
        return f"{self.nombre_destinatario} - {self.cierre.reto}"


class EntregableFinal(models.Model):
    cierre = models.ForeignKey(CierreReto, on_delete=models.CASCADE, related_name="entregables_finales")
    nombre = models.CharField(max_length=180)
    archivo = models.FileField(upload_to="cierres/entregables/")
    cargado_por = models.ForeignKey(
        "usuarios.Usuario", on_delete=models.SET_NULL, null=True, related_name="entregables_finales_cargados"
    )
    creado_en = models.DateTimeField(auto_now_add=True)

    class Meta:
        ordering = ["-creado_en"]

    def __str__(self):
        return self.nombre


class EncuestaSatisfaccion(models.Model):
    cierre = models.ForeignKey(CierreReto, on_delete=models.CASCADE, related_name="encuestas")
    participante = models.ForeignKey(
        "usuarios.Usuario", on_delete=models.CASCADE, related_name="encuestas_satisfaccion"
    )
    calificacion = models.PositiveSmallIntegerField(null=True, blank=True)
    comentario = models.TextField(blank=True)
    respondida_en = models.DateTimeField(null=True, blank=True)
    creada_en = models.DateTimeField(auto_now_add=True)

    class Meta:
        unique_together = ["cierre", "participante"]

    @property
    def respondida(self):
        return self.respondida_en is not None
