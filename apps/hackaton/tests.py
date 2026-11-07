from datetime import timedelta
from decimal import Decimal
from unittest import mock

from django.core.exceptions import ValidationError
from django.test import TestCase
from django.urls import reverse
from django.utils import timezone

from apps.notificaciones.models import Notificacion
from apps.participaciones.models import Equipo, MiembroEquipo, Postulacion
from apps.retos.models import Reto
from apps.seguimiento.models import IntegracionAcademica
from apps.seguimiento.services import cambiar_estado
from apps.usuarios.models import Usuario

from . import services
from .forms import InscripcionForm
from .models import EtapaHackaton, Hackathon, InscripcionHackaton, Jurado, VotacionHackaton


class BaseHackathonTests(TestCase):
    def setUp(self):
        self.admin = Usuario.objects.create_superuser(
            username="admin-hack", password="x", email="admin@ean.edu.co"
        )
        self.empresa = Usuario.objects.create_user(
            username="empresa-hack", password="x", rol="EMPRESA"
        )
        self.profesor = Usuario.objects.create_user(
            username="profe-hack", password="x", rol="PROFESOR"
        )
        self.reto = Reto.objects.create(
            empresa=self.empresa, titulo="Reto hackathon", estado="en_curso", tipo="hackathon"
        )
        IntegracionAcademica.objects.create(
            reto=self.reto, profesor=self.profesor, estado="aprobada"
        )
        self.hackathon = Hackathon.objects.create(reto=self.reto, estado="publicado")

    def _equipo(self, nombre, *estudiantes):
        equipo = Equipo.objects.create(reto=self.reto, nombre=nombre)
        for estudiante in estudiantes:
            MiembroEquipo.objects.create(equipo=equipo, estudiante=estudiante)
        return equipo

    def _estudiante(self, username):
        estudiante = Usuario.objects.create_user(username=username, password="x", rol="ESTUDIANTE")
        Postulacion.objects.create(reto=self.reto, estudiante=estudiante, estado="ACEPTADA")
        return estudiante

    def _jurado(self, username):
        usuario = Usuario.objects.create_user(username=username, password="x", rol="PROFESOR")
        return Jurado.objects.create(hackathon=self.hackathon, usuario=usuario, nombre=username)

    def _inscribir(self, equipo, estado="ACEPTADA"):
        return InscripcionHackaton.objects.create(
            hackathon=self.hackathon, equipo=equipo, estado=estado
        )

    def _abrir_presentacion(self):
        """Etapa de presentacion fechada y vigente; sin ella no se vota (HCK-05)."""
        ahora = timezone.now()
        return EtapaHackaton.objects.create(
            hackathon=self.hackathon, tipo="presentacion", titulo="Presentacion",
            inicia_en=ahora - timedelta(days=1), termina_en=ahora + timedelta(days=1),
        )


class EtapasTests(BaseHackathonTests):
    def test_crea_las_cuatro_etapas_del_backlog(self):
        services.crear_etapas_por_defecto(self.hackathon)
        tipos = list(self.hackathon.etapas.values_list("tipo", flat=True))
        self.assertEqual(
            sorted(tipos), ["desarrollo", "inscripcion", "premiacion", "presentacion"]
        )

    def test_no_duplica_etapas_si_ya_existen(self):
        services.crear_etapas_por_defecto(self.hackathon)
        services.crear_etapas_por_defecto(self.hackathon)
        self.assertEqual(self.hackathon.etapas.count(), 4)

    def test_etapa_fuera_de_fecha_no_esta_vigente(self):
        ayer = timezone.now() - timedelta(days=2)
        etapa = EtapaHackaton.objects.create(
            hackathon=self.hackathon, tipo="inscripcion", titulo="Inscripcion",
            inicia_en=ayer, termina_en=ayer + timedelta(days=1),
        )
        self.assertFalse(etapa.vigente)
        self.assertFalse(services.inscripciones_abiertas(self.hackathon))

    def test_etapa_en_curso_abre_las_inscripciones(self):
        ahora = timezone.now()
        EtapaHackaton.objects.create(
            hackathon=self.hackathon, tipo="inscripcion", titulo="Inscripcion",
            inicia_en=ahora - timedelta(days=1), termina_en=ahora + timedelta(days=5),
        )
        self.assertTrue(services.inscripciones_abiertas(self.hackathon))


class InscripcionTests(BaseHackathonTests):
    def test_estudiante_inscribe_su_equipo(self):
        estudiante = self._estudiante("est-insc")
        self._equipo("Equipo Alfa", estudiante)
        self.client.force_login(estudiante)
        equipo_id = Equipo.objects.get(nombre="Equipo Alfa").pk
        response = self.client.post(
            reverse("hackaton:inscribir_equipo", args=[self.hackathon.pk]),
            {"equipo": equipo_id},
        )
        self.assertEqual(response.status_code, 302)
        inscripcion = InscripcionHackaton.objects.get(equipo_id=equipo_id)
        self.assertEqual(inscripcion.estado, "PENDIENTE")

    def test_no_se_puede_inscribir_dos_veces_el_mismo_equipo(self):
        estudiante = self._estudiante("est-doble")
        equipo = self._equipo("Equipo Beta", estudiante)
        self._inscribir(equipo, estado="PENDIENTE")
        self.client.force_login(estudiante)
        response = self.client.post(
            reverse("hackaton:inscribir_equipo", args=[self.hackathon.pk]),
            {"equipo": equipo.pk},
        )
        # El equipo ya no aparece en el queryset del formulario.
        self.assertEqual(response.status_code, 200)
        self.assertEqual(InscripcionHackaton.objects.filter(equipo=equipo).count(), 1)

    def test_gestor_acepta_la_inscripcion(self):
        equipo = self._equipo("Equipo Gamma")
        inscripcion = self._inscribir(equipo, estado="PENDIENTE")
        self.client.force_login(self.admin)
        self.client.post(
            reverse("hackaton:gestionar_inscripcion", args=[inscripcion.pk]),
            {"accion": "aceptar"},
        )
        inscripcion.refresh_from_db()
        self.assertEqual(inscripcion.estado, "ACEPTADA")


class VotacionTests(BaseHackathonTests):
    def test_voto_fuera_de_rango_es_invalido(self):
        equipo = self._equipo("Equipo Delta")
        self._inscribir(equipo)
        jurado = self._jurado("jurado-uno")
        voto = VotacionHackaton(
            hackathon=self.hackathon, jurado=jurado, equipo=equipo, puntaje=Decimal("7.00")
        )
        with self.assertRaises(ValidationError):
            voto.full_clean()

    def test_no_se_puede_votar_un_equipo_sin_inscripcion_aceptada(self):
        equipo = self._equipo("Equipo Epsilon")
        self._inscribir(equipo, estado="PENDIENTE")
        jurado = self._jurado("jurado-dos")
        voto = VotacionHackaton(
            hackathon=self.hackathon, jurado=jurado, equipo=equipo, puntaje=Decimal("4.00")
        )
        with self.assertRaises(ValidationError):
            voto.full_clean()

    def test_jurado_registra_y_actualiza_su_voto(self):
        self._abrir_presentacion()
        equipo = self._equipo("Equipo Zeta")
        self._inscribir(equipo)
        jurado = self._jurado("jurado-tres")
        self.client.force_login(jurado.usuario)
        url = reverse("hackaton:votar", args=[self.hackathon.pk])

        self.client.post(url, {"equipo": equipo.pk, "puntaje": "4.00", "comentario": "Bien"})
        self.assertEqual(VotacionHackaton.objects.count(), 1)

        self.client.post(url, {"equipo": equipo.pk, "puntaje": "4.75", "comentario": "Mejor"})
        self.assertEqual(VotacionHackaton.objects.count(), 1)
        self.assertEqual(VotacionHackaton.objects.get().puntaje, Decimal("4.75"))

    def test_quien_no_es_jurado_no_puede_votar(self):
        self.client.force_login(self.profesor)
        response = self.client.get(reverse("hackaton:votar", args=[self.hackathon.pk]))
        self.assertRedirects(response, reverse("hackaton:detalle", args=[self.hackathon.pk]))

    # ---- HCK-05: la votacion falla cerrado sin etapa de presentacion fechada.
    def test_sin_etapa_de_presentacion_no_se_vota(self):
        equipo = self._equipo("Equipo Sin Etapa")
        self._inscribir(equipo)
        jurado = self._jurado("jurado-cerrado")
        self.assertFalse(services.votacion_abierta(self.hackathon))
        self.client.force_login(jurado.usuario)
        response = self.client.post(
            reverse("hackaton:votar", args=[self.hackathon.pk]),
            {"equipo": equipo.pk, "puntaje": "5", "comentario": ""},
        )
        self.assertRedirects(response, reverse("hackaton:detalle", args=[self.hackathon.pk]))
        self.assertEqual(VotacionHackaton.objects.count(), 0)

    def test_etapa_de_presentacion_sin_fechas_no_abre_la_votacion(self):
        EtapaHackaton.objects.create(
            hackathon=self.hackathon, tipo="presentacion", titulo="Presentacion"
        )
        self.assertFalse(services.votacion_abierta(self.hackathon))

    def test_etapa_de_presentacion_vigente_abre_la_votacion(self):
        self._abrir_presentacion()
        self.assertTrue(services.votacion_abierta(self.hackathon))


class RankingTests(BaseHackathonTests):
    def setUp(self):
        super().setUp()
        self.equipo_a = self._equipo("Equipo A")
        self.equipo_b = self._equipo("Equipo B")
        self._inscribir(self.equipo_a)
        self._inscribir(self.equipo_b)
        self.jurado_1 = self._jurado("j1")
        self.jurado_2 = self._jurado("j2")

    def _votar(self, jurado, equipo, puntaje):
        VotacionHackaton.objects.create(
            hackathon=self.hackathon, jurado=jurado, equipo=equipo, puntaje=Decimal(puntaje)
        )

    def test_ordena_por_promedio_no_por_suma(self):
        # A: un solo voto de 5.0 (promedio 5). B: dos votos de 4.0 (promedio 4, suma 8).
        self._votar(self.jurado_1, self.equipo_a, "5.00")
        self._votar(self.jurado_1, self.equipo_b, "4.00")
        self._votar(self.jurado_2, self.equipo_b, "4.00")

        tabla = services.ranking(self.hackathon)
        self.assertEqual(tabla[0]["equipo"], "Equipo A")
        self.assertEqual(tabla[0]["posicion"], 1)
        self.assertEqual(tabla[1]["equipo"], "Equipo B")
        # Aunque B suma mas puntos totales.
        self.assertGreater(tabla[1]["total"], tabla[0]["total"])

    def test_desempate_por_numero_de_votos(self):
        self._votar(self.jurado_1, self.equipo_a, "4.00")
        self._votar(self.jurado_1, self.equipo_b, "4.00")
        self._votar(self.jurado_2, self.equipo_b, "4.00")
        tabla = services.ranking(self.hackathon)
        self.assertEqual(tabla[0]["equipo"], "Equipo B")

    def test_equipos_sin_votar_del_jurado(self):
        self._votar(self.jurado_1, self.equipo_a, "4.00")
        pendientes = services.equipos_sin_votar(self.hackathon, self.jurado_1)
        self.assertEqual([i.equipo_id for i in pendientes], [self.equipo_b.pk])

    def test_resultados_ocultos_hasta_publicarlos(self):
        estudiante = self._estudiante("est-ranking")
        self.client.force_login(estudiante)
        response = self.client.get(reverse("hackaton:ranking", args=[self.hackathon.pk]))
        self.assertFalse(response.context["puede_ver"])

    def test_publicar_resultados_los_hace_visibles_y_notifica(self):
        estudiante = self._estudiante("est-ganador")
        MiembroEquipo.objects.create(equipo=self.equipo_a, estudiante=estudiante)
        self._votar(self.jurado_1, self.equipo_a, "5.00")

        services.publicar_resultados(self.hackathon, self.admin)
        self.hackathon.refresh_from_db()
        self.assertTrue(self.hackathon.resultados_publicados)
        self.assertTrue(
            Notificacion.objects.filter(
                usuario=estudiante, evento="HACKATON_RESULTADOS"
            ).exists()
        )

        self.client.force_login(estudiante)
        response = self.client.get(reverse("hackaton:ranking", args=[self.hackathon.pk]))
        self.assertTrue(response.context["puede_ver"])

    def test_equipo_rechazado_sale_del_ranking_y_no_gana(self):
        # HCK-02: dos equipos con votos; el mayor queda rechazado despues.
        self._votar(self.jurado_1, self.equipo_a, "4.00")
        self._votar(self.jurado_1, self.equipo_b, "5.00")
        cambiar_estado(
            InscripcionHackaton.objects.get(equipo=self.equipo_b),
            services.MAQUINA_INSCRIPCION, "RECHAZADA", self.admin,
        )

        tabla = services.ranking(self.hackathon)
        self.assertEqual([fila["equipo"] for fila in tabla], ["Equipo A"])
        self.assertEqual(services.ganadores(tabla)[0]["equipo"], "Equipo A")

    def test_equipo_aceptado_sin_votos_cierra_el_ranking(self):
        # HCK-10: antes el equipo sin votos desaparecia de la tabla y del informe.
        self._votar(self.jurado_1, self.equipo_a, "4.00")
        tabla = services.ranking(self.hackathon)
        self.assertEqual([fila["equipo"] for fila in tabla], ["Equipo A", "Equipo B"])
        self.assertEqual(tabla[1]["posicion"], 2)
        self.assertEqual(tabla[1]["votos"], 0)
        self.assertEqual(tabla[1]["promedio"], Decimal("0"))

    def test_sin_votos_no_se_declara_ganador(self):
        estudiante = self._estudiante("est-sinvotos")
        MiembroEquipo.objects.create(equipo=self.equipo_a, estudiante=estudiante)
        tabla = services.ranking(self.hackathon)
        self.assertEqual(services.ganadores(tabla), [])
        services.publicar_resultados(self.hackathon, self.admin)
        aviso = Notificacion.objects.get(usuario=estudiante, evento="HACKATON_RESULTADOS")
        self.assertNotIn("ganador", aviso.mensaje.lower())

    def test_empate_total_no_declara_un_unico_ganador(self):
        # HCK-06: mismo promedio y mismos votos => empate; antes coronaba al
        # primero por orden alfabetico.
        estudiante = self._estudiante("est-empate")
        MiembroEquipo.objects.create(equipo=self.equipo_a, estudiante=estudiante)
        self._votar(self.jurado_1, self.equipo_a, "4.00")
        self._votar(self.jurado_1, self.equipo_b, "4.00")

        tabla = services.ranking(self.hackathon)
        self.assertEqual(
            {fila["equipo"] for fila in services.ganadores(tabla)},
            {"Equipo A", "Equipo B"},
        )
        services.publicar_resultados(self.hackathon, self.admin)
        aviso = Notificacion.objects.get(usuario=estudiante, evento="HACKATON_RESULTADOS")
        self.assertIn("Equipo A", aviso.mensaje)
        self.assertIn("Equipo B", aviso.mensaje)
        self.assertIn("empate", aviso.mensaje)


class PublicacionTests(BaseHackathonTests):
    def test_publicar_notifica_a_los_estudiantes_aceptados(self):
        estudiante = self._estudiante("est-aviso")
        self.hackathon.estado = "borrador"
        self.hackathon.save()

        services.publicar(self.hackathon, self.admin)
        self.hackathon.refresh_from_db()
        self.assertEqual(self.hackathon.estado, "publicado")
        self.assertTrue(
            Notificacion.objects.filter(
                usuario=estudiante, evento="HACKATON_PUBLICADO"
            ).exists()
        )

    def test_profesor_ajeno_no_gestiona_el_hackathon(self):
        ajeno = Usuario.objects.create_user(username="profe-ajeno", password="x", rol="PROFESOR")
        self.client.force_login(ajeno)
        response = self.client.post(reverse("hackaton:publicar", args=[self.hackathon.pk]))
        self.assertRedirects(
            response, reverse("hackaton:detalle", args=[self.hackathon.pk]),
            fetch_redirect_response=False,
        )
        # HCK-09: el detalle de un hackathon ajeno tampoco se abre por URL.
        self.assertRedirects(
            self.client.get(reverse("hackaton:detalle", args=[self.hackathon.pk])),
            reverse("hackaton:lista"),
        )
        self.hackathon.refresh_from_db()
        self.assertEqual(self.hackathon.estado, "publicado")


class CalendarioAutomaticoHU09Tests(BaseHackathonTests):
    """HU09: avance por calendario y publicacion automatica de resultados al cierre."""

    def _etapa(self, tipo, inicia, termina=None):
        return EtapaHackaton.objects.create(
            hackathon=self.hackathon, tipo=tipo, titulo=tipo, inicia_en=inicia, termina_en=termina
        )

    def test_pasa_a_en_curso_al_iniciar_el_desarrollo(self):
        ahora = timezone.now()
        self._etapa("desarrollo", ahora - timedelta(hours=1))
        self._etapa("premiacion", ahora + timedelta(days=5))
        self.assertEqual(services.avanzar_por_calendario(self.hackathon), ["en_curso"])
        self.hackathon.refresh_from_db()
        self.assertEqual(self.hackathon.estado, "en_curso")
        self.assertFalse(self.hackathon.resultados_publicados)

    def test_al_cierre_publica_resultados_y_ganador_y_deja_historial(self):
        from apps.seguimiento.services import historial_de

        ahora = timezone.now()
        self._etapa("desarrollo", ahora - timedelta(days=3))
        self._etapa("premiacion", ahora - timedelta(minutes=5))
        estudiante = self._estudiante("est-cierre")
        equipo = self._equipo("Equipo Cierre", estudiante)
        self._inscribir(equipo)
        VotacionHackaton.objects.create(hackathon=self.hackathon, jurado=self._jurado("jur-cierre"),
                                        equipo=equipo, puntaje=Decimal("4.5"))

        services.avanzar_por_calendario(self.hackathon)
        self.hackathon.refresh_from_db()
        self.assertEqual(self.hackathon.estado, "finalizado")
        self.assertTrue(self.hackathon.resultados_publicados)
        aviso = Notificacion.objects.get(usuario=estudiante, evento="HACKATON_RESULTADOS")
        self.assertIn("Equipo Cierre", aviso.mensaje)
        pasos = list(historial_de(self.hackathon).values_list("estado_nuevo", flat=True))
        self.assertIn("finalizado", pasos)
        # Idempotente: una segunda pasada no repite nada.
        self.assertEqual(services.avanzar_por_calendario(self.hackathon), [])

    def test_sin_fechas_no_cierra_solo(self):
        self._etapa("premiacion", None)
        self.assertEqual(services.avanzar_por_calendario(self.hackathon), [])

    def test_sin_premiacion_cierra_al_terminar_la_ultima_etapa(self):
        ahora = timezone.now()
        self._etapa("presentacion", ahora - timedelta(days=2), ahora - timedelta(hours=1))
        self.assertEqual(services.fecha_de_cierre(self.hackathon), ahora - timedelta(hours=1))
        services.avanzar_por_calendario(self.hackathon)
        self.hackathon.refresh_from_db()
        self.assertTrue(self.hackathon.resultados_publicados)

    def test_el_comando_programado_avanza_los_hackathones(self):
        from io import StringIO

        from django.core.management import call_command

        self._etapa("premiacion", timezone.now() - timedelta(minutes=1))
        salida = StringIO()
        call_command("avanzar_hackathones", stdout=salida)
        self.hackathon.refresh_from_db()
        self.assertTrue(self.hackathon.resultados_publicados)
        self.assertIn("1 hackathon(es) actualizados", salida.getvalue())

    def test_abrir_el_detalle_aplica_el_calendario(self):
        self._etapa("premiacion", timezone.now() - timedelta(minutes=1))
        self.client.force_login(self.admin)
        self.client.get(reverse("hackaton:detalle", args=[self.hackathon.pk]))
        self.hackathon.refresh_from_db()
        self.assertTrue(self.hackathon.resultados_publicados)


class LogisticaEInformeHU09Tests(BaseHackathonTests):
    def test_gestor_registra_logistica_e_invitados(self):
        self.client.force_login(self.admin)
        respuesta = self.client.post(reverse("hackaton:logistica", args=[self.hackathon.pk]), {
            "modalidad": "hibrida", "lugar": "Auditorio EAN", "enlace_transmision": "https://youtube.com/live/x",
            "notas_logistica": "Refrigerios a las 10",
            "invitados-TOTAL_FORMS": "1", "invitados-INITIAL_FORMS": "0",
            "invitados-MIN_NUM_FORMS": "0", "invitados-MAX_NUM_FORMS": "1000",
            "invitados-0-tipo": "MENTOR", "invitados-0-nombre": "Ana Mentora",
            "invitados-0-organizacion": "ACME", "invitados-0-email": "ana@acme.co", "invitados-0-confirmado": "on",
        })
        self.assertRedirects(respuesta, reverse("hackaton:detalle", args=[self.hackathon.pk]))
        self.hackathon.refresh_from_db()
        self.assertEqual(self.hackathon.lugar, "Auditorio EAN")
        invitado = self.hackathon.invitados.get()
        self.assertTrue(invitado.confirmado)
        detalle = self.client.get(reverse("hackaton:detalle", args=[self.hackathon.pk]))
        self.assertContains(detalle, "Ana Mentora")

    def test_evento_virtual_exige_enlace_de_transmision(self):
        from .forms import LogisticaForm

        form = LogisticaForm({"modalidad": "virtual"}, instance=self.hackathon)
        self.assertFalse(form.is_valid())
        self.assertIn("enlace_transmision", form.errors)

    def test_estudiante_no_gestiona_la_logistica(self):
        estudiante = self._estudiante("est-log")
        self.client.force_login(estudiante)
        respuesta = self.client.get(reverse("hackaton:logistica", args=[self.hackathon.pk]))
        self.assertEqual(respuesta.status_code, 302)

    def test_informe_final_en_pdf_y_excel_con_el_ranking(self):
        from io import BytesIO

        from openpyxl import load_workbook

        estudiante = self._estudiante("est-inf")
        equipo = self._equipo("Equipo =SUMA(A1)", estudiante)
        self._inscribir(equipo)
        VotacionHackaton.objects.create(hackathon=self.hackathon, jurado=self._jurado("jur-inf"),
                                        equipo=equipo, puntaje=Decimal("5"))
        services.publicar_resultados(self.hackathon, self.admin)
        url = reverse("hackaton:informe_final", args=[self.hackathon.pk])

        # La empresa duena del reto tambien puede descargarlo.
        self.client.force_login(self.empresa)
        pdf = self.client.get(url + "?formato=pdf")
        self.assertEqual(pdf["Content-Type"], "application/pdf")
        self.assertTrue(pdf.content.startswith(b"%PDF"))

        excel = self.client.get(url + "?formato=excel")
        libro = load_workbook(BytesIO(excel.content))
        self.assertIn("Ranking", libro.sheetnames)
        valores = [c.value for fila in libro["Ranking"].iter_rows() for c in fila]
        self.assertIn("Equipo =SUMA(A1)", valores)
        self.assertEqual(libro["Ranking"].cell(row=5, column=2).data_type, "s")

    def test_estudiante_no_descarga_el_informe(self):
        estudiante = self._estudiante("est-noinf")
        self.client.force_login(estudiante)
        respuesta = self.client.get(reverse("hackaton:informe_final", args=[self.hackathon.pk]))
        self.assertEqual(respuesta.status_code, 302)


class ExpertoJuradoTests(BaseHackathonTests):
    """Un experto asignado como jurado ve y vota su hackathon; los demas no le aparecen."""

    def setUp(self):
        super().setUp()
        self.experto = Usuario.objects.create_user(username="exp-jur", password="x", rol="EXPERTO")
        Jurado.objects.create(hackathon=self.hackathon, usuario=self.experto, nombre="Experto")
        otro_reto = Reto.objects.create(empresa=self.empresa, titulo="Otro", estado="en_curso", tipo="hackathon")
        self.ajeno = Hackathon.objects.create(reto=otro_reto, estado="publicado")
        self._abrir_presentacion()
        self.client.force_login(self.experto)

    def test_lista_solo_sus_hackathones(self):
        respuesta = self.client.get(reverse("hackaton:lista"))
        self.assertEqual(respuesta.status_code, 200)
        self.assertEqual(list(respuesta.context["hackathones"]), [self.hackathon])

    def test_ve_el_detalle_del_suyo_pero_no_el_ajeno(self):
        self.assertEqual(self.client.get(reverse("hackaton:detalle", args=[self.hackathon.pk])).status_code, 200)
        self.assertRedirects(self.client.get(reverse("hackaton:detalle", args=[self.ajeno.pk])),
                             reverse("hackaton:lista"))

    def test_vota_como_jurado(self):
        estudiante = self._estudiante("est-exp")
        equipo = self._equipo("Equipo Experto", estudiante)
        self._inscribir(equipo)
        self.client.post(reverse("hackaton:votar", args=[self.hackathon.pk]), {
            "equipo": equipo.pk, "puntaje": "4", "comentario": "Bien",
        })
        self.assertTrue(VotacionHackaton.objects.filter(jurado__usuario=self.experto, equipo=equipo).exists())

    def test_el_formulario_de_jurados_ofrece_expertos(self):
        # HCK-12 + MAN-HCK-08: EXPERTO asignable y nombre completado desde la cuenta.
        experto = Usuario.objects.create_user(username="exp-asignable", password="x", rol="EXPERTO")
        self.client.force_login(self.admin)
        respuesta = self.client.post(reverse("hackaton:jurados", args=[self.hackathon.pk]), {
            "usuario": experto.pk, "nombre": "", "email": "", "especialidad": "Innovacion",
        })
        self.assertRedirects(respuesta, reverse("hackaton:jurados", args=[self.hackathon.pk]))
        jurado = Jurado.objects.get(usuario=experto)
        self.assertEqual(jurado.nombre, "exp-asignable")


class JuradoIntegridadTests(BaseHackathonTests):
    """HCK-03: retirar un jurado no puede reescribir resultados publicados."""

    def test_eliminar_jurado_no_altera_resultados_publicados(self):
        jurado = self._jurado("jur-publicado")
        equipo = self._equipo("Equipo Jurado")
        self._inscribir(equipo)
        VotacionHackaton.objects.create(
            hackathon=self.hackathon, jurado=jurado, equipo=equipo, puntaje=Decimal("5")
        )
        services.publicar_resultados(self.hackathon, self.admin)

        self.client.force_login(self.admin)
        respuesta = self.client.post(reverse("hackaton:eliminar_jurado", args=[jurado.pk]))
        self.assertRedirects(respuesta, reverse("hackaton:jurados", args=[self.hackathon.pk]))
        self.assertTrue(Jurado.objects.filter(pk=jurado.pk).exists())
        self.assertEqual(VotacionHackaton.objects.filter(jurado=jurado).count(), 1)
        self.assertEqual(services.ranking(self.hackathon)[0]["equipo"], "Equipo Jurado")

    def test_no_se_retira_un_jurado_con_resultados_publicados_aunque_no_votara(self):
        jurado = self._jurado("jur-sinvotos")
        services.publicar_resultados(self.hackathon, self.admin)
        self.client.force_login(self.admin)
        self.client.post(reverse("hackaton:eliminar_jurado", args=[jurado.pk]))
        self.assertTrue(Jurado.objects.filter(pk=jurado.pk).exists())

    def test_jurado_sin_votos_antes_de_publicar_si_se_puede_retirar(self):
        jurado = self._jurado("jur-temporal")
        self.client.force_login(self.admin)
        respuesta = self.client.post(reverse("hackaton:eliminar_jurado", args=[jurado.pk]))
        self.assertRedirects(respuesta, reverse("hackaton:jurados", args=[self.hackathon.pk]))
        self.assertFalse(Jurado.objects.filter(pk=jurado.pk).exists())


class ReaperturaHackathonTests(BaseHackathonTests):
    """HCK-04: un hackathon finalizado no se reabre ni deja re-votar."""

    def test_editar_no_puede_reabrir_un_hackathon_finalizado(self):
        services.publicar_resultados(self.hackathon, self.admin)
        self.client.force_login(self.admin)
        respuesta = self.client.post(
            reverse("hackaton:editar", args=[self.hackathon.pk]),
            {"reglas": "Nuevas reglas", "estado": "en_curso"},
        )
        # El formulario rechaza la reapertura y la instancia no cambia.
        self.assertEqual(respuesta.status_code, 200)
        self.hackathon.refresh_from_db()
        self.assertEqual(self.hackathon.estado, "finalizado")
        self.assertTrue(self.hackathon.resultados_publicados)

    def test_con_resultados_publicados_la_votacion_queda_cerrada(self):
        # Aunque el estado volviera a abrirse por otra via, no se re-vota.
        self._abrir_presentacion()
        self.hackathon.resultados_publicados = True
        self.hackathon.estado = "en_curso"
        self.hackathon.save()
        equipo = self._equipo("Equipo Reabierto")
        self._inscribir(equipo)
        jurado = self._jurado("jur-reabierto")
        self.assertFalse(services.votacion_abierta(self.hackathon))
        self.client.force_login(jurado.usuario)
        respuesta = self.client.post(
            reverse("hackaton:votar", args=[self.hackathon.pk]),
            {"equipo": equipo.pk, "puntaje": "5", "comentario": ""},
        )
        self.assertRedirects(respuesta, reverse("hackaton:detalle", args=[self.hackathon.pk]))
        self.assertEqual(VotacionHackaton.objects.count(), 0)


class CalendarioConcurrenteTests(BaseHackathonTests):
    """HCK-01: el avance por calendario decide sobre el estado bloqueado en BD."""

    def _etapa(self, tipo, inicia, termina=None):
        return EtapaHackaton.objects.create(
            hackathon=self.hackathon, tipo=tipo, titulo=tipo, inicia_en=inicia, termina_en=termina
        )

    def test_instancia_obsoleta_no_duplica_la_transicion(self):
        from apps.seguimiento.services import historial_de

        ahora = timezone.now()
        self._etapa("desarrollo", ahora - timedelta(hours=1))
        obsoleta = Hackathon.objects.get(pk=self.hackathon.pk)  # copia "en vuelo"

        self.assertEqual(services.avanzar_por_calendario(self.hackathon), ["en_curso"])
        # La llamada simultanea decide con el estado fresco, no con su copia.
        self.assertEqual(services.avanzar_por_calendario(obsoleta), [])
        self.hackathon.refresh_from_db()
        self.assertEqual(self.hackathon.estado, "en_curso")
        self.assertEqual(historial_de(self.hackathon).filter(estado_nuevo="en_curso").count(), 1)

    def test_no_sobrescribe_un_cierre_ya_publicado(self):
        from apps.seguimiento.services import historial_de

        ahora = timezone.now()
        self._etapa("premiacion", ahora - timedelta(minutes=5))
        obsoleta = Hackathon.objects.get(pk=self.hackathon.pk)
        services.publicar_resultados(self.hackathon, self.admin)

        self.assertEqual(services.avanzar_por_calendario(obsoleta), [])
        obsoleta.refresh_from_db()
        self.assertEqual(obsoleta.estado, "finalizado")
        self.assertTrue(obsoleta.resultados_publicados)
        self.assertEqual(historial_de(obsoleta).filter(estado_nuevo="finalizado").count(), 1)


class CreacionHackathonTests(BaseHackathonTests):
    """HCK-07: solo el admin o el profesor que integro el reto crean su hackathon."""

    def test_profesor_ajeno_no_crea_hackathon(self):
        otro_reto = Reto.objects.create(
            empresa=self.empresa, titulo="Reto de otro profesor", estado="en_curso", tipo="hackathon"
        )
        ajeno = Usuario.objects.create_user(username="profe-sin-integracion", password="x", rol="PROFESOR")
        self.client.force_login(ajeno)
        respuesta = self.client.post(
            reverse("hackaton:crear", args=[otro_reto.pk]),
            {"reglas": "Reglas", "estado": "borrador"},
        )
        self.assertEqual(respuesta.status_code, 302)
        self.assertEqual(respuesta.url, reverse("retos:detalle", args=[otro_reto.pk]))
        self.assertFalse(Hackathon.objects.filter(reto=otro_reto).exists())

    def test_profesor_con_integracion_si_crea_el_hackathon(self):
        otro_reto = Reto.objects.create(
            empresa=self.empresa, titulo="Reto integrado", estado="en_curso", tipo="hackathon"
        )
        IntegracionAcademica.objects.create(reto=otro_reto, profesor=self.profesor, estado="aprobada")
        self.client.force_login(self.profesor)
        respuesta = self.client.post(
            reverse("hackaton:crear", args=[otro_reto.pk]),
            {"reglas": "Reglas", "estado": "borrador"},
        )
        self.assertEqual(respuesta.status_code, 302)
        self.assertTrue(Hackathon.objects.filter(reto=otro_reto).exists())


class InscripcionDobleTests(BaseHackathonTests):
    """HCK-08 y HCK-11: un equipo por estudiante y sin 500 en la carrera."""

    def test_un_estudiante_no_inscribe_dos_equipos_distintos(self):
        estudiante = self._estudiante("est-dos-equipos")
        alfa = self._equipo("Alfa Doble", estudiante)
        beta = self._equipo("Beta Doble", estudiante)
        self.client.force_login(estudiante)
        url = reverse("hackaton:inscribir_equipo", args=[self.hackathon.pk])
        self.client.post(url, {"equipo": alfa.pk})
        respuesta = self.client.post(url, {"equipo": beta.pk})
        # El segundo equipo ya no esta disponible en el formulario.
        self.assertEqual(respuesta.status_code, 200)
        self.assertEqual(InscripcionHackaton.objects.filter(hackathon=self.hackathon).count(), 1)

    def test_inscripcion_duplicada_concurrente_no_devuelve_500(self):
        estudiante = self._estudiante("est-carrera")
        equipo = self._equipo("Equipo Carrera", estudiante)
        self.client.force_login(estudiante)
        url = reverse("hackaton:inscribir_equipo", args=[self.hackathon.pk])
        hackathon = self.hackathon
        original = InscripcionForm.is_valid

        def is_valid_con_carrera(self, *args, **kwargs):
            valido = original(self, *args, **kwargs)
            if valido and "equipo" in self.cleaned_data:
                # Otra peticion simultanea inscribe al equipo entre la
                # validacion del formulario y el create de esta.
                InscripcionHackaton.objects.create(
                    hackathon=hackathon, equipo=self.cleaned_data["equipo"]
                )
            return valido

        with mock.patch.object(InscripcionForm, "is_valid", is_valid_con_carrera):
            respuesta = self.client.post(url, {"equipo": equipo.pk})
        # Mensaje amigable y una sola inscripcion, no un error 500.
        self.assertEqual(respuesta.status_code, 302)
        self.assertEqual(InscripcionHackaton.objects.filter(equipo=equipo).count(), 1)


class AlcancePorRolTests(BaseHackathonTests):
    """HCK-09: el acceso directo por URL respeta el alcance del listado."""

    def setUp(self):
        super().setUp()
        reto_borrador = Reto.objects.create(
            empresa=self.empresa, titulo="Reto en borrador", estado="aprobado", tipo="hackathon"
        )
        self.borrador = Hackathon.objects.create(reto=reto_borrador, estado="borrador")

    def test_empresa_ajena_no_abre_el_detalle(self):
        ajena = Usuario.objects.create_user(username="empresa-ajena", password="x", rol="EMPRESA")
        self.client.force_login(ajena)
        respuesta = self.client.get(reverse("hackaton:detalle", args=[self.hackathon.pk]))
        self.assertRedirects(respuesta, reverse("hackaton:lista"))

    def test_profesor_ajeno_no_abre_el_borrador(self):
        ajeno = Usuario.objects.create_user(username="profe-lector", password="x", rol="PROFESOR")
        self.client.force_login(ajeno)
        respuesta = self.client.get(reverse("hackaton:detalle", args=[self.borrador.pk]))
        self.assertRedirects(respuesta, reverse("hackaton:lista"))

    def test_profesor_ajeno_no_abre_el_ranking(self):
        ajeno = Usuario.objects.create_user(username="profe-ranking", password="x", rol="PROFESOR")
        self.client.force_login(ajeno)
        respuesta = self.client.get(reverse("hackaton:ranking", args=[self.hackathon.pk]))
        self.assertRedirects(respuesta, reverse("hackaton:lista"))

    def test_la_empresa_duena_del_reto_si_ve_el_detalle(self):
        self.client.force_login(self.empresa)
        respuesta = self.client.get(reverse("hackaton:detalle", args=[self.hackathon.pk]))
        self.assertEqual(respuesta.status_code, 200)
