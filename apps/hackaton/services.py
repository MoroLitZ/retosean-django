"""Logica de negocio del hackathon (HU09): etapas, ranking y publicacion."""

from decimal import Decimal

from django.db import transaction
from django.db.models import Avg, Count, Max, Sum
from django.urls import reverse
from django.utils import timezone

from apps.notificaciones.services import notificar_muchos
from apps.seguimiento.services import MaquinaEstados, registrar_cambio_estado

from .models import EtapaHackaton, Hackathon, InscripcionHackaton, Jurado, VotacionHackaton


# Inscripcion de un equipo al hackathon. El gestor puede corregir su decision
# (Aceptada <-> Rechazada); cada cambio queda en el historial.
MAQUINA_INSCRIPCION = MaquinaEstados(
    "la inscripcion",
    {
        "PENDIENTE": {"ACEPTADA", "RECHAZADA"},
        "ACEPTADA": {"RECHAZADA"},
        "RECHAZADA": {"ACEPTADA"},
    },
    dict(InscripcionHackaton.ESTADOS),
)


def inscripciones_abiertas(hackathon):
    if hackathon.estado not in {"publicado", "en_curso"}:
        return False
    etapa = hackathon.etapas.filter(tipo="inscripcion").first()
    # Sin etapa de inscripcion configurada no bloqueamos: el hackathon
    # publicado se considera abierto.
    return etapa.vigente if etapa and (etapa.inicia_en or etapa.termina_en) else True


def votacion_abierta(hackathon):
    # Con los resultados ya publicados no se vuelve a votar, aunque el estado
    # se reabriera por otra via (HCK-04).
    if hackathon.resultados_publicados:
        return False
    if hackathon.estado not in {"publicado", "en_curso"}:
        return False
    etapa = hackathon.etapas.filter(tipo="presentacion").first()
    # Al reves que en inscripciones, aqui se falla cerrado: sin etapa de
    # presentacion fechada y vigente no se vota (HCK-05), porque el ganador
    # sale de esta ventana.
    return bool(etapa) and etapa.vigente


def es_jurado(hackathon, usuario):
    return Jurado.objects.filter(hackathon=hackathon, usuario=usuario).exists()


def ranking(hackathon):
    """Clasificacion de equipos por promedio de puntaje.

    Solo puntuan los equipos con inscripcion aceptada: uno rechazado despues de
    votar sale de la tabla y no puede ganar (HCK-02). Los aceptados sin votos
    cierran la tabla en cero para no desaparecer del ranking ni del informe
    (HCK-10).

    Se usa el promedio y no la suma para no penalizar a un equipo al que
    votaron menos jurados. Desempate: mas votos primero, luego alfabetico.
    """
    votados = {
        fila["equipo_id"]: fila
        for fila in (
            VotacionHackaton.objects
            .filter(hackathon=hackathon)
            .values("equipo_id")
            .annotate(
                promedio=Avg("puntaje"),
                total=Sum("puntaje"),
                votos=Count("id"),
            )
        )
    }
    filas = []
    aceptadas = (
        InscripcionHackaton.objects
        .filter(hackathon=hackathon, estado="ACEPTADA")
        .select_related("equipo")
    )
    for inscripcion in aceptadas:
        voto = votados.get(inscripcion.equipo_id)
        filas.append({
            "equipo_id": inscripcion.equipo_id,
            "equipo": inscripcion.equipo.nombre,
            "promedio": voto["promedio"] if voto else Decimal("0"),
            "total": voto["total"] if voto else Decimal("0"),
            "votos": voto["votos"] if voto else 0,
        })
    filas.sort(key=lambda fila: (-fila["promedio"], -fila["votos"], fila["equipo"]))
    return [dict(fila, posicion=posicion) for posicion, fila in enumerate(filas, start=1)]


def ganadores(tabla):
    """Equipo(s) en el primer puesto del ranking.

    Devuelve varios cuando hay empate total (mismo promedio y mismo numero de
    votos); antes deshacia el empate por orden alfabetico y coronaba un unico
    ganador arbitrario (HCK-06). Lista vacia si nadie ha votado.
    """
    if not tabla or not tabla[0]["votos"]:
        return []
    primero = tabla[0]
    return [
        fila for fila in tabla
        if fila["promedio"] == primero["promedio"] and fila["votos"] == primero["votos"]
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


def publicar_resultados(hackathon, usuario, comentario="Resultados publicados."):
    """Marca los resultados como publicos y notifica a los equipos."""
    anterior = hackathon.estado
    hackathon.resultados_publicados = True
    if hackathon.estado != "finalizado":
        hackathon.estado = "finalizado"
    hackathon.save(update_fields=["resultados_publicados", "estado", "actualizado_en"])
    if anterior != hackathon.estado:
        registrar_cambio_estado(hackathon, anterior, hackathon.estado, usuario, comentario)

    tabla = ranking(hackathon)
    campeones = ganadores(tabla)
    mensaje = f"Ya estan publicados los resultados del hackathon '{hackathon.reto.titulo}'."
    if len(campeones) == 1:
        mensaje += f" Equipo ganador: {campeones[0]['equipo']}."
    elif campeones:
        nombres = " y ".join(fila["equipo"] for fila in campeones)
        mensaje += f" Equipos ganadores en empate: {nombres}."

    notificar_muchos(
        _participantes(hackathon), "HACKATON_RESULTADOS",
        mensaje=mensaje,
        link=reverse("hackaton:ranking", kwargs={"pk": hackathon.pk}),
        clave_dedupe=f"hackaton:{hackathon.pk}:resultados",
    )
    return tabla


def fecha_de_cierre(hackathon):
    """Momento en que se publican solos los resultados: el inicio de la etapa de
    premiacion o, si no tiene fecha, el fin de la ultima etapa con fecha."""
    premiacion = hackathon.etapas.filter(tipo="premiacion", inicia_en__isnull=False).order_by("inicia_en").first()
    if premiacion:
        return premiacion.inicia_en
    return hackathon.etapas.aggregate(fin=Max("termina_en"))["fin"]


def avanzar_por_calendario(hackathon, ahora=None):
    """Avanza el hackathon segun sus etapas (HU09). Devuelve lo que hizo.

    Publicado -> En curso al empezar la etapa de desarrollo; al llegar el
    cierre, finaliza y publica resultados y ganadores automaticamente. Es
    idempotente: lo usan el comando programado y la vista de detalle. Corre en
    una transaccion con la fila bloqueada (`select_for_update`) y decide sobre
    el estado recien leido de la BD, para que dos llamadas simultaneas no
    dupliquen transiciones ni sobrescriban un estado mas avanzado (HCK-01).
    """
    ahora = ahora or timezone.now()
    cambios = []
    with transaction.atomic():
        bloqueado = Hackathon.objects.select_for_update().get(pk=hackathon.pk)
        # La instancia del llamador puede estar obsoleta (la cargo otra
        # peticion antes): se trabaja con el estado leido bajo bloqueo y la
        # instancia del llamador queda sincronizada para las vistas.
        hackathon.estado = bloqueado.estado
        hackathon.resultados_publicados = bloqueado.resultados_publicados
        if hackathon.estado == "publicado" and hackathon.etapas.filter(
            tipo="desarrollo", inicia_en__isnull=False, inicia_en__lte=ahora
        ).exists():
            hackathon.estado = "en_curso"
            hackathon.save(update_fields=["estado", "actualizado_en"])
            registrar_cambio_estado(
                hackathon, "publicado", "en_curso", None, "Avance automatico: inicio de la etapa de desarrollo."
            )
            cambios.append("en_curso")

        cierre = fecha_de_cierre(hackathon)
        if (hackathon.estado in {"publicado", "en_curso"} and not hackathon.resultados_publicados
                and cierre and cierre <= ahora):
            publicar_resultados(hackathon, None, "Cierre automatico segun el calendario: resultados publicados.")
            cambios.append("resultados_publicados")
    return cambios


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
