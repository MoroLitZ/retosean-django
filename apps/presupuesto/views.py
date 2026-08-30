from django.contrib import messages
from django.core.paginator import Paginator
from django.db.models import Prefetch
from django.shortcuts import get_object_or_404, redirect, render
from django.views.decorators.http import require_POST

from apps.retos.models import Reto
from apps.usuarios.decorators import rol_requerido, solo_admin
from apps.usuarios.roles import es_admin

from .forms import GastoForm, PresupuestoForm
from .models import Gasto, Presupuesto
from .services import registrar_gasto, revisar_umbral


def _puede_ver(user, reto):
    """El admin ve todo; la empresa solo el presupuesto de sus propios retos."""
    return es_admin(user) or reto.empresa_id == user.pk


@solo_admin
def panel(request):
    """Listado de presupuestos con su nivel de ejecucion."""
    presupuestos = (
        Presupuesto.objects
        .select_related("reto", "reto__empresa")
        .prefetch_related(Prefetch("gastos", queryset=Gasto.objects.only("id", "monto", "presupuesto_id")))
        .order_by("-actualizado_en")
    )
    paginator = Paginator(presupuestos, 25)
    page_obj = paginator.get_page(request.GET.get("page"))
    filas = [
        {
            "obj": p,
            "porcentaje": p.porcentaje_ejecutado,
            "en_alerta": p.porcentaje_ejecutado >= 80,
        }
        for p in page_obj
    ]
    retos_sin_presupuesto = Reto.objects.filter(
        presupuesto__isnull=True
    ).exclude(estado__in=["borrador", "cancelado"]).select_related("empresa")

    return render(request, "presupuesto/panel.html", {
        "titulo": "Gestión de Presupuesto",
        "filas": filas,
        "page_obj": page_obj,
        "retos_sin_presupuesto": retos_sin_presupuesto,
    })


@rol_requerido("ADMIN", "EMPRESA")
def detalle(request, reto_id):
    """Presupuesto de un reto: monto asignado, gastos y ejecucion."""
    reto = get_object_or_404(Reto.objects.select_related("empresa"), pk=reto_id)
    if not _puede_ver(request.user, reto):
        messages.error(request, "No tienes acceso al presupuesto de ese reto.")
        return redirect("dashboard:panel")

    presupuesto = Presupuesto.objects.filter(reto=reto).first()
    gastos = (
        presupuesto.gastos.select_related("registrado_por").order_by("-creado_en")
        if presupuesto else []
    )

    return render(request, "presupuesto/detalle.html", {
        "titulo": f"Presupuesto: {reto.titulo}",
        "reto": reto,
        "presupuesto": presupuesto,
        "gastos": gastos,
        "form_presupuesto": PresupuestoForm(instance=presupuesto),
        "form_gasto": GastoForm(),
        "puede_editar": es_admin(request.user),
        "porcentaje": presupuesto.porcentaje_ejecutado if presupuesto else 0,
    })


@require_POST
@solo_admin
def definir_presupuesto(request, reto_id):
    reto = get_object_or_404(Reto, pk=reto_id)
    presupuesto = Presupuesto.objects.filter(reto=reto).first()
    form = PresupuestoForm(request.POST, instance=presupuesto)
    if form.is_valid():
        presupuesto = form.save(commit=False)
        presupuesto.reto = reto
        presupuesto.save()
        # Ampliar o recortar el monto puede cruzar (o descruzar) el umbral.
        revisar_umbral(presupuesto)
        messages.success(request, "Presupuesto actualizado.")
    else:
        messages.error(request, "Revisa el monto del presupuesto.")
    return redirect("presupuesto:detalle", reto_id=reto.pk)


@require_POST
@solo_admin
def agregar_gasto(request, reto_id):
    reto = get_object_or_404(Reto, pk=reto_id)
    presupuesto = Presupuesto.objects.filter(reto=reto).first()
    if presupuesto is None:
        messages.error(request, "Primero define el presupuesto del reto.")
        return redirect("presupuesto:detalle", reto_id=reto.pk)

    form = GastoForm(request.POST, request.FILES)
    if form.is_valid():
        registrar_gasto(presupuesto, form.save(commit=False), request.user)
        presupuesto.refresh_from_db()
        if presupuesto.porcentaje_ejecutado >= 80:
            messages.warning(
                request,
                f"Gasto registrado. Atención: la ejecución va en "
                f"{presupuesto.porcentaje_ejecutado:.1f}% del presupuesto.",
            )
        else:
            messages.success(request, "Gasto registrado.")
    else:
        messages.error(request, "Revisa los datos del gasto.")
    return redirect("presupuesto:detalle", reto_id=reto.pk)


@require_POST
@solo_admin
def eliminar_gasto(request, pk):
    gasto = get_object_or_404(Gasto.objects.select_related("presupuesto__reto"), pk=pk)
    presupuesto = gasto.presupuesto
    gasto.delete()
    revisar_umbral(presupuesto)
    messages.success(request, "Gasto eliminado.")
    return redirect("presupuesto:detalle", reto_id=presupuesto.reto_id)
