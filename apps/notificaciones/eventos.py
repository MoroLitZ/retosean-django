"""Catalogo de eventos de dominio que generan notificacion.

Tener los eventos en un solo sitio evita que cada vista invente su propio
titulo y su propio tipo, que es como estaba antes de centralizar el motor.
"""

from dataclasses import dataclass


@dataclass(frozen=True)
class Evento:
    slug: str
    tipo: str
    titulo: str
    # Si es False, el evento nunca sale por correo (solo campana in-app).
    permite_email: bool = True


def _registrar(*eventos):
    return {evento.slug: evento for evento in eventos}


EVENTOS = _registrar(
    # --- Empresas y documentacion (HU00, HU01) ---
    Evento("DOCUMENTO_REVISADO", "INFO", "Documentacion revisada"),
    Evento("DOCUMENTO_CARGADO_ADMIN", "INFO", "Documento legal cargado por empresa"),
    Evento("EMPRESA_LISTAS_RESTRICTIVAS", "ERROR", "Verificacion en listas restrictivas"),
    # --- Ciclo de vida del reto (HU02, HU04) ---
    Evento("RETO_ESTADO_CAMBIADO", "INFO", "Cambio de estado de tu reto"),
    Evento("RETO_ENVIADO_REVISION", "INFO", "Reto enviado a revision"),
    # --- Integracion academica (HU03) ---
    Evento("INTEGRACION_ENVIADA_REVISION", "INFO", "Nueva postulacion docente a reto"),
    Evento("INTEGRACION_REVISADA", "INFO", "Integracion academica revisada"),
    Evento("EQUIPO_ASIGNADO", "EXITO", "Fuiste asignado a un equipo"),
    # --- Participacion estudiantil (HU10, HU11) ---
    Evento("POSTULACION_NUEVA", "INFO", "Nueva postulacion"),
    Evento("POSTULACION_GESTIONADA", "INFO", "Respuesta a tu postulacion"),
    # --- Evaluacion y seguimiento (HU12, HU14) ---
    Evento("ENTREGABLE_RECIBIDO", "INFO", "Nuevo entregable recibido"),
    Evento("ENTREGABLE_CALIFICADO", "EXITO", "Tu entregable fue calificado"),
    # --- Cierre (HU08) y certificacion (HU16) ---
    Evento("RETO_FINALIZADO", "EXITO", "Reto finalizado"),
    Evento("CERTIFICADO_DISPONIBLE", "EXITO", "Tu certificado esta disponible"),
    Evento("ENCUESTA_PENDIENTE", "INFO", "Encuesta de satisfaccion"),
    # --- Presupuesto (HU07) ---
    Evento("PRESUPUESTO_80", "ADVERTENCIA", "Presupuesto por encima del 80%"),
    # --- Recordatorios automaticos (HU15) ---
    Evento("RECORDATORIO_FECHA_LIMITE", "ADVERTENCIA", "Recordatorio de fecha limite"),
    # --- Hackathon (HU09) ---
    Evento("HACKATON_PUBLICADO", "INFO", "Hackathon publicado"),
    Evento("HACKATON_RESULTADOS", "EXITO", "Resultados del hackathon"),
)


def obtener(slug):
    """Devuelve el Evento del catalogo, o uno generico si no esta registrado."""
    return EVENTOS.get(slug) or Evento(slug, "INFO", slug.replace("_", " ").capitalize())
