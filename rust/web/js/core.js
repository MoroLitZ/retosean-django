"use strict";
const $ = (id) => document.getElementById(id);
let user,
  csrf,
  resources = [],
  selected = "",
  page = 1,
  query = "",
  total = 0,
  charts = [];
// Fallback names for tables whose Django model has no explicit verbose_name.
const labels = {
  dashboard: "Dashboard",
  cierre_agendacierre: "Agenda de cierre",
  cierre_entregablefinal: "Entregables finales",
  cierre_encuestasatisfaccion: "Encuestas",
  evaluacion_rubrica: "Rúbricas",
  evaluacion_criteriorubrica: "Criterios de evaluación",
  evaluacion_evaluacioncriterio: "Puntajes por criterio",
  evaluacion_comentarioentregable: "Comentarios de entregables",
};
const capfirst = (s) => String(s).replace(/^./, (c) => c.toUpperCase());
const human = (text) =>
  labels[text] || capfirst(String(text).replaceAll("_", " ").toLowerCase());
const meta = (table) => resources.find((r) => r.table === table);
const field = (table, name) =>
  meta(table)?.fields.find((f) => f.name === name);
// Django's verbose_name_plural; auto-generated ones are lowercase, so prefer our label.
function title(table) {
  const plural = meta(table)?.verbose_name_plural;
  return plural && plural !== plural.toLowerCase()
    ? plural
    : labels[table] || capfirst(plural || human(table));
}
// get_FOO_display(): the choice label of a value, else the raw value.
function choice(table, name, value) {
  const found = field(table, name)?.choices.find((c) => c[0] === value);
  return found ? found[1] : value;
}
// Same colours as the Django estado badge partial.
function badgeClass(value) {
  const v = String(value).toLowerCase();
  if (/^(aprobad[oa]|abierto|en_curso|finalizado|publicad[oa]|aceptada|verificad[oa]|calificado|firmado)$/.test(v))
    return "bg-success";
  if (/^(borrador|pendiente|no_radicado)$/.test(v)) return "bg-secondary";
  if (v === "en_revision") return "bg-warning text-dark";
  if (/^(rechazad[oa]|cancelad[oa]|retirada|vencido)$/.test(v)) return "bg-danger";
  return "bg-info text-dark";
}
// Sidebar per role, same sections, labels and icons as templates/base.html.
const MENU = {
  ADMIN: [
    ["Administración"],
    ["dashboard", "Dashboard", "speedometer2"],
    ["usuarios", "Usuarios", "people"],
    ["empresas", "Empresas", "building"],
    ["tipos_documento_legal", "Tipos de documento", "file-earmark-ruled"],
    ["notificaciones", "Registro de notificaciones", "envelope-paper"],
    ["retos", "Retos", "trophy"],
    ["integraciones_academicas", "Vinculaciones", "link-45deg"],
    ["cierres_reto", "Cierre de Retos", "flag"],
    ["reportes", "Reportes", "bar-chart-line"],
    ["presupuestos", "Presupuesto", "cash-coin"],
    ["Gestión Académica"],
    ["unidades_estudio", "Unidades de Estudio", "journal-bookmark-fill"],
  ],
  EMPRESA: [
    ["Mi Empresa"],
    ["dashboard", "Dashboard", "speedometer2"],
    ["publicar", "Publicar Reto", "plus-circle"],
    ["retos", "Mis Retos", "trophy"],
    ["integraciones_academicas", "Vinculaciones", "person-plus"],
    ["postulaciones", "Postulaciones", "person-check"],
    ["documentos_empresas", "Documentos Legales", "file-earmark-text"],
    ["seguimientos_reto", "Seguimiento", "activity"],
    ["cierres_reto", "Cierre de Retos", "flag"],
    ["cierre_encuestasatisfaccion", "Mis Encuestas", "chat-square-text"],
  ],
  PROFESOR: [
    ["Docencia"],
    ["dashboard", "Dashboard", "speedometer2"],
    ["integraciones_academicas", "Mis Cursos", "journal-text"],
    ["retos", "Integrar Reto", "diagram-3"],
    ["postulaciones", "Mis Estudiantes", "people"],
    ["entregables", "Entregables", "folder-check"],
    ["evaluaciones", "Evaluaciones", "clipboard2-check"],
    ["cierres_reto", "Cierre de Retos", "flag"],
    ["cierre_encuestasatisfaccion", "Mis Encuestas", "chat-square-text"],
    ["unidades_estudio", "Unidades de Estudio", "journal-bookmark-fill"],
  ],
  EXPERTO: [
    ["Jurado"],
    ["dashboard", "Dashboard", "speedometer2"],
  ],
  ESTUDIANTE: [
    ["Mi Espacio"],
    ["dashboard", "Dashboard", "speedometer2"],
    ["retos", "Explorar Retos", "search"],
    ["postulaciones", "Mis Postulaciones", "send"],
    ["retos_favoritos", "Mis Favoritos", "star"],
    ["entregables", "Mis Entregables", "folder2-open"],
    ["cierre_encuestasatisfaccion", "Mis Encuestas", "chat-square-text"],
    ["certificados", "Certificados", "award"],
    ["equipos_reto_academico", "Mis Equipos y Retos", "people"],
  ],
};
const ROLES = {
  ADMIN: "Administrador",
  EMPRESA: "Empresa",
  PROFESOR: "Profesor",
  ESTUDIANTE: "Estudiante",
  EXPERTO: "Experto",
};
let navLabels = {};
const admin = () => user.is_superuser || user.rol === "ADMIN";
const role = () => (admin() ? "ADMIN" : user.rol);
async function api(path, data) {
  const response = await fetch(path, {
    method: data === undefined ? "GET" : "POST",
    headers:
      data === undefined
        ? {}
        : { "Content-Type": "application/json", "X-CSRF-Token": csrf || "" },
    body: data === undefined ? undefined : JSON.stringify(data),
  });
  const body = await readJson(response);
  if (!response.ok) {
    if (response.status === 401 && !path.endsWith("/login")) showLogin();
    throw new Error(body.error || "No se pudo completar la operación");
  }
  return body;
}
// Axum rejections (413 body limit, 415/422 JSON errors) and proxies answer
// with plain text; never surface a raw SyntaxError to the user.
async function readJson(response) {
  const text = await response.text().catch(() => "");
  try {
    return text ? JSON.parse(text) : {};
  } catch {
    if (response.ok) return {};
    return {
      error: response.status === 413
        ? "El contenido enviado es demasiado grande."
        : "Respuesta inesperada del servidor.",
    };
  }
}
let toastTimer;
function hideToast() {
  const t = $("toast");
  clearTimeout(toastTimer);
  try {
    t.hidePopover();
  } catch {}
  t.hidden = true;
}
// The toast is a manual popover so it joins the top layer and stays visible
// above an open modal <dialog> and its backdrop. Long messages stay longer.
function toast(message) {
  const t = $("toast");
  const text = String(message ?? "");
  clearTimeout(toastTimer);
  t.textContent = text;
  t.title = "Haz clic para cerrar";
  t.onclick = hideToast;
  t.hidden = false;
  try {
    t.hidePopover();
  } catch {}
  try {
    t.showPopover();
  } catch {}
  toastTimer = setTimeout(hideToast, Math.max(4500, 60 * text.length));
}
function showLogin() {
  $("app-view").hidden = true;
  $("login-view").hidden = false;
  document.body.className = "auth";
  document.title = "Retosean — Universidad EAN";
  user = null;
  csrf = null;
  authRoute().catch(() => {});
}
function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}
// Peso amounts in es-CO format: $1.234.567,89
function cop(value) {
  const n = Number(value);
  if (value === null || value === undefined || value === "" || !Number.isFinite(n)) return String(value ?? "");
  return "$" + n.toLocaleString("es-CO", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}
// Every row of a records list, paging past the server's 100-row cap.
async function allRecords(table) {
  const out = [];
  for (let page = 1; page <= 1000; page++) {
    const data = await api(`/api/records/${table}?page=${page}&q=&limit=100`);
    out.push(...data.records);
    if (!data.records.length || out.length >= Number(data.total ?? 0)) break;
  }
  return out;
}
function icon(name) {
  return el("i", "bi bi-" + name);
}
async function start() {
  const session = await api("/api/me");
  user = { ...session.user, permisos: session.permisos || [] };
  csrf = session.csrf;
  resources = await api("/api/resources");
  $("login-view").hidden = true;
  $("app-view").hidden = false;
  document.body.className = "role-" + role().toLowerCase();
  const name =
    [user.first_name, user.last_name].filter(Boolean).join(" ") ||
    user.username;
  $("user-avatar").textContent = (
    ([...(user.first_name || "")][0] || "") + ([...(user.last_name || "")][0] || "")
  ).toUpperCase();
  $("user-name").textContent = name;
  $("user-name").title = name;
  $("user-role").textContent = admin() ? "Administrador" : ROLES[user.rol] || "Administrador";
  $("topbar-email").textContent = user.email;
  buildNav();
  await route();
  refreshBell().catch(() => {});
}
function buildNav() {
  const nav = $("navigation");
  nav.replaceChildren();
  navLabels = {};
  const has = (table) =>
    table === "dashboard" ||
    (table === "publicar"
      ? has("retos")
      : table === "reportes"
        ? has("reportes_generados")
        : resources.some((r) => r.table === table));
  const item = (target, text, iconName) => {
    const li = el("li", "nav-item");
    const a = el("a", "nav-link");
    a.href = "#" + target;
    a.dataset.resource = target;
    a.append(icon(iconName), " " + text);
    li.append(a);
    nav.append(li);
    navLabels[target] ??= text;
    return a;
  };
  const section = (text) => {
    const li = el("li");
    li.append(el("div", "nav-section-label", text));
    nav.append(li);
  };
  for (const [target, text, iconName] of MENU[role()] || [])
    if (!text) section(target);
    else if (has(target)) item(target, text, iconName);
  if (admin()) {
    // Rust-only: every other table the admin may browse (Django admin territory).
    const shown = new Set(MENU.ADMIN.map((m) => m[0]));
    const rest = resources.filter((r) => !shown.has(r.table) && !["hackathons", "notificaciones"].includes(r.table));
    if (rest.length) section("Registros");
    for (const r of rest) item(r.table, title(r.table), "table");
  }
  section("Cuenta");
  const perfil = el("li", "nav-item");
  const perfilButton = el("button", "nav-link");
  perfilButton.type = "button";
  perfilButton.append(icon("person-circle"), " Mi Perfil");
  perfilButton.onclick = () => profileForm().catch(showError);
  perfil.append(perfilButton);
  nav.append(perfil);
  const salir = el("li", "nav-item");
  const salirButton = el("button", "nav-link logout");
  salirButton.type = "button";
  salirButton.append(icon("box-arrow-left"), " Cerrar Sesión");
  salirButton.onclick = logout;
  salir.append(salirButton);
  nav.append(salir);
  const hr = el("li");
  hr.append(el("hr", "my-2"));
  nav.append(hr);
  if (has("hackathons")) item("hackathons", "Hackathones", "lightning-charge");
  if (has("notificaciones")) {
    const a = item("notificaciones", "Notificaciones", "bell");
    a.append(el("span", "badge bg-danger ms-1 nav-unread"));
  }
}
async function refreshBell() {
  // Prefer the per-user endpoint (exact unread count, own notifications only);
  // fall back to the generic list filtered client-side.
  let mine, unread;
  const own = await api("/api/notificaciones/mias").catch(() => null);
  if (own && Array.isArray(own.items)) {
    mine = own.items;
    unread = Number(own.no_leidas) || 0;
  } else {
    const data = await api("/api/records/notificaciones?page=1&q=");
    mine = data.records.filter((n) => n.usuario_id === user.id);
    unread = mine.filter((n) => !n.leida).length;
  }
  for (const badge of [$("bell-count"), ...document.querySelectorAll(".nav-unread")]) {
    badge.hidden = !unread;
    badge.textContent = unread || "";
  }
  const list = $("bell-items");
  list.replaceChildren();
  for (const n of mine.slice(0, 8)) {
    const a = el("a", "dropdown-item d-flex gap-2 align-items-start py-2" + (n.leida ? "" : " bg-light"));
    a.href = "#notificaciones";
    const kind = { EXITO: "check-circle-fill text-success", ADVERTENCIA: "exclamation-triangle-fill text-warning", ERROR: "x-circle-fill text-danger" }[n.tipo] || "info-circle-fill text-info";
    const iconWrap = el("span", "pt-1");
    iconWrap.append(icon(kind));
    const body = el("span", "flex-grow-1");
    body.append(
      el("span", "d-block fw-semibold small text-dark", n.titulo),
      el("span", "d-block text-muted small text-wrap bell-message", String(n.mensaje || "")),
      el("span", "d-block text-muted small", fecha(n.creada_en, true)),
    );
    a.append(iconWrap, body);
    list.append(a);
  }
  if (!mine.length) {
    const empty = el("div", "text-center text-muted py-4 small");
    empty.append(icon("bell-slash d-block mb-2"), "Sin notificaciones");
    list.append(empty);
  }
}
// Django's es-co date format: d/m/Y (H:i).
function fecha(value, withTime) {
  const d = new Date(value);
  if (Number.isNaN(d.getTime())) return String(value ?? "");
  const pad = (n) => String(n).padStart(2, "0");
  const text = `${pad(d.getDate())}/${pad(d.getMonth() + 1)}/${d.getFullYear()}`;
  return withTime ? `${text} ${pad(d.getHours())}:${pad(d.getMinutes())}` : text;
}
async function route() {
  if (!user) return authRoute().catch(showError);
  $("app-error").hidden = true;
  $("bell-menu").hidden = true;
  $("sidebar").classList.remove("open");
  $("sidebar-overlay").classList.remove("show");
  selected = location.hash.slice(1) || "dashboard";
  if (selected === "publicar") {
    location.hash = "retos";
    challengeForm();
    return;
  }
  page = 1;
  query = "";
  $("search").value = "";
  if (
    !["dashboard", "reportes"].includes(selected) &&
    !resources.some((r) => r.table === selected)
  ) {
    location.hash = "dashboard";
    return;
  }
  document
    .querySelectorAll("#navigation a")
    .forEach((a) =>
      a.classList.toggle("active", a.dataset.resource === selected),
    );
  const heading =
    selected === "dashboard"
      ? "Dashboard de Indicadores"
      : selected === "reportes"
        ? "Generación de Informes y Reportes"
        : navLabels[selected] || title(selected);
  $("page-heading").textContent = heading;
  $("page-title").textContent = heading;
  document.title = `${heading} — Retosean`;
  $("new-challenge").hidden = !(
    (selected === "retos" && user.rol === "EMPRESA") ||
    ([
      "usuarios",
      "facultades",
      "programas",
      "ecosistemas",
      "unidades_estudio",
    ].includes(selected) &&
      admin()) ||
    (selected === "unidades_estudio" && user.rol === "PROFESOR")
  );
  $("new-label").textContent =
    selected === "usuarios"
      ? "Crear usuario"
      : selected === "retos"
        ? "Publicar Reto"
        : "Agregar registro";
  $("import-users").hidden = !(selected === "usuarios" && admin());
  $("dashboard-panel").hidden = selected !== "dashboard";
  $("builder-panel").hidden = selected !== "reportes";
  $("records-panel").hidden = ["dashboard", "reportes"].includes(selected);
  if (selected === "dashboard") await loadDashboard();
  else if (selected === "reportes") await loadBuilder();
  else await loadRecords();
}
// ── Dashboard de Indicadores (apps/dashboard/templates/dashboard/*.html) ──
