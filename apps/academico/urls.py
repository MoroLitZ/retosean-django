from django.urls import path
from . import views

app_name = 'academico'

urlpatterns = [
    # Estudiante
    path('explorar-retos/',  views.explorar_retos,    name='explorar_retos'),
    path('postulaciones/',   views.mis_postulaciones, name='mis_postulaciones'),
    path('certificados/',    views.certificados,      name='certificados'),
]
