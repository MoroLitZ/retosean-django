from django.contrib import admin

from .models import IntegracionAcademica, SeguimientoArchivo, SeguimientoReto


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
