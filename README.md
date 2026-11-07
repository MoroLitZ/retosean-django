# RetosEAN — Sistema de Control de Retos Universidad–Empresa

Plataforma nativa en Rust (Axum + SQLx + PostgreSQL) que conecta empresas,
docentes y estudiantes de la Universidad EAN alrededor de retos empresariales:
la empresa publica el reto, el administrador lo aprueba, un docente lo integra
a su curso, los estudiantes se postulan y entregan, el docente evalúa y el
administrador cierra el ciclo emitiendo certificados.

## Estado por historia de usuario

| Sprint | HU | Módulo (Rust) | Estado |
|---|---|---|---|
| 1 | HU13 Usuarios y roles | `accounts.rs`, `import.rs`, `registration.rs` | Registro multirol, login, importación masiva CSV/Excel, bitácora de actividad |
| 2 | HU01 / HU00 Empresas y documentación | `companies.rs`, `media.rs` | Registro, carga de documentos con validación de vigencia, verificación por admin, listas restrictivas |
| 3 | HU02 / HU04 Ciclo del reto | `lifecycle.rs` | Borrador → revisión → aprobación con consecutivo e historial de estados |
| 4 | HU03 Integración académica | `academic.rs`, `team.rs` | Integración por docente, equipos, sesiones y flujo de publicación |
| 5 | HU10 / HU11 Participación | `workflows.rs`, `resources.rs` | Explorador filtrable, favoritos, postulación con validaciones |
| 6 | HU12 / HU14 Evaluación y seguimiento | `grading.rs`, `workflows.rs` | Rúbricas o puntaje libre, historial de comentarios, avance visible para la empresa |
| 7 | HU08 Cierre | `closure.rs` | Agenda, acta, entregables finales, reconocimientos y encuesta |
| 8 | HU15 Notificaciones | `mail.rs`, `reminders.rs`, `workflows.rs` | Motor central, campana in-app, correo, preferencias y recordatorios |
| 9 | HU05 / HU06 / HU07 | `dashboard.rs`, `reports.rs`, `builder.rs`, `finance.rs` | KPIs con Chart.js, reportes PDF/Excel y control presupuestal con alerta al 80% |
| 10 | HU09 / HU16 | `hackathon.rs`, `event.rs` | Hackathon completo (etapas, inscripción, jurados, ranking) y certificados con verificación pública |

## Puesta en marcha

La documentación completa está en [`rust/README.md`](rust/README.md). El arranque
rápido desde la raíz del repositorio (PowerShell) es:

```powershell
.\rust\Start-Retos.ps1
```

Abre http://127.0.0.1:8080. La configuración se toma del `.env` de la raíz del
repositorio (ver `.env.example`). La inicialización de la base de datos, la
semilla de demostración y el resto de comandos están documentados en
[`rust/README.md`](rust/README.md).

## Estructura del repositorio

```
rust/
  src/                      aplicación Axum + SQLx (módulos por dominio)
  web/                      interfaz JavaScript vanilla (SPA), sistema de diseño propio, DESIGN.md
  sql/                      esquema y migraciones SQL
  tests/                    pruebas unitarias, end-to-end y de paridad
docs/
  QA_REPORT.md              informe de aseguramiento de calidad
  decisions.md              decisiones de diseño
.github/workflows/rust.yml  integración continua
```

## Migrar datos desde la versión Django

El comando `import-django` copia en una sola transacción una base de datos
PostgreSQL de Django y sus archivos `media/` a la base de datos de Rust; los
usuarios conservan sus contraseñas. Los pasos exactos están en
[`rust/README.md`](rust/README.md).

## Pruebas

```powershell
cd rust
cargo test --locked -- --include-ignored
```

Requiere PostgreSQL; cada prueba crea su propio esquema y lo elimina al terminar.
