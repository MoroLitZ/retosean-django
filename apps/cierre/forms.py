from django import forms

from .models import AgendaCierre, CierreReto, EntregableFinal, EncuestaSatisfaccion, Reconocimiento


class AgendaCierreForm(forms.ModelForm):
    class Meta:
        model = AgendaCierre
        fields = ["fecha_hora", "espacio", "recursos", "invitados", "agenda"]
        widgets = {"fecha_hora": forms.DateTimeInput(attrs={"type": "datetime-local"})}


class CierreRetoForm(forms.ModelForm):
    class Meta:
        model = CierreReto
        fields = ["acta_url", "notas"]


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
