from django.contrib import admin

from .models import ComentarioEntregable, CriterioRubrica, Entregable, Evaluacion, EvaluacionCriterio, Rubrica

admin.site.register([Entregable, Evaluacion, Rubrica, CriterioRubrica, EvaluacionCriterio, ComentarioEntregable])
