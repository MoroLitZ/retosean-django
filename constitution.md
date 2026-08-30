# Retos EAN Project Constitution

## Objective

This repository hosts the Retos EAN platform. The codebase must stay modular, easy to grow, and safe for multiple contributors working in parallel.

## Architectural Rules

1. Django settings must live under `config/settings/`.
2. Runtime secrets and environment-specific configuration must come from environment variables.
3. Business capabilities must be separated into domain apps under `apps/`.
4. Shared templates and static assets stay at the project level only when they are reused across apps.
5. Each app owns its own models, forms, admin, tests, templates, and future services.

## App Boundaries

- `usuarios`: authentication, registration, profiles, role management, bulk import, and the activity log. Role helpers live in `usuarios/roles.py` and the shared access decorators in `usuarios/decorators.py`; no other app should define its own.
- `empresas`: company records, legal onboarding, and company-specific administration.
- `retos`: challenge creation, publication, challenge lifecycle, and academic integration (teams and sessions).
- `academico`: faculties, programs, student/teacher profiles, challenge discovery, and certificates.
- `participaciones`: applications, operational teams, and student deliverable submission.
- `evaluacion`: rubrics, scoring, and review decisions.
- `seguimiento`: progress tracking, milestones, and audit trail. Owns `IntegracionAcademica`.
- `notificaciones`: the notification engine (`services.notificar`), preferences, in-app inbox, and the reminder command.
- `presupuesto`: per-challenge budget and expense tracking, with the 80% execution alert.
- `reportes`: exports (PDF/Excel), analytics summaries, and reporting endpoints.
- `dashboard`: role-based dashboards, KPIs, charts, and the shared period filters.
- `cierre`: closure, archival, and final outcome workflows.
- `unidades_estudio`: study units per academic program.
- `hackaton`: hackathon-specific logic that should not leak into the generic challenge flow.

### Naming exceptions

- The hackathon app is `apps/hackaton` (no `h`), not `hackathon`. Its app label is
  recorded in `django_migrations`, `django_content_type`, and the permission rows of
  every developer's database, so renaming it would require a data migration for a
  purely cosmetic gain.
- There is no `estudiantes` app: student data lives in `academico` (profiles,
  certificates) and `participaciones` (applications, teams, deliverables).

## Environment Rules

1. Local development uses `config.settings.development`.
2. Production deployments use `config.settings.production`.
3. A `.env.example` file must document every required variable.
4. Secrets must never be committed.
5. PostgreSQL is the default and only supported relational database for this project.

## Branching Strategy

1. `main` is the stable branch.
2. `develop` is the integration branch for ongoing work.
3. Feature work branches from `develop` using `feature/<ticket-or-scope>`.
4. Hotfixes branch from `main` using `hotfix/<scope>`.
5. Every branch should be tied to a ticket, HU, or clearly named scope.

## Quality Rules

1. New behavior should include focused tests.
2. `python manage.py check` must pass before merging, and `python manage.py makemigrations --check --dry-run` must report "No changes".
3. Migrations must be committed with model changes.
4. Cross-app imports should be kept intentional and minimal. Never import from another app's `views.py`; import from its `services.py`, `models.py`, or `decorators.py` instead.
5. Large business workflows should move toward service or use-case modules inside the owning app once complexity grows.
6. Notifications must go through `apps.notificaciones.services.notificar` / `notificar_muchos` / `notificar_admins`, never by creating `Notificacion` objects directly: the service handles preferences, deduplication, and email.
7. Admin checks use `apps.usuarios.roles.es_admin` (superuser OR `rol == "ADMIN"`), never a bare `is_superuser`. Templates use the `es_admin` / `es_empresa` / `es_profesor` / `es_estudiante` flags from the context processor.
