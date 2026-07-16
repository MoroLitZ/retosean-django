from django.core.files.uploadedfile import SimpleUploadedFile
from django.core.exceptions import ValidationError
from django.test import SimpleTestCase

from .forms import MAX_UPLOAD_SIZE, RetoForm, SeguimientoRetoForm


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
