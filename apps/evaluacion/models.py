import os
from django.db import models


def ruta_entregables(instance, filename):
    ext = filename.split(".")[-1]
    return os.path.join("entregables", f"reto_{instance.reto_id}", f"estudiante_{instance.estudiante.username}.{ext}")


class Entregable(models.Model):
    ESTADOS = [
        ("ENVIADO", "Enviado"),
        ("EN_REVISION", "En Revision"),
        ("CALIFICADO", "Calificado"),
    ]
    reto = models.ForeignKey("retos.Reto", on_delete=models.CASCADE, related_name="entregables")
    estudiante = models.ForeignKey("usuarios.Usuario", on_delete=models.CASCADE, related_name="entregas")
    archivo = models.FileField(upload_to=ruta_entregables)
    comentario_estudiante = models.TextField(blank=True, null=True)
    comentario_profesor = models.TextField(blank=True, null=True)
    nota = models.DecimalField(max_digits=4, decimal_places=2, null=True, blank=True)
    fecha_entrega = models.DateTimeField(auto_now_add=True)
    actualizado_en = models.DateTimeField(auto_now=True)
    estado = models.CharField(max_length=20, choices=ESTADOS, default="ENVIADO")

    class Meta:
        db_table = "entregables"
        verbose_name = "Entregable"
        verbose_name_plural = "Entregables"
        unique_together = ["reto", "estudiante"]

    def save(self, *args, **kwargs):
        try:
            this = Entregable.objects.get(id=self.id)
            if this.archivo != self.archivo:
                if os.path.isfile(this.archivo.path):
                    os.remove(this.archivo.path)
        except Entregable.DoesNotExist:
            pass
        super().save(*args, **kwargs)

    def __str__(self):
        return f"Reto {self.reto_id} - {self.estudiante.username} ({self.estado})"

    @property
    def nombre_archivo(self):
        return os.path.basename(self.archivo.name)


class Evaluacion(models.Model):
    entregable = models.ForeignKey(Entregable, on_delete=models.CASCADE, related_name="evaluaciones")
    profesor = models.ForeignKey("usuarios.Usuario", on_delete=models.CASCADE, related_name="evaluaciones")
    nota = models.DecimalField(max_digits=4, decimal_places=2)
    comentario = models.TextField(blank=True)
    fecha = models.DateTimeField(auto_now_add=True)

    class Meta:
        db_table = "evaluaciones"
        verbose_name = "Evaluacion"
        verbose_name_plural = "Evaluaciones"
        unique_together = ["entregable", "profesor"]

    def __str__(self):
        return f"Evaluacion de {self.entregable} por {self.profesor.username}"
