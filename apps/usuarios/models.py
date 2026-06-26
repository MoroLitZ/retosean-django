from django.contrib.auth.models import AbstractUser
from django.db import models
import os

class Usuario(AbstractUser):
    ROL_CHOICES = [
        ('EMPRESA', 'Empresa'),
        ('PROFESOR', 'Profesor'),
        ('ESTUDIANTE', 'Estudiante'),
    ]
    rol = models.CharField(max_length=20, choices=ROL_CHOICES)
    telefono = models.CharField(max_length=20, blank=True)
    activo = models.BooleanField(default=True)
    fecha_creacion = models.DateTimeField(auto_now_add=True)

class PerfilEstudiante(models.Model):
    usuario = models.OneToOneField(Usuario, on_delete=models.CASCADE, related_name='perfil_estudiante')
    carrera = models.CharField(max_length=100)
    semestre = models.IntegerField()

    def __str__(self):
        return f"Estudiante: {self.usuario.username}"

class PerfilProfesor(models.Model):
    usuario = models.OneToOneField(Usuario, on_delete=models.CASCADE, related_name='perfil_profesor')
    facultad = models.CharField(max_length=100)
    especialidad = models.CharField(max_length=100)

    def __str__(self):
        return f"Profesor: {self.usuario.username}"

class PerfilEmpresa(models.Model):
    usuario = models.OneToOneField(Usuario, on_delete=models.CASCADE, related_name='perfil_empresa')
    nit = models.CharField(max_length=20, unique=True)
    razon_social = models.CharField(max_length=150)
    sector_industrial = models.CharField(max_length=100)

    def __str__(self):
        return f"Empresa: {self.razon_social}"


def ruta_documentos_empresa(instance, filename):
    ext = filename.split('.')[-1]
    nit = instance.perfil_empresa.nit.replace('-', '')
    return os.path.join('documentos_empresas', nit, f"{nit}_{instance.tipo_documento}.{ext}")


class DocumentoEmpresa(models.Model):
    TIPO_DOCUMENTO_CHOICES = [
        ('CAMARA_COMERCIO', 'Cámara de Comercio (No mayor a 30 días)'),
        ('RUT', 'Registro Único Tributario (RUT)'),
        ('CEDULA_REPRESENTANTE', 'Cédula del Representante Legal'),
        ('CONVENIO', 'Convenio Institucional / Prácticas'),
    ]
    ESTADOS = [
        ('PENDIENTE', 'Pendiente'),
        ('CARGADO', 'En Revisión'),
        ('VERIFICADO', 'Aprobado'),
        ('RECHAZADO', 'Rechazado'),
    ]

    perfil_empresa = models.ForeignKey(PerfilEmpresa, on_delete=models.CASCADE, related_name='documentos')
    tipo_documento = models.CharField(max_length=50, choices=TIPO_DOCUMENTO_CHOICES)
    archivo = models.FileField(upload_to=ruta_documentos_empresa, null=True, blank=True)
    estado = models.CharField(max_length=15, choices=ESTADOS, default='PENDIENTE')
    fecha_expedicion = models.DateField(null=True, blank=True, help_text="Fecha en que se emitió el documento")
    fecha_carga = models.DateTimeField(auto_now_add=True)
    motivo_rechazo = models.TextField(null=True, blank=True)

    class Meta:
        db_table = 'documentos_empresas'
        verbose_name = "Documento de Empresa"
        verbose_name_plural = "Documentos de Empresa"
        unique_together = ['perfil_empresa', 'tipo_documento']

    def save(self, *args, **kwargs):
        try:
            existente = DocumentoEmpresa.objects.get(id=self.id)
            if existente.archivo != self.archivo and existente.archivo:
                if os.path.isfile(existente.archivo.path):
                    os.remove(existente.archivo.path)
        except DocumentoEmpresa.DoesNotExist:
            pass
        super().save(*args, **kwargs)

    def __str__(self):
        return f"{self.perfil_empresa.razon_social} - {self.get_tipo_documento_display()} ({self.estado})"