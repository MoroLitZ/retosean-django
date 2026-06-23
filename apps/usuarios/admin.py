from django.contrib import admin
from django.contrib.auth.admin import UserAdmin
from .models import Usuario

# añadimos una configuración propia para el admin para no usar la de django por defecto
try:
    admin.site.unregister(Usuario)
except admin.sites.NotRegistered:
    pass

@admin.register(Usuario)
class UsuarioAdmin(UserAdmin):
    list_display = ['username', 'email', 'first_name', 'last_name', 'rol', 'is_staff']
    list_filter = ['rol', 'is_staff', 'is_active']
    
    fieldsets = UserAdmin.fieldsets + (
        ('Información de Roles de la EAN', {
            'fields': ('rol', 'telefono', 'activo'),
        }),
    )