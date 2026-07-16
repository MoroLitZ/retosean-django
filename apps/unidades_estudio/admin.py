from django.contrib import admin

from .models import UnidadEstudio


@admin.register(UnidadEstudio)
class UnidadEstudioAdmin(admin.ModelAdmin):
    list_display = ["codigo", "nombre", "programa", "periodo", "ciclo", "activo", "actualizado_en"]
    list_filter = ["activo", "programa", "periodo", "ciclo"]
    search_fields = ["codigo", "nombre", "programa__nombre"]
    ordering = ["programa", "ciclo", "nombre"]
    fieldsets = [
        ("Identificación", {"fields": ["codigo", "nombre", "programa"]}),
        ("Clasificación", {"fields": ["periodo", "ciclo"]}),
        ("Archivo", {"fields": ["archivo"]}),
        ("Estado", {"fields": ["activo"]}),
    ]
