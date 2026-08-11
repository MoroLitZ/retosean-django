from django.contrib import messages
from django.db import transaction
from django.shortcuts import get_object_or_404, redirect, render
from django.utils import timezone

from apps.notificaciones.models import Notificacion
from apps.participaciones.models import MiembroEquipo, Postulacion
from apps.retos.models import Reto
from apps.retos.services import cambiar_estado_reto
from apps.retos.views import rol_requerido
from apps.seguimiento.models import IntegracionAcademica

from .forms import AgendaCierreForm, CierreRetoForm, EncuestaSatisfaccionForm, EntregableFinalForm, ReconocimientoForm
from .models import AgendaCierre, CierreReto, EncuestaSatisfaccion


def _participantes(reto):
    ids = {reto.empresa_id}
    ids.update(IntegracionAcademica.objects.filter(reto=reto).values_list("profesor_id", flat=True))
    ids.update(Postulacion.objects.filter(reto=reto, estado="ACEPTADA").values_list("estudiante_id", flat=True))
    ids.update(MiembroEquipo.objects.filter(equipo__reto=reto).values_list("estudiante_id", flat=True))
    return {pk for pk in ids if pk}


@rol_requerido("ADMIN")
def panel(request):
    retos = Reto.objects.select_related("empresa").filter(estado__in=["aprobado", "en_curso", "pausado", "finalizado"])
    return render(request, "cierre/panel.html", {"retos": retos})


@rol_requerido("ADMIN")
def gestionar(request, reto_id):
    reto = get_object_or_404(
        Reto, pk=reto_id, estado__in=["aprobado", "en_curso", "pausado", "finalizado"]
    )
    agenda, _ = AgendaCierre.objects.get_or_create(reto=reto)
    cierre, _ = CierreReto.objects.get_or_create(reto=reto, defaults={"cerrado_por": request.user})
    agenda_form = AgendaCierreForm(request.POST or None, instance=agenda, prefix="agenda")
    cierre_form = CierreRetoForm(request.POST or None, request.FILES or None, instance=cierre, prefix="cierre")
    if request.method == "POST" and request.POST.get("accion") == "guardar" and agenda_form.is_valid() and cierre_form.is_valid():
        agenda_form.save()
        registro = cierre_form.save(commit=False)
        registro.cerrado_por = request.user
        registro.save()
        messages.success(request, "Documentacion y agenda guardadas.")
        return redirect("cierre:gestionar", reto_id=reto.pk)
    return render(request, "cierre/gestionar.html", {
        "reto": reto, "agenda_form": agenda_form, "cierre_form": cierre_form, "cierre": cierre,
        "entregable_form": EntregableFinalForm(), "reconocimiento_form": ReconocimientoForm(),
    })


@rol_requerido("ADMIN")
def agregar_entregable(request, reto_id):
    cierre = get_object_or_404(CierreReto, reto_id=reto_id)
    form = EntregableFinalForm(request.POST or None, request.FILES or None)
    if request.method == "POST" and form.is_valid():
        item = form.save(commit=False)
        item.cierre = cierre
        item.cargado_por = request.user
        item.save()
        messages.success(request, "Entregable final cargado.")
    else:
        messages.error(request, "No fue posible cargar el entregable final.")
    return redirect("cierre:gestionar", reto_id=reto_id)


@rol_requerido("ADMIN")
def agregar_reconocimiento(request, reto_id):
    cierre = get_object_or_404(CierreReto, reto_id=reto_id)
    form = ReconocimientoForm(request.POST or None)
    if request.method == "POST" and form.is_valid():
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


@rol_requerido("ADMIN")
@transaction.atomic
def finalizar(request, reto_id):
    reto = get_object_or_404(
        Reto.objects.select_for_update(), pk=reto_id,
        estado__in=["aprobado", "en_curso", "pausado", "finalizado"],
    )
    if request.method != "POST":
        return redirect("cierre:gestionar", reto_id=reto.pk)
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
    existentes = set(Notificacion.objects.filter(
        usuario_id__in=participantes, titulo=f"Reto finalizado: {reto.titulo}"
    ).values_list("usuario_id", flat=True))
    Notificacion.objects.bulk_create([
        Notificacion(
            usuario_id=pk, tipo="INFO", titulo=f"Reto finalizado: {reto.titulo}",
            mensaje="El reto fue cerrado formalmente. Te invitamos a responder la encuesta de satisfaccion.",
            link="/cierre/encuestas/",
        ) for pk in participantes if pk not in existentes
    ])
    cierre.encuesta_enviada = True
    cierre.cerrado_por = request.user
    cierre.cerrado_en = timezone.now()
    cierre.save(update_fields=["encuesta_enviada", "cerrado_por", "cerrado_en", "actualizado_en"])
    messages.success(request, "Reto finalizado; participantes notificados y encuestas generadas.")
    return redirect("cierre:gestionar", reto_id=reto.pk)


@rol_requerido("EMPRESA", "PROFESOR", "ESTUDIANTE")
def mis_encuestas(request):
    encuestas = request.user.encuestas_satisfaccion.select_related("cierre__reto")
    return render(request, "cierre/encuestas.html", {"encuestas": encuestas})


@rol_requerido("EMPRESA", "PROFESOR", "ESTUDIANTE")
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
