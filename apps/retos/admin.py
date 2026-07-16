from django.contrib import admin
from .models import Reto, HistorialEstadoReto

from .models import (
    HistorialEstadoReto,
    IntegracionAcademica,
    Reto,
    RetoArchivo,
    SeguimientoArchivo,
    SeguimientoReto,
)


class SeguimientoInline(admin.TabularInline):
    model = SeguimientoReto
    extra = 0
    readonly_fields = ['creado_en']


class RetoArchivoInline(admin.TabularInline):
    model = RetoArchivo
    extra = 0
    readonly_fields = ['nombre_original', 'tamano', 'creado_en']


class SeguimientoArchivoInline(admin.TabularInline):
    model = SeguimientoArchivo
    extra = 0
    readonly_fields = ['nombre_original', 'tamano', 'creado_en']


class HistorialInline(admin.TabularInline):
    model = HistorialEstadoReto
    extra = 0
    readonly_fields = ['fecha']
    can_delete = False


@admin.register(Reto)
class RetoAdmin(admin.ModelAdmin):
    list_display = ['titulo', 'empresa', 'tipo', 'estado', 'consecutivo', 'actualizado_en']
    list_filter = ['estado', 'tipo', 'area', 'nivel_academico']
    search_fields = ['titulo', 'descripcion', 'empresa__username', 'empresa__email']
    inlines = [RetoArchivoInline, SeguimientoInline, HistorialInline]


@admin.register(IntegracionAcademica)
class IntegracionAcademicaAdmin(admin.ModelAdmin):
    list_display = ['reto', 'profesor', 'programa_academico', 'estado', 'actualizado_en']
    list_filter = ['estado', 'facultad', 'nivel_formacion', 'ecosistema']
    search_fields = ['reto__titulo', 'profesor__username', 'programa_academico']


@admin.register(SeguimientoReto)
class SeguimientoRetoAdmin(admin.ModelAdmin):
    list_display = ['reto', 'tipo_sesion', 'fecha_sesion', 'porcentaje_avance', 'creado_por']
    list_filter = ['tipo_sesion', 'fecha_sesion']
    search_fields = ['reto__titulo', 'avances', 'observaciones', 'acuerdos']
    inlines = [SeguimientoArchivoInline]


@admin.register(RetoArchivo)
class RetoArchivoAdmin(admin.ModelAdmin):
    list_display = ['reto', 'nombre_original', 'tamano', 'creado_en']
    search_fields = ['reto__titulo', 'nombre_original']


@admin.register(SeguimientoArchivo)
class SeguimientoArchivoAdmin(admin.ModelAdmin):
    list_display = ['seguimiento', 'nombre_original', 'tamano', 'creado_en']
    search_fields = ['seguimiento__reto__titulo', 'nombre_original']


@admin.register(HistorialEstadoReto)
class HistorialEstadoRetoAdmin(admin.ModelAdmin):
    list_display = ['reto', 'estado_anterior', 'estado_nuevo', 'realizado_por', 'fecha']
    list_filter = ['estado_nuevo', 'fecha']
    search_fields = ['reto__titulo', 'comentario']
    readonly_fields = ['reto', 'estado_anterior', 'estado_nuevo', 'comentario', 'realizado_por', 'fecha']

    def has_add_permission(self, request):
        return False

    def has_change_permission(self, request, obj=None):
        return False
