from django.core.files.uploadedfile import SimpleUploadedFile
from django.test import TestCase
from django.urls import reverse
from django.utils import timezone

from apps.notificaciones.models import Notificacion
from apps.participaciones.models import Postulacion
from apps.retos.models import HistorialEstadoReto, Reto
from apps.usuarios.models import Usuario

from .models import AgendaCierre, CierreReto, EncuestaSatisfaccion, EntregableFinal


class CierreSprintTests(TestCase):
    def setUp(self):
        self.admin = Usuario.objects.create_user(username="admin-cierre", password="x", rol="ADMIN")
        self.empresa = Usuario.objects.create_user(username="empresa-cierre", password="x", rol="EMPRESA")
        self.estudiante = Usuario.objects.create_user(username="est-cierre", password="x", rol="ESTUDIANTE")
        self.reto = Reto.objects.create(empresa=self.empresa, titulo="Reto para cerrar", estado="en_curso")
        Postulacion.objects.create(reto=self.reto, estudiante=self.estudiante, estado="ACEPTADA")
        self.agenda = AgendaCierre.objects.create(
            reto=self.reto, fecha_hora=timezone.now(), espacio="Auditorio", agenda="Presentacion y premiacion"
        )
        self.cierre = CierreReto.objects.create(
            reto=self.reto, cerrado_por=self.admin,
            acta_url=SimpleUploadedFile("acta.pdf", b"acta"),
        )
        EntregableFinal.objects.create(
            cierre=self.cierre, nombre="Producto final", cargado_por=self.admin,
            archivo=SimpleUploadedFile("final.zip", b"final"),
        )

    def test_finalizar_cambia_estado_notifica_y_genera_encuestas(self):
        self.client.force_login(self.admin)
        response = self.client.post(reverse("cierre:finalizar", args=[self.reto.pk]))
        self.assertRedirects(response, reverse("cierre:gestionar", args=[self.reto.pk]))
        self.reto.refresh_from_db()
        self.assertEqual(self.reto.estado, "finalizado")
        self.assertEqual(EncuestaSatisfaccion.objects.filter(cierre=self.cierre).count(), 2)
        self.assertEqual(Notificacion.objects.filter(titulo=f"Reto finalizado: {self.reto.titulo}").count(), 2)
        self.assertTrue(HistorialEstadoReto.objects.filter(reto=self.reto, estado_nuevo="finalizado").exists())

    def test_empresa_no_puede_finalizar(self):
        self.client.force_login(self.empresa)
        response = self.client.post(reverse("cierre:finalizar", args=[self.reto.pk]))
        self.assertEqual(response.status_code, 403)
        self.reto.refresh_from_db()
        self.assertEqual(self.reto.estado, "en_curso")

    def test_participante_responde_encuesta(self):
        encuesta = EncuestaSatisfaccion.objects.create(cierre=self.cierre, participante=self.estudiante)
        self.client.force_login(self.estudiante)
        response = self.client.post(reverse("cierre:responder_encuesta", args=[encuesta.pk]), {
            "calificacion": 5, "comentario": "Excelente",
        })
        self.assertRedirects(response, reverse("cierre:mis_encuestas"))
        encuesta.refresh_from_db()
        self.assertTrue(encuesta.respondida)
