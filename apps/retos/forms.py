from django import forms

from .models import IntegracionAcademica, Reto, SeguimientoReto

MAX_UPLOAD_SIZE = 50 * 1024 * 1024
MAX_UPLOAD_SIZE_MB = 50


class MultipleFileInput(forms.ClearableFileInput):
    allow_multiple_selected = True


class MultipleFileField(forms.FileField):
    widget = MultipleFileInput

    def clean(self, data, initial=None):
        files = data or []
        if not isinstance(files, (list, tuple)):
            files = [files]
        cleaned_files = []
        errors = []
        for uploaded_file in files:
            if uploaded_file.size > MAX_UPLOAD_SIZE:
                errors.append(
                    forms.ValidationError(
                        '%(name)s supera el limite de %(max_size)s MB.',
                        params={'name': uploaded_file.name, 'max_size': MAX_UPLOAD_SIZE_MB},
                    )
                )
            else:
                cleaned_files.append(super().clean(uploaded_file, initial))
        if errors:
            raise forms.ValidationError(errors)
        return cleaned_files


class BootstrapModelForm(forms.ModelForm):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        for field in self.fields.values():
            css_class = 'form-select' if isinstance(field.widget, forms.Select) else 'form-control'
            if isinstance(field.widget, forms.CheckboxInput):
                css_class = 'form-check-input'
            field.widget.attrs.setdefault('class', css_class)


class RetoForm(BootstrapModelForm):
    archivos = MultipleFileField(
        required=False,
        label='Archivos de soporte',
        help_text=f'Puedes cargar uno o varios archivos. Tamano maximo por archivo: {MAX_UPLOAD_SIZE_MB} MB.',
        widget=MultipleFileInput(attrs={'multiple': True}),
    )

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
            'criterios_evaluacion',
            'archivos',
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
    archivos = MultipleFileField(
        required=False,
        label='Archivos de avance',
        help_text=f'Puedes cargar uno o varios archivos. Tamano maximo por archivo: {MAX_UPLOAD_SIZE_MB} MB.',
        widget=MultipleFileInput(attrs={'multiple': True}),
    )

    class Meta:
        model = SeguimientoReto
        fields = ['tipo_sesion', 'fecha_sesion', 'porcentaje_avance', 'avances', 'observaciones', 'acuerdos', 'archivos']
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
