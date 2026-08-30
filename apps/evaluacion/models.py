import os
import uuid
from decimal import Decimal

from django.core.exceptions import ValidationError
from django.core.validators import MinValueValidator
from django.db import models


def ruta_entregables(instance, filename):
    ext = filename.split(".")[-1]
    return os.path.join("entregables", f"reto_{instance.reto_id}", f"{uuid.uuid4().hex}.{ext}")


class Entregable(models.Model):
    ESTADOS = [
        ("ENVIADO", "Enviado"),
        ("EN_REVISION", "En Revision"),
        ("CALIFICADO", "Calificado"),
    ]
    reto = models.ForeignKey("retos.Reto", on_delete=models.CASCADE, related_name="entregables")
    equipo = models.ForeignKey(
        "participaciones.Equipo", on_delete=models.SET_NULL, null=True, blank=True,
        related_name="entregables",
    )
    estudiante = models.ForeignKey("usuarios.Usuario", on_delete=models.CASCADE, related_name="entregas")
    titulo = models.CharField(max_length=180, default="Entregable del reto")
    es_final = models.BooleanField(default=False)
    puntaje_maximo = models.DecimalField(
        max_digits=6, decimal_places=2, default=Decimal("5.00"),
        validators=[MinValueValidator(Decimal("0.01"))],
    )
    archivo = models.FileField(upload_to=ruta_entregables)
    comentario_estudiante = models.TextField(blank=True, null=True)
    comentario_profesor = models.TextField(blank=True, null=True)
    nota = models.DecimalField(max_digits=6, decimal_places=2, null=True, blank=True)
    fecha_entrega = models.DateTimeField(auto_now_add=True)
    actualizado_en = models.DateTimeField(auto_now=True)
    estado = models.CharField(max_length=20, choices=ESTADOS, default="ENVIADO")

    class Meta:
        db_table = "entregables"
        verbose_name = "Entregable"
        verbose_name_plural = "Entregables"
        unique_together = ["reto", "estudiante", "titulo"]

    def save(self, *args, **kwargs):
        # Al reemplazar el archivo borramos el anterior, pero solo si existe
        # y si el storage es local: en otro caso .path lanza excepcion.
        try:
            anterior = Entregable.objects.get(id=self.id)
        except Entregable.DoesNotExist:
            anterior = None
        if anterior and anterior.archivo and anterior.archivo != self.archivo:
            try:
                if os.path.isfile(anterior.archivo.path):
                    os.remove(anterior.archivo.path)
            except (ValueError, NotImplementedError, OSError):
                pass
        super().save(*args, **kwargs)

    def __str__(self):
        return f"{self.reto.titulo} - {self.estudiante.username} ({self.estado})"

    @property
    def nombre_archivo(self):
        return os.path.basename(self.archivo.name)


class Rubrica(models.Model):
    reto = models.ForeignKey("retos.Reto", on_delete=models.CASCADE, related_name="rubricas")
    nombre = models.CharField(max_length=150)
    descripcion = models.TextField(blank=True)
    activa = models.BooleanField(default=True)
    creada_por = models.ForeignKey(
        "usuarios.Usuario", on_delete=models.SET_NULL, null=True, related_name="rubricas_creadas"
    )
    creada_en = models.DateTimeField(auto_now_add=True)

    class Meta:
        ordering = ["nombre"]
        unique_together = ["reto", "nombre"]

    def __str__(self):
        return f"{self.nombre} - {self.reto}"

    @property
    def puntaje_maximo(self):
        return sum((criterio.puntaje_maximo for criterio in self.criterios.all()), Decimal("0"))


class CriterioRubrica(models.Model):
    rubrica = models.ForeignKey(Rubrica, on_delete=models.CASCADE, related_name="criterios")
    nombre = models.CharField(max_length=150)
    descripcion = models.TextField(blank=True)
    puntaje_maximo = models.DecimalField(
        max_digits=6, decimal_places=2, validators=[MinValueValidator(Decimal("0.01"))]
    )
    orden = models.PositiveSmallIntegerField(default=0)

    class Meta:
        ordering = ["orden", "id"]

    def __str__(self):
        return self.nombre


class Evaluacion(models.Model):
    entregable = models.ForeignKey(Entregable, on_delete=models.CASCADE, related_name="evaluaciones")
    profesor = models.ForeignKey("usuarios.Usuario", on_delete=models.CASCADE, related_name="evaluaciones")
    nota = models.DecimalField(max_digits=6, decimal_places=2)
    comentario = models.TextField(blank=True)
    rubrica = models.ForeignKey(
        Rubrica, on_delete=models.SET_NULL, null=True, blank=True, related_name="evaluaciones"
    )
    fecha = models.DateTimeField(auto_now_add=True)
    actualizado_en = models.DateTimeField(auto_now=True)

    class Meta:
        db_table = "evaluaciones"
        verbose_name = "Evaluacion"
        verbose_name_plural = "Evaluaciones"
        unique_together = ["entregable", "profesor"]

    def __str__(self):
        return f"Evaluacion de {self.entregable} por {self.profesor.username}"

    def clean(self):
        maximo = self.rubrica.puntaje_maximo if self.rubrica_id else self.entregable.puntaje_maximo
        if self.nota < 0 or self.nota > maximo:
            raise ValidationError({"nota": f"La nota debe estar entre 0 y {maximo}."})
        if self.rubrica_id and self.rubrica.reto_id != self.entregable.reto_id:
            raise ValidationError({"rubrica": "La rubrica no pertenece al reto del entregable."})


class EvaluacionCriterio(models.Model):
    evaluacion = models.ForeignKey(Evaluacion, on_delete=models.CASCADE, related_name="puntajes_criterio")
    criterio = models.ForeignKey(CriterioRubrica, on_delete=models.PROTECT, related_name="puntajes")
    puntaje = models.DecimalField(max_digits=6, decimal_places=2)

    class Meta:
        unique_together = ["evaluacion", "criterio"]

    def __str__(self):
        return f"{self.evaluacion} / {self.criterio}: {self.puntaje}"

    def clean(self):
        if self.criterio_id and (self.puntaje < 0 or self.puntaje > self.criterio.puntaje_maximo):
            raise ValidationError({"puntaje": f"El puntaje debe estar entre 0 y {self.criterio.puntaje_maximo}."})


class ComentarioEntregable(models.Model):
    entregable = models.ForeignKey(Entregable, on_delete=models.CASCADE, related_name="historial_comentarios")
    autor = models.ForeignKey(
        "usuarios.Usuario", on_delete=models.SET_NULL, null=True, related_name="comentarios_entregables"
    )
    comentario = models.TextField()
    creado_en = models.DateTimeField(auto_now_add=True)

    class Meta:
        ordering = ["-creado_en"]

    def __str__(self):
        return f"Comentario en {self.entregable_id} - {self.creado_en:%Y-%m-%d}"
