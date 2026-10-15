"""Servicios de participacion: postulaciones, equipos operativos y su sincronizacion."""

from apps.seguimiento.services import MaquinaEstados

from .models import Equipo, MiembroEquipo, Postulacion


# Postulacion del estudiante (HU11). Aceptada y Rechazada son finales; una
# postulacion retirada puede reenviarse y vuelve a Pendiente.
MAQUINA_POSTULACION = MaquinaEstados(
    "la postulacion",
    {
        "PENDIENTE": {"EN_REVISION", "ACEPTADA", "RECHAZADA", "RETIRADA"},
        # La empresa abrio la lista de postulaciones de su reto.
        "EN_REVISION": {"ACEPTADA", "RECHAZADA", "RETIRADA"},
        "RETIRADA": {"PENDIENTE"},
        "ACEPTADA": set(),
        "RECHAZADA": set(),
    },
    dict(Postulacion.ESTADOS),
)


def marcar_postulaciones_en_revision(postulaciones, usuario):
    """Pasa a En revision las postulaciones pendientes que la empresa acaba de abrir."""
    from apps.seguimiento.services import cambiar_estado

    for postulacion in postulaciones.filter(estado="PENDIENTE"):
        cambiar_estado(postulacion, MAQUINA_POSTULACION, "EN_REVISION", usuario, "La empresa abrio la postulacion.")


def sincronizar_equipo_academico(equipo_academico):
    """Refleja un EquipoRetoAcademico en un Equipo operativo.

    El equipo que arma el profesor (`retos.EquipoRetoAcademico`) es la plantilla
    academica; los entregables y las votaciones de hackathon trabajan sobre
    `participaciones.Equipo`. Esta funcion mantiene el segundo al dia a partir
    del primero y es idempotente: se puede llamar en cada guardado.
    """
    equipo = equipo_academico.equipo_espejo
    if equipo is None:
        equipo = Equipo.objects.create(
            reto=equipo_academico.reto,
            nombre=equipo_academico.nombre_equipo,
        )
        equipo_academico.equipo_espejo = equipo
        equipo_academico.save(update_fields=['equipo_espejo'])
    elif equipo.nombre != equipo_academico.nombre_equipo:
        equipo.nombre = equipo_academico.nombre_equipo
        equipo.save(update_fields=['nombre'])

    estudiantes = list(equipo_academico.estudiantes.order_by('pk'))
    ids_actuales = {e.pk for e in estudiantes}

    MiembroEquipo.objects.filter(equipo=equipo).exclude(
        estudiante_id__in=ids_actuales
    ).delete()

    ya_registrados = set(
        MiembroEquipo.objects.filter(equipo=equipo).values_list('estudiante_id', flat=True)
    )
    MiembroEquipo.objects.bulk_create([
        MiembroEquipo(equipo=equipo, estudiante=estudiante)
        for estudiante in estudiantes
        if estudiante.pk not in ya_registrados
    ])

    # Rol LIDER: el primer integrante es el lider, el resto miembros.
    if estudiantes:
        lider_id = estudiantes[0].pk
        MiembroEquipo.objects.filter(equipo=equipo, estudiante_id=lider_id).update(rol="LIDER")
        MiembroEquipo.objects.filter(equipo=equipo).exclude(
            estudiante_id=lider_id
        ).update(rol="MIEMBRO")
    else:
        MiembroEquipo.objects.filter(equipo=equipo).update(rol="MIEMBRO")

    return equipo
