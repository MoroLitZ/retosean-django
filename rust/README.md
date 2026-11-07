# RetosEAN in Rust

A native Axum + SQLx + PostgreSQL implementation on `Rust-attempt`. The server,
authentication, authorization, domain workflows, uploads, reports and demo seed
run without Python or Django. Browser presentation uses HTML, CSS and JavaScript,
embedded in the executable. There is no Node build or development server.

The web application is a vanilla-JS single-page app in `web/`, built with an
in-house design system (`web/DESIGN.md`). CSS and JavaScript are split per area
in `web/css` and `web/js`, embedded in the executable, and served from
`/assets/{file}`. It loads no external resources, so the browser needs no
internet access.

This is the migrated application; the former Django version has been removed
from this repository. Coverage and verification sections below describe parity
with that former version.

To run it as a public site (settings, HTTPS, scheduled jobs, backups and the full endpoint list), see [DEPLOY.md](DEPLOY.md).

## Start on this computer

PostgreSQL, Rust, a separate `retosean_rust` database and demo records have been
prepared locally. From the repository root in PowerShell:

```powershell
.\rust\Start-Retos.ps1
```

Open http://127.0.0.1:8080. Stop with Ctrl+C. If PowerShell blocks local scripts,
use the executable directly:

```powershell
cd rust
& "$env:USERPROFILE\.cargo\bin\cargo.exe" run --locked -- serve
```

For an optimized build, use `Start-Retos.ps1 -Release` or
`cargo run --locked --release -- serve`. Debug builds verify passwords more slowly.

## First setup on another computer

Install a Rust toolchain and PostgreSQL. On Windows the MSVC toolchain also needs
Visual Studio C++ Build Tools. Copy `.env.example` to `.env` inside `rust/` and
set the PostgreSQL credentials, or reuse the repository's existing `.env`.

The PostgreSQL role needs CREATEDB for initialization. It needs ownership of the
new schema for the bootstrap DDL. From the repository root:

```powershell
.\rust\Start-Retos.ps1 -Initialize
```

Equivalent commands from `rust/`:

```text
cargo run --locked -- init-db
cargo run --locked -- seed-demo
cargo run --locked -- serve
cargo run --locked -- advance-hackathons   # schedule hourly (HU09)
cargo run --locked -- send-reminders       # schedule daily (HU15), like enviar_recordatorios
```

`init-db` only initializes an empty database. `seed-demo` only runs when the user
table is empty. Both refuse to reset existing data. Use a new `RUST_DB_NAME` for
a fresh demonstration. The legacy Django `DB_NAME` is never implicitly selected.

## Demo accounts

All newly seeded accounts use `RetosEAN2026`:

| Account | Role |
|---|---|
| `admin.demo` | Administrator |
| `empresa.verificada` | Verified company |
| `profesor.demo` | Professor |
| `estudiante.demo` | Student |
| `experto.demo` | Expert / assigned hackathon juror |

These are local demonstration credentials. Do not seed an internet-facing system.

## Native coverage

| Area | Implemented in Rust |
|---|---|
| Data model | 49 typed domain records, PostgreSQL bootstrap schema with database defaults mirroring the former Django version's Python defaults, seven state-machine definitions, one audit table for every state change |
| Users | Bulk import from CSV or Excel with row-by-row preview, unusable passwords plus a set-password email for rows without one (HU13); public self-registration (Comunidad EAN with institutional email, or company with its catalog documents) with welcome email, password recovery by single-use 3-day email link, Django PBKDF2-SHA256 compatible login, expiring sessions, CSRF, login throttling, admin account creation, login by username or email (as in the former Django version), contact editing, password changes and revocation, activation/deactivation, audited role changes and per-module permissions such as Cargar Listas Informa (HU13) |
| Companies | Configurable legal-document catalog (HU00): PDF uploads, catalog validity (e.g. 30-day Cámara de Comercio), document decisions, status derived from the documents as in the former Django version, restrictive-list checks, agreement transitions and expiry dates, participation block with reason, the agreement-or-not choice, special documents uploaded only by permission holders, and the former Django version's operating gate (block, lists, every required document verified and in force) before publishing retos |
| Challenges | Company draft creation/editing, required-field review submission, admin approval/rejection, serialized numbering, audited manual state transitions |
| Academic integration | Professor proposal with structured team (cédula, email, faculty/program) and dated schedule (HU03), the same review requirements, admin review, publication that opens the challenge and notifies assigned students |
| Participation | Search, favorites, applications and withdrawal, acceptance/rejection, academic and operational team creation, editing (professors, students, invited experts) and deletion with the operational mirror team kept in sync, as the former Django version's sincronizar_equipo_academico did |
| Deliverables | Private PDF/image/text upload, replacement; grading by free score or rubric criteria (HU12) with re-grade confirmation; grades hidden from students until the professor publishes them, then notified |
| Progress | Dated progress entries, 0–100% validation, company notification |
| Closure | Closure management (HU08) for the company, integrated professors and admins: agenda and acta with the solution's impact, final artifact uploads, recognitions limited to participating students, logistics requests for the start and closing events with admin confirm/discard and history; formal close with required deliverable, artifact linking, state transition, idempotent certificates and satisfaction surveys |
| Certificates | Owner/admin PDF download and public UUID verification |
| Notifications | The former Django version's event catalog (titles, types), in-app bell, preferences and deduplication, email outbox within 20 s using the former Django version's email template and EMAIL_* settings (sent/failed recorded), daily reminders (`send-reminders`, same rules as `enviar_recordatorios`) |
| Finance | Exact decimal budgets/expenses, role-scoped summaries, 80% threshold notifications |
| Hackathons | Creation, stages/time windows, jurors (experts included), team registration/review, voting, ranking, automatic advance and result publication at close, event logistics with guests, final report in PDF/Excel (HU09) |
| Reports | The former Django version's report builder (HU06): the same five reports, columns, filters and roles, with preview, PDF/Excel generation logged in the history, and sending by email as an attachment; verified cell by cell against the former Django version's catalog. Also role-scoped CSV/PDF/Excel exports of any module |
| Dashboard | Port of the former Django dashboard: the same per-role KPIs, charts, quick links and period/faculty/program filters, including the expert juror panel, ecosystem filter and impact indicators; verified to return identical numbers to the former Django version's services on the same database |
| Catalogs | Native creation of faculties, programs and ecosystems; study units managed by admins and professors (create, edit, optional PDF, deactivate/activate, CSV bulk load matched by program name); role-scoped browsing |

All 49 domain tables have authenticated, paginated read access with an explicit
role policy. Unmapped tables default to administrator-only access. This read
coverage does not imply every original form or command has been ported.

## Known limitations

- Deliberate differences from the former Django version: passwords need at
  least 12 characters, and generic module exports (a Rust addition) are capped
  at 5,000 records. Excel stores values as literal strings to preserve
  identifiers, exact decimals and formula safety.
- PDFs use the standard Helvetica fonts, which cover Windows-1252 (all Spanish
  text); other characters print as "?". Embedding a Unicode font is the upgrade.
- Before production: a deployment review (HTTPS, `RUST_SECURE_COOKIE=true`,
  backups, scheduled `send-reminders` and `advance-hackathons`).

## Schema changes

`init-db` creates the full schema once. Later changes go in
`sql/migrations/NNNN_name.sql`, listed in `src/schema_migrations.rs`, and are
applied with `retosean migrate` (each once, in order, recorded in
`rust_schema_migrations`). Never run migrations from the former Django project
against a Rust database.

`seed-demo` also verifies the demo company's required documents, so the company can
publish retos right away; the end-to-end test loads the bare demo data and
walks the verification itself.

## Importing an existing Django database

On the same PostgreSQL server, create an empty Rust database and copy the former
Django version's rows and media into it:

```powershell
$env:RUST_DB_NAME = "retosean_rust_prod"
.\target\debug\retosean.exe init-db
.\target\debug\retosean.exe import-django retosean_django ..\media
```

The import runs in one transaction and refuses any database that already has
rows. Every shared table is copied (sessions and migration history excluded),
sequences are reset, and each referenced file under the former Django project's
`MEDIA_ROOT` is copied to `RUST_MEDIA_DIR` with a random name. Files missing on
disk or of an unsupported type are listed and keep their old path. Accounts keep
their Django password hashes, so users sign in with the same passwords. On the
development database this copied 962 rows from 59 tables and 52 files, row
counts matched table by table, and the dashboard parity check gave 0 mismatches.

Rust uploads are stored under `RUST_MEDIA_DIR` using random names and are served
only through authorization-checked download endpoints. Media from the former
Django project is served only after `import-django` copies it under new names.
File replacement leaves old blobs for a future retention/garbage-collection
policy. It clears old evaluation scores rather than treating a changed file as
already graded.

## Ten-minute showcase

Use separate browser profiles for the four main roles. Existing seeded challenges
provide examples of each challenge status. To demonstrate a fresh complete flow:

1. Company: create a challenge, complete dates and description, submit for review.
2. Admin: approve the challenge; show its consecutive number and audit history.
3. Professor: propose an academic integration and submit it for review.
4. Admin: approve the integration. Professor: publish it; the challenge opens.
5. Student: find the challenge, favorite it and apply. Company: accept the application.
6. Admin: move the challenge into progress. Student: upload a real sample PDF or TXT.
7. Professor: grade the deliverable and register progress. Company: see the update.
8. Admin: define a budget and record a gasto reaching 80%; show its notification.
9. Admin or professor: close the challenge. Student: download the certificate PDF
   and verify its code. Respond to the satisfaction survey.
10. Admin: export a module to Excel/PDF. Optionally demonstrate the hackathon
    flow with a challenge whose type is `hackathon`.

## Verification

From `rust/`:

```text
cargo fmt --check
for f in web/js/*.js; do node --check "$f"; done
cargo test --locked -- --include-ignored
```

That runs three layers:

- **Unit tests** (`src/`): parsing, money, PDF text, state machines.
- **End-to-end** (`tests/end_to_end.rs`): one long walk through the whole
  platform over real Axum handlers and PostgreSQL.
- **Django parity** (`tests/parity_<app>.rs`): all 271 tests from the former
  Django version are recreated against the Rust API, one Rust test per former
  Django test with the same name, grouped by the former Django test class. A few
  adapt where the two stacks differ by design (the API returns 404 instead of
  redirecting, the single-page app replaces server-rendered pages); each such
  test says so in a comment. The route walk in `parity_dashboard.rs` reads the
  GET routes from `src/lib.rs`, so a new route is visited by every role
  automatically.

Every database test creates its own schema in the configured Rust database
with the demo data, and drops it afterwards; nothing touches demonstration
rows or tables from the former Django project. The database role needs CREATE on
the Rust database. Tests use a low-cost password hash only inside those schemas;
real accounts use 1,000,000 PBKDF2 iterations. To run one app's suite:
`cargo test --test parity_empresas -- --include-ignored`.

GitHub Actions runs all of it on every change under `rust/`
(`.github/workflows/rust.yml`).

## Load test

`examples/stress.rs` is a read-only load generator with no extra dependencies.
Start a release server on a spare port, then ramp the concurrency:

```text
$env:RUST_BIND='127.0.0.1:8082'; cargo run --release -- serve
cargo run --release --example stress -- 127.0.0.1:8082 15 1,10,50,100,250,500,1000,2000
```

It logs in once per demo role (each login counts toward the 10-attempts-per-10-minutes
throttle), then each worker keeps a keep-alive connection and loops over the
dashboard, record lists, a record detail, search, static assets and `/health`,
skipping endpoints a role may not open. It reports throughput, latency
percentiles and any non-2xx response by status and path.

Reference run (release build, 24-core Windows desktop, PostgreSQL 18 on the same
machine, demo data, pool of 10 connections):

| Concurrent | req/s | p50 ms | p95 ms | p99 ms | errors |
|---:|---:|---:|---:|---:|---:|
| 1 | 981 | 0.9 | 2.0 | 2.4 | 0 |
| 10 | 8,501 | 1.0 | 2.7 | 3.4 | 0 |
| 100 | 7,803 | 12.3 | 27.5 | 42.2 | 0 |
| 500 | 7,067 | 67.1 | 144.9 | 231.3 | 0 |
| 1000 | 7,069 | 135.6 | 285.0 | 457.8 | 0 |
| 2000 | 6,730 | 284.5 | 589.7 | 970.3 | 150 connect errors |

Throughput saturates near 8,500 req/s from about 10 concurrent requests; beyond
that, latency grows with the queue while no request fails until 2,000
connections. Server memory stays under 250 MB. A login takes about 100 ms
(1,000,000 PBKDF2 iterations, 4 concurrent hashing slots), so logins top out
around 40 per second. A 40-connection pool raised high-concurrency throughput
by about 25%, so the database round trips per request are the main cost.

## Docker

From `rust/`, using the separate Compose stack:

```text
docker compose up -d db
docker compose run --build --rm web init-db
docker compose run --rm web seed-demo
docker compose up -d web
```

The stack binds the web port to localhost and keeps its own database/media volumes.
Use `docker compose down` to stop it. The default database password is for local
development; set `RUST_POSTGRES_PASSWORD` for another environment. The Docker
build must be validated in the target deployment environment.

## Implementation map

- `src/auth.rs`, `accounts.rs`: sessions, password compatibility, account operations.
- `src/resources.rs`: allowlisted tables, row policies, search and pagination.
- `src/workflows.rs`, `academic.rs`, `lifecycle.rs`: transactional domain workflows.
- `src/companies.rs`, `finance.rs`, `hackathon.rs`: module-specific business rules.
- `src/dashboard.rs`: port of the former Django dashboard indicators (HU05).
- `src/team.rs`: structured integration team and schedule (HU03).
- `src/builder.rs`: report builder (HU06).
- `src/grading.rs`: rubrics, grading and grade publication (HU12).
- `src/import.rs`: bulk user import (HU13).
- `src/closure.rs`: closure agenda, acta, final artifacts, recognitions and logistics requests (HU08).
- `src/study_units.rs`: study-unit management and CSV load.
- `src/migrate.rs`: `import-django`, the Django database and media import.
- `src/permissions.rs`: roles, module permissions, participation block and the company operating gate (HU13/HU00).
- `src/registration.rs`: self-registration and password recovery.
- `src/mail.rs`, `reminders.rs`: notification email outbox and daily reminders (HU15).
- `src/event.rs`: hackathon schedule advance, logistics and final report (HU09).
- `src/media.rs`, `reports.rs`: private files, PDF and spreadsheet generation.
- `src/models.rs`, `schema.json`, `states.json`, `sql/`: typed domain records and schema inputs; `models.rs` is maintained by hand.
- `web/`: browser UI compiled into the executable.

Libraries: [Axum](https://docs.rs/axum/), [SQLx](https://docs.rs/sqlx/0.8.6/),
[pdf-writer](https://docs.rs/pdf-writer/0.15.0/),
[rust_xlsxwriter](https://docs.rs/rust_xlsxwriter/0.96.0/).