# RetosEAN in Rust

A native Axum + SQLx + PostgreSQL implementation on `Rust-attempt`. The server,
authentication, authorization, domain workflows, uploads, reports and demo seed
run without Python or Django. Browser presentation uses HTML, CSS and JavaScript,
embedded in the executable. There is no Node build or development server.

The interface mirrors the Django templates: the same login page, `retosean`
branding, per-role sidebar sections, labels and accent colours, top bar with the
notification bell, Bootstrap 5.3.3 components, and the Django verbose names and
choice labels for column headers and states. Like Django, it loads Bootstrap,
Bootstrap Icons and the Open Sans/Poppins fonts from their CDNs, so the browser
needs internet access for full styling; scripts (including Chart.js 4.4.1) are
served locally.

This is a working migration, not complete Django feature parity. The original
application remains available for the features listed under remaining work.

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
a fresh demonstration. The Django `DB_NAME` is never implicitly selected.

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
| Data model | 49 typed domain records, PostgreSQL bootstrap schema with database defaults mirroring Django's Python defaults, seven state-machine definitions, one audit table for every state change |
| Users | Bulk import from CSV or Excel with row-by-row preview, unusable passwords plus a set-password email for rows without one (HU13); public self-registration (Comunidad EAN with institutional email, or company with its catalog documents) with welcome email, password recovery by single-use 3-day email link, Django PBKDF2-SHA256 compatible login, expiring sessions, CSRF, login throttling, admin account creation, login by username or email (as Django), contact editing, password changes and revocation, activation/deactivation |
| Companies | Configurable legal-document catalog (HU00): PDF uploads, catalog validity (e.g. 30-day Cámara de Comercio), document decisions, status derived from the documents as in Django, restrictive-list checks, agreement transitions and expiry dates |
| Challenges | Company draft creation/editing, required-field review submission, admin approval/rejection, serialized numbering, audited manual state transitions |
| Academic integration | Professor proposal with structured team (cédula, email, faculty/program) and dated schedule (HU03), the same review requirements, admin review, publication that opens the challenge and notifies assigned students |
| Participation | Search, favorites, applications and withdrawal, acceptance/rejection, academic and operational team creation |
| Deliverables | Private PDF/image/text upload, replacement; grading by free score or rubric criteria (HU12) with re-grade confirmation; grades hidden from students until the professor publishes them, then notified |
| Progress | Dated progress entries, 0–100% validation, company notification |
| Closure | Required deliverable, final artifact linking, state transition, idempotent certificates and satisfaction surveys |
| Certificates | Owner/admin PDF download and public UUID verification |
| Notifications | Django's event catalog (titles, types), in-app bell, preferences and deduplication, email outbox within 20 s using Django's template and EMAIL_* settings (sent/failed recorded), daily reminders (`send-reminders`, same rules as `enviar_recordatorios`) |
| Finance | Exact decimal budgets/expenses, role-scoped summaries, 80% threshold notifications |
| Hackathons | Creation, stages/time windows, jurors (experts included), team registration/review, voting, ranking, automatic advance and result publication at close, event logistics with guests, final report in PDF/Excel (HU09) |
| Reports | Django's report builder (HU06): the same five reports, columns, filters and roles, with preview, PDF/Excel generation logged in the history, and sending by email as an attachment; verified cell by cell against Django's catalog. Also role-scoped CSV/PDF/Excel exports of any module |
| Dashboard | Port of `apps/dashboard`: the same per-role KPIs, charts, quick links and period/faculty/program filters, including the expert juror panel, ecosystem filter and impact indicators; verified to return identical numbers to the Django services on the same database |
| Catalogs | Native creation of faculties, programs, ecosystems and study units; role-scoped browsing |

All 49 domain tables have authenticated, paginated read access with an explicit
role policy. Unmapped tables default to administrator-only access. This read
coverage does not imply every original form or command has been ported.

## Remaining work before replacing Django

- Full user-role administration (role changes, per-module permissions). Rust
  requires 12-character passwords; Django has no password validators configured.
- Editing the operational student teams of an integration (structured team and schedule are native).
- Complete company registration/contact forms and every legal-onboarding edge case.
- Full closure agenda editing, recognition management and final-artifact upload.
- Report PDFs list rows as labelled text rather than Django's landscape table.
  Generic module exports are capped at 5,000 records; Excel stores values as
  literal strings to preserve identifiers, exact decimals and formula safety.
- Full CRUD and file management for study-unit catalogs (creation is native).
- Automated migration/import of an existing Django database and its media.
  Django migration history is not synthesized by `init-db`; do not run Django
  migrations against a Rust-initialized database. A later schema change needs
  an explicit incremental migration, not another `init-db`.
- Production hardening, Unicode font embedding beyond Windows-1252 in PDFs,
  accessibility review and exhaustive parity with the original test suite.

Rust uploads are stored under `RUST_MEDIA_DIR` using random names and are served
only through authorization-checked download endpoints. Existing Django media
paths are deliberately not exposed through the Rust file server. File replacement
leaves old blobs for a future retention/garbage-collection policy. It clears old
evaluation scores rather than treating a changed file as already graded.

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
cargo check --locked
cargo test --locked --lib
cargo test --locked --test end_to_end -- --ignored --nocapture
node --check web/app.js
```

The end-to-end test exercises real Axum handlers and PostgreSQL. It creates a
unique schema in the configured Rust database, initializes the schema and seed,
then removes that schema after the assertions. It does not mutate demonstration
rows or Django tables. The database role must have CREATE privilege on the Rust
database. Tests use a deliberately low-cost password fixture only in their
isolated schema; real account creation uses 1,000,000 PBKDF2 iterations.

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
- `src/dashboard.rs`: port of the Django dashboard indicators (HU05).
- `src/team.rs`: structured integration team and schedule (HU03).
- `src/builder.rs`: report builder (HU06).
- `src/grading.rs`: rubrics, grading and grade publication (HU12).
- `src/import.rs`: bulk user import (HU13).
- `src/registration.rs`: self-registration and password recovery.
- `src/mail.rs`, `reminders.rs`: notification email outbox and daily reminders (HU15).
- `src/event.rs`: hackathon schedule advance, logistics and final report (HU09).
- `src/media.rs`, `reports.rs`: private files, PDF and spreadsheet generation.
- `src/models.rs`, `schema.json`, `states.json`, `sql/`: exported migration inputs.
- `web/`: browser UI compiled into the executable.
- `tools/export_django.py`: development-only reproducible exporter, not a runtime
  dependency. Run it with the existing Django virtual environment if models change.

Libraries: [Axum](https://docs.rs/axum/), [SQLx](https://docs.rs/sqlx/0.8.6/),
[pdf-writer](https://docs.rs/pdf-writer/0.15.0/),
[rust_xlsxwriter](https://docs.rs/rust_xlsxwriter/0.96.0/).
