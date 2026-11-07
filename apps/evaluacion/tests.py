from decimal import Decimal

from django.core.files.uploadedfile import SimpleUploadedFile
from django.test import TestCase
from django.urls import reverse

from apps.participaciones.models import Equipo
from apps.retos.models import Reto
from apps.seguimiento.models import IntegracionAcademica
from apps.usuarios.models import Usuario

from .models import ComentarioEntregable, Entregable, Evaluacion, Rubrica, CriterioRubrica


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


class IntegracionInactivaNoCalificaTests(TestCase):
    """PAR-02: solo integraciones activas (aprobada/publicada) pueden calificar/publicar."""

    def setUp(self):
        self.empresa = Usuario.objects.create_user(username="empresa-inact", password="x", rol="EMPRESA")
        self.profesor = Usuario.objects.create_user(username="profe-inact", password="x", rol="PROFESOR")
        self.estudiante = Usuario.objects.create_user(username="est-inact", password="x", rol="ESTUDIANTE")
        self.reto = Reto.objects.create(empresa=self.empresa, titulo="Reto inactivo", estado="en_curso")
        self.entregable = Entregable.objects.create(
            reto=self.reto, estudiante=self.estudiante, titulo="Avance",
            archivo=SimpleUploadedFile("a.pdf", b"x"),
        )

    def test_profesor_con_integracion_rechazada_no_puede_calificar_ni_publicar(self):
        IntegracionAcademica.objects.create(reto=self.reto, profesor=self.profesor, estado="rechazada")
        self.client.force_login(self.profesor)

        response = self.client.get(reverse("evaluacion:panel_profesor"))
        self.assertEqual(response.status_code, 200)
        self.assertNotContains(response, "Avance")

        response = self.client.post(
            reverse("evaluacion:calificar", args=[self.entregable.pk]), {"nota": "5"}
        )
        self.assertEqual(response.status_code, 404)

        response = self.client.post(reverse("evaluacion:publicar_calificaciones", args=[self.reto.pk]))
        self.assertEqual(response.status_code, 403)


class EscalaRubricaTests(TestCase):
    """PAR-01: el total de la rubrica no puede superar la escala del entregable."""

    def setUp(self):
        self.empresa = Usuario.objects.create_user(username="empresa-rub", password="x", rol="EMPRESA")
        self.profesor = Usuario.objects.create_user(username="profe-rub", password="x", rol="PROFESOR")
        self.estudiante = Usuario.objects.create_user(username="est-rub", password="x", rol="ESTUDIANTE")
        self.reto = Reto.objects.create(empresa=self.empresa, titulo="Reto rubrica", estado="en_curso")
        IntegracionAcademica.objects.create(reto=self.reto, profesor=self.profesor, estado="publicada")
        self.entregable = Entregable.objects.create(
            reto=self.reto, estudiante=self.estudiante, titulo="Avance",
            archivo=SimpleUploadedFile("a.pdf", b"x"),
        )
        self.rubrica = Rubrica.objects.create(reto=self.reto, nombre="Rubrica", activa=True)
        self.c1 = CriterioRubrica.objects.create(
            rubrica=self.rubrica, nombre="C1", puntaje_maximo=Decimal("3.00"), orden=0
        )
        self.c2 = CriterioRubrica.objects.create(
            rubrica=self.rubrica, nombre="C2", puntaje_maximo=Decimal("3.00"), orden=1
        )

    def test_rechaza_nota_total_de_rubrica_superior_al_puntaje_maximo(self):
        self.client.force_login(self.profesor)
        self.client.post(reverse("evaluacion:calificar", args=[self.entregable.pk]), {
            "rubrica": self.rubrica.pk,
            f"criterio_{self.c1.pk}": "3",
            f"criterio_{self.c2.pk}": "3",
        })
        self.entregable.refresh_from_db()
        self.assertIsNone(self.entregable.nota)
        self.assertEqual(self.entregable.estado, "ENVIADO")


class RedireccionSeguraCalificacionTests(TestCase):
    """PAR-05: las vistas de evaluacion no redirigen a URLs externas."""

    def setUp(self):
        self.empresa = Usuario.objects.create_user(username="empresa-red", password="x", rol="EMPRESA")
        self.profesor = Usuario.objects.create_user(username="profe-red", password="x", rol="PROFESOR")
        self.estudiante = Usuario.objects.create_user(username="est-red", password="x", rol="ESTUDIANTE")
        self.reto = Reto.objects.create(empresa=self.empresa, titulo="Reto redir", estado="en_curso")
        IntegracionAcademica.objects.create(reto=self.reto, profesor=self.profesor, estado="publicada")
        self.entregable = Entregable.objects.create(
            reto=self.reto, estudiante=self.estudiante, titulo="Avance",
            archivo=SimpleUploadedFile("a.pdf", b"x"),
        )

    def test_calificar_no_redirige_a_dominio_externo(self):
        self.client.force_login(self.profesor)
        response = self.client.post(
            reverse("evaluacion:calificar", args=[self.entregable.pk]),
            {"nota": "4", "next": "https://evil.example.com/x"},
        )
        self.assertRedirects(response, reverse("evaluacion:panel_profesor"))

    def test_publicar_no_redirige_a_dominio_externo(self):
        self.client.force_login(self.profesor)
        response = self.client.post(
            reverse("evaluacion:publicar_calificaciones", args=[self.reto.pk]),
            {"next": "https://evil.example.com/x"},
        )
        self.assertEqual(response.status_code, 302)
        self.assertEqual(response.url, reverse("evaluacion:panel_profesor"))


class PanelNoMutanteTests(TestCase):
    """PAR-07: GET del panel del profesor no muta el estado de los entregables."""

    def setUp(self):
        self.empresa = Usuario.objects.create_user(username="empresa-panel", password="x", rol="EMPRESA")
        self.profesor = Usuario.objects.create_user(username="profe-panel", password="x", rol="PROFESOR")
        self.estudiante = Usuario.objects.create_user(username="est-panel", password="x", rol="ESTUDIANTE")
        self.reto = Reto.objects.create(empresa=self.empresa, titulo="Reto panel", estado="en_curso")
        IntegracionAcademica.objects.create(reto=self.reto, profesor=self.profesor, estado="publicada")
        self.entregable = Entregable.objects.create(
            reto=self.reto, estudiante=self.estudiante, titulo="Avance",
            archivo=SimpleUploadedFile("a.pdf", b"x"),
        )

    def test_get_panel_profesor_no_cambia_estado_a_en_revision(self):
        self.client.force_login(self.profesor)
        response = self.client.get(reverse("evaluacion:panel_profesor"))
        self.assertEqual(response.status_code, 200)
        self.entregable.refresh_from_db()
        self.assertEqual(self.entregable.estado, "ENVIADO")
