from datetime import date, timedelta

from django.core.files.uploadedfile import SimpleUploadedFile
from django.test import TestCase
from django.urls import reverse

from apps.notificaciones.models import Notificacion
from apps.usuarios.models import Usuario

from .models import DocumentoEmpresa, Empresa
from .services import puede_la_empresa_operar


class DocumentacionEmpresaTests(TestCase):
    def setUp(self):
        self.admin = Usuario.objects.create_superuser(
            username="admin-doc", password="x", email="admin@ean.edu.co"
        )
        self.usuario_empresa = Usuario.objects.create_user(
            username="empresa-doc", password="x", rol="EMPRESA", email="empresa@example.com"
        )
        self.empresa = Empresa.objects.create(
            usuario=self.usuario_empresa, nit="900123456", razon_social="ACME SAS"
        )

    def _documento(self, tipo, estado="CARGADO"):
        return DocumentoEmpresa.objects.create(
            empresa=self.empresa, tipo_documento=tipo, estado=estado,
            archivo=SimpleUploadedFile(f"{tipo}.pdf", b"contenido"),
            fecha_expedicion=date.today() if tipo == "CAMARA_COMERCIO" else None,
        )

    def test_rechazar_documento_notifica_a_la_empresa(self):
        documento = self._documento("RUT")
        self.client.force_login(self.admin)
        self.client.post(reverse("empresas:procesar_aprobacion", args=[documento.pk]), {
            "nuevo_estado": "RECHAZADO", "motivo": "El archivo esta ilegible.",
        })
        documento.refresh_from_db()
        self.assertEqual(documento.estado, "RECHAZADO")
        notificacion = Notificacion.objects.get(usuario=self.usuario_empresa)
        self.assertEqual(notificacion.evento, "DOCUMENTO_REVISADO")
        self.assertIn("El archivo esta ilegible.", notificacion.mensaje)

    def test_aprobar_documento_notifica_a_la_empresa(self):
        documento = self._documento("RUT")
        self.client.force_login(self.admin)
        self.client.post(reverse("empresas:procesar_aprobacion", args=[documento.pk]), {
            "nuevo_estado": "VERIFICADO",
        })
        notificacion = Notificacion.objects.get(usuario=self.usuario_empresa)
        self.assertEqual(notificacion.tipo, "EXITO")

    def test_cargar_documento_notifica_al_admin(self):
        self.client.force_login(self.usuario_empresa)
        self.client.post(
            reverse("empresas:documentos"),
            {
                "tipo_documento": "RUT",
                "archivo": SimpleUploadedFile("rut.pdf", b"contenido"),
                "fecha_expedicion": date.today().isoformat(),
            },
        )

        notificacion = Notificacion.objects.get(
            usuario=self.admin,
            evento="DOCUMENTO_CARGADO_ADMIN",
        )
        self.assertIn(self.empresa.razon_social, notificacion.mensaje)
        self.assertIn("RUT", notificacion.mensaje)

    def test_el_estado_de_validacion_avanza_a_verificada(self):
        """Antes solo se escribia RECHAZADA y la UI siempre mostraba Pendiente."""
        rut = self._documento("RUT")
        camara = self._documento("CAMARA_COMERCIO")
        self.client.force_login(self.admin)

        self.client.post(reverse("empresas:procesar_aprobacion", args=[rut.pk]),
                         {"nuevo_estado": "VERIFICADO"})
        self.empresa.refresh_from_db()
        self.assertEqual(self.empresa.estado_validacion, "EN_REVISION")

        self.client.post(reverse("empresas:procesar_aprobacion", args=[camara.pk]),
                         {"nuevo_estado": "VERIFICADO"})
        self.empresa.refresh_from_db()
        self.assertEqual(self.empresa.estado_validacion, "VERIFICADA")
        self.assertTrue(puede_la_empresa_operar(self.empresa))

    def test_un_documento_rechazado_deja_la_empresa_rechazada(self):
        documento = self._documento("RUT")
        self.client.force_login(self.admin)
        self.client.post(reverse("empresas:procesar_aprobacion", args=[documento.pk]),
                         {"nuevo_estado": "RECHAZADO", "motivo": "No sirve"})
        self.empresa.refresh_from_db()
        self.assertEqual(self.empresa.estado_validacion, "RECHAZADA")


class VigenciaCamaraComercioTests(TestCase):
    """HU00: la Camara de Comercio debe tener menos de 30 dias."""

    def setUp(self):
        usuario = Usuario.objects.create_user(
            username="empresa-vig", password="x", rol="EMPRESA"
        )
        self.empresa = Empresa.objects.create(
            usuario=usuario, nit="900999888", razon_social="Vigencia SAS"
        )

    def _form(self, dias_atras):
        from .forms import CargarDocumentoForm

        return CargarDocumentoForm(
            data={
                "tipo_documento": "CAMARA_COMERCIO",
                "fecha_expedicion": date.today() - timedelta(days=dias_atras),
            },
            files={"archivo": SimpleUploadedFile("camara.pdf", b"contenido")},
        )

    def test_documento_reciente_es_valido(self):
        self.assertTrue(self._form(10).is_valid())

    def test_documento_de_mas_de_30_dias_se_rechaza(self):
        self.assertFalse(self._form(45).is_valid())


class GateDeDocumentacionTests(TestCase):
    def setUp(self):
        self.usuario = Usuario.objects.create_user(
            username="empresa-gate", password="x", rol="EMPRESA"
        )
        self.empresa = Empresa.objects.create(
            usuario=self.usuario, nit="900777666", razon_social="Gate SAS"
        )

    def test_sin_documentos_no_puede_publicar_retos(self):
        self.client.force_login(self.usuario)
        response = self.client.get(reverse("retos:crear"))
        self.assertRedirects(
            response, reverse("empresas:elegir_convenio"), fetch_redirect_response=False
        )

    def test_renunciando_al_convenio_si_puede_operar(self):
        self.empresa.renuncio_a_convenio = True
        self.empresa.save()
        self.assertTrue(puede_la_empresa_operar(self.empresa))


class ConvenioEmpresaTests(TestCase):
    """Tramite del convenio (HU00): No radicado -> Radicado -> Juridica -> Firmado."""

    def setUp(self):
        self.admin = Usuario.objects.create_user(username="admin-conv", password="x", rol="ADMIN")
        self.usuario_empresa = Usuario.objects.create_user(username="empresa-conv", password="x", rol="EMPRESA")
        self.empresa = Empresa.objects.create(usuario=self.usuario_empresa, nit="900333444", razon_social="Conv SAS")
        self.url = reverse("empresas:actualizar_convenio", args=[self.empresa.pk])

    def _avanzar(self, estado, **extra):
        self.client.force_login(self.admin)
        return self.client.post(self.url, {"estado_convenio": estado, **extra})

    def test_tramite_completo_queda_firmado_con_vencimiento_e_historial(self):
        from apps.seguimiento.services import historial_de

        vence = date.today() + timedelta(days=365)
        self._avanzar("RADICADO")
        self._avanzar("EN_JURIDICA", comentario="En revision de Juridica")
        self._avanzar("FIRMADO", fecha_vencimiento=vence.isoformat())

        self.empresa.refresh_from_db()
        self.assertEqual(self.empresa.estado_convenio, "FIRMADO")
        self.assertEqual(self.empresa.fecha_vencimiento_convenio, vence)
        self.assertTrue(self.empresa.tiene_convenio_vigente)
        pasos = list(historial_de(self.empresa, campo="estado_convenio").order_by("fecha", "id")
                     .values_list("estado_anterior", "estado_nuevo"))
        self.assertEqual(pasos, [
            ("NO_RADICADO", "RADICADO"), ("RADICADO", "EN_JURIDICA"), ("EN_JURIDICA", "FIRMADO"),
        ])
        self.assertTrue(Notificacion.objects.filter(
            usuario=self.usuario_empresa, evento="CONVENIO_ACTUALIZADO", tipo="EXITO"
        ).exists())

    def test_no_se_salta_juridica(self):
        self._avanzar("RADICADO")
        self._avanzar("FIRMADO", fecha_vencimiento=(date.today() + timedelta(days=30)).isoformat())
        self.empresa.refresh_from_db()
        self.assertEqual(self.empresa.estado_convenio, "RADICADO")

    def test_firmar_exige_fecha_de_vencimiento(self):
        self._avanzar("FIRMADO")
        self.empresa.refresh_from_db()
        self.assertEqual(self.empresa.estado_convenio, "NO_RADICADO")

    def test_convenio_vigente_registrado_directamente(self):
        """Respuesta "Si" del brief: la empresa ya llega con convenio firmado."""
        self._avanzar("FIRMADO", fecha_vencimiento=(date.today() + timedelta(days=30)).isoformat())
        self.empresa.refresh_from_db()
        self.assertTrue(self.empresa.tiene_convenio_vigente)

    def test_convenio_vencido_no_esta_vigente(self):
        self.empresa.estado_convenio = "FIRMADO"
        self.empresa.fecha_vencimiento_convenio = date.today() - timedelta(days=1)
        self.assertFalse(self.empresa.tiene_convenio_vigente)

    def test_solo_el_admin_actualiza_el_convenio(self):
        self.client.force_login(self.usuario_empresa)
        self.client.post(self.url, {"estado_convenio": "RADICADO"})
        self.empresa.refresh_from_db()
        self.assertEqual(self.empresa.estado_convenio, "NO_RADICADO")
