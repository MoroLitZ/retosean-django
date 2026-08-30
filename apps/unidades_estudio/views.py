from django.contrib import messages
from django.contrib.auth.decorators import login_required
from django.shortcuts import get_object_or_404, redirect, render
from django.views.decorators.http import require_POST

from .models import UnidadEstudio


def _puede_gestionar(user):
    return user.is_superuser or user.rol == "ADMIN" or user.rol == "PROFESOR"


@login_required(login_url="usuarios:login")
def lista_unidades(request):
    if not _puede_gestionar(request.user):
        messages.error(request, "No tienes permiso para gestionar unidades de estudio.")
        return redirect("usuarios:perfil")

    from apps.academico.models import Programa

    programas = Programa.objects.filter(unidades_estudio__isnull=False).order_by("nombre")

    programa_id = request.GET.get("programa", "").strip()
    filtro_activo = request.GET.get("activo", "1")
    unidades = UnidadEstudio.objects.select_related("programa__facultad")

    # El borrado logico no cerraba: las unidades desactivadas seguian listandose.
    # Por defecto solo se muestran las activas; "activo=0" revela las inactivas.
    if filtro_activo == "1":
        unidades = unidades.filter(activo=True)

    if programa_id:
        unidades = unidades.filter(programa_id=programa_id)

    return render(request, "unidades_estudio/lista.html", {
        "unidades": unidades,
        "programas": programas,
        "programa_id": programa_id,
        "filtro_activo": filtro_activo,
    })


@login_required(login_url="usuarios:login")
def crear_unidad(request):
    if not _puede_gestionar(request.user):
        messages.error(request, "No tienes permiso para gestionar unidades de estudio.")
        return redirect("usuarios:perfil")

    errores = {}

    if request.method == "POST" and request.POST.get("accion") == "cargar_csv":
        # El formulario ofrecia esta carga masiva pero la vista nunca la leia.
        return _procesar_csv(request)

    if request.method == "POST":
        codigo = request.POST.get("codigo", "").strip()
        nombre = request.POST.get("nombre", "").strip()
        programa_id = request.POST.get("programa", "").strip()
        periodo = request.POST.get("periodo", "").strip()
        ciclo = request.POST.get("ciclo", "").strip()
        archivo = request.FILES.get("archivo")

        if not codigo:
            errores["codigo"] = "El código es obligatorio."
        if not nombre:
            errores["nombre"] = "El nombre es obligatorio."

        programa = _resolver_programa(programa_id)
        if not programa:
            errores["programa"] = "El programa académico es obligatorio."

        if UnidadEstudio.objects.filter(codigo=codigo).exists():
            errores["codigo"] = "Ese código ya está vinculado a otra materia."

        if not errores:
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
        "programas": _programas_contexto(),
        "programa_seleccionado": _entero(request.POST.get("programa")),
        "unidad": type("obj", (), {
            "codigo": request.POST.get("codigo", ""),
            "nombre": request.POST.get("nombre", ""),
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
        programa_id = request.POST.get("programa", "").strip()
        periodo = request.POST.get("periodo", "").strip()
        ciclo = request.POST.get("ciclo", "").strip()
        archivo = request.FILES.get("archivo")

        errores = {}
        if not codigo_nuevo:
            errores["codigo"] = "El código es obligatorio."
        if not nombre:
            errores["nombre"] = "El nombre es obligatorio."

        programa = _resolver_programa(programa_id)
        if not programa:
            errores["programa"] = "El programa académico es obligatorio."

        if not errores:
            if codigo_nuevo != unidad.codigo and UnidadEstudio.objects.filter(codigo=codigo_nuevo).exists():
                errores["codigo"] = "Ese código ya está vinculado a otra materia."

        if errores:
            for msg in errores.values():
                messages.error(request, msg)
            return render(request, "unidades_estudio/form.html", {
                "accion": "Editar",
                "programas": _programas_contexto(),
                "programa_seleccionado": _entero(programa_id),
                "unidad": type("obj", (), {
                    "codigo": codigo_nuevo,
                    "nombre": nombre,
                    "periodo": periodo,
                    "ciclo": ciclo,
                    "archivo": unidad.archivo,
                })(),
            })

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
        "programas": _programas_contexto(),
        "programa_seleccionado": unidad.programa_id,
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


CSV_COLUMNAS_OBLIGATORIAS = ["codigo", "nombre", "programa"]


def _programas_contexto():
    from apps.academico.models import Programa

    return Programa.objects.select_related("facultad").order_by("nombre")


def _resolver_programa(programa_id):
    """Devuelve el Programa por su id, o None si no es valido."""
    from apps.academico.models import Programa

    if not programa_id:
        return None
    try:
        return Programa.objects.get(pk=int(programa_id))
    except (ValueError, TypeError, Programa.DoesNotExist):
        return None


def _entero(valor):
    try:
        return int(valor)
    except (TypeError, ValueError):
        return None


def _buscar_programa_por_nombre(nombre):
    """Busca un programa por nombre exacto, sin crear filas nuevas."""
    from apps.academico.models import Programa

    return Programa.objects.filter(nombre__iexact=nombre.strip()).first()


def _procesar_csv(request):
    """Crea varias unidades de estudio a partir de un CSV."""
    import csv
    import io

    archivo = request.FILES.get("archivo_csv")
    if archivo is None:
        messages.error(request, "Selecciona un archivo CSV.")
        return redirect("unidades_estudio:crear")

    contenido = archivo.read()
    texto = None
    for codificacion in ("utf-8-sig", "latin-1"):
        try:
            texto = contenido.decode(codificacion)
            break
        except UnicodeDecodeError:
            continue
    if texto is None:
        messages.error(request, "No se pudo leer el archivo: codificacion no reconocida.")
        return redirect("unidades_estudio:crear")

    try:
        dialecto = csv.Sniffer().sniff(texto[:2048], delimiters=",;	")
    except csv.Error:
        dialecto = csv.excel

    lector = csv.DictReader(io.StringIO(texto), dialect=dialecto)
    cabeceras = [(c or "").strip().lower() for c in (lector.fieldnames or [])]
    faltantes = [c for c in CSV_COLUMNAS_OBLIGATORIAS if c not in cabeceras]
    if faltantes:
        messages.error(
            request,
            "Al CSV le faltan estas columnas obligatorias: " + ", ".join(faltantes) + ".",
        )
        return redirect("unidades_estudio:crear")

    cache_programas = {}
    creadas, problemas = 0, []

    for numero, fila in enumerate(lector, start=2):
        datos = {(k or "").strip().lower(): (v or "").strip() for k, v in fila.items()}
        codigo = datos.get("codigo", "")
        nombre = datos.get("nombre", "")
        programa_nombre = datos.get("programa", "")

        if not (codigo and nombre and programa_nombre):
            problemas.append(f"fila {numero}: faltan codigo, nombre o programa")
            continue
        if UnidadEstudio.objects.filter(codigo=codigo).exists():
            problemas.append(f"fila {numero}: el codigo {codigo} ya existe")
            continue

        clave = programa_nombre.lower()
        programa = cache_programas.get(clave)
        if programa is None:
            programa = _buscar_programa_por_nombre(programa_nombre)
            if programa is None:
                problemas.append(
                    f"fila {numero}: el programa '{programa_nombre}' no existe en el catálogo"
                )
                continue
            cache_programas[clave] = programa

        UnidadEstudio.objects.create(
            codigo=codigo,
            nombre=nombre,
            programa=programa,
            periodo=datos.get("periodo", ""),
            ciclo=datos.get("ciclo", ""),
        )
        creadas += 1

    if creadas:
        messages.success(request, f"{creadas} unidad(es) de estudio creada(s) desde el CSV.")
    if problemas:
        detalle = "; ".join(problemas[:8]) + ("; ..." if len(problemas) > 8 else "")
        messages.warning(request, f"{len(problemas)} fila(s) omitida(s): {detalle}")
    if not creadas and not problemas:
        messages.warning(request, "El CSV no contenia filas de datos.")

    return redirect("unidades_estudio:lista")
