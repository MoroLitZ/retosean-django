from django.db import models


class ReporteGenerado(models.Model):
    FORMATOS = [
        ("PDF", "PDF"),
        ("EXCEL", "Excel"),
    ]
    usuario = models.ForeignKey("usuarios.Usuario", on_delete=models.CASCADE, related_name="reportes")
    formato = models.CharField(max_length=10, choices=FORMATOS)
    filtros = models.JSONField(null=True, blank=True)
    archivo = models.FileField(upload_to="reportes/", null=True, blank=True)
    enviado_por_email = models.BooleanField(default=False)
    creado_en = models.DateTimeField(auto_now_add=True)

    class Meta:
        db_table = "reportes_generados"
        verbose_name = "Reporte Generado"
        verbose_name_plural = "Reportes Generados"
        ordering = ["-creado_en"]

    def __str__(self):
        return f"{self.usuario.username} - {self.formato} ({self.creado_en.date()})"
