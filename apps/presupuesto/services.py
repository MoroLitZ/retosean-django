"""Registro de gastos y alerta de ejecucion presupuestal (HU07)."""

from decimal import Decimal

from django.db import transaction
from django.urls import reverse

from apps.notificaciones.services import notificar, notificar_admins

UMBRAL_ALERTA = Decimal("80")


@transaction.atomic
def registrar_gasto(presupuesto, gasto, usuario):
    """Guarda el gasto y dispara la alerta si se supera el 80% del presupuesto."""
    gasto.presupuesto = presupuesto
    gasto.registrado_por = usuario
    gasto.save()
    revisar_umbral(presupuesto)
    return gasto


def revisar_umbral(presupuesto):
    """Notifica una sola vez cuando la ejecucion supera el umbral configurado."""
    porcentaje = presupuesto.porcentaje_ejecutado

    if porcentaje < UMBRAL_ALERTA:
        # Si el presupuesto se amplia o se borra un gasto, se rearma la alerta.
        if presupuesto.alerta_80_enviada:
            presupuesto.alerta_80_enviada = False
            presupuesto.save(update_fields=["alerta_80_enviada"])
        return False

    if presupuesto.alerta_80_enviada:
        return False

    reto = presupuesto.reto
    mensaje = (
        f"El presupuesto del reto '{reto.titulo}' va en {porcentaje:.1f}% de ejecucion "
        f"({presupuesto.monto_ejecutado} de {presupuesto.monto_total}). "
        f"Disponible: {presupuesto.monto_disponible}."
    )
    link = reverse("presupuesto:detalle", kwargs={"reto_id": reto.pk})

    notificar_admins(
        "PRESUPUESTO_80", mensaje=mensaje, link=link,
        clave_dedupe=f"presupuesto:{presupuesto.pk}:80",
    )
    notificar(
        reto.empresa, "PRESUPUESTO_80", mensaje=mensaje, link=link,
        clave_dedupe=f"presupuesto:{presupuesto.pk}:80",
    )

    presupuesto.alerta_80_enviada = True
    presupuesto.save(update_fields=["alerta_80_enviada"])
    return True


# ------------------------------------------------------------- EXPORTACION
def _resumen_por_categoria(presupuesto):
    """[(categoria, total)] con todas las categorias, aunque no tengan gastos."""
    from django.db.models import Sum

    from .models import Gasto

    totales = {
        fila["categoria"]: fila["total"]
        for fila in presupuesto.gastos.values("categoria").annotate(total=Sum("monto"))
    }
    return [(etiqueta, totales.get(valor) or Decimal("0")) for valor, etiqueta in Gasto.CATEGORIAS]


def _gastos_ordenados(presupuesto):
    return presupuesto.gastos.select_related("registrado_por").order_by("creado_en")


def exportar_excel(presupuesto):
    """Reporte de gastos del reto en Excel: resumen, totales por categoria y detalle (HU07)."""
    import io

    from django.utils import timezone
    from openpyxl import Workbook
    from openpyxl.styles import Font, PatternFill

    libro = Workbook()
    hoja = libro.active
    hoja.title = "Gastos"
    relleno = PatternFill("solid", fgColor="111827")
    blanca = Font(color="FFFFFF", bold=True)

    def encabezado(*valores):
        hoja.append(list(valores))
        for celda in hoja[hoja.max_row]:
            celda.fill = relleno
            celda.font = blanca

    hoja.append([f"Presupuesto del reto: {presupuesto.reto.titulo}"])
    hoja["A1"].font = Font(bold=True, size=14)
    hoja.append([f"Generado el {timezone.localtime():%d/%m/%Y %H:%M}"])
    hoja.append([])
    for etiqueta, valor in [
        ("Asignado", presupuesto.monto_total),
        ("Ejecutado", presupuesto.monto_ejecutado),
        ("Disponible", presupuesto.monto_disponible),
        ("% ejecutado", round(presupuesto.porcentaje_ejecutado, 2)),
    ]:
        hoja.append([etiqueta, float(valor)])
        hoja.cell(row=hoja.max_row, column=1).font = Font(bold=True)

    hoja.append([])
    encabezado("Categoria", "Total")
    for etiqueta, total in _resumen_por_categoria(presupuesto):
        hoja.append([etiqueta, float(total)])

    hoja.append([])
    encabezado("Fecha", "Categoria", "Descripcion", "Monto", "Registrado por", "Comprobante")
    for gasto in _gastos_ordenados(presupuesto):
        hoja.append([
            timezone.localtime(gasto.creado_en).strftime("%d/%m/%Y"),
            gasto.get_categoria_display(),
            gasto.descripcion,
            float(gasto.monto),
            gasto.registrado_por.username if gasto.registrado_por else "",
            "Si" if gasto.comprobante else "No",
        ])
    for columna, ancho in zip("ABCDEF", (14, 18, 40, 14, 18, 12)):
        hoja.column_dimensions[columna].width = ancho

    buffer = io.BytesIO()
    libro.save(buffer)
    return buffer.getvalue()


def exportar_pdf(presupuesto):
    """Reporte de gastos del reto en PDF con encabezado institucional (HU07)."""
    import io

    from django.utils import timezone
    from reportlab.lib import colors
    from reportlab.lib.pagesizes import A4
    from reportlab.lib.styles import getSampleStyleSheet
    from reportlab.lib.units import mm
    from reportlab.platypus import Paragraph, SimpleDocTemplate, Spacer, Table, TableStyle

    buffer = io.BytesIO()
    documento = SimpleDocTemplate(
        buffer, pagesize=A4, leftMargin=15 * mm, rightMargin=15 * mm,
        topMargin=15 * mm, bottomMargin=15 * mm,
        title=f"Presupuesto - {presupuesto.reto.titulo}", author="RetosEAN",
    )
    estilos = getSampleStyleSheet()
    estilo_tabla = TableStyle([
        ("BACKGROUND", (0, 0), (-1, 0), colors.HexColor("#111827")),
        ("TEXTCOLOR", (0, 0), (-1, 0), colors.white),
        ("FONTNAME", (0, 0), (-1, 0), "Helvetica-Bold"),
        ("FONTSIZE", (0, 0), (-1, -1), 8.5),
        ("GRID", (0, 0), (-1, -1), 0.25, colors.HexColor("#d1d5db")),
        ("VALIGN", (0, 0), (-1, -1), "TOP"),
    ])

    def pesos(valor):
        return f"${valor:,.2f}"

    def tabla(filas, **kwargs):
        resultado = Table(filas, repeatRows=1, **kwargs)
        resultado.setStyle(estilo_tabla)
        return resultado

    detalle = [["Fecha", "Categoria", "Descripcion", "Monto"]] + [
        [
            timezone.localtime(gasto.creado_en).strftime("%d/%m/%Y"),
            gasto.get_categoria_display(),
            Paragraph(gasto.descripcion or "-", estilos["Normal"]),
            pesos(gasto.monto),
        ]
        for gasto in _gastos_ordenados(presupuesto)
    ]
    documento.build([
        Paragraph("retos<font color='#3bac52'>ean</font> - Universidad EAN", estilos["Title"]),
        Paragraph(f"Presupuesto del reto: {presupuesto.reto.titulo}", estilos["Heading2"]),
        Paragraph(f"Generado el {timezone.localtime():%d/%m/%Y %H:%M}", estilos["Normal"]),
        Spacer(1, 6 * mm),
        tabla([
            ["Asignado", "Ejecutado", "Disponible", "% ejecutado"],
            [pesos(presupuesto.monto_total), pesos(presupuesto.monto_ejecutado),
             pesos(presupuesto.monto_disponible), f"{presupuesto.porcentaje_ejecutado:.1f}%"],
        ]),
        Spacer(1, 6 * mm),
        Paragraph("Totales por categoria", estilos["Heading3"]),
        tabla([["Categoria", "Total"]] + [
            [etiqueta, pesos(total)] for etiqueta, total in _resumen_por_categoria(presupuesto)
        ]),
        Spacer(1, 6 * mm),
        Paragraph("Detalle de gastos", estilos["Heading3"]),
        tabla(detalle, colWidths=[25 * mm, 32 * mm, 90 * mm, 30 * mm]),
    ])
    return buffer.getvalue()
