# Reto EAN - Plataforma de Desafíos Académicos y Hackatones

Proyecto de grado desarrollado para la **Universidad EAN** enfocado en conectar el ecosistema empresarial con la comunidad universitaria mediante la publicación, gestión y evaluación de retos tecnológicos y hackatones.

---

## Roles del Sistema y Flujo de Trabajo

La plataforma gestiona tres tipos de usuarios clave con flujos dinámicos e independientes:

* **Empresas:** Entidades encargadas de proponer, estructurar y publicar retos o hackatones basados en problemáticas reales del sector tecnológico.
* **Profesores:** Actúan como mentores y evaluadores, realizando el seguimiento, revisión y calificación de las propuestas o soluciones entregadas por los alumnos.
* **Estudiantes:** Usuarios finales que exploran el catálogo de retos activos, se postulan a las hackatones y cargan sus proyectos o soluciones directamente en la plataforma.

---

## Requisitos Previos

Antes de levantar el proyecto en tu máquina local, asegúrate de cumplir con lo siguiente:
* **Python 3.10** o superior instalado.
* **Git** configurado en tu sistema.
* Entorno **WSL (Windows Subsystem for Linux)** si te encuentras desarrollando en Windows.

---

## Instalación y Configuración Local

Sigue este orden de comandos en tu terminal para desplegar el entorno de desarrollo:

### 1. Clonar el repositorio y acceder al directorio
git clone URL_DE_TU_REPOSITORIO_AQUÍ
cd retosean-django

### 2. Configurar el Entorno Virtual (Virtual Env)
Para aislar las dependencias de Python del resto de tu sistema, crea y activa el entorno:
python3 -m venv venv
source venv/bin/activate

### 3. Instalar las dependencias oficiales
Utiliza el archivo de requerimientos generado para instalar el framework Django y todos sus componentes adicionales con un solo comando:
pip install -r requirements.txt

### 4. Preparar la Base de Datos (Migraciones)
Aplica la estructura del modelo relacional de usuarios y roles a tu base de datos local:
python manage.py migrate

### 5. Crear una cuenta de Administrador (Opcional)
Si necesitas acceder al panel de administración general de Django (/admin) para gestionar registros manualmente, crea un superusuario:
python manage.py createsuperuser

### 6. Encender el Servidor de Desarrollo
Una vez todo esté configurado, ejecuta el backend de Django:
python manage.py runserver

---

## Acceso a la Aplicación

Con el servidor corriendo localmente, abre tu navegador web de preferencia (se recomienda **Brave**) e ingresa a las siguientes direcciones:

* **Plataforma Principal (Inicio de Sesión):** http://127.0.0.1:8000/usuarios/login/
* **Panel de Administración Global:** http://127.0.0.1:8000/admin/

---

## Estructura del Proyecto

* apps/usuarios/: Módulo encargado del registro dinámico según rol, carga de documentación para empresas y control de perfiles.
* config/: Directorio raíz de configuración global de Django (settings.py, urls.py).