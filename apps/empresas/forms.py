from django import forms
from django.utils import timezone
from .models import DocumentoEmpresa, TipoDocumentoLegal

class CargarDocumentoForm(forms.Form):
    """Carga de un documento legal (HU00).

    Los tipos salen del catalogo `TipoDocumentoLegal`; los de permiso especial
    (Listas Informa) solo aparecen con `incluir_especiales=True`, que la vista
    pasa unicamente a quien tiene `empresas.cargar_listas_informa`.
    """

    tipo_documento = forms.ChoiceField(
        label="Tipo de documento",
        widget=forms.Select(attrs={"class": "form-control"}),
    )
    archivo = forms.FileField(
        label="Archivo (PDF)",
        widget=forms.FileInput(attrs={"class": "form-control", "accept": ".pdf"}),
    )
    fecha_expedicion = forms.DateField(
        label="Fecha de expedicion",
        required=False,
        widget=forms.DateInput(attrs={"class": "form-control", "type": "date"}, format="%Y-%m-%d"),
    )

    def __init__(self, *args, incluir_especiales=False, solo_especiales=False, **kwargs):
        super().__init__(*args, **kwargs)
        tipos = TipoDocumentoLegal.objects.filter(activo=True)
        if solo_especiales:
            tipos = tipos.filter(requiere_permiso_especial=True)
        elif not incluir_especiales:
            tipos = tipos.filter(requiere_permiso_especial=False)
        self._tipos = {tipo.codigo: tipo for tipo in tipos}
        self.fields["tipo_documento"].choices = [
            (tipo.codigo, f"{tipo.nombre}{'' if tipo.obligatorio else ' (opcional)'}") for tipo in tipos
        ]

    def clean_archivo(self):
        archivo = self.cleaned_data["archivo"]
        extension = archivo.name.rsplit(".", 1)[-1].lower() if "." in archivo.name else ""
        if extension not in DocumentoEmpresa.EXTENSIONES_PERMITIDAS:
            permitidas = ", ".join(DocumentoEmpresa.EXTENSIONES_PERMITIDAS).upper()
            raise forms.ValidationError(f"Formato no permitido. Solo se aceptan archivos {permitidas}.")
        return archivo

    def clean(self):
        cleaned_data = super().clean()
        tipo = self._tipos.get(cleaned_data.get("tipo_documento"))
        fecha_exp = cleaned_data.get("fecha_expedicion")
        if cleaned_data.get("archivo") and not fecha_exp:
            self.add_error("fecha_expedicion", "Debes indicar la fecha de expedicion.")
        if fecha_exp:
            dias = (timezone.localdate() - fecha_exp).days
            if dias < 0:
                self.add_error("fecha_expedicion", "La fecha no puede ser futura.")
            elif tipo and tipo.vigencia_dias and dias > tipo.vigencia_dias:
                self.add_error(
                    "fecha_expedicion",
                    f"{tipo.nombre} tiene {dias} dias de expedida y el maximo es {tipo.vigencia_dias}.",
                )
        return cleaned_data


class TipoDocumentoLegalForm(forms.ModelForm):
    class Meta:
        model = TipoDocumentoLegal
        fields = [
            "nombre", "descripcion", "obligatorio", "aplica_sin_convenio",
            "vigencia_dias", "requiere_permiso_especial", "activo", "orden",
        ]
        labels = {
            "obligatorio": "Obligatorio",
            "aplica_sin_convenio": "Se exige tambien a empresas sin convenio",
            "vigencia_dias": "Vigencia maxima (dias desde la expedicion)",
            "requiere_permiso_especial": "Solo lo carga un usuario con permiso especial",
        }
        widgets = {
            "nombre": forms.TextInput(attrs={"class": "form-control"}),
            "descripcion": forms.Textarea(attrs={"class": "form-control", "rows": 2}),
            "vigencia_dias": forms.NumberInput(attrs={"class": "form-control"}),
            "orden": forms.NumberInput(attrs={"class": "form-control"}),
        }


class BloqueoParticipacionForm(forms.Form):
    motivo = forms.CharField(
        label="Motivo del bloqueo",
        required=False,
        widget=forms.TextInput(attrs={"class": "form-control form-control-sm"}),
    )


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
        widget=forms.DateInput(attrs={"class": "form-control form-control-sm", "type": "date"}, format="%Y-%m-%d"),
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
