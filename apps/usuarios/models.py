from django.contrib.auth.models import AbstractUser
from django.db import models

class Usuario(AbstractUser):
    ROL_CHOICES = [
        ('ADMIN', 'Administrador'),
        ('EMPRESA', 'Empresa'),
        ('PROFESOR', 'Profesor'),
        ('ESTUDIANTE', 'Estudiante'),
    ]
    rol = models.CharField(max_length=20, choices=ROL_CHOICES, default='ESTUDIANTE')
    telefono = models.CharField(max_length=20, blank=True)
    activo = models.BooleanField(default=True)
    fecha_creacion = models.DateTimeField(auto_now_add=True)

    def es_administrador(self):
        return self.rol == 'ADMIN'

    def es_empresa(self):
        return self.rol == 'EMPRESA'

    def es_profesor(self):
        return self.rol == 'PROFESOR'

    def es_estudiante(self):
        return self.rol == 'ESTUDIANTE'
    
    def __str__(self):
        return f"{self.get_full_name() or self.username} ({self.get_rol_display()}"