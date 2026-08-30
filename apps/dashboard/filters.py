"""Filtros de periodo compartidos por el dashboard (HU05) y los reportes (HU06).

Viven aqui y no en cada vista para que el dashboard y los reportes exportados
den siempre las mismas cifras para el mismo rango.
"""

from datetime import timedelta

from django.utils import timezone

PERIODOS = [
    ("30d", "Ultimos 30 dias"),
    ("90d", "Ultimos 90 dias"),
    ("anio", "Ultimo anio"),
    ("todo", "Historico completo"),
    ("custom", "Rango personalizado"),
]

_DIAS = {"30d": 30, "90d": 90, "anio": 365}


def _a_fecha(valor):
    if not valor:
        return None
    from django.utils.dateparse import parse_date

    return parse_date(valor)


def rango_de_fechas(request):
    """Devuelve (desde, hasta, etiqueta, periodo) a partir del querystring.

    Acepta ?periodo=30d|90d|anio|todo|custom y, para custom, ?desde= y ?hasta=
    en formato ISO. `desde`/`hasta` pueden ser None (sin acotar).
    """
    periodo = request.GET.get("periodo", "90d")
    hoy = timezone.localdate()

    if periodo == "todo":
        return None, None, "Historico completo", periodo

    if periodo == "custom":
        desde = _a_fecha(request.GET.get("desde"))
        hasta = _a_fecha(request.GET.get("hasta"))
        if desde and hasta and desde > hasta:
            desde, hasta = hasta, desde
        etiqueta = "Rango personalizado"
        if desde and hasta:
            etiqueta = f"{desde:%d/%m/%Y} - {hasta:%d/%m/%Y}"
        return desde, hasta, etiqueta, periodo

    dias = _DIAS.get(periodo)
    if dias is None:
        periodo, dias = "90d", 90
    desde = hoy - timedelta(days=dias)
    return desde, hoy, dict(PERIODOS)[periodo], periodo


def acotar(queryset, campo, desde, hasta):
    """Aplica el rango sobre un campo de fecha, ignorando los limites vacios."""
    if desde:
        queryset = queryset.filter(**{f"{campo}__date__gte": desde})
    if hasta:
        queryset = queryset.filter(**{f"{campo}__date__lte": hasta})
    return queryset
