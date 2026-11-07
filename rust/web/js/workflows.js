"use strict";
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
async function integrationForm(retoId, record = null, faculty = null) {
  const current = faculty ?? record?.facultad ?? "";
  const options = await api(`/api/integraciones/opciones?facultad=${encodeURIComponent(current)}`);
  const parent = modal(record ? "Editar integración académica" : "Integración académica");
  const form = el("form");
  const grid = el("div", "form-grid");
  // IntegracionAcademicaForm: catalog selects; a stored older value stays selectable.
  const withCurrent = (list, value) => [["", "Selecciona"], ...list.map((v) => [v, v]), ...(value && !list.includes(value) ? [[value, `${value} (actual)`]] : [])];
  const facultySelect = choiceSelect(grid, "facultad", "Facultad", withCurrent(options.facultades, record?.facultad), current);
  // Only the study units depend on the faculty: refresh that select and keep
  // everything else the professor already typed.
  facultySelect.onchange = async () => {
    try {
      const o = await api(`/api/integraciones/opciones?facultad=${encodeURIComponent(facultySelect.value)}`);
      grid.querySelector("[name=unidad_estudio]").replaceChildren(
        new Option("Selecciona una unidad de estudio", ""),
        ...o.unidades.map((u) => new Option(u.label, String(u.value))),
      );
    } catch (error) {
      toast(error.message);
    }
  };
  choiceSelect(grid, "nivel_formacion", "Nivel de formación", withCurrent(options.niveles, record?.nivel_formacion), record?.nivel_formacion || "");
  choiceSelect(grid, "unidad_estudio", "Unidad de estudio", [["", "Selecciona una unidad de estudio"], ...options.unidades.map((u) => [String(u.value), u.label])]);
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
  const save = el("button", "secondary", "Guardar borrador");
  const send = el("button", "primary", "Guardar y enviar a revisión");
  save.type = send.type = "button";
  for (const [button, accion] of [[save, "guardar"], [send, "enviar"]])
    button.onclick = async () => {
      button.disabled = true;
      try {
        const result = await api(url, collect(accion));
        $("detail-dialog").close();
        toast(result.estado === "en_revision" ? "Integración enviada a aprobación del administrador." : "Integración académica guardada como borrador.");
        await loadRecords();
      } catch (error) {
        toast(error.message);
      } finally {
        button.disabled = false;
      }
    };
  const buttons = el("div", "actions mt-3");
  buttons.append(save, send);
  form.append(buttons);
  parent.append(form);
}
// SeguimientoRetoForm: session type, progress and optional files.
function progressForm(retoId) {
  const parent = modal("Seguimiento");
  const form = el("form", "form-grid");
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
  form.append(el("button", "primary full", "Registrar seguimiento"));
  form.onsubmit = (e) => sendMultipart(e, form, `/api/retos/${retoId}/seguimiento`, "Seguimiento registrado.");
  parent.append(form);
}
function filesForm(url, title) {
  const parent = modal(title);
  const form = el("form", "form-grid");
  const files = input(form, "archivos", "Archivos (uno o varios, máximo 50 MB cada uno)", "", "file", true);
  files.multiple = true;
  form.append(el("button", "primary full", "Cargar archivos"));
  form.onsubmit = (e) => sendMultipart(e, form, url, "Archivos cargados.");
  parent.append(form);
}
async function sendMultipart(e, form, url, done) {
  e.preventDefault();
  const button = form.querySelector("button");
  button.disabled = true;
  try {
    const response = await fetch(url, { method: "POST", headers: { "X-CSRF-Token": csrf }, body: new FormData(form) });
    const data = await readJson(response);
    if (!response.ok) throw new Error(data.error || "No se pudo guardar");
    $("detail-dialog").close();
    toast(done);
    await loadRecords();
  } catch (error) {
    toast(error.message);
  } finally {
    button.disabled = false;
  }
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
    const body = await readJson(response);
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
