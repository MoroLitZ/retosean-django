from django.contrib import messages
from django.contrib.auth.decorators import login_required
from django.shortcuts import get_object_or_404, redirect, render

from .models import UnidadEstudio


def _puede_gestionar(user):
    return user.is_superuser or user.rol == "ADMIN" or user.rol == "PROFESOR"


@login_required(login_url="usuarios:login")
def lista_unidades(request):
    if not _puede_gestionar(request.user):
        messages.error(request, "No tienes permiso para gestionar unidades de estudio.")
        return redirect("usuarios:perfil")

    unidades = UnidadEstudio.objects.select_related("programa__facultad").all()
    return render(request, "unidades_estudio/lista.html", {"unidades": unidades})


@login_required(login_url="usuarios:login")
def crear_unidad(request):
    if not _puede_gestionar(request.user):
        messages.error(request, "No tienes permiso para gestionar unidades de estudio.")
        return redirect("usuarios:perfil")

    if request.method == "POST":
        codigo = request.POST.get("codigo", "").strip()
        nombre = request.POST.get("nombre", "").strip()
        programa_nombre = request.POST.get("programa", "").strip()
        periodo = request.POST.get("periodo", "").strip()
        ciclo = request.POST.get("ciclo", "").strip()
        archivo = request.FILES.get("archivo")

        if not codigo or not nombre or not programa_nombre:
            messages.error(request, "Código, nombre y programa son obligatorios.")
            return render(request, "unidades_estudio/form.html", {"accion": "Crear"})

        from apps.academico.models import Programa

        programa, _ = Programa.objects.get_or_create(
            nombre__iexact=programa_nombre,
            defaults={"nombre": programa_nombre},
        )

        UnidadEstudio.objects.create(
            codigo=codigo,
            nombre=nombre,
            programa=programa,
            periodo=periodo,
            ciclo=ciclo,
            archivo=archivo,
        )
        messages.success(request, f"Unidad de estudio '{codigo} - {nombre}' creada correctamente.")
        return redirect("unidades_estudio:lista")

    return render(request, "unidades_estudio/form.html", {"accion": "Crear"})


@login_required(login_url="usuarios:login")
def editar_unidad(request, pk):
    if not _puede_gestionar(request.user):
        messages.error(request, "No tienes permiso para gestionar unidades de estudio.")
        return redirect("usuarios:perfil")

    unidad = get_object_or_404(UnidadEstudio, pk=pk)

    if request.method == "POST":
        unidad.codigo = request.POST.get("codigo", "").strip()
        unidad.nombre = request.POST.get("nombre", "").strip()
        programa_nombre = request.POST.get("programa", "").strip()
        unidad.periodo = request.POST.get("periodo", "").strip()
        unidad.ciclo = request.POST.get("ciclo", "").strip()
        archivo = request.FILES.get("archivo")

        if not unidad.codigo or not unidad.nombre or not programa_nombre:
            messages.error(request, "Código, nombre y programa son obligatorios.")
            return render(request, "unidades_estudio/form.html", {
                "accion": "Editar",
                "unidad": unidad,
            })

        from apps.academico.models import Programa

        programa, _ = Programa.objects.get_or_create(
            nombre__iexact=programa_nombre,
            defaults={"nombre": programa_nombre},
        )
        unidad.programa = programa
        if archivo:
            unidad.archivo = archivo
        unidad.save()

        messages.success(request, f"Unidad de estudio '{unidad.codigo}' actualizada correctamente.")
        return redirect("unidades_estudio:lista")

    return render(request, "unidades_estudio/form.html", {
        "accion": "Editar",
        "unidad": unidad,
    })


@login_required(login_url="usuarios:login")
def eliminar_unidad(request, pk):
    if not _puede_gestionar(request.user):
        messages.error(request, "No tienes permiso para gestionar unidades de estudio.")
        return redirect("usuarios:perfil")

    unidad = get_object_or_404(UnidadEstudio, pk=pk)
    unidad.activo = False
    unidad.save()
    messages.success(request, f"Unidad de estudio '{unidad.codigo}' desactivada correctamente.")
    return redirect("unidades_estudio:lista")


@login_required(login_url="usuarios:login")
def activar_unidad(request, pk):
    if not _puede_gestionar(request.user):
        messages.error(request, "No tienes permiso.")
        return redirect("usuarios:perfil")

    unidad = get_object_or_404(UnidadEstudio, pk=pk)
    unidad.activo = True
    unidad.save()
    messages.success(request, f"Unidad de estudio '{unidad.codigo}' activada correctamente.")
    return redirect("unidades_estudio:lista")
