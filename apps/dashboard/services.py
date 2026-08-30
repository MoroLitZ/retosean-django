"""Calculo de indicadores del dashboard (HU05).

Toda la agregacion vive aqui para que las vistas queden delgadas y para que
los reportes (HU06) puedan reutilizar los mismos numeros.
"""

from django.db.models import Avg, Count, Q
from django.db.models.functions import TruncMonth

from .filters import acotar

# Paleta consistente para todas las graficas del panel.
COLORES = [
    "#2563eb", "#16a34a", "#f59e0b", "#dc2626",
    "#7c3aed", "#0891b2", "#65a30d", "#db2777",
]


def _serie(pares):
    """Convierte [(etiqueta, valor), ...] al formato que consume Chart.js."""
    etiquetas = [str(etiqueta) for etiqueta, _ in pares]
    valores = [valor for _, valor in pares]
    return {
        "labels": etiquetas,
        "data": valores,
        "colors": [COLORES[i % len(COLORES)] for i in range(len(valores))],
    }


def _retos_filtrados(desde, hasta, facultad_id=None, programa_id=None):
    from apps.retos.models import Reto

    retos = Reto.objects.all()
    retos = acotar(retos, "creado_en", desde, hasta)
    if facultad_id:
        retos = retos.filter(facultad_id=facultad_id)
    if programa_id:
        retos = retos.filter(programa_id=programa_id)
    return retos


# ---------------------------------------------------------------- ADMIN
def kpis_admin(desde, hasta, facultad_id=None, programa_id=None):
    from apps.empresas.models import Empresa
    from apps.evaluacion.models import Entregable
    from apps.participaciones.models import Postulacion
    from apps.usuarios.models import Usuario

    retos = _retos_filtrados(desde, hasta, facultad_id, programa_id)
    postulaciones = Postulacion.objects.filter(reto__in=retos)

    return {
        "retos_total": retos.count(),
        "retos_activos": retos.filter(estado__in=["aprobado", "en_curso"]).count(),
        "retos_finalizados": retos.filter(estado="finalizado").count(),
        "retos_en_revision": retos.filter(estado="en_revision").count(),
        "empresas_total": Empresa.objects.count(),
        "empresas_verificadas": Empresa.objects.filter(estado_validacion="VERIFICADA").count(),
        "estudiantes_total": Usuario.objects.filter(rol="ESTUDIANTE", is_active=True).count(),
        "profesores_total": Usuario.objects.filter(rol="PROFESOR", is_active=True).count(),
        "postulaciones_total": postulaciones.count(),
        "postulaciones_aceptadas": postulaciones.filter(estado="ACEPTADA").count(),
        "entregables_total": Entregable.objects.filter(reto__in=retos).count(),
        "entregables_calificados": Entregable.objects.filter(
            reto__in=retos, estado="CALIFICADO"
        ).count(),
    }


def series_admin(desde, hasta, facultad_id=None, programa_id=None):
    from apps.retos.models import Reto
    from apps.seguimiento.models import IntegracionAcademica

    retos = _retos_filtrados(desde, hasta, facultad_id, programa_id)

    # Retos por estado (torta)
    etiquetas_estado = dict(Reto.ESTADO_CHOICES)
    por_estado = [
        (etiquetas_estado.get(fila["estado"], fila["estado"]), fila["total"])
        for fila in retos.values("estado").annotate(total=Count("id")).order_by("-total")
    ]

    # Retos por facultad (barras)
    por_facultad = [
        (fila["facultad__nombre"] or "Sin facultad", fila["total"])
        for fila in retos.values("facultad__nombre")
        .annotate(total=Count("id"))
        .order_by("-total")[:8]
    ]

    # Retos creados por mes (linea de tiempo)
    por_mes = [
        (fila["mes"].strftime("%m/%Y"), fila["total"])
        for fila in retos.annotate(mes=TruncMonth("creado_en"))
        .values("mes")
        .annotate(total=Count("id"))
        .order_by("mes")
        if fila["mes"]
    ]

    # Integraciones academicas por estado
    integraciones = IntegracionAcademica.objects.filter(reto__in=retos)
    etiquetas_integracion = dict(IntegracionAcademica.ESTADOS)
    por_integracion = [
        (etiquetas_integracion.get(fila["estado"], fila["estado"]), fila["total"])
        for fila in integraciones.values("estado").annotate(total=Count("id")).order_by("-total")
    ]

    return {
        "retos_por_estado": _serie(por_estado),
        "retos_por_facultad": _serie(por_facultad),
        "retos_por_mes": _serie(por_mes),
        "integraciones_por_estado": _serie(por_integracion),
    }


# -------------------------------------------------------------- EMPRESA
def kpis_empresa(usuario, desde, hasta):
    from apps.evaluacion.models import Entregable
    from apps.participaciones.models import Postulacion
    from apps.retos.models import Reto
    from apps.seguimiento.models import IntegracionAcademica

    retos = acotar(Reto.objects.filter(empresa=usuario), "creado_en", desde, hasta)
    postulaciones = Postulacion.objects.filter(reto__in=retos)

    return {
        "retos_total": retos.count(),
        "retos_activos": retos.filter(estado__in=["aprobado", "en_curso"]).count(),
        "retos_finalizados": retos.filter(estado="finalizado").count(),
        "retos_en_revision": retos.filter(estado="en_revision").count(),
        # Antes este KPI contaba integraciones academicas y se rotulaba
        # "postulaciones"; ahora cuenta lo que dice.
        "postulaciones_total": postulaciones.count(),
        "postulaciones_pendientes": postulaciones.filter(estado="PENDIENTE").count(),
        "postulaciones_aceptadas": postulaciones.filter(estado="ACEPTADA").count(),
        "integraciones_total": IntegracionAcademica.objects.filter(reto__in=retos).count(),
        "entregables_total": Entregable.objects.filter(reto__in=retos).count(),
    }


def series_empresa(usuario, desde, hasta):
    from apps.retos.models import Reto

    retos = acotar(Reto.objects.filter(empresa=usuario), "creado_en", desde, hasta)
    etiquetas_estado = dict(Reto.ESTADO_CHOICES)
    por_estado = [
        (etiquetas_estado.get(fila["estado"], fila["estado"]), fila["total"])
        for fila in retos.values("estado").annotate(total=Count("id")).order_by("-total")
    ]
    por_mes = [
        (fila["mes"].strftime("%m/%Y"), fila["total"])
        for fila in retos.annotate(mes=TruncMonth("creado_en"))
        .values("mes").annotate(total=Count("id")).order_by("mes")
        if fila["mes"]
    ]
    return {
        "retos_por_estado": _serie(por_estado),
        "retos_por_mes": _serie(por_mes),
    }


# ------------------------------------------------------------- PROFESOR
def kpis_profesor(usuario, desde, hasta):
    from apps.evaluacion.models import Entregable
    from apps.seguimiento.models import IntegracionAcademica
    from apps.usuarios.models import Usuario

    integraciones = acotar(
        IntegracionAcademica.objects.filter(profesor=usuario), "creado_en", desde, hasta
    )
    retos_ids = list(integraciones.values_list("reto_id", flat=True))
    entregables = Entregable.objects.filter(reto_id__in=retos_ids)

    return {
        "cursos_activos": integraciones.filter(estado__in=["aprobada", "publicada"]).count(),
        "retos_asignados": integraciones.count(),
        "integraciones_en_revision": integraciones.filter(estado="en_revision").count(),
        "estudiantes_total": Usuario.objects.filter(
            equipos_academicos_estudiante__reto_id__in=retos_ids
        ).distinct().count(),
        "entregables_total": entregables.count(),
        "evaluaciones_pendientes": entregables.filter(
            estado__in=["ENVIADO", "EN_REVISION"]
        ).count(),
        "nota_promedio": entregables.filter(nota__isnull=False).aggregate(
            promedio=Avg("nota")
        )["promedio"],
    }


def series_profesor(usuario, desde, hasta):
    from apps.evaluacion.models import Entregable
    from apps.seguimiento.models import IntegracionAcademica

    integraciones = acotar(
        IntegracionAcademica.objects.filter(profesor=usuario), "creado_en", desde, hasta
    )
    retos_ids = list(integraciones.values_list("reto_id", flat=True))
    entregables = Entregable.objects.filter(reto_id__in=retos_ids)

    etiquetas = dict(Entregable.ESTADOS)
    por_estado = [
        (etiquetas.get(fila["estado"], fila["estado"]), fila["total"])
        for fila in entregables.values("estado").annotate(total=Count("id")).order_by("-total")
    ]
    por_reto = [
        (fila["reto__titulo"] or "Sin titulo", fila["total"])
        for fila in entregables.values("reto__titulo")
        .annotate(total=Count("id")).order_by("-total")[:8]
    ]
    return {
        "entregables_por_estado": _serie(por_estado),
        "entregables_por_reto": _serie(por_reto),
    }


# ------------------------------------------------------------ ESTUDIANTE
def kpis_estudiante(usuario, desde, hasta):
    from apps.evaluacion.models import Entregable
    from apps.participaciones.models import Postulacion, RetoFavorito
    from apps.retos.models import Reto

    postulaciones = acotar(
        Postulacion.objects.filter(estudiante=usuario), "fecha_postulacion", desde, hasta
    )
    entregables = Entregable.objects.filter(estudiante=usuario)

    return {
        "retos_disponibles": Reto.objects.filter(estado__in=["aprobado", "en_curso"]).count(),
        "favoritos_total": RetoFavorito.objects.filter(usuario=usuario).count(),
        "postulaciones_total": postulaciones.count(),
        "postulaciones_pendientes": postulaciones.filter(estado="PENDIENTE").count(),
        "postulaciones_aceptadas": postulaciones.filter(estado="ACEPTADA").count(),
        "entregables_total": entregables.count(),
        "entregables_pendientes": entregables.filter(
            estado__in=["ENVIADO", "EN_REVISION"]
        ).count(),
        "nota_promedio": entregables.filter(nota__isnull=False).aggregate(
            promedio=Avg("nota")
        )["promedio"],
    }


def series_estudiante(usuario, desde, hasta):
    from apps.evaluacion.models import Entregable
    from apps.participaciones.models import Postulacion

    postulaciones = acotar(
        Postulacion.objects.filter(estudiante=usuario), "fecha_postulacion", desde, hasta
    )
    etiquetas = dict(Postulacion.ESTADOS)
    por_estado = [
        (etiquetas.get(fila["estado"], fila["estado"]), fila["total"])
        for fila in postulaciones.values("estado").annotate(total=Count("id")).order_by("-total")
    ]

    notas = [
        (entregable.titulo[:24], float(entregable.nota))
        for entregable in Entregable.objects.filter(
            estudiante=usuario, nota__isnull=False
        ).order_by("-fecha_entrega")[:8]
    ]
    return {
        "postulaciones_por_estado": _serie(por_estado),
        "notas_recientes": _serie(list(reversed(notas))),
    }


def opciones_de_filtro():
    """Facultades y programas disponibles para los selectores del panel."""
    from apps.academico.models import Facultad, Programa

    return {
        "facultades": Facultad.objects.order_by("nombre"),
        "programas": Programa.objects.select_related("facultad").order_by("nombre"),
    }
