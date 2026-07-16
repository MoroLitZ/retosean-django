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

    # --- CARGA MASIVA POR CSV ---
    if request.method == "POST" and request.POST.get("accion") == "cargar_csv":
        archivo_csv = request.FILES.get("archivo_csv")
        if not archivo_csv:
            messages.error(request, "Debes seleccionar un archivo CSV.")
            return render(request, "unidades_estudio/form.html", {"accion": "Crear"})

        if not archivo_csv.name.endswith(".csv"):
            messages.error(request, "El archivo debe ser CSV.")
            return render(request, "unidades_estudio/form.html", {"accion": "Crear"})

        import csv, io
        from apps.academico.models import Programa, Facultad

        facultad, _ = Facultad.objects.get_or_create(
            nombre="Facultad de Ingenieria y Ciencias Basicas",
            defaults={"codigo": "FICB"},
        )

        decoded = archivo_csv.read().decode("utf-8-sig")
        lector = csv.DictReader(io.StringIO(decoded))
        campos_requeridos = {"codigo", "nombre", "programa"}

        if not campos_requeridos.issubset(lector.fieldnames or []):
            messages.error(request, "El CSV debe tener las columnas: codigo, nombre, programa (periodo y ciclo son opcionales).")
            return render(request, "unidades_estudio/form.html", {"accion": "Crear"})

        creadas = 0
        errores_csv = []

        for fila, linea in enumerate(lector, start=2):
            codigo = linea.get("codigo", "").strip()
            nombre = linea.get("nombre", "").strip()
            programa_nombre = linea.get("programa", "").strip()
            periodo = linea.get("periodo", "").strip()
            ciclo = linea.get("ciclo", "").strip()

            if not codigo or not nombre or not programa_nombre:
                errores_csv.append(f"Linea {fila}: faltan campos obligatorios (codigo, nombre, programa).")
                continue

            if UnidadEstudio.objects.filter(codigo=codigo).exists():
                errores_csv.append(f"Linea {fila}: el codigo '{codigo}' ya existe.")
                continue

            programa_obj, _ = Programa.objects.get_or_create(
                nombre__iexact=programa_nombre,
                defaults={"nombre": programa_nombre, "facultad": facultad},
            )

            UnidadEstudio.objects.create(
                codigo=codigo,
                nombre=nombre,
                programa=programa_obj,
                periodo=periodo,
                ciclo=ciclo,
            )
            creadas += 1

        if creadas:
            messages.success(request, f"Se crearon {creadas} unidad(es) de estudio desde el CSV.")

        for err in errores_csv:
            messages.warning(request, err)

        return redirect("unidades_estudio:lista")

    # --- FORMULARIO INDIVIDUAL ---
    errores = {}

    if request.method == "POST":
        codigo = request.POST.get("codigo", "").strip()
        nombre = request.POST.get("nombre", "").strip()
        programa_nombre = request.POST.get("programa", "").strip()
        periodo = request.POST.get("periodo", "").strip()
        ciclo = request.POST.get("ciclo", "").strip()
        archivo = request.FILES.get("archivo")

        if not codigo:
            errores["codigo"] = "El codigo es obligatorio."
        if not nombre:
            errores["nombre"] = "El nombre es obligatorio."
        if not programa_nombre:
            errores["programa"] = "El programa academico es obligatorio."

        if UnidadEstudio.objects.filter(codigo=codigo).exists():
            errores["codigo"] = "Ese codigo ya esta vinculado a otra materia."

        if not errores:
            from apps.academico.models import Programa, Facultad

            facultad, _ = Facultad.objects.get_or_create(
                nombre="Facultad de Ingenieria y Ciencias Basicas",
                defaults={"codigo": "FICB"},
            )

            programa, _ = Programa.objects.get_or_create(
                nombre__iexact=programa_nombre,
                defaults={"nombre": programa_nombre, "facultad": facultad},
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

        for msg in errores.values():
            messages.error(request, msg)

    return render(request, "unidades_estudio/form.html", {
        "accion": "Crear",
        "unidad": type("obj", (), {
            "codigo": request.POST.get("codigo", ""),
            "nombre": request.POST.get("nombre", ""),
            "programa": type("obj", (), {"nombre": request.POST.get("programa", "")})(),
            "periodo": request.POST.get("periodo", ""),
            "ciclo": request.POST.get("ciclo", ""),
            "archivo": None,
        })(),
    })


@login_required(login_url="usuarios:login")
def editar_unidad(request, pk):
    if not _puede_gestionar(request.user):
        messages.error(request, "No tienes permiso para gestionar unidades de estudio.")
        return redirect("usuarios:perfil")

    unidad = get_object_or_404(UnidadEstudio, pk=pk)

    if request.method == "POST":
        codigo_nuevo = request.POST.get("codigo", "").strip()
        nombre = request.POST.get("nombre", "").strip()
        programa_nombre = request.POST.get("programa", "").strip()
        periodo = request.POST.get("periodo", "").strip()
        ciclo = request.POST.get("ciclo", "").strip()
        archivo = request.FILES.get("archivo")

        errores = {}
        if not codigo_nuevo:
            errores["codigo"] = "El código es obligatorio."
        if not nombre:
            errores["nombre"] = "El nombre es obligatorio."
        if not programa_nombre:
            errores["programa"] = "El programa académico es obligatorio."

        if not errores:
            if codigo_nuevo != unidad.codigo and UnidadEstudio.objects.filter(codigo=codigo_nuevo).exists():
                errores["codigo"] = "Ese código ya está vinculado a otra materia."

        if errores:
            for msg in errores.values():
                messages.error(request, msg)
            return render(request, "unidades_estudio/form.html", {
                "accion": "Editar",
                "unidad": type("obj", (), {
                    "codigo": codigo_nuevo,
                    "nombre": nombre,
                    "programa": type("obj", (), {"nombre": programa_nombre})(),
                    "periodo": periodo,
                    "ciclo": ciclo,
                    "archivo": unidad.archivo,
                })(),
            })

        from apps.academico.models import Programa, Facultad

        facultad, _ = Facultad.objects.get_or_create(
            nombre="Facultad de Ingeniería y Ciencias Básicas",
            defaults={"codigo": "FICB"},
        )

        programa, _ = Programa.objects.get_or_create(
            nombre__iexact=programa_nombre,
            defaults={"nombre": programa_nombre, "facultad": facultad},
        )

        unidad.codigo = codigo_nuevo
        unidad.nombre = nombre
        unidad.programa = programa
        unidad.periodo = periodo
        unidad.ciclo = ciclo
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
