from django.urls import path
from . import views

app_name = 'usuarios'

urlpatterns = [
    path('login/',    views.vista_login,    name='login'),
    path('logout/',   views.vista_logout,   name='logout'),
    path('registro/', views.vista_registro, name='registro'),
    path('perfil/',   views.vista_perfil,   name='perfil'),
    path('empresa/documentos/', views.panel_documentos_empresa, name='panel_documentos_empresa'),
]