import os

from django.conf import settings
from django.core.exceptions import ValidationError
from django.db import models
from django.utils import timezone


class Reto(models.Model):
    TIPO_CHOICES = [
        ('reto', 'Reto'),
        ('hackathon', 'Hackathon'),
    ]
    ESTADO_CHOICES = [
        ('borrador', 'Borrador'),
        ('en_revision', 'En aprobacion'),
        ('aprobado', 'Aprobado'),
        ('rechazado', 'Rechazado'),
        ('en_curso', 'En curso'),
        ('pausado', 'Pausado'),
        ('finalizado', 'Finalizado'),
        ('cancelado', 'Cancelado'),
    ]

    empresa = models.ForeignKey(
        settings.AUTH_USER_MODEL,
        on_delete=models.CASCADE,
        related_name='retos_empresa',
        verbose_name='Empresa',
    )
    tipo = models.CharField(max_length=20, choices=TIPO_CHOICES, default='reto')
    titulo = models.CharField(max_length=180, blank=True)
    descripcion = models.TextField(blank=True)
    area = models.CharField(max_length=120, blank=True)
    nivel_academico = models.CharField(max_length=120, blank=True)
    fecha_inicio_tentativa = models.DateField(null=True, blank=True)
    fecha_fin_tentativa = models.DateField(null=True, blank=True)
    fecha_limite_postulacion = models.DateField(null=True, blank=True)
    premios = models.TextField(blank=True)
    documento_soporte = models.FileField(upload_to='retos/soportes/', blank=True, null=True)
    criterios_evaluacion = models.TextField(blank=True)
    estado = models.CharField(max_length=20, choices=ESTADO_CHOICES, default='borrador')
    consecutivo = models.PositiveIntegerField(unique=True, null=True, blank=True)
    comentarios_revision = models.TextField(blank=True)
    fecha_envio_revision = models.DateTimeField(null=True, blank=True)
    fecha_aprobacion = models.DateTimeField(null=True, blank=True)
    creado_en = models.DateTimeField(auto_now_add=True)
    actualizado_en = models.DateTimeField(auto_now=True)

    class Meta:
        ordering = ['-actualizado_en']
        indexes = [
            models.Index(fields=['empresa', 'estado']),
            models.Index(fields=['estado', 'fecha_limite_postulacion']),
        ]
        verbose_name = 'Reto'
        verbose_name_plural = 'Retos'

    def __str__(self):
        return self.titulo or f'Reto #{self.pk or "nuevo"}'

    def clean(self):
        if self.empresa_id and getattr(self.empresa, 'rol', None) != 'EMPRESA':
            raise ValidationError({'empresa': 'El propietario del reto debe tener rol Empresa.'})

    def campos_faltantes_para_revision(self):
        campos = {
            'titulo': self.titulo,
            'descripcion': self.descripcion,
            'area': self.area,
            'nivel academico': self.nivel_academico,
            'fecha inicio tentativa': self.fecha_inicio_tentativa,
            'fecha fin tentativa': self.fecha_fin_tentativa,
            'criterios de evaluacion': self.criterios_evaluacion,
        }
        return [nombre for nombre, valor in campos.items() if not valor]

    @property
    def puede_editar_empresa(self):
        return self.estado in {'borrador', 'rechazado'}

    @property
    def esta_aprobado_o_activo(self):
        return self.estado in {'aprobado', 'en_curso', 'pausado', 'finalizado'}

    def asignar_consecutivo_si_falta(self):
        if self.consecutivo:
            return
        ultimo = Reto.objects.exclude(consecutivo__isnull=True).order_by('-consecutivo').first()
        self.consecutivo = (ultimo.consecutivo if ultimo else 0) + 1


class HistorialEstadoReto(models.Model):
    reto = models.ForeignKey(Reto, on_delete=models.CASCADE, related_name='historial_estados')
    estado_anterior = models.CharField(max_length=20, blank=True)
    estado_nuevo = models.CharField(max_length=20, choices=Reto.ESTADO_CHOICES)
    comentario = models.TextField(blank=True)
    realizado_por = models.ForeignKey(
        settings.AUTH_USER_MODEL,
        on_delete=models.SET_NULL,
        null=True,
        blank=True,
        related_name='cambios_estado_retos',
    )
    fecha = models.DateTimeField(auto_now_add=True)

    class Meta:
        ordering = ['-fecha']
        verbose_name = 'Historial de estado de reto'
        verbose_name_plural = 'Historial de estados de retos'

    def __str__(self):
        return f'{self.reto} -> {self.get_estado_nuevo_display()}'


class SeguimientoReto(models.Model):
    SESION_CHOICES = [
        ('inicio', 'Inicio'),
        ('seguimiento', 'Seguimiento'),
        ('preseleccion', 'Preseleccion'),
        ('evaluacion', 'Evaluacion / reconocimiento'),
        ('otro', 'Otro'),
    ]

    reto = models.ForeignKey(Reto, on_delete=models.CASCADE, related_name='seguimientos')
    tipo_sesion = models.CharField(max_length=30, choices=SESION_CHOICES, default='seguimiento')
    fecha_sesion = models.DateField(default=timezone.localdate)
    porcentaje_avance = models.PositiveSmallIntegerField(default=0)
    avances = models.TextField(blank=True)
    observaciones = models.TextField(blank=True)
    acuerdos = models.TextField(blank=True)
    creado_por = models.ForeignKey(
        settings.AUTH_USER_MODEL,
        on_delete=models.SET_NULL,
        null=True,
        related_name='seguimientos_retos',
    )
    creado_en = models.DateTimeField(auto_now_add=True)

    class Meta:
        ordering = ['-fecha_sesion', '-creado_en']
        verbose_name = 'Seguimiento de reto'
        verbose_name_plural = 'Seguimientos de retos'

    def __str__(self):
        return f'{self.reto} - {self.get_tipo_sesion_display()}'


class RetoArchivo(models.Model):
    reto = models.ForeignKey(Reto, on_delete=models.CASCADE, related_name='archivos')
    archivo = models.FileField(upload_to='retos/soportes/')
    nombre_original = models.CharField(max_length=255, blank=True)
    tamano = models.PositiveBigIntegerField(default=0)
    creado_en = models.DateTimeField(auto_now_add=True)

    class Meta:
        ordering = ['-creado_en']
        verbose_name = 'Archivo de reto'
        verbose_name_plural = 'Archivos de reto'

    def save(self, *args, **kwargs):
        if self.archivo and not self.nombre_original:
            self.nombre_original = os.path.basename(self.archivo.name)
        if self.archivo and not self.tamano:
            self.tamano = self.archivo.size
        super().save(*args, **kwargs)

    def __str__(self):
        return self.nombre_original or os.path.basename(self.archivo.name)


class SeguimientoArchivo(models.Model):
    seguimiento = models.ForeignKey(SeguimientoReto, on_delete=models.CASCADE, related_name='archivos')
    archivo = models.FileField(upload_to='retos/seguimientos/')
    nombre_original = models.CharField(max_length=255, blank=True)
    tamano = models.PositiveBigIntegerField(default=0)
    creado_en = models.DateTimeField(auto_now_add=True)

    class Meta:
        ordering = ['-creado_en']
        verbose_name = 'Archivo de seguimiento'
        verbose_name_plural = 'Archivos de seguimiento'

    def save(self, *args, **kwargs):
        if self.archivo and not self.nombre_original:
            self.nombre_original = os.path.basename(self.archivo.name)
        if self.archivo and not self.tamano:
            self.tamano = self.archivo.size
        super().save(*args, **kwargs)

    def __str__(self):
        return self.nombre_original or os.path.basename(self.archivo.name)


class IntegracionAcademica(models.Model):
    ESTADO_CHOICES = [
        ('borrador', 'Borrador'),
        ('en_revision', 'En revision'),
        ('aprobada', 'Aprobada'),
        ('rechazada', 'Rechazada'),
        ('publicada', 'Publicada'),
    ]

    reto = models.ForeignKey(Reto, on_delete=models.CASCADE, related_name='integraciones')
    profesor = models.ForeignKey(
        settings.AUTH_USER_MODEL,
        on_delete=models.CASCADE,
        related_name='integraciones_profesor',
    )
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
    estado = models.CharField(max_length=20, choices=ESTADO_CHOICES, default='borrador')
    comentarios_revision = models.TextField(blank=True)
    fecha_envio_revision = models.DateTimeField(null=True, blank=True)
    fecha_aprobacion = models.DateTimeField(null=True, blank=True)
    creado_en = models.DateTimeField(auto_now_add=True)
    actualizado_en = models.DateTimeField(auto_now=True)

    class Meta:
        ordering = ['-actualizado_en']
        indexes = [
            models.Index(fields=['profesor', 'estado']),
            models.Index(fields=['reto', 'estado']),
        ]
        verbose_name = 'Integracion academica'
        verbose_name_plural = 'Integraciones academicas'

    def __str__(self):
        return f'{self.reto} - {self.profesor}'

    def clean(self):
        if self.profesor_id and getattr(self.profesor, 'rol', None) != 'PROFESOR':
            raise ValidationError({'profesor': 'La integracion debe pertenecer a un usuario Profesor.'})
        if self.reto_id and not self.reto.esta_aprobado_o_activo:
            raise ValidationError({'reto': 'Solo se pueden integrar retos aprobados o en curso.'})

    def campos_faltantes_para_revision(self):
        campos = {
            'facultad': self.facultad,
            'nivel formacion': self.nivel_formacion,
            'programa academico': self.programa_academico,
            'ecosistema': self.ecosistema,
            'alcance': self.alcance,
            'entregable esperado': self.entregable_esperado,
            'cronograma sesiones': self.cronograma_sesiones,
            'equipo profesores': self.equipo_profesores,
            'equipo estudiantes': self.equipo_estudiantes,
            'requerimientos empresa': self.requerimientos_empresa,
            'requerimientos internos': self.requerimientos_internos,
            'espacio fisico': self.espacio_fisico,
        }
        return [nombre for nombre, valor in campos.items() if not valor]

    @property
    def puede_editar_profesor(self):
        return self.estado in {'borrador', 'rechazada'} or self.reto.estado == 'en_curso'
