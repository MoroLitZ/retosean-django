from django.db import models


class Postulacion(models.Model):
    ESTADOS = [
        ("PENDIENTE", "Pendiente de Aprobacion"),
        ("ACEPTADA", "Aceptada"),
        ("RECHAZADA", "Rechazada"),
        ("RETIRADA", "Retirada"),
    ]
    reto = models.ForeignKey("retos.Reto", on_delete=models.CASCADE, related_name="postulaciones")
    estudiante = models.ForeignKey("usuarios.Usuario", on_delete=models.CASCADE, related_name="postulaciones")
    fecha_postulacion = models.DateTimeField(auto_now_add=True)
    estado = models.CharField(max_length=20, choices=ESTADOS, default="PENDIENTE")
    motivacion = models.TextField(blank=True)
    habilidades = models.TextField(blank=True, help_text="Habilidades y conocimientos relevantes")
    semestre = models.IntegerField(null=True, blank=True)
    programa = models.CharField(max_length=150, blank=True, help_text="Programa academico del estudiante")

    class Meta:
        db_table = "postulaciones"
        verbose_name = "Postulacion"
        verbose_name_plural = "Postulaciones"
        unique_together = ["reto", "estudiante"]

    def __str__(self):
        return f"{self.estudiante.username} -> Reto {self.reto_id} ({self.estado})"


class Equipo(models.Model):
    reto = models.ForeignKey("retos.Reto", on_delete=models.CASCADE, related_name="equipos")
    nombre = models.CharField(max_length=100)
    creado_en = models.DateTimeField(auto_now_add=True)

    class Meta:
        db_table = "equipos"
        verbose_name = "Equipo"
        verbose_name_plural = "Equipos"

    def __str__(self):
        return f"{self.nombre} - Reto {self.reto_id}"


class MiembroEquipo(models.Model):
    ROLES = [
        ("LIDER", "Lider"),
        ("MIEMBRO", "Miembro"),
    ]
    equipo = models.ForeignKey(Equipo, on_delete=models.CASCADE, related_name="miembros")
    estudiante = models.ForeignKey("usuarios.Usuario", on_delete=models.CASCADE, related_name="equipos")
    rol = models.CharField(max_length=20, choices=ROLES, default="MIEMBRO")
    fecha_ingreso = models.DateTimeField(auto_now_add=True)

    class Meta:
        db_table = "miembros_equipo"
        verbose_name = "Miembro de Equipo"
        verbose_name_plural = "Miembros de Equipo"
        unique_together = ["equipo", "estudiante"]

    def __str__(self):
        return f"{self.estudiante.username} en {self.equipo.nombre}"


class RetoFavorito(models.Model):
    usuario = models.ForeignKey("usuarios.Usuario", on_delete=models.CASCADE, related_name="favoritos")
    reto = models.ForeignKey("retos.Reto", on_delete=models.CASCADE, related_name="favoritos")
    fecha = models.DateTimeField(auto_now_add=True)

    class Meta:
        db_table = "retos_favoritos"
        verbose_name = "Reto Favorito"
        verbose_name_plural = "Retos Favoritos"
        unique_together = ["usuario", "reto"]

    def __str__(self):
        return f"{self.usuario.username} - Reto {self.reto_id}"
