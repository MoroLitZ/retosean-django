from django.contrib import admin

from .models import Notificacion, PreferenciaNotificacion


@admin.register(Notificacion)
class NotificacionAdmin(admin.ModelAdmin):
    list_display = ("titulo", "usuario", "evento", "tipo", "leida", "enviada_por_correo", "creada_en")
    list_filter = ("tipo", "evento", "leida", "enviada_por_correo")
    search_fields = ("titulo", "mensaje", "usuario__username", "usuario__email")
    readonly_fields = ("creada_en", "leida_en")
    date_hierarchy = "creada_en"


@admin.register(PreferenciaNotificacion)
class PreferenciaNotificacionAdmin(admin.ModelAdmin):
    list_display = ("usuario", "recibir_email", "recibir_recordatorios", "actualizado_en")
    list_filter = ("recibir_email", "recibir_recordatorios")
    search_fields = ("usuario__username", "usuario__email")
