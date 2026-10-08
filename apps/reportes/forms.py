from django import forms

from apps.academico.models import Facultad, Programa
from apps.empresas.models import Empresa
from apps.dashboard.filters import PERIODOS
from apps.participaciones.models import Postulacion
from apps.retos.models import Reto

from .catalogo import disponibles_para, obtener

CLASE_CONTROL = "form-select form-select-sm"
CLASE_INPUT = "form-control form-control-sm"


class ReporteForm(forms.Form):
    """Constructor de reportes: que reporte, que columnas y con que filtros."""

    reporte = forms.ChoiceField(
        label="Reporte", widget=forms.Select(attrs={"class": CLASE_CONTROL})
    )
    columnas = forms.MultipleChoiceField(
        label="Variables a incluir",
        required=False,
        widget=forms.CheckboxSelectMultiple,
        help_text="Si no eliges ninguna se incluyen todas.",
    )
    formato = forms.ChoiceField(
        label="Formato",
        choices=[("PDF", "PDF"), ("EXCEL", "Excel")],
        initial="PDF",
        widget=forms.Select(attrs={"class": CLASE_CONTROL}),
    )

    periodo = forms.ChoiceField(
        label="Periodo", choices=PERIODOS, initial="90d", required=False,
        widget=forms.Select(attrs={"class": CLASE_CONTROL}),
    )
    desde = forms.DateField(
        label="Desde", required=False,
        widget=forms.DateInput(attrs={"type": "date", "class": CLASE_INPUT}),
    )
    hasta = forms.DateField(
        label="Hasta", required=False,
        widget=forms.DateInput(attrs={"type": "date", "class": CLASE_INPUT}),
    )

    facultad = forms.ModelChoiceField(
        label="Facultad", queryset=Facultad.objects.order_by("nombre"),
        required=False, empty_label="Todas",
        widget=forms.Select(attrs={"class": CLASE_CONTROL}),
    )
    programa = forms.ModelChoiceField(
        label="Programa", queryset=Programa.objects.order_by("nombre"),
        required=False, empty_label="Todos",
        widget=forms.Select(attrs={"class": CLASE_CONTROL}),
    )
    empresa = forms.ModelChoiceField(
        label="Empresa", queryset=Empresa.objects.none(),
        required=False, empty_label="Todas",
        widget=forms.Select(attrs={"class": CLASE_CONTROL}),
    )
    estado = forms.ChoiceField(
        label="Estado del reto", required=False,
        choices=[("", "Todos")] + list(Reto.ESTADO_CHOICES),
        widget=forms.Select(attrs={"class": CLASE_CONTROL}),
    )
    estado_postulacion = forms.ChoiceField(
        label="Estado de postulacion", required=False,
        choices=[("", "Todos")] + list(Postulacion.ESTADOS),
        widget=forms.Select(attrs={"class": CLASE_CONTROL}),
    )
    estado_empresa = forms.ChoiceField(
        label="Estado de la empresa", required=False,
        choices=[("", "Todos")] + list(Empresa.ESTADO_VALIDACION),
        widget=forms.Select(attrs={"class": CLASE_CONTROL}),
    )

    def __init__(self, *args, usuario=None, **kwargs):
        super().__init__(*args, **kwargs)
        definiciones = disponibles_para(usuario) if usuario else []
        self.fields["reporte"].choices = [(d.clave, d.nombre) for d in definiciones]

        # Las empresas se resuelven aqui para no evaluar el queryset al importar.
        self.fields["empresa"].queryset = Empresa.objects.order_by("razon_social")

        # Las columnas dependen del reporte elegido; con POST ya lo sabemos.
        clave = (self.data.get("reporte") or self.initial.get("reporte")
                 or (definiciones[0].clave if definiciones else ""))
        definicion = obtener(clave)
        if definicion:
            self.fields["columnas"].choices = definicion.opciones_de_columnas()
            self.definicion_actual = definicion
        else:
            self.fields["columnas"].choices = []
            self.definicion_actual = None

    def clean(self):
        datos = super().clean()
        periodo = datos.get("periodo") or "90d"

        if periodo != "custom":
            from datetime import timedelta

            from django.utils import timezone

            hoy = timezone.localdate()
            dias = {"30d": 30, "90d": 90, "anio": 365}.get(periodo)
            datos["desde"] = hoy - timedelta(days=dias) if dias else None
            datos["hasta"] = hoy if dias else None
        elif datos.get("desde") and datos.get("hasta") and datos["desde"] > datos["hasta"]:
            datos["desde"], datos["hasta"] = datos["hasta"], datos["desde"]

        return datos

    def filtros(self):
        """Diccionario de filtros listo para el catalogo."""
        datos = self.cleaned_data
        return {
            "desde": datos.get("desde"),
            "hasta": datos.get("hasta"),
            "facultad": datos["facultad"].pk if datos.get("facultad") else None,
            "programa": datos["programa"].pk if datos.get("programa") else None,
            "empresa": datos["empresa"].usuario_id if datos.get("empresa") else None,
            "estado": datos.get("estado") or None,
            "estado_postulacion": datos.get("estado_postulacion") or None,
            "estado_empresa": datos.get("estado_empresa") or None,
        }
