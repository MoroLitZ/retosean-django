from django import forms
from django.forms import inlineformset_factory

from apps.usuarios.models import Usuario

from .models import EtapaHackaton, Hackathon, Jurado, VotacionHackaton

CLASE_INPUT = "form-control form-control-sm"
CLASE_SELECT = "form-select form-select-sm"


class HackathonForm(forms.ModelForm):
    class Meta:
        model = Hackathon
        fields = ["reglas", "estado"]
        labels = {"reglas": "Reglas y bases del hackathon"}
        widgets = {
            "reglas": forms.Textarea(attrs={"class": CLASE_INPUT, "rows": 6}),
            "estado": forms.Select(attrs={"class": CLASE_SELECT}),
        }


class EtapaHackatonForm(forms.ModelForm):
    class Meta:
        model = EtapaHackaton
        fields = ["tipo", "titulo", "inicia_en", "termina_en", "orden"]
        widgets = {
            "tipo": forms.Select(attrs={"class": CLASE_SELECT}),
            "titulo": forms.TextInput(attrs={"class": CLASE_INPUT}),
            "inicia_en": forms.DateTimeInput(
                attrs={"class": CLASE_INPUT, "type": "datetime-local"}, format="%Y-%m-%dT%H:%M"
            ),
            "termina_en": forms.DateTimeInput(
                attrs={"class": CLASE_INPUT, "type": "datetime-local"}, format="%Y-%m-%dT%H:%M"
            ),
            "orden": forms.NumberInput(attrs={"class": CLASE_INPUT, "min": 0}),
        }

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        for campo in ("inicia_en", "termina_en"):
            self.fields[campo].input_formats = ["%Y-%m-%dT%H:%M", "%Y-%m-%d %H:%M:%S"]

    def clean(self):
        datos = super().clean()
        inicia, termina = datos.get("inicia_en"), datos.get("termina_en")
        if inicia and termina and termina < inicia:
            raise forms.ValidationError("La etapa no puede terminar antes de empezar.")
        return datos


EtapaFormSet = inlineformset_factory(
    Hackathon, EtapaHackaton, form=EtapaHackatonForm, extra=1, can_delete=True
)


class JuradoForm(forms.ModelForm):
    class Meta:
        model = Jurado
        fields = ["usuario", "nombre", "email", "especialidad"]
        labels = {"usuario": "Cuenta en la plataforma (opcional)"}
        widgets = {
            "usuario": forms.Select(attrs={"class": CLASE_SELECT}),
            "nombre": forms.TextInput(attrs={"class": CLASE_INPUT}),
            "email": forms.EmailInput(attrs={"class": CLASE_INPUT}),
            "especialidad": forms.TextInput(attrs={"class": CLASE_INPUT}),
        }

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        # Un jurado suele ser un docente o un representante de la empresa.
        self.fields["usuario"].queryset = Usuario.objects.filter(
            rol__in=["PROFESOR", "EMPRESA", "ADMIN"], is_active=True
        ).order_by("first_name", "username")
        self.fields["usuario"].required = False

    def clean(self):
        datos = super().clean()
        if not datos.get("nombre") and datos.get("usuario"):
            usuario = datos["usuario"]
            datos["nombre"] = usuario.get_full_name() or usuario.username
        if not datos.get("nombre"):
            raise forms.ValidationError("Indica el nombre del jurado o selecciona una cuenta.")
        return datos


class VotacionForm(forms.ModelForm):
    class Meta:
        model = VotacionHackaton
        fields = ["puntaje", "comentario"]
        labels = {"puntaje": "Puntaje (0 a 5)"}
        widgets = {
            "puntaje": forms.NumberInput(
                attrs={"class": CLASE_INPUT, "step": "0.25", "min": "0", "max": "5"}
            ),
            "comentario": forms.Textarea(attrs={"class": CLASE_INPUT, "rows": 3}),
        }


class InscripcionForm(forms.Form):
    """Inscripcion de un equipo del estudiante al hackathon."""

    equipo = forms.ModelChoiceField(
        label="Equipo", queryset=None,
        widget=forms.Select(attrs={"class": CLASE_SELECT}),
        empty_label="Selecciona tu equipo",
    )

    def __init__(self, *args, hackathon=None, usuario=None, **kwargs):
        super().__init__(*args, **kwargs)
        from apps.participaciones.models import Equipo

        ya_inscritos = hackathon.inscripciones.values_list("equipo_id", flat=True)
        # Solo equipos del mismo reto en los que el estudiante es miembro.
        self.fields["equipo"].queryset = Equipo.objects.filter(
            reto=hackathon.reto, miembros__estudiante=usuario
        ).exclude(pk__in=ya_inscritos).distinct()
