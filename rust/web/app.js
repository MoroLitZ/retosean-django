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
    ["retos", "Retos", "trophy"],
    ["integraciones_academicas", "Vinculaciones", "link-45deg"],
    ["cierres_reto", "Cierre de Retos", "flag"],
    ["reportes_generados", "Reportes", "bar-chart-line"],
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
  const body = await response.json();
  if (!response.ok) {
    if (response.status === 401 && !path.endsWith("/login")) showLogin();
    throw new Error(body.error || "No se pudo completar la operación");
  }
  return body;
}
function toast(message) {
  $("toast").textContent = message;
  $("toast").hidden = false;
  setTimeout(() => ($("toast").hidden = true), 4500);
}
function showLogin() {
  $("app-view").hidden = true;
  $("login-view").hidden = false;
  document.body.className = "auth";
  document.title = "Retosean — Universidad EAN";
  user = null;
  csrf = null;
}
function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}
function icon(name) {
  return el("i", "bi bi-" + name);
}
async function start() {
  const session = await api("/api/me");
  user = session.user;
  csrf = session.csrf;
  resources = await api("/api/resources");
  $("login-view").hidden = true;
  $("app-view").hidden = false;
  document.body.className = "role-" + role().toLowerCase();
  const name =
    [user.first_name, user.last_name].filter(Boolean).join(" ") ||
    user.username;
  $("user-avatar").textContent = (
    (user.first_name[0] || "") + (user.last_name[0] || "")
  ).toUpperCase();
  $("user-name").textContent = name;
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
    (table === "publicar" ? has("retos") : resources.some((r) => r.table === table));
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
  perfilButton.onclick = profileForm;
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
  const data = await api("/api/records/notificaciones?page=1&q=");
  const mine = data.records.filter((n) => n.usuario_id === user.id);
  const unread = mine.filter((n) => !n.leida).length;
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
      el("span", "d-block text-muted small", String(n.mensaje || "").slice(0, 82)),
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
  if (!user) return;
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
    selected !== "dashboard" &&
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
      admin())
  );
  $("new-label").textContent =
    selected === "usuarios"
      ? "Crear usuario"
      : selected === "retos"
        ? "Publicar Reto"
        : "Agregar registro";
  $("dashboard-panel").hidden = selected !== "dashboard";
  $("records-panel").hidden = selected === "dashboard";
  if (selected === "dashboard") await loadDashboard();
  else await loadRecords();
}
// ── Dashboard de Indicadores (apps/dashboard/templates/dashboard/*.html) ──
const KPIS = {
  ADMIN: [
    ["Retos totales", "retos_total", "trophy", "#2563eb"],
    ["Retos activos", "retos_activos", "play-circle", "#16a34a"],
    ["Finalizados", "retos_finalizados", "flag", "#7c3aed"],
    ["En revisión", "retos_en_revision", "hourglass-split", "#f59e0b"],
    ["Empresas", "empresas_total", "building", "#0891b2", (k) => `${k.empresas_verificadas} verificadas`],
    ["Estudiantes", "estudiantes_total", "mortarboard", "#65a30d"],
    ["Docentes", "profesores_total", "person-video3", "#db2777"],
    ["Postulaciones", "postulaciones_total", "send", "#dc2626", (k) => `${k.postulaciones_aceptadas} aceptadas`],
    ["Solución aplicada", "impacto_si", "check2-circle", "#16a34a", () => "la empresa la aplicó"],
    ["No aplicada", "impacto_no", "x-circle", "#dc2626"],
    ["Impacto en evaluación", "impacto_evaluacion", "hourglass", "#f59e0b"],
  ],
  EMPRESA: [
    ["Mis retos", "retos_total", "trophy", "#2563eb"],
    ["Activos", "retos_activos", "play-circle", "#16a34a"],
    ["En revisión", "retos_en_revision", "hourglass-split", "#f59e0b"],
    ["Finalizados", "retos_finalizados", "flag", "#7c3aed"],
    ["Postulaciones", "postulaciones_total", "send", "#0891b2"],
    ["Pendientes", "postulaciones_pendientes", "clock-history", "#f59e0b"],
    ["Aceptadas", "postulaciones_aceptadas", "person-check", "#16a34a"],
    ["Entregables", "entregables_total", "folder-check", "#db2777"],
  ],
  PROFESOR: [
    ["Cursos activos", "cursos_activos", "journal-text", "#2563eb"],
    ["Retos asignados", "retos_asignados", "trophy", "#16a34a"],
    ["Mis estudiantes", "estudiantes_total", "people", "#0891b2"],
    ["Por calificar", "evaluaciones_pendientes", "clipboard2-check", "#f59e0b"],
    ["Entregables recibidos", "entregables_total", "folder-check", "#7c3aed"],
    ["Integraciones en revisión", "integraciones_en_revision", "hourglass-split", "#dc2626"],
    ["Nota promedio", "nota_promedio", "star-half", "#65a30d"],
  ],
  ESTUDIANTE: [
    ["Retos disponibles", "retos_disponibles", "search", "#2563eb"],
    ["Mis postulaciones", "postulaciones_total", "send", "#0891b2"],
    ["Aceptadas", "postulaciones_aceptadas", "person-check", "#16a34a"],
    ["Favoritos", "favoritos_total", "star", "#f59e0b"],
    ["Entregables", "entregables_total", "folder-check", "#7c3aed"],
    ["Sin calificar", "entregables_pendientes", "hourglass-split", "#dc2626"],
    ["Nota promedio", "nota_promedio", "star-half", "#65a30d"],
  ],
  EXPERTO: [
    ["Hackathones asignados", "hackathones_asignados", "lightning-charge", "#2563eb"],
    ["En curso", "hackathones_en_curso", "play-circle", "#16a34a"],
    ["Equipos por evaluar", "equipos_por_evaluar", "hourglass-split", "#f59e0b"],
    ["Votos emitidos", "votos_emitidos", "check2-square", "#7c3aed"],
    ["Puntaje promedio", "puntaje_promedio", "star-half", "#65a30d"],
    ["Equipos academicos", "equipos_academicos", "people", "#0891b2"],
  ],
};
// [title, serie, chart type, dataset label, column class]
const GRAFICAS = {
  ADMIN: [
    ["Retos por estado", "retos_por_estado", "doughnut", "Total", "col-lg-5"],
    ["Retos por facultad", "retos_por_facultad", "bar", "Retos", "col-lg-7"],
    ["Retos creados por mes", "retos_por_mes", "line", "Retos creados", "col-lg-7"],
    ["Integraciones académicas", "integraciones_por_estado", "doughnut", "Total", "col-lg-5"],
    ["Impacto: ¿la empresa aplicó la solución?", "impacto", "doughnut", "Total", "col-lg-5"],
  ],
  EMPRESA: [
    ["Mis retos por estado", "retos_por_estado", "doughnut", "Total", "col-lg-5"],
    ["Retos publicados por mes", "retos_por_mes", "line", "Retos", "col-lg-7"],
  ],
  PROFESOR: [
    ["Entregables por estado", "entregables_por_estado", "doughnut", "Total", "col-lg-5"],
    ["Entregables por reto", "entregables_por_reto", "bar", "Entregables", "col-lg-7"],
  ],
  ESTUDIANTE: [
    ["Mis postulaciones", "postulaciones_por_estado", "doughnut", "Total", "col-lg-5"],
    ["Mis notas recientes", "notas_recientes", "bar", "Nota", "col-lg-7"],
  ],
  EXPERTO: [["Votos por hackathon", "votos_por_hackathon", "bar", "Votos", "col-lg-7"]],
};
const ACCESOS = {
  ADMIN: [
    ["usuarios", "Gestionar usuarios", "people"],
    ["retos", "Revisar retos", "trophy"],
    ["empresas", "Empresas", "building"],
    ["reportes_generados", "Ver reportes", "bar-chart"],
  ],
  EMPRESA: [
    ["retos", "Mis retos", "trophy"],
    ["postulaciones", "Postulaciones recibidas", "send"],
    ["publicar", "Publicar reto", "plus-lg"],
    ["documentos_empresas", "Mis documentos", "file-earmark-text"],
  ],
  PROFESOR: [
    ["entregables", "Calificar entregables", "clipboard2-check"],
    ["integraciones_academicas", "Mis integraciones", "diagram-3"],
    ["integraciones_academicas", "Mis cursos", "journal-text"],
    ["postulaciones", "Mis estudiantes", "people"],
  ],
  ESTUDIANTE: [
    ["retos", "Explorar retos", "search"],
    ["entregables", "Mis entregables", "cloud-arrow-up"],
    ["postulaciones", "Mis postulaciones", "send"],
    ["certificados", "Mis certificados", "award"],
  ],
  EXPERTO: [["hackathons", "Mis hackathones", "lightning-charge"]],
};
let dashboardQuery = "";
async function loadDashboard() {
  const data = await api("/api/dashboard" + dashboardQuery);
  const select = $("periodo");
  select.replaceChildren(
    ...data.periodos.map(([value, text]) => {
      const option = el("option", "", text);
      option.value = value;
      option.selected = value === data.periodo;
      return option;
    }),
  );
  document.querySelectorAll(".custom-range").forEach((n) => (n.hidden = data.periodo !== "custom"));
  $("etiqueta-periodo").textContent = data.etiqueta_periodo;
  $("sin-rol").hidden = !data.sin_rol;
  const isAdmin = data.rol === "ADMIN";
  document.querySelectorAll(".admin-filter").forEach((n) => (n.hidden = !isAdmin));
  if (isAdmin) {
    const params = new URLSearchParams(dashboardQuery);
    const fill = (id, empty, rows) => {
      const current = params.get(id) || "";
      $(id).replaceChildren(new Option(empty, ""), ...rows.map((r) => new Option(r.nombre, r.id, false, String(r.id) === current)));
    };
    fill("facultad", "Todas", data.facultades);
    fill("programa", "Todos", data.programas);
    fill("ecosistema", "Todos", data.ecosistemas);
  }
  const kpis = $("kpis");
  kpis.replaceChildren();
  for (const [text, key, iconName, color, note] of data.sin_rol ? [] : KPIS[data.rol]) {
    let value = data.kpis[key];
    if (key === "nota_promedio" || key === "puntaje_promedio")
      value = value == null ? "" : Number(value).toFixed(2);
    const col = el("div", "col-6 col-lg-3");
    const card = el("div", "card border-0 shadow-sm h-100");
    const body = el("div", "card-body d-flex align-items-center gap-3");
    const box = el("div", "kpi-icon rounded-3 d-flex align-items-center justify-content-center flex-shrink-0");
    box.style.background = color + "1a";
    const i = icon(iconName);
    i.style.color = color;
    box.append(i);
    const textBox = el("div", "min-w-0");
    textBox.append(
      el("div", "kpi-label text-muted text-uppercase fw-bold", text),
      el("div", "kpi-value fw-bold text-dark", value || "0"),
    );
    if (note) textBox.append(el("div", "kpi-note text-muted", note(data.kpis)));
    body.append(box, textBox);
    card.append(body);
    col.append(card);
    kpis.append(col);
  }
  charts.forEach((c) => c.destroy());
  charts = [];
  if (window.Chart) {
    Chart.defaults.font.family = "'Segoe UI', system-ui, sans-serif";
    Chart.defaults.color = "#6b7280";
    Chart.defaults.plugins.legend.labels.boxWidth = 12;
  }
  const graficas = $("graficas");
  graficas.replaceChildren();
  for (const [heading, serieName, tipo, etiqueta, colClass] of data.sin_rol ? [] : GRAFICAS[data.rol]) {
    const col = el("div", "col-12 " + colClass);
    const panel = el("div", "card-panel h-100");
    const box = el("div", "p-3 chart-box");
    panel.append(el("div", "card-panel-header", heading), box);
    col.append(panel);
    graficas.append(col);
    const serie = data.series[serieName];
    if (!serie || !serie.data.length || !window.Chart) {
      box.replaceChildren(el("p", "text-muted text-center small py-5 mb-0", "Sin datos en el periodo seleccionado."));
      continue;
    }
    const canvas = el("canvas");
    box.append(canvas);
    const circular = tipo === "doughnut";
    charts.push(
      new Chart(canvas, {
        type: tipo,
        data: {
          labels: serie.labels,
          datasets: [
            {
              label: etiqueta,
              data: serie.data,
              backgroundColor: circular ? serie.colors : tipo === "line" ? "rgba(37,99,235,.12)" : serie.colors,
              borderColor: tipo === "line" ? "#2563eb" : "#ffffff",
              borderWidth: tipo === "line" ? 2 : 1,
              fill: tipo === "line",
              tension: 0.3,
              pointRadius: 3,
            },
          ],
        },
        options: {
          responsive: true,
          maintainAspectRatio: false,
          plugins: { legend: { display: circular, position: "bottom" } },
          scales: circular
            ? {}
            : { y: { beginAtZero: true, ticks: { precision: 0 } }, x: { grid: { display: false } } },
        },
      }),
    );
  }
  const accesos = $("accesos");
  accesos.replaceChildren();
  for (const [target, text, iconName] of data.sin_rol ? [] : ACCESOS[data.rol]) {
    if (target !== "publicar" && !resources.some((r) => r.table === target)) continue;
    const a = el("a", "btn btn-outline-secondary btn-sm");
    a.href = "#" + target;
    a.append(icon(iconName), " " + text);
    accesos.append(a);
  }
}

function display(value) {
  if (value === null || value === undefined || value === "") return "—";
  if (typeof value === "boolean") return value ? "Sí" : "No";
  if (typeof value === "object") return JSON.stringify(value);
  if (typeof value === "string" && /^\d{4}-\d{2}-\d{2}(T|$)/.test(value))
    return fecha(value.length > 10 ? value : value + "T00:00", value.length > 10);
  return String(value);
}
// Columns and headers of the matching Django list template; "table:ROL" wins over "table".
const COLUMNS = {
  retos: [["titulo", "Titulo"], ["empresa", "Empresa"], ["estado", "Estado"], ["codigo", "Consecutivo"], ["actualizado_en", "Actualizado"]],
  "retos:EMPRESA": [["titulo", "Titulo"], ["tipo", "Tipo"], ["estado", "Estado"], ["area", "Area"], ["actualizado_en", "Actualizado"]],
  "retos:ESTUDIANTE": [["titulo", "Titulo"], ["empresa", "Empresa"], ["area", "Area"], ["estado", "Estado"], ["fecha_limite_postulacion", "Fecha límite"]],
  usuarios: [["username", "Usuario"], ["first_name", "Nombre"], ["last_name", "Apellido"], ["email", "Correo"], ["rol", "Rol"], ["date_joined", "Registro"], ["is_active", "Activo"]],
  empresas: [["razon_social", "Razón social"], ["nit", "NIT"], ["sector_industrial", "Sector"], ["estado_validacion", "Validación"], ["estado_convenio", "Convenio"]],
  postulaciones: [["estudiante", "Estudiante"], ["reto", "Reto"], ["fecha_postulacion", "Fecha"], ["estado", "Estado"]],
  "postulaciones:ESTUDIANTE": [["reto", "Reto"], ["fecha_postulacion", "Fecha de Postulación"], ["estado", "Estado"]],
  entregables: [["estudiante", "Estudiante / Equipo"], ["reto", "Reto / Proyecto"], ["fecha_entrega", "Fecha Entrega"], ["estado", "Estado"], ["nota", "Nota"]],
  "entregables:ESTUDIANTE": [["titulo", "Título"], ["reto", "Reto"], ["fecha_entrega", "Fecha entrega"], ["estado", "Estado"], ["nota", "Nota"]],
  integraciones_academicas: [["profesor", "Profesor"], ["reto", "Reto"], ["programa_academico", "Programa"], ["estado", "Estado"], ["creado_en", "Enviada"]],
  "integraciones_academicas:PROFESOR": [["reto", "Reto Vinculado"], ["programa_academico", "Programa"], ["facultad", "Facultad"], ["estado", "Estado"], ["actualizado_en", "Actualizado"]],
  "integraciones_academicas:EMPRESA": [["profesor", "Profesor"], ["reto", "Reto"], ["facultad", "Facultad"], ["programa_academico", "Programa"], ["estado", "Estado"], ["creado_en", "Enviada"]],
  cierres_reto: [["reto", "Reto"], ["cerrado_por", "Cerrado por"], ["cerrado_en", "Fecha de cierre"], ["encuesta_enviada", "Encuesta enviada"]],
  presupuestos: [["reto", "Reto"], ["monto_total", "Asignado"], ["actualizado_en", "Actualizado"]],
  gastos: [["categoria", "Categoría"], ["descripcion", "Descripción"], ["monto", "Monto"], ["registrado_por", "Registrado por"], ["creado_en", "Fecha"]],
  reportes_generados: [["usuario", "Generado por"], ["formato", "Formato"], ["enviado_por_email", "Enviado por correo"], ["creado_en", "Fecha"]],
  unidades_estudio: [["codigo", "Código"], ["nombre", "Nombre"], ["programa", "Programa"], ["periodo", "Periodo"], ["ciclo", "Ciclo"], ["activo", "Estado"]],
  log_actividad: [["creado_en", "Fecha"], ["accion", "Acción"], ["usuario", "Cuenta"], ["detalle", "Detalle"], ["realizado_por", "Realizado por"], ["ip", "IP"]],
  historial_estados_reto: [["fecha", "Fecha"], ["reto", "Reto"], ["estado_anterior", "Estado Anterior"], ["estado_nuevo", "Estado Nuevo"], ["realizado_por", "Responsable"], ["comentario", "Comentario"]],
  seguimientos_reto: [["fecha_sesion", "Fecha"], ["reto", "Reto"], ["tipo_sesion", "Sesion"], ["porcentaje_avance", "Avance"], ["observaciones", "Observaciones"], ["creado_por", "Creado por"]],
  documentos_empresas: [["empresa", "Empresa"], ["tipo_documento", "Documento"], ["estado", "Estado"], ["fecha_expedicion", "Expedición"]],
  "documentos_empresas:EMPRESA": [["tipo_documento", "Documento"], ["estado", "Estado"], ["fecha_expedicion", "Expedición"]],
  notificaciones: [["titulo", "Título"], ["tipo", "Tipo"], ["leida", "Leída"], ["creada_en", "Fecha"]],
  certificados: [["reto", "Reto"], ["usuario", "Participante"], ["rol_participacion", "Rol"], ["horas", "Horas"], ["emitido_en", "Emitido"]],
  retos_favoritos: [["reto", "Reto"], ["fecha", "Agregado"]],
  equipos_reto_academico: [["nombre_equipo", "Equipo"], ["reto", "Reto"]],
  hackathons: [["reto", "Reto"], ["estado", "Estado"], ["resultados_publicados", "Resultados publicados"], ["creado_en", "Creado"]],
  cierre_encuestasatisfaccion: [["participante", "Participante"], ["calificacion", "Calificación"], ["respondida_en", "Respondida"]],
  evaluaciones: [["entregable", "Entregable"], ["profesor", "Profesor"], ["nota", "Nota"], ["fecha", "Fecha"]],
  participantes_integracion: [["integracion", "Integración"], ["rol", "Rol"], ["nombre", "Nombre"], ["documento", "Cédula / documento"], ["email", "Correo"]],
  sesiones_reto_academico: [["reto", "Reto"], ["tipo", "Tipo"], ["fecha_hora", "Fecha y hora"], ["enlace_reunion", "Enlace"]],
  invitados_hackaton: [["hackathon", "Hackathon"], ["tipo", "Tipo"], ["nombre", "Nombre"], ["organizacion", "Organización"], ["confirmado", "Confirmado"]],
  tipos_documento_legal: [["orden", "Orden"], ["nombre", "Documento"], ["obligatorio", "Obligatorio"], ["vigencia_dias", "Vigencia (días)"], ["activo", "Activo"]],
  cierre_solicitudlogistica: [["reto", "Reto"], ["evento", "Evento"], ["tipo", "Tipo"], ["estado", "Estado"], ["creada_en", "Solicitada"]],
};
let columnHeaders = {};
function columns(records) {
  const spec = COLUMNS[`${selected}:${role()}`] || COLUMNS[selected];
  if (spec) {
    columnHeaders = Object.fromEntries(spec);
    return spec.map(([key]) => key);
  }
  columnHeaders = {};
  const keys = Object.keys(records[0] || {}).filter(
    // Show the related name rather than its raw foreign key.
    (k) => !(k.endsWith("_id") && records[0][k.slice(0, -3)] !== undefined),
  );
  const preferred = [
    "id",
    "titulo",
    "razon_social",
    "username",
    "nombre",
    "estado",
    "estado_validacion",
    "rol",
    "monto_total",
    "monto",
    "usuario_id",
    "reto_id",
    "creado_en",
    "fecha_postulacion",
  ];
  return [
    ...preferred.filter((k) => keys.includes(k)),
    ...keys.filter(
      (k) =>
        !preferred.includes(k) &&
        ![
          "descripcion",
          "mensaje",
          "password",
          "archivo",
          "comentarios_revision",
        ].includes(k),
    ),
  ].slice(0, 6);
}
async function loadRecords() {
  const data = await api(
    `/api/records/${selected}?page=${page}&q=${encodeURIComponent(query)}`,
  );
  $("extra-panel").replaceChildren();
  if (selected === "equipos_reto_academico" && user.rol === "ESTUDIANTE")
    scheduleForStudent().catch(showError);
  total = data.total;
  $("record-count").textContent = `${total} registro${total === 1 ? "" : "s"}`;
  // Exports are an admin feature in Django (apps/reportes).
  $("exports").classList.toggle("d-none", !admin());
  $("export-link").href =
    `/api/export/${selected}?q=${encodeURIComponent(query)}`;
  $("export-link").download = "retosean.csv";
  $("export-pdf").href =
    `/api/export/${selected}/pdf?q=${encodeURIComponent(query)}`;
  $("export-xlsx").href =
    `/api/export/${selected}/xlsx?q=${encodeURIComponent(query)}`;
  const cols = columns(data.records);
  const head = document.createElement("tr");
  for (const key of [...cols, "Acciones"]) {
    const th = document.createElement("th");
    th.textContent =
      columnHeaders[key] ||
      capfirst(
        field(selected, key)?.verbose_name ||
          field(selected, key + "_id")?.verbose_name ||
          key,
      );
    head.append(th);
  }
  $("table-head").replaceChildren(head);
  $("table-body").replaceChildren();
  if (!data.records.length) {
    const tr = document.createElement("tr");
    const td = document.createElement("td");
    td.colSpan = 7;
    td.className = "text-center text-muted py-5";
    td.textContent = "No hay registros para mostrar.";
    tr.append(td);
    $("table-body").append(tr);
  }
  for (const record of data.records) {
    const tr = document.createElement("tr");
    for (const key of cols) {
      const td = document.createElement("td");
      const text = display(record[key]);
      td.title = text;
      if (key.includes("estado") && record[key]) {
        const badge = el("span", "badge " + badgeClass(record[key]));
        badge.textContent = choice(selected, key, record[key]);
        td.append(badge);
      } else
        td.textContent = field(selected, key)?.choices.length
          ? choice(selected, key, record[key])
          : text;
      tr.append(td);
    }
    const td = document.createElement("td");
    const button = el("button", "btn btn-sm btn-outline-secondary");
    button.type = "button";
    button.append(icon("eye"), " Ver detalle");
    button.onclick = () => openRecord(record.id).catch(showError);
    td.append(button);
    tr.append(td);
    $("table-body").append(tr);
  }
  $("page-number").textContent =
    `Página ${page} de ${Math.max(1, Math.ceil(total / 20))}`;
  $("previous").disabled = page <= 1;
  $("next").disabled = page * 20 >= total;
}
function showError(error) {
  $("app-error").textContent = error.message;
  $("app-error").hidden = false;
  toast(error.message);
}
function modal(title) {
  $("dialog-title").textContent = title;
  $("dialog-content").replaceChildren();
  if (!$("detail-dialog").open) $("detail-dialog").showModal();
  return $("dialog-content");
}
function action(parent, label, handler, primary = false) {
  const button = document.createElement("button");
  button.type = "button";
  button.className = primary ? "primary" : "secondary";
  button.textContent = label;
  button.onclick = async () => {
    button.disabled = true;
    try {
      await handler();
    } catch (e) {
      toast(e.message);
    } finally {
      button.disabled = false;
    }
  };
  parent.append(button);
  return button;
}
function input(
  parent,
  name,
  label,
  value = "",
  type = "text",
  required = false,
) {
  const wrapper = document.createElement("div");
  if (type === "textarea") wrapper.className = "full";
  const l = document.createElement("label");
  l.htmlFor = "field-" + name;
  l.textContent = label;
  const field = document.createElement(
    type === "textarea" ? "textarea" : "input",
  );
  field.id = "field-" + name;
  field.name = name;
  if (type !== "textarea") field.type = type;
  field.value = value ?? "";
  field.required = required;
  wrapper.append(l, field);
  parent.append(wrapper);
  return field;
}
async function mutate(path, data) {
  await api(path, data);
  $("detail-dialog").close();
  toast("Cambios guardados");
  if (selected === "dashboard") await route();
  else await loadRecords();
}
async function openRecord(id) {
  const record = await api(`/api/records/${selected}/${id}`);
  const parent = modal(
    record.titulo ||
      record.razon_social ||
      record.nombre ||
      (record.reto && `${capfirst(meta(selected)?.verbose_name || selected)}: ${record.reto}`) ||
      `${capfirst(meta(selected)?.verbose_name || selected)} #${id}`,
  );
  const dl = document.createElement("dl");
  dl.className = "details";
  // Model field order, related names instead of raw foreign keys, codigo instead of its parts.
  const keys = (meta(selected)?.fields || [])
    .map((f) => (f.related_table && f.name.slice(0, -3) in record ? f.name.slice(0, -3) : f.name))
    .flatMap((k) => (selected === "retos" && k === "consecutivo" ? ["codigo"] : [k]))
    .filter((k) => k in record && !["id", "eliminado_en", "anio_consecutivo"].includes(k));
  for (const key of keys) {
    const value = record[key];
    const dt = document.createElement("dt");
    dt.textContent =
      key === "codigo" && selected === "retos"
        ? "Consecutivo"
        : capfirst(
            field(selected, key)?.verbose_name ||
              field(selected, key + "_id")?.verbose_name ||
              key,
          );
    const dd = document.createElement("dd");
    dd.textContent = field(selected, key)?.choices.length
      ? choice(selected, key, value)
      : display(value);
    if (value && field(selected, key)?.type === "FileField") {
      const a = document.createElement("a");
      a.className = "secondary";
      a.textContent = "Descargar archivo";
      a.href = `/api/archivo/${selected}/${id}/${key}`;
      dd.replaceChildren(a);
    }
    dl.append(dt, dd);
  }
  parent.append(dl);
  const actions = document.createElement("div");
  actions.className = "actions";
  parent.append(actions);
  if (selected === "retos") {
    if (
      user.rol === "EMPRESA" &&
      ["borrador", "rechazado"].includes(record.estado)
    )
      action(actions, "Editar reto", () => challengeForm(record));
    if (user.rol === "EMPRESA" && record.estado === "borrador")
      action(
        actions,
        "Enviar a revisión",
        () => changeState(id, "en_revision"),
        true,
      );
    if (admin()) {
      if (record.estado === "en_revision") {
        action(actions, "Aprobar", () => changeState(id, "aprobado"), true);
        action(actions, "Rechazar", () => stateForm(id, "rechazado"));
      }
      if (["abierto", "pausado"].includes(record.estado))
        action(actions, "Iniciar / reanudar", () =>
          changeState(id, "en_curso"),
        );
      if (record.estado === "en_curso")
        action(actions, "Pausar", () => changeState(id, "pausado"));
      if (!["finalizado", "cancelado"].includes(record.estado))
        action(actions, "Cancelar reto", () => stateForm(id, "cancelado"));
    }
    if (user.rol === "ESTUDIANTE") {
      action(actions, "Alternar favorito", () =>
        mutate(`/api/retos/${id}/favorito`, {}),
      );
      if (record.estado === "abierto")
        action(actions, "Postularme", () => applicationForm(id), true);
      if (["abierto", "en_curso"].includes(record.estado))
        action(actions, "Entregar archivo", () => deliveryForm(id));
    }
    if (
      (admin() || user.rol === "PROFESOR") &&
      ["aprobado", "abierto", "en_curso", "pausado"].includes(record.estado)
    )
      action(actions, "Crear equipo", async () => {
        const options = await api(`/api/retos/${id}/opciones`);
        simpleForm(
          "Equipo académico",
          `/api/retos/${id}/equipo`,
          [
            ["nombre", "Nombre del equipo"],
            [
              "estudiantes",
              "Estudiantes aceptados (selecciona uno o varios)",
              "multiselect",
              options.students,
            ],
          ],
          (data) => ({ ...data, estudiantes: data.estudiantes.map(Number) }),
        );
      });
    if (admin() && record.tipo === "hackathon")
      action(actions, "Crear hackathon", () =>
        simpleForm("Configurar hackathon", `/api/retos/${id}/hackathon`, [
          ["reglas", "Reglas", "textarea"],
        ]),
      );
    if (
      user.rol === "PROFESOR" &&
      ["aprobado", "abierto", "en_curso", "pausado"].includes(record.estado)
    )
      action(actions, "Integrar a mi curso", () => integrationForm(id));
    if (
      (admin() || user.rol === "PROFESOR") &&
      ["abierto", "en_curso", "pausado"].includes(record.estado)
    )
      action(actions, "Registrar seguimiento", () =>
        simpleForm(
          "Seguimiento",
          `/api/retos/${id}/seguimiento`,
          [
            [
              "fecha_sesion",
              "Fecha",
              "date",
              new Date().toLocaleDateString("en-CA", {
                timeZone: "America/Bogota",
              }),
            ],
            ["porcentaje_avance", "Avance (%)", "number", "0"],
            ["avances", "Avances", "textarea"],
            ["observaciones", "Observaciones", "textarea"],
            ["acuerdos", "Acuerdos", "textarea"],
          ],
          (data) => ({
            ...data,
            porcentaje_avance: Number(data.porcentaje_avance),
          }),
        ),
      );
    if (admin() && !["borrador", "cancelado"].includes(record.estado))
      action(actions, "Definir presupuesto", () =>
        simpleForm("Presupuesto", `/api/retos/${id}/presupuesto`, [
          ["monto_total", "Monto total (COP)", "number", "0.00"],
        ]),
      );
    if ((admin() || user.rol === "PROFESOR") && record.estado === "en_curso")
      action(
        actions,
        "Cerrar y emitir certificados",
        () =>
          simpleForm(
            "Cierre formal",
            `/api/retos/${id}/cerrar`,
            [
              ["notas", "Notas del cierre", "textarea"],
              ["horas", "Horas certificadas", "number", "40"],
            ],
            (data) => ({ ...data, horas: Number(data.horas) }),
          ),
        true,
      );
  }
  if (
    selected === "notificaciones" &&
    record.usuario_id === user.id &&
    !record.leida
  )
    action(
      actions,
      "Marcar como leída",
      () => mutate(`/api/notificaciones/${id}/leer`, {}),
      true,
    );
  if (selected === "postulaciones" && record.estado === "PENDIENTE") {
    if (user.rol === "ESTUDIANTE")
      action(actions, "Retirar postulación", () =>
        mutate(`/api/postulaciones/${id}/estado`, { estado: "RETIRADA" }),
      );
    if (admin() || user.rol === "PROFESOR" || user.rol === "EMPRESA") {
      action(
        actions,
        "Aceptar",
        () => mutate(`/api/postulaciones/${id}/estado`, { estado: "ACEPTADA" }),
        true,
      );
      action(actions, "Rechazar", () =>
        mutate(`/api/postulaciones/${id}/estado`, { estado: "RECHAZADA" }),
      );
    }
  }
  if (
    selected === "usuarios" &&
    admin() &&
    record.id !== user.id &&
    record.rol !== "ADMIN"
  )
    action(
      actions,
      record.activo ? "Desactivar usuario" : "Activar usuario",
      () => mutate(`/api/usuarios/${id}/estado`, { activo: !record.activo }),
    );
  if (selected === "certificados")
    action(actions, "Verificar certificado", async () => {
      const data = await api(`/api/verificar/${record.codigo_verificacion}`);
      toast(`Certificado válido: ${data.participante} · ${data.reto}`);
    });
  if (selected === "certificados") {
    const a = document.createElement("a");
    a.className = "primary";
    a.textContent = "Descargar certificado PDF";
    a.href = `/api/certificados/${id}/pdf`;
    actions.append(a);
  }
  if (selected === "integraciones_academicas") {
    const team = await api(`/api/integraciones/${id}/equipo`);
    parent.insertBefore(teamTables(team), actions);
    const editable =
      ["borrador", "rechazada"].includes(record.estado) ||
      ["abierto", "en_curso"].includes(record.reto_estado);
    if (user.rol === "PROFESOR" && record.profesor_id === user.id && editable)
      action(actions, "Editar equipo y cronograma", () => teamForm(id, team));
    if (user.rol === "PROFESOR" && record.profesor_id === user.id) {
      if (record.estado === "borrador")
        action(
          actions,
          "Enviar a revisión",
          () =>
            mutate(`/api/integraciones/${id}/estado`, {
              estado: "en_revision",
            }),
          true,
        );
      if (record.estado === "aprobada")
        action(
          actions,
          "Publicar integración",
          () =>
            mutate(`/api/integraciones/${id}/estado`, { estado: "publicada" }),
          true,
        );
      if (record.estado === "rechazada")
        action(actions, "Volver a borrador", () =>
          mutate(`/api/integraciones/${id}/estado`, { estado: "borrador" }),
        );
    }
    if (admin() && record.estado === "en_revision") {
      action(
        actions,
        "Aprobar integración",
        () => mutate(`/api/integraciones/${id}/estado`, { estado: "aprobada" }),
        true,
      );
      action(actions, "Rechazar integración", () =>
        simpleForm(
          "Rechazar integración",
          `/api/integraciones/${id}/estado`,
          [["comentario", "Motivo", "textarea"]],
          (data) => ({ ...data, estado: "rechazada" }),
        ),
      );
    }
  }
  if (selected === "presupuestos") {
    const summary = await api(`/api/presupuestos/${id}/resumen`);
    const p = document.createElement("p");
    p.textContent = `Ejecutado: $${summary.ejecutado} · Disponible: $${summary.disponible} · ${summary.porcentaje}%${summary.alerta ? " · Umbral de alerta alcanzado" : ""}`;
    parent.prepend(p);
    if (admin())
      action(
        actions,
        "Registrar gasto",
        () =>
          simpleForm("Nuevo gasto", `/api/presupuestos/${id}/gastos`, [
            [
              "categoria",
              "Categoría",
              "select",
              ["REFRIGERIOS", "MATERIALES", "IMPRESIONES", "ESPACIOS", "OTROS"],
            ],
            ["monto", "Monto (COP)", "number", "0.00"],
            ["descripcion", "Descripción", "textarea"],
          ]),
        true,
      );
  }
  if (selected === "entregables" && user.rol === "PROFESOR")
    action(
      actions,
      "Calificar",
      () =>
        simpleForm("Evaluar entregable", `/api/entregables/${id}/calificar`, [
          ["nota", `Nota (máximo ${record.puntaje_maximo})`, "number"],
          ["comentario", "Retroalimentación", "textarea"],
        ]),
      true,
    );
  if (
    selected === "cierre_encuestasatisfaccion" &&
    !record.respondida_en &&
    record.participante_id === user.id
  )
    action(
      actions,
      "Responder encuesta",
      () =>
        simpleForm(
          "Tu experiencia",
          `/api/encuestas/${id}`,
          [
            [
              "calificacion",
              "Calificación de 1 a 5",
              "select",
              ["5", "4", "3", "2", "1"],
            ],
            ["comentario", "Comentario", "textarea"],
          ],
          (data) => ({ ...data, calificacion: Number(data.calificacion) }),
        ),
      true,
    );
  if (selected === "preferencias_notificacion" && record.usuario_id === user.id)
    action(actions, "Editar preferencias", () =>
      simpleForm(
        "Preferencias",
        `/api/preferencias`,
        [
          ["recibir_email", "Recibir correo", "select", ["Sí", "No"]],
          [
            "recibir_recordatorios",
            "Recibir recordatorios",
            "select",
            ["Sí", "No"],
          ],
          [
            "eventos_silenciados",
            "Eventos silenciados (separados por coma)",
            "text",
            (record.eventos_silenciados || []).join(","),
          ],
        ],
        (data) => ({
          recibir_email: data.recibir_email === "Sí",
          recibir_recordatorios: data.recibir_recordatorios === "Sí",
          eventos_silenciados: data.eventos_silenciados
            .split(",")
            .map((s) => s.trim())
            .filter(Boolean),
        }),
      ),
    );
  if (selected === "empresas" && admin()) {
    action(actions, "Revisar empresa", () =>
      simpleForm("Revisión de empresa", `/api/empresas/${id}/revisar`, [
        [
          "estado_validacion",
          "Validación",
          "select",
          ["PENDIENTE", "EN_REVISION", "VERIFICADA", "RECHAZADA"],
        ],
        [
          "estado_listas_restrictivas",
          "Listas restrictivas",
          "select",
          ["PENDIENTE", "APROBADO", "RECHAZADO"],
        ],
        ["observaciones", "Observaciones", "textarea"],
      ]),
    );
    action(actions, "Tramitar convenio", () =>
      simpleForm(
        "Convenio",
        `/api/empresas/${id}/convenio`,
        [
          [
            "estado",
            "Nuevo estado",
            "select",
            ["RADICADO", "EN_JURIDICA", "FIRMADO"],
          ],
          ["fecha_vencimiento", "Fecha de vencimiento", "date"],
          ["comentario", "Comentario", "textarea"],
        ],
        (data) => ({
          ...data,
          fecha_vencimiento: data.fecha_vencimiento || null,
        }),
      ),
    );
  }
  if (selected === "documentos_empresas" && admin())
    action(actions, "Revisar documento", () =>
      simpleForm("Documento legal", `/api/documentos/${id}/revisar`, [
        ["estado", "Decisión", "select", ["VERIFICADO", "RECHAZADO"]],
        ["comentario", "Motivo / comentario", "textarea"],
      ]),
    );
  if (selected === "documentos_empresas" && user.rol === "EMPRESA")
    action(actions, "Cargar documento", () => documentForm(id));
  if (selected === "hackathons") {
    // HU09: logistics, guests, automatic close and the final report.
    const event = await api(`/api/hackathons/${id}/logistica`);
    const info = el("div", "mb-3");
    info.append(
      el("h6", "fw-bold mt-2", "Logística del evento"),
      el("p", "small mb-1", `Modalidad: ${label(MODALIDADES, event.modalidad)} · Espacio: ${event.lugar || "Por definir"}`),
    );
    if (event.enlace_transmision) {
      const link = el("a", "small", event.enlace_transmision);
      link.href = event.enlace_transmision;
      link.target = "_blank";
      link.rel = "noopener";
      info.append(link);
    }
    if (event.cierre && !record.resultados_publicados)
      info.append(el("p", "small text-muted mt-2 mb-1", `Los resultados se publican automáticamente el ${fecha(event.cierre, true)}.`));
    if (event.invitados.length)
      info.append(table(["Invitado", "Tipo", "Organización", "Confirmado"],
        event.invitados.map((g) => [g.nombre, label(TIPOS_INVITADO, g.tipo), g.organizacion, g.confirmado ? "Sí" : "Pendiente"])));
    parent.insertBefore(info, actions);
    if (event.puede_gestionar && !["finalizado", "cancelado"].includes(record.estado))
      action(actions, "Logística e invitados", () => logisticsForm(id, event));
    if (event.puede_exportar)
      for (const [format, text] of [["pdf", "Informe final PDF"], ["xlsx", "Informe final Excel"]]) {
        const a = el("a", "secondary", text);
        a.href = `/api/hackathons/${id}/informe/${format}`;
        actions.append(a);
      }
    if (admin() || user.rol === "PROFESOR") {
      const targets =
        {
          borrador: ["publicado", "cancelado"],
          publicado: ["en_curso", "cancelado"],
          en_curso: ["finalizado", "cancelado"],
        }[record.estado] || [];
      for (const estado of targets)
        action(actions, choice("hackathons", "estado", estado), () =>
          mutate(`/api/hackathons/${id}/estado`, { estado }),
        );
      if (!["finalizado", "cancelado"].includes(record.estado)) {
        action(actions, "Asignar jurado", async () => {
          const options = await api(`/api/hackathons/${id}/opciones`);
          simpleForm(
            "Asignar jurado",
            `/api/hackathons/${id}/jurado`,
            [
              ["usuario_id", "Experto o docente", "select", options.jurors],
              ["especialidad", "Especialidad"],
            ],
            (data) => ({ ...data, usuario_id: Number(data.usuario_id) }),
          );
        });
        action(actions, "Configurar etapa", () =>
          simpleForm(
            "Etapa del hackathon",
            `/api/hackathons/${id}/etapa`,
            [
              [
                "tipo",
                "Tipo",
                "select",
                ["inscripcion", "desarrollo", "presentacion", "premiacion"],
              ],
              ["titulo", "Título"],
              ["inicia_en", "Inicio", "datetime-local"],
              ["termina_en", "Fin", "datetime-local"],
              ["orden", "Orden", "number", "0"],
            ],
            (data) => ({
              ...data,
              orden: Number(data.orden),
              inicia_en: data.inicia_en
                ? new Date(data.inicia_en).toISOString()
                : null,
              termina_en: data.termina_en
                ? new Date(data.termina_en).toISOString()
                : null,
            }),
          ),
        );
      }
    }
    if (
      user.rol === "ESTUDIANTE" &&
      ["publicado", "en_curso"].includes(record.estado)
    )
      action(actions, "Inscribir mi equipo", async () => {
        const options = await api(`/api/hackathons/${id}/opciones`);
        simpleForm(
          "Inscripción",
          `/api/hackathons/${id}/inscribir`,
          [["equipo_id", "Mi equipo", "select", options.teams]],
          (data) => ({ equipo_id: Number(data.equipo_id) }),
        );
      });
    if (
      ["EXPERTO", "PROFESOR"].includes(user.rol) &&
      ["publicado", "en_curso"].includes(record.estado)
    )
      action(actions, "Votar como jurado", async () => {
        const options = await api(`/api/hackathons/${id}/opciones`);
        simpleForm(
          "Votación",
          `/api/hackathons/${id}/votar`,
          [
            ["equipo_id", "Equipo", "select", options.teams],
            ["puntaje", "Puntaje de 0 a 5", "number"],
            ["comentario", "Comentario", "textarea"],
          ],
          (data) => ({ ...data, equipo_id: Number(data.equipo_id) }),
        );
      });
    if (record.resultados_publicados || admin())
      action(actions, "Ver ranking", async () => {
        const data = await api(`/api/hackathons/${id}/ranking`);
        const content = modal("Ranking del hackathon");
        const ol = document.createElement("ol");
        for (const row of data.ranking) {
          const li = document.createElement("li");
          li.textContent = `${row.equipo} · ${row.promedio} puntos · ${row.votos} votos`;
          ol.append(li);
        }
        if (!data.ranking.length) ol.textContent = "Todavía no hay votos.";
        content.append(ol);
      });
  }
  if (
    selected === "inscripciones_hackaton" &&
    (admin() || user.rol === "PROFESOR")
  )
    for (const estado of ["ACEPTADA", "RECHAZADA"])
      if (record.estado !== estado)
        action(actions, choice("inscripciones_hackaton", "estado", estado), () =>
          mutate(`/api/inscripciones/${id}/estado`, { estado }),
        );
}
// ── HU03: structured team and dated schedule (Django ParticipanteIntegracion) ──
const ROLES_EQUIPO = [["PROFESOR", "Profesor"], ["ESTUDIANTE", "Estudiante"], ["EXPERTO", "Experto invitado"]];
const TIPOS_SESION = [["inicio", "Inicio"], ["seguimiento", "Seguimiento"], ["preseleccion", "Preselección"], ["evaluacion", "Evaluación"]];
function cell(tag, text) {
  return el(tag, "", text);
}
function table(headers, rows) {
  const t = el("table", "table table-sm align-middle mb-2");
  const head = el("tr");
  headers.forEach((h) => head.append(cell("th", h)));
  const thead = el("thead", "table-light");
  thead.append(head);
  const body = el("tbody");
  rows.forEach((r) => {
    const tr = el("tr");
    r.forEach((v) => tr.append(cell("td", v === "" || v == null ? "-" : String(v))));
    body.append(tr);
  });
  if (!rows.length) {
    const tr = el("tr");
    const td = cell("td", "Sin registros.");
    td.colSpan = headers.length;
    td.className = "text-muted text-center";
    tr.append(td);
    body.append(tr);
  }
  t.append(thead, body);
  const wrap = el("div", "table-responsive");
  wrap.append(t);
  return wrap;
}
function label(list, value) {
  return (list.find((x) => x[0] === value) || [value, value])[1];
}
function teamTables(team) {
  const box = el("div", "mb-3");
  box.append(
    el("h6", "fw-bold mt-2", "Equipo"),
    table(["Rol", "Nombre", "Cédula / documento", "Correo", "Facultad / programa"],
      team.participantes.map((p) => [label(ROLES_EQUIPO, p.rol), p.nombre + (p.cuenta ? " ✓" : ""), p.documento, p.email,
        p.rol === "ESTUDIANTE" ? p.programa : p.facultad])),
    el("h6", "fw-bold mt-3", "Cronograma de sesiones"),
    table(["Tipo", "Fecha y hora", "Enlace", "Observaciones"],
      team.sesiones.map((s) => [label(TIPOS_SESION, s.tipo), fecha(s.fecha_hora, true), s.enlace_reunion, s.observaciones])),
  );
  return box;
}
function editableRows(parent, title, columns, rows) {
  parent.append(el("h6", "fw-bold mt-3 mb-1", title));
  const t = el("table", "table table-sm align-middle mb-1");
  const head = el("tr");
  [...columns.map((c) => c.label), "Quitar"].forEach((h) => head.append(cell("th", h)));
  const thead = el("thead", "table-light");
  thead.append(head);
  const body = el("tbody");
  t.append(thead, body);
  const addRow = (values = {}) => {
    const tr = el("tr");
    for (const c of columns) {
      const td = el("td");
      let field;
      if (c.options) {
        field = el("select", "form-select form-select-sm");
        c.options.forEach(([v, l]) => field.append(new Option(l, v)));
      } else {
        field = el("input", "form-control form-control-sm");
        field.type = c.type || "text";
      }
      field.name = c.name;
      field.setAttribute("aria-label", c.label);
      let value = values[c.name] ?? "";
      if (c.type === "datetime-local" && value) value = new Date(value).toLocaleString("sv-SE").slice(0, 16).replace(" ", "T");
      field.value = value;
      td.append(field);
      tr.append(td);
    }
    const remove = el("button", "btn btn-sm btn-outline-danger", "✕");
    remove.type = "button";
    remove.setAttribute("aria-label", "Quitar fila");
    remove.onclick = () => tr.remove();
    const td = el("td");
    td.append(remove);
    tr.append(td);
    body.append(tr);
  };
  rows.forEach(addRow);
  const wrap = el("div", "table-responsive");
  wrap.append(t);
  const add = el("button", "secondary", "+ Agregar fila");
  add.type = "button";
  add.onclick = () => addRow();
  parent.append(wrap, add);
  return () =>
    [...body.querySelectorAll("tr")].map((tr) =>
      Object.fromEntries([...tr.querySelectorAll("input,select")].map((f) => [f.name, f.value.trim()])),
    );
}
function teamEditor(parent, team = { participantes: [], sesiones: [] }) {
  const participants = editableRows(parent, "Equipo (al menos un profesor)", [
    { name: "rol", label: "Rol", options: ROLES_EQUIPO },
    { name: "nombre", label: "Nombre" },
    { name: "documento", label: "Cédula / documento" },
    { name: "email", label: "Correo", type: "email" },
    { name: "facultad", label: "Facultad u organización" },
    { name: "programa", label: "Programa" },
  ], team.participantes.length ? team.participantes : [{ rol: "PROFESOR" }]);
  const sessions = editableRows(parent, "Cronograma (inicio, seguimiento y evaluación)", [
    { name: "tipo", label: "Tipo", options: TIPOS_SESION },
    { name: "fecha_hora", label: "Fecha y hora", type: "datetime-local" },
    { name: "enlace_reunion", label: "Enlace", type: "url" },
    { name: "observaciones", label: "Observaciones" },
  ], team.sesiones.length ? team.sesiones : [{ tipo: "inicio" }, { tipo: "seguimiento" }, { tipo: "evaluacion" }]);
  return () => ({
    participantes: participants(),
    sesiones: sessions()
      .filter((s) => s.fecha_hora)
      .map((s) => ({ ...s, fecha_hora: new Date(s.fecha_hora).toISOString() })),
  });
}
function submitForm(form, button, path, collect, done) {
  form.onsubmit = async (e) => {
    e.preventDefault();
    button.disabled = true;
    try {
      await mutate(path, collect());
      if (done) done();
    } catch (error) {
      toast(error.message);
    } finally {
      button.disabled = false;
    }
  };
}
function integrationForm(retoId) {
  const parent = modal("Integración académica");
  const form = el("form");
  const grid = el("div", "form-grid");
  input(grid, "facultad", "Facultad", "", "text", true);
  input(grid, "nivel_formacion", "Nivel de formación", "Pregrado", "text", true);
  input(grid, "programa_academico", "Programa", "", "text", true);
  input(grid, "ecosistema", "Ecosistema");
  input(grid, "descripcion", "Descripción", "", "textarea", true);
  input(grid, "alcance", "Alcance", "", "textarea", true);
  input(grid, "entregable_esperado", "Entregable esperado", "", "text", true);
  input(grid, "cronograma_sesiones", "Notas del cronograma", "", "textarea");
  form.append(grid);
  const collectTeam = teamEditor(form);
  const button = el("button", "primary", "Guardar borrador");
  form.append(el("div", "mt-3"), button);
  submitForm(form, button, `/api/retos/${retoId}/integrar`, () => ({
    ...Object.fromEntries([...grid.querySelectorAll("input,textarea")].map((f) => [f.name, f.value])),
    ...collectTeam(),
  }));
  parent.append(form);
}
function teamForm(integrationId, team) {
  const parent = modal("Equipo y cronograma");
  const form = el("form");
  const collectTeam = teamEditor(form, team);
  const button = el("button", "primary", "Guardar equipo y cronograma");
  form.append(el("div", "mt-3"), button);
  submitForm(form, button, `/api/integraciones/${integrationId}/equipo`, collectTeam);
  parent.append(form);
}
async function scheduleForStudent() {
  const data = await api("/api/records/sesiones_reto_academico?page=1&q=&limit=100");
  const panel = el("div", "card-panel mt-4");
  panel.append(el("div", "card-panel-header", "Cronograma de sesiones"));
  const rows = data.records
    .sort((a, b) => String(a.fecha_hora).localeCompare(String(b.fecha_hora)))
    .map((s) => [s.reto, label(TIPOS_SESION, s.tipo), fecha(s.fecha_hora, true), s.enlace_reunion || ""]);
  const box = el("div", "p-3");
  box.append(table(["Reto", "Sesión", "Fecha y hora", "Enlace"], rows));
  panel.append(box);
  $("extra-panel").append(panel);
}

// ── HU09: event logistics (Django LogisticaForm + InvitadoFormSet) ──
const MODALIDADES = [["presencial", "Presencial"], ["virtual", "Virtual"], ["hibrida", "Híbrida"]];
const TIPOS_INVITADO = [["INVITADO", "Invitado"], ["MENTOR", "Mentor"], ["PONENTE", "Ponente"], ["PATROCINADOR", "Patrocinador"]];
function logisticsForm(id, event) {
  const parent = modal("Logística e invitados");
  const form = el("form");
  const grid = el("div", "form-grid");
  const wrap = el("div");
  const l = el("label", "", "Modalidad");
  l.htmlFor = "field-modalidad";
  const select = el("select");
  select.id = "field-modalidad";
  select.name = "modalidad";
  MODALIDADES.forEach(([v, t]) => select.append(new Option(t, v, false, v === event.modalidad)));
  wrap.append(l, select);
  grid.append(wrap);
  input(grid, "lugar", "Espacio físico", event.lugar);
  input(grid, "enlace_transmision", "Enlace de transmisión", event.enlace_transmision, "url");
  input(grid, "notas_logistica", "Notas de logística", event.notas_logistica, "textarea");
  form.append(grid);
  const guests = editableRows(form, "Invitados", [
    { name: "tipo", label: "Tipo", options: TIPOS_INVITADO },
    { name: "nombre", label: "Nombre" },
    { name: "organizacion", label: "Organización" },
    { name: "email", label: "Correo", type: "email" },
    { name: "confirmado", label: "Confirmado", options: [["false", "Pendiente"], ["true", "Confirmado"]] },
  ], event.invitados.map((g) => ({ ...g, confirmado: String(g.confirmado) })));
  const button = el("button", "primary", "Guardar logística");
  form.append(el("div", "mt-3"), button);
  submitForm(form, button, `/api/hackathons/${id}/logistica`, () => ({
    ...Object.fromEntries([...grid.querySelectorAll("input,textarea,select")].map((f) => [f.name, f.value])),
    invitados: guests().filter((g) => g.nombre).map((g) => ({ ...g, confirmado: g.confirmado === "true" })),
  }));
  parent.append(form);
}

function simpleForm(title, path, fields, transform = (data) => data) {
  const parent = modal(title),
    form = document.createElement("form");
  form.className = "form-grid";
  for (const [name, label, type = "text", value = ""] of fields) {
    if (type === "select" || type === "multiselect") {
      const wrap = document.createElement("div"),
        l = document.createElement("label");
      l.htmlFor = "field-" + name;
      l.textContent = label;
      const select = document.createElement("select");
      select.id = "field-" + name;
      select.name = name;
      select.required = true;
      select.multiple = type === "multiselect";
      for (const v of value) {
        const option = document.createElement("option");
        option.value = typeof v === "object" ? v.value : v;
        option.textContent = typeof v === "object" ? v.label : choiceLabel(v);
        select.append(option);
      }
      wrap.append(l, select);
      form.append(wrap);
    } else {
      const field = input(
        form,
        name,
        label,
        value,
        type,
        ![
          "comentario",
          "observaciones",
          "acuerdos",
          "eventos_silenciados",
          "fecha_vencimiento",
          "nit",
          "razon_social",
          "telefono",
        ].includes(name),
      );
      if (type === "number") {
        field.step = "0.01";
        field.min = "0";
      }
    }
  }
  const button = document.createElement("button");
  button.className = "primary full";
  button.textContent = "Guardar";
  form.append(button);
  form.onsubmit = async (e) => {
    e.preventDefault();
    button.disabled = true;
    try {
      const data = Object.fromEntries(new FormData(form));
      for (const [name, , type] of fields)
        if (type === "multiselect")
          data[name] = Array.from(form.elements[name].selectedOptions).map(
            (o) => o.value,
          );
      await mutate(path, transform(data));
    } catch (error) {
      toast(error.message);
    } finally {
      button.disabled = false;
    }
  };
  parent.append(form);
}
function deliveryForm(id) {
  const parent = modal("Entregar un archivo");
  const form = document.createElement("form");
  input(form, "titulo", "Título del entregable", "", "text", true).maxLength =
    180;
  input(form, "comentario", "Comentario", "", "textarea");
  const file = input(
    form,
    "archivo",
    "Archivo (PDF, PNG, JPG o TXT; máximo 9 MB)",
    "",
    "file",
    true,
  );
  file.accept = ".pdf,.png,.jpg,.jpeg,.txt";
  const button = document.createElement("button");
  button.className = "primary";
  button.textContent = "Enviar entregable";
  form.append(button);
  form.onsubmit = async (e) => {
    e.preventDefault();
    button.disabled = true;
    try {
      const response = await fetch(`/api/retos/${id}/entregar`, {
        method: "POST",
        headers: { "X-CSRF-Token": csrf },
        body: new FormData(form),
      });
      const data = await response.json();
      if (!response.ok)
        throw new Error(data.error || "No se pudo cargar el archivo");
      $("detail-dialog").close();
      toast("Archivo entregado");
      await loadRecords();
    } catch (error) {
      toast(error.message);
    } finally {
      button.disabled = false;
    }
  };
  parent.append(form);
}
function documentForm(id) {
  const parent = modal("Cargar documento legal");
  const form = document.createElement("form");
  input(form, "fecha_expedicion", "Fecha de expedición", "", "date");
  input(
    form,
    "fecha_vencimiento",
    "Fecha de vencimiento (si aplica)",
    "",
    "date",
  );
  const file = input(
    form,
    "archivo",
    "Documento PDF, PNG o JPG (máximo 9 MB)",
    "",
    "file",
    true,
  );
  file.accept = ".pdf,.png,.jpg,.jpeg";
  const button = document.createElement("button");
  button.className = "primary";
  button.textContent = "Cargar documento";
  form.append(button);
  form.onsubmit = async (e) => {
    e.preventDefault();
    button.disabled = true;
    try {
      const response = await fetch(`/api/documentos/${id}/cargar`, {
        method: "POST",
        headers: { "X-CSRF-Token": csrf },
        body: new FormData(form),
      });
      const data = await response.json();
      if (!response.ok)
        throw new Error(data.error || "No se pudo cargar el documento");
      $("detail-dialog").close();
      toast("Documento enviado a revisión");
      await loadRecords();
    } catch (error) {
      toast(error.message);
    } finally {
      button.disabled = false;
    }
  };
  parent.append(form);
}
async function changeState(id, estado, comentario = "") {
  await mutate(`/api/retos/${id}/estado`, { estado, comentario });
}
function stateForm(id, estado) {
  const parent = modal(choice("retos", "estado", estado));
  const form = document.createElement("form");
  const comment = input(
    form,
    "comentario",
    "Motivo / comentario",
    "",
    "textarea",
    true,
  );
  const submit = document.createElement("button");
  submit.className = "primary";
  submit.textContent = "Confirmar";
  form.append(submit);
  form.onsubmit = async (e) => {
    e.preventDefault();
    try {
      await changeState(id, estado, comment.value);
    } catch (error) {
      toast(error.message);
    }
  };
  parent.append(form);
}
function challengeForm(record = {}) {
  const parent = modal(record.id ? "Editar reto" : "Crear un reto");
  const form = document.createElement("form");
  form.className = "form-grid";
  input(form, "titulo", "Título", record.titulo, "text", true).maxLength = 180;
  if (!record.id) {
    const wrap = document.createElement("div");
    const label = document.createElement("label");
    label.textContent = "Tipo";
    label.htmlFor = "field-tipo";
    const select = document.createElement("select");
    select.id = "field-tipo";
    select.name = "tipo";
    for (const name of ["reto", "hackathon"]) {
      const option = document.createElement("option");
      option.value = name;
      option.textContent = choice("retos", "tipo", name);
      select.append(option);
    }
    wrap.append(label, select);
    form.append(wrap);
  }
  input(
    form,
    "area",
    "Área",
    record.area || "Tecnología",
    "text",
    true,
  ).maxLength = 120;
  input(
    form,
    "descripcion",
    "Descripción",
    record.descripcion,
    "textarea",
    true,
  );
  input(
    form,
    "nivel_academico",
    "Nivel académico",
    record.nivel_academico || "Pregrado",
  );
  input(
    form,
    "fecha_inicio_tentativa",
    "Inicio tentativo",
    record.fecha_inicio_tentativa,
    "date",
    true,
  );
  input(
    form,
    "fecha_fin_tentativa",
    "Fin tentativo",
    record.fecha_fin_tentativa,
    "date",
    true,
  );
  input(
    form,
    "fecha_limite_postulacion",
    "Límite de postulación",
    record.fecha_limite_postulacion,
    "date",
    true,
  );
  const button = document.createElement("button");
  button.className = "primary full";
  button.textContent = "Guardar borrador";
  form.append(button);
  form.onsubmit = async (e) => {
    e.preventDefault();
    button.disabled = true;
    try {
      const data = Object.fromEntries(new FormData(form));
      data.tipo = record.tipo || data.tipo || "reto";
      for (const key of [
        "fecha_inicio_tentativa",
        "fecha_fin_tentativa",
        "fecha_limite_postulacion",
      ])
        data[key] ||= null;
      await mutate(record.id ? `/api/retos/${record.id}` : "/api/retos", data);
    } catch (error) {
      toast(error.message);
    } finally {
      button.disabled = false;
    }
  };
  parent.append(form);
}
function applicationForm(id) {
  const parent = modal("Postularme al reto");
  const form = document.createElement("form");
  input(
    form,
    "motivacion",
    "¿Por qué quieres participar?",
    "",
    "textarea",
    true,
  );
  input(form, "habilidades", "Habilidades", "", "textarea");
  input(form, "programa", "Programa académico");
  const semester = input(form, "semestre", "Semestre", "1", "number", true);
  semester.min = 1;
  semester.max = 20;
  const button = document.createElement("button");
  button.className = "primary";
  button.textContent = "Enviar postulación";
  form.append(button);
  form.onsubmit = async (e) => {
    e.preventDefault();
    try {
      const data = Object.fromEntries(new FormData(form));
      data.semestre = Number(data.semestre);
      await mutate(`/api/retos/${id}/postular`, data);
    } catch (error) {
      toast(error.message);
    }
  };
  parent.append(form);
}
$("login-form").onsubmit = async (e) => {
  e.preventDefault();
  $("login-error").hidden = true;
  const button = e.target.querySelector("button");
  button.disabled = true;
  try {
    const response = await api(
      "/api/login",
      Object.fromEntries(new FormData(e.target)),
    );
    csrf = response.csrf;
    $("password").value = "";
    await start();
  } catch (error) {
    $("login-error").replaceChildren(
      icon("exclamation-circle-fill"),
      el("span", "", error.message),
    );
    $("login-error").hidden = false;
  } finally {
    button.disabled = false;
  }
};
async function logout() {
  try {
    await api("/api/logout", {});
    showLogin();
  } catch (error) {
    showError(error);
  }
}
$("logout").onclick = logout;
$("close-dialog").onclick = () => $("detail-dialog").close();
$("new-challenge").onclick = () => {
  if (selected === "usuarios")
    simpleForm(
      "Crear usuario",
      "/api/usuarios",
      [
        ["username", "Usuario"],
        ["password", "Contraseña inicial (mínimo 12 caracteres)", "password"],
        ["email", "Correo", "email"],
        ["first_name", "Nombre"],
        ["last_name", "Apellido"],
        [
          "rol",
          "Rol",
          "select",
          ["ESTUDIANTE", "PROFESOR", "EMPRESA", "EXPERTO"],
        ],
        ["nit", "NIT (solo empresa)"],
        ["razon_social", "Razón social (solo empresa)"],
      ],
      (data) => ({ ...data, programa_id: null, facultad_id: null }),
    );
  else if (selected === "retos") challengeForm();
  else catalogForm().catch(showError);
};
async function catalogForm() {
  const fields = [["nombre", "Nombre"]];
  if (["facultades", "unidades_estudio"].includes(selected))
    fields.push(["codigo", "Código"]);
  if (selected === "ecosistemas")
    fields.push(["descripcion", "Descripción", "textarea"]);
  if (selected === "programas") {
    const data = await api("/api/records/facultades?limit=100");
    fields.push([
      "facultad_id",
      "Facultad",
      "select",
      data.records.map((r) => ({ value: r.id, label: r.nombre })),
    ]);
  }
  if (selected === "unidades_estudio") {
    const data = await api("/api/records/programas?limit=100");
    fields.push(
      [
        "programa_id",
        "Programa",
        "select",
        data.records.map((r) => ({ value: r.id, label: r.nombre })),
      ],
      ["periodo", "Periodo (ej. 2026-2)"],
      ["ciclo", "Ciclo o semestre"],
    );
  }
  simpleForm(
    "Agregar a " + title(selected),
    `/api/catalogos/${selected}`,
    fields,
    (data) => ({
      ...data,
      programa_id: data.programa_id ? Number(data.programa_id) : null,
      facultad_id: data.facultad_id ? Number(data.facultad_id) : null,
    }),
  );
}
function profileForm() {
  simpleForm("Mi Perfil", "/api/perfil", [
    ["first_name", "Nombre", "text", user.first_name],
    ["last_name", "Apellido", "text", user.last_name],
    ["email", "Correo", "email", user.email],
    ["telefono", "Teléfono"],
  ]);
  const extra = el("p", "mt-3 mb-0");
  const change = el("button", "secondary", "Cambiar contraseña");
  change.type = "button";
  change.onclick = () =>
    simpleForm(
      "Cambiar contraseña; deberás iniciar sesión nuevamente",
      "/api/password",
      [
        ["actual", "Contraseña actual", "password"],
        ["nueva", "Contraseña nueva (mínimo 12 caracteres)", "password"],
      ],
    );
  extra.append(change);
  $("dialog-content").append(extra);
}
// Label of a bare choice value used by the action forms (first model that defines it).
function choiceLabel(value) {
  for (const r of resources)
    for (const f of r.fields)
      for (const [k, v] of f.choices) if (k === value) return v;
  return human(value);
}
$("toggle-password").onclick = () => {
  const input = $("password");
  input.type = input.type === "password" ? "text" : "password";
  $("toggle-password").firstElementChild.className =
    "bi " + (input.type === "password" ? "bi-eye" : "bi-eye-slash");
};
$("bell").onclick = () => {
  $("bell-menu").hidden = !$("bell-menu").hidden;
  $("bell").setAttribute("aria-expanded", String(!$("bell-menu").hidden));
};
$("sidebar-toggle").onclick = () => {
  $("sidebar").classList.toggle("open");
  $("sidebar-overlay").classList.toggle("show");
};
$("sidebar-overlay").onclick = () => {
  $("sidebar").classList.remove("open");
  $("sidebar-overlay").classList.remove("show");
};
$("periodo").onchange = () => $("periodo-form").requestSubmit();
$("periodo-form").onsubmit = (e) => {
  e.preventDefault();
  const params = new URLSearchParams(new FormData(e.target));
  for (const [k, v] of [...params]) if (!v) params.delete(k);
  dashboardQuery = "?" + params;
  loadDashboard().catch(showError);
};
$("search-form").onsubmit = (e) => {
  e.preventDefault();
  query = $("search").value;
  page = 1;
  loadRecords().catch(showError);
};
$("previous").onclick = () => {
  if (page > 1) {
    page--;
    loadRecords().catch(showError);
  }
};
$("next").onclick = () => {
  if (page * 20 < total) {
    page++;
    loadRecords().catch(showError);
  }
};
window.addEventListener("hashchange", () => route().catch(showError));
start().catch(() => showLogin());
