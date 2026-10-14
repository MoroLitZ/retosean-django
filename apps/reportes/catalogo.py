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
def _impacto_del_cierre(reto):
    """Respuesta de la empresa sobre si aplico la solucion; vacio si el reto aun no tiene cierre."""
    cierre = getattr(reto, "cierre", None)
    return cierre.get_impacto_display() if cierre and cierre.impacto else ""


def _qs_retos(filtros):
    from apps.retos.models import Reto

    qs = Reto.objects.select_related("empresa", "facultad", "programa", "ecosistema", "cierre").prefetch_related(
        "postulaciones", "equipos_academicos", "archivos"
    )
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
        Columna("nivel_academico", "Nivel academico", lambda r: _texto(r.nivel_academico), 20),
        Columna("facultad", "Facultad", lambda r: _texto(r.facultad), 28),
        Columna("programa", "Programa", lambda r: _texto(r.programa), 28),
        Columna("ecosistema", "Ecosistema", lambda r: _texto(r.ecosistema), 24),
        Columna("convenio", "Convenio institucional",
                lambda r: "Si" if r.tiene_convenio_institucional else "No", 18),
        Columna("premios", "Premios", lambda r: _texto(r.premios), 40),
        Columna("criterios_evaluacion", "Criterios de evaluacion",
                lambda r: _texto(r.criterios_evaluacion), 44),
        Columna("fecha_inicio", "Inicio tentativo", lambda r: _fecha(r.fecha_inicio_tentativa), 18),
        Columna("fecha_fin", "Fin tentativo", lambda r: _fecha(r.fecha_fin_tentativa), 18),
        Columna("fecha_limite_postulacion", "Limite de postulacion",
                lambda r: _fecha(r.fecha_limite_postulacion), 20),
        Columna("fecha_aprobacion", "Fecha de aprobacion", lambda r: _fecha(r.fecha_aprobacion), 20),
        Columna("postulaciones", "Postulaciones", lambda r: r.postulaciones.count(), 14),
        Columna("equipos_academicos", "Equipos academicos",
                lambda r: r.equipos_academicos.count(), 18),
        Columna("archivos", "Archivos adjuntos", lambda r: r.archivos.count(), 16),
        Columna("comentarios_revision", "Comentarios de revision",
                lambda r: _texto(r.comentarios_revision), 40),
        Columna("nombre_corto", "Nombre corto", lambda r: _texto(r.nombre_corto), 22),
        Columna("uso_esperado", "Uso esperado de los resultados",
                lambda r: _texto(r.uso_esperado_resultados), 40),
        Columna("insumos_inicio", "Insumos al inicio", lambda r: _texto(r.insumos_inicio), 36),
        Columna("insumos_cierre", "Insumos al cierre", lambda r: _texto(r.insumos_cierre), 36),
        Columna("impacto", "La empresa aplico la solucion",
                lambda r: _impacto_del_cierre(r), 24),
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
        Columna("telefono", "Telefono", lambda p: _texto(p.estudiante.telefono), 16),
        Columna("programa", "Programa", lambda p: _texto(p.programa), 28),
        Columna("semestre", "Semestre", lambda p: _texto(p.semestre), 12),
        Columna("reto", "Reto", lambda p: _texto(p.reto.titulo), 36),
        Columna("empresa", "Empresa", lambda p: _texto(p.reto.empresa), 26),
        Columna("area_reto", "Area del reto", lambda p: _texto(p.reto.area), 22),
        Columna("estado", "Estado", lambda p: p.get_estado_display(), 20),
        Columna("motivacion", "Motivacion", lambda p: _texto(p.motivacion), 44),
        Columna("habilidades", "Habilidades", lambda p: _texto(p.habilidades), 36),
        Columna("hoja_de_vida", "Adjunto hoja de vida", lambda p: "Si" if p.hoja_de_vida else "No", 18),
        Columna("acepta_terminos", "Acepto los terminos", lambda p: "Si" if p.acepta_terminos else "No", 18),
        Columna("fecha", "Fecha de postulacion", lambda p: _fecha(p.fecha_postulacion), 20),
    ],
    roles=("ADMIN", "PROFESOR"),
)


# --------------------------------------------------------- EVALUACION
def _qs_evaluacion(filtros):
    from apps.evaluacion.models import Entregable

    qs = Entregable.objects.select_related("reto", "estudiante", "equipo").prefetch_related(
        "evaluaciones__profesor", "evaluaciones__rubrica", "historial_comentarios"
    )
    qs = acotar(qs, "fecha_entrega", filtros.get("desde"), filtros.get("hasta"))
    if filtros.get("facultad"):
        qs = qs.filter(reto__facultad_id=filtros["facultad"])
    if filtros.get("programa"):
        qs = qs.filter(reto__programa_id=filtros["programa"])
    if filtros.get("empresa"):
        qs = qs.filter(reto__empresa_id=filtros["empresa"])
    return qs.order_by("-fecha_entrega")


def _evaluacion_formal(entregable):
    """La primera evaluacion con rubrica registrada para este entregable, si hay."""
    evaluaciones = list(entregable.evaluaciones.all())
    return evaluaciones[0] if evaluaciones else None


REPORTE_EVALUACION = DefinicionReporte(
    clave="evaluacion",
    nombre="Evaluacion de entregables",
    descripcion="Entregables recibidos, su estado, nota y retroalimentacion.",
    construir_queryset=_qs_evaluacion,
    columnas=[
        Columna("reto", "Reto", lambda e: _texto(e.reto.titulo), 34),
        Columna("estudiante", "Estudiante",
                lambda e: e.estudiante.get_full_name() or e.estudiante.username, 30),
        Columna("correo", "Correo", lambda e: _texto(e.estudiante.email), 32),
        Columna("equipo", "Equipo", lambda e: _texto(e.equipo.nombre if e.equipo else ""), 22),
        Columna("titulo", "Entregable", lambda e: _texto(e.titulo), 30),
        Columna("es_final", "Entrega final", lambda e: "Si" if e.es_final else "No", 14),
        Columna("estado", "Estado", lambda e: e.get_estado_display(), 16),
        Columna("nota", "Nota", lambda e: _texto(e.nota), 10),
        Columna("puntaje_maximo", "Sobre", lambda e: _texto(e.puntaje_maximo), 10),
        Columna("comentario_estudiante", "Comentario del estudiante",
                lambda e: _texto(e.comentario_estudiante), 40),
        Columna("comentario", "Retroalimentacion", lambda e: _texto(e.comentario_profesor), 44),
        Columna("rubrica", "Rubrica usada",
                lambda e: _texto(getattr(_evaluacion_formal(e), "rubrica", None)), 26),
        Columna("evaluador", "Evaluado por",
                lambda e: _texto(getattr(_evaluacion_formal(e), "profesor", "")), 26),
        Columna("historial_comentarios", "N.o de comentarios",
                lambda e: len(e.historial_comentarios.all()), 16),
        Columna("calificacion_publicada", "Calificacion publicada al estudiante",
                lambda e: "Si" if e.calificacion_publicada else "No", 22),
        Columna("fecha_publicacion", "Fecha de publicacion",
                lambda e: _fecha(e.calificacion_publicada_en), 18),
        Columna("fecha", "Fecha de entrega", lambda e: _fecha(e.fecha_entrega), 18),
        Columna("actualizado_en", "Ultima actualizacion", lambda e: _fecha(e.actualizado_en), 18),
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


def _gastos_por_estado_y_categoria(presupuesto):
    """Recorre presupuesto.gastos.all() (ya prefetcheado) una sola vez y suma
    por estado/categoria en memoria, en vez de usar las propiedades del modelo
    (que harian una query .aggregate() nueva por cada presupuesto del reporte)."""
    from decimal import Decimal
    from apps.presupuesto.models import Gasto

    comprometido = Decimal("0.00")
    ejecutado = Decimal("0.00")
    por_categoria = {clave: Decimal("0.00") for clave, _ in Gasto.CATEGORIAS}
    for gasto in presupuesto.gastos.all():
        if gasto.estado == Gasto.COMPROMETIDO:
            comprometido += gasto.monto
        else:
            ejecutado += gasto.monto
        por_categoria[gasto.categoria] = por_categoria.get(gasto.categoria, Decimal("0.00")) + gasto.monto
    return comprometido, ejecutado, por_categoria


REPORTE_PRESUPUESTO = DefinicionReporte(
    clave="presupuesto",
    nombre="Ejecucion presupuestal",
    descripcion="Presupuesto asignado, comprometido, ejecutado y disponible por reto.",
    construir_queryset=_qs_presupuesto,
    columnas=[
        Columna("reto", "Reto", lambda p: _texto(p.reto.titulo), 36),
        Columna("empresa", "Empresa", lambda p: _texto(p.reto.empresa), 26),
        Columna("monto_total", "Presupuesto", lambda p: _texto(p.monto_total), 16),
        Columna("comprometido", "Comprometido",
                lambda p: _texto(_gastos_por_estado_y_categoria(p)[0]), 16),
        Columna("ejecutado", "Ejecutado",
                lambda p: _texto(_gastos_por_estado_y_categoria(p)[1]), 16),
        Columna("disponible", "Disponible",
                lambda p: _texto(p.monto_total - sum(_gastos_por_estado_y_categoria(p)[:2])), 16),
        Columna("porcentaje", "% asignado",
                lambda p: (
                    f"{(sum(_gastos_por_estado_y_categoria(p)[:2]) / p.monto_total * 100):.1f}%"
                    if p.monto_total else "0.0%"
                ), 14),
        Columna("gastos", "N.o de gastos", lambda p: len(p.gastos.all()), 14),
        Columna("gasto_refrigerios", "Refrigerios",
                lambda p: _texto(_gastos_por_estado_y_categoria(p)[2]["REFRIGERIOS"]), 16),
        Columna("gasto_materiales", "Materiales",
                lambda p: _texto(_gastos_por_estado_y_categoria(p)[2]["MATERIALES"]), 16),
        Columna("gasto_impresiones", "Impresiones",
                lambda p: _texto(_gastos_por_estado_y_categoria(p)[2]["IMPRESIONES"]), 16),
        Columna("gasto_espacios", "Espacios",
                lambda p: _texto(_gastos_por_estado_y_categoria(p)[2]["ESPACIOS"]), 16),
        Columna("gasto_otros", "Otros",
                lambda p: _texto(_gastos_por_estado_y_categoria(p)[2]["OTROS"]), 16),
        Columna("alerta_80", "Alerta 80% enviada",
                lambda p: "Si" if p.alerta_80_enviada else "No", 18),
    ],
)


# ------------------------------------------------------------ EMPRESAS
def _qs_empresas(filtros):
    from apps.empresas.models import Empresa

    qs = Empresa.objects.select_related("usuario").prefetch_related(
        "documentos", "contactos", "usuario__retos_empresa"
    )
    if filtros.get("estado_empresa"):
        qs = qs.filter(estado_validacion=filtros["estado_empresa"])
    return qs.order_by("razon_social")


def _motivos_para_no_operar(empresa):
    """Motivos por los que la empresa aun no puede participar; se calcula una vez por fila."""
    if not hasattr(empresa, "_motivos_reporte"):
        from apps.empresas.services import motivos_para_no_operar

        empresa._motivos_reporte = motivos_para_no_operar(empresa)
    return empresa._motivos_reporte


def _contacto_principal(empresa):
    contactos = list(empresa.contactos.all())
    principal = next((c for c in contactos if c.es_principal), None)
    return principal or (contactos[0] if contactos else None)


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
        Columna("retos_publicados", "Retos publicados",
                lambda e: len(e.usuario.retos_empresa.all()) if e.usuario else 0, 18),
        Columna("documentos", "Documentos cargados", lambda e: len(e.documentos.all()), 18),
        Columna("documentos_verificados", "Documentos verificados",
                lambda e: sum(1 for d in e.documentos.all() if d.estado == "VERIFICADO"), 20),
        Columna("contacto_nombre", "Contacto principal",
                lambda e: _texto(getattr(_contacto_principal(e), "nombre", "")), 28),
        Columna("contacto_cargo", "Cargo del contacto",
                lambda e: _texto(getattr(_contacto_principal(e), "cargo", "")), 22),
        Columna("contacto_telefono", "Telefono del contacto",
                lambda e: _texto(getattr(_contacto_principal(e), "telefono", "")), 18),
        Columna("correo", "Correo de contacto",
                lambda e: _texto(e.usuario.email if e.usuario else ""), 30),
        Columna("tipo_convocatoria", "Tipo de convocatoria", lambda e: e.get_tipo_convocatoria_display(), 20),
        Columna("area_responsable", "Area responsable", lambda e: _texto(e.area_responsable), 26),
        Columna("sitio_web", "Sitio web", lambda e: _texto(e.sitio_web), 30),
        Columna("estado_convenio", "Tramite del convenio", lambda e: e.get_estado_convenio_display(), 22),
        Columna("vence_convenio", "Vencimiento del convenio",
                lambda e: _fecha(e.fecha_vencimiento_convenio), 22),
        Columna("puede_participar", "Puede participar",
                lambda e: "No" if _motivos_para_no_operar(e) else "Si", 16),
        Columna("pendientes_documentales", "Pendientes para participar",
                lambda e: "; ".join(_motivos_para_no_operar(e)), 60),
        Columna("bloqueada", "Participacion bloqueada",
                lambda e: "Si" if e.participacion_bloqueada else "No", 20),
        Columna("motivo_bloqueo", "Motivo del bloqueo", lambda e: _texto(e.motivo_bloqueo), 36),
        Columna("observaciones", "Observaciones del admin",
                lambda e: _texto(e.observaciones_admin), 40),
        Columna("fecha_creacion", "Registrada el", lambda e: _fecha(e.fecha_creacion), 18),
        Columna("fecha_actualizacion", "Ultima actualizacion",
                lambda e: _fecha(e.fecha_ultima_actualizacion), 20),
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
