from django.core.files.uploadedfile import SimpleUploadedFile
from django.test import TestCase
from django.urls import reverse

from apps.academico.models import Facultad, Programa
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
