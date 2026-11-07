from decimal import Decimal

from django.db import models
from django.db.models import Q, Sum
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

    def _suma_gastos(self, estado):
        # Agregamos en la base de datos: iterar self.gastos.all() era N+1
        # y devolvia el int 0 en vez de Decimal cuando no habia gastos.
        return self.gastos.filter(estado=estado).aggregate(
            total=Coalesce(Sum("monto"), Decimal("0.00"))
        )["total"]

    def totales(self):
        """(comprometido, ejecutado, asignado, disponible) en UNA sola consulta.

        Cada propiedad de abajo lanza su propio agregado; leerlas juntas
        (p. ej. al revisar el umbral o al exportar) multiplicaba las consultas
        con los mismos numeros. Esto las resuelve todas de una vez.
        """
        cero = Decimal("0.00")
        fila = self.gastos.aggregate(
            comprometido=Coalesce(Sum("monto", filter=Q(estado=Gasto.COMPROMETIDO)), cero),
            ejecutado=Coalesce(Sum("monto", filter=Q(estado=Gasto.EJECUTADO)), cero),
        )
        comprometido, ejecutado = fila["comprometido"], fila["ejecutado"]
        asignado = comprometido + ejecutado
        disponible = self.monto_total - asignado
        return comprometido, ejecutado, asignado, disponible

    @property
    def monto_ejecutado(self):
        return self._suma_gastos(Gasto.EJECUTADO)

    @property
    def monto_comprometido(self):
        return self._suma_gastos(Gasto.COMPROMETIDO)

    @property
    def monto_asignado(self):
        """Comprometido + ejecutado: lo que ya no esta libre, se haya pagado o no."""
        return self.totales()[2]

    @property
    def porcentaje_ejecutado(self):
        if not self.monto_total:
            return Decimal("0.00")
        return (self.monto_asignado / self.monto_total) * 100

    @property
    def monto_disponible(self):
        return self.totales()[3]


class Gasto(models.Model):
    CATEGORIAS = [
        ("REFRIGERIOS", "Refrigerios"),
        ("MATERIALES", "Materiales"),
        ("IMPRESIONES", "Impresiones"),
        ("ESPACIOS", "Espacios"),
        ("OTROS", "Otros"),
    ]

    COMPROMETIDO = "COMPROMETIDO"
    EJECUTADO = "EJECUTADO"
    ESTADOS = [
        (COMPROMETIDO, "Comprometido"),
        (EJECUTADO, "Ejecutado"),
    ]

    presupuesto = models.ForeignKey(Presupuesto, on_delete=models.CASCADE, related_name="gastos")
    categoria = models.CharField(max_length=30, choices=CATEGORIAS)
    monto = models.DecimalField(max_digits=12, decimal_places=2)
    # Comprometido = reservado/planeado, aun no pagado. Ejecutado = ya pagado.
    # Default EJECUTADO para que los gastos historicos (previos a este campo)
    # sigan contando como ya-gastados, sin cambiar su significado retroactivo.
    estado = models.CharField(max_length=15, choices=ESTADOS, default=EJECUTADO)
    descripcion = models.TextField(blank=True)
    comprobante = models.FileField(upload_to="comprobantes/", null=True, blank=True)
    registrado_por = models.ForeignKey("usuarios.Usuario", on_delete=models.SET_NULL, null=True, related_name="gastos")
    creado_en = models.DateTimeField(auto_now_add=True)
    ejecutado_en = models.DateTimeField(null=True, blank=True)

    class Meta:
        db_table = "gastos"
        verbose_name = "Gasto"
        verbose_name_plural = "Gastos"

    def __str__(self):
        return f"{self.get_categoria_display()} - ${self.monto} ({self.get_estado_display()})"
