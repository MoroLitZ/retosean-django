from django.urls import path

from . import views

app_name = 'notificaciones'

urlpatterns = [
    path('', views.bandeja, name='bandeja'),
    path('<int:pk>/leer/', views.marcar_leida, name='marcar_leida'),
    path('leer-todas/', views.marcar_todas_leidas, name='marcar_todas_leidas'),
    path('preferencias/', views.preferencias, name='preferencias'),
]
