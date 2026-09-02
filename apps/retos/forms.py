from django import forms

from .models import Reto, EquipoRetoAcademico, SesionRetoAcademico
from apps.seguimiento.models import IntegracionAcademica, SeguimientoReto
from apps.unidades_estudio.models import UnidadEstudio
from apps.usuarios.models import Usuario

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
            'facultad',
            'programa',
            'ecosistema',
            'fecha_inicio_tentativa',
            'fecha_fin_tentativa',
            'fecha_limite_postulacion',
            'premios',
            'criterios_evaluacion',
            'tiene_convenio_institucional',
            'archivos',
        ]
        widgets = {
            'fecha_inicio_tentativa': forms.DateInput(attrs={'type': 'date'}),
            'fecha_fin_tentativa': forms.DateInput(attrs={'type': 'date'}),
            'fecha_limite_postulacion': forms.DateInput(attrs={'type': 'date'}),
            'descripcion': forms.Textarea(attrs={'rows': 4}),
            'premios': forms.Textarea(attrs={'rows': 3}),
            'criterios_evaluacion': forms.Textarea(attrs={'rows': 4}),
            'tiene_convenio_institucional': forms.CheckboxInput(attrs={'class': 'form-check-input'}),
        }

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        from apps.academico.models import Facultad, Programa, Ecosistema
        if 'facultad' in self.fields:
            self.fields['facultad'].queryset = Facultad.objects.all().order_by('nombre')
        if 'programa' in self.fields:
            self.fields['programa'].queryset = Programa.objects.filter(
                facultad_id=self.instance.facultad_id
            ).order_by('nombre') if self.instance and self.instance.facultad_id else Programa.objects.all().order_by('nombre')
        if 'ecosistema' in self.fields:
            self.fields['ecosistema'].queryset = Ecosistema.objects.all().order_by('nombre')


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
    NIVEL_FORMACION_CHOICES = [('', 'Selecciona un nivel')] + list(Reto.NIVEL_ACADEMICO_CHOICES)

    facultad = forms.ChoiceField(
        choices=[('', 'Selecciona una facultad')],
        required=False,
        widget=forms.Select(attrs={'class': 'form-select'}),
    )
    nivel_formacion = forms.ChoiceField(
        choices=NIVEL_FORMACION_CHOICES,
        required=False,
        widget=forms.Select(attrs={'class': 'form-select'}),
    )
    ecosistema = forms.ChoiceField(
        choices=[('', 'Selecciona un ecosistema')],
        required=False,
        widget=forms.Select(attrs={'class': 'form-select'}),
    )

    unidad_estudio = forms.ModelChoiceField(
        queryset=UnidadEstudio.objects.all().order_by('nombre'),
        label='Unidad de Estudio',
        required=False, # Ponlo en False por si acaso para evitar bloqueos si recarga
        widget=forms.Select(attrs={'class': 'form-select'})
    )
    
    class Meta:
        model = IntegracionAcademica
        fields = [
            'reto',
            'facultad',
            'nivel_formacion',
            'unidad_estudio',
            'ecosistema',
            'descripcion',
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
            'descripcion': forms.Textarea(attrs={'rows': 4}),
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
        from apps.academico.models import Ecosistema, Facultad

        self.fields['reto'].queryset = Reto.objects.filter(
            estado__in=['aprobado', 'en_curso', 'pausado']
        ).order_by('titulo')

        facultades = list(Facultad.objects.order_by('nombre').values_list('nombre', flat=True))
        self.fields['facultad'].choices = [('', 'Selecciona una facultad')] + [
            (nombre, nombre) for nombre in facultades
        ]

        ecosistemas = list(Ecosistema.objects.order_by('nombre').values_list('nombre', flat=True))
        self.fields['ecosistema'].choices = [('', 'Selecciona un ecosistema')] + [
            (nombre, nombre) for nombre in ecosistemas
        ]

        # Compatibilidad hacia atras: si la integracion guarda un valor legacy
        # que ya no existe en catalogo, lo mantenemos seleccionable al editar.
        if self.instance and self.instance.pk:
            facultad_actual = (self.instance.facultad or '').strip()
            nivel_actual = (self.instance.nivel_formacion or '').strip()
            ecosistema_actual = (self.instance.ecosistema or '').strip()

            if facultad_actual and facultad_actual not in dict(self.fields['facultad'].choices):
                self.fields['facultad'].choices.append((facultad_actual, f'{facultad_actual} (actual)'))

            if nivel_actual and nivel_actual not in dict(self.fields['nivel_formacion'].choices):
                self.fields['nivel_formacion'].choices.append((nivel_actual, f'{nivel_actual} (actual)'))

            if ecosistema_actual and ecosistema_actual not in dict(self.fields['ecosistema'].choices):
                self.fields['ecosistema'].choices.append((ecosistema_actual, f'{ecosistema_actual} (actual)'))

        if 'unidad_estudio' in self.fields and hasattr(UnidadEstudio, 'objects'):
            facultad_seleccionada = (self.data.get('facultad') or '').strip()
            if not facultad_seleccionada and self.instance and self.instance.pk:
                facultad_seleccionada = (self.instance.facultad or '').strip()

            unidades = UnidadEstudio.objects.select_related('programa', 'programa__facultad').filter(activo=True)
            if facultad_seleccionada:
                unidades = unidades.filter(programa__facultad__nombre=facultad_seleccionada)
            self.fields['unidad_estudio'].queryset = unidades.order_by('programa__nombre', 'ciclo', 'nombre')
            self.fields['unidad_estudio'].empty_label = 'Selecciona una unidad de estudio'
            self.fields['unidad_estudio'].label_from_instance = (
                lambda unidad: f"{unidad.codigo} - {unidad.nombre} ({unidad.programa.nombre})"
            )

        # AQUÍ ESTÁ LA CLAVE: Si la instancia ya tiene guardado un programa_academico, 
        # intentamos buscar la UnidadEstudio correspondiente para preseleccionarla si el formulario se recarga.
        if self.instance and self.instance.pk and self.instance.programa_academico:
            unidad_coincidente = UnidadEstudio.objects.filter(nombre=self.instance.programa_academico).first()
            if unidad_coincidente:
                self.fields['unidad_estudio'].initial = unidad_coincidente
         
    def save(self, commit=True):
        instance = super().save(commit=False)
        unidad_seleccionada = self.cleaned_data.get('unidad_estudio')
        
        if unidad_seleccionada:
            instance.programa_academico = str(unidad_seleccionada.nombre)
            
        if commit:
            instance.save()
        return instance

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

class EquipoRetoAcademicoForm(BootstrapModelForm):
    profesores = forms.ModelMultipleChoiceField(
        queryset=Usuario.objects.filter(rol='PROFESOR').order_by('first_name', 'last_name'),
        required=False,
        widget=forms.SelectMultiple(attrs={'class': 'form-select', 'size': '5'})
    )
    estudiantes = forms.ModelMultipleChoiceField(
        queryset=Usuario.objects.filter(rol='ESTUDIANTE').order_by('first_name', 'last_name'),
        required=False,
        widget=forms.SelectMultiple(attrs={'class': 'form-select', 'size': '6'})
    )
    expertos_invitados = forms.ModelMultipleChoiceField(
        queryset=Usuario.objects.filter(rol__in=['EMPRESA', 'EXPERTO']).order_by('first_name', 'last_name'),
        required=False,
        widget=forms.SelectMultiple(attrs={'class': 'form-select', 'size': '4'})
    )

    class Meta:
        model = EquipoRetoAcademico
        fields = ['reto', 'nombre_equipo', 'profesores', 'estudiantes', 'expertos_invitados']

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        # El reto lo fija la vista a partir de la integracion; se muestra de solo lectura.
        if 'reto' in self.fields:
            self.fields['reto'].disabled = True
        # Esto asegura que los nombres salgan limpios como "Nombre Apellido" en lugar de "usuario123 (ROL)"
        for field_name in ['profesores', 'estudiantes', 'expertos_invitados']:
            if field_name in self.fields:
                self.fields[field_name].label_from_instance = lambda obj: f"{obj.get_full_name() or obj.username}"


class SesionRetoAcademicoForm(BootstrapModelForm):
    class Meta:
        model = SesionRetoAcademico
        fields = ['reto', 'tipo', 'fecha_hora', 'enlace_reunion', 'observaciones']
        widgets = {
            'fecha_hora': forms.DateTimeInput(attrs={'type': 'datetime-local', 'class': 'form-control'}),
            'observaciones': forms.Textarea(attrs={'rows': 3, 'class': 'form-control'}),
        }

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        # El reto lo fija la vista a partir de la integracion; se muestra de solo lectura.
        if 'reto' in self.fields:
            self.fields['reto'].disabled = True 
