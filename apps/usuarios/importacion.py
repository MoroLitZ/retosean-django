"""Importacion masiva de usuarios desde CSV o Excel (HU13).

El flujo es en dos pasos: primero se previsualiza el archivo (validando fila a
fila y sin escribir nada) y solo despues se confirma la creacion. Asi el
administrador ve los errores antes de tocar la base de datos.
"""

import csv
import io
from dataclasses import dataclass, field

from django.contrib.auth.password_validation import validate_password
from django.core.exceptions import ValidationError
from django.core.validators import validate_email
from django.db import transaction

from .models import LogActividad, Usuario

COLUMNAS = ["username", "email", "first_name", "last_name", "rol", "telefono", "password"]
COLUMNAS_OBLIGATORIAS = ["username", "email", "rol"]
ROLES_VALIDOS = {codigo for codigo, _ in Usuario.ROL_CHOICES}
MAX_FILAS = 1000


@dataclass
class FilaImportacion:
    numero: int
    datos: dict
    errores: list = field(default_factory=list)

    @property
    def valida(self):
        return not self.errores


@dataclass
class ResultadoLectura:
    filas: list = field(default_factory=list)
    error_global: str = ""

    @property
    def validas(self):
        return [f for f in self.filas if f.valida]

    @property
    def invalidas(self):
        return [f for f in self.filas if not f.valida]


def _normalizar_cabeceras(cabeceras):
    return [str(c or "").strip().lower().replace(" ", "_") for c in cabeceras]


def _leer_csv(archivo):
    contenido = archivo.read()
    for codificacion in ("utf-8-sig", "latin-1"):
        try:
            texto = contenido.decode(codificacion)
            break
        except UnicodeDecodeError:
            continue
    else:
        raise ValueError("No se pudo leer el archivo: codificacion no reconocida.")

    # Detectamos si el separador es coma o punto y coma (Excel en espanol).
    muestra = texto[:2048]
    try:
        dialecto = csv.Sniffer().sniff(muestra, delimiters=",;\t")
    except csv.Error:
        dialecto = csv.excel

    lector = csv.reader(io.StringIO(texto), dialecto)
    filas = list(lector)
    if not filas:
        raise ValueError("El archivo esta vacio.")
    return _normalizar_cabeceras(filas[0]), filas[1:]


def _leer_excel(archivo):
    from openpyxl import load_workbook

    libro = load_workbook(archivo, read_only=True, data_only=True)
    hoja = libro.active
    filas = list(hoja.iter_rows(values_only=True))
    if not filas:
        raise ValueError("El archivo esta vacio.")
    cabeceras = _normalizar_cabeceras(filas[0])
    cuerpo = [["" if v is None else str(v).strip() for v in fila] for fila in filas[1:]]
    return cabeceras, cuerpo


def leer_archivo(archivo):
    """Lee y valida el archivo sin escribir nada en la base de datos."""
    resultado = ResultadoLectura()
    nombre = (archivo.name or "").lower()

    try:
        if nombre.endswith((".xlsx", ".xlsm")):
            cabeceras, cuerpo = _leer_excel(archivo)
        elif nombre.endswith(".csv"):
            cabeceras, cuerpo = _leer_csv(archivo)
        else:
            resultado.error_global = "Formato no soportado. Usa un archivo .csv o .xlsx."
            return resultado
    except Exception as exc:  # noqa: BLE001 - el mensaje se muestra al usuario
        resultado.error_global = f"No se pudo leer el archivo: {exc}"
        return resultado

    faltantes = [c for c in COLUMNAS_OBLIGATORIAS if c not in cabeceras]
    if faltantes:
        resultado.error_global = (
            "Al archivo le faltan estas columnas obligatorias: " + ", ".join(faltantes)
            + ". Columnas esperadas: " + ", ".join(COLUMNAS) + "."
        )
        return resultado

    if len(cuerpo) > MAX_FILAS:
        resultado.error_global = (
            f"El archivo tiene {len(cuerpo)} filas y el maximo permitido es {MAX_FILAS}."
        )
        return resultado

    indices = {columna: cabeceras.index(columna) for columna in cabeceras if columna in COLUMNAS}
    usernames_vistos, correos_vistos = set(), set()

    for numero, fila in enumerate(cuerpo, start=2):
        if not any(str(valor).strip() for valor in fila):
            continue  # fila en blanco al final del archivo

        datos = {
            columna: str(fila[indice]).strip() if indice < len(fila) and fila[indice] is not None else ""
            for columna, indice in indices.items()
        }
        registro = FilaImportacion(numero=numero, datos=datos)
        _validar_fila(registro, usernames_vistos, correos_vistos)
        resultado.filas.append(registro)

    if not resultado.filas:
        resultado.error_global = "El archivo no contiene filas de datos."
    return resultado


def _validar_fila(registro, usernames_vistos, correos_vistos):
    datos = registro.datos
    username = datos.get("username", "")
    email = datos.get("email", "")
    rol = (datos.get("rol") or "").upper()
    datos["rol"] = rol

    if not username:
        registro.errores.append("El usuario es obligatorio.")
    elif username.lower() in usernames_vistos:
        registro.errores.append("El usuario esta repetido dentro del archivo.")
    elif Usuario.objects.filter(username__iexact=username).exists():
        registro.errores.append("Ya existe un usuario con ese nombre.")
    else:
        usernames_vistos.add(username.lower())

    if not email:
        registro.errores.append("El correo es obligatorio.")
    else:
        try:
            validate_email(email)
        except ValidationError:
            registro.errores.append("El correo no tiene un formato valido.")
        else:
            if email.lower() in correos_vistos:
                registro.errores.append("El correo esta repetido dentro del archivo.")
            elif Usuario.objects.filter(email__iexact=email).exists():
                registro.errores.append("Ya existe un usuario con ese correo.")
            else:
                correos_vistos.add(email.lower())

    if not rol:
        registro.errores.append("El rol es obligatorio.")
    elif rol not in ROLES_VALIDOS:
        registro.errores.append(
            "Rol no valido. Usa uno de: " + ", ".join(sorted(ROLES_VALIDOS)) + "."
        )

    contrasena = datos.get("password", "")
    if contrasena:
        try:
            validate_password(contrasena)
        except ValidationError as exc:
            registro.errores.append("Contrasena debil: " + " ".join(exc.messages))


@transaction.atomic
def importar(filas_validas, realizado_por):
    """Crea los usuarios de las filas validas. Devuelve los creados."""
    creados = []
    for registro in filas_validas:
        datos = registro.datos
        usuario = Usuario(
            username=datos["username"],
            email=datos["email"],
            first_name=datos.get("first_name", ""),
            last_name=datos.get("last_name", ""),
            rol=datos["rol"],
            telefono=datos.get("telefono", ""),
        )
        contrasena = datos.get("password", "")
        if contrasena:
            usuario.set_password(contrasena)
        else:
            # Sin contrasena en el archivo la cuenta queda inutilizable hasta
            # que la persona use "olvide mi contrasena".
            usuario.set_unusable_password()
        usuario.save()
        creados.append(usuario)

    if creados:
        LogActividad.objects.create(
            realizado_por=realizado_por,
            accion="IMPORTACION",
            detalle=(
                f"Importacion masiva de {len(creados)} usuario(s): "
                + ", ".join(u.username for u in creados[:20])
                + ("..." if len(creados) > 20 else "")
            ),
        )
    return creados


def plantilla_csv():
    """Contenido de la plantilla de ejemplo que puede descargar el administrador."""
    buffer = io.StringIO()
    escritor = csv.writer(buffer)
    escritor.writerow(COLUMNAS)
    escritor.writerow([
        "jperez", "jperez@universidadean.edu.co", "Juana", "Perez",
        "ESTUDIANTE", "3001234567", "",
    ])
    escritor.writerow([
        "mgomez", "mgomez@universidadean.edu.co", "Mario", "Gomez",
        "PROFESOR", "3009876543", "",
    ])
    return buffer.getvalue()
