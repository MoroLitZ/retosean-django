"use strict";
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
    if (!response.ok) throw new Error((await readJson(response)).error || "No se pudo generar el reporte");
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
