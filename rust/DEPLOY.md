# Deploying RetosEAN (Rust)

How to run the Rust version as a public site. The whole application, including
the web pages, is a single binary (`retosean`) backed by PostgreSQL.

## Settings

Set these as environment variables. The binary also reads `.env` and `../.env`.

| Variable | Production value | Default |
|---|---|---|
| `DB_HOST`, `DB_PORT`, `DB_USER`, `DB_PASSWORD` | Your PostgreSQL server | `localhost`, `5432`, `retosean`, empty |
| `RUST_DB_NAME` | Database name (1–63 ASCII letters, digits or underscores), e.g. `retosean_rust` | `retosean_rust` |
| `RUST_POSTGRES_PASSWORD` | Required by Compose; passed as a Docker secret | no default |
| `RUST_BIND` | `127.0.0.1:8080` behind a proxy; `0.0.0.0:8080` in Docker | `127.0.0.1:8080` |
| `RUST_SITE_URL` | Public address, e.g. `https://retos.example.edu.co`; used in email links | `http://<RUST_BIND>` |
| `DB_PASSWORD_FILE` | Path to a file holding the password (Docker secret); trailing CR/LF are stripped; wins over `DB_PASSWORD` | unset |
| `RUST_SECURE_COOKIE` | `true`, required on HTTPS | `true` (`false` logs a warning; set `false` only for local HTTP) |
| `RUST_TRUST_PROXY` | `true` only behind a reverse proxy that sets `X-Forwarded-For`; the proxy must also keep `Host` or send `X-Forwarded-Host`, because cross-site POSTs are refused by Origin | `false` |
| `RUST_MEDIA_DIR` | A persistent folder, e.g. `/data/media` | `.rust-media` |
| `RUST_LOG` | Rust log filter | `retosean=info` |
| `EMAIL_HOST`, `EMAIL_PORT`, `EMAIL_HOST_USER`, `EMAIL_HOST_PASSWORD`, `EMAIL_USE_TLS`, `DEFAULT_FROM_EMAIL` | Your SMTP account | Without `EMAIL_HOST`, emails are only written to the log |

## Steps to go live

1. **Server.** A Linux machine with PostgreSQL. `compose.yml` in this folder
   runs PostgreSQL 18 and the app together; set `RUST_POSTGRES_PASSWORD` in
   `rust/.env` (see `.env.example`) — compose refuses to start without it.
   The container runs with a read-only root filesystem, a `tmpfs` `/tmp`,
   `cap_drop: ALL`, `no-new-privileges`, `mem_limit: 1g`, health checks, the
   database password as a Docker secret, and the web port published only on
   `127.0.0.1:8080`. The image sets `RUST_BIND=0.0.0.0:8080` and
   `RUST_MEDIA_DIR=/data/media` (volume `rust_media`).
2. **Build.** `cargo build --release` produces `target/release/retosean`, or
   build the Docker image from the `Dockerfile`. Release builds use fat LTO,
   one codegen unit and stripped debug info.
3. **Create the database (once).** Run `retosean init-db` (creates the database
   if absent and the schema only if the `public` schema has no tables), then
   load data:
   - Real data from Django: `retosean import-django retosean_django /path/to/django/media`
     (both databases on the same PostgreSQL server; the target must be empty).
   - Demo data instead: `retosean seed-demo` (only when the `usuarios` table is
     empty; all accounts use `RetosEAN2026`).
4. **HTTPS.** Put Caddy or Nginx in front, forwarding your domain to
   `127.0.0.1:8080`. The app does not handle TLS itself. Set
   `RUST_SECURE_COOKIE=true` and `RUST_SITE_URL`. The default request body limit
   is 64 KB. User import and reto applications accept 6 MB; most file uploads
   10 MB; progress and reto support-file uploads up to 110 MB per request
   (each file at most 50 MB, at most 10 files and 100 MB per request). Set the
   reverse proxy body limit to at least 110 MB.
5. **Keep it running.** A systemd service, or Docker with
   `restart: unless-stopped`, running `retosean serve`.
6. **Scheduled jobs.**
   - `retosean advance-hackathons` every hour.
   - `retosean send-reminders` once a day.
7. **Backups.** `pg_dump` of the database plus a copy of `RUST_MEDIA_DIR`
   (for Compose, the `rust_media` volume). Uploaded files are stored only there.
8. **Updates.** Stop the service, replace the binary, run `retosean migrate`,
   then start it again. Never run Django migrations against this database.
9. **Monitoring.** `GET /health` returns
   `{"status":"ok","runtime":"rust","database":"postgresql"}` when the app and
   database are up.

## Commands

| Command | What it does |
|---|---|
| `retosean serve` | Runs the site (the default) |
| `retosean init-db` | Creates the database if absent and the schema only if the `public` schema has no tables |
| `retosean migrate` | Applies pending schema changes from `sql/migrations` |
| `retosean seed-demo` | Loads the demo accounts and data only when the `usuarios` table is empty |
| `retosean import-django <db> <media_dir>` | Copies a Django database and its files into an empty database |
| `retosean advance-hackathons` | Moves hackathons through their stages and publishes results |
| `retosean send-reminders [days...]` | Sends deadline reminders (default 7, 3 and 1 days before) |

## Process

`retosean serve` shuts down gracefully on Ctrl-C/SIGTERM and, on start, removes
interrupted upload spool files (`.<uuid>.part`) from `RUST_MEDIA_DIR`.

## Endpoints

All routes are defined in `src/lib.rs`. Requests that change data (POST) need
the session cookie and an `X-CSRF-Token` header; the web app sends both.

**Pages and status**
- `/` and `/assets/{file}`: the web app (HTML, CSS and scripts embedded in the binary; no CDN)
- `GET /health`
- `GET /api/verificar/{code}`: public certificate check

**Login and accounts**
- `POST /api/login`, `POST /api/logout`, `GET /api/me`
- `POST /api/registro`, `GET /api/registro/opciones`
- `POST /api/registro/verificar` (email verification)
- `POST /api/password/reset`, `POST /api/password/reset/confirm`, `POST /api/password`
- `GET`/`POST /api/perfil`

**Users (admin)**
- `POST /api/usuarios`
- `POST /api/usuarios/importar`, `GET /api/usuarios/plantilla`
- `POST /api/usuarios/{id}/estado`, `POST /api/usuarios/{id}/rol`
- `GET`/`POST /api/usuarios/{id}/permisos`
- `GET /api/log-actividad`: activity log with filters

**Browsing and exports**
- `GET /api/resources`
- `GET /api/records/{resource}`, `GET /api/records/{resource}/{id}`
- `GET /api/export/{resource}`, `GET /api/export/{resource}/{pdf|xlsx}`
- `GET /api/archivo/{resource}/{id}/{field}`: file downloads

**Catalogs and study units**
- `POST /api/catalogos/{resource}`
- `POST /api/unidades`, `POST /api/unidades/{id}`, `POST /api/unidades/{id}/activo`

**Companies**
- `POST /api/empresas/{id}/revisar`, `/convenio`, `/bloqueo`
- `POST /api/empresas/{id}/documentos/{kind}/cargar`
- `POST /api/empresa/modo-convenio`, `POST /api/empresa/documentos/{kind}/cargar`
- `GET /api/empresas/{id}/historial-documental`
- `POST /api/tipos-documento`, `POST /api/tipos-documento/{id}`: document catalog (admin)
- `POST /api/documentos/{id}/revisar`, `POST /api/documentos/{id}/cargar`

**Retos**
- `POST /api/retos`, `POST /api/retos/{id}`
- `POST /api/retos/{id}/estado`, `/favorito`, `/postular`, `/entregar`, `/seguimiento`, `/presupuesto`, `/integrar`, `/archivos`, `/eliminar`
- `GET /api/retos/{id}/avance`: the company's progress view
- `POST /api/postulaciones/{id}/estado`

**Academic work and teams**
- `GET /api/integraciones/opciones`, `POST /api/integraciones/{id}`
- `POST /api/integraciones/{id}/estado`, `GET`/`POST /api/integraciones/{id}/equipo`
- `POST /api/retos/{id}/equipo`, `GET /api/retos/{id}/opciones`
- `GET`/`POST /api/equipos-academicos/{id}`, `POST /api/equipos-academicos/{id}/eliminar`

**Grading**
- `POST /api/retos/{id}/rubrica`, `POST /api/retos/{id}/publicar-calificaciones`
- `GET /api/entregables/{id}/rubricas`, `POST /api/entregables/{id}/calificar`, `POST /api/entregables/{id}/comentario`

**Closure**
- `POST /api/retos/{id}/cerrar`
- `GET`/`POST /api/retos/{id}/cierre`
- `POST /api/retos/{id}/cierre/entregables`, `POST /api/retos/{id}/cierre/reconocimientos`
- `POST /api/retos/{id}/solicitudes-logistica`, `POST /api/solicitudes-logistica/{id}/estado`
- `POST /api/encuestas/{id}`
- `GET /api/certificados/{id}/pdf`, `GET /api/portafolio`

**Hackathons**
- `POST /api/retos/{id}/hackathon`
- `POST /api/hackathons/{id}/estado`, `/etapa`, `/jurado`, `/inscribir`, `/votar`
- `GET /api/hackathons/{id}/ranking`, `GET /api/hackathons/{id}/opciones`
- `GET`/`POST /api/hackathons/{id}/logistica`, `GET /api/hackathons/{id}/informe/{format}`
- `POST /api/inscripciones/{id}/estado`

**Budget**
- `POST /api/presupuestos/{id}/gastos`, `GET /api/presupuestos/{id}/resumen`
- `POST /api/gastos/{id}/ejecutar`, `POST /api/gastos/{id}/eliminar`
- `GET /api/retos/{id}/presupuesto/{xlsx|pdf}`

**Dashboard and reports**
- `GET /api/dashboard`
- `GET /api/reportes/catalogo`
- `POST /api/reportes/vista-previa`, `POST /api/reportes/generar`, `POST /api/reportes/enviar`

**Notifications**
- `POST /api/notificaciones/{id}/leer`, `POST /api/preferencias`
- `GET /api/notificaciones/mias` (own notifications)
- `GET /api/notificaciones/registro`: delivery log (admin)