from django import forms

from .models import AgendaCierre, CierreReto, EntregableFinal, EncuestaSatisfaccion, Reconocimiento, SolicitudLogistica


class AgendaCierreForm(forms.ModelForm):
    class Meta:
        model = AgendaCierre
        fields = ["fecha_hora", "espacio", "recursos", "invitados", "agenda"]
        widgets = {"fecha_hora": forms.DateTimeInput(attrs={"type": "datetime-local"}, format="%Y-%m-%dT%H:%M")}


class CierreRetoForm(forms.ModelForm):
    class Meta:
        model = CierreReto
        fields = ["acta_url", "notas", "impacto"]


class EntregableFinalForm(forms.ModelForm):
    class Meta:
        model = EntregableFinal
        fields = ["nombre", "archivo"]


class ReconocimientoForm(forms.ModelForm):
    class Meta:
        model = Reconocimiento
        fields = ["usuario", "nombre_destinatario", "descripcion"]


class EncuestaSatisfaccionForm(forms.ModelForm):
    calificacion = forms.IntegerField(min_value=1, max_value=5)

    class Meta:
        model = EncuestaSatisfaccion
        fields = ["calificacion", "comentario"]


class SolicitudLogisticaForm(forms.ModelForm):
    class Meta:
        model = SolicitudLogistica
        fields = ["evento", "tipo", "descripcion"]
        labels = {"evento": "Evento", "tipo": "Tipo de solicitud", "descripcion": "Detalle"}
        widgets = {
            "evento": forms.Select(attrs={"class": "form-select form-select-sm"}),
            "tipo": forms.Select(attrs={"class": "form-select form-select-sm"}),
            "descripcion": forms.Textarea(attrs={"class": "form-control form-control-sm", "rows": 2,
                                                 "placeholder": "Ej.: auditorio para 80 personas, 9:00 a 12:00"}),
        }
