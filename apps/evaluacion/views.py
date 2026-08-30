from decimal import Decimal, InvalidOperation

from django.contrib import messages
from django.core.exceptions import PermissionDenied, ValidationError
from django.db import transaction
from django.urls import reverse
from django.shortcuts import get_object_or_404, redirect, render

from apps.usuarios.decorators import rol_requerido
from apps.seguimiento.models import IntegracionAcademica

from .forms import ComentarioEntregableForm, RubricaForm
from apps.notificaciones.services import notificar

from .models import CriterioRubrica, Entregable, Evaluacion, EvaluacionCriterio, Rubrica


def _retos_profesor(usuario):
    return IntegracionAcademica.objects.filter(profesor=usuario).values_list("reto_id", flat=True)


@rol_requerido("PROFESOR", raise_exception=True)
def panel_profesor(request):
    # Al abrir el panel, el profesor "abre" sus entregables para revisarlos:
    # los que siguen ENVIADO pasan a EN_REVISION. Es el único punto donde ese
    # estado se alcanza; los KPIs y las plantillas ya lo contaban.
    Entregable.objects.filter(
        reto_id__in=_retos_profesor(request.user), estado="ENVIADO"
    ).update(estado="EN_REVISION")

    entregables = Entregable.objects.filter(reto_id__in=_retos_profesor(request.user)).select_related(
        "reto", "estudiante", "equipo"
    ).prefetch_related("historial_comentarios__autor").order_by("estado", "-fecha_entrega")
    rubricas = Rubrica.objects.filter(reto_id__in=_retos_profesor(request.user), activa=True).prefetch_related("criterios")
    return render(request, "evaluacion/panel_profesor.html", {"entregables": entregables, "rubricas": rubricas})


@rol_requerido("PROFESOR", raise_exception=True)
@transaction.atomic
def calificar(request, pk):
    if request.method != "POST":
        raise PermissionDenied("La calificacion solo se registra mediante POST.")
    entregable = get_object_or_404(Entregable, pk=pk, reto_id__in=_retos_profesor(request.user))
    rubrica = None
    rubrica_id = request.POST.get("rubrica")
    try:
        if rubrica_id:
            rubrica = Rubrica.objects.prefetch_related("criterios").get(
                pk=rubrica_id, reto=entregable.reto, activa=True
            )
            puntajes = []
            for criterio in rubrica.criterios.all():
                valor = Decimal(request.POST.get(f"criterio_{criterio.pk}", ""))
                if valor < 0 or valor > criterio.puntaje_maximo:
                    raise ValueError
                puntajes.append((criterio, valor))
            nota = sum((valor for _, valor in puntajes), Decimal("0"))
        else:
            nota = Decimal(request.POST.get("nota", ""))
            if nota < 0 or nota > entregable.puntaje_maximo:
                raise ValueError
            puntajes = []
    except (Rubrica.DoesNotExist, InvalidOperation, ValueError):
        messages.error(request, "La calificacion o los puntajes de la rubrica no son validos.")
        return redirect("evaluacion:panel_profesor")

    comentario = request.POST.get("comentario", "").strip()
    evaluacion, _ = Evaluacion.objects.update_or_create(
        entregable=entregable,
        profesor=request.user,
        defaults={"nota": nota, "comentario": comentario, "rubrica": rubrica},
    )
    try:
        evaluacion.full_clean()
    except ValidationError:
        transaction.set_rollback(True)
        messages.error(request, "La evaluacion no cumple con la escala configurada.")
        return redirect("evaluacion:panel_profesor")
    evaluacion.puntajes_criterio.all().delete()
    for criterio, valor in puntajes:
        EvaluacionCriterio.objects.create(evaluacion=evaluacion, criterio=criterio, puntaje=valor)
    entregable.nota = nota
    entregable.comentario_profesor = comentario
    entregable.estado = "CALIFICADO"
    entregable.save(update_fields=["nota", "comentario_profesor", "estado", "actualizado_en"])
    if comentario:
        entregable.historial_comentarios.create(autor=request.user, comentario=comentario)

    mensaje = (
        f'Tu entregable "{entregable.titulo}" del reto "{entregable.reto.titulo}" '
        f'fue calificado con {nota} de {entregable.puntaje_maximo}.'
    )
    if comentario:
        mensaje += chr(10) + f'Retroalimentacion: {comentario}'
    notificar(
        entregable.estudiante, 'ENTREGABLE_CALIFICADO',
        mensaje=mensaje,
        link=reverse('participaciones:mis_entregables_reto', kwargs={'reto_id': entregable.reto_id}),
        clave_dedupe=f'entregable:{entregable.pk}:calificado:{evaluacion.pk}:{nota}',
    )
    messages.success(request, "Evaluacion formal registrada y visible para el estudiante.")
    return redirect("evaluacion:panel_profesor")


@rol_requerido("PROFESOR", raise_exception=True)
def crear_rubrica(request, reto_id):
    if not IntegracionAcademica.objects.filter(reto_id=reto_id, profesor=request.user).exists():
        raise PermissionDenied("No puedes configurar una rubrica para este reto.")
    form = RubricaForm(request.POST or None)
    if request.method == "POST" and form.is_valid():
        with transaction.atomic():
            rubrica = form.save(commit=False)
            rubrica.reto_id = reto_id
            rubrica.creada_por = request.user
            rubrica.save()
            for orden, (nombre, puntaje) in enumerate(form.cleaned_data["criterios"]):
                CriterioRubrica.objects.create(
                    rubrica=rubrica, nombre=nombre, puntaje_maximo=puntaje, orden=orden
                )
        messages.success(request, "Rubrica creada correctamente.")
        return redirect("evaluacion:panel_profesor")
    return render(request, "evaluacion/rubrica_form.html", {"form": form})


@rol_requerido("PROFESOR", "ESTUDIANTE", raise_exception=True)
def comentario(request, pk):
    entregable = get_object_or_404(Entregable, pk=pk)
    autorizado = (
        (request.user.rol == "ESTUDIANTE" and entregable.estudiante_id == request.user.pk)
        or (
            request.user.rol == "PROFESOR"
            and IntegracionAcademica.objects.filter(reto_id=entregable.reto_id, profesor=request.user).exists()
        )
    )
    if not autorizado:
        raise PermissionDenied("No tienes acceso a este entregable.")
    form = ComentarioEntregableForm(request.POST or None)
    if request.method == "POST":
        if form.is_valid():
            registro = form.save(commit=False)
            registro.entregable = entregable
            registro.autor = request.user
            registro.save()
            messages.success(request, "Comentario agregado al historial.")
        else:
            messages.error(request, "No se pudo agregar el comentario. Revisa el texto ingresado.")
    destino = "evaluacion:panel_profesor" if request.user.rol == "PROFESOR" else "participaciones:mis_entregables_reto"
    kwargs = {} if request.user.rol == "PROFESOR" else {"reto_id": entregable.reto_id}
    return redirect(destino, **kwargs)
