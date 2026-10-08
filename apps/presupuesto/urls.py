from django.urls import path

from . import views

app_name = 'presupuesto'

urlpatterns = [
    path('', views.panel, name='panel'),
    path('reto/<int:reto_id>/', views.detalle, name='detalle'),
    path('reto/<int:reto_id>/definir/', views.definir_presupuesto, name='definir'),
    path('reto/<int:reto_id>/gasto/', views.agregar_gasto, name='agregar_gasto'),
    path('gasto/<int:pk>/eliminar/', views.eliminar_gasto, name='eliminar_gasto'),
]
