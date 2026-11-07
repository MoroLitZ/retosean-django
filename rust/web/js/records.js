"use strict";
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
  retos: [["titulo", "Título"], ["empresa", "Empresa"], ["estado", "Estado"], ["codigo", "Consecutivo"], ["actualizado_en", "Actualizado"]],
  "retos:EMPRESA": [["titulo", "Título"], ["tipo", "Tipo"], ["estado", "Estado"], ["area", "Área"], ["actualizado_en", "Actualizado"]],
  "retos:ESTUDIANTE": [["titulo", "Título"], ["empresa", "Empresa"], ["area", "Área"], ["estado", "Estado"], ["fecha_limite_postulacion", "Fecha límite"]],
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
  seguimientos_reto: [["fecha_sesion", "Fecha"], ["reto", "Reto"], ["tipo_sesion", "Sesión"], ["porcentaje_avance", "Avance"], ["observaciones", "Observaciones"], ["creado_por", "Creado por"]],
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
  if (selected === "tipos_documento_legal" && admin())
    action($("extra-panel"), "Nuevo tipo de documento", () => documentTypeForm(null));
  if (selected === "log_actividad" && admin())
    action($("extra-panel"), "Filtrar bitácora", () => activityLog({}));
  if (selected === "notificaciones" && admin())
    action($("extra-panel"), "Registro de envíos", () => deliveryLog(""));
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
