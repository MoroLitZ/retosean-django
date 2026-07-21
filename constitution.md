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

- `usuarios`: authentication, registration, profiles, and role management.
- `empresas`: company records, legal onboarding, and company-specific administration.
- `retos`: challenge creation, publication, and challenge lifecycle.
- `estudiantes`: student profile extensions and student participation workflows.
- `evaluacion`: rubrics, scoring, and review decisions.
- `seguimiento`: progress tracking, milestones, and audit trail.
- `notificaciones`: notifications, reminders, and delivery events.
- `reportes`: exports, analytics summaries, and reporting endpoints.
- `dashboard`: role-based dashboards and landing surfaces.
- `cierre`: closure, archival, and final outcome workflows.
- `hackathon`: hackathon-specific logic that should not leak into the generic challenge flow.

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
2. `python manage.py check` must pass before merging.
3. Migrations must be committed with model changes.
4. Cross-app imports should be kept intentional and minimal.
5. Large business workflows should move toward service or use-case modules inside the owning app once complexity grows.
