"""Utilidades para pruebas: dejar una empresa habilitada para participar."""

from datetime import timedelta

from django.core.files.uploadedfile import SimpleUploadedFile
from django.utils import timezone

from .models import DocumentoEmpresa, TipoDocumentoLegal


def habilitar_empresa(empresa):
    """Crea todos los documentos obligatorios de la empresa, verificados y vigentes."""
    expedicion = timezone.localdate() - timedelta(days=1)
    for tipo in TipoDocumentoLegal.objects.filter(activo=True):
        if not tipo.es_requerido_para(empresa):
            continue
        DocumentoEmpresa.objects.update_or_create(
            empresa=empresa, tipo_documento=tipo.codigo,
            defaults={
                "estado": "VERIFICADO",
                "archivo": SimpleUploadedFile(f"{tipo.codigo.lower()}.pdf", b"%PDF-1.4"),
                "fecha_expedicion": expedicion,
                "fecha_vencimiento": expedicion + timedelta(days=tipo.vigencia_dias) if tipo.vigencia_dias else None,
            },
        )
    return empresa
