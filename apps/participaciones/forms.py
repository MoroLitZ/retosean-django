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
        fields = ['motivacion', 'habilidades', 'semestre', 'programa', 'hoja_de_vida', 'acepta_terminos']
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
            'hoja_de_vida': forms.ClearableFileInput(attrs={'class': 'form-control', 'accept': '.pdf'}),
            'acepta_terminos': forms.CheckboxInput(attrs={'class': 'form-check-input'}),
        }
        labels = {
            'motivacion': 'Motivación',
            'habilidades': 'Habilidades',
            'semestre': 'Semestre',
            'programa': 'Programa académico',
            'hoja_de_vida': 'Hoja de vida o portafolio (PDF, opcional)',
            'acepta_terminos': 'Acepto los términos y condiciones del reto y el tratamiento de mis datos personales',
        }

    TAMANO_MAXIMO_HOJA_DE_VIDA = 5 * 1024 * 1024

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.fields['acepta_terminos'].required = True
        self.fields['acepta_terminos'].error_messages['required'] = (
            'Debes aceptar los términos y condiciones para postularte.'
        )

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

    def clean_hoja_de_vida(self):
        archivo = self.cleaned_data.get('hoja_de_vida')
        if not archivo or not hasattr(archivo, 'size'):
            return archivo
        if not archivo.name.lower().endswith('.pdf'):
            raise forms.ValidationError('La hoja de vida debe ser un archivo PDF.')
        if archivo.size > self.TAMANO_MAXIMO_HOJA_DE_VIDA:
            raise forms.ValidationError('La hoja de vida no puede superar 5 MB.')
        return archivo
