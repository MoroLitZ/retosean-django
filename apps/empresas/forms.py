from django import forms
from django.utils import timezone
from .models import DocumentoEmpresa

class CargarDocumentoForm(forms.ModelForm):
    class Meta:
        model = DocumentoEmpresa
        fields = ["tipo_documento", "archivo", "fecha_expedicion"]
        exclude = ('empresa',)
        widgets = {
            "tipo_documento": forms.Select(attrs={"class": "form-control"}),
            "archivo": forms.FileInput(attrs={"class": "form-control", "accept": ".pdf"}),
            "fecha_expedicion": forms.DateInput(attrs={"class": "form-control", "type": "date"}),
        }

    def clean(self):
        cleaned_data = super().clean()
        tipo = cleaned_data.get("tipo_documento")
        fecha_exp = cleaned_data.get("fecha_expedicion")
        archivo = cleaned_data.get("archivo")
        if archivo and not fecha_exp:
            self.add_error("fecha_expedicion", "Debes indicar la fecha de expedicion.")
        if tipo == "CAMARA_COMERCIO" and fecha_exp:
            hoy = timezone.now().date()
            dias = (hoy - fecha_exp).days
            if dias > 30:
                self.add_error("fecha_expedicion", f"La Camara de Comercio tiene {dias} dias y el maximo es 30.")
            elif dias < 0:
                self.add_error("fecha_expedicion", "La fecha no puede ser futura.")
        return cleaned_data