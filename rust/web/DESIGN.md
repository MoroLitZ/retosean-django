# RetosEAN Front-end Design System — Contract for View Engineers

This document is the **single source of truth** for anyone converting the view
files (`js/{dashboard,records,detail,workflows,auth,evaluation,builder,forms,main}.js`
and their per-view `.css` files) off Bootstrap and onto the new design system.

The app shell (sidebar, topbar, login, toast, dialog, icons, navigation and
router) has already been migrated. Everything below is **stable** and ready to
consume. Do **not** edit `tokens.css`, `base.css`, `components.css`, `shell.css`,
`icons.js`, `ui.js`, or `core.js` — instead call the helpers and classes defined
here and delete the Bootstrap markup as you go.

---

## 1. File map

| File | Role | You edit? |
|---|---|---|
| `css/tokens.css` | Design tokens (colour, type, spacing, radius, shadow, focus, motion) | No |
| `css/base.css` | Reset, typography, utilities, **temporary Bootstrap-compat layer** | No |
| `css/components.css` | Buttons, forms, tables, badges, alerts, toast, dialog, menus, etc. | No |
| `css/shell.css` | Sidebar, topbar, auth layout, skip link | No |
| `css/legacy.css` | Frozen overrides for still-unconverted views | No — leave alone |
| `js/icons.js` | Inline SVG icon set (`window.ICONS`) | No |
| `js/ui.js` | `ui.*` component helpers + `busy()` / `setPageTitle()` | No |
| `js/core.js` | `el`, `icon`, `toast`, `api`, router, nav, bell | No |
| `js/{views}.js` + `css/{view}.css` | **Your files** | Yes |

---

## 2. Design tokens (`tokens.css`)

```css
/* colour */
var(--brand)          #3bac52   /* EAN green */
var(--brand-hover)
var(--brand-subtle)   rgba(59,172,82,.13)
var(--accent)         #005e85   /* EAN blue */
var(--bg) var(--surface) var(--surface-subtle) var(--surface-raised)
var(--ink) var(--ink-soft) var(--ink-muted) var(--ink-faint) var(--ink-inverse)
var(--border) var(--border-strong)
var(--success) var(--success-bg)
var(--warning) var(--warning-bg)
var(--danger)  var(--danger-bg)
var(--info)    var(--info-bg)

/* type */
var(--font-sans)  var(--font-mono)   /* system stack only, no webfonts */
var(--text-xs) … var(--text-3xl)
var(--weight-light) var(--weight-normal) var(--weight-semibold) var(--weight-bold) var(--weight-extrabold)

/* spacing / radius / shadow / motion */
var(--space-1) … var(--space-8)
var(--radius-sm|md|lg|xl|pill)
var(--shadow-sm|md|lg)
var(--duration-fast|base|slow)  var(--ease-out)
```

Dark mode is automatic via `prefers-color-scheme: dark`. Reduced motion is
respected automatically — **never write fixed `transition`s or `animation`s
inside view CSS without `prefers-reduced-motion` guards.**

---

## 3. Layout primitives & utilities (`base.css`)

Use these instead of `row`/`col-*`/`d-flex`/`justify-content-*`.

```css
.stack              /* vertical flex, gap 16px */
.stack--sm .stack--lg
.cluster            /* horizontal wrapping flex, gap 12px */
.cluster--sm .cluster--lg
.cluster--center .cluster--between .cluster--start
.grid               /* responsive auto-fill grid, min 220px */
.grid--2 .grid--3 .grid--4
.visually-hidden
```

Text tone + alignment:

```css
.text-muted .text-soft .text-dark .text-brand .text-accent
.text-success .text-warning .text-danger .text-info
.text-center .text-start .text-end
.text-uppercase .text-truncate .text-wrap
.fw-normal .fw-semibold .fw-bold
```

Spacing steps (numbers follow the classic 0–6 scale; `0 .25 .5 1 1.5 3 4 rem`):

```css
.mt-{0..6} .mb-{0..6} .ms-{0..3} .me-{0..3} .mx-* .my-{1..4}
.p-{0..5} .px-* .py-{1..5} .pt-1 .pt-3 .pb-3
.gap-{0..5}
.ms-auto .me-auto .mx-auto
```

> These short margin/padding/gap names are **our** utilities now, not Bootstrap.
> They are intentionally kept short; keep using them.

---

## 4. Components & their CSS classes (`components.css`)

### Buttons

```html
<button class="btn btn--primary">…</button>
<button class="btn btn--secondary">…</button>
<button class="btn btn--ghost">…</button>
<button class="btn btn--danger">…</button>
<button class="btn btn--accent">…</button>   <!-- outline blue -->
<button class="btn btn--primary btn--sm">…</button>
<button class="btn btn--primary btn--lg">…</button>
<button class="icon-btn icon-btn--sm" aria-label="…"><svg…/></button>
```

Variants exist for `primary`, `secondary`, `ghost`, `danger`, `accent`. Sizes:
`--sm`, `--lg`. Loading state comes from `busy(button, promise)` (see below),
which toggles `.is-loading` and injects a `.spinner`.

### Form fields

```html
<div class="field">
  <label class="field__label" for="f-nombre">Nombre<span class="req"> *</span></label>
  <input class="input" id="f-nombre" name="nombre" />
  <p class="field__hint">Ayuda opcional.</p>
  <p class="field__error" aria-live="polite"></p>
</div>
```

Controls: `.input`, `.select`, `.textarea`. Straightforward equivalents:

- Bootstrap `form-control` → `.input` (or `.textarea`)
- Bootstrap `form-select`  → `.select`
- `form-control-sm` / `form-select-sm` → add class `.input--sm`
- Required marker: `<span class="req">*</span>` inside the label (or `required` on the control)
- Inline error: put the message in `.field__error` and set `aria-invalid="true"`
  + `aria-describedby` to the error's id on the control (see `ui.field`).
- Checkbox: `.checkbox` container + `input.checkbox__input` + `label.checkbox__label`
- Switch: `.switch` with `<label><input type=checkbox><span class=switch__track></span><span class=switch__label>…</span></label>`
- File: `.file input[type=file]` (dashed drop style), use the `::file-selector-button`
- Selects get an SVG chevron automatically.

### Cards / surfaces

```html
<section class="surface">
  <header class="surface__header">Título</header>
  <div class="surface__body">…</div>
</section>
```

`.card-panel` / `.card-panel-header` still work during migration (used by the
dashboard/report builder), and are token-aware.

### KPI tile

```html
<div class="kpi">
  <div class="kpi__icon"><svg class="icon icon-lg">…</svg></div>
  <div>
    <div class="kpi__label">Retos activos</div>
    <div class="kpi__value">42</div>
    <div class="kpi__note">…</div>
  </div>
</div>
```

### Data table

```html
<div class="table-wrap">            <!-- responsive horizontal scroll -->
  <table class="data-table is-zebra is-hoverable">
    <caption class="visually-hidden">…</caption>
    <thead><tr><th scope="col">…</th></tr></thead>
    <tbody>…</tbody>
  </table>
</div>
```

- Sticky header is automatic. Add `.num` to numeric `th`/`td` (right-aligned,
  tabular figures). Sortable `th` gets `aria-sort`; no client sort logic yet.
- Empty state goes inside the table area (see `ui.empty` / `ui.table`).

### Badges / status pills

```html
<span class="pill pill--success dot">Aprobado</span>
```

Variants: `pill--success`, `pill--warning`, `pill--danger`, `pill--info`,
`pill--neutral`, `pill--brand`. The `dot` class prefixes a small status dot.
The bare `.badge`/`.chip` classes are the same base pill (used by legacy views).

### Alerts

```html
<div class="alert alert--success" role="status">…</div>
<div class="alert alert--warning" role="alert">…</div>
<div class="alert alert--danger" role="alert">…</div>
<div class="alert alert--info" role="status">…</div>
```

### Toast / dialog / menu — already migrated in the shell

- Toast: call `toast(message, variant)`; variants `success|error|info|warning`.
  Default is `info`. It is `role="status"` `aria-live="polite"` and dismissible.
- Modal: `modal(title)` (existing global) returns `#dialog-content`. The dialog
  is `aria-labelledby="dialog-title"` and focus-restored by the browser.
- Dropdown/popover menu: `<div class="menu" role="menu">…</div>` with
  `.menu__item`, `.menu__divider`, `.menu__header`. The bell already uses it.

### Empty / skeleton / breadcrumbs / avatar / progress / tabs / pagination

Handled through `ui.*` helpers (below). Breadcrumbs container `#breadcrumbs`
already exists in the shell and is populated by the router; use `setPageTitle`
to add per-view crumbs.

---

## 5. `ui.*` helpers (`js/ui.js`)

All are dependency-free and return DOM nodes you can `append()`.

```js
ui.button("Guardar", { variant: "primary", icon: "check", onClick: save })   // <button>
ui.button("Cancelar", { variant: "secondary" })

ui.badge("success", { label: "Aprobado" })       // <span class="pill pill--success dot">
ui.badge("danger",  { dot: false })

ui.card("Título", bodyNode, actionsNode)          // <section class="surface">

ui.field({
  name: "nombre", label: "Nombre", type: "text", required: true,
  hint: "Ayuda", error: "Campo requerido", onInput: fn,
})
// type: text|email|password|number|date|select|textarea|checkbox|switch|file(accept)

ui.table(
  [{ key: "nombre", label: "Nombre" }, { key: "monto", label: "Monto", align: "right" }],
  rows,                                   // array of objects or arrays
  { empty: "No hay registros", caption: "…" },
)

ui.empty("search", "Sin resultados", "Prueba otro filtro", ui.button("Limpiar", {…}))

ui.skeleton(4, { kind: "row" })            // kind: row|text|title|avatar

ui.pagination({ page: 1, pages: 10, onPage: p => …, info: "1–20 de 200" })

ui.tabs([{ label: "General", content: node }, { label: "Docs", content: node }])

await ui.confirm("¿Eliminar este reto?", { title, confirmText: "Eliminar", variant: "danger" })
// → Promise<boolean>
```

### Global helpers

```js
// Disable a button, show a spinner, re-enable when settled:
busy(submitButton, api("/api/reto", payload));

// Heading + <title> + breadcrumbs:
setPageTitle("Mis Retos", [{ label: "RetosEAN", href: "#dashboard" }, { label: "Mis Retos" }]);
```

---

## 6. Badge / status colour mapping (`badgeClass`)

`core.js` exports `badgeClass(value)` — keep using it; it now returns a semantic
pill variant token (compatible with the `badge`/`pill` base class):

| Value pattern (`estado` fields) | Pill class |
|---|---|
| `aprobado/a`, `abierto`, `en_curso`, `finalizado`, `publicado/a`, `aceptada`, `verificado/a`, `calificado`, `firmado` | `pill--success` |
| `borrador`, `pendiente`, `no_radicado` | `pill--neutral` |
| `en_revision` | `pill--warning` |
| `rechazado/a`, `cancelado/a`, `retirada`, `vencido` | `pill--danger` |
| anything else | `pill--info` |

Render it as: `<span class="badge pill--success dot">${choice(table,key,value)}</span>`
(or just `ui.badge(state, { label })`).

---

## 7. Layout patterns

### List page (records)

```
content-wrapper
└─ cluster.stack (title + record-count)   …  cluster (import / new buttons)
└─ surface
   ├─ surface__header (search form, exports)
   └─ table-wrap > table.data-table
└─ pagination (or cluster--center for prev/next)
```

Use `aria-busy` + `ui.skeleton()` while `api()` resolves (the router already
toggles `aria-busy` on `#main-content`).

### Detail modal

```
modal(title)                    // existing helper
├─ field(s) via ui.field
├─ <dl class="details">…</dl>   // existing key/value grid (legacy) — migrate to ui.table or keep
└─ <div class="dialog-actions"> … </div>
```

Use `busy(button, api(...))` for submit buttons, and inline `.field__error` for
validation. Always `mobile`: forms collapse via `.form-grid` (2-col → 1-col).

### Forms

```
form.form-grid            // 2 columns, gap 16px
├─ .field                 // each control
├─ .field.field--full     // spans both columns (textarea, submit)
```

---

## 8. Accessibility rules

- Decorative icons: `icon()` already emits `aria-hidden="true"`. For a labelled
  icon use `icon("name").setAttribute("aria-label", …)` + `role="img"`.
- Tables: `th scope="col"`; body first cell `th scope="row"` where sensible.
- Dialogs: keep `aria-labelledby="dialog-title"`.
- Toast/inline errors: `aria-live="polite"`.
- Focus: after route change focus is moved to `#page-heading` by the router;
  keep `tabindex="-1"` there.
- No colour-only meaning: every status pill has a dot + text, not just colour.

---

## 9. Bootstrap → new migration table

Covers **every** Bootstrap class family currently emitted by the view files.

### Grid & layout

| Bootstrap | New |
|---|---|
| `row` | `.grid`, `.grid--2/--3/--4`, or `.form-grid` (forms), or `.cluster` |
| `g-0…g-3`, `g-*` | gap on the container: `.cluster .cluster--sm/--lg` / `.grid` gap |
| `col`, `col-*`, `col-sm-*`, `col-md-*`, `col-lg-*` | grid columns are automatic; for explicit columns use CSS grid or keep `.grid` minmax. (Temporary `row`/`col-*`/`g-*` classes still render until converted — see `base.css` compat block.) |

### Flex & display

| Bootstrap | New |
|---|---|
| `d-flex` | `.cluster` (wraps, gap 12) or bare `display:flex` in view CSS |
| `d-block`, `d-inline`, `d-none` | hide/show via `hidden` attribute; `.d-none` still works (compat) |
| `d-md-flex`, `d-lg-inline` etc. | media queries in view CSS |
| `flex-column`, `flex-wrap`, `flex-shrink-0`, `flex-grow-0/1` | view CSS, or `.stack` for columns |
| `justify-content-*` | `.cluster--center`, `.cluster--between`, `.cluster--start` |
| `align-items-*` | `.cluster--start`, or view CSS |
| `gap-*` | `.gap-{0..5}` / `.cluster--sm/--lg` / `.stack--sm/--lg` |

### Spacing

| Bootstrap | New |
|---|---|
| `mt-*` `mb-*` `ms-*` `me-*` `mx-*` `my-*` | same names (our utilities) — keep |
| `p-*` `px-*` `py-*` `pt-*` `pb-*` | same names — keep |
| `w-100`, `h-100`, `min-w-0` | `.w-100` `.h-100` `.min-w-0` (kept) |

### Typography / text

| Bootstrap | New |
|---|---|
| `text-center/start/end` | `.text-center/.text-start/.text-end` |
| `text-muted` | `.text-muted` |
| `text-secondary` | `.text-soft` |
| `text-dark` | `.text-dark` |
| `text-success/warning/danger/info` | `.text-success/…` |
| `text-uppercase` | `.text-uppercase` |
| `text-truncate` | `.text-truncate` |
| `text-wrap` | `.text-wrap` |
| `text-decoration-none` | drop (links are unstyled by default) |
| `fw-bold` / `fw-semibold` / `fw-normal` | `.fw-bold` / `.fw-semibold` / `.fw-normal` |
| `small` | `.small` |

### Buttons

| Bootstrap | New |
|---|---|
| `btn` | `.btn` |
| `btn-primary` | `.btn .btn--primary` |
| `btn-outline-secondary` | `.btn .btn--secondary` |
| `btn-outline-danger` | `.btn .btn--danger` |
| `btn-outline-primary` | `.btn .btn--accent` |
| `btn-outline-success` | `.btn .btn--accent` (or `--primary`) |
| `btn-link` | `.btn .btn--ghost` |
| `btn-sm` / `btn-lg` | `.btn--sm` / `.btn--lg` |
| `btn-close` | `.icon-btn` + `x` icon |
| `w-100` on button | `.w-100` |

### Forms

| Bootstrap | New |
|---|---|
| `form-label` | `.field__label` |
| `form-control` | `.input` (or `.textarea`) |
| `form-control-sm` | `.input .input--sm` |
| `form-select` | `.select` |
| `form-select-sm` | `.select .input--sm` |
| `form-check` | `.checkbox` |
| `form-check-input` | `input.checkbox__input` |
| `form-check-label` | `.checkbox__label` |
| `input-group` | `.input-group` (kept) |
| `input-group-text` | `.input-group-text` (kept) |

### Tables

| Bootstrap | New |
|---|---|
| `table` | `.data-table` |
| `table-hover` | `.data-table.is-hoverable` |
| `table-striped` | `.data-table.is-zebra` |
| `table-light` | (unused — header sticky is automatic) |
| `table-sm` | view CSS or tighter padding |
| `table-responsive` | `.table-wrap` |
| `align-middle` | `.align-middle` (kept) |

### Badges / backgrounds / alerts

| Bootstrap | New |
|---|---|
| `badge` + `bg-success/secondary/warning/danger/info` | `ui.badge(state, {label})` → `.pill--*` |
| `bg-light` | `.bg-light` (kept) or `.surface-subtle` context |
| `bg-*` text pills | `.pill--*` |
| `alert alert-danger/warning` | `.alert .alert--danger/.alert--warning` |
| `rounded`, `rounded-3`, `rounded-pill`, `rounded-circle` | `.rounded`, `.rounded-3`, `.rounded-pill`, `.rounded-circle` (kept) |

### Nav / dropdown / cards

| Bootstrap | New |
|---|---|
| `nav`, `nav-item`, `nav-link` | `.nav__item`, `.nav__link` (shell-owned; not for reuse) |
| `dropdown-menu`, `dropdown-item`, `dropdown-header`, `dropdown-divider` | `.menu`, `.menu__item`, `.menu__header`, `.menu__divider` |
| `dropdown-menu-end` | `.menu` positioned by parent (see bell) |
| `card`, `card-body`, `card-header`, `card-footer` | `.surface`, `.surface__body`, `.surface__header`, `.dialog-actions` |
| `shadow`, `shadow-sm`, `shadow-lg` | `.shadow`, `.shadow-sm`, `.shadow-lg` (kept) |
| `border-0` | `.border-0` (kept) |

### Position / icons

| Bootstrap | New |
|---|---|
| `position-relative/absolute`, `top-0`, `start-100`, `translate-middle` | view CSS or kept utilities (needed by bell badge) |
| `i.bi bi-*` | `icon("name")` → inline `<svg class="icon">` |

---

## 10. Icon names

`icon(name)` accepts any name in `js/icons.js`. Key names: `speedometer2`,
`people`, `building`, `trophy`, `flag`, `bar-chart-line`, `cash-coin`,
`plus-circle`, `person-plus`, `person-check`, `file-earmark-text`,
`chat-square-text`, `journal-text`, `diagram-3`, `folder-check`,
`clipboard2-check`, `search`, `send`, `star`, `star-half`, `folder2-open`,
`award`, `lightning-charge`, `table`, `person-circle`, `box-arrow-left/right/in-right`,
`bell`, `bell-slash`, `play-circle`, `hourglass-split`, `hourglass`,
`mortarboard`, `person-video3`, `check2-circle`, `check2-square`, `x-circle`,
`clock-history`, `cloud-arrow-up`, `plus-lg`, `eye`, `eye-slash`, `download`,
`upload`, `envelope`, `lock`, `person`, `list`, `funnel`, `calendar3`,
`chevron-left/right`, `x`, `check`, `exclamation-triangle`, and the `-fill`
variants (`check-circle-fill`, `x-circle-fill`, `info-circle-fill`, …).

Unknown names fall back to a neutral dot; nothing ever throws.

---

## 11. Contract summary for view engineers

1. **Do not** add Bootstrap classes; import nothing new. Use the classes/helpers above.
2. Keep **every** global (`el`, `icon`, `toast`, `api`, `readJson`, `showLogin`,
   `start`, `buildNav`, `route`, `refreshBell`, `fecha`, `title`, `choice`,
   `badgeClass`, `human`, `capfirst`, `meta`, `field`, `admin`, `role`,
   `allRecords`, `cop`, and the view helpers `modal`, `showError`, `input`,
   `choiceSelect`, `simpleForm`, `table`, `cell`, `action`, `editableRows`, etc.)
   working with the same signatures.
3. Loading states: use `ui.skeleton()` inside your `load*()` functions; the router
   already toggles `aria-busy` on `#main-content` and a top progress bar.
4. Forms: use `ui.field()` for inline errors (`aria-invalid`, `aria-describedby`),
   and `busy(button, api(...))` for pending states.
5. File inputs: show selected name/size and a `.progress__bar` during upload;
   validate type/size before submit.
6. Tables: `scope="col"`, `.num` for numeric columns, `ui.empty` for empty rows,
   real `colSpan` (not a hard-coded `7`).
7. Microcopy: Spanish (Colombia) — prefer "Seleccione", "Regístrate", consistent
   register ("tú" is fine for the login; keep it consistent per view).
8. After conversion, delete your per-view `.css` shims and the temporary
   `Bootstrap-compatibility` sections in `base.css`/`components.css`.