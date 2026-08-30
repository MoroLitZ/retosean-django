from django.core.files.uploadedfile import SimpleUploadedFile
from django.test import TestCase
from django.urls import reverse

from apps.cierre.models import AgendaCierre, CierreReto, EntregableFinal
from apps.notificaciones.models import Notificacion
from apps.participaciones.models import Postulacion
from apps.retos.models import Reto
from apps.seguimiento.models import IntegracionAcademica
from apps.usuarios.models import Usuario
from django.utils import timezone

from .models import Certificado
from .services import emitir_certificados_de_reto, generar_pdf_certificado


class EmisionDeCertificadosTests(TestCase):
    def setUp(self):
        self.admin = Usuario.objects.create_superuser(
            username="admin-cert", password="x", email="admin@ean.edu.co"
        )
        self.empresa = Usuario.objects.create_user(
            username="empresa-cert", password="x", rol="EMPRESA"
        )
        self.profesor = Usuario.objects.create_user(
            username="profe-cert", password="x", rol="PROFESOR"
        )
        self.estudiante = Usuario.objects.create_user(
            username="est-cert", password="x", rol="ESTUDIANTE",
            first_name="Ana", last_name="Torres",
        )
        self.rechazado = Usuario.objects.create_user(
            username="est-rechazado", password="x", rol="ESTUDIANTE"
        )
        self.reto = Reto.objects.create(
            empresa=self.empresa, titulo="Reto certificable", estado="finalizado"
        )
        IntegracionAcademica.objects.create(
            reto=self.reto, profesor=self.profesor, estado="aprobada"
        )
        Postulacion.objects.create(reto=self.reto, estudiante=self.estudiante, estado="ACEPTADA")
        Postulacion.objects.create(reto=self.reto, estudiante=self.rechazado, estado="RECHAZADA")

    def test_emite_a_estudiante_profesor_y_empresa(self):
        emitir_certificados_de_reto(self.reto, self.admin)
        roles = dict(
            Certificado.objects.filter(reto=self.reto).values_list("usuario_id", "rol_participacion")
        )
        self.assertEqual(roles[self.estudiante.pk], "ESTUDIANTE")
        self.assertEqual(roles[self.profesor.pk], "PROFESOR")
        self.assertEqual(roles[self.empresa.pk], "EMPRESA")

    def test_no_emite_a_postulaciones_rechazadas(self):
        emitir_certificados_de_reto(self.reto, self.admin)
        self.assertFalse(Certificado.objects.filter(usuario=self.rechazado).exists())

    def test_no_emite_si_el_reto_no_esta_finalizado(self):
        self.reto.estado = "en_curso"
        self.reto.save()
        self.assertEqual(emitir_certificados_de_reto(self.reto, self.admin), [])
        self.assertEqual(Certificado.objects.count(), 0)

    def test_es_idempotente(self):
        emitir_certificados_de_reto(self.reto, self.admin)
        total = Certificado.objects.count()
        emitir_certificados_de_reto(self.reto, self.admin)
        self.assertEqual(Certificado.objects.count(), total)

    def test_notifica_a_los_participantes(self):
        emitir_certificados_de_reto(self.reto, self.admin)
        self.assertTrue(
            Notificacion.objects.filter(
                usuario=self.estudiante, evento="CERTIFICADO_DISPONIBLE"
            ).exists()
        )

    def test_genera_pdf_valido(self):
        emitir_certificados_de_reto(self.reto, self.admin)
        certificado = Certificado.objects.get(usuario=self.estudiante)
        contenido = generar_pdf_certificado(certificado)
        self.assertTrue(contenido.startswith(b"%PDF"))


class DescargaYVerificacionTests(TestCase):
    def setUp(self):
        self.empresa = Usuario.objects.create_user(
            username="empresa-desc", password="x", rol="EMPRESA"
        )
        self.estudiante = Usuario.objects.create_user(
            username="est-desc", password="x", rol="ESTUDIANTE"
        )
        self.otro = Usuario.objects.create_user(
            username="est-otro", password="x", rol="ESTUDIANTE"
        )
        reto = Reto.objects.create(
            empresa=self.empresa, titulo="Reto descargable", estado="finalizado"
        )
        self.certificado = Certificado.objects.create(
            reto=reto, usuario=self.estudiante, rol_participacion="ESTUDIANTE"
        )

    def test_el_titular_descarga_su_certificado(self):
        self.client.force_login(self.estudiante)
        response = self.client.get(
            reverse("academico:descargar_certificado", args=[self.certificado.codigo_verificacion])
        )
        self.assertEqual(response.status_code, 200)
        self.assertEqual(response["Content-Type"], "application/pdf")

    def test_otro_estudiante_no_descarga_el_certificado_ajeno(self):
        self.client.force_login(self.otro)
        response = self.client.get(
            reverse("academico:descargar_certificado", args=[self.certificado.codigo_verificacion])
        )
        self.assertRedirects(response, reverse("academico:certificados"))

    def test_la_verificacion_es_publica(self):
        url = reverse("academico:verificar_certificado", args=[self.certificado.codigo_verificacion])
        response = self.client.get(url)  # sin iniciar sesion
        self.assertEqual(response.status_code, 200)
        self.assertIsNotNone(response.context["certificado"])

    def test_codigo_inexistente_no_valida(self):
        url = reverse(
            "academico:verificar_certificado",
            args=["00000000-0000-0000-0000-000000000000"],
        )
        response = self.client.get(url)
        self.assertEqual(response.status_code, 200)
        self.assertIsNone(response.context["certificado"])

    def test_portafolio_devuelve_pdf(self):
        self.client.force_login(self.estudiante)
        response = self.client.get(reverse("academico:portafolio"))
        self.assertEqual(response.status_code, 200)
        self.assertEqual(response["Content-Type"], "application/pdf")


class CierreEmiteCertificadosTests(TestCase):
    """El cierre formal del reto debe disparar la emision (HU08 + HU16)."""

    def test_finalizar_el_reto_emite_certificados(self):
        admin = Usuario.objects.create_superuser(
            username="admin-cierre-cert", password="x", email="a@ean.edu.co"
        )
        empresa = Usuario.objects.create_user(
            username="empresa-cierre", password="x", rol="EMPRESA"
        )
        estudiante = Usuario.objects.create_user(
            username="est-cierre", password="x", rol="ESTUDIANTE"
        )
        reto = Reto.objects.create(empresa=empresa, titulo="Reto a cerrar", estado="en_curso")
        Postulacion.objects.create(reto=reto, estudiante=estudiante, estado="ACEPTADA")

        AgendaCierre.objects.create(
            reto=reto, fecha_hora=timezone.now(), espacio="Auditorio", agenda="Presentaciones"
        )
        cierre = CierreReto.objects.create(
            reto=reto, acta_url=SimpleUploadedFile("acta.pdf", b"acta")
        )
        EntregableFinal.objects.create(
            cierre=cierre, nombre="Informe final",
            archivo=SimpleUploadedFile("final.pdf", b"informe"),
        )

        self.client.force_login(admin)
        self.client.post(reverse("cierre:finalizar", args=[reto.pk]))

        reto.refresh_from_db()
        self.assertEqual(reto.estado, "finalizado")
        self.assertTrue(
            Certificado.objects.filter(reto=reto, usuario=estudiante).exists()
        )
