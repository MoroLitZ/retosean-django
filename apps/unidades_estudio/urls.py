from django.urls import path

from . import views

app_name = "unidades_estudio"

urlpatterns = [
    path("", views.lista_unidades, name="lista"),
    path("crear/", views.crear_unidad, name="crear"),
    path("<int:pk>/editar/", views.editar_unidad, name="editar"),
    path("<int:pk>/eliminar/", views.eliminar_unidad, name="eliminar"),
    path("<int:pk>/activar/", views.activar_unidad, name="activar"),
]
