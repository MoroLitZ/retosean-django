from datetime import date, timedelta

from django.core.files.uploadedfile import SimpleUploadedFile
from django.test import SimpleTestCase, TestCase
from django.urls import reverse

from apps.empresas.models import DocumentoEmpresa, Empresa
from apps.evaluacion.models import Entregable
from apps.participaciones.models import Postulacion
from apps.retos.models import Reto
from apps.usuarios.models import Usuario

from .models import HistorialEstado, IntegracionAcademica
from .services import MaquinaEstados, TransicionInvalida, cambiar_estado, historial_de


class MaquinaEstadosTests(SimpleTestCase):
    def setUp(self):
        self.maquina = MaquinaEstados(
            "la prueba",
            {"a": {"b"}, "b": {"c"}, "c": set()},
            {"a": "Uno", "b": "Dos", "c": "Tres"},
        )

    def test_transicion_valida(self):
        self.assertTrue(self.maquina.puede("a", "b"))
        self.maquina.validar("a", "b")

    def test_transicion_invalida_explica_el_motivo_en_espanol(self):
        with self.assertRaises(TransicionInvalida) as ctx:
            self.maquina.validar("a", "c")
        self.assertIn('de "Uno" a "Tres"', str(ctx.exception))

    def test_estado_sin_destinos_es_final(self):
        self.assertTrue(self.maquina.es_final("c"))
        self.assertFalse(self.maquina.puede("c", "a"))


class HistorialEstadoTests(TestCase):
    """Cada cambio de estado de documentos, postulaciones, entregables,
    integraciones y cuentas queda en la bitacora comun."""

    def setUp(self):
        self.admin = Usuario.objects.create_user(username="admin-hist", password="x", rol="ADMIN")
        self.empresa_user = Usuario.objects.create_user(username="empresa-hist", password="x", rol="EMPRESA")
        self.profesor = Usuario.objects.create_user(username="profe-hist", password="x", rol="PROFESOR")
        self.estudiante = Usuario.objects.create_user(username="est-hist", password="x", rol="ESTUDIANTE")
        self.empresa = Empresa.objects.create(usuario=self.empresa_user, nit="900777888", razon_social="Hist SAS")

    def test_cambiar_estado_invalido_no_modifica_ni_registra(self):
        from apps.participaciones.services import MAQUINA_POSTULACION

        reto = Reto.objects.create(empresa=self.empresa_user, titulo="R", estado="abierto")
        postulacion = Postulacion.objects.create(reto=reto, estudiante=self.estudiante, estado="RECHAZADA")
        with self.assertRaises(TransicionInvalida):
            cambiar_estado(postulacion, MAQUINA_POSTULACION, "ACEPTADA", self.admin)
        postulacion.refresh_from_db()
        self.assertEqual(postulacion.estado, "RECHAZADA")
        self.assertFalse(historial_de(postulacion).exists())

    def test_ciclo_del_documento_queda_registrado(self):
        self.client.force_login(self.empresa_user)
        self.client.post(reverse("empresas:documentos"), {
            "tipo_documento": "RUT",
            "archivo": SimpleUploadedFile("rut.pdf", b"contenido"),
            "fecha_expedicion": date.today().isoformat(),
        })
        documento = DocumentoEmpresa.objects.get(empresa=self.empresa, tipo_documento="RUT")

        self.client.force_login(self.admin)
        self.client.post(reverse("empresas:procesar_aprobacion", args=[documento.pk]), {
            "nuevo_estado": "RECHAZADO", "motivo": "Ilegible",
        })
        self.client.force_login(self.empresa_user)
        self.client.post(reverse("empresas:documentos"), {
            "tipo_documento": "RUT",
            "archivo": SimpleUploadedFile("rut2.pdf", b"contenido nuevo"),
            "fecha_expedicion": date.today().isoformat(),
        })

        pasos = list(historial_de(documento).order_by("fecha", "id").values_list(
            "estado_anterior", "estado_nuevo", "realizado_por__username"
        ))
        self.assertEqual(pasos, [
            ("", "CARGADO", "empresa-hist"),
            ("CARGADO", "RECHAZADO", "admin-hist"),
            ("RECHAZADO", "CARGADO", "empresa-hist"),
        ])

    def test_no_se_puede_verificar_un_documento_ya_rechazado(self):
        documento = DocumentoEmpresa.objects.create(
            empresa=self.empresa, tipo_documento="RUT", estado="RECHAZADO",
            archivo=SimpleUploadedFile("rut.pdf", b"x"),
        )
        self.client.force_login(self.admin)
        self.client.post(reverse("empresas:procesar_aprobacion", args=[documento.pk]), {
            "nuevo_estado": "VERIFICADO",
        })
        documento.refresh_from_db()
        self.assertEqual(documento.estado, "RECHAZADO")

    def test_ciclo_de_la_postulacion_queda_registrado(self):
        from apps.academico.models import Facultad, Programa

        programa = Programa.objects.create(nombre="Sistemas", facultad=Facultad.objects.create(nombre="Ingenieria"))
        reto = Reto.objects.create(empresa=self.empresa_user, titulo="Reto abierto", estado="abierto")
        self.client.force_login(self.estudiante)
        self.client.post(reverse("participaciones:postular_a_reto", args=[reto.pk]), {
            "motivacion": "Me interesa", "habilidades": "Python", "semestre": 5, "programa": programa.nombre,
        })
        postulacion = Postulacion.objects.get(reto=reto, estudiante=self.estudiante)
        self.client.post(reverse("participaciones:retirar_postulacion", args=[reto.pk]))

        pasos = list(historial_de(postulacion).order_by("fecha", "id").values_list("estado_anterior", "estado_nuevo"))
        self.assertEqual(pasos, [("", "PENDIENTE"), ("PENDIENTE", "RETIRADA")])

    def test_calificar_entregable_queda_registrado(self):
        reto = Reto.objects.create(empresa=self.empresa_user, titulo="Reto en curso", estado="en_curso")
        IntegracionAcademica.objects.create(reto=reto, profesor=self.profesor, estado="publicada")
        entregable = Entregable.objects.create(
            reto=reto, estudiante=self.estudiante, titulo="Avance",
            archivo=SimpleUploadedFile("a.pdf", b"x"),
        )
        self.client.force_login(self.profesor)
        self.client.get(reverse("evaluacion:panel_profesor"))
        self.client.post(reverse("evaluacion:calificar", args=[entregable.pk]), {"nota": "4", "comentario": ""})

        pasos = list(historial_de(entregable).order_by("fecha", "id").values_list("estado_anterior", "estado_nuevo"))
        self.assertEqual(pasos, [("ENVIADO", "EN_REVISION"), ("EN_REVISION", "CALIFICADO")])

    def test_inactivar_usuario_queda_registrado(self):
        self.client.force_login(self.admin)
        self.client.post(reverse("usuarios:alternar_estado", args=[self.estudiante.pk]))
        self.estudiante.refresh_from_db()
        self.assertFalse(self.estudiante.is_active)
        registro = historial_de(self.estudiante, campo="is_active").get()
        self.assertEqual((registro.estado_anterior, registro.estado_nuevo), ("ACTIVO", "INACTIVO"))
        self.assertEqual(registro.realizado_por, self.admin)

    def test_integracion_no_se_reenvia_si_ya_esta_aprobada(self):
        reto = Reto.objects.create(empresa=self.empresa_user, titulo="Reto aprobado", estado="aprobado")
        integracion = IntegracionAcademica.objects.create(
            reto=reto, profesor=self.profesor, estado="aprobada",
            facultad="F", programa_academico="P", nivel_formacion="Pregrado", descripcion="d",
            alcance="a", entregable_esperado="e", cronograma_sesiones="c",
        )
        self.client.force_login(self.profesor)
        self.client.get(reverse("retos:enviar_integracion_revision", args=[integracion.pk]))
        integracion.refresh_from_db()
        self.assertEqual(integracion.estado, "aprobada")
        self.assertFalse(HistorialEstado.objects.filter(object_id=integracion.pk, estado_nuevo="en_revision").exists())
