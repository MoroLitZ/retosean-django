"""Siembra el catalogo de documentos legales del brief (HU00).

Los ocho documentos del brief quedan obligatorios; el administrador puede
desactivarlos o volverlos opcionales desde "Tipos de documento". Los codigos
CAMARA_COMERCIO, RUT, CEDULA_REPRESENTANTE y CONVENIO coinciden con los que ya
usaban los documentos cargados, asi que no hay que tocar datos existentes.
"""

from django.db import migrations

TIPOS = [
    # codigo, nombre, obligatorio, aplica_sin_convenio, vigencia_dias, permiso especial, descripcion
    ("MINUTA_CONVENIO", "Minuta de convenio", True, False, None, False,
     "Versión actualizada 2026-1 (abril)."),
    ("CAMARA_COMERCIO", "Cámara de Comercio", True, True, 30, False,
     "Expedida hace menos de 30 días."),
    ("RUT", "Registro Único Tributario (RUT)", True, True, None, False, ""),
    ("CEDULA_REPRESENTANTE", "Cédula del representante legal", True, True, None, False, ""),
    ("LISTAS_INFORMA", "Listas Informa", True, True, None, True,
     "Solo la carga un usuario con permiso especial (líder de facultad designado)."),
    ("FORMATO_COMPROMISO", "Formato de compromiso de estudiantes", True, True, None, False, ""),
    ("TERMINOS_REFERENCIA", "Términos de referencia", True, True, None, False, ""),
    ("FICHA_RETO", "Ficha del reto (documento ISO)", True, True, None, False, ""),
    ("CONVENIO", "Convenio institucional firmado", False, False, None, False,
     "Copia del convenio firmado por las partes, si existe."),
]


def sembrar(apps, schema_editor):
    TipoDocumentoLegal = apps.get_model("empresas", "TipoDocumentoLegal")
    for orden, (codigo, nombre, obligatorio, sin_convenio, vigencia, especial, descripcion) in enumerate(TIPOS, 1):
        TipoDocumentoLegal.objects.update_or_create(
            codigo=codigo,
            defaults={
                "nombre": nombre,
                "obligatorio": obligatorio,
                "aplica_sin_convenio": sin_convenio,
                "vigencia_dias": vigencia,
                "requiere_permiso_especial": especial,
                "descripcion": descripcion,
                "orden": orden,
                "activo": True,
            },
        )


def quitar(apps, schema_editor):
    apps.get_model("empresas", "TipoDocumentoLegal").objects.filter(
        codigo__in=[t[0] for t in TIPOS]
    ).delete()


class Migration(migrations.Migration):

    dependencies = [
        ("empresas", "0004_catalogo_documentos_y_bloqueo"),
    ]

    operations = [
        migrations.RunPython(sembrar, quitar),
    ]
