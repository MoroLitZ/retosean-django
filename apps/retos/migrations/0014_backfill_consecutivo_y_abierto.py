"""Datos para el ciclo de vida unificado.

1. Renumera los consecutivos existentes por tipo y anio de aprobacion
   (RETO-2026-001, HACK-2026-001). Antes era una sola secuencia global.
2. Los retos aprobados que ya tienen una integracion academica publicada pasan
   a Abierto, que es el estado que ahora lista el explorador de estudiantes.
"""

from django.db import migrations


def _fecha_base(reto):
    return reto.fecha_aprobacion or reto.creado_en


def adelante(apps, schema_editor):
    Reto = apps.get_model("retos", "Reto")
    HistorialEstadoReto = apps.get_model("retos", "HistorialEstadoReto")

    contadores = {}
    numerados = Reto.objects.exclude(consecutivo__isnull=True)
    for reto in sorted(numerados, key=lambda r: (_fecha_base(r), r.consecutivo, r.pk)):
        anio = _fecha_base(reto).year
        clave = (reto.tipo, anio)
        contadores[clave] = contadores.get(clave, 0) + 1
        reto.anio_consecutivo = anio
        reto.consecutivo = contadores[clave]
        reto.save(update_fields=["anio_consecutivo", "consecutivo"])

    abiertos = Reto.objects.filter(
        estado="aprobado", integraciones_seguimiento__estado="publicada"
    ).distinct()
    for reto in abiertos:
        reto.estado = "abierto"
        reto.save(update_fields=["estado"])
        HistorialEstadoReto.objects.create(
            reto=reto,
            estado_anterior="aprobado",
            estado_nuevo="abierto",
            comentario="Migracion: el reto ya tenia una integracion academica publicada.",
        )


def atras(apps, schema_editor):
    Reto = apps.get_model("retos", "Reto")
    Reto.objects.filter(estado="abierto").update(estado="aprobado")
    # Los consecutivos renumerados no se pueden reconstruir; se conservan
    # como secuencia unica por orden de aprobacion para no chocar en reversa.
    for numero, reto in enumerate(
        Reto.objects.exclude(consecutivo__isnull=True).order_by("anio_consecutivo", "tipo", "consecutivo"),
        start=1,
    ):
        reto.consecutivo = numero
        reto.anio_consecutivo = None
        reto.save(update_fields=["consecutivo", "anio_consecutivo"])


class Migration(migrations.Migration):

    dependencies = [
        ("retos", "0013_ciclo_vida_abierto_consecutivo_borrado_logico"),
        ("seguimiento", "0007_historialestado"),
    ]

    operations = [
        migrations.RunPython(adelante, atras),
    ]
