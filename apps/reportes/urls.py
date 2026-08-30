from django.urls import path

from . import views

app_name = 'reportes'

urlpatterns = [
    path('', views.constructor, name='constructor'),
    path('historial/', views.historial, name='historial'),
    path('<int:pk>/descargar/', views.descargar, name='descargar'),
]
