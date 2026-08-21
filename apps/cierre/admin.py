from django.contrib import admin

from .models import AgendaCierre, CierreReto, EncuestaSatisfaccion, EntregableFinal, Reconocimiento

admin.site.register([AgendaCierre, CierreReto, EntregableFinal, Reconocimiento, EncuestaSatisfaccion])
