from decimal import Decimal, InvalidOperation

from django import forms

from .models import ComentarioEntregable, Rubrica


class RubricaForm(forms.ModelForm):
    criterios = forms.CharField(
        widget=forms.Textarea(attrs={"rows": 5, "placeholder": "Calidad|2.5\nPresentacion|2.5"}),
        help_text="Un criterio por linea usando Nombre|puntaje maximo.",
    )

    class Meta:
        model = Rubrica
        fields = ["nombre", "descripcion"]

    def clean_criterios(self):
        resultado = []
        for numero, linea in enumerate(self.cleaned_data["criterios"].splitlines(), start=1):
            if not linea.strip():
                continue
            try:
                nombre, valor = [parte.strip() for parte in linea.rsplit("|", 1)]
                puntaje = Decimal(valor)
            except (ValueError, InvalidOperation):
                raise forms.ValidationError(f"La linea {numero} debe usar el formato Nombre|puntaje.")
            if not nombre or puntaje <= 0:
                raise forms.ValidationError(f"La linea {numero} contiene un criterio invalido.")
            resultado.append((nombre, puntaje))
        if not resultado:
            raise forms.ValidationError("Agrega al menos un criterio.")
        return resultado


class ComentarioEntregableForm(forms.ModelForm):
    class Meta:
        model = ComentarioEntregable
        fields = ["comentario"]
        widgets = {"comentario": forms.Textarea(attrs={"rows": 3})}
