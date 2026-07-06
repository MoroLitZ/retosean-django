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
        return f"Profesor: {self.usuario.username}"


class Estudiante(models.Model):
    usuario = models.OneToOneField("usuarios.Usuario", on_delete=models.CASCADE, related_name="perfil_estudiante")
    programa = models.ForeignKey(Programa, on_delete=models.SET_NULL, null=True, blank=True)
    semestre = models.IntegerField(default=1)

    class Meta:
        db_table = "estudiantes"
        verbose_name = "Estudiante"
        verbose_name_plural = "Estudiantes"

    def __str__(self):
        return f"Estudiante: {self.usuario.username}"
