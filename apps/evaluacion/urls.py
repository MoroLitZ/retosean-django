from django.urls import path
from . import views

app_name = 'evaluacion'

urlpatterns = [
    path('profesor/', views.panel_profesor, name='panel_profesor'),
    path('profesor/<int:pk>/calificar/', views.calificar, name='calificar'),
    path('profesor/reto/<int:reto_id>/rubrica/', views.crear_rubrica, name='crear_rubrica'),
    path('entregable/<int:pk>/comentario/', views.comentario, name='comentario'),
]
