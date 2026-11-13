# RetosEAN Design System — v2

Permanent reference for anyone building or changing screens in the RetosEAN web
app (`rust/web`). v2 keeps every class name, DOM structure and JS helper of v1
and raises the visual and accessibility bar: real contrast, a clear type scale,
calmer surfaces, brand-consistent charts and first-class mobile and dark mode.

**Golden rule of v2: behaviour stays identical.** The redesign is CSS plus one
bounded JS change (chart palette/options in `dashboard.js`). No HTML or JS
class is renamed or removed; anything JS toggles (`hidden`, `.active`,
`aria-busy`, `.is-loading`, `.menu__item--unread`, `open`) keeps working.

---

## 0. Principles

| # | Principle | What it means in practice |
|---|---|---|
| 1 | **Legible first** | Body 15px, table cells 13px minimum, every text ≥ 4.5:1 contrast, numbers in tabular figures. |
| 2 | **One idea per surface** | A card has one title, one job. Section eyebrows separate groups; no nested cards. |
| 3 | **Calm colour, loud status** | Neutrals carry the layout; green is identity and primary action; semantic colours appear only on pills, alerts, icons and chart series. |
| 4 | **Same component everywhere** | A pill, a button, a field looks and behaves the same in a table, a modal, a workflow page and the login. |
| 5 | **Mobile and dark are not variants, they are the product** | Every component is specified at 360px and in dark mode. |
| 6 | **Motion is feedback, never decoration** | 120–280 ms, ease-out, only on hover/focus/enter; fully removed under reduced-motion. |

Audiences: **estudiantes, profesores, empresas** get the simplified navigation
and welcome dashboard (see `simpleRole()` in `core.js`). **Admin** keeps its
full navigation and filter card; it benefits from the same components.

### What changes from v1 (summary)

| Area | v1 | v2 |
|---|---|---|
| Primary button | `#3bac52` fill + white text (2.9:1, fails AA) | `--brand-strong` (#27783a, 5.5:1) fill + `--on-brand`; dark mode uses bright green + dark text |
| Type scale | 0.72–1.75rem, body 0.92rem | 11–40px scale, body 15px, tabular numerals, defined roles |
| KPI tile | icon + label-over-value row, small value | value-led tile: eyebrow label, 26px value, note, icon top-right; 2-col mobile with no wrapped 3-line labels |
| Detail modal | bordered spreadsheet-like `dl` | airy 2-column definition grid with hairlines, sticky action bar, mobile bottom sheet |
| Charts | API colours (default blue/green/orange) | `--chart-1…8` palette, hairline grid, rounded bars, truncated labels with full text in tooltip |
| Login | dark page even in light mode, low-contrast copy | theme-aware page, brand panel with ≥ 4.5:1 copy, tighter card |
| Topbar | small email + red "Salir" | user chip (avatar · name · role), ghost "Salir" that turns danger on hover |
| Tables | 18rem truncation, uniform weight | 24rem truncation, title column emphasised, 48px rows, subtle hover, header eyebrow style |
| Dark mode | partial | every token re-derived, surfaces layered, shadows deeper |

---

## 1. File map and load order

| File | Role | Owner of |
|---|---|---|
| `css/tokens.css` | Design tokens (this is the contract) | colours, type, space, radius, shadow, motion, layout, z-index |
| `css/base.css` | Reset, typography roles, layout primitives, utilities | `.stack .cluster .grid`, text/space utilities, `[hidden]` |
| `css/components.css` | Shared components | buttons, forms, surfaces, KPI, table, pills, alerts, toast, dialog, menu, tabs, pagination, empty, skeleton, breadcrumbs, avatar, progress, chips |
| `css/shell.css` | App shell + auth layout | sidebar, topbar, main, skip link, login/registro layout, responsive shell |
| `css/auth.css` | Registro/recuperar extras | `.auth-choice*`, `.auth-result*`, `.auth-toggle` |
| `css/dashboard.css` | Dashboard | welcome card, section heads, KPI grid, chart grid, skeletons |
| `css/records.css` | List pages | truncation, search width, page help, extra panel |
| `css/detail.css` | Detail modal body | `.details`, `.detail-actions` |
| `css/forms.css` | Form modals | `.form-grid`, `.forms-*` |
| `css/workflows.css` | Workflow pages | `.wf-*` |
| `css/builder.css` | Report builder | `.builder-*` |

Load order is fixed in `index.html` (tokens → base → components → shell → views).
`rust/web` is embedded with `include_str!`; **rebuild the binary after any
change** (`cargo build --release`, server stopped first).

---

## 2. Tokens (`tokens.css`)

Only tokens may carry raw colour or size values. Full list lives in the file;
these are the ones every author must know.

### 2.1 Colour roles

| Token | Use | Never use for |
|---|---|---|
| `--brand` | logo, focus ring, decorative accents, icon tints, sidebar active bar, dark-mode button fill | white text on it in light mode |
| `--brand-strong` / `--on-brand` | primary button fill and its text, selected states with text | large background areas |
| `--brand-text` | links, green text, active nav text on light surfaces | |
| `--brand-subtle` / `--brand-subtle-strong` | tinted backgrounds (icon circles, selected rows, welcome card wash) | |
| `--accent` / `--accent-text` / `--accent-subtle` | EAN blue: secondary emphasis (outline buttons, info icons, chart-2) | primary actions |
| `--bg` → `--surface-sunken` → `--surface-subtle` → `--surface` → `--surface-raised` | page → inset wells → headers/zebra → cards → menus/dialogs | |
| `--ink` / `--ink-soft` / `--ink-muted` / `--ink-faint` | titles+values / body / labels+help / placeholders+disabled | |
| `--border` / `--border-strong` / `--hairline` | card edges / control edges / dividers inside cards | |
| `--success|warning|danger|info|neutral` (+`-bg`, `-border`) | pills, alerts, icon tints, inline validation | body text |
| `--chart-1…8`, `--chart-grid`, `--chart-tick` | every Chart.js series, grid and tick | |

Contrast guarantees (light): `--ink` 16.9:1, `--ink-soft` 10.2:1, `--ink-muted`
5.3:1 on `--surface`; `--brand-text` 5.5:1; `--on-brand` on `--brand-strong`
5.5:1; each semantic fg ≥ 4.5:1 on its `-bg`. Dark: equivalents ≥ 4.5:1 on
`--surface`.

### 2.2 Type scale

| Token | px | Role |
|---|---|---|
| `--text-2xs` | 11 | counters in badges |
| `--text-xs` | 12 | eyebrows (uppercase, `--tracking-caps`), captions, table header |
| `--text-sm` | 13 | table cells, hints, pills, breadcrumbs, secondary nav |
| `--text-base` | 15 | body, inputs, buttons, nav links |
| `--text-md` | 16 | lead paragraphs, inputs on mobile (prevents iOS zoom) |
| `--text-lg` | 18 | h3 / card titles |
| `--text-xl` | 22 | h2 / page title |
| `--text-2xl` | 26 | h1 / KPI value / welcome title |
| `--text-3xl` | 32 | display (login heading) |
| `--text-4xl` | 40 | hero number (reserved) |

Weights: labels/eyebrows 600, titles 700, display 800, body 400, table title
column 500. Numbers (`.num`, `.kpi__value`, dates) use
`font-variant-numeric: tabular-nums`.

### 2.3 Space, radius, elevation, motion

* Spacing is a 4px grid: `--space-1…8` = 4 8 12 16 24 32 48 64 (plus `-0-5` 2px, `-4-5` 20px).
* Radii: controls `--radius-md` 10px, cards `--radius-lg` 14px, dialogs/welcome `--radius-xl` 20px, pills `--radius-pill`.
* Elevation: cards `--shadow-xs` + 1px `--border`; hover `--shadow-md`; menus `--shadow-md`; dialogs `--shadow-lg`; primary button hover `--shadow-brand`.
* Controls: height `--control-h` 40px (sm 32, lg 48), horizontal padding `--control-px`.
* Motion: `--duration-fast|base|slow` with `--ease-out`; `--ease-spring` only for the toast enter.
* Layout: `--sidebar-width` 264, `--topbar-h` 60, `--content-max` 1360, `--content-pad` 32 / 16 mobile, `--dialog-w` 800.
* Z-index: `--z-sticky < --z-topbar < --z-sidebar < --z-menu < --z-dialog < --z-toast`.

---

## 3. Typography and base (`base.css`)

* `body`: `--font-sans`, `--text-base`, `--leading-normal`, `--ink` on `--bg`, antialiased.
* `h1` 2xl/800 tracking-tight · `h2` xl/700 · `h3` lg/700 · `h4` md/600 · `h5`,`h6` sm/600; all `--leading-tight`.
* `.eyebrow` pattern (used by `.dash-section-title`, table `th`, `.nav__section`, `.kpi__label`): `--text-xs`, 600, uppercase, `--tracking-caps`, `--ink-muted`.
* Links: `--brand-text`, underline on hover only, 2px underline offset. `.text-link` identical.
* Focus: `:focus-visible { outline: 0; box-shadow: var(--focus-ring); }` with offset 2px; nothing on `:focus` alone; programmatic targets (`tabindex="-1"`) show no ring.
* Selection: `--brand-subtle-strong` background.
* Scrollbars: thin, `--border-strong` thumb, transparent track.
* Layout primitives and utilities keep their v1 names and values (`.stack`, `.cluster`, `.grid`, `.mt-*`, `.p-*`, `.gap-*`, `.text-*`, `.fw-*`, `.d-block`, `.flex-grow-1`, `.align-items-start`, `.position-relative`, `.w-100`, `.min-w-0`, `.small`, `.visually-hidden`). Add `.tabular` (tabular numerals) and `.text-balance`.
* `[hidden] { display: none !important; }` stays global.

---

## 4. Layout

### 4.1 App shell

```
┌ sidebar 264px (fixed, full height, --sidebar-bg) ┬ topbar 60px (fixed, --surface, 1px bottom border, 3px top accent) ┐
│ brand (72px)                                      │ [☰ mobile] page heading · ─flex─ · bell · user chip · Salir       │
│ nav sections + links (scroll)                     ├ main: padding-top 60px; .content-wrapper max 1360, padding 32px  ┤
│ user block (sticky bottom)                        │ breadcrumbs (sm, muted) → page header → surfaces                  │
└───────────────────────────────────────────────────┴───────────────────────────────────────────────────────────────────┘
```

* Breakpoints: `sm` ≤ 600 (phones), `md` ≤ 768 (sidebar becomes off-canvas drawer with `--overlay`), `lg` ≥ 992 (4 KPI columns, 12-col chart grid), `xl` ≥ 1280.
* Below 768: sidebar is off-canvas (`transform: translateX(-100%)`, `.is-open`/existing toggle classes untouched), topbar spans full width, content padding 16px, page title 20px.
* Content rhythm: page header → 24px → first surface; between surfaces 16px; between sections 32px.

### 4.2 Page header (list pages)

`.page-title` xl/800 · `.page-subtitle` sm muted (record count) · `.page-help` base `--ink-soft`, max 70ch · actions cluster right-aligned, wraps under the title on mobile.

---

## 5. Components (`components.css`)

Each entry gives the DOM contract (already emitted by JS/HTML — do not change) and the v2 visual spec.

### 5.1 Buttons `.btn`

Contract: `.btn.btn--primary|--secondary|--ghost|--danger|--accent`, sizes `.btn--sm|--lg`, `.is-loading` + injected `.spinner`, `[disabled]`, `.w-100`. Legacy aliases (`.btn-primary`, `.btn-outline-*`, `.btn-ean`, `.btn-auth`, `.btn-link`) keep mapping to the nearest variant.

| Variant | Rest | Hover | Active | Focus |
|---|---|---|---|---|
| primary | `--brand-strong` / `--on-brand`, 600 | `--brand-strong-hover` + `--shadow-brand` | translateY(1px), no shadow | ring |
| secondary | `--surface`, 1px `--border-strong`, `--ink` | `--surface-subtle`, border `--ink-faint` | | ring |
| ghost | transparent, `--ink-soft` | `--surface-subtle`, `--ink` | | ring |
| danger | transparent, 1px `--danger-border`, `--danger` | `--danger-bg` | | ring |
| accent | transparent, 1px `--accent`, `--accent-text` | `--accent-subtle` | | ring |

Geometry: height `--control-h`, padding 0 `--control-px`, radius `--control-radius`, gap 8px, icon 1.05rem; `--sm` 32px/12px/`--text-sm`; `--lg` 48px/20px/`--text-md`. Disabled: opacity .55, no pointer. Loading: label stays, spinner 1em before it, `cursor: progress`. Min touch target 40px (sm buttons get a transparent 4px vertical hit area via `::after` is **not** needed; keep height ≥ 32 and add margin via containers).

### 5.2 Icon button `.icon-btn` (`--sm`)
40px square (32 sm), ghost styling, radius md, `--ink-muted` → `--ink` on hover.

### 5.3 Fields and controls
Contract: `.field > .field__label (+ .req) + .input|.select|.textarea|.file + .field__hint + .field__error`, `.input--sm`, `.input-group > .input-group-text + .input`, `.checkbox`, `.switch`, `[aria-invalid="true"]`, `.form-grid`, `.field--full`.

* Label: `--text-sm`, 600, `--ink-soft`, margin-bottom 6px. `.req` in `--danger`.
* Control: height `--control-h`, padding 0 12px, `--surface`, 1px `--border-strong`, radius `--control-radius`, `--text-base` (`--text-md` ≤ 600px). Placeholder `--ink-faint`. Hover border `--ink-faint`. Focus: border `--brand`, ring. Invalid: border `--danger`, ring in danger tint. Disabled: `--surface-sunken`, `--ink-muted`.
* Textarea min-height 112px, resize vertical. Select keeps the inline SVG chevron (currentColor, works in dark).
* `.field__hint` sm muted; `.field__error` sm `--danger`, hidden when empty (`:empty { display:none }`).
* Input group: one bordered pill; the `.input-group-text` is `--surface-subtle` with `--ink-muted` icon; the group, not the inner input, shows the focus ring (`:focus-within`).
* Checkbox 18px, radius xs, checked `--brand-strong` with white check; switch 36×20 track, `--brand-strong` when on.
* File dropzone: dashed `--border-strong`, `--surface-subtle`, hover tint `--brand-subtle`.
* `.form-grid`: 2 columns ≥ 640px, gap 16px 20px; `.field--full` spans both.
* Date inputs: `color-scheme` inherits so the picker icon is visible in dark mode.

### 5.4 Surfaces
`.surface`, `.card-panel` (+ `.card-panel-header`, `.surface__header`, `.surface__body`): `--surface`, 1px `--border`, radius `--radius-lg`, `--shadow-xs`, overflow hidden. Header: padding 16px 20px, 1px `--hairline` bottom, title lg/700, actions right. Body padding 20px (16 mobile). No nested surfaces.

### 5.5 KPI tile `.kpi`
Contract: `article.surface.kpi.dash-kpi > .kpi__icon.kpi__icon--<tone> + .dash-kpi-text > .kpi__label + .kpi__value (+ .kpi__note)`.

v2 layout (CSS only, using `display:grid`): icon 40px circle top-right (`grid-area` via `order`/grid), label eyebrow top-left, value 26px/700 tabular `--ink`, note sm muted. Tile padding 20px, min-height 112px, radius lg. Hover lifts (`--shadow-md`, −2px). Tones: `--brand|--success|--warning|--danger|--info|--accent|--violet|--pink` → fg colour + 12–14% tint background. Mobile (≤ 600): padding 14px, value 22px, label 11px with `overflow-wrap:anywhere` so it never exceeds 2 lines, icon 32px.

### 5.6 Data table `.data-table`
Contract: `.table-wrap > table.data-table(.is-zebra)(.is-hoverable) > thead th[scope=col] / tbody td`, `.num`, `th[aria-sort]`, `.records-skeleton-row`.

* Header: eyebrow style, `--surface-subtle` background, sticky, 1px `--border` bottom, padding 12px 16px.
* Cells: `--text-sm`, padding 14px 16px (row ≈ 48px), 1px `--hairline` between rows, no vertical borders, vertical-align middle.
* First data column (`td:first-child`) `--ink` weight 500; others `--ink-soft`. `.num` right-aligned tabular.
* Hover row `--surface-subtle`; zebra uses `--surface-subtle` at 50% (`color-mix` allowed, fallback to plain).
* Last column (actions) right-aligned, `white-space: nowrap`.
* Mobile: `.table-wrap` scrolls horizontally with 16px negative inset so the card edge is kept; min table width 640px.

### 5.7 Pills `.pill` / `.badge` / `.chip`
Variants `pill--success|warning|danger|info|neutral|brand`, `.dot` prefix. Height 24px, padding 0 10px, `--text-xs` 600, radius pill, fg `--<semantic>`, bg `--<semantic>-bg`, 1px `--<semantic>-border`. Dot 6px, currentColor. `.pill--brand` uses `--brand-text` on `--brand-subtle`. Counter pills in nav/bell (`.nav__badge`, `#bell-count`) are `--danger` fill, white 11px/700, min 18px.

### 5.8 Alerts `.alert.alert--*`
Padding 12px 16px, radius md, bg `--<semantic>-bg`, 1px `--<semantic>-border`, text `--ink` with the leading icon in `--<semantic>`; sm text; gap 10px; `role` unchanged.

### 5.9 Toast `#toast.toast--*`
Bottom-centre (bottom-right ≥ 768), `--surface-raised`, `--ink`, 1px `--border`, `--shadow-lg`, radius lg, 4px left bar in the semantic colour, max-width 420px, enter 180ms translateY(8px→0) with `--ease-spring`; `.toast__close` icon button.

### 5.10 Dialog `dialog#detail-dialog`
Contract: `dialog > .dialog-top (h2#dialog-title + .icon-btn) + .dialog-body#dialog-content`; inner `.dialog-actions`, `.confirm-message`.

* Desktop: width `min(var(--dialog-w), 100vw − 32px)`, max-height `calc(100dvh − 48px)`, radius xl, `--shadow-lg`, 1px `--border`, backdrop `--overlay` + `backdrop-filter: blur(2px)`.
* `.dialog-top`: padding 18px 24px, 1px `--hairline` bottom, title xl/700 truncating to 2 lines; close button 40px.
* `.dialog-body`: padding 24px, scrolls (`overflow:auto`, `overscroll-behavior: contain`).
* `.dialog-actions`: cluster, gap 8px, margin-top 24px, primary first.
* Mobile (≤ 600): bottom sheet — `width:100vw; max-height: 94dvh; margin: auto 0 0; border-radius: xl xl 0 0`; body padding 16px.
* Enter animation: fade + translateY(8px) 180ms; none under reduced motion.

### 5.11 Menu / bell `.menu`
`--surface-raised`, 1px `--border`, radius lg, `--shadow-md`, padding 6px, min-width 320px (bell), items 10px 12px radius md hover `--surface-subtle`; `.menu__header` eyebrow; `.menu__item--unread` gets `--brand-subtle` bg and a 6px `--brand` `.bell-dot`.

### 5.12 Tabs, pagination, empty, skeleton, breadcrumbs, avatar, progress, chips
* Tabs: underline style, 2px `--brand` indicator on `[aria-selected="true"]`, labels sm/600 muted → ink.
* Pagination: ghost 32px buttons, current page `--brand-subtle` + `--brand-text`; `.pagination__info` sm muted.
* Empty: centred, icon 48px circle `--brand-subtle`/`--brand`, title md/600, hint sm muted, optional action; padding 40px 16px.
* Skeleton: `--surface-sunken` base with shimmer (`--surface-subtle`), radius sm, 1.4s linear; static under reduced motion.
* Breadcrumbs: sm, muted, "/" separators in `--ink-faint`, last item `--ink-soft`.
* Avatar: 36px (sm 28, lg 48) circle, `--brand-strong` bg `--on-brand` initials 600.
* Progress: 6px track `--surface-sunken`, bar `--brand`; `.app-progress` 3px top bar with indeterminate sweep.
* Chips: as pills but `--surface-subtle` with 1px `--border` and `.chip__remove` icon button.

---

## 6. Shell (`shell.css`)

### 6.1 Sidebar `#sidebar`
Contract: `.sidebar-brand > .brand-logo (.brand-dots + .brand-wordmark .brand-retos/.brand-ean) + .brand-sub`, `ul#navigation > li.nav__section | li.nav__item > a|button.nav__link (> svg.icon + .nav__label + .nav__badge) | li.nav__divider`, `.nav__link.active`, `.nav__link.logout`, `.sidebar-user > .cluster > .avatar.user-avatar + (.user-name + .user-role)`, `.sidebar-overlay`.

* Width `--sidebar-width`, `--sidebar-bg`, 1px `--sidebar-border` right edge.
* Brand block 72px tall, wordmark 22px/800: "retos" white, "ean" `--brand`; `.brand-sub` 10px eyebrow `--sidebar-ink-muted`.
* `.nav__section`: eyebrow 11px/700 `--sidebar-ink-muted`, padding 20px 20px 6px.
* `.nav__link`: height 40px, margin 2px 12px, padding 0 12px, radius md, gap 12px, `--text-base`/500? → **sm/500** (13px) to fit admin's long list; icon 1.1rem opacity .8; hover `--sidebar-hover` + white; `.active`: `--sidebar-active` bg, white, 600, 3px `--sidebar-accent` bar on the left **inside** the pill (use `box-shadow: inset 3px 0 0 var(--sidebar-accent)`).
* `.nav__badge`: see counters in 5.7.
* `.sidebar-user`: sticky bottom, padding 14px 20px, bg `--sidebar-bg-deep`, name sm/600 white, role xs `--sidebar-accent`.
* Navigation list scrolls with a thin dark scrollbar; brand and user block never scroll.

### 6.2 Topbar `#topbar`
Contract: `.btn-sidebar-toggle`, `h1.page-heading`, `#bell (+ #bell-count.pill) + #bell-menu`, `.topbar-email`, `#logout.btn.btn--danger.btn--sm`.

* Height `--topbar-h`, `--surface`, 1px `--border` bottom, 3px top border `--sidebar-accent`, padding 0 24px (16 mobile), gap 12px.
* `.page-heading`: md/700 `--ink`, truncating.
* `#bell`: 40px icon button; `#bell-count` counter pinned top-right.
* `.topbar-email`: rendered as a **user chip**: sm/500 `--ink-soft`, padding 0 12px, height 32px, `--surface-subtle`, radius pill, hidden ≤ 600px.
* `#logout`: styled as ghost (`--ink-soft`, no border) that becomes `--danger` text + `--danger-bg` on hover/focus — override `.btn--danger` inside `#topbar` only.

### 6.3 Main
`#main-content` margin-left `--sidebar-width`, padding-top `--topbar-h`; `.content-wrapper` padding `--content-pad`, max `--content-max`; `aria-busy` dims to .6 with 180ms transition. ≤ 768: margin-left 0, padding `--content-pad-mobile`.

### 6.4 Auth layout (login / registro / recuperar)
Contract: `body.auth > main#login-view.auth-card > .auth-panel-left + .auth-panel-right`, left: `.brand-logo-auth`, `.brand-sub-auth`, `.auth-intro`, `.auth-divider`, `.auth-feature-item > .auth-feature-icon(.icon-green|.icon-blue|.icon-green-soft) + (.auth-feature-title + .auth-feature-sub)`, `.auth-university-badge`; right: `h2`, `.auth-subtitle`, `#login-error`, `form#login-form`, `.text-link`, `.auth-hr`, `.auth-note`, `.toggle-password .eye-icon`, `#auth-alt`.

* Page: `--bg` (light) / `--bg` (dark) with a soft radial brand wash (`--brand-glow`) top-left — never a black page in light mode.
* Card: max-width 960px, min-height 560px, radius xl, `--shadow-lg`, 1px `--border`, grid 5fr/7fr (stack ≤ 768 with the brand panel collapsing to a 96px header: logo + sub only; features hidden).
* Left panel: `--sidebar-bg` with the dotted brand pattern at 6% opacity; copy `rgba(255,255,255,.82)` (≥ 4.5:1), feature titles white 600, subs `rgba(255,255,255,.62)`; icon tiles 40px, radius md, tinted (`icon-green` → `--brand` on 16% green; `icon-blue` → `#5db5dc` on 16% blue; `icon-green-soft` → `--brand-300` on 10% green).
* Right panel: padding 48px (24 mobile); heading 3xl/800 `--ink`; subtitle base muted; fields 20px apart; submit `btn--primary btn--lg w-100`; links `--brand-text`; `.auth-hr` 1px `--hairline` 24px margins.
* Error alert sits above the form; `[hidden]` keeps it out of flow.

---

## 7. Screen patterns

### 7.1 Dashboard (`dashboard.css`, `dashboard.js`)
Contract: `#dashboard-panel > .card-panel (filter form) + #sin-rol + #kpis.dash-kpis + #graficas.dash-graficas + #accesos`; simple roles add `.dash-welcome` (`.dash-welcome-title/-sub/-actions/-hint`), `.dash-section-head > .dash-section-title + .dash-periodo`; charts are `.card-panel.dash-chart.dash-chart--med|--wide > .card-panel-header (h3.dash-chart-title + button.dash-chart-toggle) + .dash-chart-box > canvas + .dash-chart-data`.

* Welcome card: `--surface` with a left-to-right wash `linear-gradient(135deg, var(--brand-subtle), transparent 60%)`, radius xl, padding 28px (20 mobile), title 2xl/800, sub base muted; the 3 action buttons are `btn--primary` at `--control-h-lg` with hints below (sm muted, centred); 1 column ≤ 600.
* `.dash-section-title`: eyebrow (xs/600 uppercase muted) with 12px bottom margin; `.dash-section-head` aligns the compact period `select.input--sm` (min 12rem) to the right.
* KPI grid: `repeat(auto-fit, minmax(200px, 1fr))` ≥ 992 (max 4 per row via `max-width` on the grid), 2 columns 600–991, 2 columns ≤ 600 with 12px gap.
* Chart cards: `.dash-chart-box` fixed height 300px (260 mobile), padding 16px 20px 20px; `.dash-chart-data` table under a hairline when toggled.
* Admin filter card: unchanged structure; `.form-grid` 2 columns, Apply button left, "Mostrando" line sm muted.

Chart options (JS, `dashboard.js` — the only JS change allowed):
* Read `--chart-1…8`, `--chart-grid`, `--chart-tick`, `--surface`, `--font-sans` with the existing `dash_token()` helper **when rendering** (so dark mode is respected on reload).
* Replace `serie.colors` with the token palette in order (cycle if > 8). Doughnut: `cutout: "62%"`, `borderColor: surface`, `borderWidth: 2`, `hoverOffset: 6`. Bar: `borderRadius: 6`, `borderSkipped: false`, `maxBarThickness: 48`, single colour `--chart-1` unless the series is categorical by state (keep per-item palette when labels ≠ time buckets — rule: use the palette per item for `doughnut`, `--chart-1` for `bar`/`line`). Line: `--chart-2` stroke 2px, fill `--chart-2` at 12% alpha, `pointRadius 3`, `tension .3`.
* Grid: y grid colour `--chart-grid`, x grid hidden, ticks `--chart-tick` 12px, `precision 0`.
* Legend: bottom, `usePointStyle: true`, `pointStyle: "circle"`, `boxWidth 8`, padding 16, font 12px.
* Labels longer than 18 characters are truncated with "…" on the axis/legend via `ticks.callback` / legend `generateLabels`, while tooltips show the full label (`tooltip.callbacks.title` returning the original). Mobile: `maxRotation: 0`, `autoSkip: true`.
* Everything else (data fetch, series shape, `Ver datos` table, skeletons) untouched.

### 7.2 List page (`records.css`)
Header (4.2) → `.card-panel` with `.card-panel-header` containing the search form (input max 20rem + secondary button + exports right) → `.table-wrap` → pagination cluster centred → `#extra-panel`. `#table-body td` truncates at 24rem with `title` preserved. Skeleton rows 48px.

### 7.3 Detail modal (`detail.css`)
Contract: `dl.details > dt + dd` pairs (dd may contain `.badge`, links, `pre-line` text), then `.detail-actions > .btn…`.

* `.details`: no outer border; `display:grid; grid-template-columns: 1fr 1fr; column-gap 32px; row-gap 0` ≥ 720px, 1 column below. Each `dt+dd` pair is rendered as a block: `dt` eyebrow (xs/600 muted) with 2px bottom margin, `dd` base `--ink` `--leading-snug`, pair padding 12px 0, 1px `--hairline` bottom. Achieve the pairing with `display: contents` **not** available for dl children in a 2-col grid — instead use `grid-auto-flow: row dense` with `dt { grid-column: span 1 }` and `dd { grid-column: span 1 }` stacked by making the grid `grid-template-columns: repeat(2, 1fr)` and each `dt`/`dd` occupying consecutive rows via `grid-row: auto`; simplest robust approach: keep **one column** of `dt/dd` blocks but limit the dialog body to two CSS columns with `column-count: 2; column-gap: 32px` and `dt, dd { break-inside: avoid }`, `dt { break-after: avoid }` ≥ 720px. Use the `column-count` approach.
* Long values (`dd` > 160 chars, e.g. descriptions) cannot be detected in CSS; accept that they flow in a column.
* `dd .badge` renders as a pill (5.7).
* `.detail-actions`: sticky to the dialog bottom (`position: sticky; bottom: -24px; margin: 24px -24px -24px; padding: 16px 24px; background: var(--surface); border-top: 1px solid var(--hairline)`), gap 8px, wraps; the first `.btn--primary` keeps emphasis, others secondary/ghost as emitted.

### 7.4 Form modal (`forms.css`)
`form.form-grid` inside the dialog body: 2 columns ≥ 640, 16px 20px gaps; submit row (`.field--full` with the primary button) right-aligned ≥ 640 (`justify-self: end`), full width on mobile; `.forms-actions` cluster; `.forms-comment` sm muted.

### 7.5 Workflow pages (`workflows.css`) and builder (`builder.css`)
Re-skin with tokens only: `.wf-section` = surface; `.wf-form.wf-grid` = `.form-grid` rules; `.wf-table` = data table; `.wf-actions` = cluster with 8px gap and 16px top margin; `.wf-files` list with file icon rows 40px; `.wf-upload-progress` = progress. Builder: `.builder-grid` 2 columns ≥ 992, `.builder-section` surface, `.builder-title` lg/700, `.builder-hint` sm muted.

---

## 8. Dark mode

Automatic via `prefers-color-scheme: dark`; no toggle. Rules:
* Surfaces layer by lightness, never by shadow alone: `--bg` < `--surface` < `--surface-raised`.
* Primary button = bright `--brand-500` fill with `--on-brand` dark text.
* Borders stay visible (`--border` #28322b); hairlines `rgba(255,255,255,.08)`.
* Semantic fg are the light variants; `-bg` are 14–16% alpha tints, so pills read on any dark surface.
* Charts use the dark palette tokens; grid `--chart-grid`.
* Login page uses dark `--bg` with the same glow; the brand panel is unchanged (already dark).
* Images/canvases are never inverted.

---

## 9. Accessibility

* Contrast: all text ≥ 4.5:1, icons/borders of controls ≥ 3:1 (checked against the token table in 2.1).
* Focus: visible ring on every interactive element via `:focus-visible`; dialogs keep focus inside (native `<dialog>`).
* Targets: ≥ 40×40px for primary controls, ≥ 32px for in-table buttons with ≥ 8px spacing.
* Status never by colour alone (pills carry text + dot; alerts carry icon + text).
* Motion removed under `prefers-reduced-motion` (tokens zero the durations).
* Text can zoom to 200% without horizontal scroll on the main content; tables scroll inside `.table-wrap`.
* Existing ARIA (`aria-live`, `aria-busy`, `aria-sort`, `aria-labelledby`, `scope`) is untouched.

---

## 10. Implementation constraints (read before editing)

1. **CSS only** except the chart block in `dashboard.js` (7.1). No HTML edits, no class renames, no new JS behaviour.
2. CSP is `default-src 'self'; script-src 'self'; style-src 'self'; font-src 'self'; img-src 'self' data:; frame-ancestors 'none'; base-uri 'none'; form-action 'self'` (Chart.js is the vendored `web/chart.umd.min.js`, served as `/assets/chart.js`): never inline `style=""`; `color-mix()`, `dvh`, `:has()`, `inset`, container queries are fine (Chromium/Firefox/Safari ≥ 16).
3. Every colour, size, radius, shadow and duration comes from `tokens.css`. If a value is missing, add a token there (and document it) instead of a literal.
4. Keep every existing selector reachable: when restyling, edit rules in place; do not delete a class that JS or HTML references (grep first).
5. Do not import fonts or frameworks. Icons are inline SVG via `icon()`/sprite.
6. Legacy Bootstrap-ish aliases (`.btn-primary`, `.table`, `.form-check`, `.dropdown-*`, `.card-stat`, `.table-responsive`) stay as thin aliases.
7. `[hidden]` always wins; never set `display` on an element in a way that a later rule overrides `[hidden]`.
8. Mobile first is not required, but every rule block must be checked at 360px and in dark mode.

---

## 11. `ui.*` helpers, badge map and icons (unchanged API)

```js
ui.button("Guardar", { variant: "primary", icon: "check", onClick })      // .btn.btn--primary
ui.badge("success", { label: "Aprobado" })                                 // .pill.pill--success.dot
ui.card(title, bodyNode, actionsNode)                                      // section.surface
ui.field({ name, label, type, required, hint, error, onInput })            // .field …
ui.table(columns, rows, { empty, caption })                                // .table-wrap > table.data-table
ui.empty(icon, title, hint, actionNode)                                    // .empty
ui.skeleton(n, { kind: "row|text|title|avatar" })
ui.pagination({ page, pages, onPage, info })
ui.tabs([{ label, content }])
await ui.confirm(message, { title, confirmText, variant })
busy(button, promise)             // .is-loading + .spinner
setPageTitle(text, crumbs)        // heading + <title> + #breadcrumbs
toast(message, "success|error|info|warning")
modal(title) → #dialog-content
```

`badgeClass(value)` → `pill--success` (aprobado, abierto, en_curso, finalizado, publicado, aceptada, verificado, calificado, firmado) · `pill--neutral` (borrador, pendiente, no_radicado) · `pill--warning` (en_revision) · `pill--danger` (rechazado, cancelado, retirada, vencido) · `pill--info` (else).

Icons: `icon(name)` from `js/icons.js` (`speedometer2, people, building, trophy, flag, bar-chart-line, cash-coin, plus-circle, person-plus, person-check, file-earmark-text, chat-square-text, journal-text, diagram-3, folder-check, clipboard2-check, search, send, star, folder2-open, award, lightning-charge, table, person-circle, box-arrow-left/right/in-right, bell, play-circle, hourglass-split, mortarboard, person-video3, check2-circle, x-circle, clock-history, cloud-arrow-up, plus-lg, eye, eye-slash, download, upload, envelope, lock, person, list, funnel, calendar3, chevron-left/right, x, check, exclamation-triangle` and `-fill` variants). Unknown names render a neutral dot.

---

## 12. QA checklist and rating rubric

Before merging a visual change, capture the screenshot set
(`node shoot.mjs <dir> estudiante.demo,profesor.demo,empresa.verificada,admin.demo`
with `SCHEME=light` and `SCHEME=dark`) and run the API smoke test. Then score:

| Dimension (0–5) | Pass criteria |
|---|---|
| Hierarchy | One obvious primary action per screen; eyebrows, titles and values differ in size **and** weight. |
| Consistency | Same control looks identical across dashboard, list, modal, workflow and login. |
| Legibility & contrast | No text under 12px; all text ≥ 4.5:1 light and dark; numbers tabular. |
| Density | Rows 44–52px; cards ≥ 20px padding; no wrapped 3-line labels at 360px. |
| Mobile | No horizontal page scroll at 360px; sheet dialogs; 2-col KPIs; drawer nav. |
| Dark mode | Every surface, border, pill, chart and input visibly styled; no black-on-black or white flashes. |
| Feedback | Hover/focus/active/loading/disabled on every control; skeletons while loading; toasts for results. |
| Brand | Green is identity + primary; blue secondary; charts use the token palette; no stray default colours. |
| Behaviour parity | All lists, details, forms and flows work exactly as before (smoke test + console clean). |

Target for v2: ≥ 4 in every dimension.

---

## 13. v2 scorecard (2026-11-09)

Scored against the rubric in §12 after rebuilding, 68 + 7 screenshots in light
and dark (desktop 1440 px and 390 px), an API smoke test on 9 accounts and a
clean browser console.

| Dimension | v1 | v2 | Evidence / remaining gap |
|---|---|---|---|
| Hierarchy | 3 | 4.5 | Eyebrow → value → note on KPIs; one primary per screen. Gap: list pages still show one identical "Ver detalle" button per row. |
| Consistency | 3 | 4.5 | All controls come from one token set; dashboard, lists, modals, workflows and login match. Gap: the auth submit is `w-100` not `btn--lg` (frozen HTML). |
| Legibility & contrast | 2.5 | 5 | Every text pair ≥ 4.5:1 in both themes (measured); body 15 px, cells 13 px, tabular numerals. |
| Density | 3 | 4 | 48 px rows, 20 px card padding, no 3-line KPI labels at 390 px. Gap: 26-field detail sheet is still long on phones. |
| Mobile | 3 | 4 | Drawer nav, bottom-sheet dialogs, 2-col KPIs, no page-level horizontal scroll. Gap: tables scroll sideways instead of stacking as cards. |
| Dark mode | 3 | 4.5 | All tokens re-derived; charts, inputs, pills, sheets verified. Gap: date-picker popups follow the OS, not the page. |
| Feedback | 3.5 | 4 | Hover/focus/active/loading/disabled on every control; skeletons; toasts. Gap: no optimistic row highlight after an action in lists. |
| Brand | 3 | 4.5 | Green = identity + primary, blue secondary, token chart palette. Gap: logo dots are still CSS spans, not an SVG mark. |
| Behaviour parity | — | 5 | Zero HTML/JS class changes; smoke test and console clean; chart data flow untouched. |

**Overall: 4.4 / 5** (v1 ≈ 3.0). Next improvements, in order of value:

1. **Clickable rows + row chevron** on list pages (JS: one delegated click handler) so the repeated "Ver detalle" button can become an icon-only affordance.
2. **Card layout for tables under 600 px** (`data-label` on `td` + a `.data-table--stack` rule) instead of horizontal scroll.
3. **Grouped detail sheet**: let `detail.js` emit `<section>` groups (Datos, Fechas, Estado, Auditoría) so long records collapse auditing fields by default.
4. **Readable document-type names** (FICHA_RETO → "Ficha del reto") via the existing `choice()` map.
5. **Sticky table header inside `.table-wrap`** on desktop for lists longer than the viewport.
6. **Theme toggle** (light/dark/system) persisted in `localStorage`, since the tokens already support it.
7. **Inline SVG logo** (replace the 9 dot spans) and a 512 px app icon for the PWA manifest.
8. **Optimistic feedback**: highlight the changed row for 1.5 s after approve/reject actions.
