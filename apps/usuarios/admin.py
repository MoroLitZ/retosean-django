from django.contrib import admin
from django.contrib.auth.admin import UserAdmin as BaseUserAdmin

from .models import LogActividad, Usuario


@admin.register(Usuario)
class UsuarioAdmin(BaseUserAdmin):
    list_display = ("username", "email", "first_name", "last_name", "rol", "is_active", "date_joined")
    list_filter = ("rol", "is_active", "is_staff", "is_superuser")
    search_fields = ("username", "email", "first_name", "last_name")
    ordering = ("-date_joined",)

    # Anadimos los campos propios del proyecto a los fieldsets de Django.
    fieldsets = BaseUserAdmin.fieldsets + (
        ("Datos de RetosEAN", {"fields": ("rol", "telefono", "activo")}),
    )
    add_fieldsets = BaseUserAdmin.add_fieldsets + (
        ("Datos de RetosEAN", {"fields": ("email", "rol", "telefono")}),
    )


@admin.register(LogActividad)
class LogActividadAdmin(admin.ModelAdmin):
    list_display = ("creado_en", "accion", "usuario", "identificador", "realizado_por", "ip")
    list_filter = ("accion", "creado_en")
    search_fields = ("identificador", "usuario__username", "detalle")
    date_hierarchy = "creado_en"
    readonly_fields = [campo.name for campo in LogActividad._meta.fields]

    def has_add_permission(self, request):
        return False
