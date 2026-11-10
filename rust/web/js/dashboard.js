"use strict";
/* ═══════════════════════════════════════════════════════════════════════════
   Dashboard de Indicadores — KPIs, gráficas, filtros y accesos rápidos.
   Construido con el design system RetosEAN (helper ui.*, icon(), tokens CSS);
   sin Bootstrap. Nombres de nivel superior: KPIS, GRAFICAS, ACCESOS,
   dashboardQuery y loadDashboard (estables para core.js/main.js). Los helpers
   privados usan el prefijo `dash_` para evitar colisiones globales.
   ═══════════════════════════════════════════════════════════════════════════ */

// [texto, clave del API, icono, tono (kpi__icon--<tono>), nota(kpis) opcional]
const KPIS = {
  ADMIN: [
    ["Retos totales", "retos_total", "trophy", "accent"],
    ["Retos activos", "retos_activos", "play-circle", "success"],
    ["Finalizados", "retos_finalizados", "flag", "violet"],
    ["En revisión", "retos_en_revision", "hourglass-split", "warning"],
    ["Empresas", "empresas_total", "building", "info", (k) => `${k.empresas_verificadas} verificadas`],
    ["Estudiantes", "estudiantes_total", "mortarboard", "brand"],
    ["Docentes", "profesores_total", "person-video3", "pink"],
    ["Postulaciones", "postulaciones_total", "send", "danger", (k) => `${k.postulaciones_aceptadas} aceptadas`],
    ["Solución aplicada", "impacto_si", "check2-circle", "success", () => "la empresa la aplicó"],
    ["No aplicada", "impacto_no", "x-circle", "danger"],
    ["Impacto en evaluación", "impacto_evaluacion", "hourglass", "warning"],
  ],
  EMPRESA: [
    ["Mis retos", "retos_total", "trophy", "accent"],
    ["Activos", "retos_activos", "play-circle", "success"],
    ["En revisión", "retos_en_revision", "hourglass-split", "warning"],
    ["Finalizados", "retos_finalizados", "flag", "violet"],
    ["Postulaciones", "postulaciones_total", "send", "info"],
    ["Pendientes", "postulaciones_pendientes", "clock-history", "warning"],
    ["Aceptadas", "postulaciones_aceptadas", "person-check", "success"],
    ["Entregables", "entregables_total", "folder-check", "pink"],
  ],
  PROFESOR: [
    ["Cursos activos", "cursos_activos", "journal-text", "accent"],
    ["Retos asignados", "retos_asignados", "trophy", "success"],
    ["Mis estudiantes", "estudiantes_total", "people", "info"],
    ["Por calificar", "evaluaciones_pendientes", "clipboard2-check", "warning"],
    ["Entregables recibidos", "entregables_total", "folder-check", "violet"],
    ["Integraciones en revisión", "integraciones_en_revision", "hourglass-split", "danger"],
    ["Nota promedio", "nota_promedio", "star-half", "brand"],
  ],
  ESTUDIANTE: [
    ["Retos disponibles", "retos_disponibles", "search", "accent"],
    ["Mis postulaciones", "postulaciones_total", "send", "info"],
    ["Aceptadas", "postulaciones_aceptadas", "person-check", "success"],
    ["Favoritos", "favoritos_total", "star", "warning"],
    ["Entregables", "entregables_total", "folder-check", "violet"],
    ["Sin calificar", "entregables_pendientes", "hourglass-split", "danger"],
    ["Nota promedio", "nota_promedio", "star-half", "brand"],
  ],
  EXPERTO: [
    ["Hackathones asignados", "hackathones_asignados", "lightning-charge", "accent"],
    ["En curso", "hackathones_en_curso", "play-circle", "success"],
    ["Equipos por evaluar", "equipos_por_evaluar", "hourglass-split", "warning"],
    ["Votos emitidos", "votos_emitidos", "check2-square", "violet"],
    ["Puntaje promedio", "puntaje_promedio", "star-half", "brand"],
    ["Equipos académicos", "equipos_academicos", "people", "info"],
  ],
};

// [título, clave de la serie, tipo de gráfica, etiqueta del dataset, ancho ("med"|"wide")]
const GRAFICAS = {
  ADMIN: [
    ["Retos por estado", "retos_por_estado", "doughnut", "Total", "med"],
    ["Retos por facultad", "retos_por_facultad", "bar", "Retos", "wide"],
    ["Retos creados por mes", "retos_por_mes", "line", "Retos creados", "wide"],
    ["Integraciones académicas", "integraciones_por_estado", "doughnut", "Total", "med"],
    ["Impacto: ¿la empresa aplicó la solución?", "impacto", "doughnut", "Total", "med"],
  ],
  EMPRESA: [
    ["Mis retos por estado", "retos_por_estado", "doughnut", "Total", "med"],
    ["Retos publicados por mes", "retos_por_mes", "line", "Retos", "wide"],
  ],
  PROFESOR: [
    ["Entregables por estado", "entregables_por_estado", "doughnut", "Total", "med"],
    ["Entregables por reto", "entregables_por_reto", "bar", "Entregables", "wide"],
  ],
  ESTUDIANTE: [
    ["Mis postulaciones", "postulaciones_por_estado", "doughnut", "Total", "med"],
    ["Mis notas recientes", "notas_recientes", "bar", "Nota", "wide"],
  ],
  EXPERTO: [["Votos por hackathon", "votos_por_hackathon", "bar", "Votos", "wide"]],
};

const ACCESOS = {
  ADMIN: [
    ["usuarios", "Gestionar usuarios", "people"],
    ["retos", "Revisar retos", "trophy"],
    ["empresas", "Empresas", "building"],
    ["reportes", "Ver reportes", "bar-chart-line"],
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

// Tarjeta de bienvenida de los roles simplificados: [ruta, texto, explicación].
const BIENVENIDA = {
  ESTUDIANTE: [
    ["retos", "Buscar un reto", "Encuentra un reto y postúlate."],
    ["entregables", "Mis entregables", "Sube tus avances y mira tus notas."],
    ["postulaciones", "Mis postulaciones", "Revisa si ya te aceptaron."],
  ],
  PROFESOR: [
    ["entregables", "Calificar entregables", "Califica lo que enviaron tus estudiantes."],
    ["integraciones_academicas", "Mis cursos", "Conecta retos con tus cursos."],
    ["postulaciones", "Mis estudiantes", "Revisa quién está en cada reto."],
  ],
  EMPRESA: [
    ["publicar", "Publicar un reto", "Plantea un problema real para que los estudiantes lo resuelvan."],
    ["postulaciones", "Estudiantes postulados", "Acepta o rechaza a quienes se postularon."],
    ["documentos_empresas", "Mis documentos legales", "Sube los documentos que la universidad necesita."],
  ],
};

let dashboardQuery = "";

/* ── Helpers privados (prefijo dash_, propios del área dashboard) ─────────── */

// Tipo de gráfica en español, para el texto alternativo del canvas.
function dash_tipoEs(tipo) {
  return tipo === "doughnut" ? "de dona" : tipo === "bar" ? "de barras" : "de líneas";
}

// Lee un token CSS (custom property) con un valor de respaldo.
function dash_token(name, fallback) {
  const value = getComputedStyle(document.documentElement).getPropertyValue(name);
  return (value && value.trim()) || fallback;
}

// Misma regla de visibilidad del menú lateral: la acción solo se muestra si el
// rol realmente tiene esa ruta («publicar» existe cuando hay «retos»).
function dash_has(target) {
  return resources.some((r) => r.table === (target === "publicar" ? "retos" : target));
}

// Tarjeta «Hola, …» con las acciones principales del rol simplificado.
function dash_renderWelcome() {
  const panel = $("dashboard-panel");
  let box = $("dash-welcome");
  const actions = (BIENVENIDA[role()] || []).filter(([target]) => dash_has(target));
  if (!simpleRole() || !actions.length) {
    if (box) box.hidden = true;
    return;
  }
  if (!box) {
    box = el("section", "surface dash-welcome");
    box.id = "dash-welcome";
    panel.prepend(box);
  }
  box.hidden = false;
  const list = el("div", "dash-welcome-actions");
  for (const [target, text, hint] of actions) {
    const item = el("div", "dash-welcome-action");
    const a = el("a", "btn btn--primary btn--lg w-100", text);
    a.href = "#" + target;
    item.append(a, el("p", "dash-welcome-hint text-center", hint));
    list.append(item);
  }
  box.replaceChildren(
    el("h2", "dash-welcome-title", `Hola, ${user.first_name || user.username}`),
    el("p", "dash-welcome-sub", "Esto es lo que puedes hacer ahora:"),
    list,
  );
}

// Filtro de periodo compacto para los roles simplificados: un <select> pequeño
// a la derecha del título «Tu resumen» que replica las opciones de #periodo y
// dispara la lógica de aplicación ya existente (periodo-form) al cambiar.
function dash_compactPeriod(simple, sinRol) {
  const kpiTitle = $("dash-kpis-title");
  let head = $("dash-kpis-head");
  if (simple && kpiTitle && !head) {
    head = el("div", "dash-section-head");
    head.id = "dash-kpis-head";
    kpiTitle.before(head);
    head.append(kpiTitle);
    const compact = el("select", "select input--sm dash-periodo");
    compact.id = "periodo-compact";
    compact.setAttribute("aria-label", "Periodo");
    compact.onchange = () => {
      $("periodo").value = compact.value;
      $("periodo-form").requestSubmit();
    };
    head.append(compact);
  }
  // Al volver a entrar con ADMIN/EXPERTO sin recargar, se deshace el envoltorio.
  if (head && !simple) {
    head.before(kpiTitle);
    head.remove();
    head = null;
  }
  if (head) head.hidden = sinRol;
  const compact = $("periodo-compact");
  if (compact) {
    compact.replaceChildren(
      // «Personalizado» necesita las fechas Desde/Hasta de la tarjeta oculta.
      ...[...$("periodo").options].filter((o) => o.value !== "custom").map((o) => {
        const option = el("option", "", o.textContent);
        option.value = o.value;
        option.selected = o.selected;
        return option;
      }),
    );
    compact.hidden = !simple || sinRol;
  }
}

// Inserta un encabezado de sección antes de `target` (idempotente) y lo devuelve.
function dash_sectionTitle(target, id, text) {
  let heading = document.getElementById(id);
  if (!heading) {
    heading = el("h2", "dash-section-title", text);
    heading.id = id;
    target.before(heading);
  } else {
    heading.textContent = text;
  }
  return heading;
}

// Esqueleto de la cuadrícula de KPIs mientras llega la respuesta (UX audit #1).
function dash_kpiSkeletons() {
  const frag = document.createDocumentFragment();
  for (let i = 0; i < 8; i++) {
    const tile = el("div", "surface kpi dash-kpi dash-skeleton-tile");
    tile.setAttribute("aria-hidden", "true");
    const iconBox = el("div", "kpi__icon");
    iconBox.append(el("span", "skeleton dash-skeleton-avatar"));
    const textBox = el("div", "dash-kpi-text dash-skeleton-lines");
    textBox.append(
      el("span", "skeleton skeleton--title"),
      el("span", "skeleton skeleton--text"),
    );
    tile.append(iconBox, textBox);
    frag.append(tile);
  }
  return frag;
}

// Esqueleto de los paneles de gráfica mientras llega la respuesta.
function dash_chartSkeletons() {
  const frag = document.createDocumentFragment();
  for (const width of ["med", "wide"]) {
    const panel = el("div", "card-panel dash-chart dash-chart--" + width);
    panel.setAttribute("aria-hidden", "true");
    const header = el("div", "card-panel-header");
    header.append(el("span", "skeleton skeleton--title"));
    const box = el("div", "chart-box dash-chart-box");
    box.append(el("div", "skeleton dash-chart-skeleton-block"));
    panel.append(header, box);
    frag.append(panel);
  }
  return frag;
}

async function loadDashboard() {
  const kpis = $("kpis");
  const graficas = $("graficas");
  const accesos = $("accesos");
  const periodForm = $("periodo-form");
  const applyBtn = periodForm ? periodForm.querySelector('[type="submit"]') : null;
  const simple = simpleRole();
  // Bienvenida arriba de todo y, para roles simplificados, la tarjeta grande
  // de filtros se reemplaza por un selector compacto junto a «Tu resumen».
  dash_renderWelcome();
  $("periodo-form").closest(".card-panel").hidden = simple;

  // ── Estado de carga: esqueletos + aria-busy en las regiones dinámicas ──
  charts.forEach((c) => c.destroy());
  charts = [];
  kpis.className = "dash-kpis mb-4";
  graficas.className = "dash-graficas mb-4";
  accesos.className = "cluster cluster--sm";
  kpis.setAttribute("aria-busy", "true");
  graficas.setAttribute("aria-busy", "true");
  kpis.replaceChildren(dash_kpiSkeletons());
  graficas.replaceChildren(dash_chartSkeletons());
  accesos.replaceChildren();

  const request = api("/api/dashboard" + dashboardQuery);
  let data;
  try {
    // busy() deshabilita el botón «Aplicar» con spinner hasta que termine.
    data = applyBtn ? await busy(applyBtn, request) : await request;
  } catch (error) {
    kpis.removeAttribute("aria-busy");
    graficas.removeAttribute("aria-busy");
    kpis.className = "mb-4";
    graficas.className = "mb-4";
    kpis.replaceChildren(
      ui.empty(
        "exclamation-triangle",
        "No se pudieron cargar los indicadores",
        "Tuvimos un problema al consultar el panel. Intenta de nuevo en unos instantes.",
      ),
    );
    graficas.replaceChildren();
    showError(error);
    return;
  }
  kpis.removeAttribute("aria-busy");
  graficas.removeAttribute("aria-busy");

  // ── Filtros + etiqueta de periodo (comportamiento del API sin cambios) ──
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
    const fill = (id, empty, lista) => {
      const current = params.get(id) || "";
      $(id).replaceChildren(new Option(empty, ""), ...lista.map((r) => new Option(r.nombre, r.id, false, String(r.id) === current)));
    };
    fill("facultad", "Todas", data.facultades);
    fill("programa", "Todos", data.programas);
    fill("ecosistema", "Todos", data.ecosistemas);
  }

  // ── Encabezados de sección (jerarquía visual); ocultos si no hay rol ──
  const sinRol = !!data.sin_rol;
  const kpiTitle = dash_sectionTitle(kpis, "dash-kpis-title", simple ? "Tu resumen" : "Indicadores clave");
  kpiTitle.hidden = sinRol;
  kpis.setAttribute("aria-labelledby", "dash-kpis-title");
  const chartTitle = dash_sectionTitle(graficas, "dash-charts-title", simple ? "Gráficas" : "Distribución y tendencias");
  chartTitle.hidden = sinRol;
  graficas.setAttribute("aria-labelledby", "dash-charts-title");
  dash_compactPeriod(simple, sinRol);

  // ── Tarjetas KPI ──
  kpis.replaceChildren();
  for (const [text, key, iconName, tone, note] of sinRol ? [] : KPIS[data.rol]) {
    let value = data.kpis[key];
    const isAverage = key === "nota_promedio" || key === "puntaje_promedio";
    if (isAverage) value = value == null ? "" : Number(value).toFixed(2);
    // «—» para datos ausentes; «0» solo para un cero real (UX audit #15).
    const display = value == null || value === "" ? "—" : String(value);
    const tile = el("article", "surface kpi dash-kpi");
    const iconBox = el("div", "kpi__icon kpi__icon--" + (tone || "brand"));
    iconBox.append(icon(iconName));
    const textBox = el("div", "dash-kpi-text");
    textBox.append(
      el("div", "kpi__label", text),
      el("div", "kpi__value", display),
    );
    if (note) textBox.append(el("div", "kpi__note", note(data.kpis)));
    tile.append(iconBox, textBox);
    kpis.append(tile);
  }

  // ── Gráficas ──
  if (window.Chart) {
    Chart.defaults.font.family = dash_token("--font-sans", "system-ui, sans-serif");
    Chart.defaults.color = dash_token("--ink-muted", "#6b7280");
    Chart.defaults.plugins.legend.labels.boxWidth = 12;
  }
  const palette = [
    dash_token("--chart-1", "#27783a"),
    dash_token("--chart-2", "#005e85"),
    dash_token("--chart-3", "#d97706"),
    dash_token("--chart-4", "#7c3aed"),
    dash_token("--chart-5", "#0e9f8e"),
    dash_token("--chart-6", "#db2777"),
    dash_token("--chart-7", "#64748b"),
    dash_token("--chart-8", "#84cc16"),
  ];
  const gridColor = dash_token("--chart-grid", "rgba(0, 0, 0, 0.08)");
  const tickColor = dash_token("--chart-tick", "#677069");
  const surfaceColor = dash_token("--surface", "#ffffff");
  const colorAt = (i) => palette[i % palette.length];
  const short = (s) => {
    s = String(s ?? "");
    return s.length > 18 ? s.slice(0, 17) + "…" : s;
  };

  graficas.replaceChildren();
  let chartIndex = 0;
  for (const [heading, serieName, tipo, etiqueta, width] of sinRol ? [] : GRAFICAS[data.rol]) {
    chartIndex += 1;
    const serie = data.series[serieName];
    const hasData = !!(serie && serie.data && serie.data.length && window.Chart);

    const panel = el("div", "card-panel dash-chart dash-chart--" + (width || "med"));
    const header = el("div", "card-panel-header");
    const headingEl = el("h3", "dash-chart-title", heading);
    header.append(headingEl);
    panel.append(header);

    const box = el("div", "chart-box dash-chart-box");

    if (!hasData) {
      box.replaceChildren(
        ui.empty(
          "bar-chart-line",
          "Sin datos",
          "Aún no hay valores para este indicador en el periodo seleccionado.",
          ui.button("Cambiar periodo", {
            variant: "ghost",
            icon: "funnel",
            onClick: () => {
              // En roles simplificados la tarjeta de filtros está oculta.
              const compactSel = $("periodo-compact");
              const period = compactSel && !compactSel.hidden ? compactSel : $("periodo");
              if (!period) return;
              period.scrollIntoView({ block: "center" });
              period.focus();
            },
          }),
        ),
      );
      panel.append(box);
      graficas.append(panel);
      continue;
    }

    const canvas = el("canvas");
    canvas.setAttribute("role", "img");
    canvas.setAttribute(
      "aria-label",
      `${heading}. Gráfica ${dash_tipoEs(tipo)}, serie ${etiqueta}. Usa «Ver datos» para consultar los valores en una tabla.`,
    );
    box.append(canvas);
    panel.append(box);

    // Alternativa accesible: tabla de datos tras un control de difusión (UX audit #8).
    const dataId = "dash-chart-data-" + chartIndex;
    const dataBox = el("div", "dash-chart-data");
    dataBox.id = dataId;
    dataBox.hidden = true;
    const datosGrafica = serie.labels.map((label, i) => ({ categoria: label, valor: serie.data[i] }));
    dataBox.append(
      ui.table(
        [{ key: "categoria", label: "Categoría" }, { key: "valor", label: etiqueta, align: "right" }],
        datosGrafica,
        { caption: `Datos de ${heading}`, empty: "Sin valores para mostrar." },
      ),
    );
    panel.append(dataBox);

    const toggle = ui.button("Ver datos", {
      variant: "ghost",
      icon: "table",
      onClick: () => {
        dataBox.hidden = !dataBox.hidden;
        toggle.setAttribute("aria-expanded", String(!dataBox.hidden));
      },
    });
    toggle.classList.add("btn--sm", "dash-chart-toggle");
    toggle.setAttribute("aria-controls", dataId);
    toggle.setAttribute("aria-expanded", "false");
    header.append(toggle);

    graficas.append(panel);

    const circular = tipo === "doughnut";
    const isLine = tipo === "line";
    charts.push(
      new Chart(canvas, {
        type: tipo,
        data: {
          labels: serie.labels,
          datasets: [
            {
              label: etiqueta,
              data: serie.data,
              ...(circular
                ? {
                    backgroundColor: serie.labels.map((_, i) => colorAt(i)),
                    borderColor: surfaceColor,
                    borderWidth: 2,
                    hoverOffset: 6,
                  }
                : isLine
                  ? {
                      borderColor: colorAt(1),
                      backgroundColor: colorAt(1) + "1f",
                      borderWidth: 2,
                      fill: true,
                      pointRadius: 3,
                      tension: 0.3,
                    }
                  : {
                      backgroundColor: colorAt(0),
                      borderRadius: 6,
                      borderSkipped: false,
                      maxBarThickness: 48,
                      borderWidth: 0,
                    }),
            },
          ],
        },
        options: {
          responsive: true,
          maintainAspectRatio: false,
          ...(circular ? { cutout: "62%" } : {}),
          plugins: {
            legend: circular
              ? {
                  display: true,
                  position: "bottom",
                  labels: {
                    usePointStyle: true,
                    pointStyle: "circle",
                    boxWidth: 8,
                    padding: 16,
                    font: { size: 12 },
                    // Doughnut keeps Chart.js' per-slice label generator (Chart.overrides),
                    // falling back to the generic per-dataset one.
                    generateLabels: (chart) => {
                      const base =
                        Chart.overrides?.[chart.config.type]?.plugins?.legend?.labels?.generateLabels ||
                        Chart.defaults.plugins.legend.labels.generateLabels;
                      return base(chart).map((l) => ({ ...l, text: short(l.text) }));
                    },
                  },
                }
              : { display: false },
            tooltip: {
              callbacks: {
                title: (items) =>
                  items.map((it) =>
                    String(it.chart.data.labels?.[it.dataIndex] ?? it.label ?? ""),
                  ),
              },
            },
          },
          scales: circular
            ? {}
            : {
                y: {
                  beginAtZero: true,
                  grid: { color: gridColor },
                  ticks: { precision: 0, color: tickColor },
                },
                x: {
                  grid: { display: false },
                  ticks: {
                    color: tickColor,
                    maxRotation: 0,
                    autoSkip: true,
                    callback(value) {
                      return short(this.getLabelForValue(value));
                    },
                  },
                },
              },
        },
      }),
    );
  }

  // ── Accesos rápidos (la tarjeta de bienvenida los reemplaza en roles simples) ──
  accesos.replaceChildren();
  let accesosCount = 0;
  for (const [target, text, iconName] of sinRol || simple ? [] : ACCESOS[data.rol]) {
    if (!["publicar", "reportes"].includes(target) && !resources.some((r) => r.table === target)) continue;
    const a = el("a", "btn btn--secondary btn--sm");
    a.href = "#" + target;
    a.append(icon(iconName), el("span", "", text));
    accesos.append(a);
    accesosCount += 1;
  }
  const accesosTitle = dash_sectionTitle(accesos, "dash-accesos-title", "Accesos rápidos");
  accesosTitle.hidden = !accesosCount;
  accesos.hidden = !accesosCount;
  accesos.setAttribute("role", "navigation");
  accesos.setAttribute("aria-labelledby", "dash-accesos-title");
}
