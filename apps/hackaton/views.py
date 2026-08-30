from django.contrib import messages
from django.shortcuts import get_object_or_404, redirect, render
from django.views.decorators.http import require_POST

from apps.retos.models import Reto
from apps.usuarios.decorators import rol_requerido, solo_estudiante
from apps.usuarios.roles import es_admin

from . import services
from .forms import (
    EtapaFormSet,
    HackathonForm,
    InscripcionForm,
    JuradoForm,
    VotacionForm,
)
from .models import Hackathon, InscripcionHackaton, Jurado, VotacionHackaton

GESTORES = ("ADMIN", "PROFESOR")


def _puede_gestionar(user, hackathon):
    """Admin siempre; un profesor solo si integro academicamente ese reto."""
    if es_admin(user):
        return True
    return hackathon.reto.integraciones_seguimiento.filter(profesor=user).exists()


def _jurado_de(hackathon, user):
    return Jurado.objects.filter(hackathon=hackathon, usuario=user).first()


def _puede_ver(user, hackathon):
    """Evita que un estudiante abra por URL un hackathon que aun no es publico.

    El resto de roles conserva el acceso que ya tenia; solo el estudiante queda
    filtrado por estado, que es el agujero que cerraba C.5.4.
    """
    if es_admin(user) or user.rol != "ESTUDIANTE":
        return True
    return hackathon.estado in ("publicado", "en_curso", "finalizado")


# ------------------------------------------------------------- LISTADOS
@rol_requerido("ADMIN", "PROFESOR", "EMPRESA", "ESTUDIANTE")
def lista(request):
    hackathones = Hackathon.objects.select_related("reto", "reto__empresa").prefetch_related("etapas")
    if not es_admin(request.user):
        rol = request.user.rol
        if rol == "EMPRESA":
            hackathones = hackathones.filter(reto__empresa=request.user)
        elif rol == "PROFESOR":
            hackathones = hackathones.filter(reto__integraciones_seguimiento__profesor=request.user)
        else:
            hackathones = hackathones.filter(estado__in=["publicado", "en_curso", "finalizado"])
    return render(request, "hackaton/lista.html", {
        "titulo": "Hackathones",
        "hackathones": hackathones.distinct(),
        "puede_crear": es_admin(request.user),
    })


@rol_requerido("ADMIN", "PROFESOR", "EMPRESA", "ESTUDIANTE")
def detalle(request, pk):
    hackathon = get_object_or_404(
        Hackathon.objects.select_related("reto", "reto__empresa").prefetch_related("etapas", "jurados"),
        pk=pk,
    )
    if not _puede_ver(request.user, hackathon):
        messages.error(request, "No tienes acceso a este hackathon.")
        return redirect("hackaton:lista")
    inscripciones = hackathon.inscripciones.select_related("equipo").order_by("estado", "-creado_en")
    jurado = _jurado_de(hackathon, request.user)

    return render(request, "hackaton/detalle.html", {
        "titulo": f"Hackathon: {hackathon.reto.titulo}",
        "hackathon": hackathon,
        "etapas": hackathon.etapas.all(),
        "jurados": hackathon.jurados.all(),
        "inscripciones": inscripciones,
        "puede_gestionar": _puede_gestionar(request.user, hackathon),
        "es_jurado": jurado is not None,
        "inscripciones_abiertas": services.inscripciones_abiertas(hackathon),
        "votacion_abierta": services.votacion_abierta(hackathon),
    })


# --------------------------------------------------------- CONFIGURACION
@rol_requerido(*GESTORES)
def crear(request, reto_id):
    reto = get_object_or_404(Reto, pk=reto_id)
    if reto.tipo != "hackathon":
        messages.error(request, "Solo los retos de tipo Hackathon admiten esta modalidad.")
        return redirect("retos:detalle", pk=reto.pk)
    if hasattr(reto, "hackathon"):
        return redirect("hackaton:detalle", pk=reto.hackathon.pk)

    form = HackathonForm(request.POST or None)
    if request.method == "POST" and form.is_valid():
        hackathon = form.save(commit=False)
        hackathon.reto = reto
        hackathon.save()
        services.crear_etapas_por_defecto(hackathon)
        messages.success(request, "Hackathon creado con sus cuatro etapas base.")
        return redirect("hackaton:etapas", pk=hackathon.pk)

    return render(request, "hackaton/form.html", {
        "titulo": f"Configurar hackathon: {reto.titulo}",
        "form": form,
        "reto": reto,
    })


@rol_requerido(*GESTORES)
def editar(request, pk):
    """Edicion de reglas y estado (en_curso/cancelado incluidos).

    Antes el estado solo se fijaba al crear, asi que los valores `en_curso` y
    `cancelado` que la interfaz pinta eran inalcanzables una vez creado.
    """
    hackathon = get_object_or_404(Hackathon.objects.select_related("reto"), pk=pk)
    if not _puede_gestionar(request.user, hackathon):
        messages.error(request, "No gestionas este hackathon.")
        return redirect("hackaton:detalle", pk=hackathon.pk)

    form = HackathonForm(request.POST or None, instance=hackathon)
    if request.method == "POST" and form.is_valid():
        form.save()
        messages.success(request, "Hackathon actualizado.")
        return redirect("hackaton:detalle", pk=hackathon.pk)

    return render(request, "hackaton/form.html", {
        "titulo": f"Editar hackathon: {hackathon.reto.titulo}",
        "form": form,
        "reto": hackathon.reto,
        "hackathon": hackathon,
    })


@rol_requerido(*GESTORES)
def etapas(request, pk):
    hackathon = get_object_or_404(Hackathon.objects.select_related("reto"), pk=pk)
    if not _puede_gestionar(request.user, hackathon):
        messages.error(request, "No gestionas este hackathon.")
        return redirect("hackaton:detalle", pk=hackathon.pk)

    formset = EtapaFormSet(request.POST or None, instance=hackathon)
    if request.method == "POST" and formset.is_valid():
        formset.save()
        messages.success(request, "Cronograma de etapas actualizado.")
        return redirect("hackaton:detalle", pk=hackathon.pk)

    return render(request, "hackaton/etapas.html", {
        "titulo": f"Etapas: {hackathon.reto.titulo}",
        "hackathon": hackathon,
        "formset": formset,
    })


@rol_requerido(*GESTORES)
def jurados(request, pk):
    hackathon = get_object_or_404(Hackathon.objects.select_related("reto"), pk=pk)
    if not _puede_gestionar(request.user, hackathon):
        messages.error(request, "No gestionas este hackathon.")
        return redirect("hackaton:detalle", pk=hackathon.pk)

    form = JuradoForm(request.POST or None)
    if request.method == "POST" and form.is_valid():
        nuevo = form.save(commit=False)
        nuevo.hackathon = hackathon
        nuevo.nombre = form.cleaned_data["nombre"]
        nuevo.save()
        messages.success(request, f"Jurado {nuevo.nombre} agregado.")
        return redirect("hackaton:jurados", pk=hackathon.pk)

    return render(request, "hackaton/jurados.html", {
        "titulo": f"Jurados: {hackathon.reto.titulo}",
        "hackathon": hackathon,
        "jurados": hackathon.jurados.select_related("usuario"),
        "form": form,
    })


@require_POST
@rol_requerido(*GESTORES)
def eliminar_jurado(request, pk):
    jurado = get_object_or_404(Jurado.objects.select_related("hackathon"), pk=pk)
    if not _puede_gestionar(request.user, jurado.hackathon):
        messages.error(request, "No gestionas este hackathon.")
        return redirect("hackaton:detalle", pk=jurado.hackathon_id)
    hackathon_id = jurado.hackathon_id
    jurado.delete()
    messages.success(request, "Jurado retirado.")
    return redirect("hackaton:jurados", pk=hackathon_id)


@require_POST
@rol_requerido(*GESTORES)
def publicar(request, pk):
    hackathon = get_object_or_404(Hackathon.objects.select_related("reto"), pk=pk)
    if not _puede_gestionar(request.user, hackathon):
        messages.error(request, "No gestionas este hackathon.")
        return redirect("hackaton:detalle", pk=hackathon.pk)
    services.publicar(hackathon, request.user)
    messages.success(request, "Hackathon publicado y participantes notificados.")
    return redirect("hackaton:detalle", pk=hackathon.pk)


@require_POST
@rol_requerido(*GESTORES)
def publicar_resultados(request, pk):
    hackathon = get_object_or_404(Hackathon.objects.select_related("reto"), pk=pk)
    if not _puede_gestionar(request.user, hackathon):
        messages.error(request, "No gestionas este hackathon.")
        return redirect("hackaton:detalle", pk=hackathon.pk)
    tabla = services.publicar_resultados(hackathon, request.user)
    if tabla:
        messages.success(request, f"Resultados publicados. Ganador: {tabla[0]['equipo']}.")
    else:
        messages.warning(request, "Resultados publicados, pero todavia no hay votaciones.")
    return redirect("hackaton:ranking", pk=hackathon.pk)


# --------------------------------------------------------- INSCRIPCIONES
@solo_estudiante
def inscribir_equipo(request, pk):
    hackathon = get_object_or_404(Hackathon.objects.select_related("reto"), pk=pk)
    if not services.inscripciones_abiertas(hackathon):
        messages.error(request, "Las inscripciones a este hackathon no estan abiertas.")
        return redirect("hackaton:detalle", pk=hackathon.pk)

    form = InscripcionForm(request.POST or None, hackathon=hackathon, usuario=request.user)
    if request.method == "POST" and form.is_valid():
        InscripcionHackaton.objects.create(
            hackathon=hackathon,
            equipo=form.cleaned_data["equipo"],
            inscrito_por=request.user,
        )
        messages.success(request, "Equipo inscrito. Queda pendiente de aprobacion.")
        return redirect("hackaton:detalle", pk=hackathon.pk)

    return render(request, "hackaton/inscribir.html", {
        "titulo": f"Inscribir equipo: {hackathon.reto.titulo}",
        "hackathon": hackathon,
        "form": form,
    })


@require_POST
@rol_requerido(*GESTORES)
def gestionar_inscripcion(request, pk):
    inscripcion = get_object_or_404(
        InscripcionHackaton.objects.select_related("hackathon", "equipo"), pk=pk
    )
    if not _puede_gestionar(request.user, inscripcion.hackathon):
        messages.error(request, "No gestionas este hackathon.")
        return redirect("hackaton:detalle", pk=inscripcion.hackathon_id)

    accion = request.POST.get("accion")
    if accion in {"aceptar", "rechazar"}:
        inscripcion.estado = "ACEPTADA" if accion == "aceptar" else "RECHAZADA"
        inscripcion.save(update_fields=["estado"])
        messages.success(request, f"Inscripcion de {inscripcion.equipo.nombre} actualizada.")
    return redirect("hackaton:detalle", pk=inscripcion.hackathon_id)


# --------------------------------------------------------------- JURADO
@rol_requerido("ADMIN", "PROFESOR", "EMPRESA")
def votar(request, pk):
    hackathon = get_object_or_404(Hackathon.objects.select_related("reto"), pk=pk)
    jurado = _jurado_de(hackathon, request.user)
    if jurado is None:
        messages.error(request, "No estas registrado como jurado de este hackathon.")
        return redirect("hackaton:detalle", pk=hackathon.pk)
    if not services.votacion_abierta(hackathon):
        messages.error(request, "La etapa de presentacion no esta abierta.")
        return redirect("hackaton:detalle", pk=hackathon.pk)

    if request.method == "POST":
        equipo_id = request.POST.get("equipo")
        try:
            equipo_id = int(equipo_id)
        except (TypeError, ValueError):
            messages.error(request, "Selecciona un equipo valido.")
            return redirect("hackaton:votar", pk=hackathon.pk)
        inscripcion = get_object_or_404(
            InscripcionHackaton, hackathon=hackathon, equipo_id=equipo_id, estado="ACEPTADA"
        )
        form = VotacionForm(request.POST)
        if form.is_valid():
            VotacionHackaton.objects.update_or_create(
                hackathon=hackathon,
                jurado=jurado,
                equipo=inscripcion.equipo,
                defaults={
                    "puntaje": form.cleaned_data["puntaje"],
                    "comentario": form.cleaned_data["comentario"],
                },
            )
            messages.success(request, f"Voto registrado para {inscripcion.equipo.nombre}.")
            return redirect("hackaton:votar", pk=hackathon.pk)
        messages.error(request, "Revisa el puntaje: debe estar entre 0 y 5.")

    mis_votos = {
        voto.equipo_id: voto
        for voto in VotacionHackaton.objects.filter(hackathon=hackathon, jurado=jurado)
    }
    inscripciones = hackathon.inscripciones.filter(estado="ACEPTADA").select_related("equipo")

    return render(request, "hackaton/votar.html", {
        "titulo": f"Votacion: {hackathon.reto.titulo}",
        "hackathon": hackathon,
        "jurado": jurado,
        "form": VotacionForm(),
        "equipos": [
            {"inscripcion": i, "voto": mis_votos.get(i.equipo_id)} for i in inscripciones
        ],
        "pendientes": sum(1 for i in inscripciones if i.equipo_id not in mis_votos),
    })


# -------------------------------------------------------------- RANKING
@rol_requerido("ADMIN", "PROFESOR", "EMPRESA", "ESTUDIANTE")
def ranking(request, pk):
    hackathon = get_object_or_404(Hackathon.objects.select_related("reto"), pk=pk)
    puede_ver = (
        hackathon.resultados_publicados
        or _puede_gestionar(request.user, hackathon)
        or _jurado_de(hackathon, request.user) is not None
    )
    return render(request, "hackaton/ranking.html", {
        "titulo": f"Resultados: {hackathon.reto.titulo}",
        "hackathon": hackathon,
        "puede_ver": puede_ver,
        "tabla": services.ranking(hackathon) if puede_ver else [],
        "puede_gestionar": _puede_gestionar(request.user, hackathon),
    })
