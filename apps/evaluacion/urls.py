from django.urls import path
from . import views

app_name = 'evaluacion'

urlpatterns = [
    path('profesor/', views.panel_profesor, name='panel_profesor'),
    path('profesor/reto/<int:reto_id>/', views.detalle_reto, name='detalle_reto'),
    path('profesor/<int:pk>/calificar/', views.calificar, name='calificar'),
    path('profesor/reto/<int:reto_id>/rubrica/', views.crear_rubrica, name='crear_rubrica'),
    path('profesor/reto/<int:reto_id>/publicar/', views.publicar_calificaciones, name='publicar_calificaciones'),
    path('entregable/<int:pk>/comentario/', views.comentario, name='comentario'),
]
