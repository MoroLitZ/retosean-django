"""Normaliza el catalogo academico: fusiona facultades y programas duplicados.

Antes de sembrar, las bases del equipo arrastraban facultades duplicadas
("Ingenieria" / "Ingenieria y Ciencias Basicas") y programas repetidos
("Administracion de Empresas" en dos facultades). Esta migracion:

1. Fusiona las facultades por nombre normalizado (sin tildes ni mayusculas),
   reapuntando Programa.facultad, Profesor.facultad y Reto.facultad.
2. Fusiona los programas por nombre normalizado, reapuntando Estudiante.programa,
   Reto.programa y UnidadEstudio.programa.
3. Renombra la fila superviviente al nombre institucional (con tildes) cuando
   coincide con el catalogo oficial, para que la siembra posterior no cree
   duplicados por diferencia de acentos.

No borra retos ni usuarios: solo reapunta claves foraneas.
"""

import unicodedata

from django.db import migrations

from apps.academico import catalogo_base


def _normalizar(texto):
    """Minusculas y sin tildes: 'Ingeniería' -> 'ingenieria'."""
    texto = unicodedata.normalize("NFKD", texto or "")
    texto = "".join(c for c in texto if not unicodedata.combining(c))
    return texto.lower().strip()


def _fusionar_facultades(apps, schema_editor):
    Facultad = apps.get_model("academico", "Facultad")
    Programa = apps.get_model("academico", "Programa")
    Profesor = apps.get_model("academico", "Profesor")
    Reto = apps.get_model("retos", "Reto")

    nombre_canonico = {_normalizar(nombre): nombre for nombre, _ in catalogo_base.FACULTADES}

    # Canonical: primera facultad (menor id) de cada nombre normalizado.
    canonicas = {}
    for facultad in Facultad.objects.all().order_by("id"):
        clave = _normalizar(facultad.nombre)
        if clave not in canonicas:
            canonicas[clave] = facultad

    for facultad in Facultad.objects.all().order_by("id"):
        clave = _normalizar(facultad.nombre)
        canonica = canonicas[clave]
        # Renombrar la canonica al nombre institucional si se conoce.
        if canonica.pk == facultad.pk and clave in nombre_canonico:
            Facultad.objects.filter(pk=canonica.pk).update(nombre=nombre_canonico[clave])
        elif canonica.pk != facultad.pk:
            Profesor.objects.filter(facultad=facultad).update(facultad=canonica)
            Reto.objects.filter(facultad=facultad).update(facultad=canonica)
            Programa.objects.filter(facultad=facultad).update(facultad=canonica)
            facultad.delete()


def _fusionar_programas(apps, schema_editor):
    Programa = apps.get_model("academico", "Programa")
    Estudiante = apps.get_model("academico", "Estudiante")
    Reto = apps.get_model("retos", "Reto")
    UnidadEstudio = apps.get_model("unidades_estudio", "UnidadEstudio")
    Facultad = apps.get_model("academico", "Facultad")

    nombre_canonico = {_normalizar(nombre): nombre for nombre, _ in catalogo_base.PROGRAMAS}
    facultad_de = {_normalizar(nombre): facultad for nombre, facultad in catalogo_base.PROGRAMAS}

    canonicos = {}
    for programa in Programa.objects.all().order_by("id"):
        clave = _normalizar(programa.nombre)
        if clave not in canonicos:
            canonicos[clave] = programa

    for programa in Programa.objects.all().order_by("id"):
        clave = _normalizar(programa.nombre)
        canonico = canonicos[clave]
        if canonico.pk == programa.pk:
            # Renombrar y recolgar a la facultad institucional correcta.
            cambios = {}
            if clave in nombre_canonico and programa.nombre != nombre_canonico[clave]:
                cambios["nombre"] = nombre_canonico[clave]
            if clave in facultad_de:
                facultad_canonica = Facultad.objects.filter(
                    nombre=facultad_de[clave]
                ).first()
                if facultad_canonica and programa.facultad_id != facultad_canonica.pk:
                    cambios["facultad"] = facultad_canonica
            if cambios:
                Programa.objects.filter(pk=programa.pk).update(**cambios)
        else:
            Estudiante.objects.filter(programa=programa).update(programa=canonico)
            Reto.objects.filter(programa=programa).update(programa=canonico)
            UnidadEstudio.objects.filter(programa=programa).update(programa=canonico)
            programa.delete()


def _normalizar_catalogo(apps, schema_editor):
    _fusionar_facultades(apps, schema_editor)
    _fusionar_programas(apps, schema_editor)


class Migration(migrations.Migration):

    dependencies = [
        ("academico", "0003_certificado"),
        ("unidades_estudio", "0001_initial"),
    ]

    operations = [
        migrations.RunPython(_normalizar_catalogo, migrations.RunPython.noop),
    ]
