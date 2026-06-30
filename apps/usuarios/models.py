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
    
    
def ruta_entregables_estudiante(instance, filename):
    ext = filename.split('.')[-1]
    return os.path.join('entregables_retos', f"reto_{instance.reto_id}", f"estudiante_{instance.estudiante.username}.{ext}")


class Entregable(models.Model):
    ESTADOS_ENTREGABLE = [
        ('ENVIADO', 'Enviado'),
        ('EN_REVISION', 'En Revisión'),
        ('CALIFICADO', 'Calificado'),
    ]

    reto = models.ForeignKey('retos.Reto', on_delete=models.CASCADE, related_name='entregables')
    estudiante = models.ForeignKey(Usuario, on_delete=models.CASCADE, limit_choices_to={'rol': 'ESTUDIANTE'}, related_name='entregas')
    archivo = models.FileField(upload_to=ruta_entregables_estudiante)
    comentario_estudiante = models.TextField(blank=True, null=True)
    comentario_profesor = models.TextField(blank=True, null=True)
    nota = models.DecimalField(max_digits=4, decimal_places=2, null=True, blank=True)
    fecha_entrega = models.DateTimeField(auto_now_add=True)
    actualizado_en = models.DateTimeField(auto_now=True)
    estado = models.CharField(max_length=20, choices=ESTADOS_ENTREGABLE, default='ENVIADO')

    class Meta:
        db_table = 'entregables_estudiantes'
        verbose_name = "Entregable"
        verbose_name_plural = "Entregables"
        unique_together = ['reto', 'estudiante']

    def save(self, *args, **kwargs):
        try:
            this = Entregable.objects.get(id=self.id)
            if this.archivo != self.archivo:
                if os.path.isfile(this.archivo.path):
                    os.remove(this.archivo.path)
        except Entregable.DoesNotExist:
            pass
        super(Entregable, self).save(*args, **kwargs)

    def __str__(self):
        return f"Reto {self.reto_id} - Estudiante: {self.estudiante.username} ({self.estado})"

    @property
    def nombre_archivo(self):
        return os.path.basename(self.archivo.name)
    

class PostulacionReto(models.Model):
    ESTADOS_POSTULACION = [
        ('PENDIENTE', 'Pendiente de Aprobación'),
        ('ACEPTADA', 'Aceptada (Asignado)'),
        ('RECHAZADA', 'Rechazada'),
    ]

    reto = models.ForeignKey('retos.Reto', on_delete=models.CASCADE, related_name='postulaciones')
    estudiante = models.ForeignKey(Usuario, on_delete=models.CASCADE, limit_choices_to={'rol': 'ESTUDIANTE'}, related_name='postulaciones')
    fecha_postulacion = models.DateTimeField(auto_now_add=True)
    estado = models.CharField(max_length=20, choices=ESTADOS_POSTULACION, default='PENDIENTE')

    class Meta:
        db_table = 'postulaciones_retos'
        verbose_name = "Postulación a Reto"
        verbose_name_plural = "Postulaciones a Retos"
        unique_together = ['reto', 'estudiante']

    def __str__(self):
        return f"{self.estudiante.username} -> Reto {self.reto_id} ({self.estado})"