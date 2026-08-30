import os

from django.conf import settings
from django.core.exceptions import ValidationError
from django.db import models
from django.utils import timezone

class Reto(models.Model):
    TIPO_CHOICES = [
        ("reto", "Reto"),
        ("hackathon", "Hackathon"),
    ]
    AREA_CHOICES = [
        ("Tecnología", "Tecnología"),
        ("Sostenibilidad", "Sostenibilidad"),
        ("Emprendimiento", "Emprendimiento"),
        ("Salud y Bienestar", "Salud y Bienestar"),
        ("Finanzas", "Finanzas"),
        ("Mercadeo", "Mercadeo"),
        ("Derecho", "Derecho"),
        ("Educación", "Educación"),
        ("Diseño y Creatividad", "Diseño y Creatividad"),
        ("Otra", "Otra"),
    ]
    NIVEL_ACADEMICO_CHOICES = [
        ("Pregrado", "Pregrado"),
        ("Posgrado", "Posgrado"),
        ("Especialización", "Especialización"),
        ("Maestría", "Maestría"),
        ("Doctorado", "Doctorado"),
        ("Educación Continua", "Educación Continua"),
    ]
    ESTADO_CHOICES = [
        ("borrador", "Borrador"),
        ("en_revision", "En Revision"),
        ("aprobado", "Aprobado"),
        ("rechazado", "Rechazado"),
        ("en_curso", "En Curso"),
        ("pausado", "Pausado"),
        ("finalizado", "Finalizado"),
        ("cancelado", "Cancelado"),
    ]

    empresa = models.ForeignKey(
        settings.AUTH_USER_MODEL,
        on_delete=models.CASCADE,
        related_name="retos_empresa",
    )
    tipo = models.CharField(max_length=20, choices=TIPO_CHOICES, default="reto")
    titulo = models.CharField(max_length=180, blank=True)
    descripcion = models.TextField(blank=True)
    area = models.CharField(max_length=120, blank=True, choices=AREA_CHOICES)
    nivel_academico = models.CharField(max_length=120, blank=True, choices=NIVEL_ACADEMICO_CHOICES)
    facultad = models.ForeignKey(
        "academico.Facultad",
        on_delete=models.SET_NULL,
        null=True,
        blank=True,
        related_name="retos",
    )
    programa = models.ForeignKey(
        "academico.Programa",
        on_delete=models.SET_NULL,
        null=True,
        blank=True,
        related_name="retos",
    )
    ecosistema = models.ForeignKey(
        "academico.Ecosistema",
        on_delete=models.SET_NULL,
        null=True,
        blank=True,
        related_name="retos",
    )
    fecha_inicio_tentativa = models.DateField(null=True, blank=True)
    fecha_fin_tentativa = models.DateField(null=True, blank=True)
    fecha_limite_postulacion = models.DateField(null=True, blank=True)
    premios = models.TextField(blank=True)
    documento_soporte = models.FileField(upload_to="retos/soportes/", blank=True, null=True)
    criterios_evaluacion = models.TextField(blank=True)
    estado = models.CharField(max_length=20, choices=ESTADO_CHOICES, default="borrador")
    consecutivo = models.PositiveIntegerField(unique=True, null=True, blank=True)
    comentarios_revision = models.TextField(blank=True)
    fecha_envio_revision = models.DateTimeField(null=True, blank=True)
    fecha_aprobacion = models.DateTimeField(null=True, blank=True)
    creado_en = models.DateTimeField(auto_now_add=True)
    actualizado_en = models.DateTimeField(auto_now=True)
    tiene_convenio_institucional = models.BooleanField(default=False)

    class Meta:
        db_table = "retos"
        ordering = ["-actualizado_en"]
        verbose_name = "Reto"
        verbose_name_plural = "Retos"

    def __str__(self):
        return self.titulo or f"Reto #{self.pk or 'nuevo'}"

    def campos_faltantes_para_revision(self):
        """
        Calcula qué campos obligatorios están vacíos para enviar a revisión.
        """
        faltantes = []
        if not self.titulo: faltantes.append("Título")
        if not self.descripcion: faltantes.append("Descripción")
        if not self.area: faltantes.append("Área")
        if not self.fecha_inicio_tentativa: faltantes.append("Fecha de inicio tentativa")
        if not self.fecha_fin_tentativa: faltantes.append("Fecha de fin tentativa")
        if not self.fecha_limite_postulacion: faltantes.append("Fecha límite de postulación")
        return faltantes

    @property
    def puede_editar_empresa(self):
        return self.estado in {"borrador", "rechazado"}

    @property
    def esta_aprobado_o_activo(self):
        return self.estado in {"aprobado", "en_curso", "pausado", "finalizado"}

    def asignar_consecutivo_si_falta(self):
        if self.consecutivo:
            return
        ultimo = Reto.objects.exclude(consecutivo__isnull=True).order_by("-consecutivo").first()
        self.consecutivo = (ultimo.consecutivo if ultimo else 0) + 1
        self.save()


class HistorialEstadoReto(models.Model):
    reto = models.ForeignKey('retos.Reto', on_delete=models.CASCADE, related_name="historial_estados")
    estado_anterior = models.CharField(max_length=20, blank=True)
    estado_nuevo = models.CharField(max_length=20, choices=Reto.ESTADO_CHOICES)
    comentario = models.TextField(blank=True)
    realizado_por = models.ForeignKey(
        settings.AUTH_USER_MODEL,
        on_delete=models.SET_NULL,
        null=True,
        blank=True,
        related_name="cambios_estado_retos",
    )
    fecha = models.DateTimeField(auto_now_add=True)

    class Meta:
        db_table = "historial_estados_reto"
        ordering = ["-fecha"]
        verbose_name = "Historial de Estado de Reto"
        verbose_name_plural = "Historial de Estados de Reto"

    def __str__(self):
        return f"{self.reto} -> {self.estado_nuevo}"


class RetoArchivo(models.Model):
    reto = models.ForeignKey(Reto, on_delete=models.CASCADE, related_name="archivos")
    archivo = models.FileField(upload_to="retos/soportes/")
    nombre_original = models.CharField(max_length=255, blank=True)
    tamano = models.PositiveBigIntegerField(default=0)
    creado_en = models.DateTimeField(auto_now_add=True)

    class Meta:
        ordering = ["-creado_en"]
        verbose_name = "Archivo de reto"
        verbose_name_plural = "Archivos de reto"

    def save(self, *args, **kwargs):
        if self.archivo and not self.nombre_original:
            self.nombre_original = os.path.basename(self.archivo.name)
        if self.archivo and not self.tamano:
            self.tamano = self.archivo.size
        super().save(*args, **kwargs)

    def __str__(self):
        return self.nombre_original or os.path.basename(self.archivo.name)

class EquipoRetoAcademico(models.Model): # <--- Cambiamos el nombre para que no choque con 'participaciones.Equipo'
    reto = models.ForeignKey(Reto, on_delete=models.CASCADE, related_name='equipos_academicos') # <--- related_name único
    nombre_equipo = models.CharField(max_length=100)
    profesores = models.ManyToManyField(settings.AUTH_USER_MODEL, related_name='equipos_academicos_profesor', blank=True)
    estudiantes = models.ManyToManyField(settings.AUTH_USER_MODEL, related_name='equipos_academicos_estudiante', blank=True)
    expertos_invitados = models.ManyToManyField(settings.AUTH_USER_MODEL, related_name='equipos_academicos_experto', blank=True)
    # Equipo operativo espejo. Los entregables y las votaciones de hackathon
    # apuntan a participaciones.Equipo, asi que el equipo que arma el profesor
    # se refleja alli en vez de fusionar ambos modelos.
    equipo_espejo = models.OneToOneField(
        'participaciones.Equipo',
        on_delete=models.SET_NULL,
        null=True,
        blank=True,
        related_name='origen_academico',
    )

    class Meta:
        db_table = "equipos_reto_academico" # <--- Tabla única
        verbose_name = "Equipo de Reto Académico"
        verbose_name_plural = "Equipos de Reto Académico"

    def __str__(self):
        return f"{self.nombre_equipo} - {self.reto.titulo}"


class SesionRetoAcademico(models.Model): # <--- Cambiamos el nombre para que no choque con 'seguimiento.SesionReto'
    TIPO_SESION_CHOICES = [
        ('inicio', 'Inicio'),
        ('seguimiento', 'Seguimiento'),
        ('preseleccion', 'Preselección'),
        ('evaluacion', 'Evaluación'),
    ]
    
    reto = models.ForeignKey(Reto, on_delete=models.CASCADE, related_name='sesiones_academicas') # <--- related_name único
    tipo = models.CharField(max_length=20, choices=TIPO_SESION_CHOICES)
    fecha_hora = models.DateTimeField()
    enlace_reunion = models.URLField(blank=True, null=True)
    observaciones = models.TextField(blank=True)

    class Meta:
        db_table = "sesiones_reto_academico" # <--- Tabla única
        ordering = ["fecha_hora"]
        verbose_name = "Sesión Académica de Reto"
        verbose_name_plural = "Sesiones Académicas de Reto"

    def __str__(self):
        return f"Sesión ({self.get_tipo_display()}) - {self.reto.titulo}"