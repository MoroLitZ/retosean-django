from django.db import models


class AgendaCierre(models.Model):
    reto = models.OneToOneField("retos.Reto", on_delete=models.CASCADE, related_name="agenda_cierre")
    fecha_hora = models.DateTimeField(null=True, blank=True)
    espacio = models.CharField(max_length=200, blank=True)
    recursos = models.TextField(blank=True)
    invitados = models.TextField(blank=True)
    agenda = models.TextField(blank=True)
    actualizado_en = models.DateTimeField(auto_now=True)

    class Meta:
        verbose_name = "Agenda de cierre"
        verbose_name_plural = "Agendas de cierre"

    def __str__(self):
        return f"Agenda de cierre - {self.reto}"


class CierreReto(models.Model):
    """Cierre formal del reto. La logistica del evento vive en AgendaCierre."""

    reto = models.OneToOneField("retos.Reto", on_delete=models.CASCADE, related_name="cierre")
    acta_url = models.FileField(upload_to="actas_cierre/", null=True, blank=True)
    notas = models.TextField(blank=True)
    # HU05: si la empresa aplico la solucion del reto.
    IMPACTO = [
        ("", "Sin registrar"),
        ("SI", "Sí, la aplicó"),
        ("NO", "No la aplicó"),
        ("EN_EVALUACION", "En evaluación"),
    ]
    impacto = models.CharField(
        "¿La empresa aplicó la solución?", max_length=20, choices=IMPACTO, blank=True, default=""
    )
    encuesta_enviada = models.BooleanField(default=False)
    cerrado_por = models.ForeignKey("usuarios.Usuario", on_delete=models.SET_NULL, null=True, related_name="cierres")
    cerrado_en = models.DateTimeField(null=True, blank=True)
    actualizado_en = models.DateTimeField(auto_now=True)

    class Meta:
        db_table = "cierres_reto"
        verbose_name = "Cierre de Reto"
        verbose_name_plural = "Cierres de Reto"

    def __str__(self):
        return f"Cierre de {self.reto}"


class Reconocimiento(models.Model):
    cierre = models.ForeignKey(CierreReto, on_delete=models.CASCADE, related_name="reconocimientos")
    usuario = models.ForeignKey("usuarios.Usuario", on_delete=models.SET_NULL, null=True, blank=True, related_name="reconocimientos")
    nombre_destinatario = models.CharField(max_length=150)
    descripcion = models.TextField()
    creado_en = models.DateTimeField(auto_now_add=True)

    class Meta:
        db_table = "reconocimientos"
        verbose_name = "Reconocimiento"
        verbose_name_plural = "Reconocimientos"

    def __str__(self):
        return f"{self.nombre_destinatario} - {self.cierre.reto}"


class EntregableFinal(models.Model):
    cierre = models.ForeignKey(CierreReto, on_delete=models.CASCADE, related_name="entregables_finales")
    nombre = models.CharField(max_length=180)
    archivo = models.FileField(upload_to="cierres/entregables/")
    cargado_por = models.ForeignKey(
        "usuarios.Usuario", on_delete=models.SET_NULL, null=True, related_name="entregables_finales_cargados"
    )
    creado_en = models.DateTimeField(auto_now_add=True)

    class Meta:
        ordering = ["-creado_en"]

    def __str__(self):
        return self.nombre


class EncuestaSatisfaccion(models.Model):
    cierre = models.ForeignKey(CierreReto, on_delete=models.CASCADE, related_name="encuestas")
    participante = models.ForeignKey(
        "usuarios.Usuario", on_delete=models.CASCADE, related_name="encuestas_satisfaccion"
    )
    calificacion = models.PositiveSmallIntegerField(null=True, blank=True)
    comentario = models.TextField(blank=True)
    respondida_en = models.DateTimeField(null=True, blank=True)
    creada_en = models.DateTimeField(auto_now_add=True)

    class Meta:
        unique_together = ["cierre", "participante"]

    @property
    def respondida(self):
        return self.respondida_en is not None

    def __str__(self):
        return f"Encuesta {self.cierre.reto} - {self.participante.username}"


class SolicitudLogistica(models.Model):
    """Solicitudes de logistica para los eventos de inicio y cierre del reto (HU08).

    Espacios fisicos, apoyo del area de comunicaciones y requerimientos extra
    (refrigerios, equipos audiovisuales, invitados). El administrador las marca
    como confirmadas o descartadas; cada cambio queda en el historial de estados.
    """

    EVENTOS = [("INICIO", "Evento de inicio"), ("CIERRE", "Evento de cierre")]
    TIPOS = [
        ("ESPACIO", "Espacio físico"),
        ("COMUNICACIONES", "Apoyo del área de comunicaciones"),
        ("REFRIGERIOS", "Refrigerios / catering"),
        ("EQUIPOS_AV", "Equipos audiovisuales"),
        ("INVITADOS", "Invitados"),
        ("OTRO", "Otro"),
    ]
    ESTADOS = [("SOLICITADA", "Solicitada"), ("CONFIRMADA", "Confirmada"), ("DESCARTADA", "Descartada")]

    reto = models.ForeignKey("retos.Reto", on_delete=models.CASCADE, related_name="solicitudes_logistica")
    evento = models.CharField(max_length=10, choices=EVENTOS)
    tipo = models.CharField(max_length=20, choices=TIPOS)
    descripcion = models.TextField()
    estado = models.CharField(max_length=12, choices=ESTADOS, default="SOLICITADA")
    solicitada_por = models.ForeignKey(
        "usuarios.Usuario", on_delete=models.SET_NULL, null=True, related_name="solicitudes_logistica"
    )
    creada_en = models.DateTimeField(auto_now_add=True)
    actualizado_en = models.DateTimeField(auto_now=True)

    class Meta:
        ordering = ["evento", "creada_en"]
        verbose_name = "Solicitud de logística"
        verbose_name_plural = "Solicitudes de logística"

    def __str__(self):
        return f"{self.get_tipo_display()} ({self.get_evento_display()}) - {self.reto}"
