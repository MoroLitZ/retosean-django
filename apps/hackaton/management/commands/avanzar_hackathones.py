"""Avanza los hackathones segun su calendario (HU09).

Publicado -> En curso al iniciar la etapa de desarrollo, y al cierre publica
resultados y ganadores. Programalo junto a enviar_recordatorios (p. ej. cada
hora); la vista de detalle tambien lo aplica al abrirse.

    python manage.py avanzar_hackathones
"""

from django.core.management.base import BaseCommand

from apps.hackaton.models import Hackathon
from apps.hackaton.services import avanzar_por_calendario


class Command(BaseCommand):
    help = "Avanza el estado de los hackathones y publica resultados segun sus etapas."

    def handle(self, *args, **options):
        total = 0
        for hackathon in Hackathon.objects.filter(estado__in=["publicado", "en_curso"]).select_related("reto"):
            cambios = avanzar_por_calendario(hackathon)
            if cambios:
                total += 1
                self.stdout.write(f"{hackathon.reto.titulo}: {', '.join(cambios)}")
        self.stdout.write(self.style.SUCCESS(f"{total} hackathon(es) actualizados."))
