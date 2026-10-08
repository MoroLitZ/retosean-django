import uuid

from django.db import models


class Facultad(models.Model):
    nombre = models.CharField(max_length=150, unique=True)
    codigo = models.CharField(max_length=20, blank=True)

    class Meta:
        db_table = "facultades"
        verbose_name = "Facultad"
        verbose_name_plural = "Facultades"

    def __str__(self):
        return self.nombre


class Programa(models.Model):
    nombre = models.CharField(max_length=150)
    facultad = models.ForeignKey(Facultad, on_delete=models.CASCADE, related_name="programas")
    nivel = models.CharField(max_length=50, blank=True)

    class Meta:
        db_table = "programas"
        verbose_name = "Programa"
        verbose_name_plural = "Programas"
        unique_together = (("nombre", "facultad"),)

    def __str__(self):
        return f"{self.nombre} - {self.facultad.nombre}"


class Ecosistema(models.Model):
    nombre = models.CharField(max_length=150, unique=True)
    descripcion = models.TextField(blank=True)

    class Meta:
        db_table = "ecosistemas"
        verbose_name = "Ecosistema"
        verbose_name_plural = "Ecosistemas"

    def __str__(self):
        return self.nombre


class Profesor(models.Model):
    usuario = models.OneToOneField("usuarios.Usuario", on_delete=models.CASCADE, related_name="perfil_profesor")
    facultad = models.ForeignKey(Facultad, on_delete=models.SET_NULL, null=True, blank=True)
    especialidad = models.CharField(max_length=100, blank=True)

    class Meta:
        db_table = "profesores"
        verbose_name = "Profesor"
        verbose_name_plural = "Profesores"

    def __str__(self):
        return self.usuario.get_full_name() or self.usuario.username


class Estudiante(models.Model):
    usuario = models.OneToOneField("usuarios.Usuario", on_delete=models.CASCADE, related_name="perfil_estudiante")
    programa = models.ForeignKey(Programa, on_delete=models.SET_NULL, null=True, blank=True)
    semestre = models.IntegerField(default=1)

    class Meta:
        db_table = "estudiantes"
        verbose_name = "Estudiante"
        verbose_name_plural = "Estudiantes"

    def __str__(self):
        return self.usuario.get_full_name() or self.usuario.username


class Certificado(models.Model):
    """Certificado de participacion en un reto finalizado (HU16).

    Se emite al cerrar el reto. El PDF se genera bajo demanda y se cachea en
    `archivo`; `codigo_verificacion` permite validarlo publicamente sin login.
    """

    ROLES = [
        ("ESTUDIANTE", "Estudiante"),
        ("PROFESOR", "Docente"),
        ("EMPRESA", "Empresa"),
        ("JURADO", "Jurado"),
    ]
    reto = models.ForeignKey("retos.Reto", on_delete=models.CASCADE, related_name="certificados")
    usuario = models.ForeignKey(
        "usuarios.Usuario", on_delete=models.CASCADE, related_name="certificados"
    )
    rol_participacion = models.CharField(max_length=20, choices=ROLES)
    codigo_verificacion = models.UUIDField(default=uuid.uuid4, unique=True, editable=False)
    horas = models.PositiveIntegerField(null=True, blank=True)
    emitido_en = models.DateTimeField(auto_now_add=True)
    emitido_por = models.ForeignKey(
        "usuarios.Usuario", on_delete=models.SET_NULL, null=True, blank=True,
        related_name="certificados_emitidos",
    )
    archivo = models.FileField(upload_to="certificados/", null=True, blank=True)

    class Meta:
        db_table = "certificados"
        verbose_name = "Certificado"
        verbose_name_plural = "Certificados"
        unique_together = ["reto", "usuario", "rol_participacion"]
        ordering = ["-emitido_en"]

    def __str__(self):
        return f"Certificado de {self.usuario.username} - {self.reto.titulo}"

    @property
    def nombre_participante(self):
        return self.usuario.get_full_name() or self.usuario.username

    @property
    def codigo_corto(self):
        return str(self.codigo_verificacion).split("-")[0].upper()
