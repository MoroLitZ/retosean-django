"""Registro de gastos y alerta de ejecucion presupuestal (HU07)."""

from decimal import Decimal

from django.db import transaction
from django.urls import reverse

from apps.notificaciones.services import notificar, notificar_admins

UMBRAL_ALERTA = Decimal("80")


@transaction.atomic
def registrar_gasto(presupuesto, gasto, usuario):
    """Guarda el gasto y dispara la alerta si se supera el 80% del presupuesto."""
    gasto.presupuesto = presupuesto
    gasto.registrado_por = usuario
    gasto.save()
    revisar_umbral(presupuesto)
    return gasto


def revisar_umbral(presupuesto):
    """Notifica una sola vez cuando la ejecucion supera el umbral configurado."""
    porcentaje = presupuesto.porcentaje_ejecutado

    if porcentaje < UMBRAL_ALERTA:
        # Si el presupuesto se amplia o se borra un gasto, se rearma la alerta.
        if presupuesto.alerta_80_enviada:
            presupuesto.alerta_80_enviada = False
            presupuesto.save(update_fields=["alerta_80_enviada"])
        return False

    if presupuesto.alerta_80_enviada:
        return False

    reto = presupuesto.reto
    mensaje = (
        f"El presupuesto del reto '{reto.titulo}' va en {porcentaje:.1f}% de ejecucion "
        f"({presupuesto.monto_ejecutado} de {presupuesto.monto_total}). "
        f"Disponible: {presupuesto.monto_disponible}."
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
