from django.contrib.auth.decorators import login_required
from django.shortcuts import render

from apps.usuarios.roles import (
    ROL_ADMIN,
    ROL_EMPRESA,
    ROL_ESTUDIANTE,
    ROL_PROFESOR,
    rol_de,
)

from . import services
from .filters import PERIODOS, rango_de_fechas


@login_required(login_url='usuarios:login')
def panel(request):
    """Panel de indicadores. El contenido depende del rol de quien entra."""
    desde, hasta, etiqueta, periodo = rango_de_fechas(request)
    rol = rol_de(request.user)

    contexto = {
        'titulo': 'Dashboard de Indicadores',
        'periodo': periodo,
        'periodos': PERIODOS,
        'etiqueta_periodo': etiqueta,
        'desde': request.GET.get('desde', ''),
        'hasta': request.GET.get('hasta', ''),
    }

    if rol == ROL_ADMIN:
        facultad_id = request.GET.get('facultad') or None
        programa_id = request.GET.get('programa') or None
        contexto.update(services.opciones_de_filtro())
        contexto.update({
            'kpis': services.kpis_admin(desde, hasta, facultad_id, programa_id),
            'series': services.series_admin(desde, hasta, facultad_id, programa_id),
            'facultad_id': facultad_id,
            'programa_id': programa_id,
        })
        plantilla = 'dashboard/admin.html'
    elif rol == ROL_EMPRESA:
        contexto.update({
            'kpis': services.kpis_empresa(request.user, desde, hasta),
            'series': services.series_empresa(request.user, desde, hasta),
        })
        plantilla = 'dashboard/empresa.html'
    elif rol == ROL_PROFESOR:
        contexto.update({
            'kpis': services.kpis_profesor(request.user, desde, hasta),
            'series': services.series_profesor(request.user, desde, hasta),
        })
        plantilla = 'dashboard/profesor.html'
    elif rol == ROL_ESTUDIANTE:
        contexto.update({
            'kpis': services.kpis_estudiante(request.user, desde, hasta),
            'series': services.series_estudiante(request.user, desde, hasta),
        })
        plantilla = 'dashboard/estudiante.html'
    else:
        # Usuario autenticado sin rol (p. ej. creado por importacion masiva):
        # en vez de un panel vacio, se le explica que falta asignarle rol.
        contexto.update({'kpis': {}, 'series': {}, 'sin_rol': True})
        plantilla = 'dashboard/panel_base.html'

    return render(request, plantilla, contexto)
