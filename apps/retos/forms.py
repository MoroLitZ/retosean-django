from django import forms

from .models import Reto
from apps.seguimiento.models import IntegracionAcademica, SeguimientoReto


class BootstrapModelForm(forms.ModelForm):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        for field in self.fields.values():
            css_class = 'form-select' if isinstance(field.widget, forms.Select) else 'form-control'
            if isinstance(field.widget, forms.CheckboxInput):
                css_class = 'form-check-input'
            field.widget.attrs.setdefault('class', css_class)


class RetoForm(BootstrapModelForm):
    class Meta:
        model = Reto
        fields = [
            'tipo',
            'titulo',
            'descripcion',
            'area',
            'nivel_academico',
            'fecha_inicio_tentativa',
            'fecha_fin_tentativa',
            'fecha_limite_postulacion',
            'premios',
            'documento_soporte',
            'criterios_evaluacion',
        ]
        widgets = {
            'fecha_inicio_tentativa': forms.DateInput(attrs={'type': 'date'}),
            'fecha_fin_tentativa': forms.DateInput(attrs={'type': 'date'}),
            'fecha_limite_postulacion': forms.DateInput(attrs={'type': 'date'}),
            'descripcion': forms.Textarea(attrs={'rows': 4}),
            'premios': forms.Textarea(attrs={'rows': 3}),
            'criterios_evaluacion': forms.Textarea(attrs={'rows': 4}),
        }


class RevisionRetoForm(forms.Form):
    accion = forms.ChoiceField(
        choices=[('aprobar', 'Aprobar'), ('rechazar', 'Rechazar')],
        widget=forms.Select(attrs={'class': 'form-select'}),
    )
    comentario = forms.CharField(
        required=False,
        label='Comentarios para la empresa',
        widget=forms.Textarea(attrs={'class': 'form-control', 'rows': 4}),
    )


class EstadoRetoForm(forms.Form):
    estado = forms.ChoiceField(
        choices=[
            ('aprobado', 'Aprobado'),
            ('en_curso', 'En curso'),
            ('pausado', 'Pausado'),
            ('finalizado', 'Finalizado'),
            ('cancelado', 'Cancelado'),
        ],
        widget=forms.Select(attrs={'class': 'form-select'}),
    )
    comentario = forms.CharField(
        required=False,
        widget=forms.Textarea(attrs={'class': 'form-control', 'rows': 3}),
    )


class SeguimientoRetoForm(BootstrapModelForm):
    class Meta:
        model = SeguimientoReto
        fields = ['tipo_sesion', 'fecha_sesion', 'porcentaje_avance', 'avances', 'observaciones', 'acuerdos']
        widgets = {
            'fecha_sesion': forms.DateInput(attrs={'type': 'date'}),
            'avances': forms.Textarea(attrs={'rows': 4}),
            'observaciones': forms.Textarea(attrs={'rows': 4}),
            'acuerdos': forms.Textarea(attrs={'rows': 4}),
        }

    def clean_porcentaje_avance(self):
        valor = self.cleaned_data['porcentaje_avance']
        if valor > 100:
            raise forms.ValidationError('El porcentaje de avance no puede superar 100.')
        return valor


class IntegracionAcademicaForm(BootstrapModelForm):
    class Meta:
        model = IntegracionAcademica
        fields = [
            'reto',
            'facultad',
            'nivel_formacion',
            'programa_academico',
            'ecosistema',
            'alcance',
            'entregable_esperado',
            'cronograma_sesiones',
            'equipo_profesores',
            'equipo_estudiantes',
            'expertos_invitados',
            'requerimientos_empresa',
            'requerimientos_internos',
            'espacio_fisico',
        ]
        widgets = {
            'alcance': forms.Textarea(attrs={'rows': 4}),
            'cronograma_sesiones': forms.Textarea(attrs={'rows': 4}),
            'equipo_profesores': forms.Textarea(attrs={'rows': 3}),
            'equipo_estudiantes': forms.Textarea(attrs={'rows': 3}),
            'expertos_invitados': forms.Textarea(attrs={'rows': 3}),
            'requerimientos_empresa': forms.Textarea(attrs={'rows': 3}),
            'requerimientos_internos': forms.Textarea(attrs={'rows': 3}),
        }

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.fields['reto'].queryset = Reto.objects.filter(
            estado__in=['aprobado', 'en_curso', 'pausado']
        ).order_by('titulo')


class RevisionIntegracionForm(forms.Form):
    accion = forms.ChoiceField(
        choices=[('aprobar', 'Aprobar'), ('rechazar', 'Rechazar')],
        widget=forms.Select(attrs={'class': 'form-select'}),
    )
    comentario = forms.CharField(
        required=False,
        label='Comentarios para el profesor',
        widget=forms.Textarea(attrs={'class': 'form-control', 'rows': 4}),
    )
