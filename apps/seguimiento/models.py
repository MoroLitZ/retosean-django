from django.db import models
from django.utils import timezone


class IntegracionAcademica(models.Model):
    ESTADOS = [
        ("borrador", "Borrador"),
        ("en_revision", "En Revision"),
        ("aprobada", "Aprobada"),
        ("rechazada", "Rechazada"),
        ("publicada", "Publicada"),
    ]
    reto = models.ForeignKey("retos.Reto", on_delete=models.CASCADE, related_name="integraciones_seguimiento")
    profesor = models.ForeignKey("usuarios.Usuario", on_delete=models.CASCADE, related_name="integraciones")
    facultad = models.CharField(max_length=140, blank=True)
    nivel_formacion = models.CharField(max_length=120, blank=True)
    programa_academico = models.CharField(max_length=160, blank=True)
    ecosistema = models.CharField(max_length=140, blank=True)
    alcance = models.TextField(blank=True)
    entregable_esperado = models.CharField(max_length=180, blank=True)
    cronograma_sesiones = models.TextField(blank=True)
    equipo_profesores = models.TextField(blank=True)
    equipo_estudiantes = models.TextField(blank=True)
    expertos_invitados = models.TextField(blank=True)
    requerimientos_empresa = models.TextField(blank=True)
    requerimientos_internos = models.TextField(blank=True)
    espacio_fisico = models.CharField(max_length=180, blank=True)
    estado = models.CharField(max_length=20, choices=ESTADOS, default="borrador")
    comentarios_revision = models.TextField(blank=True)
    fecha_envio_revision = models.DateTimeField(null=True, blank=True)
    fecha_aprobacion = models.DateTimeField(null=True, blank=True)
    creado_en = models.DateTimeField(auto_now_add=True)
    actualizado_en = models.DateTimeField(auto_now=True)

    class Meta:
        db_table = "integraciones_academicas"
        verbose_name = "Integracion Academica"
        verbose_name_plural = "Integraciones Academicas"

    def __str__(self):
        return f"{self.reto} - {self.profesor}"

    @property
    def campos_faltantes_para_revision(self):
        """
        Calcula qué campos obligatorios están vacíos para enviar a revisión.
        """
        faltantes = []
        if not self.facultad: faltantes.append("Facultad")
        if not self.nivel_formacion: faltantes.append("Nivel de formación")
        if not self.programa_academico: faltantes.append("Programa académico")
        if not self.ecosistema: faltantes.append("Ecosistema")
        if not self.alcance: faltantes.append("Alcance")
        if not self.entregable_esperado: faltantes.append("Entregable esperado")
        if not self.cronograma_sesiones: faltantes.append("Cronograma de sesiones")
        return faltantes

    @property
    def puede_editar_profesor(self):
        return self.estado in {"borrador", "rechazada"}


class SesionReto(models.Model):
    TIPOS = [
        ("inicio", "Inicio"),
        ("seguimiento", "Seguimiento"),
        ("preseleccion", "Preseleccion"),
        ("evaluacion", "Evaluacion / Reconocimiento"),
        ("otro", "Otro"),
    ]
    integracion = models.ForeignKey(IntegracionAcademica, on_delete=models.CASCADE, related_name="sesiones")
    tipo = models.CharField(max_length=30, choices=TIPOS, default="seguimiento")
    fecha = models.DateField(default=timezone.localdate)
    lugar = models.CharField(max_length=180, blank=True)
    descripcion = models.TextField(blank=True)
    completada = models.BooleanField(default=False)
    creado_en = models.DateTimeField(auto_now_add=True)

    class Meta:
        db_table = "sesiones_reto"
        verbose_name = "Sesion de Reto"
        verbose_name_plural = "Sesiones de Reto"

    def __str__(self):
        return f"{self.integracion} - {self.get_tipo_display()}"


class SeguimientoReto(models.Model):
    reto = models.ForeignKey("retos.Reto", on_delete=models.CASCADE, related_name="seguimientos_seguimiento")
    tipo_sesion = models.CharField(max_length=30, blank=True)
    fecha_sesion = models.DateField(default=timezone.localdate)
    porcentaje_avance = models.PositiveSmallIntegerField(default=0)
    avances = models.TextField(blank=True)
    observaciones = models.TextField(blank=True)
    acuerdos = models.TextField(blank=True)
    creado_por = models.ForeignKey("usuarios.Usuario", on_delete=models.SET_NULL, null=True, related_name="seguimientos_creados")
    creado_en = models.DateTimeField(auto_now_add=True)

    class Meta:
        db_table = "seguimientos_reto"
        verbose_name = "Seguimiento de Reto"
        verbose_name_plural = "Seguimientos de Reto"

    def __str__(self):
        return f"{self.reto} - {self.fecha_sesion}"

