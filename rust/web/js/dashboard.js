"use strict";
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
    ["Equipos académicos", "equipos_academicos", "people", "#0891b2"],
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

