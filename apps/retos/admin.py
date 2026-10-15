from django.contrib import admin
from .models import Reto, RetoArchivo


class RetoArchivoInline(admin.TabularInline):
    model = RetoArchivo
    extra = 0
    readonly_fields = ("nombre_original", "tamano", "creado_en")


class RetoAdmin(admin.ModelAdmin):
    list_display = ("titulo", "empresa", "tipo", "estado", "codigo", "eliminado_en", "actualizado_en")
    list_filter = ("estado", "tipo", "area")
    search_fields = ("titulo", "descripcion", "empresa__username", "empresa__email")
    inlines = (RetoArchivoInline,)

    def get_queryset(self, request):
        # El admin ve tambien los retos eliminados logicamente.
        return Reto.todos.all()


admin.site.register(Reto, RetoAdmin)
