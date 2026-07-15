from django.contrib import admin

from .models import IntegracionAcademica, SeguimientoArchivo, SeguimientoReto, SesionReto


class SeguimientoArchivoInline(admin.TabularInline):
    model = SeguimientoArchivo
    extra = 0
    readonly_fields = ("nombre_original", "tamano", "creado_en")


@admin.register(SeguimientoReto)
class SeguimientoRetoAdmin(admin.ModelAdmin):
    list_display = ("reto", "tipo_sesion", "fecha_sesion", "porcentaje_avance", "creado_por")
    inlines = (SeguimientoArchivoInline,)


admin.site.register(IntegracionAcademica)
admin.site.register(SesionReto)
