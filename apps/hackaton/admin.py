from django.contrib import admin

from .models import EtapaHackaton, Hackathon, InscripcionHackaton, Jurado, VotacionHackaton


class EtapaInline(admin.TabularInline):
    model = EtapaHackaton
    extra = 0


class JuradoInline(admin.TabularInline):
    model = Jurado
    extra = 0


@admin.register(Hackathon)
class HackathonAdmin(admin.ModelAdmin):
    list_display = ("reto", "estado", "resultados_publicados", "creado_en")
    list_filter = ("estado", "resultados_publicados")
    search_fields = ("reto__titulo",)
    inlines = (EtapaInline, JuradoInline)


@admin.register(InscripcionHackaton)
class InscripcionHackatonAdmin(admin.ModelAdmin):
    list_display = ("equipo", "hackathon", "estado", "inscrito_por", "creado_en")
    list_filter = ("estado",)
    search_fields = ("equipo__nombre", "hackathon__reto__titulo")


@admin.register(VotacionHackaton)
class VotacionHackatonAdmin(admin.ModelAdmin):
    list_display = ("hackathon", "jurado", "equipo", "puntaje", "fecha")
    list_filter = ("hackathon",)
    search_fields = ("equipo__nombre", "jurado__nombre")


@admin.register(EtapaHackaton)
class EtapaHackatonAdmin(admin.ModelAdmin):
    list_display = ("titulo", "hackathon", "tipo", "inicia_en", "termina_en", "orden")
    list_filter = ("tipo",)
    search_fields = ("titulo", "hackathon__reto__titulo")


@admin.register(Jurado)
class JuradoAdmin(admin.ModelAdmin):
    list_display = ("nombre", "hackathon", "email", "especialidad")
    search_fields = ("nombre", "email", "hackathon__reto__titulo")
