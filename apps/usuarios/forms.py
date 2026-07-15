from django import forms
from django.contrib.auth.forms import AuthenticationForm, UserCreationForm
from django.contrib.auth import get_user_model
from django.utils import timezone
from .models import Usuario
from apps.academico.models import Estudiante, Profesor
from apps.empresas.models import Empresa
from apps.evaluacion.models import Entregable


class LoginForm(AuthenticationForm):
    username = forms.CharField(label="Usuario o correo", widget=forms.TextInput(attrs={"class": "form-control", "placeholder": "usuario"}))
    password = forms.CharField(label="Contrasena", widget=forms.PasswordInput(attrs={"class": "form-control"}))

    def clean_username(self):
        username = self.cleaned_data.get("username")
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
        choices=[("ESTUDIANTE", "Estudiante"), ("PROFESOR", "Profesor")],
        widget=forms.Select(attrs={"class": "form-select", "id": "select-rol"})
    )
    carrera = forms.CharField(max_length=100, required=False, widget=forms.TextInput(attrs={"class": "form-control"}))
    semestre = forms.IntegerField(required=False, widget=forms.NumberInput(attrs={"class": "form-control"}))
    facultad = forms.CharField(max_length=100, required=False, widget=forms.TextInput(attrs={"class": "form-control"}))
    especialidad = forms.CharField(max_length=100, required=False, widget=forms.TextInput(attrs={"class": "form-control"}))

    class Meta(UserCreationForm.Meta):
        model = Usuario
        fields = UserCreationForm.Meta.fields + ("first_name", "last_name", "email", "rol")

    def clean_email(self):
        email = self.cleaned_data.get("email")
        if not email or not email.endswith("@universidadean.edu.co"):
            raise forms.ValidationError("Estudiantes y profesores deben registrarse con su correo institucional.")
        return email

    def save(self, commit=True):
        user = super().save(commit=False)
        user.rol = self.cleaned_data.get("rol")
        if commit:
            user.save()
            if user.rol == "ESTUDIANTE":
                Estudiante.objects.create(
                    usuario=user,
                )
            elif user.rol == "PROFESOR":
                Profesor.objects.create(
                    usuario=user,
                )
        return user

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        for name in self.fields:
            self.fields[name].widget.attrs.setdefault("class", "form-control")
        self.fields["first_name"].label = "Nombre"
        self.fields["last_name"].label = "Apellido"
        self.fields["email"].label = "Correo electronico"
        self.fields["password1"].label = "Contrasena"
        self.fields["password2"].label = "Confirmar contrasena"


class RegistroEmpresaForm(UserCreationForm):
    nit = forms.CharField(max_length=20, label="NIT de la Empresa", widget=forms.TextInput(attrs={"class": "form-control"}))
    razon_social = forms.CharField(max_length=150, label="Razon Social", widget=forms.TextInput(attrs={"class": "form-control"}))
    sector_industrial = forms.CharField(max_length=100, label="Sector Industrial", widget=forms.TextInput(attrs={"class": "form-control"}))

    class Meta(UserCreationForm.Meta):
        model = Usuario
        fields = UserCreationForm.Meta.fields + ("username", "email")

    def save(self, commit=True):
        user = super().save(commit=False)
        user.rol = "EMPRESA"
        user.first_name = self.cleaned_data.get("razon_social")
        if commit:
            user.save()
            Empresa.objects.create(
                usuario=user,
                nit=self.cleaned_data.get("nit"),
                razon_social=self.cleaned_data.get("razon_social"),
                sector_industrial=self.cleaned_data.get("sector_industrial"),
            )
        return user

    def clean_nit(self):
        nit = self.cleaned_data.get("nit")
        if Empresa.objects.filter(nit=nit).exists():
            raise forms.ValidationError("Este NIT ya se encuentra registrado.")
        return nit

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        for field in self.fields.values():
            field.widget.attrs.update({"class": "form-control"})
        self.fields["username"].label = "Usuario Corporativo"
        self.fields["email"].label = "Correo Electronico Corporativo"
        self.fields["password1"].label = "Contrasena"
        self.fields["password2"].label = "Confirmar contrasena"


class EntregableForm(forms.ModelForm):
    class Meta:
        model = Entregable
        fields = ["archivo", "comentario_estudiante"]
        widgets = {
            "archivo": forms.FileInput(attrs={"class": "form-control", "accept": ".pdf,.zip,.rar,.docx"}),
            "comentario_estudiante": forms.Textarea(attrs={"class": "form-control", "rows": 3}),
        }
        labels = {
            "archivo": "Selecciona tu archivo de evidencia",
            "comentario_estudiante": "Comentarios adicionales",
        }
