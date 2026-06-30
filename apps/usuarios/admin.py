from django.contrib import admin
from django.contrib.auth.admin import UserAdmin
from .models import Usuario, DocumentoEmpresa, Empresa

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


@admin.register(Empresa)
class EmpresaAdmin(admin.ModelAdmin):
    list_display = ('nit', 'razon_social', 'sector_industrial', 'fecha_creacion')
    search_fields = ('nit', 'razon_social')


@admin.register(DocumentoEmpresa)
class DocumentoEmpresaAdmin(admin.ModelAdmin):
    list_display = ('empresa', 'tipo_documento', 'estado', 'fecha_expedicion', 'fecha_carga')
    list_filter = ('estado', 'tipo_documento', 'fecha_carga')
    search_fields = ('empresa__razon_social', 'empresa__usuario__email')    
    list_editable = ('estado',)