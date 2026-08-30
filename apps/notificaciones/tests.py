from django.core import mail
from django.test import TestCase, override_settings
from django.urls import reverse

from apps.retos.models import Reto
from apps.retos.services import cambiar_estado_reto
from apps.usuarios.models import Usuario

from .models import Notificacion, PreferenciaNotificacion
from .services import notificar, notificar_admins


@override_settings(EMAIL_BACKEND="django.core.mail.backends.locmem.EmailBackend")
class MotorNotificacionesTests(TestCase):
    def setUp(self):
        self.usuario = Usuario.objects.create_user(
            username="destinatario", password="x", rol="ESTUDIANTE",
            email="destinatario@universidadean.edu.co",
        )

    def test_notificar_crea_registro_y_envia_correo(self):
        # El correo sale en transaction.on_commit para no enviarse si la vista
        # que llamo a notificar() termina haciendo rollback.
        with self.captureOnCommitCallbacks(execute=True):
            notificacion = notificar(
                self.usuario, "POSTULACION_GESTIONADA", mensaje="Fuiste aceptado."
            )
        self.assertIsNotNone(notificacion)
        self.assertEqual(notificacion.evento, "POSTULACION_GESTIONADA")
        self.assertFalse(notificacion.leida)
        self.assertEqual(len(mail.outbox), 1)
        self.assertIn("Respuesta a tu postulacion", mail.outbox[0].subject)

    def test_clave_dedupe_evita_duplicados(self):
        primera = notificar(self.usuario, "RETO_ESTADO_CAMBIADO", mensaje="uno", clave_dedupe="k1")
        segunda = notificar(self.usuario, "RETO_ESTADO_CAMBIADO", mensaje="dos", clave_dedupe="k1")
        self.assertIsNotNone(primera)
        self.assertIsNone(segunda)
        self.assertEqual(Notificacion.objects.filter(usuario=self.usuario).count(), 1)

    def test_evento_silenciado_no_genera_notificacion(self):
        PreferenciaNotificacion.objects.create(
            usuario=self.usuario, eventos_silenciados=["ENTREGABLE_CALIFICADO"]
        )
        resultado = notificar(self.usuario, "ENTREGABLE_CALIFICADO", mensaje="4.5")
        self.assertIsNone(resultado)
        self.assertEqual(Notificacion.objects.count(), 0)

    def test_sin_email_activo_solo_queda_in_app(self):
        PreferenciaNotificacion.objects.create(usuario=self.usuario, recibir_email=False)
        notificar(self.usuario, "POSTULACION_NUEVA", mensaje="hola")
        self.assertEqual(Notificacion.objects.count(), 1)
        self.assertEqual(len(mail.outbox), 0)

    def test_notificar_admins_incluye_admin_por_rol(self):
        Usuario.objects.create_superuser(username="super", password="x", email="s@ean.edu.co")
        Usuario.objects.create_user(username="admin-rol", password="x", rol="ADMIN")
        creadas = notificar_admins("POSTULACION_NUEVA", mensaje="nueva postulacion")
        self.assertEqual(len(creadas), 2)


@override_settings(EMAIL_BACKEND="django.core.mail.backends.locmem.EmailBackend")
class NotificacionesDeDominioTests(TestCase):
    """Eventos que antes no avisaban a nadie."""

    def setUp(self):
        self.empresa = Usuario.objects.create_user(
            username="empresa-notif", password="x", rol="EMPRESA",
            email="empresa@example.com",
        )
        self.admin = Usuario.objects.create_superuser(
            username="admin-notif", password="x", email="admin@ean.edu.co"
        )
        self.reto = Reto.objects.create(
            empresa=self.empresa, titulo="Reto notificable", estado="en_revision"
        )

    def test_aprobar_reto_notifica_a_la_empresa(self):
        cambiar_estado_reto(self.reto, "aprobado", self.admin, "Todo correcto.")
        notificacion = Notificacion.objects.get(usuario=self.empresa)
        self.assertEqual(notificacion.evento, "RETO_ESTADO_CAMBIADO")
        self.assertIn("aprobado", notificacion.mensaje.lower())
        self.assertIn("Todo correcto.", notificacion.mensaje)

    def test_rechazar_reto_notifica_con_el_comentario(self):
        cambiar_estado_reto(self.reto, "rechazado", self.admin, "Falta el alcance.")
        notificacion = Notificacion.objects.get(usuario=self.empresa)
        self.assertEqual(notificacion.tipo, "ERROR")
        self.assertIn("Falta el alcance.", notificacion.mensaje)

    def test_estado_sin_cambio_no_notifica(self):
        cambiar_estado_reto(self.reto, "en_revision", self.admin)
        self.assertEqual(Notificacion.objects.filter(usuario=self.empresa).count(), 0)


class BandejaTests(TestCase):
    def setUp(self):
        self.usuario = Usuario.objects.create_user(
            username="lector", password="x", rol="ESTUDIANTE"
        )
        self.notificacion = Notificacion.objects.create(
            usuario=self.usuario, titulo="Aviso", mensaje="cuerpo"
        )

    def test_bandeja_lista_las_notificaciones(self):
        self.client.force_login(self.usuario)
        response = self.client.get(reverse("notificaciones:bandeja"))
        self.assertEqual(response.status_code, 200)
        self.assertContains(response, "Aviso")

    def test_marcar_leida_requiere_post(self):
        self.client.force_login(self.usuario)
        response = self.client.get(
            reverse("notificaciones:marcar_leida", args=[self.notificacion.pk])
        )
        self.assertEqual(response.status_code, 405)

    def test_marcar_leida_actualiza_el_registro(self):
        self.client.force_login(self.usuario)
        self.client.post(reverse("notificaciones:marcar_leida", args=[self.notificacion.pk]))
        self.notificacion.refresh_from_db()
        self.assertTrue(self.notificacion.leida)
        self.assertIsNotNone(self.notificacion.leida_en)

    def test_no_se_puede_leer_la_notificacion_de_otro(self):
        otro = Usuario.objects.create_user(username="otro", password="x", rol="ESTUDIANTE")
        self.client.force_login(otro)
        response = self.client.post(
            reverse("notificaciones:marcar_leida", args=[self.notificacion.pk])
        )
        self.assertEqual(response.status_code, 404)

    def test_el_badge_aparece_en_el_topbar(self):
        self.client.force_login(self.usuario)
        response = self.client.get(reverse("notificaciones:bandeja"))
        self.assertEqual(response.context["notificaciones_no_leidas"], 1)
