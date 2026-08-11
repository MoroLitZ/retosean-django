from django.test import TestCase

from django.urls import reverse

from apps.retos.models import Reto
from apps.usuarios.models import Usuario


class SeguimientoEmpresaSprintTests(TestCase):
    def setUp(self):
        self.empresa = Usuario.objects.create_user(username="empresa-seg", password="x", rol="EMPRESA")
        self.otra_empresa = Usuario.objects.create_user(username="otra-empresa", password="x", rol="EMPRESA")
        self.reto = Reto.objects.create(empresa=self.empresa, titulo="Seguimiento privado", estado="en_curso")

    def test_empresa_solo_ve_su_reto(self):
        self.client.force_login(self.otra_empresa)
        response = self.client.get(reverse("empresas:seguimiento_reto", args=[self.reto.pk]))
        self.assertEqual(response.status_code, 404)

    def test_empresa_no_puede_registrar_avance_academico(self):
        self.client.force_login(self.empresa)
        response = self.client.post(reverse("retos:agregar_seguimiento", args=[self.reto.pk]), {
            "porcentaje_avance": 80,
        })
        self.assertEqual(response.status_code, 403)
