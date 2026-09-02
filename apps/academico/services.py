"""Emision y renderizado de certificados y portafolio (HU16)."""

import io

from django.core.files.base import ContentFile
from django.urls import reverse
from django.utils import timezone
from reportlab.lib import colors
from reportlab.lib.pagesizes import A4, landscape
from reportlab.lib.styles import ParagraphStyle, getSampleStyleSheet
from reportlab.lib.units import mm
from reportlab.platypus import (
    Paragraph,
    SimpleDocTemplate,
    Spacer,
    Table,
    TableStyle,
)

from apps.notificaciones.services import notificar_muchos

from .models import Certificado

NEGRO = colors.HexColor("#111827")
AMBAR = colors.HexColor("#f59e0b")
GRIS = colors.HexColor("#6b7280")


# ------------------------------------------------------------- EMISION
def _participantes_del_reto(reto):
    """Devuelve [(usuario, rol_participacion), ...] elegibles para certificado."""
    from apps.evaluacion.models import Entregable
    from apps.hackaton.models import Jurado
    from apps.participaciones.models import Postulacion
    from apps.retos.models import EquipoRetoAcademico
    from apps.seguimiento.models import IntegracionAcademica
    from apps.usuarios.models import Usuario

    elegibles = {}

    # Estudiantes aceptados, incorporados a un equipo academico o que entregaron evidencias.
    for postulacion in Postulacion.objects.filter(reto=reto, estado="ACEPTADA").select_related("estudiante"):
        elegibles[postulacion.estudiante_id] = (postulacion.estudiante, "ESTUDIANTE")
    for estudiante in Usuario.objects.filter(
        equipos_academicos_estudiante__reto=reto
    ).distinct():
        elegibles.setdefault(estudiante.pk, (estudiante, "ESTUDIANTE"))
    for entregable in Entregable.objects.filter(reto=reto).select_related("estudiante"):
        elegibles.setdefault(entregable.estudiante_id, (entregable.estudiante, "ESTUDIANTE"))

    # Docentes con integracion vinculada al reto.
    for integracion in IntegracionAcademica.objects.filter(
        reto=reto
    ).select_related("profesor"):
        if integracion.profesor:
            elegibles[integracion.profesor_id] = (integracion.profesor, "PROFESOR")

    # Jurados del hackathon, si lo hubo.
    for jurado in Jurado.objects.filter(hackathon__reto=reto, usuario__isnull=False).select_related("usuario"):
        elegibles.setdefault(jurado.usuario_id, (jurado.usuario, "JURADO"))

    # La empresa que publico el reto.
    if reto.empresa_id:
        elegibles.setdefault(reto.empresa_id, (reto.empresa, "EMPRESA"))

    del EquipoRetoAcademico
    return list(elegibles.values())


def emitir_certificados_de_reto(reto, emitido_por=None, horas=None):
    """Emite los certificados del reto. Idempotente: no duplica los existentes."""
    if reto.estado != "finalizado":
        return []

    existentes = set(
        Certificado.objects.filter(reto=reto).values_list("usuario_id", "rol_participacion")
    )
    nuevos = [
        Certificado(
            reto=reto, usuario=usuario, rol_participacion=rol,
            emitido_por=emitido_por, horas=horas,
        )
        for usuario, rol in _participantes_del_reto(reto)
        if (usuario.pk, rol) not in existentes
    ]
    if not nuevos:
        return []

    Certificado.objects.bulk_create(nuevos, ignore_conflicts=True)
    creados = list(Certificado.objects.filter(
        reto=reto, usuario__in=[c.usuario for c in nuevos]
    ))

    notificar_muchos(
        [c.usuario for c in creados], "CERTIFICADO_DISPONIBLE",
        mensaje=(
            f"Ya puedes descargar tu certificado de participacion en el reto "
            f"'{reto.titulo}'."
        ),
        link=reverse("academico:certificados"),
        clave_dedupe=f"certificado:{reto.pk}",
    )
    return creados


# ---------------------------------------------------------------- PDF
def _marco(canvas, documento):
    """Borde decorativo del certificado."""
    canvas.saveState()
    ancho, alto = documento.pagesize
    canvas.setStrokeColor(NEGRO)
    canvas.setLineWidth(2.5)
    canvas.rect(12 * mm, 12 * mm, ancho - 24 * mm, alto - 24 * mm)
    canvas.setStrokeColor(AMBAR)
    canvas.setLineWidth(0.8)
    canvas.rect(16 * mm, 16 * mm, ancho - 32 * mm, alto - 32 * mm)
    canvas.restoreState()


def generar_pdf_certificado(certificado):
    buffer = io.BytesIO()
    documento = SimpleDocTemplate(
        buffer, pagesize=landscape(A4),
        leftMargin=28 * mm, rightMargin=28 * mm,
        topMargin=26 * mm, bottomMargin=24 * mm,
        title=f"Certificado {certificado.codigo_corto}", author="Universidad EAN",
    )
    base = getSampleStyleSheet()

    def estilo(nombre, **kwargs):
        return ParagraphStyle(nombre, parent=base["Normal"], alignment=1, **kwargs)

    reto = certificado.reto
    papel = certificado.get_rol_participacion_display().lower()

    elementos = [
        Paragraph("retos<font color='#f59e0b'>ean</font>",
                  estilo("Marca", fontSize=17, textColor=NEGRO, spaceAfter=2,
                         fontName="Helvetica-Bold")),
        Paragraph("Universidad EAN", estilo("Sub", fontSize=9.5, textColor=GRIS, spaceAfter=16)),
        Paragraph("CERTIFICADO DE PARTICIPACION",
                  estilo("Titulo", fontSize=22, textColor=NEGRO, spaceAfter=18,
                         fontName="Helvetica-Bold")),
        Paragraph("Se certifica que", estilo("Intro", fontSize=11, textColor=GRIS, spaceAfter=8)),
        Paragraph(certificado.nombre_participante,
                  estilo("Nombre", fontSize=24, textColor=NEGRO, spaceAfter=12,
                         fontName="Helvetica-Bold")),
        Paragraph(
            f"participo en calidad de <b>{papel}</b> en el reto empresarial",
            estilo("Rol", fontSize=11, textColor=GRIS, spaceAfter=8),
        ),
        Paragraph(f'"{reto.titulo}"',
                  estilo("Reto", fontSize=15, textColor=NEGRO, spaceAfter=8,
                         fontName="Helvetica-Bold")),
        Paragraph(
            f"propuesto por {reto.empresa.get_full_name() or reto.empresa.username}"
            + (f", con una dedicacion de {certificado.horas} horas" if certificado.horas else "")
            + ".",
            estilo("Empresa", fontSize=11, textColor=GRIS, spaceAfter=22),
        ),
    ]

    pie = Table(
        [[
            Paragraph(f"Emitido el {timezone.localtime(certificado.emitido_en):%d/%m/%Y}",
                      estilo("Pie1", fontSize=8.5, textColor=GRIS)),
            Paragraph(f"Codigo de verificacion<br/><b>{certificado.codigo_verificacion}</b>",
                      estilo("Pie2", fontSize=8.5, textColor=GRIS)),
        ]],
        colWidths=[documento.width / 2] * 2,
    )
    pie.setStyle(TableStyle([
        ("VALIGN", (0, 0), (-1, -1), "MIDDLE"),
        ("LINEABOVE", (0, 0), (-1, 0), 0.6, colors.HexColor("#d1d5db")),
        ("TOPPADDING", (0, 0), (-1, -1), 8),
    ]))
    elementos.extend([Spacer(1, 10), pie])

    documento.build(elementos, onFirstPage=_marco, onLaterPages=_marco)
    return buffer.getvalue()


def obtener_pdf_certificado(certificado):
    """Devuelve el PDF, generandolo y cacheandolo la primera vez."""
    if certificado.archivo:
        try:
            with certificado.archivo.open("rb") as archivo:
                return archivo.read()
        except (FileNotFoundError, ValueError):
            pass  # El archivo cacheado ya no esta: lo regeneramos.

    contenido = generar_pdf_certificado(certificado)
    certificado.archivo.save(
        f"certificado-{certificado.codigo_corto}.pdf", ContentFile(contenido), save=True
    )
    return contenido


# ---------------------------------------------------------- PORTAFOLIO
def generar_pdf_portafolio(usuario, certificados, entregables):
    """Portafolio institucional del estudiante en PDF."""
    buffer = io.BytesIO()
    documento = SimpleDocTemplate(
        buffer, pagesize=A4,
        leftMargin=18 * mm, rightMargin=18 * mm,
        topMargin=18 * mm, bottomMargin=18 * mm,
        title=f"Portafolio de {usuario.get_full_name() or usuario.username}",
        author="Universidad EAN",
    )
    base = getSampleStyleSheet()
    titulo = ParagraphStyle("PTitulo", parent=base["Title"], fontSize=18, textColor=NEGRO, alignment=0)
    meta = ParagraphStyle("PMeta", parent=base["Normal"], fontSize=9, textColor=GRIS)
    seccion = ParagraphStyle(
        "PSeccion", parent=base["Heading2"], fontSize=12.5, textColor=NEGRO, spaceBefore=14
    )
    celda = ParagraphStyle("PCelda", parent=base["Normal"], fontSize=8.5, leading=11)

    elementos = [
        Paragraph("retos<font color='#f59e0b'>ean</font>", meta),
        Paragraph(usuario.get_full_name() or usuario.username, titulo),
        Paragraph(usuario.email or "", meta),
        Paragraph(f"Portafolio generado el {timezone.localtime():%d/%m/%Y}", meta),
    ]

    def tabla(cabeceras, filas, anchos):
        datos = [[Paragraph(f"<b>{c}</b>", celda) for c in cabeceras]]
        datos += [[Paragraph(str(v), celda) for v in fila] for fila in filas]
        t = Table(datos, colWidths=[documento.width * a for a in anchos], repeatRows=1)
        t.setStyle(TableStyle([
            ("BACKGROUND", (0, 0), (-1, 0), colors.HexColor("#e5e7eb")),
            ("GRID", (0, 0), (-1, -1), 0.4, colors.HexColor("#d1d5db")),
            ("VALIGN", (0, 0), (-1, -1), "TOP"),
            ("ROWBACKGROUNDS", (0, 1), (-1, -1), [colors.white, colors.HexColor("#f9fafb")]),
        ]))
        return t

    elementos.append(Paragraph("Certificados obtenidos", seccion))
    if certificados:
        elementos.append(tabla(
            ["Reto", "Empresa", "Rol", "Emitido", "Codigo"],
            [[
                c.reto.titulo,
                c.reto.empresa.get_full_name() or c.reto.empresa.username,
                c.get_rol_participacion_display(),
                timezone.localtime(c.emitido_en).strftime("%d/%m/%Y"),
                c.codigo_corto,
            ] for c in certificados],
            [0.30, 0.24, 0.14, 0.14, 0.18],
        ))
    else:
        elementos.append(Paragraph("Todavia no hay certificados emitidos.", meta))

    elementos.append(Paragraph("Entregables evaluados", seccion))
    if entregables:
        elementos.append(tabla(
            ["Reto", "Entregable", "Nota", "Estado", "Fecha"],
            [[
                e.reto.titulo, e.titulo,
                f"{e.nota} / {e.puntaje_maximo}" if e.nota is not None else "-",
                e.get_estado_display(),
                timezone.localtime(e.fecha_entrega).strftime("%d/%m/%Y"),
            ] for e in entregables],
            [0.30, 0.28, 0.14, 0.14, 0.14],
        ))
    else:
        elementos.append(Paragraph("Todavia no hay entregables registrados.", meta))

    documento.build(elementos)
    return buffer.getvalue()
