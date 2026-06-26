from django import forms
from django.contrib.auth.forms import AuthenticationForm, UserCreationForm
from django.utils import timezone
from .models import Usuario, PerfilEstudiante, PerfilProfesor, PerfilEmpresa, DocumentoEmpresa

class LoginForm(AuthenticationForm):
    username = forms.CharField(label='Usuario o correo', widget=forms.TextInput(attrs={'class': 'form-control', 'placeholder': 'usuario'}))
    password = forms.CharField(label='Contraseña', widget=forms.PasswordInput(attrs={'class': 'form-control', 'placeholder': '••••••••'}))

class RegistroUsuarioForm(UserCreationForm):
    rol = forms.ChoiceField(choices=Usuario.ROL_CHOICES, widget=forms.Select(attrs={'class': 'form-select', 'id': 'select-rol'}))
    
    # Campos para estudiantes
    carrera = forms.CharField(max_length=100, required=False, widget=forms.TextInput(attrs={'class': 'form-control campo-perfil', 'id': 'id_carrera'}))
    semestre = forms.IntegerField(required=False, widget=forms.NumberInput(attrs={'class': 'form-control campo-perfil', 'id': 'id_semestre'}))
    
    # Campos profesores
    facultad = forms.CharField(max_length=100, required=False, widget=forms.TextInput(attrs={'class': 'form-control campo-perfil', 'id': 'id_facultad'}))
    especialidad = forms.CharField(max_length=100, required=False, widget=forms.TextInput(attrs={'class': 'form-control campo-perfil', 'id': 'id_especialidad'}))
    
    # Campos empresas
    nit = forms.CharField(max_length=20, required=False, widget=forms.TextInput(attrs={'class': 'form-control campo-perfil', 'id': 'id_nit'}))
    razon_social = forms.CharField(max_length=150, required=False, widget=forms.TextInput(attrs={'class': 'form-control campo-perfil', 'id': 'id_razon_social'}))
    sector_industrial = forms.CharField(max_length=100, required=False, widget=forms.TextInput(attrs={'class': 'form-control campo-perfil', 'id': 'id_sector_industrial'}))

    class Meta(UserCreationForm.Meta):
        model = Usuario
        fields = UserCreationForm.Meta.fields + ('first_name', 'last_name', 'email', 'rol')

    # Se guarda el usuario y se crea un perfil según el rol
    def save(self, commit=True):
        user = super().save(commit=False)
        user.rol = self.cleaned_data.get('rol')
        if commit:
            user.save()
            
            # Lógica de guardado del sub-perfil en la base de datos
            if user.rol == 'ESTUDIANTE':
                PerfilEstudiante.objects.create(
                    usuario=user,
                    carrera=self.cleaned_data.get('carrera'),
                    semestre=self.cleaned_data.get('semestre')
                )
            elif user.rol == 'PROFESOR':
                PerfilProfesor.objects.create(
                    usuario=user,
                    facultad=self.cleaned_data.get('facultad'),
                    especialidad=self.cleaned_data.get('especialidad')
                )
            elif user.rol == 'EMPRESA':
                PerfilEmpresa.objects.create(
                    usuario=user,
                    nit=self.cleaned_data.get('nit'),
                    razon_social=self.cleaned_data.get('razon_social'),
                    sector_industrial=self.cleaned_data.get('sector_industrial')
                )
        return user
    
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.fields['username'].help_text = "Máximo 150 caracteres. Solo letras, números y @/./+/-/_"
        # Aplicar clases Bootstrap a los campos heredados de UserCreationForm
        base_fields = ['username', 'first_name', 'last_name', 'email', 'password1', 'password2']
        for name in base_fields:
            if name in self.fields:
                self.fields[name].widget.attrs.setdefault('class', 'form-control')
        self.fields['first_name'].label = 'Nombre'
        self.fields['last_name'].label  = 'Apellido'
        self.fields['email'].label      = 'Correo electrónico'
        self.fields['password1'].label  = 'Contraseña'
        self.fields['password2'].label  = 'Confirmar contraseña'


class RegistroAcademicoForm(UserCreationForm):
    rol = forms.ChoiceField(
        choices=[('ESTUDIANTE', 'Estudiante'), ('PROFESOR', 'Profesor')],
        widget=forms.Select(attrs={'class': 'form-select', 'id': 'select-rol'})
    )
    carrera      = forms.CharField(max_length=100, required=False, widget=forms.TextInput(attrs={'class': 'form-control'}))
    semestre     = forms.IntegerField(required=False, widget=forms.NumberInput(attrs={'class': 'form-control'}))
    facultad     = forms.CharField(max_length=100, required=False, widget=forms.TextInput(attrs={'class': 'form-control'}))
    especialidad = forms.CharField(max_length=100, required=False, widget=forms.TextInput(attrs={'class': 'form-control'}))

    class Meta(UserCreationForm.Meta):
        model = Usuario
        fields = UserCreationForm.Meta.fields + ('first_name', 'last_name', 'email', 'rol')

    def clean_email(self):
        email = self.cleaned_data.get('email', '')
        if not email.endswith('@universidadean.edu.co'):
            raise forms.ValidationError("Usa tu correo institucional @universidadean.edu.co")
        return email

    def save(self, commit=True):
        user = super().save(commit=False)
        user.rol = self.cleaned_data['rol']
        if commit:
            user.save()
            if user.rol == 'ESTUDIANTE':
                PerfilEstudiante.objects.create(
                    usuario=user,
                    carrera=self.cleaned_data.get('carrera', ''),
                    semestre=self.cleaned_data.get('semestre') or 1,
                )
            elif user.rol == 'PROFESOR':
                PerfilProfesor.objects.create(
                    usuario=user,
                    facultad=self.cleaned_data.get('facultad', ''),
                    especialidad=self.cleaned_data.get('especialidad', ''),
                )
        return user

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.fields['username'].help_text = "Solo letras, números y @/./+/-/_"
        self.fields['first_name'].label = 'Nombre'
        self.fields['last_name'].label  = 'Apellido'
        self.fields['email'].label      = 'Correo institucional'
        self.fields['password1'].label  = 'Contraseña'
        self.fields['password2'].label  = 'Confirmar contraseña'
        for name, field in self.fields.items():
            if name != 'rol':
                field.widget.attrs.setdefault('class', 'form-control')


class RegistroEmpresaForm(UserCreationForm):
    nit              = forms.CharField(max_length=20, label="NIT de la Empresa",  widget=forms.TextInput(attrs={'class': 'form-control'}))
    razon_social     = forms.CharField(max_length=150, label="Razón Social",       widget=forms.TextInput(attrs={'class': 'form-control'}))
    sector_industrial = forms.CharField(max_length=100, label="Sector Industrial", widget=forms.TextInput(attrs={'class': 'form-control'}))

    class Meta(UserCreationForm.Meta):
        model = Usuario
        fields = UserCreationForm.Meta.fields + ('email',)

    def save(self, commit=True):
        user = super().save(commit=False)
        user.rol = 'EMPRESA'
        user.first_name = self.cleaned_data['razon_social']
        if commit:
            user.save()
            PerfilEmpresa.objects.create(
                usuario=user,
                nit=self.cleaned_data['nit'],
                razon_social=self.cleaned_data['razon_social'],
                sector_industrial=self.cleaned_data['sector_industrial'],
            )
        return user

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.fields['username'].label     = 'Usuario Corporativo (ID)'
        self.fields['username'].help_text = "Identificador único en la plataforma (ej: pepsico_ean)"
        self.fields['email'].label        = 'Correo Corporativo'
        self.fields['password1'].label    = 'Contraseña'
        self.fields['password2'].label    = 'Confirmar contraseña'
        for field in self.fields.values():
            field.widget.attrs.setdefault('class', 'form-control')


class CargarDocumentoForm(forms.ModelForm):
    class Meta:
        model = DocumentoEmpresa
        fields = ['tipo_documento', 'archivo', 'fecha_expedicion']
        widgets = {
            'tipo_documento':   forms.Select(attrs={'class': 'form-select'}),
            'archivo':          forms.FileInput(attrs={'class': 'form-control', 'accept': '.pdf'}),
            'fecha_expedicion': forms.DateInput(attrs={'class': 'form-control', 'type': 'date'}),
        }

    def clean(self):
        cleaned_data = super().clean()
        tipo     = cleaned_data.get('tipo_documento')
        fecha    = cleaned_data.get('fecha_expedicion')
        archivo  = cleaned_data.get('archivo')

        if archivo and not fecha:
            self.add_error('fecha_expedicion', 'Indica la fecha de expedición del documento.')

        if tipo == 'CAMARA_COMERCIO' and fecha:
            dias = (timezone.now().date() - fecha).days
            if dias > 30:
                self.add_error('fecha_expedicion', f'La Cámara de Comercio tiene {dias} días de emitida. Máximo permitido: 30 días.')
            elif dias < 0:
                self.add_error('fecha_expedicion', 'La fecha de expedición no puede ser futura.')

        return cleaned_data