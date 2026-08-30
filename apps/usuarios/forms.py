from django import forms
from django.contrib.auth.forms import AuthenticationForm, UserCreationForm
from django.contrib.auth import get_user_model
from django.utils import timezone
from .models import Usuario
from apps.academico.models import Estudiante, Profesor, Programa, Facultad
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


SEMESTRE_CHOICES = [(str(i), f"{i}") for i in range(1, 11)]

ESPECIALIDAD_CHOICES = [
    ("", "Seleccione una especialidad"),
    ("Ingeniería de Software", "Ingeniería de Software"),
    ("Sistemas de Información", "Sistemas de Información"),
    ("Inteligencia Artificial", "Inteligencia Artificial"),
    ("Gestión de Operaciones", "Gestión de Operaciones"),
    ("Finanzas Corporativas", "Finanzas Corporativas"),
    ("Marketing Digital", "Marketing Digital"),
    ("Innovación y Emprendimiento", "Innovación y Emprendimiento"),
    ("Sostenibilidad y Ambiente", "Sostenibilidad y Ambiente"),
    ("Diseño y Creatividad", "Diseño y Creatividad"),
    ("Otra", "Otra"),
]


class RegistroAcademicoForm(UserCreationForm):
    rol = forms.ChoiceField(
        choices=[("ESTUDIANTE", "Estudiante"), ("PROFESOR", "Profesor")],
        widget=forms.Select(attrs={"class": "form-select", "id": "select-rol"})
    )
    carrera = forms.ModelChoiceField(
        queryset=Programa.objects.select_related("facultad").all(),
        required=False,
        empty_label="Seleccione un programa",
        widget=forms.Select(attrs={"class": "form-select"}),
    )
    semestre = forms.ChoiceField(
        choices=SEMESTRE_CHOICES,
        required=False,
        widget=forms.Select(attrs={"class": "form-select"}),
    )
    facultad = forms.ModelChoiceField(
        queryset=Facultad.objects.all(),
        required=False,
        empty_label="Seleccione una facultad",
        widget=forms.Select(attrs={"class": "form-select"}),
    )
    especialidad = forms.ChoiceField(
        choices=ESPECIALIDAD_CHOICES,
        required=False,
        widget=forms.Select(attrs={"class": "form-select"}),
    )

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
                programa = self.cleaned_data.get("carrera")
                semestre = self.cleaned_data.get("semestre") or 1
                try:
                    semestre_int = int(semestre)
                except (TypeError, ValueError):
                    semestre_int = 1
                Estudiante.objects.create(
                    usuario=user,
                    programa=programa,
                    semestre=semestre_int,
                )
            elif user.rol == "PROFESOR":
                facultad = self.cleaned_data.get("facultad")
                Profesor.objects.create(
                    usuario=user,
                    facultad=facultad,
                    especialidad=self.cleaned_data.get("especialidad") or "",
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
        fields = ["titulo", "es_final", "archivo", "comentario_estudiante"]
        widgets = {
            "titulo": forms.TextInput(attrs={"class": "form-control"}),
            "es_final": forms.CheckboxInput(attrs={"class": "form-check-input"}),
            "archivo": forms.FileInput(attrs={"class": "form-control", "accept": ".pdf,.zip,.rar,.docx"}),
            "comentario_estudiante": forms.Textarea(attrs={"class": "form-control", "rows": 3}),
        }
        labels = {
            "titulo": "Nombre del entregable",
            "es_final": "Este es el entregable final",
            "archivo": "Selecciona tu archivo de evidencia",
            "comentario_estudiante": "Comentarios adicionales",
        }


class EditarPerfilForm(forms.ModelForm):
    """Formulario para editar datos basicos del perfil del usuario."""
    primera_carrera = forms.ModelChoiceField(
        queryset=Programa.objects.select_related("facultad").all(),
        required=False,
        empty_label="Seleccione un programa",
        label="Carrera / Programa",
        widget=forms.Select(attrs={"class": "form-select"}),
    )
    semestre = forms.ChoiceField(
        choices=SEMESTRE_CHOICES,
        required=False,
        label="Semestre",
        widget=forms.Select(attrs={"class": "form-select"}),
    )
    facultad = forms.ModelChoiceField(
        queryset=Facultad.objects.all(),
        required=False,
        empty_label="Seleccione una facultad",
        label="Facultad",
        widget=forms.Select(attrs={"class": "form-select"}),
    )
    especialidad = forms.ChoiceField(
        choices=ESPECIALIDAD_CHOICES,
        required=False,
        label="Especialidad",
        widget=forms.Select(attrs={"class": "form-select"}),
    )
    razon_social = forms.CharField(
        required=False,
        label="Razón Social",
        widget=forms.TextInput(attrs={"class": "form-control"}),
    )
    sector_industrial = forms.CharField(
        required=False,
        label="Sector Industrial",
        widget=forms.TextInput(attrs={"class": "form-control"}),
    )

    class Meta:
        model = Usuario
        fields = ["first_name", "last_name", "email", "telefono"]
        widgets = {
            "first_name": forms.TextInput(attrs={"class": "form-control"}),
            "last_name": forms.TextInput(attrs={"class": "form-control"}),
            "email": forms.EmailInput(attrs={"class": "form-control"}),
            "telefono": forms.TextInput(attrs={"class": "form-control"}),
        }
        labels = {
            "first_name": "Nombre",
            "last_name": "Apellido",
            "email": "Correo electrónico",
            "telefono": "Teléfono",
        }

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        # Cargar valores iniciales desde los perfiles por rol
        user = kwargs.get("instance")
        if user:
            perfil_est = getattr(user, "perfil_estudiante", None)
            perfil_prof = getattr(user, "perfil_profesor", None)
            empresa = getattr(user, "empresa_perfil", None)
            if perfil_est:
                if perfil_est.programa:
                    self.fields["primera_carrera"].initial = perfil_est.programa
                self.fields["semestre"].initial = str(perfil_est.semestre or "")
            if perfil_prof:
                if perfil_prof.facultad:
                    self.fields["facultad"].initial = perfil_prof.facultad
                self.fields["especialidad"].initial = perfil_prof.especialidad or ""
            if empresa:
                self.fields["razon_social"].initial = empresa.razon_social
                self.fields["sector_industrial"].initial = empresa.sector_industrial

    def clean_email(self):
        email = self.cleaned_data.get("email")
        if self.instance and self.instance.rol in ("ESTUDIANTE", "PROFESOR") and email:
            if not email.endswith("@universidadean.edu.co"):
                raise forms.ValidationError("Estudiantes y profesores deben usar su correo institucional (@universidadean.edu.co).")
        return email

    def save(self, commit=True):
        user = super().save(commit=False)
        if commit:
            user.save()
            # Guardar datos específicos del perfil
            if user.rol == "ESTUDIANTE":
                perfil = getattr(user, "perfil_estudiante", None)
                if not perfil:
                    perfil = Estudiante.objects.create(usuario=user)
                programa = self.cleaned_data.get("primera_carrera")
                if programa:
                    perfil.programa = programa
                semestre = self.cleaned_data.get("semestre") or 1
                try:
                    perfil.semestre = int(semestre)
                except (TypeError, ValueError):
                    perfil.semestre = 1
                perfil.save()
            elif user.rol == "PROFESOR":
                perfil = getattr(user, "perfil_profesor", None)
                if not perfil:
                    perfil = Profesor.objects.create(usuario=user)
                facultad = self.cleaned_data.get("facultad")
                if facultad:
                    perfil.facultad = facultad
                perfil.especialidad = self.cleaned_data.get("especialidad") or ""
                perfil.save()
            elif user.rol == "EMPRESA":
                empresa = getattr(user, "empresa_perfil", None)
                if empresa:
                    empresa.razon_social = self.cleaned_data.get("razon_social") or empresa.razon_social
                    empresa.sector_industrial = self.cleaned_data.get("sector_industrial") or empresa.sector_industrial
                    empresa.save()
        return user
