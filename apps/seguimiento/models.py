import os

from django.contrib.contenttypes.fields import GenericForeignKey
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
    reto = models.ForeignKey("retos.Reto", on_delete=models.CASCADE, related_name="integraciones_seguimiento")
    profesor = models.ForeignKey("usuarios.Usuario", on_delete=models.CASCADE, related_name="integraciones")
    facultad = models.CharField(max_length=140, blank=True)
    nivel_formacion = models.CharField(max_length=120, blank=True)
    programa_academico = models.CharField(max_length=160, blank=True)
    ecosistema = models.CharField(max_length=140, blank=True)
    descripcion = models.TextField(blank=True)
    alcance = models.TextField(blank=True)
    entregable_esperado = models.CharField(max_length=180, blank=True)
    # Notas libres del cronograma; las sesiones con fecha son SesionRetoAcademico (HU03).
    cronograma_sesiones = models.TextField("Notas del cronograma", blank=True)
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

    # Minimo academico que el profesor debe definir antes de pedir aprobacion.
    CAMPOS_REQUERIDOS_REVISION = [
        ("facultad", "Facultad"),
        ("programa_academico", "Programa academico"),
        ("nivel_formacion", "Nivel de formacion"),
        ("descripcion", "Descripcion"),
        ("alcance", "Alcance"),
        ("entregable_esperado", "Entregable esperado"),
    ]
    # HU03: sesiones obligatorias del cronograma (la preseleccion es opcional).
    SESIONES_REQUERIDAS = [
        ("inicio", "una sesion de inicio"),
        ("seguimiento", "al menos una sesion de seguimiento"),
        ("evaluacion", "una sesion de evaluacion"),
    ]

    def campos_faltantes_para_revision(self):
        """Lo que falta diligenciar para enviar la integracion a revision."""
        faltantes = [
            etiqueta
            for campo, etiqueta in self.CAMPOS_REQUERIDOS_REVISION
            if not (getattr(self, campo, "") or "").strip()
        ]
        if self.pk is None:
            return faltantes + ["al menos un profesor en el equipo"] + [e for _, e in self.SESIONES_REQUERIDAS]
        if not self.participantes.filter(rol=ParticipanteIntegracion.PROFESOR).exists():
            faltantes.append("al menos un profesor en el equipo")
        tipos = set(self.sesiones.values_list("tipo", flat=True))
        faltantes += [etiqueta for tipo, etiqueta in self.SESIONES_REQUERIDAS if tipo not in tipos]
        return faltantes

    @property
    def puede_editar_profesor(self):
        # HU03: la configuracion sigue editable mientras el reto esta abierto o en curso.
        return self.estado in {"borrador", "rechazada"} or self.reto.estado in {"abierto", "en_curso"}

    def clean(self):
        if self.profesor_id and getattr(self.profesor, "rol", None) != "PROFESOR":
            raise ValidationError({"profesor": "La integracion debe pertenecer a un usuario Profesor."})
        if self.reto_id and self.reto.estado not in self.reto.ESTADOS_INTEGRABLES:
            raise ValidationError({"reto": "Solo se pueden integrar retos aprobados, abiertos o en curso."})


class ParticipanteIntegracion(models.Model):
    """Integrante del equipo de una integracion academica (HU03).

    Profesores: nombre, cedula, correo y facultad. Estudiantes: nombre, documento,
    correo y programa. Expertos invitados (opcionales): nombre, cedula, correo y
    facultad u organizacion. Si el correo corresponde a una cuenta de la
    plataforma, queda enlazada para poder notificarla.
    """

    PROFESOR = "PROFESOR"
    ESTUDIANTE = "ESTUDIANTE"
    EXPERTO = "EXPERTO"
    ROLES = [(PROFESOR, "Profesor"), (ESTUDIANTE, "Estudiante"), (EXPERTO, "Experto invitado")]

    integracion = models.ForeignKey(IntegracionAcademica, on_delete=models.CASCADE, related_name="participantes")
    rol = models.CharField(max_length=12, choices=ROLES)
    nombre = models.CharField(max_length=150)
    documento = models.CharField("Cedula / documento", max_length=30, blank=True)
    email = models.EmailField("Correo", blank=True)
    facultad = models.CharField("Facultad u organizacion", max_length=150, blank=True)
    programa = models.CharField(max_length=160, blank=True)
    usuario = models.ForeignKey(
        "usuarios.Usuario", on_delete=models.SET_NULL, null=True, blank=True,
        related_name="participaciones_integracion",
    )
    notificado_en = models.DateTimeField(null=True, blank=True)

    class Meta:
        db_table = "participantes_integracion"
        verbose_name = "Participante de integracion"
        verbose_name_plural = "Participantes de integracion"
        ordering = ["rol", "nombre"]

    def __str__(self):
        return f"{self.nombre} ({self.get_rol_display()})"

    def save(self, *args, **kwargs):
        # Enlaza la cuenta de la plataforma cuando el correo identifica a una sola.
        if self.email and self.usuario_id is None:
            from apps.usuarios.models import Usuario

            cuentas = list(Usuario.objects.filter(email__iexact=self.email)[:2])
            if len(cuentas) == 1:
                self.usuario = cuentas[0]
        super().save(*args, **kwargs)


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


class HistorialEstado(models.Model):
    """Bitacora unica de cambios de estado de cualquier entidad del sistema.

    Documentos de empresa, convenios, integraciones, postulaciones, entregables,
    inscripciones de hackathon y cuentas de usuario escriben aqui a traves de
    `apps.seguimiento.services.cambiar_estado`. Los retos conservan su propio
    `retos.HistorialEstadoReto`, que ya muestra la UI del detalle del reto.
    """

    content_type = models.ForeignKey(
        "contenttypes.ContentType", on_delete=models.CASCADE, related_name="+"
    )
    object_id = models.PositiveBigIntegerField()
    objeto = GenericForeignKey("content_type", "object_id")
    campo = models.CharField(max_length=50, default="estado")
    estado_anterior = models.CharField(max_length=30, blank=True)
    estado_nuevo = models.CharField(max_length=30)
    comentario = models.TextField(blank=True)
    realizado_por = models.ForeignKey(
        "usuarios.Usuario",
        on_delete=models.SET_NULL,
        null=True,
        blank=True,
        related_name="cambios_estado",
    )
    fecha = models.DateTimeField(auto_now_add=True, db_index=True)

    class Meta:
        db_table = "historial_estados"
        ordering = ["-fecha", "-id"]
        indexes = [models.Index(fields=["content_type", "object_id"])]
        verbose_name = "Historial de estado"
        verbose_name_plural = "Historial de estados"

    def __str__(self):
        return f"{self.content_type.model}#{self.object_id}: {self.estado_anterior} -> {self.estado_nuevo}"
