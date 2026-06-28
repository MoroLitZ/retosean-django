from django.contrib.auth.models import AbstractUser
from django.db import models
import os

from django.utils import timezone

class Empresa(models.Model):
    nit = models.CharField(max_length=20, unique=True)
    razon_social = models.CharField(max_length=150)
    sector_industrial = models.CharField(max_length=100)
    fecha_creacion = models.DateField(auto_now_add=True)

    class Meta:
        db_table = 'empresas'
        verbose_name = "Empresa"
        verbose_name_plural = "Empresas"

    def __str__(self):
        return "Empresa: {self.razon_social}"



class Usuario(AbstractUser):
    ROL_CHOICES = [
        ('ADMIN', 'Administrador'),
        ('EMPRESA', 'Empresa'),
        ('PROFESOR', 'Profesor'),
        ('ESTUDIANTE', 'Estudiante'),
    ]
    rol = models.CharField(max_length=20, choices=ROL_CHOICES)
    telefono = models.CharField(max_length=20, blank=True)
    activo = models.BooleanField(default=True)
    fecha_creacion = models.DateTimeField(auto_now_add=True)
    empresa = models.ForeignKey(Empresa, on_delete=models.CASCADE, null=True, blank=True, related_name='usuarios')
    es_admin_empresa = models.BooleanField(default=False)

    class Meta:
        db_table = 'usuarios'
        verbose_name = "Usuario"
        verbose_name_plural = "Usuarios"

    def __str__(self):
        return f"{self.username} ({self.rol})"


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


#  se determina la ruta y el nombre del archivo para organizarlo por el NIT de la empresa
def ruta_documentos_empresa(instance, filename):
    ext = filename.split('.')[-1]
    tipo_doc = instance.tipo_documento.upper().replace(" ", "_")
    return os.path.join('documentos_empresas', instance.empresa.nit, f"{instance.empresa.nit}_{tipo_doc}.{ext}")

# genera una tabla en postgres que gestiona archivos, tipo, estado y vigencia legal de la empresa
class DocumentoEmpresa(models.Model):
    TIPO_DOCUMENTO_CHOICES = [
        ('CAMARA_COMERCIO', 'Cámara de Comercio (No mayor a 30 días)'),
        ('RUT', 'Registro Único Tributario (RUT)'),
        ('CEDULA_REPRESENTANTE', 'Cédula del Representante Legal'),
        ('CONVENIO', 'Convenio Institucional / Prácticas')
    ]

    ESTADOS = [
        ('PENDIENTE', 'Pendiente'),
        ('CARGADO', 'En Revisión'),
        ('VERIFICADO', 'Aprobado'),
        ('RECHAZADO', 'Rechazado'),
    ]

    empresa = models.ForeignKey(Empresa, on_delete=models.CASCADE, related_name='documentos')
    tipo_documento = models.CharField(max_length=50, choices=TIPO_DOCUMENTO_CHOICES)
    archivo = models.FileField(upload_to=ruta_documentos_empresa, null=True, blank=True)
    estado = models.CharField(max_length=15, choices=ESTADOS, default='PENDIENTE')
    fecha_expedicion = models.DateField(null=True, blank=True, help_text="Fecha en que se emitió el documento")
    fecha_carga = models.DateTimeField(auto_now_add=True)
    motivo_rechazo = models.TextField(null=True, blank=True, help_text="Explicación del administrador si el documento es rechazado")

   # clase de configuración sobre el comportamiento de las tablas
    class Meta:
        db_table = 'documentos_empresas'
        verbose_name = "Documento de Empresa"
        verbose_name_plural = "Documentos de Empresa"
        unique_together = ['empresa', 'tipo_documento']

    # se elimina el archivo viejo si se sube uno nuevo
    def save(self, *args, **kwargs):
        try:
            this = DocumentoEmpresa.objects.get(id=self.id)
            if this.archivo != self.archivo:
                if os.path.isfile(this.archivo.path):
                    os.remove(this.archivo.path)
        except DocumentoEmpresa.DoesNotExist:
            pass
        super(DocumentoEmpresa, self).save(*args, **kwargs)

    def __str__(self):
        return f"{self.empresa.razon_social} - {self.get_tipo_documento_display()} ({self.estado})"
    
    
