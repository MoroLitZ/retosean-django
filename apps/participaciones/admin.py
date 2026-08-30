from django.contrib import admin

from .models import Equipo, MiembroEquipo, Postulacion, RetoFavorito


class MiembroInline(admin.TabularInline):
    model = MiembroEquipo
    extra = 0


@admin.register(Postulacion)
class PostulacionAdmin(admin.ModelAdmin):
    list_display = ("estudiante", "reto", "estado", "programa", "semestre", "fecha_postulacion")
    list_filter = ("estado", "fecha_postulacion")
    search_fields = ("estudiante__username", "estudiante__email", "reto__titulo")
    date_hierarchy = "fecha_postulacion"


@admin.register(Equipo)
class EquipoAdmin(admin.ModelAdmin):
    list_display = ("nombre", "reto", "creado_en")
    search_fields = ("nombre", "reto__titulo")
    inlines = (MiembroInline,)


@admin.register(RetoFavorito)
class RetoFavoritoAdmin(admin.ModelAdmin):
    list_display = ("usuario", "reto", "fecha")
    search_fields = ("usuario__username", "reto__titulo")
