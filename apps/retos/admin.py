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

admin.site.register(Reto, RetoAdmin)
admin.site.register(HistorialEstadoReto)
