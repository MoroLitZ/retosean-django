import os

from django.contrib import messages
from django.core.paginator import Paginator
from django.http import FileResponse, HttpResponse
from django.shortcuts import get_object_or_404, redirect, render

from apps.usuarios.decorators import rol_requerido
from apps.usuarios.roles import es_admin
from apps.usuarios.models import Usuario
from apps.empresas.models import Empresa
from apps.retos.models import Reto

from .catalogo import disponibles_para, obtener
from .forms import ReporteForm
from .models import ReporteGenerado
from .services import construir_reporte

ROLES_CON_REPORTES = ("ADMIN", "PROFESOR")


@rol_requerido(*ROLES_CON_REPORTES)
def constructor(request):
    """Formulario de construccion del reporte (HU06)."""
    if request.method == "POST":
        form = ReporteForm(request.POST, usuario=request.user)
    else:
        # Al cambiar de reporte el formulario se reenvia por GET para recargar
        # la lista de variables, que depende del reporte elegido.
        form = ReporteForm(usuario=request.user, initial=request.GET.dict())

    if request.method == "POST" and form.is_valid():
        definicion = obtener(form.cleaned_data["reporte"])
        if definicion is None:
            messages.error(request, "El reporte seleccionado no existe.")
            return redirect("reportes:constructor")
        if not es_admin(request.user) and request.user.rol not in definicion.roles:
            messages.error(request, "No tienes permiso para generar ese reporte.")
            return redirect("reportes:constructor")

        columnas = definicion.columnas_por_clave(form.cleaned_data.get("columnas"))
        registro, contenido, nombre, tipo_mime = construir_reporte(
            usuario=request.user,
            definicion=definicion,
            columnas=columnas,
            formato=form.cleaned_data["formato"],
            filtros=form.filtros(),
        )
        respuesta = HttpResponse(contenido, content_type=tipo_mime)
        respuesta["Content-Disposition"] = f'attachment; filename="{nombre}"'
        return respuesta

    return render(request, "reportes/constructor.html", {
        "titulo": "Generacion de Informes y Reportes",
        "form": form,
        "definiciones": disponibles_para(request.user),
        "definicion_actual": getattr(form, "definicion_actual", None),
    })


@rol_requerido(*ROLES_CON_REPORTES)
def historial(request):
    """Log de reportes generados: quien, cuando y con que filtros."""
    reportes = ReporteGenerado.objects.select_related("usuario")
    if not es_admin(request.user):
        reportes = reportes.filter(usuario=request.user)

    paginator = Paginator(reportes, 50)
    page_obj = paginator.get_page(request.GET.get("page"))

    filas = []
    for reporte in page_obj:
        filtros = reporte.filtros or {}
        definicion = obtener(filtros.get("reporte", ""))
        filas.append({
            "obj": reporte,
            "nombre": definicion.nombre if definicion else filtros.get("reporte", "Reporte"),
            "registros": filtros.get("total_registros"),
            "columnas": len(filtros.get("columnas") or []),
        })

    return render(request, "reportes/historial.html", {
        "titulo": "Historial de Reportes",
        "filas": filas,
        "page_obj": page_obj,
        "es_admin": es_admin(request.user),
        "total_usuarios": Usuario.objects.count() if es_admin(request.user) else None,
        "total_empresas": Empresa.objects.count() if es_admin(request.user) else None,
        "total_retos": Reto.objects.count() if es_admin(request.user) else None,
    })


@rol_requerido(*ROLES_CON_REPORTES)
def descargar(request, pk):
    reporte = get_object_or_404(ReporteGenerado, pk=pk)
    # Un reporte solo lo descarga quien lo genero, o un administrador.
    if reporte.usuario_id != request.user.pk and not es_admin(request.user):
        messages.error(request, "Ese reporte no te pertenece.")
        return redirect("reportes:historial")
    if not reporte.archivo:
        messages.error(request, "El archivo de ese reporte ya no esta disponible.")
        return redirect("reportes:historial")
    if not os.path.exists(reporte.archivo.path):
        messages.error(request, "El archivo de ese reporte ya no esta disponible.")
        return redirect("reportes:historial")
    return FileResponse(reporte.archivo.open("rb"), as_attachment=True)
