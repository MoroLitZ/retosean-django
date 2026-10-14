from django.core.files.uploadedfile import SimpleUploadedFile
from django.test import TestCase
from django.urls import reverse

from .models import LogActividad, Usuario


class RoutingSmokeTests(TestCase):
    def test_login_page_loads(self):
        response = self.client.get(reverse("usuarios:login"))

        self.assertEqual(response.status_code, 200)

    def test_root_redirects_to_profile_workflow(self):
        response = self.client.get("/")

        self.assertEqual(response.status_code, 302)
        self.assertIn(reverse("usuarios:perfil"), response.url)


class RolAdminTests(TestCase):
    """Un ADMIN por rol debe operar igual que un superusuario de Django.

    Antes las vistas de `usuarios` miraban solo `is_superuser`, asi que un
    usuario creado con rol='ADMIN' se quedaba sin panel y sin menu lateral.
    """

    def setUp(self):
        self.admin_por_rol = Usuario.objects.create_user(
            username="admin-rol", password="x", rol="ADMIN"
        )
        self.superusuario = Usuario.objects.create_superuser(
            username="super", password="x", email="super@ean.edu.co"
        )
        self.estudiante = Usuario.objects.create_user(
            username="est-rol", password="x", rol="ESTUDIANTE"
        )

    def test_admin_por_rol_accede_al_panel(self):
        self.client.force_login(self.admin_por_rol)
        # usuarios:admin_dashboard redirige al panel unificado de apps.dashboard.
        response = self.client.get(reverse("usuarios:admin_dashboard"), follow=True)
        self.assertEqual(response.status_code, 200)
        self.assertRedirects(
            self.client.get(reverse("usuarios:admin_dashboard")),
            reverse("dashboard:panel"),
        )

    def test_superusuario_accede_al_panel(self):
        self.client.force_login(self.superusuario)
        response = self.client.get(reverse("dashboard:panel"))
        self.assertEqual(response.status_code, 200)

    def test_admin_por_rol_ve_el_menu_de_administracion(self):
        self.client.force_login(self.admin_por_rol)
        response = self.client.get(reverse("dashboard:panel"))
        self.assertContains(response, reverse("usuarios:lista_usuarios"))

    def test_admin_por_rol_gestiona_usuarios_y_empresas(self):
        self.client.force_login(self.admin_por_rol)
        self.assertEqual(self.client.get(reverse("usuarios:lista_usuarios")).status_code, 200)
        self.assertEqual(self.client.get(reverse("empresas:lista_empresas")).status_code, 200)

    def test_estudiante_no_accede_al_panel_de_admin(self):
        self.client.force_login(self.estudiante)
        response = self.client.get(reverse("usuarios:admin_dashboard"))
        self.assertRedirects(
            response, reverse("usuarios:estudiante_dashboard"),
            target_status_code=302,
        )


class ImportacionMasivaTests(TestCase):
    """HU13: carga masiva de usuarios desde CSV/Excel."""

    def setUp(self):
        self.admin = Usuario.objects.create_superuser(
            username="admin-import", password="x", email="admin@ean.edu.co"
        )
        self.client.force_login(self.admin)
        self.url = reverse("usuarios:importar_usuarios")

    def _csv(self, filas, cabecera="username,email,first_name,last_name,rol,telefono,password"):
        contenido = cabecera + "\n" + "\n".join(filas)
        return SimpleUploadedFile("usuarios.csv", contenido.encode("utf-8"), content_type="text/csv")

    def test_previsualizacion_no_crea_usuarios(self):
        archivo = self._csv(["nuevo1,nuevo1@ean.edu.co,Ana,Diaz,ESTUDIANTE,,"])
        response = self.client.post(self.url, {"archivo": archivo})
        self.assertEqual(response.status_code, 200)
        self.assertFalse(Usuario.objects.filter(username="nuevo1").exists())
        self.assertEqual(len(response.context["lectura"].validas), 1)

    def test_confirmar_crea_los_usuarios(self):
        archivo = self._csv([
            "nuevo2,nuevo2@ean.edu.co,Luis,Mora,ESTUDIANTE,,",
            "nuevo3,nuevo3@ean.edu.co,Sara,Ruiz,PROFESOR,,",
        ])
        self.client.post(self.url, {"archivo": archivo, "accion": "confirmar"})
        self.assertTrue(Usuario.objects.filter(username="nuevo2", rol="ESTUDIANTE").exists())
        self.assertTrue(Usuario.objects.filter(username="nuevo3", rol="PROFESOR").exists())

    def test_detecta_rol_invalido(self):
        archivo = self._csv(["malrol,malrol@ean.edu.co,X,Y,DIRECTOR,,"])
        response = self.client.post(self.url, {"archivo": archivo})
        invalidas = response.context["lectura"].invalidas
        self.assertEqual(len(invalidas), 1)
        self.assertIn("Rol no valido", " ".join(invalidas[0].errores))

    def test_detecta_correo_invalido_y_usuario_repetido(self):
        Usuario.objects.create_user(username="existente", password="x", rol="ESTUDIANTE")
        archivo = self._csv([
            "existente,otro@ean.edu.co,A,B,ESTUDIANTE,,",
            "nuevo4,correo-malo,C,D,ESTUDIANTE,,",
        ])
        response = self.client.post(self.url, {"archivo": archivo})
        self.assertEqual(len(response.context["lectura"].invalidas), 2)

    def test_detecta_duplicados_dentro_del_archivo(self):
        archivo = self._csv([
            "repetido,repetido@ean.edu.co,A,B,ESTUDIANTE,,",
            "repetido,otro@ean.edu.co,C,D,ESTUDIANTE,,",
        ])
        response = self.client.post(self.url, {"archivo": archivo})
        self.assertEqual(len(response.context["lectura"].invalidas), 1)

    def test_rechaza_archivo_sin_columnas_obligatorias(self):
        archivo = self._csv(["Ana,Diaz"], cabecera="first_name,last_name")
        response = self.client.post(self.url, {"archivo": archivo})
        self.assertIn("columnas obligatorias", response.context["lectura"].error_global)

    def test_solo_se_crean_las_filas_validas(self):
        archivo = self._csv([
            "bueno,bueno@ean.edu.co,A,B,ESTUDIANTE,,",
            "malo,malo@ean.edu.co,C,D,ROL_INEXISTENTE,,",
        ])
        self.client.post(self.url, {"archivo": archivo, "accion": "confirmar"})
        self.assertTrue(Usuario.objects.filter(username="bueno").exists())
        self.assertFalse(Usuario.objects.filter(username="malo").exists())

    def test_registra_la_importacion_en_la_bitacora(self):
        archivo = self._csv(["log1,log1@ean.edu.co,A,B,ESTUDIANTE,,"])
        self.client.post(self.url, {"archivo": archivo, "accion": "confirmar"})
        self.assertTrue(LogActividad.objects.filter(accion="IMPORTACION").exists())

    def test_estudiante_no_accede_a_la_importacion(self):
        estudiante = Usuario.objects.create_user(
            username="est-import", password="x", rol="ESTUDIANTE"
        )
        self.client.force_login(estudiante)
        self.assertEqual(self.client.get(self.url).status_code, 302)


class LogActividadTests(TestCase):
    def test_el_login_queda_registrado(self):
        Usuario.objects.create_user(username="log-user", password="clave-larga-123", rol="ESTUDIANTE")
        self.client.post(reverse("usuarios:login"), {
            "username": "log-user", "password": "clave-larga-123",
        })
        self.assertTrue(LogActividad.objects.filter(accion="LOGIN", identificador="log-user").exists())

    def test_el_login_fallido_queda_registrado(self):
        self.client.post(reverse("usuarios:login"), {
            "username": "inexistente", "password": "mala",
        })
        self.assertTrue(LogActividad.objects.filter(accion="LOGIN_FALLIDO").exists())

    def test_cambiar_rol_queda_registrado(self):
        admin = Usuario.objects.create_superuser(
            username="admin-log", password="x", email="a@ean.edu.co"
        )
        objetivo = Usuario.objects.create_user(username="objetivo", password="x", rol="ESTUDIANTE")
        self.client.force_login(admin)
        self.client.post(reverse("usuarios:cambiar_rol", args=[objetivo.pk]), {"rol": "PROFESOR"})
        objetivo.refresh_from_db()
        self.assertEqual(objetivo.rol, "PROFESOR")
        self.assertTrue(LogActividad.objects.filter(accion="CAMBIO_ROL", usuario=objetivo).exists())

    def test_no_puedo_desactivar_mi_propia_cuenta(self):
        admin = Usuario.objects.create_superuser(
            username="admin-self", password="x", email="s@ean.edu.co"
        )
        self.client.force_login(admin)
        self.client.post(reverse("usuarios:alternar_estado", args=[admin.pk]))
        admin.refresh_from_db()
        self.assertTrue(admin.is_active)


class PermisosYBienvenidaTests(TestCase):
    """HU13: permisos especiales por modulo, correo de bienvenida y filtros de la bitacora."""

    def setUp(self):
        self.admin = Usuario.objects.create_user(username="admin-perm", password="x", rol="ADMIN", email="a@ean.edu.co")
        self.lider = Usuario.objects.create_user(username="lider-perm", password="x", rol="ADMIN", email="l@ean.edu.co")

    def test_admin_otorga_y_retira_el_permiso_de_listas_informa(self):
        from .models import LogActividad

        url = reverse("usuarios:permisos_usuario", args=[self.lider.pk])
        self.client.force_login(self.admin)
        self.assertEqual(self.client.get(url).status_code, 200)

        self.client.post(url, {"permisos": ["empresas.cargar_listas_informa"]})
        self.lider = Usuario.objects.get(pk=self.lider.pk)
        self.assertTrue(self.lider.has_perm("empresas.cargar_listas_informa"))

        self.client.post(url, {})
        self.lider = Usuario.objects.get(pk=self.lider.pk)
        self.assertFalse(self.lider.has_perm("empresas.cargar_listas_informa"))
        self.assertEqual(LogActividad.objects.filter(usuario=self.lider, accion="CAMBIO_PERMISOS").count(), 2)

    def test_permisos_desconocidos_se_ignoran(self):
        self.client.force_login(self.admin)
        self.client.post(reverse("usuarios:permisos_usuario", args=[self.lider.pk]), {"permisos": ["auth.delete_user"]})
        self.assertFalse(Usuario.objects.get(pk=self.lider.pk).has_perm("auth.delete_user"))

    def test_solo_el_admin_gestiona_permisos(self):
        estudiante = Usuario.objects.create_user(username="est-perm", password="x", rol="ESTUDIANTE")
        self.client.force_login(estudiante)
        response = self.client.post(reverse("usuarios:permisos_usuario", args=[self.lider.pk]),
                                    {"permisos": ["empresas.cargar_listas_informa"]})
        self.assertNotEqual(response.status_code, 200)
        self.assertFalse(Usuario.objects.get(pk=self.lider.pk).has_perm("empresas.cargar_listas_informa"))

    def test_correo_de_bienvenida_con_enlace_para_definir_contrasena(self):
        from django.core import mail

        from .services import enviar_bienvenida

        nuevo = Usuario.objects.create_user(username="nuevo-import", email="nuevo@ean.edu.co", rol="ESTUDIANTE")
        nuevo.set_unusable_password()
        nuevo.save()
        with self.captureOnCommitCallbacks(execute=True):
            enviar_bienvenida(nuevo)
        self.assertEqual(len(mail.outbox), 1)
        self.assertIn("Bienvenido a RetosEAN", mail.outbox[0].subject)
        self.assertIn("/usuarios/recuperar-contrasena/", mail.outbox[0].body)
        self.assertEqual(mail.outbox[0].to, ["nuevo@ean.edu.co"])

    def test_bitacora_filtra_por_usuario_y_fecha(self):
        from .models import LogActividad

        LogActividad.objects.create(identificador="buscado", accion="LOGIN")
        LogActividad.objects.create(identificador="otro", accion="LOGIN")
        self.client.force_login(self.admin)
        response = self.client.get(reverse("usuarios:log_actividad"), {"usuario": "buscado"})
        identificadores = {r.identificador for r in response.context["registros"]}
        self.assertEqual(identificadores, {"buscado"})
        response = self.client.get(reverse("usuarios:log_actividad"), {"desde": "2000-01-01", "hasta": "2000-01-02"})
        self.assertEqual(len(response.context["registros"]), 0)
