import os
from celery import Celery

# Establece el módulo de configuración predeterminado de Django para el programa 'celery'.
os.environ.setdefault('DJANGO_SETTINGS_MODULE', 'config.settings.development') 
# Nota: Si tu carpeta se llama de otra forma que no sea 'config', cambia 'config.settings.development' por el nombre real de tu app principal (ej: 'tu_proyecto.settings.development')

app = Celery('config')

# Usar una cadena aquí significa que el trabajador no tiene que serializar
# el objeto de configuración para los hijos.
# namespace='CELERY' significa que todas las configuraciones de celery en settings deben empezar con CELERY_
app.config_from_object('django.conf:settings', namespace='CELERY')

# Carga tareas de todas las apps de Django registradas.
app.autodiscover_tasks()