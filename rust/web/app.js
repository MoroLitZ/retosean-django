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
  authRoute().catch(() => {});
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
    ["reportes", "Ver reportes", "bar-chart"],
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
    if (!["publicar", "reportes"].includes(target) && !resources.some((r) => r.table === target)) continue;
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
let showInactive = false;
async function loadRecords() {
  const inactive = selected === "unidades_estudio" && showInactive ? "&activo=0" : "";
  const data = await api(
    `/api/records/${selected}?page=${page}&q=${encodeURIComponent(query)}${inactive}`,
  );
  $("extra-panel").replaceChildren();
  if (selected === "certificados") {
    const link = el("a", "secondary", "Descargar mi portafolio (PDF)");
    link.href = "/api/portafolio";
    $("extra-panel").append(link);
  }
  if (selected === "unidades_estudio") {
    const wrap = el("div", "form-check mb-2");
    const box = el("input", "form-check-input");
    box.type = "checkbox";
    box.id = "show-inactive";
    box.checked = showInactive;
    box.onchange = () => {
      showInactive = box.checked;
      page = 1;
      loadRecords().catch(showError);
    };
    const l = el("label", "form-check-label", "Mostrar unidades inactivas");
    l.htmlFor = box.id;
    wrap.append(box, l);
    $("extra-panel").append(wrap);
  }
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
    if (user.rol === "PROFESOR")
      action(actions, "Crear equipo", async () => {
        const options = await api(`/api/retos/${id}/opciones`);
        teamFields("Configurar Equipo Académico", `/api/retos/${id}/equipo`, "", options);
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
    // Only the integrating professor (crear_rubrica); the server checks ownership.
    if (user.rol === "PROFESOR" && ["abierto", "en_curso", "pausado"].includes(record.estado))
      action(actions, "Crear rúbrica", () => rubricForm(id));
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
    // HU08: agenda, acta, final artifacts, recognitions and logistics.
    if (["ADMIN", "EMPRESA", "PROFESOR"].includes(user.rol) || admin())
      if (["aprobado", "abierto", "en_curso", "pausado", "finalizado"].includes(record.estado))
        action(actions, "Gestionar cierre", () => closurePanel(id));
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
  if (selected === "usuarios" && admin() && record.id !== user.id) {
    // HU13: role and per-module permissions.
    action(actions, "Cambiar rol", () =>
      simpleForm("Cambiar rol", `/api/usuarios/${id}/rol`, [
        ["rol", "Nuevo rol", "select", ["ADMIN", "EMPRESA", "PROFESOR", "ESTUDIANTE", "EXPERTO"].map((v) => ({ value: v, label: choiceLabel(v) }))],
      ]),
    );
    action(actions, "Permisos especiales", () => permissionsForm(id));
  }
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
    p.textContent = `Comprometido: $${summary.comprometido} · Ejecutado: $${summary.ejecutado} · Disponible: $${summary.disponible} · ${summary.porcentaje}% asignado${summary.alerta ? " · Umbral de alerta alcanzado" : ""}`;
    parent.prepend(p);
    // HU07: expenses with their state; admins execute commitments or remove them.
    const list = el("div", "mb-2");
    list.append(table(["Fecha", "Categoría", "Estado", "Descripción", "Monto"],
      summary.gastos.map((g) => [fecha(g.creado_en), choiceLabel(g.categoria), choiceLabel(g.estado), g.descripcion, `$${g.monto}`])));
    if (admin())
      for (const g of summary.gastos) {
        const line = el("div", "actions small mb-1");
        line.append(el("span", "me-2", `${choiceLabel(g.categoria)} $${g.monto}:`));
        if (g.estado === "COMPROMETIDO")
          action(line, "Marcar ejecutado", () => mutate(`/api/gastos/${g.id}/ejecutar`, {}));
        action(line, "Eliminar", async () => {
          if (confirm("¿Eliminar este gasto?")) await mutate(`/api/gastos/${g.id}/eliminar`, {});
        });
        list.append(line);
      }
    parent.insertBefore(list, actions);
    for (const [format, text] of [["xlsx", "Exportar gastos (Excel)"], ["pdf", "Exportar gastos (PDF)"]]) {
      const a = el("a", "secondary", text);
      a.href = `/api/retos/${summary.reto}/presupuesto/${format}`;
      actions.append(a);
    }
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
            ["estado", "Estado", "select", [{ value: "EJECUTADO", label: "Ejecutado (pagado)" }, { value: "COMPROMETIDO", label: "Comprometido (reservado)" }]],
            ["descripcion", "Descripción", "textarea"],
          ]),
        true,
      );
  }
  if (selected === "entregables" && user.rol === "PROFESOR") {
    action(actions, record.estado === "CALIFICADO" ? "Volver a calificar" : "Calificar", () => gradeForm(id, record), true);
    if (record.estado === "CALIFICADO" && !record.calificacion_publicada)
      action(actions, "Publicar calificaciones del reto", () =>
        mutate(`/api/retos/${record.reto_id}/publicar-calificaciones`, {}));
  }
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
  if (selected === "empresas" && admin())
    action(
      actions,
      record.participacion_bloqueada ? "Habilitar participación" : "Bloquear participación",
      () =>
        record.participacion_bloqueada
          ? mutate(`/api/empresas/${id}/bloqueo`, { bloquear: false })
          : simpleForm("Bloquear participación", `/api/empresas/${id}/bloqueo`, [["motivo", "Motivo del bloqueo", "textarea"]], (data) => ({ ...data, bloquear: true })),
    );
  if (selected === "empresas" && user.permisos.includes("empresas.cargar_listas_informa"))
    action(actions, "Cargar Listas Informa", () => documentForm(`/api/empresas/${id}/documentos/LISTAS_INFORMA/cargar`));
  if (selected === "empresas" && user.rol === "EMPRESA" && !record.renuncio_a_convenio)
    action(actions, "Participar sin convenio", async () => {
      const data = await api("/api/empresa/modo-convenio", { opcion: "no_convenio" });
      toast(data.motivos.length ? "Participarás sin convenio: no necesitas la minuta ni el convenio firmado. Carga el resto de documentos para poder publicar retos." : "Has seleccionado participar sin convenio.");
      $("detail-dialog").close();
      await loadRecords();
    });
  if (selected === "equipos_reto_academico" && user.rol === "PROFESOR") {
    const team = await api(`/api/equipos-academicos/${id}`).catch(() => null);
    if (team) {
      action(actions, "Editar equipo", () =>
        teamFields("Editar Equipo Académico", `/api/equipos-academicos/${id}`, team.nombre_equipo, team.opciones),
      );
      action(actions, "Eliminar equipo", async () => {
        if (confirm(`¿Eliminar el equipo "${team.nombre_equipo}"? También se elimina su equipo operativo.`))
          await mutate(`/api/equipos-academicos/${id}/eliminar`, {});
      });
    }
  }
  if (selected === "unidades_estudio" && (admin() || user.rol === "PROFESOR")) {
    action(actions, "Editar unidad", () => unitForm(record));
    action(actions, record.activo ? "Desactivar unidad" : "Activar unidad", async () => {
      const data = await api(`/api/unidades/${id}/activo`, { activo: !record.activo });
      $("detail-dialog").close();
      toast(data.mensaje);
      await loadRecords();
    });
  }
  if (selected === "documentos_empresas" && admin())
    action(actions, "Revisar documento", () =>
      simpleForm("Documento legal", `/api/documentos/${id}/revisar`, [
        ["estado", "Decisión", "select", ["VERIFICADO", "RECHAZADO"]],
        ["comentario", "Motivo / comentario", "textarea"],
      ]),
    );
  if (selected === "documentos_empresas" && user.rol === "EMPRESA")
    action(actions, "Cargar documento", () => documentForm(`/api/documentos/${id}/cargar`));
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
const EVENTOS_LOGISTICA = [["INICIO", "Evento de inicio"], ["CIERRE", "Evento de cierre"]];
const TIPOS_LOGISTICA = [["ESPACIO", "Espacio físico"], ["COMUNICACIONES", "Apoyo del área de comunicaciones"], ["REFRIGERIOS", "Refrigerios / catering"], ["EQUIPOS_AV", "Equipos audiovisuales"], ["INVITADOS", "Invitados"], ["OTRO", "Otro"]];
const ESTADOS_LOGISTICA = [["SOLICITADA", "Solicitada"], ["CONFIRMADA", "Confirmada"], ["DESCARTADA", "Descartada"]];
const IMPACTO = [["", "Sin registrar"], ["SI", "Sí, la aplicó"], ["NO", "No la aplicó"], ["EN_EVALUACION", "En evaluación"]];
function choiceSelect(parent, name, text, choices, value = "") {
  const wrap = el("div");
  const l = el("label", "", text);
  l.htmlFor = "field-" + name;
  const select = el("select");
  select.id = "field-" + name;
  select.name = name;
  for (const [v, t] of choices) {
    const option = el("option", "", t);
    option.value = v;
    option.selected = v === value;
    select.append(option);
  }
  wrap.append(l, select);
  parent.append(wrap);
  return select;
}
async function closurePanel(id) {
  const data = await api(`/api/retos/${id}/cierre`);
  const parent = modal("Gestión del cierre");
  const reload = async (message) => {
    toast(message);
    await closurePanel(id);
  };
  const sendForm = async (form, url) => {
    const response = await fetch(url, { method: "POST", headers: { "X-CSRF-Token": csrf }, body: new FormData(form) });
    const body = await response.json();
    if (!response.ok) throw new Error(body.error || "No se pudo guardar");
    return body;
  };
  const guard = (form, handler) => {
    form.onsubmit = async (e) => {
      e.preventDefault();
      const button = form.querySelector("button");
      button.disabled = true;
      try {
        await handler();
      } catch (error) {
        toast(error.message);
      } finally {
        button.disabled = false;
      }
    };
  };

  parent.append(el("h6", "fw-bold", "Agenda y acta"));
  const agenda = el("form", "form-grid");
  input(agenda, "fecha_hora", "Fecha y hora", data.agenda.fecha_hora || "", "datetime-local");
  input(agenda, "espacio", "Espacio", data.agenda.espacio || "");
  input(agenda, "recursos", "Recursos", data.agenda.recursos || "", "textarea");
  input(agenda, "invitados", "Invitados", data.agenda.invitados || "", "textarea");
  input(agenda, "agenda", "Agenda", data.agenda.agenda || "", "textarea");
  input(agenda, "notas", "Notas", data.cierre.notas || "", "textarea");
  choiceSelect(agenda, "impacto", "¿La empresa aplicó la solución?", IMPACTO, data.cierre.impacto || "");
  input(agenda, "acta_url", data.cierre.acta ? "Reemplazar acta" : "Acta de cierre", "", "file");
  if (data.cierre.acta) {
    const a = el("a", "small", "Descargar acta actual");
    a.href = `/api/archivo/cierres_reto/${data.cierre.id}/acta_url`;
    agenda.append(a);
  }
  agenda.append(el("button", "primary full", "Guardar documentación y agenda"));
  guard(agenda, async () => reload((await sendForm(agenda, `/api/retos/${id}/cierre`)).mensaje));
  parent.append(agenda);

  parent.append(el("h6", "fw-bold mt-3", "Entregables finales"));
  parent.append(table(["Nombre", "Cargado por", "Fecha"], data.entregables_finales.map((f) => [f.nombre, f.cargado_por, fecha(f.creado_en, true)])));
  const artifact = el("form", "form-grid");
  input(artifact, "nombre", "Nombre", "", "text", true);
  input(artifact, "archivo", "Archivo (PDF, PNG, JPG o texto)", "", "file", true);
  artifact.append(el("button", "secondary full", "Cargar entregable final"));
  guard(artifact, async () => reload((await sendForm(artifact, `/api/retos/${id}/cierre/entregables`)).mensaje));
  parent.append(artifact);

  parent.append(el("h6", "fw-bold mt-3", "Reconocimientos"));
  parent.append(table(["Destinatario", "Descripción"], data.reconocimientos.map((r) => [r.nombre_destinatario, r.descripcion])));
  const recognition = el("form", "form-grid");
  choiceSelect(recognition, "usuario_id", "Estudiante (opcional)", [["", "Sin vincular"], ...data.estudiantes.map((s) => [String(s.value), s.label])]);
  input(recognition, "nombre_destinatario", "Destinatario", "", "text", true);
  input(recognition, "descripcion", "Descripción", "", "textarea", true);
  recognition.append(el("button", "secondary full", "Registrar reconocimiento"));
  guard(recognition, async () => {
    const values = Object.fromEntries(new FormData(recognition));
    values.usuario_id = values.usuario_id ? Number(values.usuario_id) : null;
    reload((await api(`/api/retos/${id}/cierre/reconocimientos`, values)).mensaje);
  });
  parent.append(recognition);

  parent.append(el("h6", "fw-bold mt-3", "Solicitudes de logística"));
  const rows = el("div");
  rows.append(table(["Evento", "Tipo", "Detalle", "Estado", "Solicitada por"], data.solicitudes_logistica.map((r) => [label(EVENTOS_LOGISTICA, r.evento), label(TIPOS_LOGISTICA, r.tipo), r.descripcion, label(ESTADOS_LOGISTICA, r.estado), r.solicitada_por])));
  if (admin())
    for (const r of data.solicitudes_logistica) {
      const next = r.estado === "SOLICITADA" ? [["CONFIRMADA", "Confirmar"], ["DESCARTADA", "Descartar"]] : [["SOLICITADA", "Reabrir"]];
      const line = el("div", "actions small mb-1");
      line.append(el("span", "me-2", `${label(TIPOS_LOGISTICA, r.tipo)}:`));
      for (const [estado, text] of next)
        action(line, text, async () => {
          await api(`/api/solicitudes-logistica/${r.id}/estado`, { estado });
          await reload(`Solicitud marcada como ${label(ESTADOS_LOGISTICA, estado).toLowerCase()}.`);
        });
      rows.append(line);
    }
  parent.append(rows);
  const logistics = el("form", "form-grid");
  choiceSelect(logistics, "evento", "Evento", EVENTOS_LOGISTICA);
  choiceSelect(logistics, "tipo", "Tipo de solicitud", TIPOS_LOGISTICA);
  const detail = input(logistics, "descripcion", "Detalle", "", "textarea", true);
  detail.placeholder = "Ej.: auditorio para 80 personas, 9:00 a 12:00";
  logistics.append(el("button", "secondary full", "Solicitar logística"));
  guard(logistics, async () => reload((await api(`/api/retos/${id}/solicitudes-logistica`, Object.fromEntries(new FormData(logistics)))).mensaje));
  parent.append(logistics);
}
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

// ── Public pages: registration and password recovery (Django usuarios views) ──
function authField(parent, name, label, type = "text", options = null, value = "") {
  const wrap = el("div");
  const l = el("label", "form-label", label);
  l.htmlFor = "auth-" + name;
  let field;
  if (options) {
    field = el("select", "form-select");
    options.forEach(([v, t]) => field.append(new Option(t, v)));
  } else {
    field = el("input", "form-control");
    field.type = type;
  }
  field.id = "auth-" + name;
  field.name = name;
  // An empty value would deselect a <select>'s first option.
  if (value) field.value = value;
  if (type === "password") field.autocomplete = "new-password";
  wrap.append(l, field);
  parent.append(wrap);
  return field;
}
function authForm(title, subtitle, build, submitLabel, onSubmit, footer) {
  const box = $("auth-alt");
  box.replaceChildren(el("h2", "", title), el("p", "auth-subtitle", subtitle));
  const error = el("div", "alert alert-danger py-2 mb-3");
  error.hidden = true;
  const form = el("form");
  form.noValidate = true;
  build(form);
  const button = el("button", "btn btn-auth w-100 mt-4", submitLabel);
  form.append(button);
  form.onsubmit = async (e) => {
    e.preventDefault();
    error.hidden = true;
    button.disabled = true;
    try {
      await onSubmit(Object.fromEntries(new FormData(form)));
    } catch (err) {
      error.textContent = err.message;
      error.hidden = false;
    } finally {
      button.disabled = false;
    }
  };
  box.append(error, form);
  if (footer) box.append(footer);
}
function backToLogin(text = "¿Ya tienes cuenta?") {
  const p = el("p", "text-center mt-3 mb-0 auth-note", text + " ");
  const a = el("a", "text-link-green", "Inicia sesión aquí");
  a.href = "#";
  p.append(a);
  return p;
}
async function authRoute() {
  const hash = location.hash.slice(1);
  const alt = hash.startsWith("registro") || hash === "recuperar" || hash.startsWith("restablecer/");
  $("auth-login").hidden = alt;
  $("auth-alt").hidden = !alt;
  if (!alt) return;
  if (hash === "registro") {
    const box = $("auth-alt");
    box.replaceChildren(el("h2", "", "Crea tu cuenta"), el("p", "auth-subtitle", "Elige cómo vas a participar en RetosEAN"));
    for (const [target, title, text] of [
      ["registro/academico", "Comunidad EAN", "Si eres Estudiante o Profesor de la Universidad EAN, usa esta opción para validar tu correo institucional."],
      ["registro/empresa", "Empresas / Organizaciones", "Si eres un Aliado Comercial externo y deseas proponer retos o validar documentación legal, crea tu perfil corporativo aquí."],
    ]) {
      const a = el("a", "auth-choice text-decoration-none text-dark");
      a.href = "#" + target;
      a.append(el("div", "fw-bold", title), el("div", "small text-muted", text));
      box.append(a);
    }
    box.append(backToLogin());
    return;
  }
  const register = async (data) => {
    for (const k of ["programa_id", "facultad_id", "semestre"]) data[k] = data[k] ? Number(data[k]) : null;
    const session = await api("/api/registro", data);
    csrf = session.csrf;
    location.hash = "dashboard";
    await start();
  };
  if (hash === "registro/academico" || hash === "registro/empresa") {
    const academic = hash.endsWith("academico");
    const options = await api("/api/registro/opciones");
    authForm(academic ? "Registro Comunidad EAN" : "Registro de Empresa",
      academic ? "Usa tu correo institucional @universidadean.edu.co" : "Crea el perfil corporativo de tu organización",
      (form) => {
        const hidden = el("input");
        hidden.type = "hidden";
        hidden.name = "tipo";
        hidden.value = academic ? "academico" : "empresa";
        form.append(hidden);
        if (academic) {
          const rol = authField(form, "rol", "Rol", "text", [["ESTUDIANTE", "Estudiante"], ["PROFESOR", "Profesor"]]);
          authField(form, "first_name", "Nombre");
          authField(form, "last_name", "Apellido");
          const programa = authField(form, "programa_id", "Programa", "text", [["", "Seleccione un programa"], ...options.programas.map((p) => [p.value, p.label])]).parentElement;
          const semestre = authField(form, "semestre", "Semestre", "text", Array.from({ length: 10 }, (_, i) => [String(i + 1), String(i + 1)])).parentElement;
          const facultad = authField(form, "facultad_id", "Facultad", "text", [["", "Seleccione una facultad"], ...options.facultades.map((f) => [f.value, f.label])]).parentElement;
          const especialidad = authField(form, "especialidad", "Especialidad", "text", [["", "Seleccione una especialidad"], ...options.especialidades.map((e) => [e, e])]).parentElement;
          const toggle = () => {
            const student = rol.value === "ESTUDIANTE";
            programa.hidden = semestre.hidden = !student;
            facultad.hidden = especialidad.hidden = student;
          };
          rol.onchange = toggle;
          toggle();
        } else {
          authField(form, "nit", "NIT de la Empresa");
          authField(form, "razon_social", "Razon Social");
          authField(form, "sector_industrial", "Sector Industrial");
          authField(form, "tipo_convocatoria", "Tipo de convocatoria", "text", options.convocatorias);
          authField(form, "area_responsable", "Área responsable");
          authField(form, "sitio_web", "Sitio web", "url");
        }
        authField(form, "username", academic ? "Usuario" : "Usuario Corporativo");
        authField(form, "email", academic ? "Correo electronico" : "Correo Electronico Corporativo", "email");
        authField(form, "password", "Contrasena (mínimo 12 caracteres)", "password");
        authField(form, "password2", "Confirmar contrasena", "password");
      }, "Crear cuenta", register, backToLogin());
    return;
  }
  if (hash === "recuperar") {
    authForm("Recuperar contraseña", "Te enviaremos un enlace para elegir una nueva contraseña",
      (form) => authField(form, "email", "Correo electrónico", "email"),
      "Enviar enlace", async (data) => {
        await api("/api/password/reset", data);
        const box = $("auth-alt");
        box.replaceChildren(el("h2", "", "Revisa tu correo"),
          el("p", "auth-subtitle", "Si existe una cuenta con ese correo, te enviamos un enlace para restablecer la contraseña. El enlace vence en 3 días."),
          backToLogin("¿Recordaste tu contraseña?"));
      }, backToLogin("¿Recordaste tu contraseña?"));
    return;
  }
  const token = hash.slice("restablecer/".length);
  authForm("Elige una nueva contraseña", "Usa al menos 12 caracteres",
    (form) => {
      authField(form, "nueva", "Nueva contraseña", "password");
      authField(form, "nueva2", "Confirmar contraseña", "password");
    },
    "Cambiar contraseña", async (data) => {
      await api("/api/password/reset/confirm", { ...data, token });
      const box = $("auth-alt");
      box.replaceChildren(el("h2", "", "Contraseña actualizada"),
        el("p", "auth-subtitle", "Ya puedes iniciar sesión con tu nueva contraseña."), backToLogin(""));
    }, backToLogin());
}
// ── HU12: grading by free score or rubric; grades published per reto ──
async function gradeForm(id, record) {
  const { rubricas } = await api(`/api/entregables/${id}/rubricas`);
  const parent = modal("Evaluar entregable");
  const form = el("form");
  const modo = el("select", "form-select mb-2");
  modo.setAttribute("aria-label", "Forma de calificar");
  modo.append(new Option("Puntaje libre", ""));
  rubricas.forEach((r) => modo.append(new Option(`Rúbrica: ${r.nombre}`, r.id)));
  const libre = el("div");
  const nota = input(libre, "nota", `Nota (máximo ${record.puntaje_maximo})`, "", "number");
  nota.step = "0.01";
  nota.min = "0";
  const criterios = el("div");
  const draw = () => {
    const rubrica = rubricas.find((r) => String(r.id) === modo.value);
    libre.hidden = !!rubrica;
    criterios.replaceChildren();
    for (const c of rubrica?.criterios || []) {
      const f = input(criterios, `criterio_${c.id}`, `${c.nombre} (máximo ${c.puntaje_maximo})`, "", "number");
      f.step = "0.01";
      f.min = "0";
      f.dataset.criterio = c.id;
    }
  };
  modo.onchange = draw;
  draw();
  form.append(modo, libre, criterios);
  input(form, "comentario", "Retroalimentación", record.comentario_profesor || "", "textarea");
  let confirmar = null;
  if (record.estado === "CALIFICADO") {
    const wrap = el("div", "form-check mt-2");
    confirmar = el("input", "form-check-input");
    confirmar.type = "checkbox";
    confirmar.id = "confirmar-recalificacion";
    const l = el("label", "form-check-label", `Ya tiene nota (${record.nota}). Confirmo que quiero volver a calificarlo.`);
    l.htmlFor = confirmar.id;
    wrap.append(confirmar, l);
    form.append(wrap);
  }
  form.append(el("p", "small text-muted mt-2 mb-0", "La nota queda sin publicar hasta que publiques las calificaciones del reto."));
  const button = el("button", "primary", "Guardar evaluación");
  form.append(button);
  submitForm(form, button, `/api/entregables/${id}/calificar`, () => ({
    nota: nota.value,
    rubrica_id: modo.value ? Number(modo.value) : null,
    criterios: Object.fromEntries([...criterios.querySelectorAll("input")].map((f) => [f.dataset.criterio, f.value])),
    comentario: form.elements.comentario.value,
    confirmar_recalificacion: !!confirmar?.checked,
  }));
  parent.append(form);
}
function rubricForm(retoId) {
  const parent = modal("Nueva rúbrica");
  const form = el("form");
  input(form, "nombre", "Nombre", "", "text", true);
  input(form, "descripcion", "Descripción", "", "textarea");
  const criterios = editableRows(form, "Criterios", [
    { name: "nombre", label: "Criterio" },
    { name: "puntaje_maximo", label: "Puntaje máximo", type: "number" },
  ], [{}, {}]);
  const button = el("button", "primary", "Crear rúbrica");
  form.append(el("div", "mt-3"), button);
  submitForm(form, button, `/api/retos/${retoId}/rubrica`, () => ({
    nombre: form.elements.nombre.value,
    descripcion: form.elements.descripcion.value,
    criterios: criterios().filter((c) => c.nombre),
  }));
  parent.append(form);
}
// ── HU06: report builder (Django reportes/constructor.html) ──
const PERIODOS_REPORTE = [["30d", "Ultimos 30 dias"], ["90d", "Ultimos 90 dias"], ["anio", "Ultimo anio"], ["todo", "Historico completo"], ["custom", "Rango personalizado"]];
const FILTROS_REPORTE = {
  facultad: ["Facultad", "Todas"],
  programa: ["Programa", "Todos"],
  empresa: ["Empresa", "Todas"],
  estado: ["Estado del reto", "Todos"],
  estado_postulacion: ["Estado de postulacion", "Todos"],
  estado_empresa: ["Estado de la empresa", "Todos"],
};
let builderData = null;
async function loadBuilder() {
  builderData = builderData || (await api("/api/reportes/catalogo"));
  const panel = $("builder-panel");
  panel.replaceChildren();
  const card = el("div", "card-panel");
  const header = el("div", "card-panel-header");
  header.append(el("span", "", "Constructor de reportes"));
  if (resources.some((r) => r.table === "reportes_generados")) {
    const history = el("a", "btn btn-sm btn-outline-secondary");
    history.href = "#reportes_generados";
    history.append(icon("clock-history"), " Historial");
    header.append(history);
  }
  const body = el("div", "p-4");
  const form = el("form");
  const row = el("div", "row g-3 mb-3");
  const field = (parent, name, label, options, col = "col-md-4") => {
    const wrap = el("div", col);
    const l = el("label", "form-label small fw-semibold text-secondary", label);
    l.htmlFor = "rep-" + name;
    const select = el("select", "form-select form-select-sm");
    select.id = "rep-" + name;
    select.name = name;
    options.forEach(([v, t]) => select.append(new Option(t, v)));
    wrap.append(l, select);
    parent.append(wrap);
    return select;
  };
  const reporte = field(row, "reporte", "Reporte", builderData.reportes.map((r) => [r.clave, r.nombre]), "col-md-6");
  field(row, "formato", "Formato", [["PDF", "PDF"], ["EXCEL", "Excel"]], "col-md-3");
  const periodo = field(row, "periodo", "Periodo", PERIODOS_REPORTE, "col-md-3");
  periodo.value = "90d";
  const range = el("div", "row g-3 mb-3");
  for (const [name, label] of [["desde", "Desde"], ["hasta", "Hasta"]]) {
    const wrap = el("div", "col-md-3");
    const l = el("label", "form-label small fw-semibold text-secondary", label);
    l.htmlFor = "rep-" + name;
    const f = el("input", "form-control form-control-sm");
    f.type = "date";
    f.id = "rep-" + name;
    f.name = name;
    wrap.append(l, f);
    range.append(wrap);
  }
  periodo.onchange = () => (range.hidden = periodo.value !== "custom");
  range.hidden = true;
  const filters = el("div", "row g-3 mb-3");
  const columns = el("div", "row g-2 mb-4 p-3 bg-light rounded");
  const choice = (table, name) =>
    (meta(table)?.fields.find((f) => f.name === name)?.choices || []).filter(([v]) => v !== "");
  const filterOptions = {
    facultad: builderData.opciones.facultades,
    programa: builderData.opciones.programas,
    empresa: builderData.opciones.empresas,
    estado: choice("retos", "estado"),
    estado_postulacion: choice("postulaciones", "estado"),
    estado_empresa: choice("empresas", "estado_validacion"),
  };
  const draw = () => {
    const def = builderData.reportes.find((r) => r.clave === reporte.value);
    filters.replaceChildren();
    for (const name of def.filtros) {
      const [label, empty] = FILTROS_REPORTE[name];
      field(filters, name, label, [["", empty], ...filterOptions[name]]);
    }
    columns.replaceChildren();
    for (const [key, label] of def.columnas) {
      const wrap = el("div", "col-md-4 col-sm-6 form-check");
      const box = el("input", "form-check-input");
      box.type = "checkbox";
      box.id = "col-" + key;
      box.value = key;
      box.name = "columnas";
      const l = el("label", "form-check-label small", label);
      l.htmlFor = box.id;
      wrap.append(box, l);
      columns.append(wrap);
    }
  };
  reporte.onchange = draw;
  draw();
  const destinatarios = el("input", "form-control form-control-sm");
  destinatarios.name = "destinatarios";
  destinatarios.placeholder = "Por defecto, tu correo";
  destinatarios.id = "rep-destinatarios";
  const dWrap = el("div", "mb-3");
  const dLabel = el("label", "form-label small fw-semibold text-secondary", "Enviar a (correos separados por coma)");
  dLabel.htmlFor = destinatarios.id;
  dWrap.append(dLabel, destinatarios);
  const preview = el("div", "mt-3");
  const payload = () => {
    const data = Object.fromEntries(new FormData(form));
    data.columnas = [...form.querySelectorAll("input[name=columnas]:checked")].map((b) => b.value);
    for (const k of ["facultad", "programa", "empresa"]) data[k] = data[k] ? Number(data[k]) : null;
    for (const k of ["desde", "hasta"]) data[k] = data[k] || null;
    return data;
  };
  const buttons = el("div", "d-flex flex-wrap gap-2");
  const add = (label, iconName, cls, handler) => {
    const b = el("button", "btn fw-bold " + cls);
    b.type = "button";
    b.append(icon(iconName), " " + label);
    b.onclick = async () => {
      b.disabled = true;
      try {
        await handler();
      } catch (e) {
        toast(e.message);
      } finally {
        b.disabled = false;
      }
    };
    buttons.append(b);
  };
  add("Vista previa", "eye", "btn-outline-secondary", async () => {
    const data = await api("/api/reportes/vista-previa", payload());
    const box = el("div", "card-panel");
    const head = el("div", "card-panel-header");
    head.append(el("span", "", `Vista previa · ${data.nombre}`), el("span", "small text-muted fw-normal", `${data.mostradas} de ${data.total} registro${data.total === 1 ? "" : "s"}`));
    const inner = el("div", "p-3");
    inner.append(el("p", "text-muted small mb-2", data.filtros), table(data.encabezados, data.filas));
    if (data.total > data.mostradas)
      inner.append(el("p", "text-muted small mt-2 mb-0", `Se muestran las primeras ${data.mostradas} filas; el archivo incluye todas.`));
    box.append(head, inner);
    preview.replaceChildren(box);
  });
  add("Generar y descargar", "download", "btn-primary", async () => {
    const response = await fetch("/api/reportes/generar", {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-CSRF-Token": csrf },
      body: JSON.stringify(payload()),
    });
    if (!response.ok) throw new Error((await response.json()).error || "No se pudo generar el reporte");
    const name = (response.headers.get("content-disposition") || "").split("filename=")[1]?.replaceAll('"', "") || "reporte";
    const url = URL.createObjectURL(await response.blob());
    const a = el("a");
    a.href = url;
    a.download = name;
    a.click();
    URL.revokeObjectURL(url);
  });
  add("Generar y enviar por correo", "envelope", "btn-outline-primary", async () => {
    const data = await api("/api/reportes/enviar", payload());
    toast(`Reporte enviado a ${data.destinatarios.join(", ")}.`);
  });
  form.append(row, range, el("h6", "fw-bold text-dark mt-4 mb-2", "Filtros"), filters,
    el("h6", "fw-bold text-dark mt-4 mb-1", "Variables a incluir"),
    el("p", "text-muted small mb-2", "Si no eliges ninguna se incluyen todas."), columns, dWrap, buttons);
  body.append(form);
  card.append(header, body);
  panel.append(card, preview);
}
// ── HU13: bulk user import (Django usuarios/admin/importar_usuarios.html) ──
function importForm() {
  const parent = modal("Importación Masiva de Usuarios");
  parent.append(
    el("p", "small text-muted mb-1", "Columnas: username, email, first_name, last_name, rol, telefono, password. Obligatorias: username, email, rol."),
    el("p", "small text-muted", "Sin contraseña, la cuenta recibe por correo un enlace para definirla."),
  );
  const template = el("a", "secondary mb-3 d-inline-block", "Descargar plantilla CSV");
  template.href = "/api/usuarios/plantilla";
  const form = el("form");
  const file = input(form, "archivo", "Archivo .csv o .xlsx (máximo 1000 filas)", "", "file", true);
  file.accept = ".csv,.xlsx,.xlsm";
  const result = el("div", "mt-3");
  const send = async (accion) => {
    const data = new FormData();
    data.append("accion", accion);
    data.append("archivo", file.files[0]);
    const response = await fetch("/api/usuarios/importar", { method: "POST", headers: { "X-CSRF-Token": csrf }, body: data });
    const body = await response.json();
    if (!response.ok) throw new Error(body.error || "No se pudo leer el archivo");
    return body;
  };
  const preview = el("button", "secondary", "Previsualizar");
  preview.type = "button";
  const confirm = el("button", "primary ms-2", "Confirmar importación");
  confirm.type = "button";
  confirm.hidden = true;
  preview.onclick = async () => {
    if (!file.files[0]) return toast("Elige un archivo");
    try {
      const data = await send("previsualizar");
      result.replaceChildren();
      if (data.error_global) {
        result.append(el("div", "alert alert-danger py-2", data.error_global));
        confirm.hidden = true;
        return;
      }
      const rows = [...data.validas, ...data.invalidas].sort((a, b) => a.fila - b.fila);
      result.append(
        el("p", "small", `${data.validas.length} fila(s) válidas · ${data.invalidas.length} con errores`),
        table(["Fila", "Usuario", "Correo", "Nombre", "Rol", "Estado"], rows.map((r) => [
          r.fila, r.datos.username, r.datos.email, `${r.datos.first_name || ""} ${r.datos.last_name || ""}`.trim(), r.datos.rol,
          r.errores.length ? r.errores.join(" ") : "Lista para crear",
        ])),
      );
      confirm.hidden = data.validas.length === 0;
    } catch (e) {
      toast(e.message);
    }
  };
  confirm.onclick = async () => {
    confirm.disabled = true;
    try {
      const data = await send("confirmar");
      $("detail-dialog").close();
      toast(`${data.creados} usuario(s) creado(s) correctamente.` + (data.omitidas ? ` ${data.omitidas} fila(s) se omitieron por errores de validacion.` : ""));
      await loadRecords();
    } catch (e) {
      toast(e.message);
    } finally {
      confirm.disabled = false;
    }
  };
  form.append(el("div", "mt-2"), preview, confirm, result);
  parent.append(template, form);
}
function simpleForm(title, path, fields, transform = (data) => data) {
  const parent = modal(title),
    form = document.createElement("form");
  form.className = "form-grid";
  for (const [name, label, type = "text", value = ""] of fields) {
    if (type === "select" || type === "multiselect" || type === "optional") {
      const wrap = document.createElement("div"),
        l = document.createElement("label");
      l.htmlFor = "field-" + name;
      l.textContent = label;
      const select = document.createElement("select");
      select.id = "field-" + name;
      select.name = name;
      select.required = type === "select";
      select.multiple = type === "multiselect";
      for (const v of value) {
        const option = document.createElement("option");
        option.value = typeof v === "object" ? v.value : v;
        option.textContent = typeof v === "object" ? v.label : choiceLabel(v);
        option.selected = typeof v === "object" && !!v.selected;
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
          "sector_industrial",
          "area_responsable",
          "sitio_web",
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
function documentForm(url) {
  const parent = modal("Cargar documento legal");
  const form = document.createElement("form");
  input(form, "fecha_expedicion", "Fecha de expedición", "", "date", true);
  const file = input(
    form,
    "archivo",
    "Documento PDF (máximo 9 MB)",
    "",
    "file",
    true,
  );
  file.accept = ".pdf";
  const button = document.createElement("button");
  button.className = "primary";
  button.textContent = "Cargar documento";
  form.append(button);
  form.onsubmit = async (e) => {
    e.preventDefault();
    button.disabled = true;
    try {
      const response = await fetch(url, {
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
async function permissionsForm(id) {
  const data = await api(`/api/usuarios/${id}/permisos`);
  const parent = modal("Permisos especiales");
  const form = document.createElement("form");
  for (const p of data.permisos) {
    const wrap = el("div", "form-check mb-2");
    const box = document.createElement("input");
    box.type = "checkbox";
    box.className = "form-check-input";
    box.id = "perm-" + p.codigo;
    box.value = p.codigo;
    box.checked = p.otorgado;
    const l = el("label", "form-check-label", p.nombre);
    l.htmlFor = box.id;
    wrap.append(box, l, el("div", "small text-muted", p.descripcion));
    form.append(wrap);
  }
  const button = el("button", "primary", "Guardar permisos");
  form.append(button);
  form.onsubmit = async (e) => {
    e.preventDefault();
    const codigos = Array.from(form.querySelectorAll("input:checked")).map((b) => b.value);
    await mutate(`/api/usuarios/${id}/permisos`, { codigos });
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
async function applicationForm(id) {
  // PostulacionForm: catalog program, optional PDF CV, required terms.
  const options = await api("/api/registro/opciones");
  const parent = modal("Postularme al reto");
  const form = el("form", "form-grid");
  input(form, "motivacion", "¿Por qué quieres participar?", "", "textarea", true);
  input(form, "habilidades", "Habilidades", "", "textarea");
  choiceSelect(form, "programa", "Programa académico", [["", "Selecciona tu programa académico"], ...options.programas.map((p) => [p.nombre, p.label])]);
  const semester = input(form, "semestre", "Semestre", "1", "number", true);
  semester.min = 1;
  semester.max = 12;
  const cv = input(form, "hoja_de_vida", "Hoja de vida o portafolio (PDF, opcional)", "", "file");
  cv.accept = ".pdf";
  const terms = el("div", "form-check full");
  const box = el("input", "form-check-input");
  box.type = "checkbox";
  box.name = "acepta_terminos";
  box.id = "field-acepta_terminos";
  box.required = true;
  const label = el("label", "form-check-label", "Acepto los términos y condiciones del reto y el tratamiento de mis datos personales");
  label.htmlFor = box.id;
  terms.append(box, label);
  form.append(terms);
  const button = el("button", "primary full", "Enviar postulación");
  form.append(button);
  form.onsubmit = async (e) => {
    e.preventDefault();
    button.disabled = true;
    try {
      const response = await fetch(`/api/retos/${id}/postular`, { method: "POST", headers: { "X-CSRF-Token": csrf }, body: new FormData(form) });
      const data = await response.json();
      if (!response.ok) throw new Error(data.error || "No se pudo enviar la postulación");
      $("detail-dialog").close();
      toast("¡Tu postulación fue enviada! La empresa fue notificada y revisará tu solicitud.");
      await loadRecords();
    } catch (error) {
      toast(error.message);
    } finally {
      button.disabled = false;
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
  if (selected === "facultades") fields.push(["codigo", "Código"]);
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
  if (selected === "unidades_estudio") return unitForm();
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
async function unitForm(record = null) {
  const programs = await api("/api/records/programas?limit=100");
  const parent = modal(record ? "Editar Unidad de Estudio" : "Crear Unidad de Estudio");
  const form = el("form", "form-grid");
  input(form, "codigo", "Código", record?.codigo || "", "text", true);
  input(form, "nombre", "Nombre de la unidad", record?.nombre || "", "text", true);
  choiceSelect(form, "programa", "Programa académico", [["", "Selecciona"], ...programs.records.map((p) => [String(p.id), p.nombre])], record ? String(record.programa_id) : "");
  input(form, "periodo", "Periodo (ej: 2026-2)", record?.periodo || "");
  input(form, "ciclo", "Ciclo / Semestre", record?.ciclo || "");
  const file = input(form, "archivo", record?.archivo ? "Reemplazar archivo (PDF)" : "Archivo (PDF con info completa)", "", "file");
  file.accept = ".pdf";
  form.append(el("button", "primary full", "Guardar"));
  form.onsubmit = (e) => sendUnit(e, form, record ? `/api/unidades/${record.id}` : "/api/unidades");
  parent.append(form);
  if (!record) {
    parent.append(el("h6", "fw-bold mt-3", "Carga masiva (CSV)"));
    parent.append(el("p", "small text-muted", "Columnas: codigo, nombre, programa (nombre exacto del catálogo), periodo y ciclo opcionales."));
    const bulk = el("form", "form-grid");
    const accion = el("input");
    accion.type = "hidden";
    accion.name = "accion";
    accion.value = "cargar_csv";
    bulk.append(accion);
    input(bulk, "archivo_csv", "Archivo CSV", "", "file", true).accept = ".csv";
    bulk.append(el("button", "secondary full", "Cargar CSV"));
    bulk.onsubmit = (e) => sendUnit(e, bulk, "/api/unidades");
    parent.append(bulk);
  }
}
async function sendUnit(e, form, url) {
  e.preventDefault();
  const button = form.querySelector("button");
  button.disabled = true;
  try {
    const response = await fetch(url, { method: "POST", headers: { "X-CSRF-Token": csrf }, body: new FormData(form) });
    const data = await response.json();
    if (!response.ok) throw new Error(data.error || "No se pudo guardar");
    $("detail-dialog").close();
    toast(data.mensaje);
    await loadRecords();
  } catch (error) {
    toast(error.message);
  } finally {
    button.disabled = false;
  }
}
function teamFields(heading, url, name, options) {
  simpleForm(
    heading,
    url,
    [
      ["nombre_equipo", "Nombre del equipo", "text", name],
      ["profesores", "Profesores", "multiselect", options.profesores],
      ["estudiantes", "Estudiantes", "multiselect", options.estudiantes],
      ["expertos_invitados", "Expertos invitados", "multiselect", options.expertos_invitados],
    ],
    (data) => ({
      ...data,
      profesores: data.profesores.map(Number),
      estudiantes: data.estudiantes.map(Number),
      expertos_invitados: data.expertos_invitados.map(Number),
    }),
  );
}
async function profileForm() {
  const [current, options] = await Promise.all([api("/api/perfil"), api("/api/registro/opciones")]);
  const pick = (list, value) => [{ value: "", label: "Seleccione" }, ...list.map((o) => ({ ...o, selected: String(o.value) === String(value ?? "") }))];
  const fields = [
    ["first_name", "Nombre", "text", current.first_name],
    ["last_name", "Apellido", "text", current.last_name],
    ["email", "Correo", "email", current.email],
    ["telefono", "Teléfono", "text", current.telefono],
  ];
  // EditarPerfilForm: role fields, all optional.
  if (user.rol === "ESTUDIANTE")
    fields.push(
      ["primera_carrera", "Carrera / Programa", "optional", pick(options.programas, current.primera_carrera)],
      ["semestre", "Semestre", "optional", pick([...Array(10)].map((_, i) => ({ value: String(i + 1), label: String(i + 1) })), current.semestre)],
    );
  if (user.rol === "PROFESOR")
    fields.push(
      ["facultad", "Facultad", "optional", pick(options.facultades, current.facultad)],
      ["especialidad", "Especialidad", "optional", pick(options.especialidades.map((e) => ({ value: e, label: e })), current.especialidad)],
    );
  if (user.rol === "EMPRESA")
    fields.push(
      ["razon_social", "Razón Social", "text", current.razon_social],
      ["sector_industrial", "Sector Industrial", "text", current.sector_industrial],
      ["tipo_convocatoria", "Tipo de convocatoria", "optional", pick([{ value: "RETO", label: "Reto" }, { value: "HACKATHON", label: "Hackathon" }, { value: "AMBOS", label: "Reto y hackathon" }], current.tipo_convocatoria)],
      ["area_responsable", "Área responsable", "text", current.area_responsable],
      ["sitio_web", "Sitio web", "url", current.sitio_web],
    );
  simpleForm("Mi Perfil", "/api/perfil", fields, (data) => ({
    ...data,
    primera_carrera: data.primera_carrera ? Number(data.primera_carrera) : null,
    semestre: data.semestre ? Number(data.semestre) : null,
    facultad: data.facultad ? Number(data.facultad) : null,
  }));
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
$("import-users").onclick = importForm;
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
