from django.urls import path
from . import views

app_name = 'participaciones'

urlpatterns = [
    path('reto/<int:reto_id>/postular/', views.postular_a_reto, name='postular_a_reto'),
    path('reto/<int:reto_id>/favorito/', views.toggle_favorito, name='toggle_favorito'),
    path('entregables/', views.mis_entregables, name='mis_entregables'),
    path('reto/<int:reto_id>/entregables/', views.mis_entregables, name='mis_entregables_reto'),
    path('panel-entregables/', views.panel_entregables, name='panel_entregables'),
]
