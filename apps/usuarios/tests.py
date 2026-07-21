from django.test import TestCase
from django.urls import reverse


class RoutingSmokeTests(TestCase):
    def test_login_page_loads(self):
        response = self.client.get(reverse("usuarios:login"))

        self.assertEqual(response.status_code, 200)

    def test_root_redirects_to_profile_workflow(self):
        response = self.client.get("/")

        self.assertEqual(response.status_code, 302)
        self.assertIn(reverse("usuarios:perfil"), response.url)
