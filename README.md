# Rama Caicedo - Modulo de Retos e Integraciones

Esta rama implementa el flujo base para que empresas, profesores y administradores gestionen retos, hackatones, integraciones academicas y seguimientos dentro de la plataforma Retos EAN.

## Alcance principal

- Creacion, edicion, envio a revision y eliminacion de retos por parte de usuarios con rol Empresa.
- Revision administrativa de retos, con aprobacion, rechazo, cambio manual de estado y asignacion de consecutivo.
- Registro de historial de cambios de estado para conservar trazabilidad del ciclo de vida de cada reto.
- Registro de seguimientos sobre retos aprobados o activos.
- Creacion, edicion, envio a revision y publicacion de integraciones academicas por parte de usuarios con rol Profesor.
- Revision administrativa de integraciones academicas.
- Vistas y plantillas HTML para formularios, paneles, detalles, estados y confirmaciones del modulo `retos`.
- Reorganizacion de la configuracion Django en `config/settings/` con archivos separados para base, desarrollo y produccion.
- Estructura inicial de apps de dominio bajo `apps/` para preparar el crecimiento modular del proyecto.

## Modelos incluidos

- `Reto`: representa retos o hackatones creados por empresas, con estados como borrador, en revision, aprobado, rechazado, en curso, pausado, finalizado y cancelado.
- `HistorialEstadoReto`: guarda cada cambio de estado de un reto, el usuario que lo realizo y el comentario asociado.
- `SeguimientoReto`: registra sesiones, avances, observaciones y acuerdos sobre un reto.
- `IntegracionAcademica`: conecta un reto aprobado o activo con una propuesta academica creada por un profesor.

## Rutas principales

- `/retos/mis-retos/`: listado de retos de la empresa.
- `/retos/crear/`: creacion de retos.
- `/retos/<id>/`: detalle del reto.
- `/retos/<id>/seguimientos/`: historial de seguimientos del reto.
- `/retos/admin/panel/`: panel administrativo de revision de retos.
- `/retos/integraciones/`: listado de integraciones del profesor.
- `/retos/integraciones/crear/`: creacion de integraciones academicas.
- `/retos/admin/integraciones/`: panel administrativo de revision de integraciones.

## Configuracion local

1. Crear y activar un entorno virtual.
2. Instalar dependencias:

```bash
pip install -r requirements.txt
```

3. Crear un archivo `.env` basado en `.env.example`.
4. Configurar la base de datos PostgreSQL.
5. Ejecutar migraciones:

```bash
python manage.py migrate
```

6. Levantar el servidor:

```bash
python manage.py runserver
```

## Settings

- Desarrollo: `config.settings.development`
- Produccion: `config.settings.production`

## Notas para continuar

- Validar el flujo completo con usuarios Empresa, Profesor y Admin.
- Agregar pruebas enfocadas para permisos, transiciones de estado y formularios.
- Mantener las migraciones junto con cualquier cambio futuro en modelos.
- Evitar secretos en el repositorio; usar `.env` para credenciales y variables de entorno.
