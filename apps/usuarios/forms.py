from django import forms
from django.contrib.auth.forms import AuthenticationForm, UserCreationForm
from .models import Usuario

class LoginForm(AuthenticationForm):
    username = forms.CharField(label='Usuario o correo', widget=forms.TextInput(attrs={'class': 'form-control', 'placeholder': 'usuario'}))
    password = forms.CharField(label='Contraseña', widget=forms.PasswordInput(attrs={'class': 'form-control', 'placeholder': '••••••••'}))

class RegistroUsuarioForm(UserCreationForm):
    class Meta:
        model = Usuario
        fields = ['first_name', 'last_name', 'email', 'username', 'rol']

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        # El registro público no permite crear administradores directamente
        self.fields['rol'].choices = [c for c in Usuario.ROL_CHOICES if c[0] != 'administrador']
        for field in self.fields.values():
            field.widget.attrs.update({'class': 'form-control'})