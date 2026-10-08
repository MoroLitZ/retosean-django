"""Ciclo de vida de los entregables (HU12)."""

from apps.seguimiento.services import MaquinaEstados, cambiar_estado, registrar_cambio_estado

from .models import Entregable


# Enviado -> En revision (el profesor lo abre) -> Calificado. Volver a calificar
# (Calificado -> Calificado) y reemplazar el archivo (-> Enviado) quedan
# registrados como transiciones para que el historial muestre cada version.
MAQUINA_ENTREGABLE = MaquinaEstados(
    "el entregable",
    {
        "ENVIADO": {"ENVIADO", "EN_REVISION", "CALIFICADO"},
        "EN_REVISION": {"ENVIADO", "CALIFICADO"},
        "CALIFICADO": {"ENVIADO", "CALIFICADO"},
    },
    dict(Entregable.ESTADOS),
)


def registrar_entrega(entregable, usuario, creado):
    """Deja en el historial una entrega nueva o el reemplazo de un archivo.

    El llamador ya guardo los datos del entregable; si no es nuevo, aqui se
    devuelve a Enviado para que el profesor lo revise de nuevo.
    """
    if creado:
        registrar_cambio_estado(entregable, "", "ENVIADO", usuario, "Entregable enviado.")
    else:
        cambiar_estado(entregable, MAQUINA_ENTREGABLE, "ENVIADO", usuario, "Archivo del entregable reemplazado.")


def marcar_en_revision(entregables, usuario):
    """Pasa a En revision los entregables enviados que el profesor acaba de abrir."""
    for entregable in entregables.filter(estado="ENVIADO"):
        cambiar_estado(entregable, MAQUINA_ENTREGABLE, "EN_REVISION", usuario, "Entregable abierto para revision.")
