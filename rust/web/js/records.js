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
// Línea de ayuda en lenguaje sencillo bajo el título de cada listado,
// solo para los roles simplificados (ESTUDIANTE, PROFESOR, EMPRESA).
const PAGE_HELP = {
  ESTUDIANTE: {
    retos: "Aquí ves los retos abiertos de las empresas. Abre uno con «Ver detalle» para leerlo y postularte.",
    postulaciones: "El estado de cada reto al que te postulaste. Cuando te acepten, podrás subir tus entregables.",
    entregables: "Sube aquí los archivos que te piden en cada reto y consulta tu nota.",
    retos_favoritos: "Tus retos guardados. Ábrelos con «Ver detalle» para postularte cuando quieras.",
    cierre_encuestasatisfaccion: "Cuando termina un reto, cuenta aquí cómo te fue. Tu opinión ayuda a mejorar.",
    certificados: "Los certificados que has ganado por participar. Descárgalos desde «Ver detalle».",
    equipos_reto_academico: "Los equipos en los que participas y el reto de cada uno.",
    hackathons: "Los hackathones disponibles. Abre uno con «Ver detalle» para inscribir a tu equipo.",
    notificaciones: "Los avisos que te envía la plataforma. Abre uno para marcarlo como leído.",
  },
  PROFESOR: {
    integraciones_academicas: "Los retos que conectaste con tus cursos. Abre uno para ver su estado o editarlo.",
    retos: "Los retos de empresas que puedes llevar a tu clase. Abre uno y usa «Integrar a mi curso».",
    postulaciones: "Los estudiantes postulados a los retos de tus cursos. Acéptalos o recházalos desde «Ver detalle».",
    entregables: "Revisa y califica lo que entregaron tus estudiantes.",
    cierres_reto: "Los retos de tus cursos que ya se cerraron.",
    cierre_encuestasatisfaccion: "Cuando termina un reto, cuenta aquí cómo te fue. Tu opinión ayuda a mejorar.",
    unidades_estudio: "Las unidades de estudio disponibles para crear equipos académicos.",
    hackathons: "Los hackathones activos. Abre uno para ver su detalle o votar como jurado.",
    notificaciones: "Los avisos que te envía la plataforma. Abre uno para marcarlo como leído.",
  },
  EMPRESA: {
    retos: "Los retos que publicaste. Publica uno nuevo con el botón «Publicar Reto».",
    integraciones_academicas: "Los profesores que quieren usar tus retos en clase. Aprueba o rechaza desde «Ver detalle».",
    postulaciones: "Los estudiantes que se postularon a tus retos. Acéptalos o recházalos desde «Ver detalle».",
    documentos_empresas: "Sube los documentos legales que la universidad necesita para aprobar tu empresa.",
    seguimientos_reto: "El avance que profesores y estudiantes registran en tus retos.",
    cierres_reto: "Tus retos que ya terminaron y su cierre.",
    cierre_encuestasatisfaccion: "Cuando termina un reto, cuenta aquí cómo te fue. Tu opinión ayuda a mejorar.",
    hackathons: "Los hackathones activos. Abre uno para ver su detalle.",
    notificaciones: "Los avisos que te envía la plataforma. Abre uno para marcarlo como leído.",
  },
};
// Columnas que se muestran alineadas a la derecha con cifras tabulares.
const records_NUMERIC = [
  "monto",
  "monto_total",
  "nota",
  "horas",
  "orden",
  "vigencia_dias",
  "porcentaje_avance",
  "calificacion",
];
let columnHeaders = {};
function columns(records, table) {
  const spec = COLUMNS[`${table}:${role()}`] || COLUMNS[table];
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
// Token de carga: cada llamada lo incrementa; una respuesta que llega después
// de una llamada más reciente es obsoleta y se descarta sin tocar el DOM (FE-2).
let recordsSeq = 0;
// Filas fantasma que ocupan la tabla mientras se resuelve la consulta.
function records_skeletonRows(count = 6) {
  const head = $("table-head");
  const span = head && head.firstElementChild ? head.firstElementChild.children.length : 7;
  const frag = document.createDocumentFragment();
  for (let i = 0; i < count; i++) {
    const tr = el("tr", "records-skeleton-row");
    const td = el("td");
    td.colSpan = span;
    td.setAttribute("aria-hidden", "true");
    td.append(el("div", "skeleton skeleton--row"));
    tr.append(td);
    frag.append(tr);
  }
  return frag;
}
async function loadRecords() {
  const seq = ++recordsSeq;
  // Parámetros capturados antes del await: nunca releer los globales después,
  // o una navegación rápida pintaría filas viejas con columnas nuevas.
  const table = selected;
  const requestPage = page;
  const requestQuery = query;
  const tbl = $("table-head").closest("table");
  if (tbl && !tbl.querySelector("caption")) {
    tbl.prepend(el("caption", "visually-hidden", title(table)));
  }
  const body = $("table-body");
  // Línea de ayuda del listado: solo roles simplificados y solo si hay texto.
  const pageHelp = $("page-help");
  if (pageHelp) {
    const helpText = (PAGE_HELP[role()] || {})[table] || "";
    pageHelp.textContent = helpText;
    pageHelp.hidden = !simpleRole() || !helpText;
  }
  body.replaceChildren(records_skeletonRows(6));
  const inactive = table === "unidades_estudio" && showInactive ? "&activo=0" : "";
  let data;
  try {
    data = await api(
      `/api/records/${table}?page=${requestPage}&q=${encodeURIComponent(requestQuery)}${inactive}`,
    );
  } catch (error) {
    if (seq !== recordsSeq || table !== selected) return;
    body.replaceChildren();
    throw error;
  }
  if (seq !== recordsSeq || table !== selected) return;
  $("extra-panel").replaceChildren();
  if (table === "tipos_documento_legal" && admin())
    action($("extra-panel"), "Nuevo tipo de documento", () => documentTypeForm(null));
  if (table === "log_actividad" && admin())
    action($("extra-panel"), "Filtrar bitácora", () => activityLog({}));
  if (table === "notificaciones" && admin())
    action($("extra-panel"), "Registro de envíos", () => deliveryLog(""));
  if (table === "certificados") {
    const link = el("a", "btn btn--secondary btn--sm");
    link.href = "/api/portafolio";
    link.append(icon("download"), el("span", "", "Descargar mi portafolio (PDF)"));
    $("extra-panel").append(link);
  }
  if (table === "unidades_estudio") {
    const wrap = el("label", "switch");
    const box = el("input");
    box.type = "checkbox";
    box.id = "show-inactive";
    box.checked = showInactive;
    box.onchange = () => {
      showInactive = box.checked;
      page = 1;
      loadRecords().catch(showError);
    };
    wrap.append(
      box,
      el("span", "switch__track"),
      el("span", "switch__label", "Mostrar unidades inactivas"),
    );
    $("extra-panel").append(wrap);
  }
  if (table === "equipos_reto_academico" && user.rol === "ESTUDIANTE")
    scheduleForStudent().catch(showError);
  total = data.total;
  const recordCount = $("record-count");
  recordCount.textContent = `${total} registro${total === 1 ? "" : "s"}`;
  recordCount.setAttribute("aria-live", "polite");
  // Exports are an admin feature in Django (apps/reportes).
  $("exports").hidden = !admin();
  $("export-link").href =
    `/api/export/${table}?q=${encodeURIComponent(requestQuery)}`;
  $("export-link").download = "retosean.csv";
  $("export-pdf").href =
    `/api/export/${table}/pdf?q=${encodeURIComponent(requestQuery)}`;
  $("export-xlsx").href =
    `/api/export/${table}/xlsx?q=${encodeURIComponent(requestQuery)}`;
  const cols = columns(data.records, table);
  const head = document.createElement("tr");
  for (const key of [...cols, "Acciones"]) {
    const th = document.createElement("th");
    th.scope = "col";
    th.textContent =
      columnHeaders[key] ||
      capfirst(
        field(table, key)?.verbose_name ||
          field(table, key + "_id")?.verbose_name ||
          key,
      );
    if (records_NUMERIC.includes(key)) th.classList.add("num");
    head.append(th);
  }
  $("table-head").replaceChildren(head);
  body.replaceChildren();
  if (!data.records.length) {
    const tr = document.createElement("tr");
    const td = document.createElement("td");
    td.colSpan = cols.length + 1;
    td.append(
      requestQuery
        ? ui.empty(
            "search",
            "Sin resultados",
            "Ningún registro coincide con tu búsqueda.",
            ui.button("Limpiar búsqueda", {
              variant: "secondary",
              icon: "x",
              onClick: () => {
                query = "";
                page = 1;
                $("search").value = "";
                loadRecords().catch(showError);
              },
            }),
          )
        : ui.empty(
            "folder2-open",
            "No hay registros",
            "Aún no hay registros en esta sección.",
          ),
    );
    tr.append(td);
    body.append(tr);
  }
  for (const record of data.records) {
    const tr = document.createElement("tr");
    for (const key of cols) {
      const td = document.createElement("td");
      if (records_NUMERIC.includes(key)) td.classList.add("num");
      if (key.includes("estado") && record[key]) {
        const badge = el("span", "pill " + badgeClass(record[key]) + " dot");
        badge.textContent = choice(table, key, record[key]);
        td.append(badge);
      } else {
        const text = display(record[key]);
        td.title = text;
        td.textContent = field(table, key)?.choices.length
          ? choice(table, key, record[key])
          : text;
      }
      tr.append(td);
    }
    const td = document.createElement("td");
    const button = el("button", "btn btn--secondary btn--sm");
    button.type = "button";
    button.append(icon("eye"), el("span", "", "Ver detalle"));
    button.onclick = () => openRecord(record.id).catch(showError);
    td.append(button);
    tr.append(td);
    body.append(tr);
  }
  const pageNumber = $("page-number");
  pageNumber.textContent =
    `Página ${requestPage} de ${Math.max(1, Math.ceil(total / 20))}`;
  pageNumber.setAttribute("aria-live", "polite");
  $("previous").disabled = requestPage <= 1;
  $("next").disabled = requestPage * 20 >= total;
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
  button.className = "btn " + (primary ? "btn--primary" : "btn--secondary");
  button.textContent = label;
  button.onclick = () => {
    const task = Promise.resolve().then(handler).catch((e) => toast(e.message));
    busy(button, task);
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
  const wrapper = el("div", "field");
  if (type === "textarea") wrapper.classList.add("field--full");
  if (type === "file") wrapper.classList.add("file");
  const l = el("label", "field__label", label);
  l.htmlFor = "field-" + name;
  if (required) l.append(el("span", "req", " *"));
  const field = document.createElement(
    type === "textarea" ? "textarea" : "input",
  );
  field.id = "field-" + name;
  field.name = name;
  if (type !== "textarea") field.type = type;
  if (type === "textarea") field.className = "textarea";
  else if (type !== "file") field.className = "input";
  if (type !== "file") field.value = value ?? "";
  field.required = required;
  const err = el("p", "field__error");
  err.id = field.id + "-error";
  field.setAttribute("aria-describedby", err.id);
  wrapper.append(l, field, err);
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