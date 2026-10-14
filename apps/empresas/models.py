import os
from django.db import models
from django.core.exceptions import ValidationError


def ruta_documentos_empresa(instance, filename):
    ext = filename.split(".")[-1]
    nit = getattr(instance.empresa, 'nit', 'sin_nit')
    tipo_doc = instance.tipo_documento.upper().replace(" ", "_")
    return os.path.join("documentos_empresas", str(nit), f"{nit}_{tipo_doc}.{ext}")


class Empresa(models.Model):
    nit = models.CharField(max_length=20, unique=True)
    razon_social = models.CharField(max_length=150)
    sector_industrial = models.CharField(max_length=100)
    # HU01: datos del registro de la organizacion.
    TIPOS_CONVOCATORIA = [
        ("RETO", "Reto"),
        ("HACKATHON", "Hackathon"),
        ("AMBOS", "Reto y hackathon"),
    ]
    tipo_convocatoria = models.CharField(max_length=20, choices=TIPOS_CONVOCATORIA, default="RETO")
    area_responsable = models.CharField(max_length=150, blank=True)
    sitio_web = models.URLField(blank=True)
    fecha_creacion = models.DateField(auto_now_add=True)
    ESTADO_VALIDACION = [
    ('PENDIENTE', 'Pendiente'),
    ('EN_REVISION', 'En Revisión'),
    ('VERIFICADA', 'Verificada'),
    ('RECHAZADA', 'Rechazada'),]
    estado_validacion = models.CharField(max_length=20, choices=ESTADO_VALIDACION, default='PENDIENTE')
    ESTADO_LISTAS = [
        ('PENDIENTE', 'Pendiente de revisión'),
        ('APROBADO', 'No aparece en listas'),
        ('RECHAZADO', 'Aparece en listas restrictivas'),
    ]
    estado_listas_restrictivas = models.CharField(max_length=20, choices=ESTADO_LISTAS, default='PENDIENTE')
    evidencia_listas = models.FileField(upload_to="evidencias_listas/", null=True, blank=True)
    fecha_ultima_actualizacion = models.DateTimeField(auto_now=True)
    observaciones_admin = models.TextField(null=True, blank=True)
    renuncio_a_convenio = models.BooleanField(default=False)
    # Tramite del convenio marco (HU00). Las transiciones validas viven en
    # `empresas.services.MAQUINA_CONVENIO`; una vez firmado se registra su vencimiento.
    ESTADO_CONVENIO = [
        ("NO_RADICADO", "No radicado"),
        ("RADICADO", "Radicado"),
        ("EN_JURIDICA", "En proceso en Jurídica"),
        ("FIRMADO", "Firmado por las partes"),
    ]
    estado_convenio = models.CharField(max_length=20, choices=ESTADO_CONVENIO, default="NO_RADICADO")
    fecha_vencimiento_convenio = models.DateField(null=True, blank=True)
    # Bloqueo manual del administrador (HU00): la empresa no puede participar
    # aunque tenga la documentacion al dia.
    participacion_bloqueada = models.BooleanField(default=False)
    motivo_bloqueo = models.TextField(blank=True)
    usuario = models.OneToOneField(
        "usuarios.Usuario",
        on_delete=models.CASCADE,
        related_name="empresa_perfil",
        null=True,
        blank=True,
    )

    class Meta:
        db_table = "empresas"
        verbose_name = "Empresa"
        verbose_name_plural = "Empresas"

    def __str__(self):
        return self.razon_social

    @property
    def tiene_convenio_vigente(self):
        """Si: convenio firmado y no vencido. No: cualquier otro caso."""
        if self.estado_convenio != "FIRMADO":
            return False
        from django.utils import timezone

        return self.fecha_vencimiento_convenio is None or self.fecha_vencimiento_convenio >= timezone.localdate()


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


class TipoDocumentoLegal(models.Model):
    """Catalogo administrable de documentos legales que se piden a las empresas (HU00).

    El administrador decide cuales son obligatorios, su vigencia maxima y si
    solo puede cargarlos un usuario con permiso especial (Listas Informa).
    """

    codigo = models.CharField(max_length=50, unique=True)
    nombre = models.CharField(max_length=150)
    descripcion = models.TextField(blank=True)
    obligatorio = models.BooleanField(default=True)
    # Documentos propios del convenio (minuta, convenio firmado): no se exigen
    # a la empresa que eligio participar sin convenio.
    aplica_sin_convenio = models.BooleanField(default=True)
    # Antiguedad maxima de la fecha de expedicion; tambien fija el vencimiento.
    vigencia_dias = models.PositiveSmallIntegerField(null=True, blank=True)
    # Solo lo carga un usuario con el permiso `empresas.cargar_listas_informa`.
    requiere_permiso_especial = models.BooleanField(default=False)
    activo = models.BooleanField(default=True)
    orden = models.PositiveSmallIntegerField(default=0)

    class Meta:
        db_table = "tipos_documento_legal"
        ordering = ["orden", "nombre"]
        verbose_name = "Tipo de documento legal"
        verbose_name_plural = "Tipos de documento legal"

    def __str__(self):
        return self.nombre

    def es_requerido_para(self, empresa):
        return self.activo and self.obligatorio and (self.aplica_sin_convenio or not empresa.renuncio_a_convenio)


class DocumentoEmpresa(models.Model):
    # Formatos aceptados para cualquier documento legal.
    EXTENSIONES_PERMITIDAS = ("pdf",)
    ESTADOS = [
        ("PENDIENTE", "Pendiente"),
        ("CARGADO", "En Revision"),
        ("VERIFICADO", "Aprobado"),
        ("RECHAZADO", "Rechazado"),
        ("VENCIDO", "Vencido"),
    ]

    empresa = models.ForeignKey(Empresa, on_delete=models.CASCADE, related_name="documentos")
    # Codigo de `TipoDocumentoLegal`. Se guarda el codigo (no una FK) para que
    # el historial y las rutas de archivo no dependan de que el tipo siga activo.
    tipo_documento = models.CharField(max_length=50)
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
        permissions = [
            ("cargar_listas_informa", "Puede cargar documentos de permiso especial (Listas Informa)"),
        ]

    @property
    def tipo(self):
        """`TipoDocumentoLegal` del catalogo, o None si el codigo ya no existe."""
        if not hasattr(self, "_tipo_cache"):
            self._tipo_cache = TipoDocumentoLegal.objects.filter(codigo=self.tipo_documento).first()
        return self._tipo_cache

    def get_tipo_documento_display(self):
        return self.tipo.nombre if self.tipo else self.tipo_documento

    @property
    def esta_vigente(self):
        from django.utils import timezone

        return self.fecha_vencimiento is None or self.fecha_vencimiento >= timezone.localdate()

    def clean(self):
        super().clean()
        tipo = self.tipo
        if tipo is None:
            raise ValidationError({"tipo_documento": "Tipo de documento desconocido."})
        if tipo.vigencia_dias and not self.fecha_expedicion:
            raise ValidationError({"fecha_expedicion": f"{tipo.nombre} requiere fecha de expedición."})

    def save(self, *args, **kwargs):
        self.full_clean()
        
        if self.pk:
            try:
                anterior = DocumentoEmpresa.objects.get(id=self.id)
            except DocumentoEmpresa.DoesNotExist:
                anterior = None
            # Solo intentamos borrar si realmente habia archivo previo:
            # sobre un FileField vacio, .path lanza ValueError.
            if anterior and anterior.archivo and anterior.archivo != self.archivo:
                try:
                    if os.path.isfile(anterior.archivo.path):
                        os.remove(anterior.archivo.path)
                except (ValueError, NotImplementedError, OSError):
                    pass
        super().save(*args, **kwargs)

    def __str__(self):
        return f"{self.empresa.razon_social} - {self.get_tipo_documento_display()} ({self.estado})"
