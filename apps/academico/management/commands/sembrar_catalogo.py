"""Siembra el catalogo academico base (facultades, programas, ecosistemas).

Idempotente: usa get_or_create, de modo que se puede ejecutar tantas veces como
se quiera sin duplicar filas.

    python manage.py sembrar_catalogo              # solo el catalogo
    python manage.py sembrar_catalogo --demo       # + usuarios por rol y un ciclo de ejemplo
    python manage.py sembrar_catalogo --dry-run    # no escribe, solo informa
"""

from django.core.management.base import BaseCommand, CommandError
from django.db import transaction

from apps.academico import catalogo_base


class Command(BaseCommand):
    help = "Siembra facultades, programas y ecosistemas. Con --demo, ademas crea usuarios por rol y un reto de ejemplo."

    def add_arguments(self, parser):
        parser.add_argument(
            "--demo",
            action="store_true",
            help="Ademas del catalogo, crea usuarios por rol y un reto de ejemplo aprobado.",
        )
        parser.add_argument(
            "--dry-run",
            action="store_true",
            help="Muestra lo que se crearia sin escribir nada en la base de datos.",
        )

    def handle(self, *args, **options):
        demo = options["demo"]
        dry_run = options["dry_run"]

        if dry_run:
            self.stdout.write(self.style.WARNING("MODO DRY-RUN: no se escribira nada.\n"))

        self._sembrar_catalogo(dry_run)
        if demo:
            self._sembrar_demo(dry_run)

        self.stdout.write(self.style.SUCCESS("Siembra completada."))

    # ------------------------------------------------------------------ catalogo
    def _sembrar_catalogo(self, dry_run):
        from apps.academico.models import Ecosistema, Facultad, Programa

        creadas_facultades = 0
        creados_programas = 0
        creados_ecosistemas = 0

        facultades = {}
        for nombre, codigo in catalogo_base.FACULTADES:
            if dry_run:
                facultades[nombre] = None
                if not Facultad.objects.filter(nombre=nombre).exists():
                    creadas_facultades += 1
                continue
            facultad, created = Facultad.objects.get_or_create(
                nombre=nombre, defaults={"codigo": codigo}
            )
            # Reapunta el codigo si la fila ya existia sin el.
            if not created and not facultad.codigo:
                facultad.codigo = codigo
                facultad.save(update_fields=["codigo"])
            if created:
                creadas_facultades += 1
            facultades[nombre] = facultad

        for nombre_programa, nombre_facultad in catalogo_base.PROGRAMAS:
            facultad = facultades[nombre_facultad]
            if dry_run:
                if not Programa.objects.filter(nombre=nombre_programa, facultad__nombre=nombre_facultad).exists():
                    creados_programas += 1
                continue
            # Un programa solo se identifica por (nombre, facultad); un duplicado
            # previo en otra facultad se resuelve en la migracion de normalizacion.
            _, created = Programa.objects.get_or_create(
                nombre=nombre_programa,
                facultad=facultad,
            )
            if created:
                creados_programas += 1

        for nombre, descripcion in catalogo_base.ECOSISTEMAS:
            if dry_run:
                if not Ecosistema.objects.filter(nombre=nombre).exists():
                    creados_ecosistemas += 1
                continue
            _, created = Ecosistema.objects.get_or_create(
                nombre=nombre, defaults={"descripcion": descripcion}
            )
            if created:
                creados_ecosistemas += 1

        self.stdout.write(
            f"Catalogo: {creadas_facultades} facultades, "
            f"{creados_programas} programas, {creados_ecosistemas} ecosistemas "
            f"{'por crear' if dry_run else 'creados'}."
        )

    # ------------------------------------------------------------------- demo
    def _sembrar_demo(self, dry_run):
        if dry_run:
            self.stdout.write("Demo: se crearian usuarios por rol y un reto de ejemplo.")
            return

        from apps.academico.models import Ecosistema, Estudiante, Facultad, Profesor, Programa
        from apps.empresas.models import Empresa
        from apps.retos.models import Reto
        from apps.retos.services import cambiar_estado_reto
        from apps.usuarios.models import Usuario

        demo_password = "RetosEAN2026"

        # Usuarios por rol, idempotentes por username.
        def _usuario(username, email, first_name, last_name, rol, **kwargs):
            user, created = Usuario.objects.get_or_create(
                username=username,
                defaults={
                    "email": email,
                    "first_name": first_name,
                    "last_name": last_name,
                    "rol": rol,
                    "is_active": True,
                    **kwargs,
                },
            )
            if created:
                user.set_password(demo_password)
                user.save(update_fields=["password"])
            return user, created

        admin, _ = _usuario(
            "admin.demo", "admin@universidadean.edu.co", "Admin", "Demo", "ADMIN",
            is_staff=True, is_superuser=True,
        )
        empresa_user, _ = _usuario(
            "empresa.demo", "contacto@empresademo.co", "Empresa", "Demo", "EMPRESA",
        )
        profesor_user, _ = _usuario(
            "profesor.demo", "profesor@universidadean.edu.co", "Profesor", "Demo", "PROFESOR",
        )
        estudiante_user, _ = _usuario(
            "estudiante.demo", "estudiante@universidadean.edu.co", "Estudiante", "Demo", "ESTUDIANTE",
        )

        # Perfiles por rol.
        Empresa.objects.get_or_create(
            usuario=empresa_user,
            defaults={
                "nit": "900000000-1",
                "razon_social": "Empresa Demo S.A.S.",
                "sector_industrial": "Tecnologia",
                "estado_validacion": "VERIFICADA",
                "estado_listas_restrictivas": "APROBADO",
            },
        )

        facultad = Facultad.objects.order_by("id").first()
        programa = Programa.objects.order_by("id").first()

        Profesor.objects.get_or_create(
            usuario=profesor_user,
            defaults={"facultad": facultad, "especialidad": "Ingenieria de Software"},
        )
        Estudiante.objects.get_or_create(
            usuario=estudiante_user,
            defaults={"programa": programa, "semestre": 6},
        )

        # Un reto de ejemplo aprobado, con su ciclo de estado pasado por el servicio
        # real para que la demo refleje exactamente lo que hace la interfaz.
        reto, created = Reto.objects.get_or_create(
            titulo="Reto de ejemplo: transformacion digital para pymes",
            defaults={
                "empresa": empresa_user,
                "tipo": "reto",
                "descripcion": "Disena una solucion digital para una pyme real.",
                "area": "Tecnología",
                "nivel_academico": "Pregrado",
                "facultad": facultad,
                "programa": programa,
                "ecosistema": Ecosistema.objects.order_by("id").first(),
                "premios": "Reconocimiento institucional y visibilidad.",
                "criterios_evaluacion": "Impacto, viabilidad y presentacion.",
            },
        )

        if created:
            cambiar_estado_reto(reto, "aprobado", admin, "Reto de ejemplo aprobado por siembra --demo.")
        elif reto.estado == "borrador":
            cambiar_estado_reto(reto, "aprobado", admin, "Reto de ejemplo aprobado por siembra --demo.")

        self.stdout.write(self.style.SUCCESS(
            "Demo creada. Usuarios (contrasena '%s'):\n"
            "  admin.demo / empresa.demo / profesor.demo / estudiante.demo"
            % demo_password
        ))
