from django.db import transaction
from django.urls import reverse

from apps.empresas.models import DocumentoEmpresa, Empresa
from apps.seguimiento.services import (
    MaquinaEstados,
    TransicionInvalida,
    cambiar_estado,
    registrar_cambio_estado,
)


# Documento legal (HU00): Pendiente -> Cargado -> Verificado | Rechazado.
# Recargar un documento lo devuelve a Cargado desde cualquier estado no pendiente
# (Rechazado -> Cargado es la recarga del brief; Verificado/Vencido -> Cargado es
# la actualizacion de un documento vigente o vencido). El rechazo automatico por
# listas restrictivas alcanza tambien a documentos pendientes o vencidos.
MAQUINA_DOCUMENTO = MaquinaEstados(
    "el documento",
    {
        "PENDIENTE": {"CARGADO", "RECHAZADO"},
        "CARGADO": {"CARGADO", "VERIFICADO", "RECHAZADO", "VENCIDO"},
        "VERIFICADO": {"CARGADO", "VENCIDO"},
        "RECHAZADO": {"CARGADO"},
        "VENCIDO": {"CARGADO", "RECHAZADO"},
    },
    dict(DocumentoEmpresa.ESTADOS),
)

# Tramite del convenio marco (HU00). No radicado -> Radicado -> En Juridica -> Firmado.
# No radicado -> Firmado cubre a la empresa que ya llega con un convenio vigente
# ("Si" en el formulario); Juridica puede devolver el tramite a Radicado, y un
# convenio firmado vuelve a Radicado cuando se tramita su renovacion.
MAQUINA_CONVENIO = MaquinaEstados(
    "el convenio",
    {
        "NO_RADICADO": {"RADICADO", "FIRMADO"},
        "RADICADO": {"EN_JURIDICA"},
        "EN_JURIDICA": {"FIRMADO", "RADICADO"},
        "FIRMADO": {"RADICADO"},
    },
    dict(Empresa.ESTADO_CONVENIO),
)


# Documentos que exige la plataforma para participar (HU00).
DOCUMENTOS_REQUERIDOS = ("CAMARA_COMERCIO", "RUT", "CEDULA_REPRESENTANTE", "CONVENIO")
DIAS_MAXIMOS_CAMARA = 30


def documentos_requeridos(empresa):
    # Quien elige "publicar de forma directa sin convenio" (empresas.elegir_modo_convenio)
    # queda fuera del control documental: esa pantalla le promete publicar de inmediato.
    if getattr(empresa, 'renuncio_a_convenio', False):
        return ()
    return DOCUMENTOS_REQUERIDOS


def documentos_faltantes(empresa):
    """Documentos requeridos que aun no estan verificados y vigentes, con el motivo.

    Devuelve una lista de dicts `{tipo, etiqueta, motivo}`; vacia si todo esta listo.
    """
    from django.utils import timezone

    hoy = timezone.localdate()
    por_tipo = {d.tipo_documento: d for d in DocumentoEmpresa.objects.filter(empresa=empresa)}
    etiquetas = dict(DocumentoEmpresa.TIPO_DOCUMENTO_CHOICES)
    faltantes = []
    for tipo in documentos_requeridos(empresa):
        doc = por_tipo.get(tipo)
        if doc is None or (doc.estado == "PENDIENTE" and not doc.archivo):
            motivo = "Falta cargar"
        elif doc.estado == "RECHAZADO":
            motivo = "Rechazado"
        elif doc.estado == "VENCIDO" or (doc.fecha_vencimiento and doc.fecha_vencimiento < hoy):
            motivo = "Vencido"
        elif tipo == "CAMARA_COMERCIO" and doc.fecha_expedicion is None:
            motivo = "Sin fecha de expedicion"
        elif tipo == "CAMARA_COMERCIO" and (hoy - doc.fecha_expedicion).days > DIAS_MAXIMOS_CAMARA:
            motivo = f"Vencido: expedido hace mas de {DIAS_MAXIMOS_CAMARA} dias"
        elif doc.estado != "VERIFICADO":
            motivo = "Pendiente de verificacion"
        else:
            continue
        faltantes.append({"tipo": tipo, "etiqueta": etiquetas[tipo], "motivo": motivo})
    return faltantes


def puede_la_empresa_operar(empresa):
    """Una empresa opera si no le falta ningun documento requerido (la que renuncio al convenio no requiere ninguno)."""
    return not documentos_faltantes(empresa)


def verificar_participacion(empresa):
    """Bloquea la participacion de una empresa con documentacion incompleta (HU00).

    Lanza `TransicionInvalida` con la lista de lo que falta; no hace nada si la
    empresa puede participar.
    """
    if empresa is None:
        raise TransicionInvalida("La empresa no tiene perfil registrado: no puede participar.")
    faltantes = documentos_faltantes(empresa)
    if faltantes:
        nombres = ", ".join(f"{f['etiqueta']} ({f['motivo'].lower()})" for f in faltantes)
        raise TransicionInvalida(f"La empresa aun no puede participar. Faltan: {nombres}.")


@transaction.atomic
def cargar_documento(empresa, tipo_documento, archivo, fecha_expedicion, usuario):
    """Crea o recarga un documento legal y lo deja en estado Cargado.

    Devuelve `(documento, creado)`. Lanza `TransicionInvalida` si el documento
    existente no admite recarga desde su estado actual.
    """
    documento = DocumentoEmpresa.objects.select_for_update().filter(
        empresa=empresa, tipo_documento=tipo_documento
    ).first()
    if documento is None:
        documento = DocumentoEmpresa.objects.create(
            empresa=empresa,
            tipo_documento=tipo_documento,
            archivo=archivo,
            fecha_expedicion=fecha_expedicion,
            estado="CARGADO",
        )
        registrar_cambio_estado(documento, "", "CARGADO", usuario, "Documento cargado.")
        return documento, True

    documento.archivo = archivo
    documento.fecha_expedicion = fecha_expedicion
    documento.motivo_rechazo = ""
    # Un archivo nuevo es otro documento: el vencimiento del anterior ya no aplica.
    documento.fecha_vencimiento = None
    cambiar_estado(
        documento, MAQUINA_DOCUMENTO, "CARGADO", usuario, "Documento recargado.",
        update_fields=["archivo", "fecha_expedicion", "motivo_rechazo", "fecha_vencimiento"],
    )
    return documento, False


def revisar_documento(documento, nuevo_estado, usuario, motivo=""):
    """El administrador verifica o rechaza un documento cargado."""
    if nuevo_estado not in ("VERIFICADO", "RECHAZADO"):
        raise TransicionInvalida("El documento solo puede quedar verificado o rechazado.")
    documento.motivo_rechazo = motivo if nuevo_estado == "RECHAZADO" else ""
    cambiar_estado(
        documento, MAQUINA_DOCUMENTO, nuevo_estado, usuario, motivo,
        update_fields=["motivo_rechazo"],
    )
    return documento


def rechazar_documentos_por_listas(empresa, usuario):
    """Rechaza todo documento no verificado de una empresa que aparece en listas restrictivas."""
    motivo = "Rechazado automáticamente por aparecer en listas restrictivas (Clinton/OFAC)."
    rechazados = 0
    for documento in DocumentoEmpresa.objects.filter(empresa=empresa).exclude(estado__in=["VERIFICADO", "RECHAZADO"]):
        documento.motivo_rechazo = motivo
        cambiar_estado(
            documento, MAQUINA_DOCUMENTO, "RECHAZADO", usuario, motivo,
            update_fields=["motivo_rechazo"],
        )
        rechazados += 1
    return rechazados


def marcar_documento_vencido(documento, usuario=None):
    if documento.fecha_vencimiento:
        motivo = f"Documento vencido el {documento.fecha_vencimiento:%d/%m/%Y}."
    else:
        motivo = (f"Camara de Comercio con mas de {DIAS_MAXIMOS_CAMARA} dias "
                  f"(expedida el {documento.fecha_expedicion:%d/%m/%Y}).")
    cambiar_estado(documento, MAQUINA_DOCUMENTO, "VENCIDO", usuario, motivo)

    # HU00: la empresa debe saber que un documento vencio y que tiene que actualizarlo.
    from apps.notificaciones.services import notificar

    notificar(
        documento.empresa.usuario, "DOCUMENTO_REVISADO", tipo="ADVERTENCIA",
        mensaje=f'Tu documento "{documento.get_tipo_documento_display()}" esta vencido. {motivo} Cargalo de nuevo para seguir participando.',
        link=reverse("empresas:documentos"),
        clave_dedupe=f"documento:{documento.pk}:vencido:{documento.fecha_carga:%Y%m%d%H%M%S}",
    )


def documentos_camara_vencidos(hoy):
    """Camaras de Comercio verificadas o cargadas con mas de 30 dias desde su expedicion."""
    from datetime import timedelta

    limite = hoy - timedelta(days=DIAS_MAXIMOS_CAMARA)
    return DocumentoEmpresa.objects.filter(
        tipo_documento="CAMARA_COMERCIO",
        fecha_expedicion__lt=limite,
        estado__in=["VERIFICADO", "CARGADO"],
    )


def historial_documental(empresa):
    """Cambios de estado de la documentacion de una empresa (documentos y convenio), del mas reciente."""
    from django.contrib.contenttypes.models import ContentType
    from apps.seguimiento.models import HistorialEstado
    from apps.seguimiento.services import historial_de

    nombres = {d.pk: d.get_tipo_documento_display()
               for d in DocumentoEmpresa.objects.filter(empresa=empresa)}
    de_documentos = HistorialEstado.objects.filter(
        content_type=ContentType.objects.get_for_model(DocumentoEmpresa),
        object_id__in=list(nombres),
    ).select_related("realizado_por")
    entradas = [
        {
            "fecha": r.fecha,
            "elemento": nombres.get(r.object_id, "Documento") if r.campo == "estado" else "Convenio (trámite)",
            "anterior": r.estado_anterior or "-",
            "nuevo": r.estado_nuevo,
            "comentario": r.comentario,
            "usuario": r.realizado_por,
            "quien": (r.realizado_por.get_full_name() or r.realizado_por.username) if r.realizado_por else "Sistema",
        }
        for r in [*de_documentos, *historial_de(empresa, campo="estado_convenio")]
    ]
    return sorted(entradas, key=lambda e: e["fecha"], reverse=True)


def cambiar_estado_convenio(empresa, estado_nuevo, usuario, comentario="", fecha_vencimiento=None):
    """Avanza el tramite del convenio y avisa a la empresa.

    Firmar exige la fecha de vencimiento del convenio; los demas estados la limpian.
    """
    from apps.notificaciones.services import notificar

    if estado_nuevo == "FIRMADO" and not fecha_vencimiento:
        raise TransicionInvalida("Para marcar el convenio como firmado indica su fecha de vencimiento.")
    empresa.fecha_vencimiento_convenio = fecha_vencimiento if estado_nuevo == "FIRMADO" else None
    cambiar_estado(
        empresa, MAQUINA_CONVENIO, estado_nuevo, usuario, comentario,
        campo="estado_convenio", update_fields=["fecha_vencimiento_convenio"],
    )

    if empresa.usuario_id:
        mensaje = f'El tramite de tu convenio quedo en estado: {empresa.get_estado_convenio_display()}.'
        if estado_nuevo == "FIRMADO":
            mensaje += f'\nVigente hasta el {fecha_vencimiento:%d/%m/%Y}.'
        if comentario:
            mensaje += f'\nComentario: {comentario}'
        notificar(
            empresa.usuario, "CONVENIO_ACTUALIZADO",
            mensaje=mensaje,
            tipo="EXITO" if estado_nuevo == "FIRMADO" else "INFO",
            link=reverse("empresas:documentos"),
            clave_dedupe=f"convenio:{empresa.pk}:{estado_nuevo}:{fecha_vencimiento or ''}",
        )
    return empresa
