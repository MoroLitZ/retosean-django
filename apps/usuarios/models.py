from django.contrib.auth.models import AbstractUser
from django.db import models

class Usuario(AbstractUser):
    ROL_CHOICES = [
        ('ADMIN', 'Administrador'),
        ('EMPRESA', 'Empresa'),
        ('PROFESOR', 'Profesor'),
        ('ESTUDIANTE', 'Estudiante'),
    ]
    rol = models.CharField(max_length=20, choices=ROL_CHOICES)
    activo = models.BooleanField(default=True)

    def __str__(self):
        return f"{self.get_full_name()} ({self.rol})"