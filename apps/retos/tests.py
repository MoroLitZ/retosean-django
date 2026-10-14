from datetime import timedelta

from django.contrib.auth import get_user_model
from django.core.files.uploadedfile import SimpleUploadedFile
from django.core.exceptions import ValidationError
from django.test import SimpleTestCase, TestCase
from django.urls import reverse
from django.utils import timezone

from apps.empresas.models import Empresa
from apps.notificaciones.models import Notificacion
from .forms import IntegracionAcademicaForm, MAX_UPLOAD_SIZE, RetoForm, SeguimientoRetoForm
from apps.seguimiento.models import IntegracionAcademica

from .models import Reto

Usuario = get_user_model()


class MultipleFileUploadTests(SimpleTestCase):
    def test_reto_form_accepts_multiple_files(self):
        files = [
            SimpleUploadedFile("soporte-1.pdf", b"uno"),
            SimpleUploadedFile("soporte-2.pdf", b"dos"),
        ]

        field = RetoForm.base_fields["archivos"]
        cleaned_files = field.clean(files)

        self.assertEqual([file.name for file in cleaned_files], ["soporte-1.pdf", "soporte-2.pdf"])
        self.assertTrue(field.widget.allow_multiple_selected)

    def test_reto_form_rejects_a_file_larger_than_50_mb(self):
        oversized_file = SimpleUploadedFile("demasiado-grande.zip", b"")
        oversized_file.size = MAX_UPLOAD_SIZE + 1

        field = RetoForm.base_fields["archivos"]

        with self.assertRaisesMessage(ValidationError, "supera el limite de 50 MB"):
            field.clean([oversized_file])

    def test_progress_form_has_optional_multiple_file_upload(self):
        field = SeguimientoRetoForm.base_fields["archivos"]

        self.assertFalse(field.required)
        self.assertTrue(field.widget.allow_multiple_selected)
        self.assertEqual(field.clean([]), [])


class EmpresaSeguimientoAccessTests(TestCase):
    @classmethod
    def setUpTestData(cls):
        user_model = get_user_model()
        cls.owner = user_model.objects.create_user(
            username="empresa_propietaria",
            password="test-password",
            rol="EMPRESA",
        )
        cls.other_empresa = user_model.objects.create_user(
            username="otra_empresa",
            password="test-password",
            rol="EMPRESA",
        )
        cls.reto = Reto.objects.create(
            empresa=cls.owner,
            titulo="Reto con seguimiento",
        )

    def test_owner_sees_seguimiento_button_and_can_open_page(self):
        self.client.force_login(self.owner)

        detail_response = self.client.get(reverse("retos:detalle", args=[self.reto.pk]))
        seguimiento_url = reverse("retos:seguimientos", args=[self.reto.pk])

        self.assertContains(detail_response, seguimiento_url)
        self.assertEqual(self.client.get(seguimiento_url).status_code, 200)
        self.assertEqual(
            self.client.get(
                reverse("retos:agregar_seguimiento", args=[self.reto.pk])
            ).status_code,
            200,
        )

    def test_other_empresa_cannot_open_seguimiento(self):
        self.client.force_login(self.other_empresa)

        response = self.client.get(
            reverse("retos:seguimientos", args=[self.reto.pk])
        )

        self.assertEqual(response.status_code, 403)


class AccesoDelProfesorARetosTests(TestCase):
    """Regresion: `reto.integraciones` no existia y reventaba con AttributeError."""

    def setUp(self):
        self.empresa = Usuario.objects.create_user(
            username="empresa-acceso-prof", password="x", rol="EMPRESA"
        )
        self.profesor = Usuario.objects.create_user(
            username="profe-acceso", password="x", rol="PROFESOR"
        )
        self.profesor_vinculado = Usuario.objects.create_user(
            username="profe-vinculado", password="x", rol="PROFESOR"
        )

    def _reto(self, estado):
        return Reto.objects.create(
            empresa=self.empresa, titulo=f"Reto {estado}", estado=estado
        )

    def test_profesor_abre_reto_no_aprobado_sin_error_500(self):
        for estado in ["borrador", "en_revision", "rechazado", "cancelado"]:
            with self.subTest(estado=estado):
                reto = self._reto(estado)
                self.client.force_login(self.profesor)
                response = self.client.get(reverse("retos:detalle", args=[reto.pk]))
                # Sin integracion no tiene acceso, pero debe ser un 403 limpio.
                self.assertEqual(response.status_code, 403)

    def test_profesor_vinculado_si_ve_el_reto_en_borrador(self):
        from apps.seguimiento.models import IntegracionAcademica

        reto = self._reto("borrador")
        IntegracionAcademica.objects.create(
            reto=reto, profesor=self.profesor_vinculado, estado="aprobada"
        )
        self.client.force_login(self.profesor_vinculado)
        response = self.client.get(reverse("retos:detalle", args=[reto.pk]))
        self.assertEqual(response.status_code, 200)

    def test_profesor_ve_cualquier_reto_aprobado(self):
        reto = self._reto("aprobado")
        self.client.force_login(self.profesor)
        response = self.client.get(reverse("retos:detalle", args=[reto.pk]))
        self.assertEqual(response.status_code, 200)

    def test_boton_de_seguimiento_solo_para_el_profesor_vinculado(self):
        """La precedencia de and/or hacia que cualquier profesor lo viera."""
        from apps.seguimiento.models import IntegracionAcademica

        reto = self._reto("aprobado")
        IntegracionAcademica.objects.create(
            reto=reto, profesor=self.profesor_vinculado, estado="aprobada"
        )

        self.client.force_login(self.profesor_vinculado)
        vinculado = self.client.get(reverse("retos:detalle", args=[reto.pk]))
        self.assertTrue(vinculado.context["puede_ver_seguimiento"])

        self.client.force_login(self.profesor)
        ajeno = self.client.get(reverse("retos:detalle", args=[reto.pk]))
        self.assertFalse(ajeno.context["puede_ver_seguimiento"])


class FlujoIntegracionAcademicaTests(TestCase):
    """Regresion C.1: crear -> completar -> enviar -> aprobar sin bucle infinito.

    Antes `alcance` y `cronograma_sesiones` no estaban en el formulario, asi que
    `campos_faltantes_para_revision` siempre los reportaba y `enviar_integracion_revision`
    redirigia eternamente a editar sin poder avanzar.
    """

    def setUp(self):
        from apps.academico.models import Ecosistema, Facultad, Programa
        from apps.unidades_estudio.models import UnidadEstudio

        self.empresa = Usuario.objects.create_user(
            username="empresa-flujo-int", password="x", rol="EMPRESA"
        )
        self.profesor = Usuario.objects.create_user(
            username="profe-flujo-int", password="x", rol="PROFESOR"
        )
        self.admin = Usuario.objects.create_user(
            username="admin-flujo-int", password="x", rol="ADMIN"
        )
        self.reto = Reto.objects.create(
            empresa=self.empresa, titulo="Reto flujo integracion", estado="aprobado"
        )
        self.facultad = Facultad.objects.create(nombre="Facultad de Ingenieria")
        self.programa = Programa.objects.create(
            nombre="Ingenieria de Sistemas", facultad=self.facultad
        )
        # El formulario solo acepta ecosistemas del catalogo administrado.
        Ecosistema.objects.create(nombre="Ecosistema de innovacion")
        self.unidad = UnidadEstudio.objects.create(
            codigo="ISIS-101",
            nombre="Arquitectura de Software",
            programa=self.programa,
            periodo="2026-1",
            ciclo="Quinto",
        )

    def _post_integracion(self):
        return self.client.post(
            reverse("retos:crear_integracion"),
            {
                "reto": self.reto.pk,
                "facultad": "Facultad de Ingenieria",
                "nivel_formacion": "Pregrado",
                "unidad_estudio": self.unidad.pk,
                "ecosistema": "Ecosistema de innovacion",
                "descripcion": "Integracion de prueba",
                "alcance": "Dos unidades de estudio y una hackaton interna.",
                "entregable_esperado": "Prototipo funcional",
                "cronograma_sesiones": "Semana 1: kickoff; Semana 4: cierre.",
                "equipo_profesores": "Profesor A, Profesor B",
                "equipo_estudiantes": "10 estudiantes de quinto semestre",
                "expertos_invitados": "Un mentor de la empresa",
                "requerimientos_empresa": "Datos de mercado",
                "requerimientos_internos": "Sala de computo",
                "espacio_fisico": "Auditorio principal",
                "accion": "guardar",
            },
        )

    def test_crear_completar_enviar_aprobar(self):
        from apps.seguimiento.models import IntegracionAcademica

        # 1. Crear: el formulario captura alcance y cronograma_sesiones.
        self.client.force_login(self.profesor)
        self._post_integracion()
        integracion = IntegracionAcademica.objects.get(profesor=self.profesor)
        self.assertEqual(integracion.estado, "borrador")
        self.assertEqual(integracion.programa_academico, self.unidad.nombre)
        self.assertEqual(integracion.alcance, "Dos unidades de estudio y una hackaton interna.")
        self.assertEqual(integracion.cronograma_sesiones, "Semana 1: kickoff; Semana 4: cierre.")
        self.assertEqual(integracion.ecosistema, "Ecosistema de innovacion")

        # 2. Enviar a revision: ya no quedan campos obligatorios pendientes.
        response = self.client.post(
            reverse("retos:enviar_integracion_revision", args=[integracion.pk])
        )
        integracion.refresh_from_db()
        self.assertEqual(integracion.estado, "en_revision")
        self.assertIsNotNone(integracion.fecha_envio_revision)

        # 3. Aprobar como administrador.
        self.client.force_login(self.admin)
        self.client.post(
            reverse("retos:admin_revisar_integracion", args=[integracion.pk]),
            {"accion": "aprobar", "comentario": "Todo en orden."},
        )
        integracion.refresh_from_db()
        self.assertEqual(integracion.estado, "aprobada")
        self.assertEqual(integracion.comentarios_revision, "Todo en orden.")

    def test_envio_sigue_bloqueado_si_falta_alcance(self):
        from apps.seguimiento.models import IntegracionAcademica

        self.client.force_login(self.profesor)
        self._post_integracion()
        integracion = IntegracionAcademica.objects.get(profesor=self.profesor)
        integracion.alcance = ""
        integracion.save(update_fields=["alcance"])

        self.client.post(
            reverse("retos:enviar_integracion_revision", args=[integracion.pk])
        )
        integracion.refresh_from_db()
        self.assertEqual(integracion.estado, "borrador")


class NotificacionEnvioRevisionRetoTests(TestCase):
    def setUp(self):
        self.empresa_usuario = Usuario.objects.create_user(
            username="empresa-envio-reto", password="x", rol="EMPRESA"
        )
        self.admin = Usuario.objects.create_superuser(
            username="admin-envio-reto", password="x", email="admin-envio@ean.edu.co"
        )
        from apps.empresas.testing import habilitar_empresa

        habilitar_empresa(Empresa.objects.create(
            usuario=self.empresa_usuario,
            nit="901111222",
            razon_social="Empresa Envio Reto",
            renuncio_a_convenio=True,
        ))
        hoy = timezone.localdate()
        self.reto = Reto.objects.create(
            empresa=self.empresa_usuario,
            titulo="Reto para revision",
            descripcion="Descripcion completa del reto",
            area="Tecnología",
            fecha_inicio_tentativa=hoy + timedelta(days=10),
            fecha_fin_tentativa=hoy + timedelta(days=40),
            fecha_limite_postulacion=hoy + timedelta(days=7),
            estado="borrador",
        )

    def test_enviar_reto_a_revision_notifica_al_admin(self):
        self.client.force_login(self.empresa_usuario)

        response = self.client.post(reverse("retos:enviar_revision", args=[self.reto.pk]))

        self.assertRedirects(response, reverse("retos:detalle", args=[self.reto.pk]))
        self.reto.refresh_from_db()
        self.assertEqual(self.reto.estado, "en_revision")

        notificacion = Notificacion.objects.get(
            usuario=self.admin,
            evento="RETO_ENVIADO_REVISION",
        )
        self.assertIn(self.reto.titulo, notificacion.mensaje)


class IntegracionAcademicaSelectsTests(TestCase):
    def setUp(self):
        from apps.academico.models import Ecosistema, Facultad, Programa
        from apps.unidades_estudio.models import UnidadEstudio

        self.empresa = Usuario.objects.create_user(
            username="empresa-form-int", password="x", rol="EMPRESA"
        )
        self.profesor = Usuario.objects.create_user(
            username="profe-form-int", password="x", rol="PROFESOR"
        )
        self.reto = Reto.objects.create(
            empresa=self.empresa,
            titulo="Reto para formulario integracion",
            estado="aprobado",
        )
        self.facultad_ing = Facultad.objects.create(nombre="Facultad de Ingenieria")
        self.facultad_eco = Facultad.objects.create(nombre="Facultad de Economia")
        Ecosistema.objects.create(nombre="Ecosistema de innovacion")
        self.programa_ing = Programa.objects.create(
            nombre="Ingenieria de Sistemas",
            facultad=self.facultad_ing,
        )
        self.programa_eco = Programa.objects.create(
            nombre="Economia",
            facultad=self.facultad_eco,
        )
        self.unidad_ing = UnidadEstudio.objects.create(
            codigo="ING-201",
            nombre="Arquitectura de Software",
            programa=self.programa_ing,
            periodo="2026-2",
            ciclo="Quinto",
            activo=True,
        )
        self.unidad_eco_inactiva = UnidadEstudio.objects.create(
            codigo="ECO-101",
            nombre="Microeconomia",
            programa=self.programa_eco,
            periodo="2026-2",
            ciclo="Primero",
            activo=False,
        )

    def test_rechaza_valores_fuera_del_catalogo_en_selects(self):
        form = IntegracionAcademicaForm(
            data={
                "reto": self.reto.pk,
                "facultad": "Texto libre no permitido",
                "nivel_formacion": "Nivel inventado",
                "ecosistema": "Otro valor libre",
            }
        )
        self.assertFalse(form.is_valid())
        self.assertIn("facultad", form.errors)
        self.assertIn("nivel_formacion", form.errors)
        self.assertIn("ecosistema", form.errors)

    def test_permite_editar_valores_legacy_existentes(self):
        from apps.seguimiento.models import IntegracionAcademica

        integracion = IntegracionAcademica.objects.create(
            reto=self.reto,
            profesor=self.profesor,
            facultad="Facultad legacy",
            nivel_formacion="Nivel legacy",
            ecosistema="Ecosistema legacy",
            descripcion="Desc",
            alcance="Alcance",
            entregable_esperado="Entregable",
            cronograma_sesiones="Cronograma",
        )

        form = IntegracionAcademicaForm(
            instance=integracion,
            data={
                "reto": self.reto.pk,
                "facultad": "Facultad legacy",
                "nivel_formacion": "Nivel legacy",
                "ecosistema": "Ecosistema legacy",
            },
        )
        self.assertTrue(form.is_valid())

    def test_unidad_estudio_lista_unidades_activas_disponibles(self):
        form = IntegracionAcademicaForm()
        unidades = list(form.fields["unidad_estudio"].queryset)
        self.assertIn(self.unidad_ing, unidades)
        self.assertNotIn(self.unidad_eco_inactiva, unidades)

    def test_unidad_estudio_se_filtra_por_facultad(self):
        form = IntegracionAcademicaForm(data={"facultad": "Facultad de Ingenieria"})
        unidades = list(form.fields["unidad_estudio"].queryset)
        self.assertEqual(unidades, [self.unidad_ing])


class NotificacionPostulacionDocenteTests(TestCase):
    def setUp(self):
        from apps.seguimiento.models import IntegracionAcademica

        self.empresa = Usuario.objects.create_user(
            username="empresa-post-doc", password="x", rol="EMPRESA", email="empresa@ean.edu.co"
        )
        self.profesor = Usuario.objects.create_user(
            username="profe-post-doc", password="x", rol="PROFESOR", email="profe@ean.edu.co"
        )
        self.admin = Usuario.objects.create_user(
            username="admin-post-doc", password="x", rol="ADMIN", email="admin@ean.edu.co"
        )
        self.reto = Reto.objects.create(
            empresa=self.empresa,
            titulo="Reto con postulacion docente",
            estado="aprobado",
        )
        self.integracion = IntegracionAcademica.objects.create(
            reto=self.reto,
            profesor=self.profesor,
            estado="borrador",
            facultad="Facultad de Ingenieria",
            programa_academico="Ingenieria de Sistemas",
            nivel_formacion="Pregrado",
            descripcion="Descripcion",
            alcance="Alcance",
            entregable_esperado="Entregable",
            cronograma_sesiones="Cronograma",
        )

    def test_enviar_postulacion_docente_notifica_a_empresa_y_admin(self):
        self.client.force_login(self.profesor)

        response = self.client.post(
            reverse("retos:enviar_integracion_revision", args=[self.integracion.pk])
        )

        self.assertRedirects(
            response,
            reverse("retos:detalle_integracion", args=[self.integracion.pk]),
        )
        self.integracion.refresh_from_db()
        self.assertEqual(self.integracion.estado, "en_revision")

        self.assertTrue(
            Notificacion.objects.filter(
                usuario=self.empresa,
                evento="INTEGRACION_ENVIADA_REVISION",
            ).exists()
        )
        self.assertTrue(
            Notificacion.objects.filter(
                usuario=self.admin,
                evento="INTEGRACION_ENVIADA_REVISION",
            ).exists()
        )


class CicloDeVidaRetoTests(TestCase):
    """Ciclo unificado del brief: transiciones validas, Abierto, consecutivos
    por tipo y anio, y borrado logico solo en borrador."""

    def setUp(self):
        from apps.empresas.models import Empresa

        self.admin = Usuario.objects.create_user(username="admin-ciclo", password="x", rol="ADMIN")
        self.empresa = Usuario.objects.create_user(username="empresa-ciclo", password="x", rol="EMPRESA")
        from apps.empresas.testing import habilitar_empresa

        habilitar_empresa(Empresa.objects.create(usuario=self.empresa, nit="900100200", razon_social="Ciclo SAS"))
        self.profesor = Usuario.objects.create_user(username="profe-ciclo", password="x", rol="PROFESOR")
        self.estudiante = Usuario.objects.create_user(username="est-ciclo", password="x", rol="ESTUDIANTE")

    def _reto(self, estado="borrador", tipo="reto", titulo="Reto ciclo"):
        return Reto.objects.create(empresa=self.empresa, titulo=titulo, tipo=tipo, estado=estado)

    def _aprobar(self, reto):
        from .services import cambiar_estado_reto

        cambiar_estado_reto(reto, "aprobado", self.admin, "Ok")
        reto.refresh_from_db()
        return reto

    # --- transiciones -------------------------------------------------------
    def test_transicion_invalida_se_rechaza_sin_cambiar_estado(self):
        from apps.seguimiento.services import TransicionInvalida

        from .services import cambiar_estado_reto

        reto = self._reto("borrador")
        with self.assertRaises(TransicionInvalida):
            cambiar_estado_reto(reto, "aprobado", self.admin)
        reto.refresh_from_db()
        self.assertEqual(reto.estado, "borrador")
        self.assertFalse(reto.historial_estados.exists())

    def test_estados_finales_no_admiten_cambios(self):
        from .services import MAQUINA_RETO

        for final in ("finalizado", "cancelado"):
            self.assertTrue(MAQUINA_RETO.es_final(final))

    def test_cualquier_estado_no_final_puede_cancelarse(self):
        from .services import MAQUINA_RETO

        for estado in ("borrador", "en_revision", "aprobado", "abierto", "rechazado", "en_curso", "pausado"):
            self.assertTrue(MAQUINA_RETO.puede(estado, "cancelado"), estado)

    def test_cambio_manual_solo_ofrece_transiciones_validas(self):
        reto = self._reto("en_curso")
        self.client.force_login(self.admin)
        response = self.client.get(reverse("retos:admin_cambiar_estado", args=[reto.pk]))
        opciones = [valor for valor, _ in response.context["opciones"]]
        self.assertEqual(sorted(opciones), ["cancelado", "pausado"])

        # "finalizado" se hace por el cierre formal, no desde el cambio manual.
        self.client.post(reverse("retos:admin_cambiar_estado", args=[reto.pk]), {"estado": "finalizado"})
        reto.refresh_from_db()
        self.assertEqual(reto.estado, "en_curso")

    def test_reto_rechazado_no_se_reenvia_sin_editar(self):
        reto = self._reto("rechazado")
        self.client.force_login(self.empresa)
        self.client.post(reverse("retos:enviar_revision", args=[reto.pk]))
        reto.refresh_from_db()
        self.assertEqual(reto.estado, "rechazado")

    # --- Abierto -------------------------------------------------------------
    def test_publicar_integracion_abre_el_reto_a_estudiantes(self):
        reto = self._reto("aprobado")
        integracion = IntegracionAcademica.objects.create(reto=reto, profesor=self.profesor, estado="aprobada")
        self.client.force_login(self.profesor)
        self.client.post(reverse("retos:publicar_integracion", args=[integracion.pk]))

        reto.refresh_from_db()
        integracion.refresh_from_db()
        self.assertEqual(integracion.estado, "publicada")
        self.assertEqual(reto.estado, "abierto")
        self.assertTrue(reto.historial_estados.filter(estado_anterior="aprobado", estado_nuevo="abierto").exists())

    def test_explorador_solo_lista_retos_abiertos(self):
        aprobado = self._reto("aprobado", titulo="Solo aprobado")
        abierto = self._reto("abierto", titulo="Ya abierto")
        self.client.force_login(self.estudiante)
        response = self.client.get(reverse("academico:explorar_retos"))
        listados = {reto.pk for reto in response.context["retos"]}
        self.assertIn(abierto.pk, listados)
        self.assertNotIn(aprobado.pk, listados)

    def test_no_se_puede_postular_a_un_reto_que_no_esta_abierto(self):
        reto = self._reto("aprobado")
        self.client.force_login(self.estudiante)
        response = self.client.get(reverse("participaciones:postular_a_reto", args=[reto.pk]))
        self.assertEqual(response.status_code, 404)

    # --- consecutivos --------------------------------------------------------
    def test_consecutivos_separados_por_tipo_y_anio(self):
        anio = timezone.localdate().year
        reto1 = self._aprobar(self._reto("en_revision", titulo="Reto 1"))
        hack1 = self._aprobar(self._reto("en_revision", tipo="hackathon", titulo="Hack 1"))
        reto2 = self._aprobar(self._reto("en_revision", titulo="Reto 2"))

        self.assertEqual(reto1.codigo, f"RETO-{anio}-001")
        self.assertEqual(hack1.codigo, f"HACK-{anio}-001")
        self.assertEqual(reto2.codigo, f"RETO-{anio}-002")

    def test_consecutivo_no_cambia_si_el_reto_vuelve_a_aprobarse(self):
        reto = self._aprobar(self._reto("en_revision"))
        codigo = reto.codigo
        reto.asignar_consecutivo_si_falta()
        self.assertEqual(reto.codigo, codigo)

    # --- borrado logico ------------------------------------------------------
    def test_eliminar_borrador_es_logico_y_queda_en_el_historial(self):
        reto = self._reto("borrador")
        self.client.force_login(self.empresa)
        self.client.post(reverse("retos:eliminar", args=[reto.pk]))

        self.assertFalse(Reto.objects.filter(pk=reto.pk).exists())
        conservado = Reto.todos.get(pk=reto.pk)
        self.assertIsNotNone(conservado.eliminado_en)
        self.assertTrue(conservado.historial_estados.filter(comentario__icontains="eliminado").exists())

    def test_solo_se_eliminan_borradores(self):
        for estado in ("en_revision", "rechazado", "cancelado"):
            reto = self._reto(estado, titulo=f"No borrable {estado}")
            self.client.force_login(self.empresa)
            self.client.post(reverse("retos:eliminar", args=[reto.pk]))
            self.assertTrue(Reto.objects.filter(pk=reto.pk).exists(), estado)

    def test_reto_eliminado_responde_404(self):
        reto = self._reto("borrador")
        reto.eliminado_en = timezone.now()
        reto.save()
        self.client.force_login(self.empresa)
        response = self.client.get(reverse("retos:detalle", args=[reto.pk]))
        self.assertEqual(response.status_code, 404)

    def test_enviar_a_revision_no_acepta_get(self):
        """Un enlace o una precarga del navegador no deben cambiar el estado."""
        reto = self._reto("borrador")
        self.client.force_login(self.empresa)
        self.assertEqual(self.client.get(reverse("retos:enviar_revision", args=[reto.pk])).status_code, 405)
        reto.refresh_from_db()
        self.assertEqual(reto.estado, "borrador")



class FechasEnFormulariosTests(TestCase):
    """Los <input type="date"> solo aceptan ISO: con la localizacion es-co Django
    los rellenaba como dd/mm/aaaa, el navegador los mostraba vacios y al guardar
    la edicion se perdian las fechas del reto."""

    def test_editar_reto_muestra_las_fechas_en_iso(self):
        from datetime import date

        empresa = Usuario.objects.create_user(username="empresa-fechas", password="x", rol="EMPRESA")
        reto = Reto.objects.create(empresa=empresa, titulo="Con fechas", fecha_inicio_tentativa=date(2026, 10, 18))
        self.assertIn('value="2026-10-18"', str(RetoForm(instance=reto)["fecha_inicio_tentativa"]))

    def test_agenda_de_cierre_muestra_fecha_y_hora_en_iso(self):
        from datetime import datetime

        from apps.cierre.forms import AgendaCierreForm
        from apps.cierre.models import AgendaCierre

        empresa = Usuario.objects.create_user(username="empresa-agenda", password="x", rol="EMPRESA")
        reto = Reto.objects.create(empresa=empresa, titulo="Con agenda", estado="en_curso")
        agenda = AgendaCierre.objects.create(
            reto=reto, fecha_hora=timezone.make_aware(datetime(2026, 10, 9, 8, 1)),
        )
        self.assertIn('value="2026-10-09T08:01"', str(AgendaCierreForm(instance=agenda)["fecha_hora"]))
