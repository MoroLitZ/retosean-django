"""Logica de negocio del hackathon (HU09): etapas, ranking y publicacion."""

from django.db.models import Avg, Count, Sum
from django.urls import reverse

from apps.notificaciones.services import notificar_muchos

from .models import EtapaHackaton, InscripcionHackaton, Jurado, VotacionHackaton


def etapa_vigente(hackathon, tipo):
    """Devuelve la etapa del tipo indicado si esta abierta hoy, o None."""
    etapa = hackathon.etapas.filter(tipo=tipo).first()
    if etapa is None:
        return None
    return etapa if etapa.vigente else None


def inscripciones_abiertas(hackathon):
    if hackathon.estado not in {"publicado", "en_curso"}:
        return False
    etapa = hackathon.etapas.filter(tipo="inscripcion").first()
    # Sin etapa de inscripcion configurada no bloqueamos: el hackathon
    # publicado se considera abierto.
    return etapa.vigente if etapa and (etapa.inicia_en or etapa.termina_en) else True


def votacion_abierta(hackathon):
    if hackathon.estado not in {"publicado", "en_curso"}:
        return False
    etapa = hackathon.etapas.filter(tipo="presentacion").first()
    return etapa.vigente if etapa and (etapa.inicia_en or etapa.termina_en) else True


def es_jurado(hackathon, usuario):
    return Jurado.objects.filter(hackathon=hackathon, usuario=usuario).exists()


def ranking(hackathon):
    """Clasificacion de equipos por promedio de puntaje.

    Se usa el promedio y no la suma para no penalizar a un equipo al que
    votaron menos jurados. Desempate: mas votos primero, luego alfabetico.
    """
    filas = (
        VotacionHackaton.objects
        .filter(hackathon=hackathon)
        .values("equipo_id", "equipo__nombre")
        .annotate(
            promedio=Avg("puntaje"),
            total=Sum("puntaje"),
            votos=Count("id"),
        )
        .order_by("-promedio", "-votos", "equipo__nombre")
    )
    return [
        {
            "posicion": posicion,
            "equipo_id": fila["equipo_id"],
            "equipo": fila["equipo__nombre"],
            "promedio": fila["promedio"],
            "total": fila["total"],
            "votos": fila["votos"],
        }
        for posicion, fila in enumerate(filas, start=1)
    ]


def equipos_sin_votar(hackathon, jurado):
    """Equipos aceptados que este jurado todavia no ha calificado."""
    ya_votados = VotacionHackaton.objects.filter(
        hackathon=hackathon, jurado=jurado
    ).values_list("equipo_id", flat=True)
    return InscripcionHackaton.objects.filter(
        hackathon=hackathon, estado="ACEPTADA"
    ).exclude(equipo_id__in=ya_votados).select_related("equipo")


def _participantes(hackathon):
    from apps.usuarios.models import Usuario

    return Usuario.objects.filter(
        equipos__equipo__inscripciones_hackaton__hackathon=hackathon,
        is_active=True,
    ).distinct()


def publicar(hackathon, usuario):
    """Publica el hackathon y avisa a los participantes del reto."""
    from apps.usuarios.models import Usuario

    hackathon.estado = "publicado"
    hackathon.save(update_fields=["estado", "actualizado_en"])

    reto = hackathon.reto
    inscritos = Usuario.objects.filter(
        postulaciones__reto=reto, postulaciones__estado="ACEPTADA", is_active=True
    ).distinct()
    notificar_muchos(
        inscritos, "HACKATON_PUBLICADO",
        mensaje=(
            f"El hackathon del reto '{reto.titulo}' ya esta publicado. "
            "Inscribe a tu equipo desde la plataforma."
        ),
        link=reverse("hackaton:detalle", kwargs={"pk": hackathon.pk}),
        clave_dedupe=f"hackaton:{hackathon.pk}:publicado",
    )
    return hackathon


def publicar_resultados(hackathon, usuario):
    """Marca los resultados como publicos y notifica a los equipos."""
    hackathon.resultados_publicados = True
    if hackathon.estado != "finalizado":
        hackathon.estado = "finalizado"
    hackathon.save(update_fields=["resultados_publicados", "estado", "actualizado_en"])

    tabla = ranking(hackathon)
    ganador = tabla[0]["equipo"] if tabla else None
    mensaje = f"Ya estan publicados los resultados del hackathon '{hackathon.reto.titulo}'."
    if ganador:
        mensaje += f" Equipo ganador: {ganador}."

    notificar_muchos(
        _participantes(hackathon), "HACKATON_RESULTADOS",
        mensaje=mensaje,
        link=reverse("hackaton:ranking", kwargs={"pk": hackathon.pk}),
        clave_dedupe=f"hackaton:{hackathon.pk}:resultados",
    )
    return tabla


def crear_etapas_por_defecto(hackathon):
    """Crea las cuatro etapas del backlog si el hackathon aun no las tiene."""
    if hackathon.etapas.exists():
        return []
    etapas = [
        EtapaHackaton(
            hackathon=hackathon, tipo=tipo, titulo=titulo, orden=orden
        )
        for orden, (tipo, titulo) in enumerate(EtapaHackaton.TIPOS, start=1)
    ]
    return EtapaHackaton.objects.bulk_create(etapas)
