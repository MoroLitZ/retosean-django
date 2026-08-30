from decimal import Decimal

from django.test import TestCase
from django.urls import reverse

from apps.notificaciones.models import Notificacion
from apps.retos.models import Reto
from apps.usuarios.models import Usuario

from .models import Gasto, Presupuesto
from .services import registrar_gasto, revisar_umbral


class PresupuestoModeloTests(TestCase):
    def setUp(self):
        empresa = Usuario.objects.create_user(username="empresa-pre", password="x", rol="EMPRESA")
        self.reto = Reto.objects.create(empresa=empresa, titulo="Reto con plata", estado="aprobado")
        self.presupuesto = Presupuesto.objects.create(reto=self.reto, monto_total=Decimal("1000.00"))

    def test_sin_gastos_el_ejecutado_es_decimal_cero(self):
        ejecutado = self.presupuesto.monto_ejecutado
        self.assertEqual(ejecutado, Decimal("0.00"))
        self.assertIsInstance(ejecutado, Decimal)

    def test_calcula_ejecutado_disponible_y_porcentaje(self):
        Gasto.objects.create(presupuesto=self.presupuesto, categoria="MATERIALES", monto=Decimal("250.00"))
        Gasto.objects.create(presupuesto=self.presupuesto, categoria="REFRIGERIOS", monto=Decimal("150.00"))
        self.assertEqual(self.presupuesto.monto_ejecutado, Decimal("400.00"))
        self.assertEqual(self.presupuesto.monto_disponible, Decimal("600.00"))
        self.assertEqual(self.presupuesto.porcentaje_ejecutado, Decimal("40"))

    def test_presupuesto_en_cero_no_divide_por_cero(self):
        vacio = Presupuesto.objects.create(
            reto=Reto.objects.create(
                empresa=self.reto.empresa, titulo="Sin plata", estado="aprobado"
            ),
            monto_total=Decimal("0.00"),
        )
        self.assertEqual(vacio.porcentaje_ejecutado, Decimal("0.00"))


class AlertaDelOchentaTests(TestCase):
    def setUp(self):
        self.admin = Usuario.objects.create_superuser(
            username="admin-pre", password="x", email="admin@ean.edu.co"
        )
        self.empresa = Usuario.objects.create_user(
            username="empresa-alerta", password="x", rol="EMPRESA"
        )
        self.reto = Reto.objects.create(
            empresa=self.empresa, titulo="Reto vigilado", estado="en_curso"
        )
        self.presupuesto = Presupuesto.objects.create(reto=self.reto, monto_total=Decimal("1000.00"))

    def _gasto(self, monto):
        return registrar_gasto(
            self.presupuesto,
            Gasto(categoria="OTROS", monto=Decimal(monto)),
            self.admin,
        )

    def test_por_debajo_del_umbral_no_alerta(self):
        self._gasto("500.00")
        self.presupuesto.refresh_from_db()
        self.assertFalse(self.presupuesto.alerta_80_enviada)
        self.assertEqual(Notificacion.objects.filter(evento="PRESUPUESTO_80").count(), 0)

    def test_al_superar_el_80_notifica_a_admin_y_empresa(self):
        self._gasto("850.00")
        self.presupuesto.refresh_from_db()
        self.assertTrue(self.presupuesto.alerta_80_enviada)
        destinatarios = set(
            Notificacion.objects.filter(evento="PRESUPUESTO_80").values_list("usuario_id", flat=True)
        )
        self.assertIn(self.admin.pk, destinatarios)
        self.assertIn(self.empresa.pk, destinatarios)

    def test_la_alerta_no_se_repite_en_cada_gasto(self):
        self._gasto("850.00")
        self._gasto("50.00")
        self._gasto("30.00")
        self.assertEqual(Notificacion.objects.filter(evento="PRESUPUESTO_80").count(), 2)

    def test_ampliar_el_presupuesto_rearma_la_alerta(self):
        self._gasto("850.00")
        self.presupuesto.refresh_from_db()
        self.assertTrue(self.presupuesto.alerta_80_enviada)

        self.presupuesto.monto_total = Decimal("5000.00")
        self.presupuesto.save(update_fields=["monto_total"])
        revisar_umbral(self.presupuesto)
        self.presupuesto.refresh_from_db()
        self.assertFalse(self.presupuesto.alerta_80_enviada)


class AccesoAPresupuestoTests(TestCase):
    def setUp(self):
        self.admin = Usuario.objects.create_superuser(
            username="admin-acceso", password="x", email="a@ean.edu.co"
        )
        self.empresa = Usuario.objects.create_user(
            username="empresa-acceso", password="x", rol="EMPRESA"
        )
        self.otra_empresa = Usuario.objects.create_user(
            username="empresa-ajena", password="x", rol="EMPRESA"
        )
        self.reto = Reto.objects.create(
            empresa=self.empresa, titulo="Reto propio", estado="aprobado"
        )
        Presupuesto.objects.create(reto=self.reto, monto_total=Decimal("500.00"))

    def test_admin_ve_el_panel(self):
        self.client.force_login(self.admin)
        self.assertEqual(self.client.get(reverse("presupuesto:panel")).status_code, 200)

    def test_empresa_ve_el_presupuesto_de_su_reto(self):
        self.client.force_login(self.empresa)
        response = self.client.get(reverse("presupuesto:detalle", args=[self.reto.pk]))
        self.assertEqual(response.status_code, 200)

    def test_empresa_no_ve_el_presupuesto_ajeno(self):
        self.client.force_login(self.otra_empresa)
        response = self.client.get(reverse("presupuesto:detalle", args=[self.reto.pk]))
        self.assertEqual(response.status_code, 302)

    def test_empresa_no_puede_registrar_gastos(self):
        self.client.force_login(self.empresa)
        response = self.client.post(
            reverse("presupuesto:agregar_gasto", args=[self.reto.pk]),
            {"categoria": "OTROS", "monto": "100"},
        )
        self.assertEqual(response.status_code, 302)
        self.assertEqual(Gasto.objects.count(), 0)
