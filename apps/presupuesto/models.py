from django.db import models


class Presupuesto(models.Model):
    reto = models.OneToOneField("retos.Reto", on_delete=models.CASCADE, related_name="presupuesto")
    monto_total = models.DecimalField(max_digits=12, decimal_places=2, default=0)
    creado_en = models.DateTimeField(auto_now_add=True)
    actualizado_en = models.DateTimeField(auto_now=True)

    class Meta:
        db_table = "presupuestos"
        verbose_name = "Presupuesto"
        verbose_name_plural = "Presupuestos"

    def __str__(self):
        return f"Presupuesto de {self.reto}"

    @property
    def monto_ejecutado(self):
        return sum(g.monto for g in self.gastos.all())

    @property
    def porcentaje_ejecutado(self):
        if self.monto_total == 0:
            return 0
        return (self.monto_ejecutado / self.monto_total) * 100


class Gasto(models.Model):
    CATEGORIAS = [
        ("REFRIGERIOS", "Refrigerios"),
        ("MATERIALES", "Materiales"),
        ("IMPRESIONES", "Impresiones"),
        ("ESPACIOS", "Espacios"),
        ("OTROS", "Otros"),
    ]
    presupuesto = models.ForeignKey(Presupuesto, on_delete=models.CASCADE, related_name="gastos")
    categoria = models.CharField(max_length=30, choices=CATEGORIAS)
    monto = models.DecimalField(max_digits=12, decimal_places=2)
    descripcion = models.TextField(blank=True)
    comprobante = models.FileField(upload_to="comprobantes/", null=True, blank=True)
    registrado_por = models.ForeignKey("usuarios.Usuario", on_delete=models.SET_NULL, null=True, related_name="gastos")
    creado_en = models.DateTimeField(auto_now_add=True)

    class Meta:
        db_table = "gastos"
        verbose_name = "Gasto"
        verbose_name_plural = "Gastos"

    def __str__(self):
        return f"{self.get_categoria_display()} - ${self.monto}"
