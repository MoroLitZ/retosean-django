from django.urls import path
from . import views
from apps.empresas.views import admin_revisar_documentacion, procesar_aprobacion

app_name = 'usuarios'

urlpatterns = [
    path('login/',    views.vista_login,    name='login'),
    path('logout/',   views.vista_logout,   name='logout'),
    path('registro/', views.vista_registro, name='registro'),
    path('perfil/',   views.vista_perfil,   name='perfil'),

    # Dashboards por rol
    path('admin/dashboard/',      views.dashboard_admin,      name='admin_dashboard'),
    path('empresa/dashboard/',    views.dashboard_empresa,    name='empresa_dashboard'),
    path('profesor/dashboard/',   views.dashboard_profesor,   name='profesor_dashboard'),
    path('estudiante/dashboard/', views.dashboard_estudiante, name='estudiante_dashboard'),

    # Admin
    path('admin/usuarios/',  views.lista_usuarios,    name='lista_usuarios'),
    path('admin/empresas/',  views.lista_empresas,    name='lista_empresas'),
    path('admin/reportes/',  views.reportes_admin,    name='reportes_admin'),
]