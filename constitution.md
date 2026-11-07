# Retos EAN Project Constitution

## Objective

This repository hosts the Retos EAN platform as a native Rust application. The codebase must stay modular, easy to grow, and safe for multiple contributors working in parallel. The only supported stack is the Axum 0.8 + SQLx 0.8 + PostgreSQL service under `rust/` and its embedded vanilla-JS single-page app.

## Architectural Rules

1. The application is Rust edition 2024 under `rust/`. No Python/Django code is authoritative.
2. Runtime configuration and secrets must come from environment variables or a `.env` file (`rust/.env` or the repository root `.env`), loaded through `src/config.rs`. Secrets must never be committed, and `.env.example` must document every required variable.
3. PostgreSQL is the default and only supported relational database.
4. One Rust module per business domain lives in `rust/src`; domain responsibilities are listed below.
5. Schema changes must be added only as new numbered files in `rust/sql/migrations` and listed in `src/schema_migrations.rs`. Applied migrations must never be edited.
6. The browser UI is a vanilla-JS SPA split per area in `rust/web/js` and `rust/web/css`, following `rust/web/DESIGN.md`. Assets are embedded in the binary; no CDN, Bootstrap, or other external frontend resources are allowed.
7. CSP must be `self` only. No inline styles, inline scripts, or external resources are allowed.

## Module Boundaries

- `accounts`: user profiles, contact editing, passwords, activation/deactivation, role changes, and account audit.
- `registration`: self-registration and password recovery.
- `import`: bulk user import from CSV/Excel with preview and set-password emails.
- `companies`: company records, legal onboarding, document catalog, document decisions, restrictive-list checks, and the publishing gate.
- `media`: private uploads, downloads, file replacement, and authorization-checked serving.
- `lifecycle`: challenge creation, publication, lifecycle state transitions, and audit.
- `academic`: faculties, programs, student/teacher profiles, academic integration proposals, and certificates.
- `team`: structured academic/operational teams and schedules, kept synchronized.
- `study_units`: study units per academic program and CSV load.
- `workflows`: transactional participation, deliverable submission, and cross-domain business workflows.
- `grading`: rubrics, scoring, review decisions, and grade publication.
- `closure`: closure, archival, final artifacts, recognitions, surveys, and logistics requests.
- `dashboard`: role-based dashboards, KPIs, charts, and shared filters.
- `reports`: module exports, PDF/Excel/CSV generation, analytics, and export history.
- `builder`: report builder definitions, filters, and preview/generation flow.
- `finance`: budgets, expenses, and the 80% execution alert.
- `hackathon`: hackathon-specific logic that must not leak into the generic challenge flow.
- `event`: hackathon stage advancement, logistics, and final report.
- `mail`: notification email outbox and delivery status tracking.
- `reminders`: daily reminder scheduling and rules.
- `permissions`: roles, module permissions, participation block, and the company operating gate.
- `auth`: sessions, login throttling, CSRF, and password-compatibility handling.

No other module may define its own role helpers, access policies, or notification delivery. There is no `estudiantes` module: student data lives in `academic` and in `workflows`/`team`.

## Testing/Quality Rules

1. Every behaviour change must include a test in `rust/tests`, either `parity_<area>.rs` or `end_to_end.rs`.
2. CI in `.github/workflows/rust.yml` must stay green: `cargo fmt --check`, `cargo clippy`, `cargo test --locked -- --include-ignored`, and `node --check` over `rust/web/js/*.js`.
3. Database tests must create isolated schemas in the configured Rust database and drop them afterward; they must never touch demonstration or production rows.
4. Migrations are committed with schema changes and are never edited after being applied.
5. Cross-module imports must be intentional and minimal. Workflows should move into the owning module's service/use-case code as complexity grows.
6. Notifications and reminders must go through the centralized `mail`/`reminders` modules, never by writing notification records directly.
7. Access decisions must use `permissions`, never ad-hoc role checks.

## Collaboration Rules

1. `main` is the stable branch.
2. `develop` is the integration branch for ongoing work.
3. Feature work branches from `develop` using `feature/<ticket-or-scope>`.
4. Hotfixes branch from `main` using `hotfix/<scope>`.
5. Every branch should be tied to a ticket, HU, or clearly named scope.
6. Never apply Django migrations to a Rust database. Existing Django data may only enter through the supported import path.