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
        self.client.force_login(self.admin)
        documentos = [self._documento(t) for t in ("RUT", "CAMARA_COMERCIO", "CEDULA_REPRESENTANTE")]
        convenio = self._documento("CONVENIO")

        for documento in documentos:
            self.client.post(reverse("empresas:procesar_aprobacion", args=[documento.pk]),
                             {"nuevo_estado": "VERIFICADO"})
        self.empresa.refresh_from_db()
        self.assertEqual(self.empresa.estado_validacion, "EN_REVISION")

        self.client.post(reverse("empresas:procesar_aprobacion", args=[convenio.pk]),
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


class SeguimientoAvanceEmpresaTests(TestCase):
    """Panel de solo lectura del avance para la empresa (HU14)."""

    def setUp(self):
        from apps.retos.models import Reto
        from apps.seguimiento.models import SeguimientoReto

        self.empresa = Usuario.objects.create_user(username="empresa-seg", password="x", rol="EMPRESA")
        self.otra = Usuario.objects.create_user(username="empresa-otra", password="x", rol="EMPRESA")
        self.reto = Reto.objects.create(empresa=self.empresa, titulo="Reto monitoreado", estado="en_curso")
        self.ajeno = Reto.objects.create(empresa=self.otra, titulo="Reto ajeno", estado="en_curso")
        SeguimientoReto.objects.create(reto=self.reto, porcentaje_avance=20, avances="Kickoff",
                                       fecha_sesion=date.today() - timedelta(days=7))
        SeguimientoReto.objects.create(reto=self.reto, porcentaje_avance=60, avances="Prototipo",
                                       fecha_sesion=date.today())

    def test_muestra_el_porcentaje_del_ultimo_seguimiento(self):
        self.client.force_login(self.empresa)
        response = self.client.get(reverse("empresas:seguimiento_reto", args=[self.reto.pk]))
        self.assertEqual(response.status_code, 200)
        self.assertEqual(response.context["porcentaje"], 60)
        self.assertContains(response, "Prototipo")

    def test_no_ve_retos_de_otra_empresa(self):
        self.client.force_login(self.empresa)
        response = self.client.get(reverse("empresas:seguimiento_reto", args=[self.ajeno.pk]))
        self.assertEqual(response.status_code, 404)

    def test_otros_roles_no_acceden(self):
        estudiante = Usuario.objects.create_user(username="est-seg", password="x", rol="ESTUDIANTE")
        self.client.force_login(estudiante)
        response = self.client.get(reverse("empresas:seguimiento_avance"))
        self.assertNotEqual(response.status_code, 200)


class ParticipacionSegunDocumentosTests(TestCase):
    """HU00: una empresa solo participa cuando sus documentos requeridos estan verificados."""

    def setUp(self):
        from apps.participaciones.models import Postulacion
        from apps.retos.models import Reto
        from apps.seguimiento.services import TransicionInvalida

        self.TransicionInvalida = TransicionInvalida
        self.Postulacion = Postulacion
        self.admin = Usuario.objects.create_superuser(
            username="admin-part-doc", password="x", email="a@ean.edu.co"
        )
        self.usuario = Usuario.objects.create_user(
            username="empresa-part-doc", password="x", rol="EMPRESA"
        )
        self.empresa = Empresa.objects.create(
            usuario=self.usuario, nit="900777777", razon_social="Beta SAS"
        )
        self.reto = Reto.objects.create(
            empresa=self.usuario, titulo="Reto de Beta", estado="en_revision"
        )
        self.estudiante = Usuario.objects.create_user(
            username="est-part-doc", password="x", rol="ESTUDIANTE"
        )

    def _verificar(self, tipo, expedicion=None):
        return DocumentoEmpresa.objects.create(
            empresa=self.empresa, tipo_documento=tipo, estado="VERIFICADO",
            archivo=SimpleUploadedFile(f"{tipo}.pdf", b"x"),
            fecha_expedicion=expedicion,
        )

    def _verificar_todo(self):
        from datetime import date as fecha
        self._verificar("CAMARA_COMERCIO", expedicion=fecha.today())
        for tipo in ("RUT", "CEDULA_REPRESENTANTE", "CONVENIO"):
            self._verificar(tipo)

    def test_lista_los_documentos_que_faltan(self):
        from .services import documentos_faltantes
        self._verificar("RUT")
        faltantes = {f["tipo"] for f in documentos_faltantes(self.empresa)}
        self.assertEqual(faltantes, {"CAMARA_COMERCIO", "CEDULA_REPRESENTANTE", "CONVENIO"})

    def test_con_todo_verificado_no_hay_faltantes(self):
        from .services import documentos_faltantes, puede_la_empresa_operar
        self._verificar_todo()
        self.assertEqual(documentos_faltantes(self.empresa), [])
        self.assertTrue(puede_la_empresa_operar(self.empresa))

    def test_camara_de_mas_de_30_dias_cuenta_como_vencida(self):
        from datetime import date as fecha, timedelta
        from .services import documentos_faltantes
        self._verificar("CAMARA_COMERCIO", expedicion=fecha.today() - timedelta(days=31))
        for tipo in ("RUT", "CEDULA_REPRESENTANTE", "CONVENIO"):
            self._verificar(tipo)
        motivos = {f["tipo"]: f["motivo"] for f in documentos_faltantes(self.empresa)}
        self.assertIn("CAMARA_COMERCIO", motivos)
        self.assertIn("30 dias", motivos["CAMARA_COMERCIO"])

    def test_renunciar_al_convenio_no_exige_convenio(self):
        from .services import documentos_faltantes
        self.empresa.renuncio_a_convenio = True
        self.empresa.save()
        self._verificar("CAMARA_COMERCIO", expedicion=date.today())
        self._verificar("RUT")
        self._verificar("CEDULA_REPRESENTANTE")
        self.assertEqual(documentos_faltantes(self.empresa), [])

    def test_admin_no_aprueba_reto_de_empresa_incompleta(self):
        self._verificar("RUT")
        self.client.force_login(self.admin)
        self.client.post(reverse("retos:admin_revisar", args=[self.reto.pk]),
                         {"accion": "aprobar", "comentario": "ok"})
        self.reto.refresh_from_db()
        self.assertNotEqual(self.reto.estado, "aprobado")

    def test_admin_aprueba_reto_de_empresa_completa(self):
        self._verificar_todo()
        self.client.force_login(self.admin)
        self.client.post(reverse("retos:admin_revisar", args=[self.reto.pk]),
                         {"accion": "aprobar", "comentario": "ok"})
        self.reto.refresh_from_db()
        self.assertEqual(self.reto.estado, "aprobado")

    def test_empresa_incompleta_no_acepta_postulaciones(self):
        postulacion = self.Postulacion.objects.create(
            reto=self.reto, estudiante=self.estudiante, estado="PENDIENTE"
        )
        self.client.force_login(self.usuario)
        self.client.post(reverse("empresas:gestionar_postulacion", args=[postulacion.pk]),
                         {"accion": "aceptar"})
        postulacion.refresh_from_db()
        self.assertEqual(postulacion.estado, "PENDIENTE")


class VencimientoEHistorialDocumentalTests(TestCase):
    """HU00: vencimiento automatico (con aviso a la empresa) e historial consultable."""

    def setUp(self):
        self.admin = Usuario.objects.create_superuser(
            username="admin-venc", password="x", email="a@ean.edu.co"
        )
        self.usuario = Usuario.objects.create_user(
            username="empresa-venc", password="x", rol="EMPRESA", email="e@example.com"
        )
        self.empresa = Empresa.objects.create(
            usuario=self.usuario, nit="900888888", razon_social="Gamma SAS"
        )

    def _camara(self, dias_atras, estado="VERIFICADO"):
        return DocumentoEmpresa.objects.create(
            empresa=self.empresa, tipo_documento="CAMARA_COMERCIO", estado=estado,
            archivo=SimpleUploadedFile("c.pdf", b"x"),
            fecha_expedicion=date.today() - timedelta(days=dias_atras),
        )

    def _correr_comando(self):
        from django.core.management import call_command
        call_command("enviar_recordatorios", "--dias", "1", verbosity=0)

    def test_el_comando_vence_la_camara_de_mas_de_30_dias_y_avisa(self):
        camara = self._camara(dias_atras=45)
        self._correr_comando()
        camara.refresh_from_db()
        self.assertEqual(camara.estado, "VENCIDO")
        aviso = Notificacion.objects.get(usuario=self.usuario, evento="DOCUMENTO_REVISADO")
        self.assertEqual(aviso.tipo, "ADVERTENCIA")
        self.assertIn("vencido", aviso.mensaje.lower())

    def test_el_comando_no_toca_una_camara_vigente(self):
        camara = self._camara(dias_atras=10)
        self._correr_comando()
        camara.refresh_from_db()
        self.assertEqual(camara.estado, "VERIFICADO")

    def test_el_comando_es_idempotente(self):
        camara = self._camara(dias_atras=45)
        self._correr_comando()
        self._correr_comando()
        camara.refresh_from_db()
        self.assertEqual(camara.estado, "VENCIDO")
        self.assertEqual(Notificacion.objects.filter(usuario=self.usuario, evento="DOCUMENTO_REVISADO").count(), 1)

    def test_el_vencimiento_queda_en_el_historial(self):
        from .services import historial_documental
        self._camara(dias_atras=45)
        self._correr_comando()
        nuevos = [h["nuevo"] for h in historial_documental(self.empresa)]
        self.assertIn("VENCIDO", nuevos)

    def test_historial_incluye_revisiones_del_admin(self):
        from .services import historial_documental
        documento = DocumentoEmpresa.objects.create(
            empresa=self.empresa, tipo_documento="RUT", estado="CARGADO",
            archivo=SimpleUploadedFile("r.pdf", b"x"),
        )
        self.client.force_login(self.admin)
        self.client.post(reverse("empresas:procesar_aprobacion", args=[documento.pk]),
                         {"nuevo_estado": "RECHAZADO", "motivo": "Ilegible"})
        entrada = historial_documental(self.empresa)[0]
        self.assertEqual(entrada["nuevo"], "RECHAZADO")
        self.assertEqual(entrada["comentario"], "Ilegible")
        self.assertEqual(entrada["usuario"], self.admin)

    def test_pantallas_muestran_el_historial(self):
        self._camara(dias_atras=45)
        self._correr_comando()
        self.client.force_login(self.usuario)
        self.assertContains(self.client.get(reverse("empresas:documentos")), "Historial documental")
        self.client.force_login(self.admin)
        self.assertContains(
            self.client.get(reverse("empresas:admin_revisar_documentacion", args=[self.empresa.pk])),
            "Historial documental",
        )


class ReglasBordeDocumentosTests(TestCase):
    """Casos limite de HU00 detectados en la revision del codigo."""

    def setUp(self):
        from apps.retos.models import Reto
        self.admin = Usuario.objects.create_superuser(username="admin-borde", password="x", email="a@ean.edu.co")
        self.usuario = Usuario.objects.create_user(username="empresa-borde", password="x", rol="EMPRESA")
        self.empresa = Empresa.objects.create(usuario=self.usuario, nit="900999999", razon_social="Delta SAS")
        self.reto = Reto.objects.create(empresa=self.usuario, titulo="Reto de Delta", estado="en_revision")

    def _doc(self, tipo, estado="VERIFICADO", **extra):
        return DocumentoEmpresa.objects.create(
            empresa=self.empresa, tipo_documento=tipo, estado=estado,
            archivo=SimpleUploadedFile(f"{tipo}.pdf", b"x"), **extra,
        )

    def test_quien_renuncia_al_convenio_queda_exento_de_los_documentos(self):
        from .services import documentos_faltantes, puede_la_empresa_operar, verificar_participacion
        self.empresa.renuncio_a_convenio = True
        self.empresa.save()
        self.assertEqual(documentos_faltantes(self.empresa), [])
        self.assertTrue(puede_la_empresa_operar(self.empresa))
        verificar_participacion(self.empresa)  # no lanza
        self.client.force_login(self.admin)
        self.client.post(reverse("retos:admin_revisar", args=[self.reto.pk]),
                         {"accion": "aprobar", "comentario": "ok"})
        self.reto.refresh_from_db()
        self.assertEqual(self.reto.estado, "aprobado")

    def test_camara_sin_fecha_de_expedicion_no_cuenta_como_vigente(self):
        from .services import documentos_faltantes
        camara = self._doc("CAMARA_COMERCIO", fecha_expedicion=date.today())
        DocumentoEmpresa.objects.filter(pk=camara.pk).update(fecha_expedicion=None)  # dato antiguo
        motivos = {f["tipo"]: f["motivo"] for f in documentos_faltantes(self.empresa)}
        self.assertEqual(motivos["CAMARA_COMERCIO"], "Sin fecha de expedicion")

    def test_recargar_un_documento_vencido_limpia_su_vencimiento(self):
        from .services import cargar_documento, documentos_faltantes, revisar_documento
        rut = self._doc("RUT", fecha_vencimiento=date.today() - timedelta(days=1))
        self.assertEqual({f["tipo"]: f["motivo"] for f in documentos_faltantes(self.empresa)}["RUT"], "Vencido")
        cargar_documento(self.empresa, "RUT", SimpleUploadedFile("nuevo.pdf", b"y"), None, self.usuario)
        rut.refresh_from_db()
        self.assertIsNone(rut.fecha_vencimiento)
        revisar_documento(rut, "VERIFICADO", self.admin)
        self.assertNotIn("RUT", {f["tipo"] for f in documentos_faltantes(self.empresa)})
