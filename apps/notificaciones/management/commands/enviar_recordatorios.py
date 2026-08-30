"""Recordatorios automaticos de fechas clave (HU15).

Sustituye a Celery beat: se ejecuta una vez al dia desde el Programador de
tareas de Windows o desde cron. Es idempotente gracias a `clave_dedupe`, asi
que volver a lanzarlo el mismo dia no duplica avisos.

    python manage.py enviar_recordatorios --dias 7 3 1
    python manage.py enviar_recordatorios --dry-run
"""

from datetime import timedelta

from django.core.management.base import BaseCommand
from django.db.models import Q
from django.urls import NoReverseMatch, reverse
from django.utils import timezone

from apps.notificaciones.services import notificar_muchos

DIAS_POR_DEFECTO = [7, 3, 1]


class Command(BaseCommand):
    help = "Envia recordatorios de fechas limite a los participantes de los retos."

    def add_arguments(self, parser):
        parser.add_argument(
            "--dias", nargs="+", type=int, default=DIAS_POR_DEFECTO,
            help="Dias de antelacion a avisar (por defecto: 7 3 1).",
        )
        parser.add_argument(
            "--dry-run", action="store_true",
            help="Muestra que se enviaria sin escribir nada.",
        )

    def handle(self, *args, **opciones):
        dias = sorted(set(opciones["dias"]), reverse=True)
        self.dry_run = opciones["dry_run"]
        hoy = timezone.localdate()

        total = 0
        total += self._marcar_documentos_vencidos(hoy)
        for antelacion in dias:
            objetivo = hoy + timedelta(days=antelacion)
            total += self._cierre_de_postulaciones(objetivo, antelacion)
            total += self._fin_de_reto(objetivo, antelacion)
            total += self._documentos_por_vencer(objetivo, antelacion)
            total += self._etapas_de_hackaton(objetivo, antelacion)

        total += self._entregables_sin_calificar(dias_espera=max(dias))

        prefijo = "[dry-run] " if self.dry_run else ""
        self.stdout.write(self.style.SUCCESS(f"{prefijo}{total} recordatorio(s) generado(s)."))

    # ------------------------------------------------------------------
    def _emitir(self, usuarios, evento, mensaje, link, clave):
        usuarios = [u for u in usuarios if u is not None]
        if not usuarios:
            return 0
        if self.dry_run:
            self.stdout.write(f"  - {clave} -> {len(usuarios)} destinatario(s): {mensaje}")
            return len(usuarios)
        # Respetamos a quien desactivo los recordatorios en sus preferencias.
        destinatarios = [
            u for u in usuarios
            if getattr(getattr(u, "preferencias_notificacion", None), "recibir_recordatorios", True)
        ]
        return len(notificar_muchos(
            destinatarios, evento, mensaje=mensaje, link=link, clave_dedupe=clave
        ))

    # ------------------------------------------------------------------
    def _cierre_de_postulaciones(self, objetivo, antelacion):
        from apps.retos.models import Reto
        from apps.usuarios.models import Usuario

        total = 0
        retos = Reto.objects.filter(
            estado="aprobado", fecha_limite_postulacion=objetivo
        ).select_related("empresa")
        for reto in retos:
            # Estudiantes interesados: lo tienen en favoritos o ya se postularon.
            interesados = Usuario.objects.filter(
                Q(favoritos__reto=reto)
                | Q(postulaciones__reto=reto, postulaciones__estado="PENDIENTE"),
                rol="ESTUDIANTE", is_active=True,
            ).distinct()
            total += self._emitir(
                interesados, "RECORDATORIO_FECHA_LIMITE",
                f"Quedan {antelacion} dia(s) para postularte al reto '{reto.titulo}'.",
                reverse("retos:detalle", kwargs={"pk": reto.pk}),
                f"recordatorio:postulacion:{reto.pk}:{antelacion}",
            )
        return total

    def _fin_de_reto(self, objetivo, antelacion):
        from apps.evaluacion.models import Entregable
        from apps.retos.models import Reto
        from apps.usuarios.models import Usuario

        total = 0
        retos = Reto.objects.filter(
            estado__in=["aprobado", "en_curso"], fecha_fin_tentativa=objetivo
        )
        for reto in retos:
            con_entrega_final = Entregable.objects.filter(
                reto=reto, es_final=True
            ).values_list("estudiante_id", flat=True)
            pendientes = Usuario.objects.filter(
                postulaciones__reto=reto, postulaciones__estado="ACEPTADA", is_active=True,
            ).exclude(pk__in=con_entrega_final).distinct()
            total += self._emitir(
                pendientes, "RECORDATORIO_FECHA_LIMITE",
                f"Quedan {antelacion} dia(s) para la entrega final del reto '{reto.titulo}'.",
                reverse("participaciones:mis_entregables_reto", kwargs={"reto_id": reto.pk}),
                f"recordatorio:entrega-final:{reto.pk}:{antelacion}",
            )

            profesores = Usuario.objects.filter(integraciones__reto=reto).distinct()
            total += self._emitir(
                profesores, "RECORDATORIO_FECHA_LIMITE",
                f"El reto '{reto.titulo}' finaliza en {antelacion} dia(s).",
                reverse("evaluacion:panel_profesor"),
                f"recordatorio:fin-reto:{reto.pk}:{antelacion}:profesor",
            )
        return total

    def _marcar_documentos_vencidos(self, hoy):
        """Marca VENCIDO los documentos ya vencidos y aun activos.

        El estado `VENCIDO` de `DocumentoEmpresa` solo existia en el `choices`;
        nada lo escribia. Este es el punto donde el vencimiento se detecta.
        """
        from apps.empresas.models import DocumentoEmpresa

        vencidos = DocumentoEmpresa.objects.filter(
            fecha_vencimiento__lt=hoy, estado__in=["VERIFICADO", "CARGADO"]
        )
        if self.dry_run:
            for documento in vencidos:
                self.stdout.write(
                    f"  - VENCIDO: {documento} ({documento.fecha_vencimiento})"
                )
            return vencidos.count()
        return vencidos.update(estado="VENCIDO")

    def _documentos_por_vencer(self, objetivo, antelacion):
        from apps.empresas.models import DocumentoEmpresa

        total = 0
        documentos = DocumentoEmpresa.objects.filter(
            fecha_vencimiento=objetivo, estado="VERIFICADO"
        ).select_related("empresa__usuario")
        for documento in documentos:
            total += self._emitir(
                [documento.empresa.usuario], "RECORDATORIO_FECHA_LIMITE",
                f"Tu documento '{documento.get_tipo_documento_display()}' "
                f"vence en {antelacion} dia(s).",
                reverse("empresas:documentos"),
                f"recordatorio:documento:{documento.pk}:{antelacion}",
            )
        return total

    def _etapas_de_hackaton(self, objetivo, antelacion):
        from apps.hackaton.models import EtapaHackaton
        from apps.usuarios.models import Usuario

        total = 0
        etapas = EtapaHackaton.objects.filter(
            inicia_en__date=objetivo, hackathon__estado__in=["publicado", "en_curso"]
        ).select_related("hackathon__reto")
        for etapa in etapas:
            reto = etapa.hackathon.reto
            participantes = Usuario.objects.filter(
                postulaciones__reto=reto, postulaciones__estado="ACEPTADA", is_active=True
            ).distinct()
            try:
                link = reverse("hackaton:detalle", kwargs={"pk": etapa.hackathon_id})
            except NoReverseMatch:
                # El modulo de hackathon puede no estar enrutado todavia.
                link = ""
            total += self._emitir(
                participantes, "RECORDATORIO_FECHA_LIMITE",
                f"La etapa '{etapa.titulo}' del hackathon '{reto.titulo}' "
                f"inicia en {antelacion} dia(s).",
                link,
                f"recordatorio:etapa:{etapa.pk}:{antelacion}",
            )
        return total

    def _entregables_sin_calificar(self, dias_espera):
        from apps.evaluacion.models import Entregable
        from apps.usuarios.models import Usuario

        limite = timezone.now() - timedelta(days=dias_espera)
        pendientes = Entregable.objects.filter(
            estado__in=["ENVIADO", "EN_REVISION"], fecha_entrega__lt=limite
        ).select_related("reto")

        por_reto = {}
        for entregable in pendientes:
            por_reto[entregable.reto] = por_reto.get(entregable.reto, 0) + 1

        total = 0
        semana = timezone.localdate().strftime("%Y-%W")
        for reto, cantidad in por_reto.items():
            profesores = Usuario.objects.filter(integraciones__reto=reto).distinct()
            total += self._emitir(
                profesores, "RECORDATORIO_FECHA_LIMITE",
                f"Tienes {cantidad} entregable(s) del reto '{reto.titulo}' sin calificar "
                f"desde hace mas de {dias_espera} dias.",
                reverse("evaluacion:panel_profesor"),
                f"recordatorio:sin-calificar:{reto.pk}:{semana}",
            )
        return total
