from django.db import models


class Hackathon(models.Model):
    ESTADOS = [
        ("borrador", "Borrador"),
        ("publicado", "Publicado"),
        ("en_curso", "En Curso"),
        ("finalizado", "Finalizado"),
        ("cancelado", "Cancelado"),
    ]
    reto = models.OneToOneField("retos.Reto", on_delete=models.CASCADE, related_name="hackathon")
    reglas = models.TextField(blank=True)
    estado = models.CharField(max_length=20, choices=ESTADOS, default="borrador")
    resultados_publicados = models.BooleanField(default=False)
    creado_en = models.DateTimeField(auto_now_add=True)
    actualizado_en = models.DateTimeField(auto_now=True)

    class Meta:
        db_table = "hackathons"
        verbose_name = "Hackathon"
        verbose_name_plural = "Hackathons"

    def __str__(self):
        return f"Hackathon: {self.reto.titulo}"


class EtapaHackaton(models.Model):
    TIPOS = [
        ("inscripcion", "Inscripcion"),
        ("desarrollo", "Desarrollo"),
        ("presentacion", "Presentacion"),
        ("premiacion", "Premiacion"),
    ]
    hackathon = models.ForeignKey(Hackathon, on_delete=models.CASCADE, related_name="etapas")
    tipo = models.CharField(max_length=30, choices=TIPOS)
    titulo = models.CharField(max_length=150)
    inicia_en = models.DateTimeField(null=True, blank=True)
    termina_en = models.DateTimeField(null=True, blank=True)
    orden = models.PositiveSmallIntegerField(default=0)

    class Meta:
        db_table = "etapas_hackaton"
        verbose_name = "Etapa de Hackaton"
        verbose_name_plural = "Etapas de Hackaton"
        ordering = ["orden"]

    def __str__(self):
        return f"{self.hackathon} - {self.titulo}"


class Jurado(models.Model):
    hackathon = models.ForeignKey(Hackathon, on_delete=models.CASCADE, related_name="jurados")
    usuario = models.ForeignKey("usuarios.Usuario", on_delete=models.SET_NULL, null=True, blank=True, related_name="jurados")
    nombre = models.CharField(max_length=150)
    email = models.EmailField(blank=True)
    especialidad = models.CharField(max_length=150, blank=True)

    class Meta:
        db_table = "jurados_hackaton"
        verbose_name = "Jurado"
        verbose_name_plural = "Jurados"

    def __str__(self):
        return f"{self.nombre} - {self.hackathon}"


class VotacionHackaton(models.Model):
    hackathon = models.ForeignKey(Hackathon, on_delete=models.CASCADE, related_name="votaciones")
    jurado = models.ForeignKey(Jurado, on_delete=models.CASCADE, related_name="votaciones")
    equipo = models.ForeignKey("participaciones.Equipo", on_delete=models.CASCADE, related_name="votaciones")
    puntaje = models.DecimalField(max_digits=4, decimal_places=2)
    comentario = models.TextField(blank=True)
    fecha = models.DateTimeField(auto_now_add=True)

    class Meta:
        db_table = "votaciones_hackaton"
        verbose_name = "Votacion Hackaton"
        verbose_name_plural = "Votaciones Hackaton"
        unique_together = ["jurado", "equipo"]

    def __str__(self):
        return f"{self.jurado} -> {self.equipo} ({self.puntaje})"
