"""Ayudas de prueba para integraciones academicas (HU03)."""

from datetime import timedelta

from django.utils import timezone


def completar_equipo_y_cronograma(integracion):
    """Deja a la integracion con el equipo y las sesiones que exige el envio a revision."""
    from apps.retos.models import SesionRetoAcademico

    from .models import ParticipanteIntegracion

    ParticipanteIntegracion.objects.create(
        integracion=integracion, rol=ParticipanteIntegracion.PROFESOR, nombre="Profesor Titular",
        documento="1010", email="titular@ean.edu.co", facultad="Ingenieria",
    )
    inicio = timezone.now() + timedelta(days=7)
    for dias, tipo in [(0, "inicio"), (14, "seguimiento"), (28, "evaluacion")]:
        SesionRetoAcademico.objects.create(
            reto=integracion.reto, integracion=integracion, tipo=tipo, fecha_hora=inicio + timedelta(days=dias),
        )


def datos_formsets(participantes=None, sesiones=None):
    """POST de los formsets del formulario de integracion; por defecto, un equipo valido."""
    if participantes is None:
        participantes = [{
            "rol": "PROFESOR", "nombre": "Profesor Titular", "documento": "1010",
            "email": "titular@ean.edu.co", "facultad": "Ingenieria",
        }]
    if sesiones is None:
        base = timezone.localtime() + timedelta(days=7)
        sesiones = [
            {"tipo": tipo, "fecha_hora": (base + timedelta(days=d)).strftime("%Y-%m-%dT%H:%M")}
            for d, tipo in [(0, "inicio"), (14, "seguimiento"), (28, "evaluacion")]
        ]
    datos = {}
    for prefijo, filas in [("participantes", participantes), ("sesiones", sesiones)]:
        datos.update({
            f"{prefijo}-TOTAL_FORMS": str(len(filas)), f"{prefijo}-INITIAL_FORMS": "0",
            f"{prefijo}-MIN_NUM_FORMS": "0", f"{prefijo}-MAX_NUM_FORMS": "1000",
        })
        for i, fila in enumerate(filas):
            datos.update({f"{prefijo}-{i}-{campo}": valor for campo, valor in fila.items()})
    return datos
