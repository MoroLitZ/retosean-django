from django.urls import path
from . import views

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

    # Empresa
    path('empresa/publicar-reto/',  views.publicar_reto,              name='publicar_reto'),
    path('empresa/mis-retos/',      views.mis_retos_empresa,          name='mis_retos_empresa'),
    path('empresa/postulaciones/',  views.postulaciones_empresa,      name='postulaciones_empresa'),
    path('empresa/documentos/',     views.panel_documentos_empresa,   name='documentos_empresa'),
    path('empresa/indicadores/',    views.indicadores_empresa,        name='indicadores_empresa'),

    # Profesor
    path('profesor/cursos/',       views.mis_cursos,            name='mis_cursos'),
    path('profesor/retos/',        views.retos_vinculados,      name='retos_vinculados'),
    path('profesor/estudiantes/',  views.mis_estudiantes,       name='mis_estudiantes'),
    path('profesor/evaluaciones/', views.evaluaciones,          name='evaluaciones'),
    path('profesor/entregables/',  views.entregables_profesor,  name='entregables_profesor'),

    # Estudiante
    path('estudiante/explorar-retos/',  views.explorar_retos,    name='explorar_retos'),
    path('estudiante/postulaciones/',   views.mis_postulaciones, name='mis_postulaciones'),
    path('estudiante/certificados/',    views.certificados,      name='certificados'),
    path('estudiante/reto/<int:reto_id>/postular/', views.postular_a_reto, name='postular_a_reto'),
    path('estudiante/entregables/', views.mis_entregables, name='mis_entregables'),
    path('estudiante/reto/<int:reto_id>/entregables/', views.mis_entregables, name='mis_entregables'),
]