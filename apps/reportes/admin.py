from django.contrib import admin

from .models import ReporteGenerado


@admin.register(ReporteGenerado)
class ReporteGeneradoAdmin(admin.ModelAdmin):
    list_display = ("__str__", "usuario", "formato", "creado_en")
    list_filter = ("formato", "creado_en")
    search_fields = ("usuario__username", "usuario__email")
    readonly_fields = ("creado_en", "filtros")
    date_hierarchy = "creado_en"
