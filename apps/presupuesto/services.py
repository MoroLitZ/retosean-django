"""Registro de gastos y alerta de ejecucion presupuestal (HU07)."""

from decimal import Decimal

from django.db import transaction
from django.urls import reverse
from django.utils import timezone

from apps.notificaciones.services import notificar, notificar_admins

from .models import Gasto

UMBRAL_ALERTA = Decimal("80")


@transaction.atomic
def registrar_gasto(presupuesto, gasto, usuario):
    """Guarda el gasto (comprometido o ejecutado) y revisa el umbral de alerta.

    El umbral se calcula sobre comprometido + ejecutado, asi que reservar
    presupuesto ya puede disparar la alerta, no solo gastarlo de verdad.
    """
    gasto.presupuesto = presupuesto
    gasto.registrado_por = usuario
    if gasto.estado == Gasto.EJECUTADO and gasto.ejecutado_en is None:
        gasto.ejecutado_en = timezone.now()
    gasto.save()
    revisar_umbral(presupuesto)
    return gasto


@transaction.atomic
def ejecutar_gasto(gasto):
    """Pasa un gasto de COMPROMETIDO a EJECUTADO (ya se pago de verdad)."""
    if gasto.estado != Gasto.COMPROMETIDO:
        return gasto
    gasto.estado = Gasto.EJECUTADO
    gasto.ejecutado_en = timezone.now()
    gasto.save(update_fields=["estado", "ejecutado_en"])
    revisar_umbral(gasto.presupuesto)
    return gasto


def revisar_umbral(presupuesto):
    """Notifica una sola vez cuando comprometido + ejecutado supera el umbral.

    Se calcula sobre lo asignado (comprometido + ejecutado), no solo lo ya
    pagado: reservar presupuesto ya reduce lo disponible de verdad, asi que
    debe poder disparar la alerta igual que un gasto pagado.
    """
    porcentaje = presupuesto.porcentaje_ejecutado

    if porcentaje < UMBRAL_ALERTA:
        # Si el presupuesto se amplia o se borra/reduce un gasto, se rearma la alerta.
        if presupuesto.alerta_80_enviada:
            presupuesto.alerta_80_enviada = False
            presupuesto.save(update_fields=["alerta_80_enviada"])
        return False

    if presupuesto.alerta_80_enviada:
        return False

    reto = presupuesto.reto
    mensaje = (
        f"El presupuesto del reto '{reto.titulo}' va en {porcentaje:.1f}% asignado "
        f"(comprometido: {presupuesto.monto_comprometido}, ejecutado: {presupuesto.monto_ejecutado}, "
        f"de {presupuesto.monto_total}). Disponible: {presupuesto.monto_disponible}."
    )
    link = reverse("presupuesto:detalle", kwargs={"reto_id": reto.pk})

    notificar_admins(
        "PRESUPUESTO_80", mensaje=mensaje, link=link,
        clave_dedupe=f"presupuesto:{presupuesto.pk}:80",
    )
    notificar(
        reto.empresa, "PRESUPUESTO_80", mensaje=mensaje, link=link,
        clave_dedupe=f"presupuesto:{presupuesto.pk}:80",
    )

    presupuesto.alerta_80_enviada = True
    presupuesto.save(update_fields=["alerta_80_enviada"])
    return True
