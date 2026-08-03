from django import forms
from .models import Postulacion


class PostulacionForm(forms.ModelForm):
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
            'programa': forms.TextInput(attrs={
                'class': 'form-control',
                'placeholder': 'Tu programa académico'
            }),
        }
        labels = {
            'motivacion': 'Motivación',
            'habilidades': 'Habilidades',
            'semestre': 'Semestre',
            'programa': 'Programa académico',
        }
