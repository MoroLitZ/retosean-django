from django import forms

from .models import Gasto, Presupuesto

CLASE_INPUT = "form-control form-control-sm"
CLASE_SELECT = "form-select form-select-sm"


class PresupuestoForm(forms.ModelForm):
    class Meta:
        model = Presupuesto
        fields = ["monto_total"]
        labels = {"monto_total": "Presupuesto asignado"}
        widgets = {
            "monto_total": forms.NumberInput(
                attrs={"class": CLASE_INPUT, "step": "0.01", "min": "0"}
            ),
        }

    def clean_monto_total(self):
        monto = self.cleaned_data["monto_total"]
        if monto < 0:
            raise forms.ValidationError("El presupuesto no puede ser negativo.")
        return monto


class GastoForm(forms.ModelForm):
    class Meta:
        model = Gasto
        fields = ["categoria", "monto", "descripcion", "comprobante"]
        widgets = {
            "categoria": forms.Select(attrs={"class": CLASE_SELECT}),
            "monto": forms.NumberInput(attrs={"class": CLASE_INPUT, "step": "0.01", "min": "0.01"}),
            "descripcion": forms.Textarea(attrs={"class": CLASE_INPUT, "rows": 2}),
            "comprobante": forms.ClearableFileInput(attrs={"class": CLASE_INPUT}),
        }

    def clean_monto(self):
        monto = self.cleaned_data["monto"]
        if monto <= 0:
            raise forms.ValidationError("El monto del gasto debe ser mayor que cero.")
        return monto
