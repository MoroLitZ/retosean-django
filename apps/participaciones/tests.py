from django.core.files.uploadedfile import SimpleUploadedFile
from django.test import TestCase
from django.urls import reverse

from apps.academico.models import Facultad, Programa
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
        self.reto = Reto.objects.create(empresa=self.empresa, titulo="Reto abierto", estado="aprobado")
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
        self.reto = Reto.objects.create(empresa=self.empresa, titulo="Reto formulario", estado="aprobado")
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
        self.reto = Reto.objects.create(
            empresa=self.empresa,
            titulo="Reto para revisar postulaciones",
            estado="aprobado",
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
