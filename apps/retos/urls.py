from django.urls import path

from . import views

app_name = 'retos'

urlpatterns = [
    path('mis-retos/', views.mis_retos, name='mis_retos'),
    path('crear/', views.crear_reto, name='crear'),
    path('<int:pk>/', views.detalle_reto, name='detalle'),
    path('<int:pk>/editar/', views.editar_reto, name='editar'),
    path('<int:pk>/enviar-revision/', views.enviar_revision, name='enviar_revision'),
    path('<int:pk>/eliminar/', views.eliminar_reto, name='eliminar'),
    path('<int:pk>/seguimientos/', views.seguimientos_reto, name='seguimientos'),
    path('<int:pk>/seguimientos/nuevo/', views.agregar_seguimiento, name='agregar_seguimiento'),
    path('admin/panel/', views.panel_admin_retos, name='admin_panel'),
    path('admin/<int:pk>/revisar/', views.revisar_reto, name='admin_revisar'),
    path('admin/<int:pk>/estado/', views.cambiar_estado, name='admin_cambiar_estado'),
    path('integraciones/', views.mis_integraciones, name='mis_integraciones'),
    path('integraciones/crear/', views.crear_integracion, name='crear_integracion'),
    path('integraciones/<int:pk>/', views.detalle_integracion, name='detalle_integracion'),
    path('integraciones/<int:pk>/editar/', views.editar_integracion, name='editar_integracion'),
    path(
        'integraciones/<int:pk>/enviar-revision/',
        views.enviar_integracion_revision,
        name='enviar_integracion_revision',
    ),
    path('integraciones/<int:pk>/publicar/', views.publicar_integracion, name='publicar_integracion'),
    path('admin/integraciones/', views.admin_integraciones, name='admin_integraciones'),
    path('admin/integraciones/<int:pk>/revisar/', views.revisar_integracion, name='admin_revisar_integracion'),
]
