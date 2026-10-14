from decimal import Decimal

from django.test import TestCase
from django.urls import reverse

from apps.empresas.models import ContactoEmpresa, DocumentoEmpresa, Empresa
from apps.evaluacion.models import ComentarioEntregable, CriterioRubrica, Entregable, Evaluacion, Rubrica
from apps.participaciones.models import Postulacion
from apps.presupuesto.models import Gasto, Presupuesto
from apps.retos.models import Reto
from apps.usuarios.models import Usuario

from .catalogo import obtener
from .models import ReporteGenerado
from .services import construir_reporte


class GeneracionDeReportesTests(TestCase):
    def setUp(self):
        self.admin = Usuario.objects.create_superuser(
            username="admin-rep", password="x", email="admin@ean.edu.co"
        )
        self.empresa = Usuario.objects.create_user(
            username="empresa-rep", password="x", rol="EMPRESA"
        )
        self.estudiante = Usuario.objects.create_user(
            username="est-rep", password="x", rol="ESTUDIANTE"
        )
        self.profesor = Usuario.objects.create_user(
            username="prof-rep", password="x", rol="PROFESOR"
        )
        self.reto = Reto.objects.create(
            empresa=self.empresa, titulo="Reto reportable", estado="aprobado"
        )
        Postulacion.objects.create(
            reto=self.reto, estudiante=self.estudiante, estado="ACEPTADA", semestre=6,
            motivacion="Quiero aprender", habilidades="Python",
        )
        self.filtros = {"desde": None, "hasta": None}

    def _sembrar_datos_ricos(self):
        """Datos para ejercitar TODAS las columnas nuevas de los 5 reportes."""
        entregable = Entregable.objects.create(
            reto=self.reto, estudiante=self.estudiante, titulo="Avance 1",
            puntaje_maximo=Decimal("5.00"), estado="CALIFICADO", nota=Decimal("4.5"),
            comentario_estudiante="Aqui esta mi avance", comentario_profesor="Buen trabajo",
            archivo="entregables/avance.pdf",
        )
        ComentarioEntregable.objects.create(entregable=entregable, autor=self.profesor, comentario="Recibido.")
        rubrica = Rubrica.objects.create(reto=self.reto, nombre="Rubrica 1", creada_por=self.profesor)
        criterio = CriterioRubrica.objects.create(rubrica=rubrica, nombre="Impacto", puntaje_maximo=Decimal("5.00"))
        Evaluacion.objects.create(
            entregable=entregable, profesor=self.profesor, nota=Decimal("4.5"), rubrica=rubrica,
        )

        presupuesto = Presupuesto.objects.create(reto=self.reto, monto_total=Decimal("1000.00"))
        Gasto.objects.create(
            presupuesto=presupuesto, categoria="MATERIALES", monto=Decimal("200.00"),
            estado=Gasto.COMPROMETIDO,
        )
        Gasto.objects.create(
            presupuesto=presupuesto, categoria="REFRIGERIOS", monto=Decimal("100.00"),
            estado=Gasto.EJECUTADO,
        )

        empresa_perfil = Empresa.objects.create(
            usuario=self.empresa, nit="900123456-1", razon_social="Empresa Reportable",
            sector_industrial="Tecnologia", estado_validacion="VERIFICADA",
        )
        ContactoEmpresa.objects.create(
            empresa=empresa_perfil, nombre="Contacto Uno", cargo="Gerente",
            email="contacto@empresa.co", telefono="3000000000", es_principal=True,
        )
        DocumentoEmpresa.objects.create(empresa=empresa_perfil, tipo_documento="RUT", estado="VERIFICADO")

    def test_genera_pdf_y_registra_el_log(self):
        definicion = obtener("retos")
        registro, contenido, nombre, tipo = construir_reporte(
            self.admin, definicion, definicion.columnas, "PDF", self.filtros
        )
        self.assertTrue(contenido.startswith(b"%PDF"))
        self.assertEqual(tipo, "application/pdf")
        self.assertTrue(nombre.endswith(".pdf"))
        self.assertEqual(registro.usuario, self.admin)
        self.assertEqual(registro.filtros["reporte"], "retos")
        self.assertEqual(registro.filtros["total_registros"], 1)
        self.assertTrue(registro.archivo)

    def test_genera_excel(self):
        definicion = obtener("participacion")
        _, contenido, nombre, tipo = construir_reporte(
            self.admin, definicion, definicion.columnas, "EXCEL", self.filtros
        )
        # Un .xlsx es un zip: empieza por PK.
        self.assertTrue(contenido.startswith(b"PK"))
        self.assertTrue(nombre.endswith(".xlsx"))
        self.assertIn("spreadsheetml", tipo)

    def test_solo_incluye_las_columnas_elegidas(self):
        definicion = obtener("retos")
        columnas = definicion.columnas_por_clave(["titulo", "estado"])
        self.assertEqual([c.clave for c in columnas], ["titulo", "estado"])

    def test_sin_columnas_elegidas_se_incluyen_todas(self):
        definicion = obtener("retos")
        self.assertEqual(
            len(definicion.columnas_por_clave([])), len(definicion.columnas)
        )

    def test_todos_los_reportes_del_catalogo_generan_pdf(self):
        from .catalogo import REPORTES

        for clave, definicion in REPORTES.items():
            with self.subTest(reporte=clave):
                _, contenido, _, _ = construir_reporte(
                    self.admin, definicion, definicion.columnas, "PDF", self.filtros
                )
                self.assertTrue(contenido.startswith(b"%PDF"))

    def test_todos_los_reportes_generan_excel_con_columnas_ampliadas(self):
        """Con datos reales en cada modulo (entregables+evaluacion, gastos
        comprometidos/ejecutados, empresa con contactos y documentos), las
        columnas nuevas de cada reporte no deben tronar ni hacer N+1 raro."""
        from .catalogo import REPORTES

        self._sembrar_datos_ricos()
        for clave, definicion in REPORTES.items():
            with self.subTest(reporte=clave):
                _, contenido, nombre, tipo = construir_reporte(
                    self.admin, definicion, definicion.columnas, "EXCEL", self.filtros
                )
                self.assertTrue(contenido.startswith(b"PK"))
                self.assertTrue(nombre.endswith(".xlsx"))


class AccesoAReportesTests(TestCase):
    def setUp(self):
        self.admin = Usuario.objects.create_superuser(
            username="admin-acc", password="x", email="a@ean.edu.co"
        )
        self.estudiante = Usuario.objects.create_user(
            username="est-acc", password="x", rol="ESTUDIANTE"
        )

    def test_admin_abre_el_constructor(self):
        self.client.force_login(self.admin)
        response = self.client.get(reverse("reportes:constructor"))
        self.assertEqual(response.status_code, 200)

    def test_estudiante_no_accede_a_reportes(self):
        self.client.force_login(self.estudiante)
        response = self.client.get(reverse("reportes:constructor"))
        self.assertEqual(response.status_code, 302)

    def test_no_se_descarga_el_reporte_de_otro(self):
        otro = Usuario.objects.create_user(username="profe-acc", password="x", rol="PROFESOR")
        reporte = ReporteGenerado.objects.create(usuario=self.admin, formato="PDF", filtros={})
        self.client.force_login(otro)
        response = self.client.get(reverse("reportes:descargar", args=[reporte.pk]))
        self.assertRedirects(response, reverse("reportes:historial"))

    def test_generar_desde_la_vista_devuelve_el_archivo(self):
        self.client.force_login(self.admin)
        response = self.client.post(reverse("reportes:constructor"), {
            "reporte": "retos",
            "formato": "EXCEL",
            "periodo": "todo",
        })
        self.assertEqual(response.status_code, 200)
        self.assertIn("attachment", response["Content-Disposition"])
        self.assertEqual(ReporteGenerado.objects.count(), 1)
