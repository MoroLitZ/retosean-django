"use strict";
const ROLES_EQUIPO = [["PROFESOR", "Profesor"], ["ESTUDIANTE", "Estudiante"], ["EXPERTO", "Experto invitado"]];
const TIPOS_SESION = [["inicio", "Inicio"], ["seguimiento", "Seguimiento"], ["preseleccion", "Preselección"], ["evaluacion", "Evaluación"]];

/* ── Utilidades locales del área de flujos (prefijo wf_) ── */

// Colombia no observa horario de verano: UTC-5 todo el año.
function wf_bogotaISO(value) {
  const base = String(value);
  const withSeconds = base.length === 16 ? base + ":00" : base;
  return new Date(withSeconds + "-05:00").toISOString();
}
function wf_localInputValue(value) {
  const d = new Date(value);
  if (Number.isNaN(d.getTime())) return "";
  return d.toLocaleString("sv-SE", { timeZone: "America/Bogota" }).slice(0, 16).replace(" ", "T");
}
function wf_formatSize(bytes) {
  if (bytes >= 1048576) return (bytes / 1048576).toFixed(1) + " MB";
  return Math.max(1, Math.round(bytes / 1024)) + " KB";
}
// Adjuntos de retos y seguimientos: los mismos tipos que admite el servidor
// (media::attachment_extension, 50 MB por archivo) para no subir nada que
// luego se rechace (FEB-6).
const WF_ACCEPT_ADJUNTOS = ".pdf,.png,.jpg,.jpeg,.txt,.xlsx,.zip,.docx,.pptx,.csv";
// Extensión del archivo contra la lista `accept` del input (".pdf,.png" o MIME).
function wf_extOk(input, file) {
  const exts = (input.accept || "")
    .split(",")
    .map((s) => s.trim().replace(/^\./, ""))
    .filter((e) => e && !e.includes("/"));
  if (!exts.length) return true;
  const ext = String(file.name.split(".").pop() || "").toLowerCase();
  return exts.some((e) => e.toLowerCase() === ext);
}
function wf_section(title, iconName) {
  const h = el("h3", "wf-section");
  if (iconName) h.append(icon(iconName));
  h.append(el("span", "", title));
  return h;
}
// Validación en línea (audit #4): errores bajo cada campo + foco en el primero.
function wf_showFieldError(field, message) {
  const wrap = field.closest(".field") || field.parentElement;
  if (!wrap) return;
  let err = wrap.querySelector(":scope > .field__error");
  if (!err) {
    err = el("p", "field__error");
    err.setAttribute("aria-live", "polite");
    wrap.append(err);
  }
  if (field.id) err.id = field.id + "-error";
  err.replaceChildren(icon("exclamation-circle-fill"), el("span", "", message));
  field.setAttribute("aria-invalid", "true");
  if (err.id) field.setAttribute("aria-describedby", err.id);
}
function wf_clearFieldError(field) {
  field.removeAttribute("aria-invalid");
  const wrap = field.closest(".field") || field.parentElement;
  const err = wrap?.querySelector(":scope > .field__error");
  if (err) err.replaceChildren();
}
function wf_clearValidation(form) {
  for (const f of form.querySelectorAll("input,select,textarea")) wf_clearFieldError(f);
}
function wf_validate(form) {
  wf_clearValidation(form);
  const invalid = [...form.querySelectorAll("input,select,textarea")].filter((f) => !f.disabled && !f.checkValidity());
  if (!invalid.length) return true;
  for (const f of invalid) wf_showFieldError(f, f.validationMessage || "Revisa este campo.");
  invalid[0].focus();
  return false;
}
function wf_attachValidation(form) {
  if (!form || form.dataset.wfValidated) return;
  form.dataset.wfValidated = "1";
  form.noValidate = true;
  form.addEventListener("input", (e) => {
    if (e.target.matches?.("input,select,textarea")) wf_clearFieldError(e.target);
  }, true);
  form.addEventListener("change", (e) => {
    if (e.target.matches?.('select,input[type="file"]')) wf_clearFieldError(e.target);
  }, true);
}
// Archivos (audit #6): resumen de la selección, tamaño máximo y barra de progreso.
function wf_fileSummary(fileInput, maxMB = 50) {
  const list = el("ul", "wf-files");
  list.setAttribute("aria-live", "polite");
  fileInput.after(list);
  if (fileInput.form) wf_attachValidation(fileInput.form);
  const render = () => {
    wf_clearFieldError(fileInput);
    list.replaceChildren();
    for (const file of fileInput.files) {
      const li = el("li", "", `${file.name} · ${wf_formatSize(file.size)}`);
      if (file.size > maxMB * 1048576) {
        li.classList.add("text-danger");
        wf_showFieldError(fileInput, `“${file.name}” supera el máximo de ${maxMB} MB.`);
      } else if (!wf_extOk(fileInput, file)) {
        li.classList.add("text-danger");
        wf_showFieldError(fileInput, `“${file.name}”: formato no admitido.`);
      }
      list.append(li);
    }
  };
  fileInput.addEventListener("change", render);
  render();
  return list;
}
function wf_checkFiles(form, maxMB = 50) {
  let ok = true;
  for (const f of form.querySelectorAll('input[type="file"]'))
    for (const file of f.files) {
      if (file.size > maxMB * 1048576) {
        wf_showFieldError(f, `“${file.name}” supera el máximo de ${maxMB} MB.`);
        ok = false;
      } else if (!wf_extOk(f, file)) {
        wf_showFieldError(f, `“${file.name}”: formato no admitido (se acepta: ${f.accept}).`);
        ok = false;
      }
    }
  if (!ok) toast("Hay archivos inválidos: superan el tamaño o tienen un tipo no admitido. Revisa los campos marcados.", "error");
  return ok;
}
function wf_progressBar(anchor) {
  const node = el("div", "progress wf-upload-progress");
  node.setAttribute("role", "progressbar");
  node.setAttribute("aria-label", "Progreso de la carga");
  node.setAttribute("aria-valuemin", "0");
  node.setAttribute("aria-valuemax", "100");
  node.setAttribute("aria-valuenow", "0");
  node.hidden = true;
  const bar = el("div", "progress__bar");
  node.append(bar);
  if (anchor) anchor.after(node);
  return {
    set(pct) {
      node.hidden = false;
      bar.style.width = pct + "%";
      node.setAttribute("aria-valuenow", String(pct));
    },
    remove() {
      node.remove();
    },
  };
}
function wf_uploadXhr(url, formData, onProgress) {
  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    xhr.open("POST", url);
    xhr.setRequestHeader("X-CSRF-Token", csrf || "");
    xhr.upload.onprogress = (e) => {
      if (e.lengthComputable) onProgress(Math.round((e.loaded / e.total) * 100));
    };
    xhr.onload = () => resolve({ status: xhr.status, text: xhr.responseText });
    xhr.onerror = () => reject(new Error("No se pudo conectar con el servidor."));
    xhr.send(formData);
  });
}
const WF_ESTADOS_LOGISTICA_PILL = { SOLICITADA: "warning", CONFIRMADA: "success", DESCARTADA: "danger" };
function wf_estadoLogistica(estado) {
  return ui.badge(WF_ESTADOS_LOGISTICA_PILL[estado] || "info", { label: label(ESTADOS_LOGISTICA, estado) });
}
// Esqueleto inicial en el diálogo mientras llegan los datos (audit #1).
function wf_modalLoading(title, rows = 5) {
  const parent = modal(title);
  parent.setAttribute("aria-busy", "true");
  parent.append(ui.skeleton(rows, { kind: "row" }));
  return parent;
}
function wf_modalFailed(parent, error) {
  parent.removeAttribute("aria-busy");
  $("detail-dialog").close();
  throw error;
}

function cell(tag, text) {
  return el(tag, "", text);
}
// Tabla genérica del área: acepta texto o nodos (p. ej. ui.badge) por celda.
function table(headers, rows, opts = {}) {
  const { emptyText = "Sin registros para mostrar.", caption = "" } = opts;
  const t = el("table", "data-table wf-table");
  if (caption) t.append(el("caption", "", caption));
  const head = el("tr");
  headers.forEach((h) => {
    const th = cell("th", h);
    th.setAttribute("scope", "col");
    head.append(th);
  });
  const thead = el("thead");
  thead.append(head);
  const body = el("tbody");
  rows.forEach((r) => {
    const tr = el("tr");
    r.forEach((v) => {
      const td = el("td");
      if (v instanceof Node) td.append(v);
      else td.textContent = v === "" || v == null ? "—" : String(v);
      tr.append(td);
    });
    body.append(tr);
  });
  if (!rows.length) {
    const tr = el("tr");
    const td = el("td");
    td.colSpan = headers.length;
    const box = el("div", "empty");
    box.append(el("div", "empty__title", emptyText));
    td.append(box);
    tr.append(td);
    body.append(tr);
  }
  t.append(thead, body);
  const wrap = el("div", "table-wrap");
  wrap.append(t);
  return wrap;
}
function label(list, value) {
  return (list.find((x) => x[0] === value) || [value, value])[1];
}
function wf_nombreParticipante(p) {
  const box = el("span", "wf-cell-nombre", p.nombre);
  if (p.cuenta) {
    const icn = icon("check2-circle");
    icn.setAttribute("role", "img");
    icn.setAttribute("aria-label", "Cuenta registrada");
    icn.removeAttribute("aria-hidden");
    box.append(icn, el("span", "visually-hidden", "(cuenta registrada)"));
  }
  return box;
}
function teamTables(team) {
  const box = el("div", "mb-3");
  box.append(
    wf_section("Equipo", "people"),
    table(["Rol", "Nombre", "Cédula / documento", "Correo", "Facultad / programa"],
      team.participantes.map((p) => [label(ROLES_EQUIPO, p.rol), wf_nombreParticipante(p), p.documento, p.email,
        p.rol === "ESTUDIANTE" ? p.programa : p.facultad]),
      { emptyText: "Aún no hay participantes en el equipo." }),
    wf_section("Cronograma de sesiones", "calendar3"),
    table(["Tipo", "Fecha y hora", "Enlace", "Observaciones"],
      team.sesiones.map((s) => [label(TIPOS_SESION, s.tipo), fecha(s.fecha_hora, true), s.enlace_reunion, s.observaciones]),
      { emptyText: "Sin sesiones programadas." }));
  return box;
}
function editableRows(parent, title, columns, rows) {
  parent.append(wf_section(title));
  const t = el("table", "data-table wf-table wf-table--editor");
  const head = el("tr");
  [...columns.map((c) => c.label), "Quitar"].forEach((h) => {
    const th = cell("th", h);
    th.setAttribute("scope", "col");
    head.append(th);
  });
  const thead = el("thead");
  thead.append(head);
  const body = el("tbody");
  t.append(thead, body);
  const addRow = (values = {}) => {
    const tr = el("tr");
    for (const c of columns) {
      const td = el("td");
      let field;
      if (c.options) {
        field = el("select", "select input--sm");
        c.options.forEach(([v, l]) => field.append(new Option(l, v)));
      } else {
        field = el("input", "input input--sm");
        field.type = c.type || "text";
      }
      field.name = c.name;
      field.setAttribute("aria-label", c.label);
      let value = values[c.name] ?? "";
      if (c.type === "datetime-local" && value) value = wf_localInputValue(value);
      field.value = value;
      td.append(field);
      tr.append(td);
    }
    const remove = el("button", "icon-btn icon-btn--sm");
    remove.type = "button";
    remove.title = "Quitar fila";
    remove.setAttribute("aria-label", "Quitar fila");
    remove.append(icon("x"));
    remove.onclick = () => tr.remove();
    const td = el("td");
    td.append(remove);
    tr.append(td);
    body.append(tr);
  };
  rows.forEach(addRow);
  const wrap = el("div", "table-wrap");
  wrap.append(t);
  const add = ui.button("Agregar fila", {
    variant: "secondary",
    icon: "plus-lg",
    onClick: () => addRow(),
  });
  add.classList.add("btn--sm", "mt-2");
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
      .map((s) => ({ ...s, fecha_hora: wf_bogotaISO(s.fecha_hora) })),
  });
}
function submitForm(form, button, path, collect, done) {
  wf_attachValidation(form);
  form.onsubmit = async (e) => {
    e.preventDefault();
    if (!wf_validate(form)) return;
    try {
      await busy(button, mutate(path, collect()));
      if (done) done();
    } catch (error) {
      toast(error.message);
    }
  };
}
async function integrationForm(retoId, record = null, faculty = null) {
  const current = faculty ?? record?.facultad ?? "";
  const parent = wf_modalLoading(record ? "Editar integración académica" : "Integración académica", 6);
  let options;
  try {
    options = await api(`/api/integraciones/opciones?facultad=${encodeURIComponent(current)}`);
  } catch (error) {
    wf_modalFailed(parent, error);
  }
  parent.removeAttribute("aria-busy");
  parent.replaceChildren();
  const form = el("form", "wf-form");
  const grid = el("div", "wf-grid");
  // IntegracionAcademicaForm: catalog selects; a stored older value stays selectable.
  const withCurrent = (list, value) => [["", "Seleccione"], ...list.map((v) => [v, v]), ...(value && !list.includes(value) ? [[value, `${value} (actual)`]] : [])];
  const facultySelect = choiceSelect(grid, "facultad", "Facultad", withCurrent(options.facultades, record?.facultad), current);
  // Only the study units depend on the faculty: refresh that select and keep
  // everything else the professor already typed.
  facultySelect.onchange = async () => {
    try {
      const o = await api(`/api/integraciones/opciones?facultad=${encodeURIComponent(facultySelect.value)}`);
      grid.querySelector("[name=unidad_estudio]").replaceChildren(
        new Option("Seleccione una unidad de estudio", ""),
        ...o.unidades.map((u) => new Option(u.label, String(u.value))),
      );
    } catch (error) {
      toast(error.message);
    }
  };
  choiceSelect(grid, "nivel_formacion", "Nivel de formación", withCurrent(options.niveles, record?.nivel_formacion), record?.nivel_formacion || "");
  choiceSelect(grid, "unidad_estudio", "Unidad de estudio", [["", "Seleccione una unidad de estudio"], ...options.unidades.map((u) => [String(u.value), u.label])]);
  choiceSelect(grid, "ecosistema", "Ecosistema", withCurrent(options.ecosistemas, record?.ecosistema), record?.ecosistema || "");
  input(grid, "descripcion", "Descripción", record?.descripcion || "", "textarea");
  input(grid, "alcance", "Alcance", record?.alcance || "", "textarea");
  input(grid, "entregable_esperado", "Entregable esperado", record?.entregable_esperado || "").maxLength = 180;
  input(grid, "espacio_fisico", "Espacio físico", record?.espacio_fisico || "").maxLength = 180;
  input(grid, "cronograma_sesiones", "Notas del cronograma", record?.cronograma_sesiones || "", "textarea");
  input(grid, "requerimientos_empresa", "Requerimientos a la empresa", record?.requerimientos_empresa || "", "textarea");
  input(grid, "requerimientos_internos", "Requerimientos internos", record?.requerimientos_internos || "", "textarea");
  form.append(grid);
  const collectTeam = record ? () => ({}) : teamEditor(form);
  const collect = (accion) => {
    const data = Object.fromEntries([...grid.querySelectorAll("input,textarea,select")].map((f) => [f.name, f.value]));
    data.unidad_estudio = data.unidad_estudio ? Number(data.unidad_estudio) : null;
    if (record && !data.unidad_estudio) data.programa_academico = record.programa_academico || "";
    return { ...data, ...collectTeam(), accion };
  };
  const url = record ? `/api/integraciones/${record.id}` : `/api/retos/${retoId}/integrar`;
  const save = ui.button("Guardar borrador", { variant: "secondary", icon: "clock-history" });
  const send = ui.button("Guardar y enviar a revisión", { variant: "primary", icon: "send" });
  for (const [button, accion] of [[save, "guardar"], [send, "enviar"]])
    button.onclick = async () => {
      try {
        const result = await busy(button, api(url, collect(accion)));
        $("detail-dialog").close();
        toast(result.estado === "en_revision" ? "Integración enviada a aprobación del administrador." : "Integración académica guardada como borrador.");
        await loadRecords();
      } catch (error) {
        toast(error.message);
      }
    };
  const buttons = el("div", "wf-actions");
  buttons.append(save, send);
  form.append(buttons);
  parent.append(form);
}
// SeguimientoRetoForm: session type, progress and optional files.
function progressForm(retoId) {
  const parent = modal("Seguimiento");
  const form = el("form", "wf-form wf-grid");
  choiceSelect(form, "tipo_sesion", "Tipo de sesión", [["seguimiento", "Seguimiento"], ["inicio", "Inicio"], ["preseleccion", "Preselección"], ["evaluacion", "Evaluación / reconocimiento"], ["otro", "Otro"]], "seguimiento");
  input(form, "fecha_sesion", "Fecha", new Date().toLocaleDateString("en-CA", { timeZone: "America/Bogota" }), "date");
  const pct = input(form, "porcentaje_avance", "Avance (%)", "0", "number");
  pct.min = 0;
  pct.max = 100;
  input(form, "avances", "Avances", "", "textarea");
  input(form, "observaciones", "Observaciones", "", "textarea");
  input(form, "acuerdos", "Acuerdos", "", "textarea");
  const files = input(form, "archivos", "Archivos (opcional, máximo 50 MB cada uno)", "", "file");
  files.multiple = true;
  files.accept = WF_ACCEPT_ADJUNTOS;
  files.parentElement.classList.add("full");
  wf_fileSummary(files);
  const submit = ui.button("Registrar seguimiento", { variant: "primary", type: "submit", icon: "play-circle" });
  submit.classList.add("field--full");
  form.append(submit);
  wf_attachValidation(form);
  form.onsubmit = (e) => sendMultipart(e, form, `/api/retos/${retoId}/seguimiento`, "Seguimiento registrado.");
  parent.append(form);
}
function filesForm(url, title) {
  const parent = modal(title);
  const form = el("form", "wf-form wf-grid");
  const files = input(form, "archivos", "Archivos (uno o varios, máximo 50 MB cada uno)", "", "file", true);
  files.multiple = true;
  files.accept = WF_ACCEPT_ADJUNTOS;
  files.parentElement.classList.add("full");
  wf_fileSummary(files);
  const submit = ui.button("Cargar archivos", { variant: "primary", type: "submit", icon: "cloud-arrow-up" });
  submit.classList.add("field--full");
  form.append(submit);
  wf_attachValidation(form);
  form.onsubmit = (e) => sendMultipart(e, form, url, "Archivos cargados.");
  parent.append(form);
}
async function sendMultipart(e, form, url, done) {
  e.preventDefault();
  const button = form.querySelector("button");
  if (!wf_validate(form)) return;
  if (!wf_checkFiles(form)) return;
  wf_attachValidation(form);
  const progress = wf_progressBar(button);
  try {
    await busy(button, (async () => {
      const { status, text } = await wf_uploadXhr(url, new FormData(form), progress.set);
      const body = await readJson(new Response(status === 204 ? null : text, { status }));
      if (status < 200 || status >= 300) throw new Error(body.error || "No se pudo guardar");
      return body;
    })());
  } catch (error) {
    // Falló la subida: el usuario puede corregir y reintentar.
    toast(error.message);
    return;
  } finally {
    progress.remove();
  }
  $("detail-dialog").close();
  toast(done);
  try {
    await loadRecords();
  } catch {
    // La carga ya se guardó; el fallo es solo del refresco de la lista y no
    // debe invitar a subir de nuevo los mismos archivos (FEB-9).
    toast("La carga se guardó, pero no se pudo actualizar la lista.", "warning");
  }
}
function teamForm(integrationId, team) {
  const parent = modal("Equipo y cronograma");
  const form = el("form", "wf-form");
  const collectTeam = teamEditor(form, team);
  const button = ui.button("Guardar equipo y cronograma", { variant: "primary", type: "submit", icon: "check" });
  const actions = el("div", "wf-actions");
  actions.append(button);
  form.append(actions);
  submitForm(form, button, `/api/integraciones/${integrationId}/equipo`, collectTeam);
  parent.append(form);
}
async function scheduleForStudent() {
  const holder = el("div", "mt-4");
  holder.setAttribute("aria-busy", "true");
  holder.append(ui.skeleton(3, { kind: "row" }));
  $("extra-panel").append(holder);
  try {
    const data = await api("/api/records/sesiones_reto_academico?page=1&q=&limit=100");
    const panel = el("section", "surface");
    panel.append(el("header", "surface__header", "Cronograma de sesiones"));
    const rows = data.records
      .sort((a, b) => String(a.fecha_hora).localeCompare(String(b.fecha_hora)))
      .map((s) => [s.reto, label(TIPOS_SESION, s.tipo), fecha(s.fecha_hora, true), s.enlace_reunion || ""]);
    const box = el("div", "surface__body");
    box.append(table(["Reto", "Sesión", "Fecha y hora", "Enlace"], rows, { emptyText: "Aún no tienes sesiones programadas." }));
    panel.append(box);
    holder.replaceChildren(panel);
  } catch (error) {
    const note = el("div", "alert alert--danger small");
    note.setAttribute("role", "alert");
    note.append(icon("exclamation-triangle-fill"), el("span", "", `No se pudo cargar el cronograma: ${error.message}`));
    holder.replaceChildren(note);
    throw error;
  } finally {
    holder.setAttribute("aria-busy", "false");
  }
}

// ── HU09: event logistics (Django LogisticaForm + InvitadoFormSet) ──
const EVENTOS_LOGISTICA = [["INICIO", "Evento de inicio"], ["CIERRE", "Evento de cierre"]];
const TIPOS_LOGISTICA = [["ESPACIO", "Espacio físico"], ["COMUNICACIONES", "Apoyo del área de comunicaciones"], ["REFRIGERIOS", "Refrigerios / catering"], ["EQUIPOS_AV", "Equipos audiovisuales"], ["INVITADOS", "Invitados"], ["OTRO", "Otro"]];
const ESTADOS_LOGISTICA = [["SOLICITADA", "Solicitada"], ["CONFIRMADA", "Confirmada"], ["DESCARTADA", "Descartada"]];
const IMPACTO = [["", "Sin registrar"], ["SI", "Sí, la aplicó"], ["NO", "No la aplicó"], ["EN_EVALUACION", "En evaluación"]];
function choiceSelect(parent, name, text, choices, value = "") {
  const wrap = el("div", "field");
  const l = el("label", "field__label", text);
  l.htmlFor = "field-" + name;
  const select = el("select", "select");
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
  const parent = wf_modalLoading("Gestión del cierre", 8);
  let data;
  try {
    data = await api(`/api/retos/${id}/cierre`);
  } catch (error) {
    wf_modalFailed(parent, error);
  }
  parent.removeAttribute("aria-busy");
  parent.replaceChildren();
  const reload = async (message) => {
    toast(message);
    await closurePanel(id);
  };
  const sendForm = async (form, url) => {
    const response = await fetch(url, { method: "POST", headers: { "X-CSRF-Token": csrf }, body: new FormData(form) });
    const body = await readJson(response);
    if (!response.ok) throw new Error(body.error || "No se pudo guardar");
    return body;
  };
  const guard = (form, handler) => {
    wf_attachValidation(form);
    const button = form.querySelector("button");
    form.onsubmit = async (e) => {
      e.preventDefault();
      if (!wf_validate(form)) return;
      // El cuerpo de las rutas de cierre admite 10 MB en total: los archivos
      // se validan (tipo y tamaño) antes de subirlos (FEB-6).
      if (!wf_checkFiles(form, 9)) return;
      try {
        await busy(button, handler());
      } catch (error) {
        toast(error.message);
      }
    };
  };

  parent.append(wf_section("Agenda y acta", "calendar3"));
  const agenda = el("form", "wf-form wf-grid");
  input(agenda, "fecha_hora", "Fecha y hora", data.agenda.fecha_hora || "", "datetime-local");
  input(agenda, "espacio", "Espacio", data.agenda.espacio || "");
  input(agenda, "recursos", "Recursos", data.agenda.recursos || "", "textarea");
  input(agenda, "invitados", "Invitados", data.agenda.invitados || "", "textarea");
  input(agenda, "agenda", "Agenda", data.agenda.agenda || "", "textarea");
  input(agenda, "notas", "Notas", data.cierre.notas || "", "textarea");
  choiceSelect(agenda, "impacto", "¿La empresa aplicó la solución?", IMPACTO, data.cierre.impacto || "");
  const acta = input(agenda, "acta_url", data.cierre.acta ? "Reemplazar acta (PDF)" : "Acta de cierre (PDF)", "", "file");
  acta.accept = ".pdf";
  acta.parentElement.classList.add("full");
  wf_fileSummary(acta, 9);
  if (data.cierre.acta) {
    const a = el("a", "wf-link small full", "Descargar acta actual");
    a.prepend(icon("download"));
    a.href = `/api/archivo/cierres_reto/${data.cierre.id}/acta_url`;
    agenda.append(a);
  }
  const agendaSubmit = ui.button("Guardar documentación y agenda", { variant: "primary", type: "submit", icon: "check" });
  agendaSubmit.classList.add("field--full");
  agenda.append(agendaSubmit);
  guard(agenda, async () => reload((await sendForm(agenda, `/api/retos/${id}/cierre`)).mensaje));
  parent.append(agenda);

  parent.append(wf_section("Entregables finales", "folder-check"));
  parent.append(table(["Nombre", "Cargado por", "Fecha"], data.entregables_finales.map((f) => [f.nombre, f.cargado_por, fecha(f.creado_en, true)]),
    { emptyText: "Aún no hay entregables finales cargados." }));
  const artifact = el("form", "wf-form wf-grid");
  input(artifact, "nombre", "Nombre", "", "text", true);
  const artifactFile = input(artifact, "archivo", "Archivo (PDF, PNG o JPG)", "", "file", true);
  artifactFile.accept = ".pdf,.png,.jpg,.jpeg";
  artifactFile.parentElement.classList.add("full");
  wf_fileSummary(artifactFile, 9);
  const artifactSubmit = ui.button("Cargar entregable final", { variant: "secondary", type: "submit", icon: "cloud-arrow-up" });
  artifactSubmit.classList.add("field--full");
  artifact.append(artifactSubmit);
  guard(artifact, async () => reload((await sendForm(artifact, `/api/retos/${id}/cierre/entregables`)).mensaje));
  parent.append(artifact);

  parent.append(wf_section("Reconocimientos", "award"));
  parent.append(table(["Destinatario", "Descripción"], data.reconocimientos.map((r) => [r.nombre_destinatario, r.descripcion]),
    { emptyText: "Sin reconocimientos registrados." }));
  const recognition = el("form", "wf-form wf-grid");
  choiceSelect(recognition, "usuario_id", "Estudiante (opcional)", [["", "Sin vincular"], ...data.estudiantes.map((s) => [String(s.value), s.label])]);
  input(recognition, "nombre_destinatario", "Destinatario", "", "text", true);
  input(recognition, "descripcion", "Descripción", "", "textarea", true);
  const recognitionSubmit = ui.button("Registrar reconocimiento", { variant: "secondary", type: "submit", icon: "award" });
  recognitionSubmit.classList.add("field--full");
  recognition.append(recognitionSubmit);
  guard(recognition, async () => {
    const values = Object.fromEntries(new FormData(recognition));
    values.usuario_id = values.usuario_id ? Number(values.usuario_id) : null;
    reload((await api(`/api/retos/${id}/cierre/reconocimientos`, values)).mensaje);
  });
  parent.append(recognition);

  parent.append(wf_section("Solicitudes de logística", "list"));
  const rows = el("div");
  rows.append(table(["Evento", "Tipo", "Detalle", "Estado", "Solicitada por"],
    data.solicitudes_logistica.map((r) => [label(EVENTOS_LOGISTICA, r.evento), label(TIPOS_LOGISTICA, r.tipo), r.descripcion, wf_estadoLogistica(r.estado), r.solicitada_por]),
    { emptyText: "Sin solicitudes de logística." }));
  if (admin())
    for (const r of data.solicitudes_logistica) {
      const next = r.estado === "SOLICITADA" ? [["CONFIRMADA", "Confirmar"], ["DESCARTADA", "Descartar"]] : [["SOLICITADA", "Reabrir"]];
      const line = el("div", "wf-actions mt-0 mb-1 small");
      line.append(el("span", "me-2", `${label(TIPOS_LOGISTICA, r.tipo)}:`));
      for (const [estado, text] of next)
        action(line, text, async () => {
          if (estado === "DESCARTADA") {
            const ok = await ui.confirm(`Se descartará la solicitud de ${label(TIPOS_LOGISTICA, r.tipo).toLowerCase()} del ${label(EVENTOS_LOGISTICA, r.evento).toLowerCase()}.`,
              { title: "Descartar solicitud", confirmText: "Descartar", variant: "danger" });
            if (!ok) return;
          }
          await api(`/api/solicitudes-logistica/${r.id}/estado`, { estado });
          await reload(`Solicitud marcada como ${label(ESTADOS_LOGISTICA, estado).toLowerCase()}.`);
        });
      rows.append(line);
    }
  parent.append(rows);
  const logistics = el("form", "wf-form wf-grid");
  choiceSelect(logistics, "evento", "Evento", EVENTOS_LOGISTICA);
  choiceSelect(logistics, "tipo", "Tipo de solicitud", TIPOS_LOGISTICA);
  const detail = input(logistics, "descripcion", "Detalle", "", "textarea", true);
  detail.placeholder = "Ej.: auditorio para 80 personas, 9:00 a 12:00";
  const logisticsSubmit = ui.button("Solicitar logística", { variant: "secondary", type: "submit", icon: "send" });
  logisticsSubmit.classList.add("field--full");
  logistics.append(logisticsSubmit);
  guard(logistics, async () => reload((await api(`/api/retos/${id}/solicitudes-logistica`, Object.fromEntries(new FormData(logistics)))).mensaje));
  parent.append(logistics);
}
const MODALIDADES = [["presencial", "Presencial"], ["virtual", "Virtual"], ["hibrida", "Híbrida"]];
const TIPOS_INVITADO = [["INVITADO", "Invitado"], ["MENTOR", "Mentor"], ["PONENTE", "Ponente"], ["PATROCINADOR", "Patrocinador"]];
function logisticsForm(id, event) {
  const parent = modal("Logística e invitados");
  const form = el("form", "wf-form");
  const grid = el("div", "wf-grid");
  choiceSelect(grid, "modalidad", "Modalidad", MODALIDADES, event.modalidad);
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
  const button = ui.button("Guardar logística", { variant: "primary", type: "submit", icon: "check" });
  const actions = el("div", "wf-actions");
  actions.append(button);
  form.append(actions);
  submitForm(form, button, `/api/hackathons/${id}/logistica`, () => ({
    ...Object.fromEntries([...grid.querySelectorAll("input,textarea,select")].map((f) => [f.name, f.value])),
    invitados: guests().filter((g) => g.nombre).map((g) => ({ ...g, confirmado: g.confirmado === "true" })),
  }));
  parent.append(form);
}

// ── Public pages: registration and password recovery (Django usuarios views) ──
