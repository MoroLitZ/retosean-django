"use strict";
// ── HU06: report builder (Django reportes/constructor.html) ──
const PERIODOS_REPORTE = [["30d", "Últimos 30 días"], ["90d", "Últimos 90 días"], ["anio", "Último año"], ["todo", "Histórico completo"], ["custom", "Rango personalizado"]];
const FILTROS_REPORTE = {
  facultad: ["Facultad", "Todas"],
  programa: ["Programa", "Todos"],
  empresa: ["Empresa", "Todas"],
  estado: ["Estado del reto", "Todos"],
  estado_postulacion: ["Estado de postulación", "Todos"],
  estado_empresa: ["Estado de la empresa", "Todos"],
};
let builderData = null;
async function loadBuilder() {
  const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
  const pares = (list) => list.map(([value, label]) => ({ value, label }));

  const panel = $("builder-panel");
  panel.className = "stack";
  panel.setAttribute("aria-busy", "true");
  const loading = el("section", "surface");
  const loadingBody = el("div", "surface__body");
  loadingBody.append(ui.skeleton(1, { kind: "title" }), ui.skeleton(6, { kind: "row" }));
  loading.append(loadingBody);
  panel.replaceChildren(loading);
  try {
    builderData = builderData || (await api("/api/reportes/catalogo"));
  } catch (error) {
    panel.replaceChildren();
    throw error;
  } finally {
    panel.setAttribute("aria-busy", "false");
  }
  panel.replaceChildren();

  const head = el("div", "builder-head");
  const titles = el("div", "stack stack--sm");
  titles.append(
    el("h2", "builder-title", "Constructor de reportes"),
    el("p", "builder-subtitle", "Elige el reporte, ajusta el periodo, los filtros y las variables, revisa la vista previa y descárgalo o envíalo por correo."),
  );
  head.append(titles);
  if (resources.some((r) => r.table === "reportes_generados")) {
    const history = el("a", "btn btn--secondary btn--sm");
    history.href = "#reportes_generados";
    history.append(icon("clock-history"), el("span", "", "Historial"));
    head.append(history);
  }

  const form = el("form");
  form.noValidate = true;

  const reporteField = ui.field({
    name: "reporte",
    label: "Reporte",
    type: "select",
    options: pares(builderData.reportes.map((r) => [r.clave, r.nombre])),
  });
  const reporte = reporteField.querySelector("select");
  const periodoField = ui.field({
    name: "periodo",
    label: "Periodo",
    type: "select",
    options: pares(PERIODOS_REPORTE),
    value: "90d",
  });
  const periodo = periodoField.querySelector("select");
  const basics = el("div", "builder-grid");
  basics.append(
    reporteField,
    ui.field({ name: "formato", label: "Formato", type: "select", options: pares([["PDF", "PDF"], ["EXCEL", "Excel"]]) }),
    periodoField,
  );

  const desdeField = ui.field({ name: "desde", label: "Desde", type: "date" });
  const hastaField = ui.field({ name: "hasta", label: "Hasta", type: "date" });
  const desde = desdeField.querySelector("input");
  const hasta = hastaField.querySelector("input");
  const range = el("div", "builder-grid");
  range.append(desdeField, hastaField);
  range.hidden = true;

  const destinatariosField = ui.field({
    name: "destinatarios",
    label: "Enviar a (correos separados por coma)",
    type: "text",
    placeholder: "Por defecto, tu correo",
  });
  const destinatarios = destinatariosField.querySelector("input");

  let attempted = false;
  const setFieldError = (control, message) => {
    const wrap = control.closest(".field");
    const err = wrap && wrap.querySelector(".field__error");
    if (err) {
      err.replaceChildren();
      if (message) err.append(icon("exclamation-circle-fill"), el("span", "", message));
    }
    if (message) control.setAttribute("aria-invalid", "true");
    else control.removeAttribute("aria-invalid");
  };
  const markRequired = (wrap, on) => {
    const control = wrap.querySelector(".input");
    if (control) control.required = on;
    const label = wrap.querySelector(".field__label");
    if (!label) return;
    const marker = label.querySelector(".req");
    if (on && !marker) label.append(el("span", "req", " *"));
    if (!on && marker) marker.remove();
  };
  const validate = (focusFirst = true) => {
    let invalid = null;
    const fail = (control, message) => {
      setFieldError(control, message);
      if (!invalid) invalid = control;
    };
    if (periodo.value === "custom") {
      if (!desde.value) fail(desde, "Indica la fecha de inicio.");
      else setFieldError(desde, "");
      if (!hasta.value) fail(hasta, "Indica la fecha final.");
      else if (desde.value && desde.value > hasta.value) fail(hasta, "La fecha final no puede ser anterior a la inicial.");
      else setFieldError(hasta, "");
    } else {
      setFieldError(desde, "");
      setFieldError(hasta, "");
    }
    const raw = destinatarios.value.trim();
    if (raw) {
      const parts = raw.split(",").map((s) => s.trim()).filter(Boolean);
      const bad = parts.find((p) => !EMAIL_RE.test(p));
      if (!parts.length) fail(destinatarios, "Indica al menos un correo.");
      else if (bad) fail(destinatarios, `«${bad}» no es un correo válido.`);
      else setFieldError(destinatarios, "");
    } else {
      setFieldError(destinatarios, "");
    }
    if (invalid && focusFirst) invalid.focus();
    return !invalid;
  };
  const revalidate = () => {
    if (attempted) validate(false);
  };
  const syncRange = () => {
    const custom = periodo.value === "custom";
    range.hidden = !custom;
    markRequired(desdeField, custom);
    markRequired(hastaField, custom);
    if (!custom) {
      setFieldError(desde, "");
      setFieldError(hasta, "");
    }
  };
  periodo.onchange = () => {
    syncRange();
    revalidate();
  };
  syncRange();
  desde.addEventListener("input", revalidate);
  hasta.addEventListener("input", revalidate);
  destinatarios.addEventListener("input", revalidate);

  const filtersTitle = el("h3", "builder-section");
  filtersTitle.id = "builder-filtros-title";
  filtersTitle.append(icon("funnel"), el("span", "", "Filtros"));
  const filters = el("div", "builder-grid");
  filters.setAttribute("role", "group");
  filters.setAttribute("aria-labelledby", filtersTitle.id);

  const columnsTitle = el("h3", "builder-section");
  columnsTitle.id = "builder-columnas-title";
  columnsTitle.append(icon("list"), el("span", "", "Variables a incluir"));
  const columnsHint = el("p", "builder-hint mb-2", "Si no eliges ninguna se incluyen todas.");
  columnsHint.id = "builder-columnas-hint";
  const columns = el("div", "builder-cols");
  columns.setAttribute("role", "group");
  columns.setAttribute("aria-labelledby", columnsTitle.id);
  columns.setAttribute("aria-describedby", columnsHint.id);

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
    if (!def.filtros.length)
      filters.append(el("p", "builder-hint", "Este reporte no admite filtros adicionales."));
    for (const name of def.filtros) {
      const [label, empty] = FILTROS_REPORTE[name];
      filters.append(ui.field({
        name,
        label,
        type: "select",
        options: [{ value: "", label: empty }, ...pares(filterOptions[name])],
      }));
    }
    columns.replaceChildren();
    if (!def.columnas.length)
      columns.append(el("p", "builder-hint", "Este reporte incluye siempre las mismas variables."));
    for (const [key, label] of def.columnas) {
      const wrap = el("div", "checkbox");
      const box = el("input", "checkbox__input");
      box.type = "checkbox";
      box.id = "builder-var-" + key;
      box.value = key;
      box.name = "columnas";
      const l = el("label", "checkbox__label", label);
      l.htmlFor = box.id;
      wrap.append(box, l);
      columns.append(wrap);
    }
  };
  reporte.onchange = draw;
  draw();

  const preview = el("div");
  preview.id = "builder-preview";
  preview.setAttribute("role", "region");
  preview.setAttribute("aria-label", "Vista previa del reporte");
  const previewStatus = el("p", "visually-hidden");
  previewStatus.setAttribute("role", "status");
  const previewContent = el("div");
  preview.append(previewStatus, previewContent);
  const previewEmpty = () => {
    const box = el("section", "surface");
    box.append(ui.empty("eye", "Sin vista previa todavía", "Configura el reporte y pulsa «Vista previa» para revisar los datos antes de generarlo."));
    return box;
  };
  previewContent.append(previewEmpty());

  const payload = () => {
    const data = Object.fromEntries(new FormData(form));
    data.columnas = [...form.querySelectorAll("input[name=columnas]:checked")].map((b) => b.value);
    for (const k of ["facultad", "programa", "empresa"]) data[k] = data[k] ? Number(data[k]) : null;
    for (const k of ["desde", "hasta"]) data[k] = data[k] || null;
    return data;
  };

  const actions = el("div", "cluster builder-actions");
  const add = (label, iconName, variant, handler) => {
    const b = ui.button(label, { variant, icon: iconName });
    b.addEventListener("click", () => {
      attempted = true;
      if (!validate()) return;
      busy(b, Promise.resolve().then(handler).catch((e) => toast(e.message, "error")));
    });
    actions.append(b);
    return b;
  };
  const previewButton = add("Vista previa", "eye", "secondary", async () => {
    previewStatus.textContent = "";
    const loadingBox = el("section", "surface");
    const loadingInner = el("div", "surface__body");
    loadingInner.append(ui.skeleton(1, { kind: "title" }), ui.skeleton(4, { kind: "row" }));
    loadingBox.append(loadingInner);
    previewContent.replaceChildren(loadingBox);
    let data;
    try {
      data = await api("/api/reportes/vista-previa", payload());
    } catch (error) {
      previewContent.replaceChildren(previewEmpty());
      throw error;
    }
    const box = el("section", "surface");
    const boxHead = el("header", "surface__header");
    boxHead.append(
      el("span", "", `Vista previa · ${data.nombre}`),
      ui.badge("neutral", { dot: false, label: `${data.mostradas} de ${data.total} registro${data.total === 1 ? "" : "s"}` }),
    );
    const inner = el("div", "surface__body");
    inner.append(el("p", "text-muted small mb-2", data.filtros));
    inner.append(ui.table(data.encabezados, data.filas, { empty: "Sin registros.", caption: "Primeras filas del reporte generado." }));
    if (data.total > data.mostradas)
      inner.append(el("p", "text-muted small mt-2 mb-0", `Se muestran las primeras ${data.mostradas} filas; el archivo incluye todas.`));
    box.append(boxHead, inner);
    previewContent.replaceChildren(box);
    previewStatus.textContent = `Vista previa lista: ${data.mostradas} de ${data.total} registro${data.total === 1 ? "" : "s"}.`;
  });
  add("Generar y descargar", "download", "primary", async () => {
    const response = await fetch("/api/reportes/generar", {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-CSRF-Token": csrf },
      body: JSON.stringify(payload()),
    });
    if (!response.ok) throw new Error((await readJson(response)).error || "No se pudo generar el reporte");
    const name = (response.headers.get("content-disposition") || "").split("filename=")[1]?.replaceAll('"', "") || "reporte";
    const url = URL.createObjectURL(await response.blob());
    const a = el("a");
    a.href = url;
    a.download = name;
    a.click();
    URL.revokeObjectURL(url);
  });
  add("Generar y enviar por correo", "envelope", "accent", async () => {
    const data = await api("/api/reportes/enviar", payload());
    toast(`Reporte enviado a ${data.destinatarios.join(", ")}.`, "success");
  });

  form.addEventListener("submit", (e) => {
    e.preventDefault();
    previewButton.click();
  });
  form.append(basics, range, filtersTitle, filters, columnsTitle, columnsHint, columns, destinatariosField, actions);

  const card = el("section", "surface");
  const cardBody = el("div", "surface__body");
  cardBody.append(form);
  card.append(cardBody);
  panel.append(head, card, preview);
}
