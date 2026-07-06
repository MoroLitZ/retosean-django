import os
from django.db import models


def ruta_documentos_empresa(instance, filename):
    ext = filename.split(".")[-1]
    tipo_doc = instance.tipo_documento.upper().replace(" ", "_")
    return os.path.join("documentos_empresas", instance.empresa.nit, f"{instance.empresa.nit}_{tipo_doc}.{ext}")


class Empresa(models.Model):
    nit = models.CharField(max_length=20, unique=True)
    razon_social = models.CharField(max_length=150)
    sector_industrial = models.CharField(max_length=100)
    fecha_creacion = models.DateField(auto_now_add=True)

    class Meta:
        db_table = "empresas"
        verbose_name = "Empresa"
        verbose_name_plural = "Empresas"

    def __str__(self):
        return self.razon_social


class ContactoEmpresa(models.Model):
    empresa = models.ForeignKey(Empresa, on_delete=models.CASCADE, related_name="contactos")
    nombre = models.CharField(max_length=100)
    cargo = models.CharField(max_length=100, blank=True)
    email = models.EmailField()
    telefono = models.CharField(max_length=20, blank=True)
    es_principal = models.BooleanField(default=False)

    class Meta:
        db_table = "contactos_empresa"
        verbose_name = "Contacto de Empresa"
        verbose_name_plural = "Contactos de Empresa"

    def __str__(self):
        return f"{self.nombre} - {self.empresa.razon_social}"


class DocumentoEmpresa(models.Model):
    TIPO_DOCUMENTO_CHOICES = [
        ("CAMARA_COMERCIO", "Camara de Comercio"),
        ("RUT", "Registro Unico Tributario (RUT)"),
        ("CEDULA_REPRESENTANTE", "Cedula del Representante Legal"),
        ("CONVENIO", "Convenio Institucional"),
        ("MINUTA_CONVENIO", "Minuta Convenio"),
        ("LISTAS_INFORMA", "Listas Informa"),
        ("COMPROMISO_ESTUDIANTES", "Formato Compromiso Estudiantes"),
        ("TERMINOS_REFERENCIA", "Terminos de Referencia"),
        ("FICHA_RETO_ISO", "Ficha del Reto ISO"),
    ]
    ESTADOS = [
        ("PENDIENTE", "Pendiente"),
        ("CARGADO", "En Revision"),
        ("VERIFICADO", "Aprobado"),
        ("RECHAZADO", "Rechazado"),
        ("VENCIDO", "Vencido"),
    ]

    empresa = models.ForeignKey(Empresa, on_delete=models.CASCADE, related_name="documentos")
    tipo_documento = models.CharField(max_length=50, choices=TIPO_DOCUMENTO_CHOICES)
    archivo = models.FileField(upload_to=ruta_documentos_empresa, null=True, blank=True)
    estado = models.CharField(max_length=15, choices=ESTADOS, default="PENDIENTE")
    fecha_expedicion = models.DateField(null=True, blank=True)
    fecha_vencimiento = models.DateField(null=True, blank=True)
    fecha_carga = models.DateTimeField(auto_now_add=True)
    motivo_rechazo = models.TextField(null=True, blank=True)

    class Meta:
        db_table = "documentos_empresas"
        verbose_name = "Documento de Empresa"
        verbose_name_plural = "Documentos de Empresa"
        unique_together = ["empresa", "tipo_documento"]

    def save(self, *args, **kwargs):
        try:
            this = DocumentoEmpresa.objects.get(id=self.id)
            if this.archivo != self.archivo:
                if os.path.isfile(this.archivo.path):
                    os.remove(this.archivo.path)
        except DocumentoEmpresa.DoesNotExist:
            pass
        super().save(*args, **kwargs)

    def __str__(self):
        return f"{self.empresa.razon_social} - {self.get_tipo_documento_display()} ({self.estado})"
