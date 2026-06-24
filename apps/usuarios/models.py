from django.contrib.auth.models import AbstractUser
from django.db import models

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