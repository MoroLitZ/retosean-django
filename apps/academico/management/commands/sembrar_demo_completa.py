"""Siembra datos falsos que cubren TODAS las variantes visuales de la app.

A diferencia de `sembrar_catalogo --demo` (que crea un usuario por rol y un
solo reto aprobado), este comando crea, para cada modelo con `choices` de
estado, al menos una fila en cada valor posible: retos en sus 9 estados,
hackathones en sus 5, postulaciones en sus 4, entregables en sus 3, empresas
cruzando validacion x listas restrictivas, documentos legales en sus 5
estados, integraciones academicas en sus 5, etc. Asi cualquier vista de
listado/panel tiene, en local, un ejemplo de cada badge/color/rama que pueda
mostrar.

Es aditivo e idempotente por titulo/username: se puede correr varias veces
sin duplicar filas (usa get_or_create donde el modelo lo permite).

    python manage.py sembrar_demo_completa
    python manage.py sembrar_demo_completa --reset   # borra lo sembrado antes y repite

Requiere haber corrido antes `sembrar_catalogo` (o corre con --demo aqui
tambien: este comando lo invoca solo si el catalogo esta vacio).
"""

import itertools
from datetime import timedelta

from django.contrib.auth import get_user_model
from django.core.files.base import ContentFile
from django.core.management.base import BaseCommand
from django.db import transaction
from django.utils import timezone

PASSWORD_DEMO = "RetosEAN2026"
EMPRESAS_DEMO = [
    "empresa.verificada", "empresa.pendiente", "empresa.revision",
    "empresa.rechazada", "empresa.sinconvenio",
]


class Command(BaseCommand):
    help = "Siembra datos falsos cubriendo cada estado/rol/flujo visible de la app."

    def add_arguments(self, parser):
        parser.add_argument(
            "--reset",
            action="store_true",
            help="Borra los retos con prefijo 'DEMO ·' sembrados previamente antes de recrearlos.",
        )

    def handle(self, *args, **options):
        with transaction.atomic():
            if options["reset"]:
                self._reset()
            self._asegurar_catalogo()
            usuarios = self._sembrar_usuarios()
            self._sembrar_empresas(usuarios)
            self._sembrar_retos_en_todos_los_estados(usuarios)
            self._sembrar_reto_flujo_completo(usuarios)
            self._sembrar_hackathon_en_todos_los_estados(usuarios)
            self._sembrar_presupuestos(usuarios)
            self._sembrar_archivos_de_retos()
            self._sembrar_unidades_estudio()
            self._sembrar_notificaciones(usuarios)

        self.stdout.write(self.style.SUCCESS(
            f"\nSiembra completa. Usuarios de prueba (contrasena '{PASSWORD_DEMO}'):\n"
            "  admin.demo | admin2.demo\n"
            "  empresa.verificada / empresa.pendiente / empresa.revision / empresa.rechazada / empresa.sinconvenio\n"
            "  profesor.demo / profesor2.demo\n"
            "  estudiante.demo / estudiante2.demo / estudiante3.demo / estudiante4.demo\n"
            "  experto.demo\n"
        ))

    # ------------------------------------------------------------------ reset
    def _reset(self):
        from apps.empresas.models import Empresa
        from apps.evaluacion.models import EvaluacionCriterio
        from apps.retos.models import Reto

        # EvaluacionCriterio.criterio usa on_delete=PROTECT hacia CriterioRubrica,
        # asi que la cascada Reto -> Rubrica -> CriterioRubrica se bloquea si no
        # se limpian antes las evaluaciones que puntuan esos criterios.
        retos_demo = Reto.todos.filter(titulo__startswith="DEMO ·")
        EvaluacionCriterio.objects.filter(evaluacion__entregable__reto__in=retos_demo).delete()
        borrados, _ = retos_demo.delete()

        # Las empresas demo (y sus documentos/contactos en cascada) tambien se
        # recrean en cada corrida, para no arrastrar documentos de versiones
        # anteriores del seed que ya no coinciden con el reparto actual.
        from django.contrib.contenttypes.models import ContentType

        from apps.empresas.models import DocumentoEmpresa
        from apps.seguimiento.models import HistorialEstado
        from apps.unidades_estudio.models import UnidadEstudio

        empresas_demo = Empresa.objects.filter(usuario__username__in=EMPRESAS_DEMO)
        # HistorialEstado apunta por GenericForeignKey y no se borra en cascada.
        ids_empresas = list(empresas_demo.values_list("pk", flat=True))
        ids_docs = list(DocumentoEmpresa.objects.filter(empresa__in=ids_empresas).values_list("pk", flat=True))
        HistorialEstado.objects.filter(
            content_type=ContentType.objects.get_for_model(DocumentoEmpresa), object_id__in=ids_docs
        ).delete()
        HistorialEstado.objects.filter(
            content_type=ContentType.objects.get_for_model(Empresa), object_id__in=ids_empresas
        ).delete()
        borrados_empresas, _ = empresas_demo.delete()
        UnidadEstudio.objects.filter(codigo__startswith="DEMO-").delete()

        self.stdout.write(
            f"Reset: {borrados} filas relacionadas con retos DEMO borradas, "
            f"{borrados_empresas} filas relacionadas con empresas DEMO borradas."
        )

    # --------------------------------------------------------------- catalogo
    def _asegurar_catalogo(self):
        from apps.academico.models import Ecosistema, Facultad, Programa

        if Facultad.objects.exists():
            return
        from apps.academico import catalogo_base

        for nombre, codigo in catalogo_base.FACULTADES:
            Facultad.objects.get_or_create(nombre=nombre, defaults={"codigo": codigo})
        for nombre_programa, nombre_facultad in catalogo_base.PROGRAMAS:
            facultad = Facultad.objects.get(nombre=nombre_facultad)
            Programa.objects.get_or_create(nombre=nombre_programa, facultad=facultad)
        for nombre, descripcion in catalogo_base.ECOSISTEMAS:
            Ecosistema.objects.get_or_create(nombre=nombre, defaults={"descripcion": descripcion})
        self.stdout.write("Catalogo base sembrado (no existia).")

    # --------------------------------------------------------------- usuarios
    def _usuario(self, username, email, first_name, last_name, rol, **kwargs):
        Usuario = get_user_model()
        user, created = Usuario.objects.get_or_create(
            username=username,
            defaults={
                "email": email, "first_name": first_name, "last_name": last_name,
                "rol": rol, "is_active": True, **kwargs,
            },
        )
        if created:
            user.set_password(PASSWORD_DEMO)
            user.save(update_fields=["password"])
        return user

    def _sembrar_usuarios(self):
        from apps.academico.models import Estudiante, Facultad, Profesor, Programa

        facultades = list(Facultad.objects.order_by("id"))
        programas = list(Programa.objects.order_by("id"))
        fac = lambda i: facultades[i % len(facultades)] if facultades else None
        prog = lambda i: programas[i % len(programas)] if programas else None

        u = {}
        u["admin"] = self._usuario("admin.demo", "admin@universidadean.edu.co", "Admin", "Demo", "ADMIN", is_staff=True, is_superuser=True)
        u["admin2"] = self._usuario("admin2.demo", "admin2@universidadean.edu.co", "Ana", "Administradora", "ADMIN", is_staff=True, is_superuser=True)

        u["experto"] = self._usuario("experto.demo", "experto@externo.co", "Experto", "Invitado", "EXPERTO")

        empresas_usuarios = []
        for i, (slug, nombre) in enumerate([
            ("verificada", "Verificada"), ("pendiente", "Pendiente"),
            ("revision", "En Revision"), ("rechazada", "Rechazada"), ("sinconvenio", "Sin Convenio"),
        ]):
            eu = self._usuario(f"empresa.{slug}", f"contacto@empresa-{slug}.co", f"Empresa {nombre}", "S.A.S.", "EMPRESA")
            empresas_usuarios.append((slug, eu))
        u["empresas"] = dict(empresas_usuarios)

        profesores = []
        for i, nombre in enumerate(["Carlos Profesor", "Diana Docente"]):
            first, last = nombre.split(" ", 1)
            pu = self._usuario(f"profesor{'' if i == 0 else i+1}.demo", f"profesor{i+1}@universidadean.edu.co", first, last, "PROFESOR")
            Profesor.objects.get_or_create(usuario=pu, defaults={"facultad": fac(i), "especialidad": "Ingenieria de Software" if i == 0 else "Marketing Digital"})
            profesores.append(pu)
        u["profesores"] = profesores

        estudiantes = []
        for i, nombre in enumerate(["Estudiante Uno", "Estudiante Dos", "Estudiante Tres", "Estudiante Cuatro"]):
            first, last = nombre.split(" ", 1)
            username = "estudiante.demo" if i == 0 else f"estudiante{i+1}.demo"
            su = self._usuario(username, f"{username}@universidadean.edu.co", first, last, "ESTUDIANTE")
            Estudiante.objects.get_or_create(usuario=su, defaults={"programa": prog(i), "semestre": 3 + i})
            estudiantes.append(su)
        u["estudiantes"] = estudiantes

        self.stdout.write(f"Usuarios: 2 admin, {len(u['empresas'])} empresa, {len(profesores)} profesor, {len(estudiantes)} estudiante, 1 experto.")
        return u

    # --------------------------------------------------------------- empresas
    def _sembrar_empresas(self, u):
        from apps.empresas.models import ContactoEmpresa, DocumentoEmpresa, Empresa

        specs = {
            "verificada": ("900111111-1", "VERIFICADA", "APROBADO"),
            "pendiente": ("900222222-2", "PENDIENTE", "PENDIENTE"),
            "revision": ("900333333-3", "EN_REVISION", "PENDIENTE"),
            "rechazada": ("900444444-4", "RECHAZADA", "RECHAZADO"),
            "sinconvenio": ("900555555-5", "VERIFICADA", "APROBADO"),
        }
        empresas = {}
        for slug, (nit, estado_val, estado_listas) in specs.items():
            eu = u["empresas"][slug]
            empresa, _ = Empresa.objects.get_or_create(
                usuario=eu,
                defaults={
                    "nit": nit, "razon_social": eu.first_name,
                    "sector_industrial": "Tecnologia", "estado_validacion": estado_val,
                    "estado_listas_restrictivas": estado_listas,
                    "observaciones_admin": "Pendiente de revisar soportes." if estado_val == "EN_REVISION" else "",
                    "renuncio_a_convenio": slug == "sinconvenio",
                },
            )
            empresas[slug] = empresa
            ContactoEmpresa.objects.get_or_create(
                empresa=empresa, email=eu.email,
                defaults={"nombre": eu.first_name, "cargo": "Gerente de Innovacion", "telefono": "3000000000", "es_principal": True},
            )

        self._sembrar_documentos(u["admin"], empresas)
        self.stdout.write(
            f"Empresas: {len(empresas)} (una por cada estado de validacion + una que renuncia al convenio), "
            "los 8 documentos de HU00 en distintos estados, convenio en sus 4 tramites e historial documental."
        )
        return empresas

    # ------------------------------------------------------- retos por estado
    def _reto_base(self, u, empresa_key, titulo, estado_objetivo, **overrides):
        from apps.academico.models import Ecosistema, Facultad, Programa
        from apps.retos.models import Reto
        from apps.retos.services import cambiar_estado_reto

        empresa_user = u["empresas"][empresa_key]
        defaults = {
            "empresa": empresa_user,
            "tipo": overrides.pop("tipo", "reto"),
            "descripcion": f"Reto de demostracion en estado '{estado_objetivo}'. Generado para revisar cada vista posible.",
            "area": overrides.pop("area", "Tecnología"),
            "nivel_academico": overrides.pop("nivel_academico", "Pregrado"),
            "facultad": Facultad.objects.order_by("id").first(),
            "programa": Programa.objects.order_by("id").first(),
            "ecosistema": Ecosistema.objects.order_by("id").first(),
            "fecha_inicio_tentativa": timezone.localdate() + timedelta(days=10),
            "fecha_fin_tentativa": timezone.localdate() + timedelta(days=100),
            "fecha_limite_postulacion": timezone.localdate() + timedelta(days=5),
            "premios": "Reconocimiento institucional y carta de recomendacion.",
            "criterios_evaluacion": "Impacto, viabilidad, presentacion.",
            **overrides,
        }
        reto, created = Reto.objects.get_or_create(titulo=titulo, defaults=defaults)
        if not created:
            return reto

        admin = u["admin"]
        # Recorre la maquina de estados real via el servicio, para que el
        # historial y las notificaciones queden como en un caso real.
        secuencia = {
            "borrador": [],
            "en_revision": ["en_revision"],
            "aprobado": ["en_revision", "aprobado"],
            "abierto": ["en_revision", "aprobado", "abierto"],
            "rechazado": ["en_revision", "rechazado"],
            "en_curso": ["en_revision", "aprobado", "abierto", "en_curso"],
            "pausado": ["en_revision", "aprobado", "abierto", "en_curso", "pausado"],
            "finalizado": ["en_revision", "aprobado", "abierto", "en_curso", "finalizado"],
            "cancelado": ["en_revision", "aprobado", "cancelado"],
        }[estado_objetivo]
        for paso in secuencia:
            comentario = "Aprobado: cumple los criterios institucionales." if paso == "aprobado" else (
                "Rechazado: falta alcance claro para el semestre." if paso == "rechazado" else ""
            )
            cambiar_estado_reto(reto, paso, admin, comentario)
        return reto

    def _sembrar_retos_en_todos_los_estados(self, u):
        estados = ["borrador", "en_revision", "aprobado", "abierto", "rechazado", "en_curso", "pausado", "finalizado", "cancelado"]
        empresas_ciclo = itertools.cycle(["verificada", "pendiente", "revision", "rechazada"])
        for estado in estados:
            self._reto_base(
                u, next(empresas_ciclo),
                f"DEMO · Reto en estado {estado}", estado,
            )
        self.stdout.write(f"Retos: {len(estados)} (uno por cada estado de ESTADO_CHOICES).")

    # --------------------------------------------------- reto con flujo completo
    def _sembrar_reto_flujo_completo(self, u):
        """Un reto 'finalizado' con TODO alrededor: equipo academico, postulaciones
        en sus 4 estados, entregables en sus 3 estados + rubrica + evaluaciones +
        comentarios, seguimiento, presupuesto con gastos, cierre con encuesta,
        certificados, favoritos."""
        from apps.academico.models import Certificado
        from apps.cierre.models import (
            AgendaCierre, CierreReto, EncuestaSatisfaccion, EntregableFinal, Reconocimiento,
        )
        from apps.evaluacion.models import (
            ComentarioEntregable, CriterioRubrica, Entregable, Evaluacion, EvaluacionCriterio, Rubrica,
        )
        from apps.participaciones.models import Equipo, MiembroEquipo, Postulacion, RetoFavorito
        from apps.presupuesto.models import Gasto, Presupuesto
        from apps.retos.models import EquipoRetoAcademico, SesionRetoAcademico
        from apps.seguimiento.models import IntegracionAcademica, SeguimientoArchivo, SeguimientoReto

        reto = self._reto_base(u, "verificada", "DEMO · Reto con flujo completo (finalizado)", "finalizado")
        admin = u["admin"]
        profesor = u["profesores"][0]
        estudiantes = u["estudiantes"]

        # Postulaciones: una por cada estado posible.
        postulacion_estados = ["PENDIENTE", "ACEPTADA", "RECHAZADA", "RETIRADA"]
        for estudiante, estado in zip(estudiantes, postulacion_estados):
            Postulacion.objects.get_or_create(
                reto=reto, estudiante=estudiante,
                defaults={
                    "estado": estado, "motivacion": "Quiero aplicar lo aprendido en clase a un caso real.",
                    "habilidades": "Python, trabajo en equipo, analisis de datos.",
                    "semestre": 5, "programa": "Ingenieria de Sistemas",
                },
            )

        # Integracion academica del profesor con este reto (necesaria para
        # habilitar equipo/entregables en la practica), en estado 'publicada'.
        IntegracionAcademica.objects.get_or_create(
            reto=reto, profesor=profesor,
            defaults={
                "facultad": "Ingenieria", "nivel_formacion": "Pregrado", "programa_academico": "Ingenieria de Sistemas",
                "descripcion": "Curso de Proyecto Integrador vinculado al reto.",
                "alcance": "Los estudiantes desarrollan una propuesta de solucion end-to-end.",
                "entregable_esperado": "Prototipo funcional y documento tecnico.",
                "estado": "publicada",
            },
        )
        # Y otra integracion (de otro profesor) en cada estado restante, para
        # que el panel de vinculaciones muestre las 5 variantes.
        estados_integracion = ["borrador", "en_revision", "aprobada", "rechazada"]
        reto_publicado = self._reto_base(u, "verificada", "DEMO · Reto en estado en_curso", "en_curso")
        for estado in estados_integracion:
            IntegracionAcademica.objects.get_or_create(
                reto=reto_publicado, profesor=u["profesores"][1],
                descripcion=f"Integracion de demostracion en estado {estado}.",
                defaults={
                    "facultad": "Marketing", "nivel_formacion": "Pregrado", "programa_academico": "Mercadeo",
                    "alcance": "Alcance de ejemplo.", "entregable_esperado": "Informe final.",
                    "estado": estado,
                },
            )

        # Equipo academico (aprobados los dos primeros estudiantes) + equipo espejo operativo.
        equipo_espejo, _ = Equipo.objects.get_or_create(reto=reto, nombre="Equipo Demo Alpha")
        for est in estudiantes[:2]:
            MiembroEquipo.objects.get_or_create(
                equipo=equipo_espejo, estudiante=est,
                defaults={"rol": "LIDER" if est == estudiantes[0] else "MIEMBRO"},
            )
        equipo_academico, _ = EquipoRetoAcademico.objects.get_or_create(
            reto=reto, nombre_equipo="Equipo Demo Alpha",
            defaults={"equipo_espejo": equipo_espejo},
        )
        equipo_academico.profesores.add(profesor)
        equipo_academico.estudiantes.add(*estudiantes[:2])
        equipo_academico.expertos_invitados.add(u["experto"])

        # Sesiones del reto academico: una por cada tipo.
        for tipo in ["inicio", "seguimiento", "preseleccion", "evaluacion"]:
            SesionRetoAcademico.objects.get_or_create(
                reto=reto, tipo=tipo,
                defaults={"fecha_hora": timezone.now() + timedelta(days=1), "observaciones": f"Sesion de {tipo}."},
            )

        # Seguimiento: una sesion por cada tipo de sesion posible.
        for tipo in ["inicio", "seguimiento", "preseleccion", "evaluacion", "otro"]:
            seguimiento, creado = SeguimientoReto.objects.get_or_create(
                reto=reto, tipo_sesion=tipo, fecha_sesion=timezone.localdate(),
                defaults={
                    "porcentaje_avance": 40, "avances": "Se completo el analisis inicial.",
                    "observaciones": "Buen ritmo de trabajo.", "acuerdos": "Entregar avance la proxima semana.",
                    "creado_por": profesor,
                },
            )
            if creado and tipo == "seguimiento":
                SeguimientoArchivo.objects.get_or_create(
                    seguimiento=seguimiento,
                    archivo=ContentFile(b"contenido demo", name="avance-semana-1.txt"),
                )

        # Rubrica con 3 criterios.
        rubrica, _ = Rubrica.objects.get_or_create(
            reto=reto, nombre="Rubrica de evaluacion final",
            defaults={"descripcion": "Evalua impacto, viabilidad y presentacion.", "creada_por": profesor},
        )
        criterios = []
        for nombre, puntaje, orden in [("Impacto", 2.0, 1), ("Viabilidad", 2.0, 2), ("Presentacion", 1.0, 3)]:
            criterio, _ = CriterioRubrica.objects.get_or_create(
                rubrica=rubrica, nombre=nombre, defaults={"puntaje_maximo": puntaje, "orden": orden},
            )
            criterios.append(criterio)

        # Entregables: uno por cada estado posible, con historial de comentarios
        # y evaluacion completa (con rubrica) en el que esta CALIFICADO.
        entregable_estados = ["ENVIADO", "EN_REVISION", "CALIFICADO"]
        for i, (estudiante, estado) in enumerate(zip(estudiantes, entregable_estados)):
            entregable, creado = Entregable.objects.get_or_create(
                reto=reto, estudiante=estudiante, titulo=f"Avance {i+1} — {estado.title()}",
                defaults={
                    "equipo": equipo_espejo, "es_final": estado == "CALIFICADO",
                    "puntaje_maximo": 5.00, "estado": estado,
                    "archivo": ContentFile(b"contenido demo", name=f"entregable-{i+1}.pdf"),
                    "comentario_estudiante": "Adjunto el avance solicitado.",
                },
            )
            if not creado:
                continue
            if estado in ("EN_REVISION", "CALIFICADO"):
                ComentarioEntregable.objects.create(
                    entregable=entregable, autor=profesor,
                    comentario="Recibido, quedo pendiente de revision detallada.",
                )
            if estado == "CALIFICADO":
                entregable.comentario_profesor = "Buen trabajo, cumple con lo esperado."
                entregable.nota = 4.50
                entregable.save(update_fields=["comentario_profesor", "nota"])
                ComentarioEntregable.objects.create(
                    entregable=entregable, autor=profesor, comentario="Calificado: 4.5/5.",
                )
                evaluacion, _ = Evaluacion.objects.get_or_create(
                    entregable=entregable, profesor=profesor,
                    defaults={"nota": 4.50, "comentario": "Excelente sustentacion.", "rubrica": rubrica},
                )
                for criterio, puntaje in zip(criterios, [1.8, 1.9, 0.8]):
                    EvaluacionCriterio.objects.get_or_create(
                        evaluacion=evaluacion, criterio=criterio, defaults={"puntaje": puntaje},
                    )

        # Favoritos: un par de estudiantes marcan el reto como favorito.
        for est in estudiantes[2:]:
            RetoFavorito.objects.get_or_create(usuario=est, reto=reto)

        # Presupuesto con gastos en cada categoria.
        presupuesto, _ = Presupuesto.objects.get_or_create(reto=reto, defaults={"monto_total": 5_000_000})
        for categoria, monto in [
            ("REFRIGERIOS", 300_000), ("MATERIALES", 500_000), ("IMPRESIONES", 100_000),
            ("ESPACIOS", 400_000), ("OTROS", 150_000),
        ]:
            Gasto.objects.get_or_create(
                presupuesto=presupuesto, categoria=categoria,
                defaults={
                    "monto": monto, "descripcion": f"Gasto demo de {categoria.lower()}.", "registrado_por": profesor,
                    "estado": Gasto.COMPROMETIDO if categoria in ("ESPACIOS", "OTROS") else Gasto.EJECUTADO,
                },
            )

        # Agenda + cierre + reconocimientos + entregable final + encuesta.
        AgendaCierre.objects.get_or_create(
            reto=reto, defaults={
                "fecha_hora": timezone.now() + timedelta(days=1),
                "espacio": "Auditorio EAN", "recursos": "Proyector, microfonos.",
                "invitados": "Empresa, profesores, estudiantes.", "agenda": "Presentacion de resultados y cierre.",
            },
        )
        cierre, _ = CierreReto.objects.get_or_create(
            reto=reto,
            defaults={
                "notas": "Reto cerrado con exito, buena participacion.",
                "encuesta_enviada": True, "cerrado_por": admin, "cerrado_en": timezone.now(),
                "acta_url": ContentFile(b"%PDF-1.4 acta demo", name="acta-cierre.pdf"),
            },
        )
        Reconocimiento.objects.get_or_create(
            cierre=cierre, usuario=estudiantes[0],
            defaults={"nombre_destinatario": estudiantes[0].get_full_name(), "descripcion": "Mejor propuesta de solucion."},
        )
        EntregableFinal.objects.get_or_create(
            cierre=cierre, nombre="Informe final consolidado",
            defaults={"archivo": ContentFile(b"%PDF-1.4 informe demo", name="informe-final.pdf"), "cargado_por": admin},
        )
        # Encuestas: una respondida y otra pendiente, para ver ambos estados.
        EncuestaSatisfaccion.objects.get_or_create(
            cierre=cierre, participante=estudiantes[0],
            defaults={"calificacion": 5, "comentario": "Excelente experiencia.", "respondida_en": timezone.now()},
        )
        EncuestaSatisfaccion.objects.get_or_create(
            cierre=cierre, participante=estudiantes[1],
            defaults={"calificacion": None, "comentario": ""},
        )

        # Certificados para estudiante, profesor y empresa.
        for usuario, rol, horas in [
            (estudiantes[0], "ESTUDIANTE", 40), (profesor, "PROFESOR", None), (reto.empresa, "EMPRESA", None),
        ]:
            Certificado.objects.get_or_create(
                reto=reto, usuario=usuario, rol_participacion=rol,
                defaults={"horas": horas, "emitido_por": admin},
            )

        self.stdout.write(
            "Reto de flujo completo: postulaciones (4 estados), integraciones (5 estados), "
            "equipo, entregables (3 estados) + rubrica + evaluacion, seguimiento (5 tipos), "
            "presupuesto (5 categorias de gasto), cierre + encuestas (respondida/pendiente), "
            "certificados (estudiante/profesor/empresa)."
        )

    # -------------------------------------------------------- hackathones
    def _sembrar_hackathon_en_todos_los_estados(self, u):
        from apps.hackaton.models import EtapaHackaton, Hackathon, InscripcionHackaton, Jurado, VotacionHackaton
        from apps.hackaton.services import crear_etapas_por_defecto
        from apps.participaciones.models import Equipo, MiembroEquipo

        estados = ["borrador", "publicado", "en_curso", "finalizado", "cancelado"]
        for estado in estados:
            reto_base_estado = "en_curso" if estado in ("publicado", "en_curso") else (
                "finalizado" if estado == "finalizado" else ("cancelado" if estado == "cancelado" else "aprobado")
            )
            reto = self._reto_base(
                u, "verificada", f"DEMO · Hackathon en estado {estado}", reto_base_estado, tipo="hackathon",
            )
            hackathon, creado = Hackathon.objects.get_or_create(
                reto=reto,
                defaults={
                    "reglas": "Equipos de hasta 4 personas. Entrega final el ultimo dia.",
                    "estado": estado, "resultados_publicados": estado == "finalizado",
                },
            )
            if not creado:
                continue
            crear_etapas_por_defecto(hackathon)

            if estado in ("en_curso", "finalizado"):
                jurado, _ = Jurado.objects.get_or_create(
                    hackathon=hackathon, email="jurado.demo@externo.co",
                    defaults={"nombre": "Jurado Demo", "especialidad": "Innovacion"},
                )
                equipo, _ = Equipo.objects.get_or_create(reto=reto, nombre="Equipo Hackathon Demo")
                for est in u["estudiantes"][:2]:
                    MiembroEquipo.objects.get_or_create(equipo=equipo, estudiante=est, defaults={"rol": "MIEMBRO"})

                for insc_estado in ["PENDIENTE", "ACEPTADA", "RECHAZADA"]:
                    equipo_insc, _ = Equipo.objects.get_or_create(reto=reto, nombre=f"Equipo {insc_estado.title()}")
                    InscripcionHackaton.objects.get_or_create(
                        hackathon=hackathon, equipo=equipo_insc,
                        defaults={"estado": insc_estado, "inscrito_por": u["admin"]},
                    )

                if estado == "finalizado":
                    InscripcionHackaton.objects.get_or_create(
                        hackathon=hackathon, equipo=equipo,
                        defaults={"estado": "ACEPTADA", "inscrito_por": u["admin"]},
                    )
                    VotacionHackaton.objects.get_or_create(
                        hackathon=hackathon, jurado=jurado, equipo=equipo,
                        defaults={"puntaje": 4.2, "comentario": "Buena presentacion final."},
                    )

        self.stdout.write(f"Hackathones: {len(estados)} (uno por cada estado), con jurados/inscripciones/votacion donde aplica.")

    # ----------------------------------------------------------- notificaciones
    # ------------------------------------------------- documentos (HU00)
    def _sembrar_documentos(self, admin, empresas):
        """Cada empresa ilustra un caso de HU00 sobre el catalogo de tipos.

        Pasa por los servicios (no por el ORM) para dejar el historial y los avisos como en un caso real.
        """
        from apps.empresas.models import DocumentoEmpresa
        from apps.empresas.services import (
            cambiar_estado_convenio, cargar_documento, marcar_documento_vencido, revisar_documento,
        )
        from apps.seguimiento.services import registrar_cambio_estado

        hoy = timezone.localdate()
        # Los ocho documentos obligatorios de la historia. La minuta es propia del convenio.
        requeridos = [
            "MINUTA_CONVENIO", "CAMARA_COMERCIO", "RUT", "CEDULA_REPRESENTANTE",
            "LISTAS_INFORMA", "FORMATO_COMPROMISO", "TERMINOS_REFERENCIA", "FICHA_RETO",
        ]

        def documento(empresa, codigo, final, dias=1, motivo=""):
            if DocumentoEmpresa.objects.filter(empresa=empresa, tipo_documento=codigo).exists():
                return  # idempotente: no reabrir lo ya sembrado
            if final == "PENDIENTE":  # solicitado, aun sin archivo
                doc = DocumentoEmpresa.objects.create(empresa=empresa, tipo_documento=codigo, estado="PENDIENTE")
                registrar_cambio_estado(doc, "", "PENDIENTE", admin, "Documento solicitado a la empresa.")
                return
            archivo = ContentFile(b"%PDF-1.4 demo", name=f"{codigo.lower()}-{empresa.nit}.pdf")
            # Listas Informa la sube la universidad (permiso especial), no la empresa.
            quien = admin if codigo == "LISTAS_INFORMA" else empresa.usuario
            doc, _ = cargar_documento(empresa, codigo, archivo, hoy - timedelta(days=dias), quien)
            if final in ("VERIFICADO", "RECHAZADO"):
                revisar_documento(doc, final, admin, motivo)
            elif final == "VENCIDO":
                marcar_documento_vencido(doc, admin)

        def convenio(empresa, *pasos):
            if empresa.estado_convenio != "NO_RADICADO":
                return
            for estado, comentario, vence in pasos:
                cambiar_estado_convenio(empresa, estado, admin, comentario, vence)

        def al_dia(empresa, omitir=()):
            for codigo in requeridos:
                if codigo not in omitir:
                    documento(empresa, codigo, "VERIFICADO", dias=5 if codigo == "CAMARA_COMERCIO" else 1)

        # Completa: los ocho al dia, mas el convenio firmado (opcional) y su tramite.
        e = empresas["verificada"]
        al_dia(e)
        documento(e, "CONVENIO", "VERIFICADO")
        convenio(e, ("FIRMADO", "Convenio firmado por las partes.", hoy + timedelta(days=300)))

        # Sin nada cargado: ve la alerta de "falta cargarlo".
        documento(empresas["pendiente"], "RUT", "PENDIENTE")

        # En revision: le falta cargar los Terminos de referencia y tiene dos documentos
        # esperando al administrador. Sirve para completar el ciclo en vivo.
        e = empresas["revision"]
        for codigo in ("MINUTA_CONVENIO", "RUT", "CEDULA_REPRESENTANTE", "LISTAS_INFORMA", "FORMATO_COMPROMISO"):
            documento(e, codigo, "VERIFICADO")
        documento(e, "CAMARA_COMERCIO", "CARGADO", dias=3)
        documento(e, "FICHA_RETO", "CARGADO", dias=2)
        convenio(e, ("RADICADO", "Convenio recibido para tramite.", None))

        # Rechazada: un documento rechazado, la camara vencida y el convenio atascado en Juridica.
        e = empresas["rechazada"]
        documento(e, "RUT", "RECHAZADO", motivo="El documento no corresponde al NIT registrado.")
        documento(e, "CAMARA_COMERCIO", "VENCIDO", dias=45)
        convenio(e, ("RADICADO", "Convenio recibido para tramite.", None),
                 ("EN_JURIDICA", "En revision del area juridica.", None))

        # Sin convenio: se le eximen solo los documentos propios del convenio (la minuta).
        al_dia(empresas["sinconvenio"], omitir=("MINUTA_CONVENIO",))

    # ------------------------------------------------------- presupuestos
    def _sembrar_presupuestos(self, u):
        """Presupuestos con gastos comprometidos y ejecutados en distintos niveles de uso."""
        from apps.presupuesto.models import Gasto, Presupuesto
        from apps.presupuesto.services import registrar_gasto
        from apps.retos.models import Reto

        E, C = Gasto.EJECUTADO, Gasto.COMPROMETIDO
        casos = {  # estado del reto: (presupuesto, [(categoria, monto, estado)])
            "en_curso": (5_000_000, [("REFRIGERIOS", 600_000, E), ("MATERIALES", 1_400_000, E),
                                     ("ESPACIOS", 1_500_000, C), ("IMPRESIONES", 800_000, C)]),  # 86 %: alerta del 80 %
            "pausado": (2_000_000, [("MATERIALES", 1_200_000, E), ("OTROS", 900_000, C)]),  # 105 %: excedido
            "abierto": (3_000_000, [("IMPRESIONES", 500_000, C)]),  # 17 %: sin alerta
        }
        for estado, (monto, gastos) in casos.items():
            reto = Reto.objects.filter(titulo=f"DEMO · Reto en estado {estado}").first()
            if reto is None:
                continue
            presupuesto, _ = Presupuesto.objects.get_or_create(reto=reto, defaults={"monto_total": monto})
            if presupuesto.gastos.exists():
                continue
            for categoria, valor, estado_gasto in gastos:
                registrar_gasto(presupuesto, Gasto(
                    categoria=categoria, monto=valor, estado=estado_gasto,
                    descripcion=f"Gasto demo de {categoria.lower()} ({estado_gasto.lower()}).",
                ), u["admin"])
        self.stdout.write("Presupuestos: 4 con gastos comprometidos y ejecutados (17 %, 29 %, 86 % y 105 %).")

    def _sembrar_archivos_de_retos(self):
        """Adjuntos en retos aun sin cerrar, para ver el listado de archivos al revisar."""
        from apps.retos.models import Reto, RetoArchivo

        contenido = b"%PDF-1.4 demo"
        for estado in ("en_revision", "aprobado", "abierto"):
            reto = Reto.objects.filter(titulo=f"DEMO · Reto en estado {estado}").first()
            if reto is None:
                continue
            for nombre in ("Ficha del reto.pdf", "Terminos de referencia.pdf"):
                if not reto.archivos.filter(nombre_original=nombre).exists():
                    RetoArchivo.objects.create(
                        reto=reto, nombre_original=nombre, tamano=len(contenido),
                        archivo=ContentFile(contenido, name=nombre.replace(" ", "_")),
                    )

    # --------------------------------------------------- unidades de estudio
    def _sembrar_unidades_estudio(self):
        from apps.academico.models import Programa
        from apps.unidades_estudio.models import UnidadEstudio

        programas = list(Programa.objects.order_by("id")[:6])
        if not programas:
            return
        # (nombre, periodo, ciclo, activa, con PDF)
        unidades = [
            ("Fundamentos de Programacion", "2026-2", "Ciclo basico", True, True),
            ("Calculo Diferencial", "2026-2", "Ciclo basico", True, False),
            ("Estructuras de Datos", "2026-2", "Ciclo profesional", True, True),
            ("Bases de Datos", "2026-2", "Ciclo profesional", True, True),
            ("Ingenieria de Software", "2026-2", "Ciclo profesional", True, False),
            ("Gestion de Proyectos", "2026-1", "Ciclo profesional", True, True),
            ("Probabilidad y Estadistica", "2026-1", "Ciclo basico", True, False),
            ("Inteligencia Artificial", "2026-1", "Ciclo de profundizacion", True, True),
            ("Emprendimiento e Innovacion", "2026-1", "Ciclo de profundizacion", True, False),
            ("Etica Profesional", "2025-2", "Ciclo basico", False, False),
            ("Electiva de Analitica", "2025-2", "Ciclo de profundizacion", False, True),
            ("Practica Empresarial", "2026-2", "Ciclo de profundizacion", True, True),
        ]
        for i, (nombre, periodo, ciclo, activa, con_pdf) in enumerate(unidades, start=1):
            codigo = f"DEMO-{i:03d}"
            unidad, creada = UnidadEstudio.objects.get_or_create(
                codigo=codigo,
                defaults={
                    "nombre": nombre, "programa": programas[(i - 1) % len(programas)],
                    "periodo": periodo, "ciclo": ciclo, "activo": activa,
                },
            )
            if creada and con_pdf:
                unidad.archivo.save(f"{codigo}.pdf", ContentFile(b"%PDF-1.4 demo"), save=True)
        self.stdout.write(f"Unidades de estudio: {len(unidades)} (en {len(programas)} programas, 2 inactivas).")

    def _sembrar_notificaciones(self, u):
        from apps.notificaciones.models import Notificacion, PreferenciaNotificacion

        estudiante = u["estudiantes"][0]
        for tipo, leida, titulo, mensaje in [
            ("INFO", False, "Nuevo reto disponible", "Hay un reto nuevo que coincide con tu perfil."),
            ("EXITO", False, "Certificado disponible", "Ya puedes descargar tu certificado de participacion."),
            ("ADVERTENCIA", True, "Entrega proxima a vencer", "Tu entregable vence en 2 dias."),
            ("ERROR", True, "Postulacion rechazada", "Tu postulacion no fue aceptada esta vez."),
        ]:
            Notificacion.objects.get_or_create(
                usuario=estudiante, titulo=titulo,
                defaults={
                    "tipo": tipo, "mensaje": mensaje, "leida": leida,
                    "leida_en": timezone.now() if leida else None,
                    "evento": "DEMO_SEED",
                },
            )
        PreferenciaNotificacion.objects.get_or_create(usuario=estudiante)
        self.stdout.write("Notificaciones: 4 (una por cada tipo, mitad leidas/no leidas).")
