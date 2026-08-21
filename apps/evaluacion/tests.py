from django.core.files.uploadedfile import SimpleUploadedFile
from django.test import TestCase
from django.urls import reverse

from apps.participaciones.models import Equipo
from apps.retos.models import Reto
from apps.seguimiento.models import IntegracionAcademica
from apps.usuarios.models import Usuario

from .models import ComentarioEntregable, Entregable, Evaluacion


class EvaluacionSprintTests(TestCase):
    def setUp(self):
        self.empresa = Usuario.objects.create_user(username="empresa-eval", password="x", rol="EMPRESA")
        self.profesor = Usuario.objects.create_user(username="profe-eval", password="x", rol="PROFESOR")
        self.otro_profesor = Usuario.objects.create_user(username="otro-profe", password="x", rol="PROFESOR")
        self.estudiante = Usuario.objects.create_user(username="est-eval", password="x", rol="ESTUDIANTE")
        self.reto = Reto.objects.create(empresa=self.empresa, titulo="Reto evaluable", estado="en_curso")
        IntegracionAcademica.objects.create(reto=self.reto, profesor=self.profesor, estado="aprobada")
        equipo = Equipo.objects.create(reto=self.reto, nombre="Equipo uno")
        self.entregable = Entregable.objects.create(
            reto=self.reto, equipo=equipo, estudiante=self.estudiante, titulo="Avance 1",
            archivo=SimpleUploadedFile("avance.pdf", b"contenido"),
        )

    def test_profesor_vinculado_califica_y_genera_historial(self):
        self.client.force_login(self.profesor)
        response = self.client.post(reverse("evaluacion:calificar", args=[self.entregable.pk]), {
            "nota": "4.25", "comentario": "Buen avance",
        })
        self.assertRedirects(response, reverse("evaluacion:panel_profesor"))
        self.entregable.refresh_from_db()
        self.assertEqual(str(self.entregable.nota), "4.25")
        self.assertEqual(self.entregable.estado, "CALIFICADO")
        self.assertTrue(Evaluacion.objects.filter(entregable=self.entregable, profesor=self.profesor).exists())
        self.assertTrue(ComentarioEntregable.objects.filter(entregable=self.entregable, autor=self.profesor).exists())

    def test_profesor_no_vinculado_no_puede_calificar(self):
        self.client.force_login(self.otro_profesor)
        response = self.client.post(reverse("evaluacion:calificar", args=[self.entregable.pk]), {"nota": "5"})
        self.assertEqual(response.status_code, 404)

    def test_empresa_no_puede_acceder_a_calificacion(self):
        self.client.force_login(self.empresa)
        response = self.client.post(reverse("evaluacion:calificar", args=[self.entregable.pk]), {"nota": "5"})
        self.assertEqual(response.status_code, 403)
