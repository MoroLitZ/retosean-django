from django.contrib import messages
from django.db import transaction
from django.urls import reverse
from django.shortcuts import get_object_or_404, redirect, render
from django.views.decorators.http import require_POST
from django.utils import timezone

from apps.academico.services import emitir_certificados_de_reto
from apps.notificaciones.services import notificar_muchos
from apps.participaciones.models import MiembroEquipo, Postulacion
from apps.retos.models import Reto
from apps.retos.services import cambiar_estado_reto
from apps.usuarios.decorators import rol_requerido
from apps.seguimiento.models import IntegracionAcademica

from .forms import AgendaCierreForm, CierreRetoForm, EncuestaSatisfaccionForm, EntregableFinalForm, ReconocimientoForm
from .models import AgendaCierre, CierreReto, EncuestaSatisfaccion


def _participantes(reto):
    ids = {reto.empresa_id}
    ids.update(IntegracionAcademica.objects.filter(reto=reto).values_list("profesor_id", flat=True))
    ids.update(Postulacion.objects.filter(reto=reto, estado="ACEPTADA").values_list("estudiante_id", flat=True))
    ids.update(MiembroEquipo.objects.filter(equipo__reto=reto).values_list("estudiante_id", flat=True))
    return {pk for pk in ids if pk}


@rol_requerido("ADMIN", raise_exception=True)
def panel(request):
    retos = Reto.objects.select_related("empresa").filter(estado__in=["aprobado", "en_curso", "pausado", "finalizado"])
    return render(request, "cierre/panel.html", {"retos": retos})


@rol_requerido("ADMIN", raise_exception=True)
def gestionar(request, reto_id):
    reto = get_object_or_404(
        Reto, pk=reto_id, estado__in=["aprobado", "en_curso", "pausado", "finalizado"]
    )
    agenda, _ = AgendaCierre.objects.get_or_create(reto=reto)
    cierre, _ = CierreReto.objects.get_or_create(reto=reto, defaults={"cerrado_por": request.user})
    agenda_form = AgendaCierreForm(request.POST or None, instance=agenda, prefix="agenda")
    cierre_form = CierreRetoForm(request.POST or None, request.FILES or None, instance=cierre, prefix="cierre")
    if request.method == "POST" and request.POST.get("accion") == "guardar":
        if agenda_form.is_valid() and cierre_form.is_valid():
            agenda_form.save()
            registro = cierre_form.save(commit=False)
            registro.cerrado_por = request.user
            registro.save()
            messages.success(request, "Documentacion y agenda guardadas.")
            return redirect("cierre:gestionar", reto_id=reto.pk)
        messages.error(request, "No se pudo guardar. Revisa los datos de la agenda y del acta.")
    return render(request, "cierre/gestionar.html", {
        "reto": reto, "agenda_form": agenda_form, "cierre_form": cierre_form, "cierre": cierre,
        "entregable_form": EntregableFinalForm(), "reconocimiento_form": ReconocimientoForm(),
    })


@require_POST
@rol_requerido("ADMIN", raise_exception=True)
def agregar_entregable(request, reto_id):
    cierre = get_object_or_404(CierreReto, reto_id=reto_id)
    form = EntregableFinalForm(request.POST, request.FILES)
    if form.is_valid():
        item = form.save(commit=False)
        item.cierre = cierre
        item.cargado_por = request.user
        item.save()
        messages.success(request, "Entregable final cargado.")
    else:
        messages.error(request, "No fue posible cargar el entregable final.")
    return redirect("cierre:gestionar", reto_id=reto_id)


@require_POST
@rol_requerido("ADMIN", raise_exception=True)
def agregar_reconocimiento(request, reto_id):
    cierre = get_object_or_404(CierreReto, reto_id=reto_id)
    form = ReconocimientoForm(request.POST)
    if form.is_valid():
        reconocimiento = form.save(commit=False)
        reconocimiento.cierre = cierre
        if reconocimiento.usuario and (
            reconocimiento.usuario.rol != "ESTUDIANTE"
            or reconocimiento.usuario_id not in _participantes(cierre.reto)
        ):
            messages.error(request, "El reconocimiento debe asignarse a un estudiante participante del reto.")
        else:
            reconocimiento.save()
            messages.success(request, "Reconocimiento registrado.")
    else:
        messages.error(request, "Revisa los datos del reconocimiento.")
    return redirect("cierre:gestionar", reto_id=reto_id)


@rol_requerido("ADMIN", raise_exception=True)
@require_POST
@transaction.atomic
def finalizar(request, reto_id):
    reto = get_object_or_404(
        Reto.objects.select_for_update(), pk=reto_id,
        estado__in=["aprobado", "en_curso", "pausado", "finalizado"],
    )
    cierre = get_object_or_404(CierreReto, reto=reto)
    agenda = getattr(reto, "agenda_cierre", None)
    faltantes = []
    if not agenda or not agenda.fecha_hora or not agenda.espacio or not agenda.agenda:
        faltantes.append("agenda, fecha y espacio")
    if not cierre.acta_url:
        faltantes.append("acta de cierre")
    if not cierre.entregables_finales.exists():
        faltantes.append("al menos un entregable final")
    if faltantes:
        messages.error(request, "Antes de finalizar completa: " + ", ".join(faltantes) + ".")
        return redirect("cierre:gestionar", reto_id=reto.pk)
    if reto.estado != "finalizado":
        cambiar_estado_reto(reto, "finalizado", request.user, "Cierre formal del reto completado.")
    participantes = _participantes(reto)
    encuestas = [EncuestaSatisfaccion(cierre=cierre, participante_id=pk) for pk in participantes]
    EncuestaSatisfaccion.objects.bulk_create(encuestas, ignore_conflicts=True)
    # La deduplicacion iba por titulo, asi que dos retos homonimos colisionaban;
    # ahora la clave incluye el id del reto.
    from apps.usuarios.models import Usuario

    notificar_muchos(
        Usuario.objects.filter(pk__in=participantes),
        "ENCUESTA_PENDIENTE",
        titulo=f"Reto finalizado: {reto.titulo}",
        mensaje=(
            "El reto fue cerrado formalmente. "
            "Te invitamos a responder la encuesta de satisfaccion."
        ),
        link=reverse("cierre:mis_encuestas"),
        clave_dedupe=f"cierre:{reto.pk}:encuesta",
    )
    # Al cerrar formalmente el reto se emiten los certificados (HU16).
    emitir_certificados_de_reto(reto, emitido_por=request.user)

    cierre.encuesta_enviada = True
    cierre.cerrado_por = request.user
    cierre.cerrado_en = timezone.now()
    cierre.save(update_fields=["encuesta_enviada", "cerrado_por", "cerrado_en", "actualizado_en"])
    messages.success(request, "Reto finalizado; participantes notificados y encuestas generadas.")
    return redirect("cierre:gestionar", reto_id=reto.pk)


@rol_requerido("EMPRESA", "PROFESOR", "ESTUDIANTE", raise_exception=True)
def mis_encuestas(request):
    encuestas = request.user.encuestas_satisfaccion.select_related("cierre__reto")
    return render(request, "cierre/encuestas.html", {"encuestas": encuestas})


@rol_requerido("EMPRESA", "PROFESOR", "ESTUDIANTE", raise_exception=True)
def responder_encuesta(request, pk):
    encuesta = get_object_or_404(EncuestaSatisfaccion, pk=pk, participante=request.user)
    form = EncuestaSatisfaccionForm(request.POST or None, instance=encuesta)
    if request.method == "POST" and form.is_valid():
        respuesta = form.save(commit=False)
        respuesta.respondida_en = timezone.now()
        respuesta.save()
        messages.success(request, "Gracias por compartir tu retroalimentacion.")
        return redirect("cierre:mis_encuestas")
    return render(request, "cierre/responder_encuesta.html", {"encuesta": encuesta, "form": form})
