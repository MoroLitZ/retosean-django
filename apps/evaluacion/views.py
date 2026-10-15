from decimal import Decimal, InvalidOperation

from django.contrib import messages
from django.core.exceptions import PermissionDenied, ValidationError
from django.db import transaction
from django.db.models import Count, Q
from django.urls import reverse
from django.shortcuts import get_object_or_404, redirect, render
from django.utils import timezone

from apps.usuarios.decorators import rol_requerido
from apps.seguimiento.models import IntegracionAcademica

from .forms import ComentarioEntregableForm, RubricaForm
from apps.notificaciones.services import notificar
from apps.seguimiento.services import cambiar_estado

from .services import MAQUINA_ENTREGABLE, marcar_en_revision

from .models import CriterioRubrica, Entregable, Evaluacion, EvaluacionCriterio, Rubrica
from apps.retos.models import Reto


def _retos_profesor(usuario):
    return IntegracionAcademica.objects.filter(profesor=usuario).values_list("reto_id", flat=True)


@rol_requerido("PROFESOR", raise_exception=True)
def panel_profesor(request):
    retos = Reto.objects.filter(id__in=_retos_profesor(request.user)).select_related("empresa")
    query = request.GET.get("q", "").strip()
    estado = request.GET.get("estado", "")
    if query:
        retos = retos.filter(Q(titulo__icontains=query) | Q(empresa__username__icontains=query))
    if estado:
        retos = retos.filter(estado=estado)
    retos = retos.annotate(
        total_entregables=Count("entregables"),
        pendientes=Count("entregables", filter=Q(entregables__estado__in=["ENVIADO", "EN_REVISION"])),
        sin_publicar=Count("entregables", filter=Q(entregables__estado="CALIFICADO", entregables__calificacion_publicada=False)),
    ).order_by("-actualizado_en")
    return render(request, "evaluacion/panel_profesor.html", {
        "retos": retos,
        "query": query,
        "estado": estado,
        "estados": Reto.ESTADO_CHOICES,
    })


@rol_requerido("PROFESOR", raise_exception=True)
def detalle_reto(request, reto_id):
    if reto_id not in set(_retos_profesor(request.user)):
        raise PermissionDenied("No integraste este reto.")
    reto = get_object_or_404(Reto.objects.select_related("empresa"), pk=reto_id)
    marcar_en_revision(Entregable.objects.filter(reto=reto), request.user)
    entregables = Entregable.objects.filter(reto=reto).select_related(
        "reto", "estudiante", "equipo"
    ).prefetch_related("historial_comentarios__autor").order_by("estado", "-fecha_entrega")
    rubricas = Rubrica.objects.filter(reto=reto, activa=True).prefetch_related("criterios")
    por_publicar = entregables.filter(estado="CALIFICADO", calificacion_publicada=False).count()
    return render(request, "evaluacion/detalle_reto.html", {
        "reto": reto,
        "entregables": entregables,
        "rubricas": rubricas,
        "por_publicar": por_publicar,
    })


@rol_requerido("PROFESOR", raise_exception=True)
@transaction.atomic
def calificar(request, pk):
    if request.method != "POST":
        raise PermissionDenied("La calificacion solo se registra mediante POST.")
    entregable = get_object_or_404(Entregable, pk=pk, reto_id__in=_retos_profesor(request.user))
    next_url = request.POST.get("next") or request.META.get("HTTP_REFERER") or reverse("evaluacion:panel_profesor")
    # HU12: volver a calificar exige confirmacion explicita.
    if entregable.estado == "CALIFICADO" and request.POST.get("confirmar_recalificacion") != "1":
        messages.error(
            request,
            f'"{entregable.titulo}" ya tiene nota ({entregable.nota}). '
            "Marca la confirmacion para volver a calificarlo.",
        )
        return redirect(next_url)
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
        return redirect(next_url)

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
        return redirect(next_url)
    evaluacion.puntajes_criterio.all().delete()
    for criterio, valor in puntajes:
        EvaluacionCriterio.objects.create(evaluacion=evaluacion, criterio=criterio, puntaje=valor)
    recalificacion = entregable.estado == "CALIFICADO"
    entregable.nota = nota
    entregable.comentario_profesor = comentario
    # Una nota nueva (o corregida) queda sin publicar hasta que el profesor la publique.
    entregable.calificacion_publicada = False
    entregable.calificacion_publicada_en = None
    cambiar_estado(
        entregable, MAQUINA_ENTREGABLE, "CALIFICADO", request.user,
        f'{"Recalificado" if recalificacion else "Calificado"} con {nota} de {entregable.puntaje_maximo}.',
        update_fields=["nota", "comentario_profesor", "calificacion_publicada", "calificacion_publicada_en"],
    )
    messages.success(
        request,
        "Evaluacion registrada. Publica las calificaciones del reto para que el estudiante la vea.",
    )
    return redirect(next_url)


@rol_requerido("PROFESOR", raise_exception=True)
@transaction.atomic
def publicar_calificaciones(request, reto_id):
    """Publica a los estudiantes las notas aun no publicadas de un reto (HU12)."""
    if request.method != "POST":
        raise PermissionDenied("Las calificaciones se publican mediante POST.")
    if reto_id not in set(_retos_profesor(request.user)):
        raise PermissionDenied("No integraste este reto.")
    pendientes = list(
        Entregable.objects.filter(reto_id=reto_id, estado="CALIFICADO", calificacion_publicada=False)
        .select_related("reto", "estudiante", "equipo")
    )
    from apps.usuarios.models import Usuario
    ahora = timezone.now()
    for entregable in pendientes:
        entregable.calificacion_publicada = True
        entregable.calificacion_publicada_en = ahora
        entregable.save(update_fields=["calificacion_publicada", "calificacion_publicada_en", "actualizado_en"])
        if entregable.comentario_profesor:
            entregable.historial_comentarios.create(autor=request.user, comentario=entregable.comentario_profesor)
        mensaje = (
            f'Tu entregable "{entregable.titulo}" del reto "{entregable.reto.titulo}" '
            f'fue calificado con {entregable.nota} de {entregable.puntaje_maximo}.'
        )
        if entregable.comentario_profesor:
            mensaje += chr(10) + f'Retroalimentacion: {entregable.comentario_profesor}'
        destinatarios = [entregable.estudiante]
        if entregable.equipo_id:
            destinatarios.extend(
                Usuario.objects.filter(equipos__equipo_id=entregable.equipo_id).distinct()
            )
        from apps.notificaciones.services import notificar_muchos
        notificar_muchos(
            destinatarios, 'ENTREGABLE_CALIFICADO',
            mensaje=mensaje,
            link=reverse('participaciones:mis_entregables_reto', kwargs={'reto_id': entregable.reto_id}),
            clave_dedupe=f'entregable:{entregable.pk}:publicado:{ahora:%Y%m%d%H%M%S}',
        )
    if pendientes:
        messages.success(request, f"{len(pendientes)} calificacion(es) publicadas y estudiantes notificados.")
    else:
        messages.info(request, "No hay calificaciones pendientes de publicar en este reto.")
    return redirect(request.POST.get("next") or reverse("evaluacion:panel_profesor"))


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


@rol_requerido("PROFESOR", "ESTUDIANTE", "EMPRESA", raise_exception=True)
def comentario(request, pk):
    entregable = get_object_or_404(Entregable.objects.select_related("reto"), pk=pk)
    rol = request.user.rol
    autorizado = (
        (
            rol == "ESTUDIANTE"
            and (
                entregable.estudiante_id == request.user.pk
                or entregable.equipo_id in request.user.equipos.values_list("equipo_id", flat=True)
            )
        )
        or (
            rol == "PROFESOR"
            and IntegracionAcademica.objects.filter(reto_id=entregable.reto_id, profesor=request.user).exists()
        )
        # HU14: la empresa comenta los entregables de sus propios retos.
        or (rol == "EMPRESA" and entregable.reto.empresa_id == request.user.pk)
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
            if rol == "EMPRESA":
                _notificar_comentario_empresa(entregable, registro)
            messages.success(request, "Comentario agregado al historial.")
        else:
            messages.error(request, "No se pudo agregar el comentario. Revisa el texto ingresado.")
    if rol == "PROFESOR":
        return redirect("evaluacion:panel_profesor")
    if rol == "EMPRESA":
        return redirect("empresas:seguimiento_reto", reto_id=entregable.reto_id)
    return redirect("participaciones:mis_entregables_reto", reto_id=entregable.reto_id)


def _notificar_comentario_empresa(entregable, registro):
    """El comentario de la empresa llega al equipo (estudiante) y a los profesores del reto."""
    from apps.notificaciones.services import notificar_muchos
    from apps.usuarios.models import Usuario

    profesores = Usuario.objects.filter(integraciones__reto=entregable.reto).distinct()
    destinatarios = [entregable.estudiante, *profesores]
    notificar_muchos(
        destinatarios, "COMENTARIO_EMPRESA",
        mensaje=(
            f'La empresa comentó el entregable "{entregable.titulo}" del reto "{entregable.reto.titulo}": '
            f"{registro.comentario}"
        ),
        link=reverse("participaciones:mis_entregables_reto", kwargs={"reto_id": entregable.reto_id}),
        clave_dedupe=f"comentario-empresa:{registro.pk}",
    )
