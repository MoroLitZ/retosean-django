from django.db import models


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
    cerrado_en = models.DateTimeField(auto_now_add=True)

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
