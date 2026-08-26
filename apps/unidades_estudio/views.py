from django.contrib import messages
from django.contrib.auth.decorators import login_required
from django.shortcuts import get_object_or_404, redirect, render
from django.views.decorators.http import require_POST

from .models import UnidadEstudio
from .tasks import enviar_correo_nueva_unidad

def _puede_gestionar(user):
    return user.is_superuser or user.rol == "ADMIN" or user.rol == "PROFESOR"


@login_required(login_url="usuarios:login")
def lista_unidades(request):
    if not _puede_gestionar(request.user):
        messages.error(request, "No tienes permiso para gestionar unidades de estudio.")
        return redirect("usuarios:perfil")

    from apps.academico.models import Programa

    # Programas unicos por nombre normalizado (sin duplicados por acentos/tildes)
    programas = []
    vistos = set()
    for p in Programa.objects.filter(unidades_estudio__isnull=False).order_by("nombre"):
        key = p.nombre.lower().replace("í", "i").replace("ó", "o").replace("é", "e").replace("á", "a").replace("ú", "u")
        if key not in vistos:
            vistos.add(key)
            programas.append(p)

    programa_id = request.GET.get("programa", "").strip()
    unidades = UnidadEstudio.objects.select_related("programa__facultad")

    if programa_id:
        unidades = unidades.filter(programa_id=programa_id)

    return render(request, "unidades_estudio/lista.html", {
        "unidades": unidades,
        "programas": programas,
        "programa_id": programa_id,
    })


@login_required(login_url="usuarios:login")
def crear_unidad(request):
    if not _puede_gestionar(request.user):
        messages.error(request, "No tienes permiso para gestionar unidades de estudio.")
        return redirect("usuarios:perfil")

    errores = {}

    if request.method == "POST":
        codigo = request.POST.get("codigo", "").strip()
        nombre = request.POST.get("nombre", "").strip()
        programa_nombre = request.POST.get("programa", "").strip()
        periodo = request.POST.get("periodo", "").strip()
        ciclo = request.POST.get("ciclo", "").strip()
        archivo = request.FILES.get("archivo")

        if not codigo:
            errores["codigo"] = "El código es obligatorio."
        if not nombre:
            errores["nombre"] = "El nombre es obligatorio."
        if not programa_nombre:
            errores["programa"] = "El programa académico es obligatorio."

        if UnidadEstudio.objects.filter(codigo=codigo).exists():
            errores["codigo"] = "Ese código ya está vinculado a otra materia."

        if not errores:
            from apps.academico.models import Programa, Facultad

            facultad, _ = Facultad.objects.get_or_create(
                nombre="Facultad de Ingeniería y Ciencias Básicas",
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
            
            enviar_correo_nueva_unidad.delay(codigo, nombre)
            
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
@require_POST
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
@require_POST
def activar_unidad(request, pk):
    if not _puede_gestionar(request.user):
        messages.error(request, "No tienes permiso.")
        return redirect("usuarios:perfil")

    unidad = get_object_or_404(UnidadEstudio, pk=pk)
    unidad.activo = True
    unidad.save()
    messages.success(request, f"Unidad de estudio '{unidad.codigo}' activada correctamente.")
    return redirect("unidades_estudio:lista")
