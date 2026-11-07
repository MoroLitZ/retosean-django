"""Pruebas de configuracion y despliegue (hallazgos CFG del informe QA)."""
import importlib
import logging
import os
from unittest.mock import patch

from decouple import UndefinedValueError

from django.conf import settings
from django.contrib.auth.models import AnonymousUser
from django.contrib.auth.password_validation import validate_password
from django.core.exceptions import ValidationError
from django.test import RequestFactory, SimpleTestCase
from django.urls import reverse

import config.settings.base as base_settings
import config.urls as config_urls
from config.registro import FormateadorSeguro


PRODUCCION = "config.settings.production"


def recargar_produccion(sin=(), extra_entorno=None):
    """Reimporta config.settings.production controlando el entorno visible.

    `sin` lista variables que la configuracion debe creer ausentes (decouple
    lanza UndefinedValueError, igual que en un despliegue sin la variable).
    `extra_entorno` anade variables solo durante la recarga. Al terminar, el
    modulo se vuelve a cargar con el entorno real para no dejarlo a medias.
    """
    entorno = {
        "SECRET_KEY": "clave-de-prueba-no-usar",
        "ALLOWED_HOSTS": "testserver",
        "CSRF_TRUSTED_ORIGINS": "https://testserver",
    }
    if extra_entorno:
        entorno.update(extra_entorno)

    config_original = base_settings.config

    def config_filtrado(name, *args, **kwargs):
        if name in sin:
            raise UndefinedValueError(name)
        return config_original(name, *args, **kwargs)

    with patch.dict(os.environ, entorno):
        modulo = importlib.import_module(PRODUCCION)
        try:
            with patch.object(base_settings, "config", config_filtrado):
                return importlib.reload(modulo)
        finally:
            importlib.reload(modulo)


class ProduccionSecretKeyTests(SimpleTestCase):
    def test_sin_secret_key_falla_rapido(self):
        # CFG-01 / MAN-CFG-02: sin SECRET_KEY, produccion no arranca ni cae
        # en un default inseguro conocido.
        with self.assertRaises(UndefinedValueError):
            recargar_produccion(sin=("SECRET_KEY",))

    def test_con_secret_key_arranca(self):
        modulo = recargar_produccion(
            extra_entorno={"SECRET_KEY": "clave-propia-del-despliegue"}
        )
        self.assertEqual(modulo.SECRET_KEY, "clave-propia-del-despliegue")


class ProduccionCsrfTrustedOriginsTests(SimpleTestCase):
    def test_sin_csrf_trusted_origins_falla_rapido(self):
        # CFG-08: no hay default vacio; sin la variable, produccion no arranca.
        with self.assertRaises(UndefinedValueError):
            recargar_produccion(sin=("CSRF_TRUSTED_ORIGINS",))

    def test_origenes_definidos_se_conservan(self):
        modulo = recargar_produccion(
            extra_entorno={
                "CSRF_TRUSTED_ORIGINS": "https://a.example,https://b.example"
            }
        )
        self.assertEqual(
            modulo.CSRF_TRUSTED_ORIGINS, ["https://a.example", "https://b.example"]
        )


class ProduccionProxySslHeaderTests(SimpleTestCase):
    def test_no_confia_en_la_cabecera_por_defecto(self):
        # CFG-03 / MAN-CFG-04: sin opt-in explicito, X-Forwarded-Proto del
        # cliente no tiene ningun efecto.
        modulo = recargar_produccion()
        self.assertIsNone(modulo.SECURE_PROXY_SSL_HEADER)

    def test_confiar_en_la_cabecera_exige_opt_in(self):
        modulo = recargar_produccion(
            extra_entorno={"PROXY_SSL_HEADER_CONFIABLE": "True"}
        )
        self.assertEqual(
            modulo.SECURE_PROXY_SSL_HEADER, ("HTTP_X_FORWARDED_PROTO", "https")
        )


class ValidadoresContrasenaTests(SimpleTestCase):
    def test_validadores_por_defecto_configurados(self):
        # CFG-02: los cuatro validadores de Django estan activos.
        nombres = {v["NAME"] for v in settings.AUTH_PASSWORD_VALIDATORS}
        prefijo = "django.contrib.auth.password_validation."
        self.assertEqual(
            nombres,
            {
                prefijo + "UserAttributeSimilarityValidator",
                prefijo + "MinimumLengthValidator",
                prefijo + "CommonPasswordValidator",
                prefijo + "NumericPasswordValidator",
            },
        )

    def test_rechaza_contrasena_comun(self):
        with self.assertRaises(ValidationError):
            validate_password("12345678")

    def test_rechaza_contrasena_puramente_numerica(self):
        with self.assertRaises(ValidationError):
            validate_password("71351713")


class MediaEnDesarrolloTests(SimpleTestCase):
    def test_servir_media_exige_sesion_iniciada(self):
        # CFG-04 / MAN-CFG-05: en DEBUG un anonimo no puede leer /media/
        # adivinando la URL; se le redirige al login.
        peticion = RequestFactory().get("/media/documentos/convenio.pdf")
        peticion.user = AnonymousUser()
        respuesta = config_urls.media_en_desarrollo(
            peticion,
            path="documentos/convenio.pdf",
            document_root=settings.MEDIA_ROOT,
        )
        self.assertEqual(respuesta.status_code, 302)
        self.assertIn(settings.LOGIN_URL, respuesta.url)


class LimitesSubidaTests(SimpleTestCase):
    def test_archivos_grandes_van_a_disco_no_a_ram(self):
        # CFG-06 / MAN-CFG-06: el umbral de memoria es menor que el tamano
        # maximo de archivo, para que las subidas grandes se vuelquen a disco.
        self.assertLess(settings.FILE_UPLOAD_MAX_MEMORY_SIZE, settings.MAX_UPLOAD_SIZE)
        self.assertEqual(settings.MAX_UPLOAD_SIZE, 50 * 1024 * 1024)
        self.assertIsNotNone(settings.DATA_UPLOAD_MAX_MEMORY_SIZE)


class RegistroSeguroTests(SimpleTestCase):
    def _registro(self, mensaje):
        return logging.LogRecord(
            "apps.usuarios", logging.INFO, "views.py", 1, mensaje, None, None
        )

    def test_formateador_redacta_datos_sensibles(self):
        # CFG-10 / MAN-CFG-08: contrasenas y tokens no llegan al log.
        formato = FormateadorSeguro(format="{message}", style="{")
        salida = formato.format(
            self._registro(
                "POST login password=hunter2 csrfmiddlewaretoken=abc123token"
            )
        )
        self.assertNotIn("hunter2", salida)
        self.assertNotIn("abc123token", salida)
        self.assertIn("password=***", salida)

    def test_el_handler_de_consola_usa_el_formateador_seguro(self):
        self.assertEqual(
            base_settings.LOGGING["handlers"]["console"]["formatter"], "seguro"
        )
        self.assertEqual(
            base_settings.LOGGING["formatters"]["seguro"]["()"],
            "config.registro.FormateadorSeguro",
        )

    def test_logger_de_seguridad_configurado(self):
        self.assertIn("django.security", base_settings.LOGGING["loggers"])


class AdminUrlTests(SimpleTestCase):
    def test_admin_en_la_ruta_configurable(self):
        # CFG-11: la ruta del administrador viene de settings.ADMIN_URL y se
        # puede cambiar por entorno en lugar de dejar "admin/" fijo.
        self.assertEqual(reverse("admin:index"), f"/{settings.ADMIN_URL}")
