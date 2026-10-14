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
        """Antes solo se escribia RECHAZADA y la UI siempre mostraba Pendiente.

        Con el catalogo (HU00) la empresa queda verificada solo cuando el
        ultimo documento obligatorio pendiente se aprueba.
        """
        from .testing import habilitar_empresa

        habilitar_empresa(self.empresa)
        rut = DocumentoEmpresa.objects.get(empresa=self.empresa, tipo_documento="RUT")
        rut.estado = "CARGADO"
        rut.save()
        self.client.force_login(self.admin)

        self.client.post(reverse("empresas:procesar_aprobacion", args=[rut.pk]),
                         {"nuevo_estado": "VERIFICADO"})
        self.empresa.refresh_from_db()
        self.assertEqual(self.empresa.estado_validacion, "VERIFICADA")
        self.assertTrue(puede_la_empresa_operar(self.empresa))

    def test_con_solo_rut_y_camara_aun_no_puede_operar(self):
        self._documento("RUT", estado="VERIFICADO")
        self._documento("CAMARA_COMERCIO", estado="VERIFICADO")
        self.assertFalse(puede_la_empresa_operar(self.empresa))

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

    def test_sin_convenio_no_se_salta_la_documentacion(self):
        """Elegir participar sin convenio ya no habilita a la empresa por si solo."""
        self.empresa.renuncio_a_convenio = True
        self.empresa.save()
        self.assertFalse(puede_la_empresa_operar(self.empresa))

    def test_sin_convenio_no_se_exigen_los_documentos_del_convenio(self):
        from .services import tipos_requeridos

        self.empresa.renuncio_a_convenio = True
        codigos = {tipo.codigo for tipo in tipos_requeridos(self.empresa)}
        self.assertNotIn("MINUTA_CONVENIO", codigos)
        self.assertIn("RUT", codigos)

        self.empresa.renuncio_a_convenio = False
        self.assertIn("MINUTA_CONVENIO", {tipo.codigo for tipo in tipos_requeridos(self.empresa)})

    def test_con_todos_los_obligatorios_verificados_puede_publicar(self):
        from .testing import habilitar_empresa

        habilitar_empresa(self.empresa)
        self.assertTrue(puede_la_empresa_operar(self.empresa))
        self.client.force_login(self.usuario)
        self.assertEqual(self.client.get(reverse("retos:crear")).status_code, 200)

    def test_documento_vencido_deshabilita_a_la_empresa(self):
        from .testing import habilitar_empresa

        habilitar_empresa(self.empresa)
        camara = DocumentoEmpresa.objects.get(empresa=self.empresa, tipo_documento="CAMARA_COMERCIO")
        camara.fecha_vencimiento = date.today() - timedelta(days=1)
        camara.save()
        self.assertFalse(puede_la_empresa_operar(self.empresa))


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


class CatalogoDocumentosTests(TestCase):
    """Catalogo configurable, permiso especial, formato y bloqueo (HU00)."""

    def setUp(self):
        from django.contrib.auth.models import Permission

        self.admin = Usuario.objects.create_user(username="admin-cat", password="x", rol="ADMIN")
        self.lider = Usuario.objects.create_user(username="lider-facultad", password="x", rol="ADMIN")
        self.lider.user_permissions.add(Permission.objects.get(codename="cargar_listas_informa"))
        self.usuario_empresa = Usuario.objects.create_user(username="empresa-cat", password="x", rol="EMPRESA")
        self.empresa = Empresa.objects.create(usuario=self.usuario_empresa, nit="900888999", razon_social="Cat SAS")

    def _pdf(self, nombre="doc.pdf"):
        return SimpleUploadedFile(nombre, b"%PDF-1.4")

    def test_catalogo_trae_los_ocho_documentos_obligatorios_del_brief(self):
        from .models import TipoDocumentoLegal

        obligatorios = set(TipoDocumentoLegal.objects.filter(obligatorio=True, activo=True).values_list("codigo", flat=True))
        self.assertEqual(obligatorios, {
            "MINUTA_CONVENIO", "CAMARA_COMERCIO", "RUT", "CEDULA_REPRESENTANTE",
            "LISTAS_INFORMA", "FORMATO_COMPROMISO", "TERMINOS_REFERENCIA", "FICHA_RETO",
        })

    def test_la_empresa_no_puede_cargar_listas_informa(self):
        from .forms import CargarDocumentoForm

        opciones = dict(CargarDocumentoForm().fields["tipo_documento"].choices)
        self.assertNotIn("LISTAS_INFORMA", opciones)
        self.client.force_login(self.usuario_empresa)
        self.client.post(reverse("empresas:documentos"), {
            "tipo_documento": "LISTAS_INFORMA", "archivo": self._pdf(),
            "fecha_expedicion": date.today().isoformat(),
        })
        self.assertFalse(DocumentoEmpresa.objects.filter(empresa=self.empresa, tipo_documento="LISTAS_INFORMA").exists())

    def test_solo_quien_tiene_el_permiso_carga_listas_informa(self):
        url = reverse("empresas:cargar_documento_especial", args=[self.empresa.pk])
        datos = {"tipo_documento": "LISTAS_INFORMA", "fecha_expedicion": date.today().isoformat()}

        self.client.force_login(self.admin)
        self.assertEqual(self.client.post(url, {**datos, "archivo": self._pdf()}).status_code, 403)

        self.client.force_login(self.lider)
        self.client.post(url, {**datos, "archivo": self._pdf()})
        documento = DocumentoEmpresa.objects.get(empresa=self.empresa, tipo_documento="LISTAS_INFORMA")
        self.assertEqual(documento.estado, "CARGADO")

    def test_formato_distinto_de_pdf_se_rechaza(self):
        self.client.force_login(self.usuario_empresa)
        self.client.post(reverse("empresas:documentos"), {
            "tipo_documento": "RUT", "archivo": SimpleUploadedFile("rut.docx", b"x"),
            "fecha_expedicion": date.today().isoformat(),
        })
        self.assertFalse(DocumentoEmpresa.objects.filter(empresa=self.empresa, tipo_documento="RUT").exists())

    def test_vigencia_fija_la_fecha_de_vencimiento(self):
        self.client.force_login(self.usuario_empresa)
        expedicion = date.today() - timedelta(days=10)
        self.client.post(reverse("empresas:documentos"), {
            "tipo_documento": "CAMARA_COMERCIO", "archivo": self._pdf(),
            "fecha_expedicion": expedicion.isoformat(),
        })
        camara = DocumentoEmpresa.objects.get(empresa=self.empresa, tipo_documento="CAMARA_COMERCIO")
        self.assertEqual(camara.fecha_vencimiento, expedicion + timedelta(days=30))

    def test_bloqueo_del_admin_deshabilita_y_queda_registrado(self):
        from apps.seguimiento.services import historial_de

        from .testing import habilitar_empresa

        habilitar_empresa(self.empresa)
        self.assertTrue(puede_la_empresa_operar(self.empresa))
        self.client.force_login(self.admin)
        url = reverse("empresas:cambiar_bloqueo", args=[self.empresa.pk])

        self.client.post(url, {"accion": "bloquear", "motivo": ""})
        self.empresa.refresh_from_db()
        self.assertFalse(self.empresa.participacion_bloqueada, "bloquear exige motivo")

        self.client.post(url, {"accion": "bloquear", "motivo": "Incumplimiento del convenio"})
        self.empresa.refresh_from_db()
        self.assertTrue(self.empresa.participacion_bloqueada)
        self.assertFalse(puede_la_empresa_operar(self.empresa))
        self.assertTrue(historial_de(self.empresa, campo="participacion_bloqueada").exists())
        self.assertTrue(Notificacion.objects.filter(usuario=self.usuario_empresa, evento="EMPRESA_PARTICIPACION").exists())

        self.client.post(url, {"accion": "habilitar"})
        self.empresa.refresh_from_db()
        self.assertTrue(puede_la_empresa_operar(self.empresa))

    def test_admin_no_aprueba_reto_de_empresa_sin_documentacion(self):
        from apps.retos.models import Reto

        reto = Reto.objects.create(empresa=self.usuario_empresa, titulo="Reto sin papeles", estado="en_revision")
        self.client.force_login(self.admin)
        self.client.post(reverse("retos:admin_revisar", args=[reto.pk]), {"accion": "aprobar", "comentario": ""})
        reto.refresh_from_db()
        self.assertEqual(reto.estado, "en_revision")

    def test_admin_configura_el_catalogo(self):
        from .models import TipoDocumentoLegal

        self.client.force_login(self.admin)
        self.assertEqual(self.client.get(reverse("empresas:tipos_documento")).status_code, 200)
        self.client.post(reverse("empresas:crear_tipo_documento"), {
            "codigo": "poliza", "nombre": "Póliza de responsabilidad", "descripcion": "",
            "obligatorio": "on", "aplica_sin_convenio": "on", "activo": "on", "orden": 20,
        })
        nuevo = TipoDocumentoLegal.objects.get(codigo="POLIZA")
        self.assertTrue(nuevo.obligatorio)

        ficha = TipoDocumentoLegal.objects.get(codigo="FICHA_RETO")
        self.client.post(reverse("empresas:editar_tipo_documento", args=[ficha.pk]), {
            "nombre": ficha.nombre, "descripcion": "", "aplica_sin_convenio": "on", "activo": "on", "orden": ficha.orden,
        })
        ficha.refresh_from_db()
        self.assertFalse(ficha.obligatorio)

    def test_empresa_no_accede_al_catalogo(self):
        self.client.force_login(self.usuario_empresa)
        self.assertNotEqual(self.client.get(reverse("empresas:tipos_documento")).status_code, 200)
