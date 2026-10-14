"""Prueba de humo: cada ruta del proyecto, con cada rol, sin errores 500.

Siembra los datos de demostracion (un reto por estado, hackathon por estado,
documentos, postulaciones, entregables...) y recorre todas las rutas
registradas como cada usuario demo. Las rutas con parametros se resuelven con
objetos reales; para los retos se prueban todos, asi que cada estado del ciclo
de vida pasa por cada vista. Solo se exige que ninguna respuesta sea un 5xx:
los 403/404/302 por permisos son comportamiento esperado.
"""

import re
from io import StringIO

from django.core.management import call_command
from django.test import TestCase
from django.urls import URLPattern, URLResolver, get_resolver

# Rutas que no tiene sentido recorrer: cierran la sesion del cliente o
# requieren un token firmado.
RUTAS_EXCLUIDAS = {"usuarios:logout", "usuarios:password_reset_confirm"}

USUARIOS_DEMO = ["admin.demo", "empresa.verificada", "profesor.demo", "estudiante.demo", "experto.demo"]

PARAMETRO = re.compile(r"<(?:(?P<tipo>\w+):)?(?P<nombre>\w+)>")


def _rutas(patrones=None, prefijo="", espacio=""):
    """[(nombre 'ns:url', ruta en texto)] de todas las URL del proyecto."""
    patrones = get_resolver().url_patterns if patrones is None else patrones
    for entrada in patrones:
        if isinstance(entrada, URLResolver):
            ns = entrada.namespace or espacio
            yield from _rutas(entrada.url_patterns, prefijo + str(entrada.pattern), ns)
        elif isinstance(entrada, URLPattern) and entrada.name:
            yield f"{espacio}:{entrada.name}", prefijo + str(entrada.pattern)


def _modelo_para(ruta, parametro):
    """Modelo cuyo pk/uuid llena el parametro de la ruta."""
    from apps.academico.models import Certificado
    from apps.cierre.models import EncuestaSatisfaccion
    from apps.empresas.models import DocumentoEmpresa, Empresa
    from apps.evaluacion.models import Entregable
    from apps.hackaton.models import Hackathon, InscripcionHackaton, Jurado
    from apps.notificaciones.models import Notificacion
    from apps.participaciones.models import Postulacion
    from apps.presupuesto.models import Gasto
    from apps.reportes.models import ReporteGenerado
    from apps.retos.models import EquipoRetoAcademico, Reto
    from apps.seguimiento.models import IntegracionAcademica
    from apps.unidades_estudio.models import UnidadEstudio
    from apps.usuarios.models import Usuario

    if parametro == "reto_id":
        return Reto
    if parametro == "empresa_id":
        return Empresa
    if parametro == "documento_id":
        return DocumentoEmpresa
    if parametro == "postulacion_id":
        return Postulacion
    if parametro == "codigo":
        return Certificado
    if ruta.startswith("empresas/admin/tipos-documento/"):
        from apps.empresas.models import TipoDocumentoLegal

        return TipoDocumentoLegal
    reglas = [
        (r"^retos/(admin/)?(empresa/)?(integraciones?|vinculaciones|integracion)/", IntegracionAcademica),
        (r"^retos/equipo/", EquipoRetoAcademico),
        (r"^retos/", Reto),
        (r"^hackatones/inscripcion/", InscripcionHackaton),
        (r"^hackatones/jurado/", Jurado),
        (r"^hackatones/", Hackathon),
        (r"^evaluacion/", Entregable),
        (r"^notificaciones/", Notificacion),
        (r"^cierre/encuestas/", EncuestaSatisfaccion),
        (r"^presupuesto/gasto/", Gasto),
        (r"^reportes/", ReporteGenerado),
        (r"^unidades-estudio/", UnidadEstudio),
        (r"^usuarios/", Usuario),
    ]
    for patron, modelo in reglas:
        if re.search(patron, ruta):
            return modelo
    raise AssertionError(f"Sin modelo para el parametro {parametro!r} de {ruta!r}")


class RecorridoDeRutasTests(TestCase):
    @classmethod
    def setUpTestData(cls):
        call_command("sembrar_catalogo", "--demo", stdout=StringIO())
        call_command("sembrar_demo_completa", stdout=StringIO())

    def _urls(self):
        """URLs concretas a visitar: una por objeto para retos, dos para el resto."""
        from apps.retos.models import Reto

        for nombre, ruta in _rutas():
            # El admin de Django tiene su propia cobertura; aqui solo las vistas del proyecto.
            if nombre in RUTAS_EXCLUIDAS or nombre.startswith("admin:"):
                continue
            parametros = list(PARAMETRO.finditer(ruta))
            if not parametros:
                yield nombre, "/" + ruta.replace("^", "").replace("$", "")
                continue
            modelo = _modelo_para(ruta, parametros[0].group("nombre"))
            campo = "codigo_verificacion" if parametros[0].group("nombre") == "codigo" else "pk"
            objetos = modelo.objects.all() if modelo is Reto else modelo.objects.all()[:2]
            for valor in objetos.values_list(campo, flat=True):
                yield nombre, "/" + PARAMETRO.sub(str(valor), ruta)

    def test_ninguna_ruta_responde_500_para_ningun_rol(self):
        from apps.usuarios.models import Usuario

        urls = list(self._urls())
        self.assertGreater(len(urls), 100)
        fallos = []
        for username in USUARIOS_DEMO:
            self.client.force_login(Usuario.objects.get(username=username))
            for nombre, url in urls:
                try:
                    respuesta = self.client.get(url)
                    estado = respuesta.status_code
                except Exception as error:  # el cliente de pruebas relanza las excepciones de la vista
                    estado = f"{type(error).__name__}: {error}"[:200]
                if not isinstance(estado, int) or estado >= 500:
                    fallos.append(f"{username:20} {nombre:45} {url} -> {estado}")
        self.assertFalse(fallos, "Rutas con error:\n" + "\n".join(fallos))

    def test_dashboard_admin_filtra_por_ecosistema_y_muestra_impacto(self):
        from apps.academico.models import Ecosistema
        from apps.cierre.models import CierreReto
        from apps.retos.models import Reto
        from apps.usuarios.models import Usuario

        from . import services

        ecosistema = Ecosistema.objects.create(nombre="Ecosistema de prueba HU05")
        empresa = Usuario.objects.get(username="empresa.verificada")
        reto = Reto.objects.create(empresa=empresa, titulo="Reto con impacto", estado="finalizado", ecosistema=ecosistema)
        CierreReto.objects.create(reto=reto, impacto="SI")

        kpis = services.kpis_admin(None, None, ecosistema_id=ecosistema.pk)
        self.assertEqual(kpis["retos_total"], 1)
        self.assertEqual(kpis["impacto_si"], 1)
        serie = services.series_admin(None, None, ecosistema_id=ecosistema.pk)["impacto"]
        self.assertEqual(serie["data"], [1])

        self.client.force_login(Usuario.objects.get(username="admin.demo"))
        response = self.client.get("/dashboard/", {"periodo": "todo", "ecosistema": ecosistema.pk})
        self.assertEqual(response.context["kpis"]["impacto_si"], 1)

    def test_rutas_publicas_sin_sesion(self):
        for nombre, url in [("home", "/"), ("login", "/usuarios/login/"), ("registro", "/usuarios/registro/"),
                            ("recuperar", "/usuarios/recuperar-contrasena/")]:
            with self.subTest(nombre):
                self.assertLess(self.client.get(url).status_code, 400)
