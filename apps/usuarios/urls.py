from django.urls import path
from django.contrib.auth import views as auth_views
from . import views
from apps.empresas.views import admin_revisar_documentacion, procesar_aprobacion

app_name = 'usuarios'

urlpatterns = [
    path('login/',    views.vista_login,    name='login'),
    path('logout/',   views.vista_logout,   name='logout'),
    path('registro/', views.vista_registro, name='registro'),
    path('perfil/',   views.vista_perfil,   name='perfil'),
    path('perfil/editar/', views.vista_editar_perfil, name='editar_perfil'),

    # Cambio de contraseña
    path('perfil/cambiar-contrasena/',
         auth_views.PasswordChangeView.as_view(
             template_name='usuarios/cambiar_contrasena.html',
             success_url='/usuarios/perfil/cambiar-contrasena/hecho/'
         ),
         name='cambiar_contrasena'),
    path('perfil/cambiar-contrasena/hecho/',
         auth_views.PasswordChangeDoneView.as_view(template_name='usuarios/cambiar_contrasena_done.html'),
         name='cambiar_contrasena_done'),

    # Dashboards por rol
    path('admin/dashboard/',      views.dashboard_admin,      name='admin_dashboard'),
    path('empresa/dashboard/',    views.dashboard_empresa,    name='empresa_dashboard'),
    path('profesor/dashboard/',   views.dashboard_profesor,   name='profesor_dashboard'),
    path('estudiante/dashboard/', views.dashboard_estudiante, name='estudiante_dashboard'),

    # Admin
    path('admin/usuarios/',  views.lista_usuarios,    name='lista_usuarios'),
    path('admin/reportes/',  views.reportes_admin,    name='reportes_admin'),
]