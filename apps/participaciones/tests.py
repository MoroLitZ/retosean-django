from django.core.files.uploadedfile import SimpleUploadedFile
from django.test import TestCase
from django.urls import reverse

from apps.academico.models import Facultad, Programa
from apps.empresas.models import Empresa
from apps.evaluacion.models import Entregable
from apps.notificaciones.models import Notificacion
from apps.retos.models import EquipoRetoAcademico, Reto
from apps.usuarios.models import Usuario

from .forms import PostulacionForm
from .models import Postulacion, RetoFavorito


class AccesoAEntregablesTests(TestCase):
    """La subida de entregables solo esta abierta a quien participa en el reto."""

    def setUp(self):
        self.empresa = Usuario.objects.create_user(username="empresa-part", password="x", rol="EMPRESA")
        self.reto = Reto.objects.create(empresa=self.empresa, titulo="Reto abierto", estado="en_curso")
        self.aceptado = Usuario.objects.create_user(username="est-ok", password="x", rol="ESTUDIANTE")
        self.intruso = Usuario.objects.create_user(username="est-intruso", password="x", rol="ESTUDIANTE")
        Postulacion.objects.create(reto=self.reto, estudiante=self.aceptado, estado="ACEPTADA")
        Postulacion.objects.create(reto=self.reto, estudiante=self.intruso, estado="RECHAZADA")
        self.url = reverse("participaciones:mis_entregables_reto", args=[self.reto.pk])

    def _payload(self):
        return {
            "titulo": "Avance",
            "archivo": SimpleUploadedFile("avance.pdf", b"contenido"),
            "comentario_estudiante": "",
        }

    def test_estudiante_aceptado_puede_subir(self):
        self.client.force_login(self.aceptado)
        self.client.post(self.url, self._payload())
        self.assertTrue(Entregable.objects.filter(reto=self.reto, estudiante=self.aceptado).exists())

    def test_estudiante_sin_postulacion_aceptada_no_puede_subir(self):
        self.client.force_login(self.intruso)
        response = self.client.post(self.url, self._payload())
        self.assertRedirects(response, reverse("participaciones:mis_entregables"))
        self.assertFalse(Entregable.objects.filter(reto=self.reto, estudiante=self.intruso).exists())

    def test_estudiante_de_equipo_academico_puede_subir(self):
        miembro = Usuario.objects.create_user(username="est-equipo", password="x", rol="ESTUDIANTE")
        equipo = EquipoRetoAcademico.objects.create(reto=self.reto, nombre_equipo="Equipo A")
        equipo.estudiantes.add(miembro)
        self.client.force_login(miembro)
        self.client.post(self.url, self._payload())
        self.assertTrue(Entregable.objects.filter(reto=self.reto, estudiante=miembro).exists())


class FavoritosTests(TestCase):
    def setUp(self):
        empresa = Usuario.objects.create_user(username="empresa-fav", password="x", rol="EMPRESA")
        self.reto = Reto.objects.create(empresa=empresa, titulo="Reto favorito", estado="aprobado")
        self.estudiante = Usuario.objects.create_user(username="est-fav", password="x", rol="ESTUDIANTE")
        self.url = reverse("participaciones:toggle_favorito", args=[self.reto.pk])

    def test_get_no_modifica_favoritos(self):
        self.client.force_login(self.estudiante)
        response = self.client.get(self.url)
        self.assertEqual(response.status_code, 405)
        self.assertFalse(RetoFavorito.objects.filter(usuario=self.estudiante).exists())

    def test_post_alterna_favorito(self):
        self.client.force_login(self.estudiante)
        self.client.post(self.url)
        self.assertTrue(RetoFavorito.objects.filter(usuario=self.estudiante, reto=self.reto).exists())
        self.client.post(self.url)
        self.assertFalse(RetoFavorito.objects.filter(usuario=self.estudiante, reto=self.reto).exists())

    def test_no_redirige_a_dominio_externo(self):
        self.client.force_login(self.estudiante)
        response = self.client.post(self.url, {"next": "https://evil.example.com/robo"})
        self.assertRedirects(response, reverse("academico:explorar_retos"))


class PostulacionProgramaDropdownTests(TestCase):
    def setUp(self):
        self.facultad = Facultad.objects.create(nombre="Facultad de Ingenieria")
        self.programa = Programa.objects.create(
            nombre="Ingenieria de Sistemas",
            facultad=self.facultad,
        )
        self.empresa = Usuario.objects.create_user(username="empresa-pf", password="x", rol="EMPRESA")
        self.reto = Reto.objects.create(empresa=self.empresa, titulo="Reto formulario", estado="abierto")
        self.estudiante = Usuario.objects.create_user(username="est-pf", password="x", rol="ESTUDIANTE")

    def test_formulario_programa_se_renderiza_como_select(self):
        form = PostulacionForm()
        self.assertEqual(form.fields["programa"].widget.__class__.__name__, "Select")
        self.assertIn((self.programa.nombre, "Ingenieria de Sistemas (Facultad de Ingenieria)"), form.fields["programa"].choices)

    def test_formulario_rechaza_programa_fuera_de_catalogo(self):
        form = PostulacionForm(
            data={
                "motivacion": "Quiero participar",
                "habilidades": "Python",
                "semestre": 6,
                "programa": "Programa inventado",
            }
        )
        self.assertFalse(form.is_valid())
        self.assertIn("programa", form.errors)

    def test_postular_con_programa_de_catalogo_guarda_valor(self):
        self.client.force_login(self.estudiante)
        response = self.client.post(
            reverse("participaciones:postular_a_reto", args=[self.reto.pk]),
            {
                "motivacion": "Me interesa el reto",
                "habilidades": "Analisis de datos",
                "semestre": 7,
                "programa": self.programa.nombre,
                "acepta_terminos": "on",
            },
        )
        self.assertRedirects(response, reverse("academico:mis_postulaciones"))
        postulacion = Postulacion.objects.get(reto=self.reto, estudiante=self.estudiante)
        self.assertEqual(postulacion.programa, self.programa.nombre)


class FlujoNotificacionesYRevisionPostulacionTests(TestCase):
    def setUp(self):
        self.facultad = Facultad.objects.create(nombre="Facultad de Ingenieria")
        self.programa = Programa.objects.create(
            nombre="Ingenieria de Sistemas",
            facultad=self.facultad,
        )
        self.empresa = Usuario.objects.create_user(
            username="empresa-revision", password="x", rol="EMPRESA", email="empresa@ean.edu.co"
        )
        self.admin = Usuario.objects.create_user(
            username="admin-revision", password="x", rol="ADMIN", email="admin@ean.edu.co"
        )
        self.estudiante = Usuario.objects.create_user(
            username="est-revision", password="x", rol="ESTUDIANTE", email="est@ean.edu.co"
        )
        # La vista de postulaciones de la empresa exige un perfil de empresa.
        Empresa.objects.create(usuario=self.empresa, nit="900555111", razon_social="Empresa Revision SAS")
        self.reto = Reto.objects.create(
            empresa=self.empresa,
            titulo="Reto para revisar postulaciones",
            estado="abierto",
        )

    def _postular(self):
        self.client.force_login(self.estudiante)
        return self.client.post(
            reverse("participaciones:postular_a_reto", args=[self.reto.pk]),
            {
                "motivacion": "Quiero participar porque me interesa aplicar mis conocimientos.",
                "habilidades": "Python, analitica y trabajo en equipo",
                "semestre": 6,
                "programa": self.programa.nombre,
                "acepta_terminos": "on",
            },
        )

    def test_postulacion_notifica_a_empresa_y_no_a_admin(self):
        response = self._postular()
        self.assertRedirects(response, reverse("academico:mis_postulaciones"))

        self.assertTrue(
            Notificacion.objects.filter(
                usuario=self.empresa,
                evento="POSTULACION_NUEVA",
            ).exists()
        )
        self.assertFalse(
            Notificacion.objects.filter(
                usuario=self.admin,
                evento="POSTULACION_NUEVA",
            ).exists()
        )

    def test_empresa_ve_motivacion_habilidades_y_datos_en_tabla(self):
        self._postular()
        self.client.force_login(self.empresa)

        response = self.client.get(reverse("empresas:postulaciones"))

        self.assertEqual(response.status_code, 200)
        self.assertContains(response, "Ver solicitud")
        self.assertContains(response, "Motivación del estudiante")
        self.assertContains(response, "Python, analitica y trabajo en equipo")
        self.assertContains(response, "Ingenieria de Sistemas")


class ListaMisEntregablesTests(TestCase):
    """La lista de retos del estudiante en "Mis entregables" enlaza a cada reto.

    El template recorria los retos como si fueran postulaciones
    (`postulacion.reto.id`), asi que el enlace salia vacio y la pagina fallaba
    en cuanto el estudiante participaba en algun reto.
    """

    def test_lista_de_retos_del_estudiante_se_muestra(self):
        empresa = Usuario.objects.create_user(username="empresa-lista", password="x", rol="EMPRESA")
        estudiante = Usuario.objects.create_user(username="est-lista", password="x", rol="ESTUDIANTE")
        reto = Reto.objects.create(empresa=empresa, titulo="Reto con entregables", estado="en_curso")
        Postulacion.objects.create(reto=reto, estudiante=estudiante, estado="ACEPTADA")

        self.client.force_login(estudiante)
        response = self.client.get(reverse("participaciones:mis_entregables"))

        self.assertEqual(response.status_code, 200)
        self.assertContains(response, "Reto con entregables")
        self.assertContains(response, reverse("participaciones:mis_entregables_reto", args=[reto.pk]))


class PostulacionCompletaTests(TestCase):
    """HU10/HU11: filtro por ecosistema, terminos, hoja de vida, confirmacion y revision."""

    def setUp(self):
        from apps.academico.models import Ecosistema

        self.empresa = Usuario.objects.create_user(username="empresa-hu11", password="x", rol="EMPRESA")
        Empresa.objects.create(usuario=self.empresa, nit="900123123", razon_social="HU11 SAS")
        self.estudiante = Usuario.objects.create_user(username="est-hu11", password="x", rol="ESTUDIANTE")
        self.programa = Programa.objects.create(nombre="Sistemas", facultad=Facultad.objects.create(nombre="Ingenieria"))
        self.ecosistema = Ecosistema.objects.create(nombre="Innovacion social")
        self.reto = Reto.objects.create(
            empresa=self.empresa, titulo="Reto HU11", estado="abierto", ecosistema=self.ecosistema
        )
        self.otro = Reto.objects.create(empresa=self.empresa, titulo="Reto sin ecosistema", estado="abierto")

    def _datos(self, **extra):
        return {"motivacion": "Me interesa", "habilidades": "Python", "semestre": 5,
                "programa": self.programa.nombre, **extra}

    def test_explorador_filtra_por_ecosistema(self):
        self.client.force_login(self.estudiante)
        response = self.client.get(reverse("academico:explorar_retos"), {"ecosistema": self.ecosistema.pk})
        self.assertEqual([r.pk for r in response.context["retos"]], [self.reto.pk])

    def test_sin_aceptar_terminos_no_se_postula(self):
        self.client.force_login(self.estudiante)
        self.client.post(reverse("participaciones:postular_a_reto", args=[self.reto.pk]), self._datos())
        self.assertFalse(Postulacion.objects.filter(reto=self.reto).exists())

    def test_postulacion_con_hoja_de_vida_y_confirmacion_al_estudiante(self):
        self.client.force_login(self.estudiante)
        self.client.post(reverse("participaciones:postular_a_reto", args=[self.reto.pk]), self._datos(
            acepta_terminos="on", hoja_de_vida=SimpleUploadedFile("cv.pdf", b"%PDF-1.4"),
        ))
        postulacion = Postulacion.objects.get(reto=self.reto, estudiante=self.estudiante)
        self.assertTrue(postulacion.acepta_terminos)
        self.assertTrue(postulacion.hoja_de_vida)
        self.assertTrue(Notificacion.objects.filter(usuario=self.estudiante, evento="POSTULACION_RECIBIDA").exists())

    def test_hoja_de_vida_que_no_es_pdf_se_rechaza(self):
        self.client.force_login(self.estudiante)
        self.client.post(reverse("participaciones:postular_a_reto", args=[self.reto.pk]), self._datos(
            acepta_terminos="on", hoja_de_vida=SimpleUploadedFile("cv.exe", b"MZ"),
        ))
        self.assertFalse(Postulacion.objects.filter(reto=self.reto).exists())

    def test_la_empresa_al_abrir_la_lista_deja_la_postulacion_en_revision(self):
        postulacion = Postulacion.objects.create(reto=self.reto, estudiante=self.estudiante, estado="PENDIENTE")
        self.client.force_login(self.empresa)
        self.client.get(reverse("empresas:postulaciones"))
        postulacion.refresh_from_db()
        self.assertEqual(postulacion.estado, "EN_REVISION")

        self.client.post(reverse("empresas:gestionar_postulacion", args=[postulacion.pk]), {"accion": "aceptar"})
        postulacion.refresh_from_db()
        self.assertEqual(postulacion.estado, "ACEPTADA")

    def test_el_estudiante_retira_una_postulacion_en_revision(self):
        postulacion = Postulacion.objects.create(reto=self.reto, estudiante=self.estudiante, estado="EN_REVISION")
        self.client.force_login(self.estudiante)
        self.client.post(reverse("participaciones:retirar_postulacion", args=[self.reto.pk]))
        postulacion.refresh_from_db()
        self.assertEqual(postulacion.estado, "RETIRADA")

