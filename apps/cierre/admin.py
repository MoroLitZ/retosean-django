from django.contrib import admin

from .models import AgendaCierre, CierreReto, EncuestaSatisfaccion, EntregableFinal, Reconocimiento


@admin.register(AgendaCierre)
class AgendaCierreAdmin(admin.ModelAdmin):
    list_display = ("reto", "fecha_hora", "espacio", "actualizado_en")
    list_filter = ("fecha_hora",)
    search_fields = ("reto__titulo", "espacio")
    date_hierarchy = "fecha_hora"


@admin.register(CierreReto)
class CierreRetoAdmin(admin.ModelAdmin):
    list_display = ("reto", "cerrado_por", "cerrado_en", "encuesta_enviada")
    list_filter = ("encuesta_enviada",)
    search_fields = ("reto__titulo", "cerrado_por__username")
    date_hierarchy = "cerrado_en"


@admin.register(EncuestaSatisfaccion)
class EncuestaSatisfaccionAdmin(admin.ModelAdmin):
    list_display = ("cierre", "participante", "calificacion", "respondida_en")
    list_filter = ("respondida_en",)
    search_fields = ("participante__username", "cierre__reto__titulo")


@admin.register(EntregableFinal)
class EntregableFinalAdmin(admin.ModelAdmin):
    list_display = ("nombre", "cierre", "cargado_por", "creado_en")
    search_fields = ("nombre", "cierre__reto__titulo")
    date_hierarchy = "creado_en"


@admin.register(Reconocimiento)
class ReconocimientoAdmin(admin.ModelAdmin):
    list_display = ("nombre_destinatario", "cierre", "usuario", "creado_en")
    search_fields = ("nombre_destinatario", "cierre__reto__titulo")
