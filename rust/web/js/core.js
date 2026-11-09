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
    return "pill--success";
  if (/^(borrador|pendiente|no_radicado)$/.test(v)) return "pill--neutral";
  if (v === "en_revision") return "pill--warning";
  if (/^(rechazad[oa]|cancelad[oa]|retirada|vencido)$/.test(v)) return "pill--danger";
  return "pill--info";
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
    ["dashboard", "Inicio", "speedometer2"],
    ["publicar", "Publicar Reto", "plus-circle"],
    ["retos", "Mis Retos", "trophy"],
    ["integraciones_academicas", "Cursos vinculados", "person-plus"],
    ["postulaciones", "Estudiantes postulados", "person-check"],
    ["documentos_empresas", "Mis documentos legales", "file-earmark-text"],
    ["seguimientos_reto", "Avance de mis retos", "activity"],
    ["cierres_reto", "Cerrar retos", "flag"],
    ["cierre_encuestasatisfaccion", "Encuestas", "chat-square-text"],
  ],
  PROFESOR: [
    ["Docencia"],
    ["dashboard", "Inicio", "speedometer2"],
    ["integraciones_academicas", "Mis Cursos", "journal-text"],
    ["retos", "Integrar Reto", "diagram-3"],
    ["postulaciones", "Mis Estudiantes", "people"],
    ["entregables", "Calificar entregables", "folder-check"],
    ["evaluaciones", "Evaluaciones", "clipboard2-check"],
    ["cierres_reto", "Cerrar retos", "flag"],
    ["cierre_encuestasatisfaccion", "Encuestas", "chat-square-text"],
    ["unidades_estudio", "Unidades de Estudio", "journal-bookmark-fill"],
  ],
  EXPERTO: [
    ["Jurado"],
    ["dashboard", "Dashboard", "speedometer2"],
  ],
  ESTUDIANTE: [
    ["Mi Espacio"],
    ["dashboard", "Inicio", "speedometer2"],
    ["retos", "Explorar Retos", "search"],
    ["postulaciones", "Mis Postulaciones", "send"],
    ["retos_favoritos", "Mis Favoritos", "star"],
    ["entregables", "Mis Entregables", "folder2-open"],
    ["cierre_encuestasatisfaccion", "Encuestas", "chat-square-text"],
    ["certificados", "Certificados", "award"],
    ["equipos_reto_academico", "Mis equipos", "people"],
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
// Roles con interfaz simplificada: ESTUDIANTE, PROFESOR y EMPRESA.
// ADMIN y EXPERTO (y todo superusuario) conservan la interfaz de siempre.
const simpleRole = () => ["ESTUDIANTE", "PROFESOR", "EMPRESA"].includes(role());
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
    const error = new Error(body.error || "No se pudo completar la operación");
    error.status = response.status;
    throw error;
  }
  return body;
}
// Variante de api() para cuerpos multipart (FormData): mismo manejo de CSRF,
// de sesión expirada (401 → showLogin) y de errores normalizados.
async function apiForm(path, formData) {
  const response = await fetch(path, {
    method: "POST",
    headers: { "X-CSRF-Token": csrf || "" },
    body: formData,
  });
  const body = await readJson(response);
  if (!response.ok) {
    if (response.status === 401) showLogin();
    const error = new Error(body.error || "No se pudo completar la operación");
    error.status = response.status;
    throw error;
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
function toast(message, variant = "info") {
  const t = $("toast");
  const text = String(message ?? "");
  clearTimeout(toastTimer);
  const names = {
    success: "check-circle-fill",
    error: "x-circle-fill",
    info: "info-circle-fill",
    warning: "exclamation-triangle-fill",
  };
  const key = names[variant] ? variant : "info";
  t.className = "toast--" + key;
  t.replaceChildren();
  t.append(icon(names[key]));
  const msg = el("span", "toast__msg");
  msg.textContent = text;
  const close = el("button", "toast__close");
  close.type = "button";
  close.setAttribute("aria-label", "Cerrar notificación");
  close.append(icon("x"));
  close.onclick = hideToast;
  t.append(msg, close);
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
  // Cierra el diálogo de detalle: vive fuera de #app-view y, si quedara
  // abierto sobre el login, bloquearía el inicio de sesión con datos viejos.
  if ($("detail-dialog").open) $("detail-dialog").close();
  $("dialog-content").replaceChildren();
  // Deja el shell en un estado limpio y con su aria sincronizado.
  setBellMenu(false);
  setSidebar(false);
  $("app-view").hidden = true;
  $("login-view").hidden = false;
  document.body.className = "auth";
  document.title = "Retosean — Universidad EAN";
  user = null;
  csrf = null;
  authRoute().catch(showError);
}
function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}
// URL de datos del servidor para un href: solo http(s), mailto o relativas.
// Devuelve null para cualquier otro esquema (javascript:, data:, file:, …).
function safeHref(value) {
  const text = String(value ?? "").trim();
  if (!text) return null;
  let url;
  try {
    url = new URL(text, location.origin);
  } catch {
    return null;
  }
  return ["http:", "https:", "mailto:"].includes(url.protocol) ? url.href : null;
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
// Renders an inline <svg> from the ICONS set (icons.js). Compound names like
// "bell-slash d-block mb-2" are tolerated: the first token is the icon, the rest
// become extra classes on the <svg>. Unknown names fall back to a neutral dot.
function icon(name) {
  let extra = "";
  if (typeof name === "string") {
    const parts = name.trim().split(/\s+/);
    if (parts.length > 1) {
      name = parts[0];
      extra = parts.slice(1).join(" ");
    }
  }
  const x = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  x.setAttribute("class", "icon" + (extra ? " " + extra : ""));
  x.setAttribute("viewBox", "0 0 24 24");
  x.setAttribute("fill", "none");
  x.setAttribute("stroke", "currentColor");
  x.setAttribute("stroke-width", "1.8");
  x.setAttribute("stroke-linecap", "round");
  x.setAttribute("stroke-linejoin", "round");
  x.setAttribute("aria-hidden", "true");
  x.setAttribute("focusable", "false");
  const icons = window.ICONS || {};
  x.innerHTML = icons[name] || icons.circle || "";
  return x;
}
async function start() {
  const session = await api("/api/me");
  user = {
    ...session.user,
    permisos: session.permisos || [],
    gestionaUnidades: session.gestiona_unidades === true,
  };
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
  // Roles simplificados ven su nombre completo en la barra superior;
  // ADMIN y EXPERTO conservan el correo.
  $("topbar-email").textContent = simpleRole() ? name : user.email;
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
    const li = el("li", "nav__item");
    const a = el("a", "nav__link");
    a.href = "#" + target;
    a.dataset.resource = target;
    a.append(icon(iconName), el("span", "nav__label", text));
    li.append(a);
    nav.append(li);
    navLabels[target] ??= text;
    return a;
  };
  // Encabezados de grupo visibles también para lectores de pantalla: dan
  // orientación dentro del landmark de navegación.
  const section = (text) => {
    const li = el("li", "nav__section");
    li.append(el("span", "", text));
    nav.append(li);
  };
  for (const [target, text, iconName] of MENU[role()] || [])
    if (!text) section(target);
    else if (has(target)) item(target, text, iconName);
  // Hackathones y Notificaciones: para los roles simplificados van dentro de la
  // sección principal (antes de «Cuenta»); ADMIN y EXPERTO las conservan al
  // final, después de «Cerrar Sesión» y el divisor, como siempre.
  const extras = () => {
    if (has("hackathons")) item("hackathons", "Hackathones", "lightning-charge");
    if (has("notificaciones")) {
      const a = item("notificaciones", "Notificaciones", "bell");
      a.append(el("span", "nav__badge"));
    }
  };
  if (simpleRole()) extras();
  if (admin()) {
    // Rust-only: every other table the admin may browse (Django admin territory).
    const shown = new Set(MENU.ADMIN.map((m) => m[0]));
    const rest = resources.filter((r) => !shown.has(r.table) && !["hackathons", "notificaciones"].includes(r.table));
    if (rest.length) section("Registros");
    for (const r of rest) item(r.table, title(r.table), "table");
  }
  section("Cuenta");
  const perfil = el("li", "nav__item");
  const perfilButton = el("button", "nav__link");
  perfilButton.type = "button";
  perfilButton.append(icon("person-circle"), el("span", "nav__label", "Mi Perfil"));
  perfilButton.onclick = () => profileForm().catch(showError);
  perfil.append(perfilButton);
  nav.append(perfil);
  const salir = el("li", "nav__item");
  const salirButton = el("button", "nav__link logout");
  salirButton.type = "button";
  salirButton.append(icon("box-arrow-left"), el("span", "nav__label", "Cerrar Sesión"));
  salirButton.onclick = logout;
  salir.append(salirButton);
  nav.append(salir);
  if (!simpleRole()) {
    const divider = el("li", "nav__divider");
    divider.setAttribute("aria-hidden", "true");
    nav.append(divider);
    extras();
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
  for (const badge of [$("bell-count"), ...document.querySelectorAll(".nav__badge")]) {
    badge.hidden = !unread;
    badge.textContent = unread || "";
  }
  $("bell").setAttribute(
    "aria-label",
    unread ? `Notificaciones: ${unread} sin leer` : "Notificaciones",
  );
  // Prototipo nulo: un `tipo` como "constructor" o "toString" no hereda
  // funciones de Object.prototype y cae siempre en el respaldo INFO.
  const styles = Object.assign(Object.create(null), {
    EXITO: ["check-circle-fill", "text-success"],
    ADVERTENCIA: ["exclamation-triangle-fill", "text-warning"],
    ERROR: ["x-circle-fill", "text-danger"],
    INFO: ["info-circle-fill", "text-info"],
  });
  const list = $("bell-items");
  list.replaceChildren();
  for (const n of mine.slice(0, 8)) {
    const a = el("a", "menu__item" + (n.leida ? "" : " menu__item--unread"));
    a.href = "#notificaciones";
    a.setAttribute("role", "menuitem");
    const [iname, icls] = styles[n.tipo] || styles.INFO;
    const icn = icon(iname);
    icn.classList.add(icls);
    const iconWrap = el("span", "pt-1");
    iconWrap.append(icn);
    const body = el("span", "min-w-0 flex-grow-1");
    const title = el("span", "d-block fw-semibold small");
    title.append(el("span", "bell-dot"), n.titulo);
    body.append(
      title,
      el("span", "d-block text-muted small text-wrap bell-message", String(n.mensaje || "")),
      el("span", "d-block text-muted small", fecha(n.creada_en, true)),
    );
    a.append(iconWrap, body);
    list.append(a);
  }
  if (!mine.length) {
    const empty = el("div", "text-center text-muted py-4 small");
    empty.append(icon("bell-slash"), " Sin notificaciones");
    empty.firstElementChild.classList.add("d-block", "mb-2");
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
  setBellMenu(false);
  setSidebar(false);
  selected = location.hash.slice(1) || "dashboard";
  // Vista de ESTA navegación; si `selected` cambia, otra route() más reciente
  // tomó el control y los resultados pendientes se descartan (FE-2).
  const view = selected;
  if (selected === "publicar") {
    location.hash = "retos";
    // Se abre después de la route() que provoca el cambio de hash, para que
    // el cierre del diálogo anterior no se lleve el formulario recién abierto.
    setTimeout(() => challengeForm(), 0);
    return;
  }
  // Navegar con el diálogo de detalle abierto lo cierra y lo limpia: el
  // registro mostrado pertenece a la vista anterior (FE-3).
  if ($("detail-dialog").open) $("detail-dialog").close();
  $("dialog-content").replaceChildren();
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
      ? simpleRole()
        ? "Inicio"
        : "Dashboard de Indicadores"
      : selected === "reportes"
        ? "Generación de Informes y Reportes"
        : navLabels[selected] || title(selected);
  $("page-heading").textContent = heading;
  $("page-title").textContent = heading;
  document.title = `${heading} — Retosean`;
  const crumbs = $("breadcrumbs");
  if (crumbs) {
    // «Inicio / Inicio» no orienta a nadie: sin migas en el inicio simplificado.
    crumbs.hidden = simpleRole() && selected === "dashboard";
    crumbs.replaceChildren();
    const home = el("li", "breadcrumbs__item");
    const homeLink = el("a", "", "Inicio");
    homeLink.href = "#dashboard";
    home.append(homeLink);
    crumbs.append(home);
    const cur = el("li", "breadcrumbs__item");
    const curSpan = el("span", "", heading);
    curSpan.setAttribute("aria-current", "page");
    cur.append(curSpan);
    crumbs.append(cur);
  }
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
    (selected === "unidades_estudio" && user.gestionaUnidades)
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
  const main = $("main-content");
  const progress = $("app-progress");
  main.setAttribute("aria-busy", "true");
  if (progress) progress.hidden = false;
  try {
    if (selected === "dashboard") await loadDashboard();
    else if (selected === "reportes") await loadBuilder();
    else await loadRecords();
  } catch (error) {
    //Errores del cargador: visibles en la vista; route() nunca rechaza (FEB-1).
    if (view === selected) showError(error);
  } finally {
    if (view === selected) {
      main.setAttribute("aria-busy", "false");
      if (progress) progress.hidden = true;
    }
  }
  if (view !== selected) return;
  // Move focus to the page heading so keyboard / screen-reader users know the
  // page changed; announce via document.title above.
  const headingEl = $("page-heading");
  headingEl.setAttribute("tabindex", "-1");
  if (typeof headingEl.focus === "function") headingEl.focus({ preventScroll: true });
}
// ── Dashboard de Indicadores (apps/dashboard/templates/dashboard/*.html) ──
