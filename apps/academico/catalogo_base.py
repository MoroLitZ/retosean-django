"""Datos maestros del catalogo academico.

Este modulo es la unica fuente de verdad de las facultades, programas y
ecosistemas que el comando `sembrar_catalogo` materializa en la base de datos.
El equipo puede editar estas listas sin tocar el comando: la siembra es
idempotente (usa get_or_create) y reapunta las filas existentes.

Las facultades salen de la lista institucional que ya usaba
`apps.usuarios.forms.FACULTAD_CHOICES`. Los programas son la union de
`CARRERA_CHOICES` y las opciones que se incrustaban a mano en el formulario de
unidades de estudio, cada uno colgado de su facultad real.
"""

# (nombre, codigo)
FACULTADES = [
    ("Facultad de Ingeniería y Ciencias Básicas", "FICB"),
    ("Facultad de Administración, Finanzas y Ciencias Económicas", "FAFCE"),
    ("Facultad de Derecho", "FDER"),
    ("Facultad de Humanidades y Ciencias Sociales", "FHCS"),
    ("Facultad de Diseño", "FDIS"),
]

# (nombre del programa, nombre de la facultad a la que pertenece)
PROGRAMAS = [
    # Facultad de Ingeniería y Ciencias Básicas
    ("Ingeniería de Sistemas", "Facultad de Ingeniería y Ciencias Básicas"),
    ("Ingeniería Industrial", "Facultad de Ingeniería y Ciencias Básicas"),
    ("Ingeniería Ambiental", "Facultad de Ingeniería y Ciencias Básicas"),
    ("Ingeniería Química", "Facultad de Ingeniería y Ciencias Básicas"),
    ("Ingeniería Biomédica", "Facultad de Ingeniería y Ciencias Básicas"),
    ("Ingeniería Mecatrónica", "Facultad de Ingeniería y Ciencias Básicas"),
    ("Ciencias Ambientales", "Facultad de Ingeniería y Ciencias Básicas"),
    ("Ciencias de Datos", "Facultad de Ingeniería y Ciencias Básicas"),
    # Facultad de Administración, Finanzas y Ciencias Económicas
    ("Administración de Empresas", "Facultad de Administración, Finanzas y Ciencias Económicas"),
    ("Administración de Negocios Internacionales", "Facultad de Administración, Finanzas y Ciencias Económicas"),
    ("Negocios Internacionales", "Facultad de Administración, Finanzas y Ciencias Económicas"),
    ("Contaduría Pública", "Facultad de Administración, Finanzas y Ciencias Económicas"),
    ("Mercadeo", "Facultad de Administración, Finanzas y Ciencias Económicas"),
    ("Finanzas", "Facultad de Administración, Finanzas y Ciencias Económicas"),
    # Facultad de Derecho
    ("Derecho", "Facultad de Derecho"),
    # Facultad de Humanidades y Ciencias Sociales
    ("Psicología", "Facultad de Humanidades y Ciencias Sociales"),
    ("Enfermería", "Facultad de Humanidades y Ciencias Sociales"),
    ("Medicina", "Facultad de Humanidades y Ciencias Sociales"),
    # Facultad de Diseño
    ("Artes Visuales", "Facultad de Diseño"),
    ("Diseño Industrial", "Facultad de Diseño"),
]

# (nombre, descripcion)
ECOSISTEMAS = [
    ("Emprendimiento", "Ecosistema de emprendimiento y creacion de empresas."),
    ("Innovación y Tecnología", "Ecosistema de innovacion, tecnologia y transformacion digital."),
    ("Sostenibilidad", "Ecosistema de sostenibilidad y economia circular."),
    ("Consultoría y Gestión", "Ecosistema de consultoria y gestion empresarial."),
]
