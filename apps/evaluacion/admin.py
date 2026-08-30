from django.contrib import admin

from .models import (
    ComentarioEntregable,
    CriterioRubrica,
    Entregable,
    Evaluacion,
    EvaluacionCriterio,
    Rubrica,
)


class CriterioInline(admin.TabularInline):
    model = CriterioRubrica
    extra = 0


@admin.register(Entregable)
class EntregableAdmin(admin.ModelAdmin):
    list_display = ("titulo", "reto", "estudiante", "equipo", "estado", "nota", "es_final", "fecha_entrega")
    list_filter = ("estado", "es_final", "fecha_entrega")
    search_fields = ("titulo", "reto__titulo", "estudiante__username")
    date_hierarchy = "fecha_entrega"


@admin.register(Rubrica)
class RubricaAdmin(admin.ModelAdmin):
    list_display = ("nombre", "reto", "activa", "creada_por")
    list_filter = ("activa",)
    search_fields = ("nombre", "reto__titulo")
    inlines = (CriterioInline,)


@admin.register(Evaluacion)
class EvaluacionAdmin(admin.ModelAdmin):
    list_display = ("entregable", "profesor", "nota", "rubrica", "fecha")
    search_fields = ("entregable__titulo", "profesor__username")
    date_hierarchy = "fecha"


@admin.register(EvaluacionCriterio)
class EvaluacionCriterioAdmin(admin.ModelAdmin):
    list_display = ("evaluacion", "criterio", "puntaje")
    search_fields = ("evaluacion__entregable__titulo", "criterio__nombre")


@admin.register(ComentarioEntregable)
class ComentarioEntregableAdmin(admin.ModelAdmin):
    list_display = ("entregable", "autor", "creado_en")
    search_fields = ("entregable__titulo", "autor__username")
    date_hierarchy = "creado_en"
