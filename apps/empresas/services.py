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


def puede_la_empresa_operar(empresa):
    """
    Verifica si la empresa puede operar:
    - Si renunció al convenio, pasa directo.
    - Si no, valida sus documentos obligatorios.
    """
    # Si la empresa eligió la vía rápida sin convenio, puede operar
    if getattr(empresa, 'renuncio_a_convenio', False):
        return True

    # De lo contrario, mantenemos la validación normal de sus documentos
    tipos_requeridos = ['RUT', 'CAMARA_COMERCIO']
    for tipo in tipos_requeridos:
        if not DocumentoEmpresa.objects.filter(empresa=empresa, tipo_documento=tipo, estado='VERIFICADO').exists():
            return False

    return True


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
    cambiar_estado(
        documento, MAQUINA_DOCUMENTO, "CARGADO", usuario, "Documento recargado.",
        update_fields=["archivo", "fecha_expedicion", "motivo_rechazo"],
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
    cambiar_estado(
        documento, MAQUINA_DOCUMENTO, "VENCIDO", usuario,
        f"Documento vencido el {documento.fecha_vencimiento:%d/%m/%Y}.",
    )


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
