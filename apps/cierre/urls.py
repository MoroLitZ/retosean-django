from django.urls import path

from . import views

app_name = "cierre"

urlpatterns = [
    path("admin/", views.panel, name="panel"),
    path("admin/reto/<int:reto_id>/", views.gestionar, name="gestionar"),
    path("admin/reto/<int:reto_id>/entregable/", views.agregar_entregable, name="agregar_entregable"),
    path("admin/reto/<int:reto_id>/reconocimiento/", views.agregar_reconocimiento, name="agregar_reconocimiento"),
    path("admin/reto/<int:reto_id>/finalizar/", views.finalizar, name="finalizar"),
    path("encuestas/", views.mis_encuestas, name="mis_encuestas"),
    path("encuestas/<int:pk>/", views.responder_encuesta, name="responder_encuesta"),
]
