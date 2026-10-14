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

class ConvenioForm(forms.Form):
    """Avance del tramite del convenio. Las opciones son las transiciones validas
    desde el estado actual (`empresas.services.MAQUINA_CONVENIO`)."""

    estado_convenio = forms.ChoiceField(
        label="Nuevo estado",
        choices=[],
        widget=forms.Select(attrs={"class": "form-select form-select-sm"}),
    )
    fecha_vencimiento = forms.DateField(
        label="Fecha de vencimiento (si queda firmado)",
        required=False,
        widget=forms.DateInput(attrs={"class": "form-control form-control-sm", "type": "date"}),
    )
    comentario = forms.CharField(
        label="Comentario",
        required=False,
        widget=forms.TextInput(attrs={"class": "form-control form-control-sm"}),
    )

    def __init__(self, *args, opciones=(), **kwargs):
        super().__init__(*args, **kwargs)
        self.fields["estado_convenio"].choices = list(opciones)

    def clean(self):
        cleaned_data = super().clean()
        if cleaned_data.get("estado_convenio") == "FIRMADO" and not cleaned_data.get("fecha_vencimiento"):
            self.add_error("fecha_vencimiento", "Indica hasta cuando esta vigente el convenio firmado.")
        return cleaned_data
