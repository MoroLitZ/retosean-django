from django.contrib import admin

from .models import Certificado, Ecosistema, Estudiante, Facultad, Profesor, Programa


class ProgramaInline(admin.TabularInline):
    model = Programa
    extra = 0


@admin.register(Facultad)
class FacultadAdmin(admin.ModelAdmin):
    list_display = ("nombre", "codigo")
    search_fields = ("nombre", "codigo")
    inlines = (ProgramaInline,)


@admin.register(Programa)
class ProgramaAdmin(admin.ModelAdmin):
    list_display = ("nombre", "facultad", "nivel")
    list_filter = ("facultad", "nivel")
    search_fields = ("nombre",)


@admin.register(Certificado)
class CertificadoAdmin(admin.ModelAdmin):
    list_display = ("usuario", "reto", "rol_participacion", "codigo_corto", "emitido_en")
    list_filter = ("rol_participacion", "emitido_en")
    search_fields = ("usuario__username", "usuario__email", "reto__titulo", "codigo_verificacion")
    readonly_fields = ("codigo_verificacion", "emitido_en")
    date_hierarchy = "emitido_en"


@admin.register(Ecosistema)
class EcosistemaAdmin(admin.ModelAdmin):
    list_display = ("nombre",)
    search_fields = ("nombre", "descripcion")


@admin.register(Profesor)
class ProfesorAdmin(admin.ModelAdmin):
    list_display = ("usuario", "facultad", "especialidad")
    list_filter = ("facultad",)
    search_fields = ("usuario__username", "usuario__email", "especialidad")


@admin.register(Estudiante)
class EstudianteAdmin(admin.ModelAdmin):
    list_display = ("usuario", "programa", "semestre")
    list_filter = ("programa", "semestre")
    search_fields = ("usuario__username", "usuario__email")
