import os

from django.core.exceptions import ValidationError
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
    reto = models.ForeignKey("retos.Reto", on_delete=models.CASCADE, related_name="integraciones")
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
        campos = {
            "facultad": self.facultad,
            "nivel formacion": self.nivel_formacion,
            "programa academico": self.programa_academico,
            "ecosistema": self.ecosistema,
            "alcance": self.alcance,
            "entregable esperado": self.entregable_esperado,
            "cronograma sesiones": self.cronograma_sesiones,
            "equipo profesores": self.equipo_profesores,
            "equipo estudiantes": self.equipo_estudiantes,
            "requerimientos empresa": self.requerimientos_empresa,
            "requerimientos internos": self.requerimientos_internos,
            "espacio fisico": self.espacio_fisico,
        }
        return [nombre for nombre, valor in campos.items() if not valor]

    @property
    def puede_editar_profesor(self):
        return self.estado in {"borrador", "rechazada"} or self.reto.estado == "en_curso"

    def clean(self):
        if self.profesor_id and getattr(self.profesor, "rol", None) != "PROFESOR":
            raise ValidationError({"profesor": "La integracion debe pertenecer a un usuario Profesor."})
        if self.reto_id and not self.reto.esta_aprobado_o_activo:
            raise ValidationError({"reto": "Solo se pueden integrar retos aprobados o en curso."})


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
    SESION_CHOICES = [
        ("inicio", "Inicio"),
        ("seguimiento", "Seguimiento"),
        ("preseleccion", "Preseleccion"),
        ("evaluacion", "Evaluacion / reconocimiento"),
        ("otro", "Otro"),
    ]

    reto = models.ForeignKey("retos.Reto", on_delete=models.CASCADE, related_name="seguimientos")
    tipo_sesion = models.CharField(max_length=30, choices=SESION_CHOICES, default="seguimiento")
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


class SeguimientoArchivo(models.Model):
    seguimiento = models.ForeignKey(SeguimientoReto, on_delete=models.CASCADE, related_name="archivos")
    archivo = models.FileField(upload_to="retos/seguimientos/")
    nombre_original = models.CharField(max_length=255, blank=True)
    tamano = models.PositiveBigIntegerField(default=0)
    creado_en = models.DateTimeField(auto_now_add=True)

    class Meta:
        ordering = ["-creado_en"]
        verbose_name = "Archivo de seguimiento"
        verbose_name_plural = "Archivos de seguimiento"

    def save(self, *args, **kwargs):
        if self.archivo and not self.nombre_original:
            self.nombre_original = os.path.basename(self.archivo.name)
        if self.archivo and not self.tamano:
            self.tamano = self.archivo.size
        super().save(*args, **kwargs)

    def __str__(self):
        return self.nombre_original or os.path.basename(self.archivo.name)
