from django.contrib.auth import get_user_model
from django.core.files.uploadedfile import SimpleUploadedFile
from django.core.exceptions import ValidationError
from django.test import SimpleTestCase, TestCase
from django.urls import reverse

from .forms import MAX_UPLOAD_SIZE, RetoForm, SeguimientoRetoForm
from .models import Reto


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
