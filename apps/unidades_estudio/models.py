from django.db import models


class UnidadEstudio(models.Model):
    codigo = models.CharField(max_length=20, unique=True, verbose_name="Código")
    nombre = models.CharField(max_length=200, verbose_name="Nombre de la unidad")
    programa = models.ForeignKey(
        "academico.Programa",
        on_delete=models.CASCADE,
        related_name="unidades_estudio",
        verbose_name="Programa académico",
    )
    periodo = models.CharField(max_length=20, verbose_name="Periodo (ej: 2026-2)")
    ciclo = models.CharField(max_length=50, verbose_name="Ciclo / Semestre")
    archivo = models.FileField(
        upload_to="unidades_estudio/",
        null=True,
        blank=True,
        verbose_name="Archivo (PDF con info completa)",
    )
    activo = models.BooleanField(default=True, verbose_name="Activo")
    creado_en = models.DateTimeField(auto_now_add=True)
    actualizado_en = models.DateTimeField(auto_now=True)

    class Meta:
        db_table = "unidades_estudio"
        verbose_name = "Unidad de Estudio"
        verbose_name_plural = "Unidades de Estudio"
        ordering = ["programa", "ciclo", "nombre"]

    def __str__(self):
        return f"{self.codigo} - {self.nombre} ({self.programa.nombre})"

