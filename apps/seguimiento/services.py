"""Maquinas de estado y bitacora de cambios de estado.

Cada entidad con ciclo de vida declara sus transiciones validas con una
`MaquinaEstados` en el `services.py` de su app y cambia de estado solo a traves
de `cambiar_estado`, que rechaza las transiciones invalidas y deja el cambio en
`HistorialEstado` (quien, cuando, de que estado, a que estado y comentario).
"""

from django.contrib.contenttypes.models import ContentType

from .models import HistorialEstado


class TransicionInvalida(Exception):
    """Se intento mover una entidad a un estado no permitido desde el actual."""

    def __init__(self, mensaje, estado_actual=None, estado_nuevo=None):
        super().__init__(mensaje)
        self.mensaje = mensaje
        self.estado_actual = estado_actual
        self.estado_nuevo = estado_nuevo

    def __str__(self):
        return self.mensaje


class MaquinaEstados:
    """Tabla de transiciones validas de una entidad.

    `transiciones` es {estado: {estados destino}}; un estado sin destinos es final.
    `etiquetas` traduce cada estado a su nombre en espanol para los mensajes.
    """

    def __init__(self, nombre, transiciones, etiquetas=None):
        self.nombre = nombre
        self.transiciones = {origen: frozenset(destinos) for origen, destinos in transiciones.items()}
        self.etiquetas = dict(etiquetas or {})

    def etiqueta(self, estado):
        return self.etiquetas.get(estado, estado or "sin estado")

    def destinos(self, estado_actual):
        return self.transiciones.get(estado_actual, frozenset())

    def puede(self, estado_actual, estado_nuevo):
        return estado_nuevo in self.destinos(estado_actual)

    def es_final(self, estado):
        return not self.destinos(estado)

    def validar(self, estado_actual, estado_nuevo):
        if not self.puede(estado_actual, estado_nuevo):
            raise TransicionInvalida(
                f'No se puede pasar {self.nombre} de "{self.etiqueta(estado_actual)}" '
                f'a "{self.etiqueta(estado_nuevo)}".',
                estado_actual=estado_actual,
                estado_nuevo=estado_nuevo,
            )


def registrar_cambio_estado(objeto, estado_anterior, estado_nuevo, usuario=None, comentario="", campo="estado"):
    """Deja constancia de un cambio de estado ya aplicado sobre `objeto`."""
    if usuario is not None and not getattr(usuario, "is_authenticated", False):
        usuario = None
    return HistorialEstado.objects.create(
        content_type=ContentType.objects.get_for_model(objeto, for_concrete_model=True),
        object_id=objeto.pk,
        campo=campo,
        estado_anterior=estado_anterior or "",
        estado_nuevo=estado_nuevo,
        comentario=comentario or "",
        realizado_por=usuario,
    )


def cambiar_estado(objeto, maquina, estado_nuevo, usuario=None, comentario="",
                   campo="estado", update_fields=()):
    """Valida la transicion, asigna el estado, guarda y registra el cambio.

    `objeto` debe existir ya en BD: el estado inicial de un objeto nuevo no es
    una transicion y se registra con `registrar_cambio_estado` tras crearlo.
    `update_fields` agrega campos que la vista ya modifico en `objeto` y que
    deben persistirse junto con el estado.

    Lanza `TransicionInvalida` sin tocar nada si la transicion no es valida.
    """
    estado_anterior = getattr(objeto, campo)
    maquina.validar(estado_anterior, estado_nuevo)
    setattr(objeto, campo, estado_nuevo)
    campos = {campo, *update_fields}
    if any(f.name == "actualizado_en" for f in objeto._meta.fields):
        campos.add("actualizado_en")
    objeto.save(update_fields=sorted(campos))
    registrar_cambio_estado(objeto, estado_anterior, estado_nuevo, usuario, comentario, campo)
    return estado_anterior


def historial_de(objeto, campo=None):
    """Cambios de estado registrados para `objeto`, del mas reciente al mas antiguo."""
    qs = HistorialEstado.objects.filter(
        content_type=ContentType.objects.get_for_model(objeto, for_concrete_model=True),
        object_id=objeto.pk,
    ).select_related("realizado_por")
    if campo:
        qs = qs.filter(campo=campo)
    return qs


# Integracion academica del profesor (HU03): borrador -> revision -> aprobada -> publicada.
MAQUINA_INTEGRACION = MaquinaEstados(
    "la integracion academica",
    {
        "borrador": {"en_revision"},
        "en_revision": {"aprobada", "rechazada"},
        "rechazada": {"borrador"},
        "aprobada": {"publicada"},
        "publicada": set(),
    },
    {
        "borrador": "Borrador",
        "en_revision": "En revision",
        "aprobada": "Aprobada",
        "rechazada": "Rechazada",
        "publicada": "Publicada",
    },
)
