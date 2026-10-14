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
        # HU12: la nota queda sin publicar; la retroalimentacion pasa al historial al publicarla.
        self.assertFalse(self.entregable.calificacion_publicada)
        self.assertFalse(ComentarioEntregable.objects.filter(entregable=self.entregable).exists())
        self.client.post(reverse("evaluacion:publicar_calificaciones", args=[self.reto.pk]))
        self.entregable.refresh_from_db()
        self.assertTrue(self.entregable.calificacion_publicada)
        self.assertTrue(ComentarioEntregable.objects.filter(entregable=self.entregable, autor=self.profesor).exists())

    def test_profesor_no_vinculado_no_puede_calificar(self):
        self.client.force_login(self.otro_profesor)
        response = self.client.post(reverse("evaluacion:calificar", args=[self.entregable.pk]), {"nota": "5"})
        self.assertEqual(response.status_code, 404)

    def test_empresa_no_puede_acceder_a_calificacion(self):
        self.client.force_login(self.empresa)
        response = self.client.post(reverse("evaluacion:calificar", args=[self.entregable.pk]), {"nota": "5"})
        self.assertEqual(response.status_code, 403)


class PublicacionYRecalificacionTests(TestCase):
    """HU12: publicar calificaciones y confirmar la recalificacion."""

    def setUp(self):
        self.empresa = Usuario.objects.create_user(username="empresa-pub", password="x", rol="EMPRESA")
        self.profesor = Usuario.objects.create_user(username="profe-pub", password="x", rol="PROFESOR")
        self.estudiante = Usuario.objects.create_user(username="est-pub", password="x", rol="ESTUDIANTE")
        self.reto = Reto.objects.create(empresa=self.empresa, titulo="Reto publicable", estado="en_curso")
        IntegracionAcademica.objects.create(reto=self.reto, profesor=self.profesor, estado="publicada")
        self.entregable = Entregable.objects.create(
            reto=self.reto, estudiante=self.estudiante, titulo="Avance",
            archivo=SimpleUploadedFile("a.pdf", b"x"),
        )
        self.client.force_login(self.profesor)
        self.client.post(reverse("evaluacion:calificar", args=[self.entregable.pk]), {"nota": "3", "comentario": "Bien hecho"})

    def test_el_estudiante_no_ve_la_nota_ni_recibe_aviso_hasta_publicar(self):
        from apps.notificaciones.models import Notificacion

        self.assertFalse(Notificacion.objects.filter(usuario=self.estudiante, evento="ENTREGABLE_CALIFICADO").exists())
        self.client.force_login(self.estudiante)
        url = reverse("participaciones:mis_entregables_reto", args=[self.reto.pk])
        self.assertNotContains(self.client.get(url), "Bien hecho")

        self.client.force_login(self.profesor)
        self.client.post(reverse("evaluacion:publicar_calificaciones", args=[self.reto.pk]))
        self.assertTrue(Notificacion.objects.filter(usuario=self.estudiante, evento="ENTREGABLE_CALIFICADO").exists())
        self.client.force_login(self.estudiante)
        self.assertContains(self.client.get(url), "Bien hecho")

    def test_recalificar_exige_confirmacion(self):
        url = reverse("evaluacion:calificar", args=[self.entregable.pk])
        self.client.post(url, {"nota": "5", "comentario": "Corregida"})
        self.entregable.refresh_from_db()
        self.assertEqual(str(self.entregable.nota), "3.00")

        self.client.post(url, {"nota": "5", "comentario": "Corregida", "confirmar_recalificacion": "1"})
        self.entregable.refresh_from_db()
        self.assertEqual(str(self.entregable.nota), "5.00")
        self.assertFalse(self.entregable.calificacion_publicada)

    def test_otro_profesor_no_publica(self):
        otro = Usuario.objects.create_user(username="otro-pub", password="x", rol="PROFESOR")
        self.client.force_login(otro)
        response = self.client.post(reverse("evaluacion:publicar_calificaciones", args=[self.reto.pk]))
        self.assertEqual(response.status_code, 403)
        self.entregable.refresh_from_db()
        self.assertFalse(self.entregable.calificacion_publicada)

