from django.urls import path

from . import views

app_name = 'academico'

urlpatterns = [
    # Estudiante
    path('explorar-retos/',  views.explorar_retos,    name='explorar_retos'),
    path('postulaciones/',   views.mis_postulaciones, name='mis_postulaciones'),
    path('certificados/',    views.certificados,      name='certificados'),
    path('portafolio/',      views.portafolio,        name='portafolio'),
    path('certificados/<uuid:codigo>/pdf/', views.descargar_certificado, name='descargar_certificado'),
    # Verificacion publica: no requiere sesion.
    path('verificar/<uuid:codigo>/', views.verificar_certificado, name='verificar_certificado'),
]
