"""
URL configuration for config project.

The `urlpatterns` list routes URLs to views. For more information please see:
    https://docs.djangoproject.com/en/6.0/topics/http/urls/
Examples:
Function views
    1. Add an import:  from my_app import views
    2. Add a URL to urlpatterns:  path('', views.home, name='home')
Class-based views
    1. Add an import:  from other_app.views import Home
    2. Add a URL to urlpatterns:  path('', Home.as_view(), name='home')
Including another URLconf
    1. Import the include() function: from django.urls import include, path
    2. Add a URL to urlpatterns:  path('blog/', include('blog.urls'))
"""
from django.contrib import admin
from django.contrib.auth.decorators import login_required
from django.urls import path, include, re_path
from django.conf import settings
from django.views.generic import RedirectView
from django.views.static import serve

urlpatterns = [
    path('', RedirectView.as_view(pattern_name='usuarios:perfil', permanent=False), name='home'),
    # CFG-11: ruta configurable por entorno en lugar de "admin/" fijo.
    path(settings.ADMIN_URL, admin.site.urls),
    path('usuarios/', include('apps.usuarios.urls')),
    path('retos/', include('apps.retos.urls')),
    path('empresas/', include('apps.empresas.urls')),
    path('academico/', include('apps.academico.urls')),
    path('participaciones/', include('apps.participaciones.urls')),
    path('seguimiento/', include('apps.seguimiento.urls')),
    path('evaluacion/', include('apps.evaluacion.urls')),
    path('cierre/', include('apps.cierre.urls')),
    path('dashboard/', include('apps.dashboard.urls')),
    path('unidades-estudio/', include('apps.unidades_estudio.urls')),
    path('notificaciones/', include('apps.notificaciones.urls')),
    path('reportes/', include('apps.reportes.urls')),
    path('presupuesto/', include('apps.presupuesto.urls')),
    path('hackatones/', include('apps.hackaton.urls')),
]

# CFG-04: en DEBUG los /media/ tambien contienen archivos privados (p. ej.
# PDFs legales de empresas). Exigir sesion iniciada evita que un usuario
# anonimo los lea adivinando la URL. La autorizacion por empresa/objeto la
# aplican las vistas de la app correspondiente, no el servidor estatico.
media_en_desarrollo = login_required(serve)

if settings.DEBUG:
    urlpatterns += [
        re_path(
            r"^media/(?P<path>.*)$",
            media_en_desarrollo,
            {"document_root": settings.MEDIA_ROOT},
        ),
    ]

