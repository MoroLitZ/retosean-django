from django.contrib import admin

from .models import ContactoEmpresa, DocumentoEmpresa, Empresa


class DocumentoInline(admin.TabularInline):
    model = DocumentoEmpresa
    extra = 0
    readonly_fields = ("creado_en",) if hasattr(DocumentoEmpresa, "creado_en") else ()


class ContactoInline(admin.TabularInline):
    model = ContactoEmpresa
    extra = 0


@admin.register(Empresa)
class EmpresaAdmin(admin.ModelAdmin):
    list_display = ("razon_social", "nit", "sector_industrial", "estado_validacion",
                    "estado_listas_restrictivas", "renuncio_a_convenio")
    list_filter = ("estado_validacion", "estado_listas_restrictivas", "renuncio_a_convenio")
    search_fields = ("razon_social", "nit", "usuario__username", "usuario__email")
    inlines = (ContactoInline, DocumentoInline)


@admin.register(DocumentoEmpresa)
class DocumentoEmpresaAdmin(admin.ModelAdmin):
    list_display = ("empresa", "tipo_documento", "estado", "fecha_expedicion", "fecha_vencimiento")
    list_filter = ("tipo_documento", "estado")
    search_fields = ("empresa__razon_social", "empresa__nit")
