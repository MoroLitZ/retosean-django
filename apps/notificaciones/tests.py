from datetime import timedelta
from io import StringIO

from django.core import mail
from django.core.management import call_command
from django.test import TestCase, override_settings
from django.urls import reverse
from django.utils import timezone

from apps.retos.models import Reto
from apps.retos.services import cambiar_estado_reto
from apps.seguimiento.models import IntegracionAcademica
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

    def test_notificar_duplicado_loguea_warning(self):
        """NOT-03: un error de integridad por deduplicacion no se silencia."""
        notificar(self.usuario, "RETO_ESTADO_CAMBIADO", mensaje="uno", clave_dedupe="k2")
        with self.assertLogs("apps.notificaciones.services", level="WARNING") as cm:
            notificar(self.usuario, "RETO_ESTADO_CAMBIADO", mensaje="dos", clave_dedupe="k2")
        self.assertEqual(Notificacion.objects.filter(usuario=self.usuario).count(), 1)
        self.assertTrue(any("duplicada" in mensaje for mensaje in cm.output))


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

    def test_estado_sin_cambio_se_rechaza_y_no_notifica(self):
        from apps.seguimiento.services import TransicionInvalida

        with self.assertRaises(TransicionInvalida):
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


class RetoVencidoSinActualizarTests(TestCase):
    """HU04: alerta al admin cuando un reto pasa su fecha sin seguimientos."""

    def setUp(self):
        from datetime import timedelta

        from django.utils import timezone

        self.hoy = timezone.localdate()
        self.admin = Usuario.objects.create_user(username="admin-venc", password="x", rol="ADMIN")
        empresa = Usuario.objects.create_user(username="empresa-venc", password="x", rol="EMPRESA")
        self.vencido = Reto.objects.create(
            empresa=empresa, titulo="Reto atrasado", estado="en_curso",
            fecha_fin_tentativa=self.hoy - timedelta(days=5),
        )
        self.al_dia = Reto.objects.create(
            empresa=empresa, titulo="Reto al dia", estado="en_curso",
            fecha_fin_tentativa=self.hoy + timedelta(days=30),
        )

    def _ejecutar(self):
        from io import StringIO

        from django.core.management import call_command

        call_command("enviar_recordatorios", stdout=StringIO())
        return Notificacion.objects.filter(usuario=self.admin, evento="RETO_SIN_ACTUALIZAR")

    def test_avisa_al_admin_del_reto_vencido(self):
        avisos = self._ejecutar()
        self.assertEqual(avisos.count(), 1)
        self.assertIn("Reto atrasado", avisos.get().mensaje)

    def test_no_repite_el_aviso_la_misma_semana(self):
        self._ejecutar()
        self.assertEqual(self._ejecutar().count(), 1)

    def test_un_seguimiento_posterior_silencia_la_alerta(self):
        from apps.seguimiento.models import SeguimientoReto

        SeguimientoReto.objects.create(reto=self.vencido, porcentaje_avance=90, avances="Casi listo")
        self.assertEqual(self._ejecutar().count(), 0)


class RegistroEnviosAdminTests(TestCase):
    """HU15: el administrador consulta el estado de las notificaciones."""

    def setUp(self):
        self.admin = Usuario.objects.create_user(username="admin-reg", password="x", rol="ADMIN")
        self.destino = Usuario.objects.create_user(username="destino-reg", password="x", rol="ESTUDIANTE")
        Notificacion.objects.create(usuario=self.destino, titulo="Enviada", mensaje="m", enviada_por_correo=True)
        Notificacion.objects.create(usuario=self.destino, titulo="Con fallo", mensaje="m", error_envio="SMTP caido")
        Notificacion.objects.create(usuario=self.destino, titulo="Leida", mensaje="m", leida=True)

    def test_filtra_por_estado_de_envio(self):
        self.client.force_login(self.admin)
        url = reverse("notificaciones:registro_envios")
        titulos = lambda r: {n.titulo for n in r.context["registros"]}
        self.assertEqual(titulos(self.client.get(url, {"estado": "fallido"})), {"Con fallo"})
        self.assertEqual(titulos(self.client.get(url, {"estado": "enviado"})), {"Enviada"})
        self.assertEqual(titulos(self.client.get(url, {"estado": "leido"})), {"Leida"})
        self.assertEqual(self.client.get(url).context["resumen"]["fallidas"], 1)

    def test_solo_el_admin_lo_ve(self):
        self.client.force_login(self.destino)
        self.assertNotEqual(self.client.get(reverse("notificaciones:registro_envios")).status_code, 200)

    def test_filtro_fecha_invalida_no_genera_500(self):
        """NOT-07: fechas malformadas en el registro de envios se ignoran."""
        self.client.force_login(self.admin)
        url = reverse("notificaciones:registro_envios")
        response = self.client.get(url, {"desde": "31/12/2025", "hasta": "2025-01-01"})
        self.assertEqual(response.status_code, 200)
        self.assertEqual(response.context["filtros"]["desde"], "31/12/2025")
        # La fecha valida si aplica y la malformada no produce error.
        response2 = self.client.get(url, {"desde": "2025-12-31"})
        self.assertEqual(response2.status_code, 200)


@override_settings(EMAIL_BACKEND="django.core.mail.backends.locmem.EmailBackend")
class RecordatoriosCommandTests(TestCase):
    """Regresion para los hallazgos NOT-01, NOT-02, NOT-04, NOT-05, NOT-06."""

    def setUp(self):
        self.hoy = timezone.localdate()
        self.empresa = Usuario.objects.create_user(
            username="empresa-rec", password="x", rol="EMPRESA", email="empresa@rec.co"
        )
        self.reto = Reto.objects.create(
            empresa=self.empresa,
            titulo="Reto recordatorios",
            estado="abierto",
            fecha_inicio_tentativa=self.hoy,
            fecha_fin_tentativa=self.hoy + timedelta(days=30),
            fecha_limite_postulacion=self.hoy + timedelta(days=7),
        )
        self.profesor_activo = Usuario.objects.create_user(
            username="prof-activo", password="x", rol="PROFESOR",
            email="prof-activo@universidadean.edu.co", is_active=True,
        )
        self.profesor_inactivo = Usuario.objects.create_user(
            username="prof-inactivo", password="x", rol="PROFESOR",
            email="prof-inactivo@universidadean.edu.co", is_active=False,
        )
        self.estudiante = Usuario.objects.create_user(
            username="est-rec", password="x", rol="ESTUDIANTE", email="est@rec.co"
        )

    def _ejecutar(self, *args):
        call_command("enviar_recordatorios", *args, stdout=StringIO())

    def test_profesor_inactivo_no_recibe_recordatorio(self):
        """NOT-01: los profesores inactivos se excluyen de los recordatorios."""
        IntegracionAcademica.objects.create(reto=self.reto, profesor=self.profesor_activo)
        IntegracionAcademica.objects.create(reto=self.reto, profesor=self.profesor_inactivo)
        self.reto.estado = "en_curso"
        self.reto.fecha_fin_tentativa = self.hoy + timedelta(days=7)
        self.reto.save(update_fields=["estado", "fecha_fin_tentativa"])

        self._ejecutar("--dias", "7")

        notificaciones = Notificacion.objects.filter(
            evento="RECORDATORIO_FECHA_LIMITE",
            usuario__in=[self.profesor_activo, self.profesor_inactivo],
        )
        self.assertEqual(notificaciones.count(), 1)
        self.assertEqual(notificaciones.get().usuario, self.profesor_activo)

    def test_fecha_limite_reprogramada_genera_nuevo_recordatorio(self):
        """NOT-02: reprogramar una fecha limite crea el recordatorio correspondiente."""
        from apps.participaciones.models import Postulacion

        Postulacion.objects.create(reto=self.reto, estudiante=self.estudiante, estado="PENDIENTE")
        self._ejecutar("--dias", "7")
        self.assertEqual(Notificacion.objects.filter(usuario=self.estudiante).count(), 1)

        # Reprogramamos el cierre de postulaciones: debe llegar el aviso a 3 dias.
        self.reto.fecha_limite_postulacion = self.hoy + timedelta(days=3)
        self.reto.save(update_fields=["fecha_limite_postulacion"])
        self._ejecutar("--dias", "3")
        self.assertEqual(Notificacion.objects.filter(usuario=self.estudiante).count(), 2)

    def test_dry_run_aplica_filtro_de_opt_out(self):
        """NOT-04: --dry-run solo cuenta destinatarios que recibirian el correo."""
        from apps.participaciones.models import Postulacion

        Postulacion.objects.create(reto=self.reto, estudiante=self.estudiante, estado="PENDIENTE")
        PreferenciaNotificacion.objects.create(
            usuario=self.estudiante, recibir_recordatorios=False
        )
        out = StringIO()
        call_command("enviar_recordatorios", "--dias", "7", "--dry-run", stdout=out)
        self.assertIn("0 destinatario(s)", out.getvalue())
        self.assertEqual(Notificacion.objects.filter(usuario=self.estudiante).count(), 0)

    def test_umbral_entregables_sin_calificar_es_fijo(self):
        """NOT-05: --dias no altera el umbral de entregables pendientes."""
        from django.core.files.uploadedfile import SimpleUploadedFile
        from django.utils import timezone

        from apps.evaluacion.models import Entregable

        IntegracionAcademica.objects.create(reto=self.reto, profesor=self.profesor_activo)
        Entregable.objects.create(
            reto=self.reto,
            estudiante=self.estudiante,
            titulo="Entrega",
            archivo=SimpleUploadedFile("x.pdf", b"x"),
        )
        # Fecha de entrega hace 3 dias, menor al umbral fijo de 7.
        Entregable.objects.update(fecha_entrega=timezone.now() - timedelta(days=3))

        self._ejecutar("--dias", "1")
        self.assertEqual(Notificacion.objects.filter(evento="RECORDATORIO_FECHA_LIMITE").count(), 0)

        # Ahora si: hace 10 dias, supera el umbral fijo.
        Entregable.objects.update(fecha_entrega=timezone.now() - timedelta(days=10))
        self._ejecutar("--dias", "1")
        self.assertEqual(Notificacion.objects.filter(evento="RECORDATORIO_FECHA_LIMITE").count(), 1)

    def test_recordatorio_sin_calificar_no_se_duplica_misma_semana(self):
        """NOT-06: un entregable pendiente genera un solo aviso por semana ISO."""
        from django.core.files.uploadedfile import SimpleUploadedFile
        from django.utils import timezone

        from apps.evaluacion.models import Entregable

        IntegracionAcademica.objects.create(reto=self.reto, profesor=self.profesor_activo)
        Entregable.objects.create(
            reto=self.reto,
            estudiante=self.estudiante,
            titulo="Entrega",
            archivo=SimpleUploadedFile("x.pdf", b"x"),
        )
        Entregable.objects.update(fecha_entrega=timezone.now() - timedelta(days=10))

        self._ejecutar("--dias", "7")
        self._ejecutar("--dias", "7")
        self.assertEqual(Notificacion.objects.filter(evento="RECORDATORIO_FECHA_LIMITE").count(), 1)
