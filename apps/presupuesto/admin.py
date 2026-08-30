from django.contrib import admin

from .models import Gasto, Presupuesto


class GastoInline(admin.TabularInline):
    model = Gasto
    extra = 0
    readonly_fields = ("creado_en",)


@admin.register(Presupuesto)
class PresupuestoAdmin(admin.ModelAdmin):
    list_display = ("reto", "monto_total", "monto_ejecutado", "porcentaje_ejecutado", "alerta_80_enviada")
    search_fields = ("reto__titulo",)
    inlines = (GastoInline,)
    readonly_fields = ("creado_en", "actualizado_en")


@admin.register(Gasto)
class GastoAdmin(admin.ModelAdmin):
    list_display = ("presupuesto", "categoria", "monto", "registrado_por", "creado_en")
    list_filter = ("categoria", "creado_en")
    search_fields = ("descripcion", "presupuesto__reto__titulo")
    date_hierarchy = "creado_en"
