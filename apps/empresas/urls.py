from django.urls import path
from . import views

app_name = 'empresas'

urlpatterns = [
    # Empresa
    path('publicar-reto/',  views.publicar_reto,              name='publicar_reto'),
    path('mis-retos/',      views.mis_retos_empresa,          name='mis_retos_empresa'),
    path('postulaciones/',  views.postulaciones_empresa,      name='postulaciones'),
    path('postulaciones/<int:postulacion_id>/gestionar/', views.gestionar_postulacion, name='gestionar_postulacion'),
    path('documentos/',     views.panel_documentos_empresa,   name='documentos'),
    path('indicadores/',    views.indicadores_empresa,        name='indicadores_empresa'),
    path('seguimiento/',    views.seguimiento_avance,         name='seguimiento_avance'),
    path('seguimiento/<int:reto_id>/', views.seguimiento_avance, name='seguimiento_reto'),
    path('elegir-convenio/', views.elegir_modo_convenio, name='elegir_convenio'),
    # Admin - Gestión de documentos
    path('admin/empresas/',  views.lista_empresas,    name='lista_empresas'),
    path('admin/revisar-documentos/<int:empresa_id>/', views.admin_revisar_documentacion, name='admin_revisar_documentacion'),
    path('admin/procesar-aprobacion/<int:documento_id>/', views.procesar_aprobacion, name='procesar_aprobacion'),
    path('admin/empresa/<int:empresa_id>/listas/', views.procesar_listas_restrictivas, name='procesar_listas_restrictivas'),
    path('admin/empresa/<int:empresa_id>/convenio/', views.actualizar_convenio, name='actualizar_convenio'),
    path('admin/empresa/<int:empresa_id>/documento-especial/', views.cargar_documento_especial, name='cargar_documento_especial'),
    path('admin/empresa/<int:empresa_id>/bloqueo/', views.cambiar_bloqueo, name='cambiar_bloqueo'),
    path('admin/tipos-documento/', views.tipos_documento, name='tipos_documento'),
    path('admin/tipos-documento/nuevo/', views.editar_tipo_documento, name='crear_tipo_documento'),
    path('admin/tipos-documento/<int:pk>/', views.editar_tipo_documento, name='editar_tipo_documento'),
]
