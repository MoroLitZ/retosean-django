"""Unifica la bitacora: historial_estados_reto pasa a seguimiento.HistorialEstado.

Se conserva cada fila con su fecha original (auto_now_add la sobrescribiria al
crear, por eso se reasigna con update) y el autor del cambio.
"""

from django.db import migrations


def _tipo_reto(apps):
    ContentType = apps.get_model("contenttypes", "ContentType")
    tipo, _ = ContentType.objects.get_or_create(app_label="retos", model="reto")
    return tipo


def copiar_a_bitacora_unica(apps, schema_editor):
    Antiguo = apps.get_model("retos", "HistorialEstadoReto")
    HistorialEstado = apps.get_model("seguimiento", "HistorialEstado")
    tipo = _tipo_reto(apps)
    for fila in Antiguo.objects.order_by("fecha", "id").iterator():
        nuevo = HistorialEstado.objects.create(
            content_type=tipo, object_id=fila.reto_id, campo="estado",
            estado_anterior=fila.estado_anterior, estado_nuevo=fila.estado_nuevo,
            comentario=fila.comentario, realizado_por_id=fila.realizado_por_id,
        )
        HistorialEstado.objects.filter(pk=nuevo.pk).update(fecha=fila.fecha)


def devolver_a_tabla_de_retos(apps, schema_editor):
    Antiguo = apps.get_model("retos", "HistorialEstadoReto")
    HistorialEstado = apps.get_model("seguimiento", "HistorialEstado")
    filas = HistorialEstado.objects.filter(content_type=_tipo_reto(apps), campo="estado")
    for fila in filas.order_by("fecha", "id").iterator():
        viejo = Antiguo.objects.create(
            reto_id=fila.object_id, estado_anterior=fila.estado_anterior, estado_nuevo=fila.estado_nuevo,
            comentario=fila.comentario, realizado_por_id=fila.realizado_por_id,
        )
        Antiguo.objects.filter(pk=viejo.pk).update(fecha=fila.fecha)
    filas.delete()


class Migration(migrations.Migration):

    dependencies = [
        ('retos', '0016_estructura_equipo_hu03'),
        ('seguimiento', '0008_estructura_equipo_hu03'),
        ('contenttypes', '0002_remove_content_type_name'),
    ]

    operations = [
        migrations.RunPython(copiar_a_bitacora_unica, devolver_a_tabla_de_retos),
        migrations.DeleteModel(
            name='HistorialEstadoReto',
        ),
    ]
