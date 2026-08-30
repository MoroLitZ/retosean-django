from django.urls import path
from django.contrib.auth import views as auth_views
from . import views

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

    # Recuperar contraseña (flujo sin sesión iniciada)
    path('recuperar-contrasena/',
         auth_views.PasswordResetView.as_view(
             template_name='usuarios/password_reset.html',
             email_template_name='usuarios/password_reset_email.html',
             subject_template_name='usuarios/password_reset_subject.txt',
             success_url='/usuarios/recuperar-contrasena/enviado/',
         ),
         name='password_reset'),
    path('recuperar-contrasena/enviado/',
         auth_views.PasswordResetDoneView.as_view(template_name='usuarios/password_reset_done.html'),
         name='password_reset_done'),
    path('recuperar-contrasena/<uidb64>/<token>/',
         auth_views.PasswordResetConfirmView.as_view(
             template_name='usuarios/password_reset_confirm.html',
             success_url='/usuarios/recuperar-contrasena/completo/',
         ),
         name='password_reset_confirm'),
    path('recuperar-contrasena/completo/',
         auth_views.PasswordResetCompleteView.as_view(template_name='usuarios/password_reset_complete.html'),
         name='password_reset_complete'),

    # Dashboards por rol
    path('admin/dashboard/',      views.dashboard_admin,      name='admin_dashboard'),
    path('empresa/dashboard/',    views.dashboard_empresa,    name='empresa_dashboard'),
    path('profesor/dashboard/',   views.dashboard_profesor,   name='profesor_dashboard'),
    path('estudiante/dashboard/', views.dashboard_estudiante, name='estudiante_dashboard'),

    # Admin
    path('admin/usuarios/',  views.lista_usuarios,    name='lista_usuarios'),
    path('admin/usuarios/<int:pk>/rol/',    views.cambiar_rol_usuario,     name='cambiar_rol'),
    path('admin/usuarios/<int:pk>/estado/', views.alternar_estado_usuario, name='alternar_estado'),
    path('admin/usuarios/importar/',        views.importar_usuarios,       name='importar_usuarios'),
    path('admin/usuarios/plantilla/',       views.plantilla_importacion,   name='plantilla_importacion'),
    path('admin/actividad/', views.log_actividad,     name='log_actividad'),
    path('admin/reportes/',  views.reportes_admin,    name='reportes_admin'),
]