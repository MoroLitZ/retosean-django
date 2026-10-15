from datetime import timedelta

from django.db import transaction
from django.urls import reverse
from django.utils import timezone

from apps.empresas.models import DocumentoEmpresa, Empresa, TipoDocumentoLegal
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


def tipos_requeridos(empresa):
    """Tipos de documento obligatorios para esta empresa, segun el catalogo."""
    return [tipo for tipo in TipoDocumentoLegal.objects.filter(activo=True) if tipo.es_requerido_para(empresa)]


def estado_documental(empresa):
    """Lista de control de la empresa: un renglon por tipo activo del catalogo.

    Cada renglon trae el tipo, su documento (o None), si es requerido, si esta
    al dia (verificado y vigente) y un texto de alerta cuando falta algo.
    """
    documentos = {d.tipo_documento: d for d in DocumentoEmpresa.objects.filter(empresa=empresa)}
    filas = []
    for tipo in TipoDocumentoLegal.objects.filter(activo=True):
        documento = documentos.get(tipo.codigo)
        requerido = tipo.es_requerido_para(empresa)
        al_dia = bool(documento and documento.estado == "VERIFICADO" and documento.esta_vigente)
        alerta = ""
        if documento is None or documento.estado == "PENDIENTE":
            alerta = "Falta cargarlo." if requerido else ""
        elif documento.estado == "RECHAZADO":
            alerta = f"Rechazado: {documento.motivo_rechazo or 'sin motivo'}. Debe cargarse de nuevo."
        elif documento.estado == "VENCIDO" or not documento.esta_vigente:
            alerta = "Vencido: debe actualizarse."
        elif documento.estado == "CARGADO":
            alerta = "En revisión por el administrador."
        filas.append({
            "tipo": tipo,
            "documento": documento,
            "requerido": requerido,
            "al_dia": al_dia,
            "alerta": alerta,
        })
    return filas


def documentacion_completa(empresa):
    """Todos los documentos obligatorios estan verificados y vigentes."""
    return all(fila["al_dia"] for fila in estado_documental(empresa) if fila["requerido"])


def motivos_para_no_operar(empresa):
    """Razones por las que la empresa aun no puede participar (lista vacia = puede)."""
    motivos = []
    if empresa.participacion_bloqueada:
        motivos.append(f"Participación bloqueada por el administrador. {empresa.motivo_bloqueo}".strip())
    if empresa.estado_listas_restrictivas == "RECHAZADO":
        motivos.append("La empresa aparece en listas restrictivas.")
    for fila in estado_documental(empresa):
        if fila["requerido"] and not fila["al_dia"]:
            motivos.append(f'{fila["tipo"].nombre}: {fila["alerta"] or "pendiente de verificación."}')
    return motivos


def puede_la_empresa_operar(empresa):
    """La empresa participa solo con todos sus documentos obligatorios verificados
    y vigentes, sin bloqueo del administrador ni hallazgos en listas restrictivas.

    Elegir participar sin convenio ya no salta la verificacion: solo deja de
    exigir los documentos propios del convenio (`aplica_sin_convenio=False`).
    """
    return not motivos_para_no_operar(empresa)


@transaction.atomic
def cambiar_bloqueo_participacion(empresa, bloquear, usuario, motivo=""):
    """Bloquea o desbloquea la participacion de la empresa y lo deja en el historial."""
    from apps.notificaciones.services import notificar

    if empresa.participacion_bloqueada == bloquear:
        return False
    if bloquear and not motivo.strip():
        raise TransicionInvalida("Indica el motivo del bloqueo.")
    empresa.participacion_bloqueada = bloquear
    empresa.motivo_bloqueo = motivo.strip() if bloquear else ""
    empresa.save(update_fields=["participacion_bloqueada", "motivo_bloqueo"])
    registrar_cambio_estado(
        empresa,
        "HABILITADA" if bloquear else "BLOQUEADA",
        "BLOQUEADA" if bloquear else "HABILITADA",
        usuario, motivo, campo="participacion_bloqueada",
    )
    if empresa.usuario_id:
        notificar(
            empresa.usuario, "EMPRESA_PARTICIPACION",
            mensaje=(
                f"El administrador bloqueó la participación de tu empresa. Motivo: {motivo}"
                if bloquear else "El administrador habilitó de nuevo la participación de tu empresa."
            ),
            tipo="ERROR" if bloquear else "EXITO",
            link=reverse("empresas:documentos"),
            clave_dedupe=f"empresa:{empresa.pk}:bloqueo:{bloquear}:{timezone.now():%Y%m%d%H%M%S}",
        )
    return True


def fecha_vencimiento_para(tipo, fecha_expedicion):
    if tipo and tipo.vigencia_dias and fecha_expedicion:
        return fecha_expedicion + timedelta(days=tipo.vigencia_dias)
    return None


@transaction.atomic
def cargar_documento(empresa, tipo_documento, archivo, fecha_expedicion, usuario):
    """Crea o recarga un documento legal y lo deja en estado Cargado.

    Devuelve `(documento, creado)`. Lanza `TransicionInvalida` si el documento
    existente no admite recarga desde su estado actual.
    """
    tipo = TipoDocumentoLegal.objects.filter(codigo=tipo_documento).first()
    vencimiento = fecha_vencimiento_para(tipo, fecha_expedicion)
    documento = DocumentoEmpresa.objects.select_for_update().filter(
        empresa=empresa, tipo_documento=tipo_documento
    ).first()
    if documento is None:
        documento = DocumentoEmpresa.objects.create(
            empresa=empresa,
            tipo_documento=tipo_documento,
            archivo=archivo,
            fecha_expedicion=fecha_expedicion,
            fecha_vencimiento=vencimiento,
            estado="CARGADO",
        )
        registrar_cambio_estado(documento, "", "CARGADO", usuario, "Documento cargado.")
        return documento, True

    documento.archivo = archivo
    documento.fecha_expedicion = fecha_expedicion
    documento.fecha_vencimiento = vencimiento
    documento.motivo_rechazo = ""
    cambiar_estado(
        documento, MAQUINA_DOCUMENTO, "CARGADO", usuario, "Documento recargado.",
        update_fields=["archivo", "fecha_expedicion", "fecha_vencimiento", "motivo_rechazo"],
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
    motivo = f"Documento vencido el {documento.fecha_vencimiento:%d/%m/%Y}."
    cambiar_estado(documento, MAQUINA_DOCUMENTO, "VENCIDO", usuario, motivo)

    # HU00: la empresa debe enterarse de que un documento vencio y de que tiene que actualizarlo.
    from apps.notificaciones.services import notificar

    notificar(
        documento.empresa.usuario, "DOCUMENTO_REVISADO", tipo="ADVERTENCIA",
        mensaje=(f'Tu documento "{documento.get_tipo_documento_display()}" esta vencido. {motivo} '
                 "Cargalo de nuevo para seguir participando."),
        link=reverse("empresas:documentos"),
        clave_dedupe=f"documento:{documento.pk}:vencido:{documento.fecha_vencimiento}",
    )


def historial_documentos(empresa, limite=30):
    """Cambios de estado de los documentos de la empresa, del mas reciente al mas antiguo."""
    from django.contrib.contenttypes.models import ContentType
    from apps.seguimiento.models import HistorialEstado

    documentos = {d.pk: d for d in DocumentoEmpresa.objects.filter(empresa=empresa)}
    registros = HistorialEstado.objects.filter(
        content_type=ContentType.objects.get_for_model(DocumentoEmpresa),
        object_id__in=documentos.keys(),
    ).select_related("realizado_por")[:limite]
    for registro in registros:
        registro.documento = documentos.get(registro.object_id)
    return registros


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
