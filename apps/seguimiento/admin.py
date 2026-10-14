from django.contrib import admin

from .models import HistorialEstado, IntegracionAcademica, SeguimientoArchivo, SeguimientoReto


class SeguimientoArchivoInline(admin.TabularInline):
    model = SeguimientoArchivo
    extra = 0
    readonly_fields = ("nombre_original", "tamano", "creado_en")


@admin.register(SeguimientoReto)
class SeguimientoRetoAdmin(admin.ModelAdmin):
    list_display = ("reto", "tipo_sesion", "fecha_sesion", "porcentaje_avance", "creado_por")
    inlines = (SeguimientoArchivoInline,)


@admin.register(IntegracionAcademica)
class IntegracionAcademicaAdmin(admin.ModelAdmin):
    list_display = ("reto", "profesor", "estado", "creado_en")
    list_filter = ("estado",)
    search_fields = ("reto__titulo", "profesor__username", "programa_academico")
    date_hierarchy = "creado_en"


@admin.register(HistorialEstado)
class HistorialEstadoAdmin(admin.ModelAdmin):
    list_display = ("fecha", "content_type", "object_id", "campo", "estado_anterior", "estado_nuevo", "realizado_por")
    list_filter = ("content_type", "campo", "estado_nuevo")
    search_fields = ("comentario", "realizado_por__username", "estado_nuevo", "estado_anterior")
    date_hierarchy = "fecha"
    readonly_fields = [f.name for f in HistorialEstado._meta.fields]

    def has_add_permission(self, request):
        return False

    def has_change_permission(self, request, obj=None):
        return False
