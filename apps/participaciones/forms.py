from django import forms

from apps.academico.models import Programa

from .models import Postulacion


class PostulacionForm(forms.ModelForm):
    programa = forms.ChoiceField(
        choices=[('', 'Selecciona tu programa académico')],
        required=False,
        widget=forms.Select(attrs={'class': 'form-select'}),
        label='Programa académico',
    )

    class Meta:
        model = Postulacion
        fields = ['motivacion', 'habilidades', 'semestre', 'programa']
        widgets = {
            'motivacion': forms.Textarea(attrs={
                'class': 'form-control', 'rows': 4,
                'placeholder': '¿Por qué quieres participar en este reto?'
            }),
            'habilidades': forms.Textarea(attrs={
                'class': 'form-control', 'rows': 3,
                'placeholder': 'Menciona tus habilidades y conocimientos relevantes'
            }),
            'semestre': forms.NumberInput(attrs={
                'class': 'form-control', 'min': 1, 'max': 12
            }),
        }
        labels = {
            'motivacion': 'Motivación',
            'habilidades': 'Habilidades',
            'semestre': 'Semestre',
            'programa': 'Programa académico',
        }

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)

        programas = list(
            Programa.objects.select_related('facultad').order_by('nombre', 'facultad__nombre')
        )
        choices = [('', 'Selecciona tu programa académico')]
        choices.extend(
            [
                (
                    programa.nombre,
                    f'{programa.nombre} ({programa.facultad.nombre})',
                )
                for programa in programas
            ]
        )
        self.fields['programa'].choices = choices

        # Permite editar postulaciones antiguas con programa fuera del catálogo actual.
        programa_actual = (getattr(self.instance, 'programa', '') or '').strip()
        if programa_actual and programa_actual not in dict(self.fields['programa'].choices):
            self.fields['programa'].choices.append((programa_actual, f'{programa_actual} (actual)'))
