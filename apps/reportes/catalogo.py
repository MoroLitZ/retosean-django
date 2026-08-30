"""Catalogo de reportes disponibles (HU06).

Cada reporte declara su queryset y sus columnas. Las columnas son lo que el
usuario elige en el formulario: esa es la "seleccion de variables" del backlog.
Anadir un reporte nuevo es anadir una entrada aqui, sin tocar vistas ni PDF.
"""

from dataclasses import dataclass, field
from typing import Callable

from apps.dashboard.filters import acotar


@dataclass(frozen=True)
class Columna:
    clave: str
    etiqueta: str
    obtener: Callable
    ancho: int = 22


@dataclass(frozen=True)
class DefinicionReporte:
    clave: str
    nombre: str
    descripcion: str
    construir_queryset: Callable
    columnas: list = field(default_factory=list)
    # Roles que pueden generarlo; ADMIN siempre puede.
    roles: tuple = ("ADMIN",)

    def columnas_por_clave(self, claves):
        if not claves:
            return list(self.columnas)
        elegidas = set(claves)
        return [c for c in self.columnas if c.clave in elegidas]

    def opciones_de_columnas(self):
        return [(c.clave, c.etiqueta) for c in self.columnas]


def _texto(valor):
    return "" if valor is None else str(valor)


def _fecha(valor):
    return valor.strftime("%d/%m/%Y") if valor else ""


# --------------------------------------------------------------- RETOS
def _qs_retos(filtros):
    from apps.retos.models import Reto

    qs = Reto.objects.select_related("empresa", "facultad", "programa")
    qs = acotar(qs, "creado_en", filtros.get("desde"), filtros.get("hasta"))
    if filtros.get("estado"):
        qs = qs.filter(estado=filtros["estado"])
    if filtros.get("facultad"):
        qs = qs.filter(facultad_id=filtros["facultad"])
    if filtros.get("programa"):
        qs = qs.filter(programa_id=filtros["programa"])
    if filtros.get("empresa"):
        qs = qs.filter(empresa_id=filtros["empresa"])
    return qs.order_by("-creado_en")


REPORTE_RETOS = DefinicionReporte(
    clave="retos",
    nombre="Retos publicados",
    descripcion="Listado de retos con su empresa, estado y fechas clave.",
    construir_queryset=_qs_retos,
    columnas=[
        Columna("consecutivo", "N.o de reto", lambda r: _texto(r.consecutivo), 14),
        Columna("titulo", "Titulo", lambda r: _texto(r.titulo), 40),
        Columna("empresa", "Empresa", lambda r: _texto(r.empresa), 28),
        Columna("estado", "Estado", lambda r: r.get_estado_display(), 16),
        Columna("tipo", "Tipo", lambda r: r.get_tipo_display() if r.tipo else "", 16),
        Columna("area", "Area", lambda r: _texto(r.area), 22),
        Columna("facultad", "Facultad", lambda r: _texto(r.facultad), 28),
        Columna("programa", "Programa", lambda r: _texto(r.programa), 28),
        Columna("fecha_inicio", "Inicio tentativo", lambda r: _fecha(r.fecha_inicio_tentativa), 18),
        Columna("fecha_fin", "Fin tentativo", lambda r: _fecha(r.fecha_fin_tentativa), 18),
        Columna("postulaciones", "Postulaciones", lambda r: r.postulaciones.count(), 14),
        Columna("creado_en", "Creado", lambda r: _fecha(r.creado_en), 16),
    ],
)


# ------------------------------------------------------- PARTICIPACION
def _qs_participacion(filtros):
    from apps.participaciones.models import Postulacion

    qs = Postulacion.objects.select_related("reto", "reto__empresa", "estudiante")
    qs = acotar(qs, "fecha_postulacion", filtros.get("desde"), filtros.get("hasta"))
    if filtros.get("estado_postulacion"):
        qs = qs.filter(estado=filtros["estado_postulacion"])
    if filtros.get("facultad"):
        qs = qs.filter(reto__facultad_id=filtros["facultad"])
    if filtros.get("programa"):
        qs = qs.filter(reto__programa_id=filtros["programa"])
    if filtros.get("empresa"):
        qs = qs.filter(reto__empresa_id=filtros["empresa"])
    return qs.order_by("-fecha_postulacion")


REPORTE_PARTICIPACION = DefinicionReporte(
    clave="participacion",
    nombre="Participacion estudiantil",
    descripcion="Postulaciones de estudiantes a retos, con su estado y programa.",
    construir_queryset=_qs_participacion,
    columnas=[
        Columna("estudiante", "Estudiante",
                lambda p: p.estudiante.get_full_name() or p.estudiante.username, 30),
        Columna("correo", "Correo", lambda p: _texto(p.estudiante.email), 32),
        Columna("programa", "Programa", lambda p: _texto(p.programa), 28),
        Columna("semestre", "Semestre", lambda p: _texto(p.semestre), 12),
        Columna("reto", "Reto", lambda p: _texto(p.reto.titulo), 36),
        Columna("empresa", "Empresa", lambda p: _texto(p.reto.empresa), 26),
        Columna("estado", "Estado", lambda p: p.get_estado_display(), 20),
        Columna("fecha", "Fecha de postulacion", lambda p: _fecha(p.fecha_postulacion), 20),
    ],
    roles=("ADMIN", "PROFESOR"),
)


# --------------------------------------------------------- EVALUACION
def _qs_evaluacion(filtros):
    from apps.evaluacion.models import Entregable

    qs = Entregable.objects.select_related("reto", "estudiante", "equipo")
    qs = acotar(qs, "fecha_entrega", filtros.get("desde"), filtros.get("hasta"))
    if filtros.get("facultad"):
        qs = qs.filter(reto__facultad_id=filtros["facultad"])
    if filtros.get("programa"):
        qs = qs.filter(reto__programa_id=filtros["programa"])
    if filtros.get("empresa"):
        qs = qs.filter(reto__empresa_id=filtros["empresa"])
    return qs.order_by("-fecha_entrega")


REPORTE_EVALUACION = DefinicionReporte(
    clave="evaluacion",
    nombre="Evaluacion de entregables",
    descripcion="Entregables recibidos, su estado, nota y retroalimentacion.",
    construir_queryset=_qs_evaluacion,
    columnas=[
        Columna("reto", "Reto", lambda e: _texto(e.reto.titulo), 34),
        Columna("estudiante", "Estudiante",
                lambda e: e.estudiante.get_full_name() or e.estudiante.username, 30),
        Columna("equipo", "Equipo", lambda e: _texto(e.equipo.nombre if e.equipo else ""), 22),
        Columna("titulo", "Entregable", lambda e: _texto(e.titulo), 30),
        Columna("es_final", "Entrega final", lambda e: "Si" if e.es_final else "No", 14),
        Columna("estado", "Estado", lambda e: e.get_estado_display(), 16),
        Columna("nota", "Nota", lambda e: _texto(e.nota), 10),
        Columna("puntaje_maximo", "Sobre", lambda e: _texto(e.puntaje_maximo), 10),
        Columna("comentario", "Retroalimentacion", lambda e: _texto(e.comentario_profesor), 44),
        Columna("fecha", "Fecha de entrega", lambda e: _fecha(e.fecha_entrega), 18),
    ],
    roles=("ADMIN", "PROFESOR"),
)


# --------------------------------------------------------- PRESUPUESTO
def _qs_presupuesto(filtros):
    from apps.presupuesto.models import Presupuesto

    qs = Presupuesto.objects.select_related("reto", "reto__empresa").prefetch_related("gastos")
    qs = acotar(qs, "creado_en", filtros.get("desde"), filtros.get("hasta"))
    if filtros.get("empresa"):
        qs = qs.filter(reto__empresa_id=filtros["empresa"])
    if filtros.get("facultad"):
        qs = qs.filter(reto__facultad_id=filtros["facultad"])
    return qs.order_by("-creado_en")


REPORTE_PRESUPUESTO = DefinicionReporte(
    clave="presupuesto",
    nombre="Ejecucion presupuestal",
    descripcion="Presupuesto asignado, ejecutado y disponible por reto.",
    construir_queryset=_qs_presupuesto,
    columnas=[
        Columna("reto", "Reto", lambda p: _texto(p.reto.titulo), 36),
        Columna("empresa", "Empresa", lambda p: _texto(p.reto.empresa), 26),
        Columna("monto_total", "Presupuesto", lambda p: _texto(p.monto_total), 16),
        Columna("ejecutado", "Ejecutado", lambda p: _texto(p.monto_ejecutado), 16),
        Columna("disponible", "Disponible", lambda p: _texto(p.monto_disponible), 16),
        Columna("porcentaje", "% ejecutado",
                lambda p: f"{p.porcentaje_ejecutado:.1f}%", 14),
        Columna("gastos", "N.o de gastos", lambda p: p.gastos.count(), 14),
    ],
)


# ------------------------------------------------------------ EMPRESAS
def _qs_empresas(filtros):
    from apps.empresas.models import Empresa

    qs = Empresa.objects.select_related("usuario").prefetch_related("documentos")
    if filtros.get("estado_empresa"):
        qs = qs.filter(estado_validacion=filtros["estado_empresa"])
    return qs.order_by("razon_social")


REPORTE_EMPRESAS = DefinicionReporte(
    clave="empresas",
    nombre="Empresas aliadas",
    descripcion="Empresas registradas, su estado de validacion y documentacion.",
    construir_queryset=_qs_empresas,
    columnas=[
        Columna("nit", "NIT", lambda e: _texto(e.nit), 18),
        Columna("razon_social", "Razon social", lambda e: _texto(e.razon_social), 36),
        Columna("sector", "Sector", lambda e: _texto(e.sector_industrial), 26),
        Columna("estado", "Estado de validacion",
                lambda e: e.get_estado_validacion_display(), 22),
        Columna("listas", "Listas restrictivas",
                lambda e: e.get_estado_listas_restrictivas_display(), 20),
        Columna("convenio", "Renuncio a convenio",
                lambda e: "Si" if e.renuncio_a_convenio else "No", 18),
        Columna("documentos", "Documentos cargados", lambda e: e.documentos.count(), 18),
        Columna("correo", "Correo de contacto",
                lambda e: _texto(e.usuario.email if e.usuario else ""), 30),
    ],
)


REPORTES = {
    definicion.clave: definicion
    for definicion in [
        REPORTE_RETOS,
        REPORTE_PARTICIPACION,
        REPORTE_EVALUACION,
        REPORTE_PRESUPUESTO,
        REPORTE_EMPRESAS,
    ]
}


def disponibles_para(user):
    """Reportes que este usuario puede generar."""
    from apps.usuarios.roles import es_admin, rol_de

    if es_admin(user):
        return list(REPORTES.values())
    rol = rol_de(user)
    return [d for d in REPORTES.values() if rol in d.roles]


def obtener(clave):
    return REPORTES.get(clave)
