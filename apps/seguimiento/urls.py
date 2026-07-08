from django.urls import path
from . import views

app_name = 'seguimiento'

urlpatterns = [
    # Profesor
    path('profesor/cursos/',       views.mis_cursos,            name='mis_cursos'),
    path('profesor/retos/',        views.retos_vinculados,      name='retos_vinculados'),
    path('profesor/estudiantes/',  views.mis_estudiantes,       name='mis_estudiantes'),
    path('profesor/evaluaciones/', views.evaluaciones,          name='evaluaciones'),
    path('profesor/entregables/',  views.entregables_profesor,  name='entregables_profesor'),
]
