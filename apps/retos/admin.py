from django.contrib import admin
from .models import HistorialEstadoReto, Reto, RetoArchivo


class RetoArchivoInline(admin.TabularInline):
    model = RetoArchivo
    extra = 0
    readonly_fields = ("nombre_original", "tamano", "creado_en")


class RetoAdmin(admin.ModelAdmin):
    list_display = ("titulo", "empresa", "tipo", "estado", "consecutivo", "actualizado_en")
    list_filter = ("estado", "tipo", "area")
    search_fields = ("titulo", "descripcion", "empresa__username", "empresa__email")
    inlines = (RetoArchivoInline,)

@admin.register(HistorialEstadoReto)
class HistorialEstadoRetoAdmin(admin.ModelAdmin):
    list_display = ("reto", "estado_anterior", "estado_nuevo", "realizado_por", "fecha")
    list_filter = ("estado_nuevo",)
    search_fields = ("reto__titulo", "realizado_por__username")
    date_hierarchy = "fecha"


admin.site.register(Reto, RetoAdmin)
