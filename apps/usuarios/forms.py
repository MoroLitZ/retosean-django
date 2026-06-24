from django import forms
from django.contrib.auth.forms import AuthenticationForm, UserCreationForm
from .models import Usuario, PerfilEstudiante, PerfilProfesor, PerfilEmpresa

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