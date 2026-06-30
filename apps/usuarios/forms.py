from django import forms
from django.contrib.auth.forms import AuthenticationForm, UserCreationForm
from .models import Usuario, PerfilEstudiante, PerfilProfesor, Empresa, DocumentoEmpresa, Entregable
from django.utils import timezone
from django.contrib.auth import get_user_model

class LoginForm(AuthenticationForm):
    username = forms.CharField(label='Usuario o correo', widget=forms.TextInput(attrs={'class': 'form-control', 'placeholder': 'usuario'}))
    password = forms.CharField(label='Contraseña', widget=forms.PasswordInput(attrs={'class': 'form-control', 'placeholder': '••••••••'}))
    
    # tomamos el username del registro, y al pasarlo en el login, no importa si estuvo en mayusculas o en minusculas
    def clean_username(self):
        username = self.cleaned_data.get('username')
        if username:
            User = get_user_model()
            try:
                user_obj = User.objects.get(username__iexact=username)
                return user_obj.username  
            except User.DoesNotExist:
                return username  
        return username

class RegistroAcademicoForm(UserCreationForm):
    rol = forms.ChoiceField(
        choices=[('ESTUDIANTE', 'Estudiante'), ('PROFESOR', 'Profesor')], 
        widget=forms.Select(attrs={'class': 'form-select', 'id': 'select-rol'})
    )
    
    carrera = forms.CharField(max_length=100, required=False, widget=forms.TextInput(attrs={'class': 'form-control'}))
    semestre = forms.IntegerField(required=False, widget=forms.NumberInput(attrs={'class': 'form-control'}))
    facultad = forms.CharField(max_length=100, required=False, widget=forms.TextInput(attrs={'class': 'form-control'}))
    especialidad = forms.CharField(max_length=100, required=False, widget=forms.TextInput(attrs={'class': 'form-control'}))

    class Meta(UserCreationForm.Meta):
        model = Usuario
        fields = UserCreationForm.Meta.fields + ('first_name', 'last_name', 'email', 'rol')

    def clean_email(self):
        email = self.cleaned_data.get('email')
        # dominio de la universidad ean
        if not email or not email.endswith('@universidadean.edu.co'):
            raise forms.ValidationError(
                "Estudiantes y profesores deben registrarse obligatoriamente con su correo institucional."
            )
        return email

    def save(self, commit=True):
        user = super().save(commit=False)
        user.rol = self.cleaned_data.get('rol')
        
        if commit:
            user.save()
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
        return user
    
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.fields['username'].help_text = "Máximo 150 caracteres. Solo letras, números y @/./+/-/_"
        base_fields = ['username', 'first_name', 'last_name', 'email', 'password1', 'password2']
        for name in base_fields:
            if name in self.fields:
                self.fields[name].widget.attrs.setdefault('class', 'form-control')
                self.fields['first_name'].label = 'Nombre'
                self.fields['last_name'].label  = 'Apellido'
                self.fields['email'].label      = 'Correo electrónico'
                self.fields['password1'].label  = 'Contraseña'
                self.fields['password2'].label  = 'Confirmar contraseña'


class RegistroEmpresaForm(UserCreationForm):
    nit = forms.CharField(max_length=20, label="NIT de la Empresa", widget=forms.TextInput(attrs={'class': 'form-control'}))
    razon_social = forms.CharField(max_length=150, label="Razón Social", widget=forms.TextInput(attrs={'class': 'form-control'}))
    sector_industrial = forms.CharField(max_length=100, label="Sector Industrial", widget=forms.TextInput(attrs={'class': 'form-control'}))

    class Meta(UserCreationForm.Meta):
        model = Usuario
        fields = UserCreationForm.Meta.fields + ('username', 'email')

    def save(self, commit=True):
        user = super().save(commit=False)
        user.rol = 'EMPRESA'
        user.es_admin_empresa = True
        user.first_name = self.cleaned_data.get('razon_social')

        if commit:
            nueva_empresa = Empresa.objects.create(
                nit=self.cleaned_data.get('nit'),
                razon_social=self.cleaned_data.get('razon_social'),
                sector_industrial=self.cleaned_data.get('sector_industrial')
            )
            
            user.empresa = nueva_empresa
            user.save()
            
        return user
    
    def clean_nit(self):
        nit = self.cleaned_data.get('nit')
        
        # Validamos contra el modelo Empresa directamente
        if Empresa.objects.filter(nit=nit).exists():
            raise forms.ValidationError("Este NIT ya se encuentra registrado para otra empresa.")
        return nit

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.fields['username'].help_text = "Identificador único de la empresa en la plataforma (Ej: pepsico_ean)"
        self.fields['username'].label = "Usuario Corporativo (ID)"
        self.fields['email'].label = "Correo Electrónico Corporativo"
        self.fields['password1'].label = 'Contraseña'
        self.fields['password2'].label = 'Confirmar contraseña'
        
        for field in self.fields.values():
            field.widget.attrs.update({'class': 'form-control'})


class CargarDocumentoForm(forms.ModelForm):
    class Meta:
        model = DocumentoEmpresa
        fields = ['tipo_documento', 'archivo', 'fecha_expedicion']
        widgets = {
            'tipo_documento': forms.Select(attrs={'class': 'form-control'}),
            'archivo': forms.FileInput(attrs={'class': 'form-control', 'accept': '.pdf'}),
            'fecha_expedicion': forms.DateInput(attrs={'class': 'form-control', 'type': 'date'}),
        }

    # validación de vigencia de 30 dias
    def clean(self):
        cleaned_data = super().clean()
        tipo = cleaned_data.get('tipo_documento')
        fecha_exp = cleaned_data.get('fecha_expedicion')
        archivo = cleaned_data.get('archivo')

        # Si el usuario intenta enviar el formulario sin seleccionar archivo o fecha
        if archivo and not fecha_exp:
            self.add_error('fecha_expedicion', 'Debes indicar la fecha de expedición de este documento.')

        # Regla estricta para la Cámara de Comercio
        if tipo == 'CAMARA_COMERCIO' and fecha_exp:
            hoy = timezone.now().date()
            dias_diferencia = (hoy - fecha_exp).days

            if dias_diferencia > 30:
                self.add_error('fecha_expedicion', f'La Cámara de Comercio está vencida. Tiene {dias_diferencia} días de emitida y el máximo permitido es de 30 días.')
            elif dias_diferencia < 0:
                self.add_error('fecha_expedicion', 'La fecha de expedición no puede ser una fecha futura.')

        return cleaned_data
    

class EntregableForm(forms.ModelForm):
    class Meta:
        model = Entregable
        fields = ['archivo', 'comentario_estudiante']
        widgets = {
            'archivo': forms.FileInput(attrs={
                'class': 'form-control',
                'accept': '.pdf,.zip,.rar,.docx'
            }),
            'comentario_estudiante': forms.Textarea(attrs={
                'class': 'form-control', 
                'rows': 3, 
                'placeholder': 'Escribe aquí observaciones o comentarios sobre tu entrega (opcional)...'
            }),
        }
        labels = {
            'archivo': 'Selecciona tu archivo de evidencia',
            'comentario_estudiante': 'Comentarios adicionales',
        }