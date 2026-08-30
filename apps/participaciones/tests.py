from django.core.files.uploadedfile import SimpleUploadedFile
from django.test import TestCase
from django.urls import reverse

from apps.evaluacion.models import Entregable
from apps.retos.models import EquipoRetoAcademico, Reto
from apps.usuarios.models import Usuario

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
