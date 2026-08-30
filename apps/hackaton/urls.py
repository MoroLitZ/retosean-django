from django.urls import path

from . import views

app_name = 'hackaton'

urlpatterns = [
    path('', views.lista, name='lista'),
    path('<int:pk>/', views.detalle, name='detalle'),
    path('reto/<int:reto_id>/crear/', views.crear, name='crear'),
    path('<int:pk>/editar/', views.editar, name='editar'),
    path('<int:pk>/etapas/', views.etapas, name='etapas'),
    path('<int:pk>/jurados/', views.jurados, name='jurados'),
    path('jurado/<int:pk>/eliminar/', views.eliminar_jurado, name='eliminar_jurado'),
    path('<int:pk>/publicar/', views.publicar, name='publicar'),
    path('<int:pk>/publicar-resultados/', views.publicar_resultados, name='publicar_resultados'),
    path('<int:pk>/inscribir/', views.inscribir_equipo, name='inscribir_equipo'),
    path('inscripcion/<int:pk>/gestionar/', views.gestionar_inscripcion, name='gestionar_inscripcion'),
    path('<int:pk>/votar/', views.votar, name='votar'),
    path('<int:pk>/ranking/', views.ranking, name='ranking'),
]
