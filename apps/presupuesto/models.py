from decimal import Decimal

from django.db import models
from django.db.models import Sum
from django.db.models.functions import Coalesce


class Presupuesto(models.Model):
    reto = models.OneToOneField("retos.Reto", on_delete=models.CASCADE, related_name="presupuesto")
    monto_total = models.DecimalField(max_digits=12, decimal_places=2, default=0)
    # Evita repetir la alerta del 80% en cada gasto posterior.
    alerta_80_enviada = models.BooleanField(default=False)
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
        # Agregamos en la base de datos: iterar self.gastos.all() era N+1
        # y devolvia el int 0 en vez de Decimal cuando no habia gastos.
        return self.gastos.aggregate(
            total=Coalesce(Sum("monto"), Decimal("0.00"))
        )["total"]

    @property
    def porcentaje_ejecutado(self):
        if not self.monto_total:
            return Decimal("0.00")
        return (self.monto_ejecutado / self.monto_total) * 100

    @property
    def monto_disponible(self):
        return self.monto_total - self.monto_ejecutado


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
