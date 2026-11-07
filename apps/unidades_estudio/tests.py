from unittest.mock import patch

from django.core.files.uploadedfile import SimpleUploadedFile
from django.db import IntegrityError
from django.test import TestCase
from django.urls import reverse

from apps.academico.models import Facultad, Programa, Profesor
from apps.usuarios.models import Usuario

from .models import UnidadEstudio


class CargaMasivaCsvTests(TestCase):
    """El formulario ofrecia esta carga pero la vista nunca leia el archivo."""

    def setUp(self):
        self.admin = Usuario.objects.create_superuser(
            username="admin-ue", password="x", email="admin@ean.edu.co"
        )
        self.client.force_login(self.admin)
        self.url = reverse("unidades_estudio:crear")
        facultad = Facultad.objects.create(nombre="Ingenieria", codigo="ING")
        Programa.objects.create(nombre="Ingenieria de Sistemas", facultad=facultad)

    def _csv(self, filas, cabecera="codigo,nombre,programa,periodo,ciclo"):
        contenido = cabecera + "\n" + "\n".join(filas)
        return SimpleUploadedFile("unidades.csv", contenido.encode("utf-8"), content_type="text/csv")

    def test_crea_varias_unidades(self):
        archivo = self._csv([
            "ING-101,Introduccion a la Ingenieria,Ingenieria de Sistemas,2026-2,1",
            "MAT-201,Calculo Diferencial,Ingenieria de Sistemas,2026-2,2",
        ])
        self.client.post(self.url, {"accion": "cargar_csv", "archivo_csv": archivo})
        self.assertEqual(UnidadEstudio.objects.count(), 2)
        unidad = UnidadEstudio.objects.get(codigo="ING-101")
        self.assertEqual(unidad.programa.nombre, "Ingenieria de Sistemas")

    def test_reutiliza_el_programa_sin_distinguir_mayusculas(self):
        archivo = self._csv(["ING-102,Algoritmos,INGENIERIA DE SISTEMAS,2026-2,2"])
        self.client.post(self.url, {"accion": "cargar_csv", "archivo_csv": archivo})
        self.assertEqual(Programa.objects.filter(nombre__iexact="Ingenieria de Sistemas").count(), 1)

    def test_omite_programa_inexistente(self):
        archivo = self._csv(["ADM-101,Fundamentos,Administracion de Empresas,2026-2,1"])
        self.client.post(self.url, {"accion": "cargar_csv", "archivo_csv": archivo})
        self.assertFalse(Programa.objects.filter(nombre__iexact="Administracion de Empresas").exists())
        self.assertEqual(UnidadEstudio.objects.count(), 0)

    def test_omite_codigos_duplicados(self):
        Programa.objects.get(nombre="Ingenieria de Sistemas")
        archivo = self._csv([
            "DUP-100,Una,Ingenieria de Sistemas,2026-2,1",
            "DUP-100,Otra,Ingenieria de Sistemas,2026-2,1",
        ])
        self.client.post(self.url, {"accion": "cargar_csv", "archivo_csv": archivo})
        self.assertEqual(UnidadEstudio.objects.filter(codigo="DUP-100").count(), 1)

    def test_rechaza_csv_sin_columnas_obligatorias(self):
        archivo = self._csv(["2026-2,1"], cabecera="periodo,ciclo")
        response = self.client.post(
            self.url, {"accion": "cargar_csv", "archivo_csv": archivo}, follow=True
        )
        self.assertEqual(UnidadEstudio.objects.count(), 0)
        mensajes = [str(m) for m in response.context["messages"]]
        self.assertTrue(any("columnas obligatorias" in m for m in mensajes))

    def test_acepta_punto_y_coma_como_separador(self):
        archivo = self._csv(
            ["SEP-100;Con punto y coma;Ingenieria de Sistemas;2026-2;1"],
            cabecera="codigo;nombre;programa;periodo;ciclo",
        )
        self.client.post(self.url, {"accion": "cargar_csv", "archivo_csv": archivo})
        self.assertTrue(UnidadEstudio.objects.filter(codigo="SEP-100").exists())

    def test_estudiante_no_puede_cargar(self):
        estudiante = Usuario.objects.create_user(
            username="est-ue", password="x", rol="ESTUDIANTE"
        )
        self.client.force_login(estudiante)
        archivo = self._csv(["NO-100,No,Ingenieria de Sistemas,2026-2,1"])
        self.client.post(self.url, {"accion": "cargar_csv", "archivo_csv": archivo})
        self.assertEqual(UnidadEstudio.objects.count(), 0)


class CreacionIndividualTests(TestCase):
    def setUp(self):
        self.admin = Usuario.objects.create_superuser(
            username="admin-ue2", password="x", email="a@ean.edu.co"
        )
        self.client.force_login(self.admin)

    def test_crea_una_unidad_con_programa_existente(self):
        facultad = Facultad.objects.create(nombre="Ingenieria", codigo="ING")
        programa = Programa.objects.create(nombre="Ingenieria de Sistemas", facultad=facultad)
        self.client.post(reverse("unidades_estudio:crear"), {
            "codigo": "UNI-100",
            "nombre": "Unidad de prueba",
            "programa": str(programa.pk),
            "periodo": "2026-2",
            "ciclo": "3",
        })
        unidad = UnidadEstudio.objects.get(codigo="UNI-100")
        self.assertEqual(unidad.programa, programa)


class AutorizacionProfesorTests(TestCase):
    """CIE-03: un profesor solo gestiona unidades de su propia facultad."""

    def setUp(self):
        self.facultad_ingenieria = Facultad.objects.create(nombre="Ingenieria", codigo="ING")
        self.facultad_negocios = Facultad.objects.create(nombre="Negocios", codigo="NEG")
        self.programa_sistemas = Programa.objects.create(
            nombre="Ingenieria de Sistemas", facultad=self.facultad_ingenieria
        )
        self.programa_admin = Programa.objects.create(
            nombre="Administracion de Empresas", facultad=self.facultad_negocios
        )
        self.profesor = Usuario.objects.create_user(
            username="profe-ing", password="x", rol="PROFESOR"
        )
        Profesor.objects.create(usuario=self.profesor, facultad=self.facultad_ingenieria)
        self.profesor_sin_facultad = Usuario.objects.create_user(
            username="profe-sf", password="x", rol="PROFESOR"
        )
        self.admin = Usuario.objects.create_superuser(
            username="admin-ue3", password="x", email="admin3@ean.edu.co"
        )
        self.unidad = UnidadEstudio.objects.create(
            codigo="UNI-SIS", nombre="Unidad propia", programa=self.programa_sistemas,
            periodo="2026-2", ciclo="1",
        )
        self.unidad_ajena = UnidadEstudio.objects.create(
            codigo="UNI-ADM", nombre="Unidad ajena", programa=self.programa_admin,
            periodo="2026-2", ciclo="2",
        )

    def test_profesor_puede_desactivar_unidad_de_su_facultad(self):
        self.client.force_login(self.profesor)
        self.client.post(reverse("unidades_estudio:eliminar", args=[self.unidad.pk]))
        self.unidad.refresh_from_db()
        self.assertFalse(self.unidad.activo)

    def test_profesor_no_desactiva_unidad_de_otra_facultad(self):
        self.client.force_login(self.profesor)
        response = self.client.post(reverse("unidades_estudio:eliminar", args=[self.unidad_ajena.pk]))
        self.assertRedirects(response, reverse("unidades_estudio:lista"))
        self.unidad_ajena.refresh_from_db()
        self.assertTrue(self.unidad_ajena.activo)

    def test_profesor_no_crea_unidad_en_otra_facultad(self):
        self.client.force_login(self.profesor)
        self.client.post(reverse("unidades_estudio:crear"), {
            "codigo": "UNI-NUEVA",
            "nombre": "Unidad nueva",
            "programa": str(self.programa_admin.pk),
            "periodo": "2026-2",
            "ciclo": "1",
        })
        self.assertFalse(UnidadEstudio.objects.filter(codigo="UNI-NUEVA").exists())

    def test_profesor_no_editar_unidad_de_otra_facultad(self):
        self.client.force_login(self.profesor)
        self.client.post(reverse("unidades_estudio:editar", args=[self.unidad_ajena.pk]), {
            "codigo": "UNI-ADM-MOD",
            "nombre": "Unidad modificada",
            "programa": str(self.programa_admin.pk),
            "periodo": "2026-2",
            "ciclo": "2",
        })
        self.unidad_ajena.refresh_from_db()
        self.assertEqual(self.unidad_ajena.codigo, "UNI-ADM")

    def test_profesor_sin_facultad_no_gestiona(self):
        self.client.force_login(self.profesor_sin_facultad)
        response = self.client.get(reverse("unidades_estudio:lista"))
        self.assertRedirects(response, reverse("usuarios:perfil"))

    def test_admin_puede_gestionar_cualquier_unidad(self):
        self.client.force_login(self.admin)
        self.client.post(reverse("unidades_estudio:eliminar", args=[self.unidad_ajena.pk]))
        self.unidad_ajena.refresh_from_db()
        self.assertFalse(self.unidad_ajena.activo)

    def test_lista_unidades_filtra_por_facultad_profesor(self):
        self.client.force_login(self.profesor)
        response = self.client.get(reverse("unidades_estudio:lista"))
        self.assertContains(response, self.unidad.nombre)
        self.assertNotContains(response, self.unidad_ajena.nombre)


class CsvTransaccionalTests(TestCase):
    """CIE-08: importacion CSV atómica; CIE-09: resolucion sin ambigüedad."""

    def setUp(self):
        self.admin = Usuario.objects.create_superuser(
            username="admin-ue4", password="x", email="admin4@ean.edu.co"
        )
        self.client.force_login(self.admin)
        self.url = reverse("unidades_estudio:crear")
        self.facultad_ingenieria = Facultad.objects.create(nombre="Ingenieria", codigo="ING")
        self.facultad_negocios = Facultad.objects.create(nombre="Negocios", codigo="NEG")
        Programa.objects.create(nombre="Ingenieria de Sistemas", facultad=self.facultad_ingenieria)
        Programa.objects.create(nombre="Administracion de Empresas", facultad=self.facultad_negocios)

    def _csv(self, filas, cabecera="codigo,nombre,programa,periodo,ciclo"):
        contenido = cabecera + "\n" + "\n".join(filas)
        return SimpleUploadedFile("unidades.csv", contenido.encode("utf-8"), content_type="text/csv")

    def test_importacion_rollback_frente_a_integrityerror(self):
        """Si falla una fila, el bloque atómico revierte las filas anteriores."""
        llamadas = []

        def _create_raising(*args, **kwargs):
            llamadas.append(1)
            if len(llamadas) == 2:
                raise IntegrityError("dup")
            return UnidadEstudio.objects.create(*args, **kwargs)

        archivo = self._csv([
            "OK-100,Primera,Ingenieria de Sistemas,2026-2,1",
            "OK-101,Segunda,Ingenieria de Sistemas,2026-2,2",
        ])
        with patch.object(UnidadEstudio.objects, "create", side_effect=_create_raising):
            self.client.post(self.url, {"accion": "cargar_csv", "archivo_csv": archivo})
        self.assertEqual(UnidadEstudio.objects.filter(codigo__in=["OK-100", "OK-101"]).count(), 0)

    def test_csv_rechaza_programa_ambiguo(self):
        # Dos programas con el mismo nombre en facultades distintas.
        Programa.objects.create(nombre="Sistemas de Informacion", facultad=self.facultad_ingenieria)
        Programa.objects.create(nombre="Sistemas de Informacion", facultad=self.facultad_negocios)
        archivo = self._csv([
            "AMB-100,Unidad,Sistemas de Informacion,2026-2,1",
        ])
        self.client.post(self.url, {"accion": "cargar_csv", "archivo_csv": archivo})
        self.assertEqual(UnidadEstudio.objects.count(), 0)

    def test_csv_resuelve_programa_ambiguo_con_columna_facultad(self):
        Programa.objects.create(nombre="Sistemas de Informacion", facultad=self.facultad_ingenieria)
        Programa.objects.create(nombre="Sistemas de Informacion", facultad=self.facultad_negocios)
        archivo = self._csv(
            ["AMB-100,Unidad,Sistemas de Informacion,Negocios,2026-2,1"],
            cabecera="codigo,nombre,programa,facultad,periodo,ciclo",
        )
        self.client.post(self.url, {"accion": "cargar_csv", "archivo_csv": archivo})
        unidad = UnidadEstudio.objects.get(codigo="AMB-100")
        self.assertEqual(unidad.programa.facultad, self.facultad_negocios)
