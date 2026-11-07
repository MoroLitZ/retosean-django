"use strict";
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
    const body = await readJson(response);
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
        el("p", "small", `${data.validas.length} fila(s) válida(s) · ${data.invalidas.length} con errores`),
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
      if (data.error_global) {
        result.replaceChildren(el("div", "alert alert-danger py-2", data.error_global));
        confirm.hidden = true;
        return;
      }
      $("detail-dialog").close();
      toast(`${data.creados} usuario(s) creado(s) correctamente.` + (data.omitidas ? ` ${data.omitidas} fila(s) se omitieron por errores de validación.` : ""));
      await loadRecords();
    } catch (e) {
      toast(e.message);
    } finally {
      confirm.disabled = false;
    }
  };
  file.onchange = () => {
    confirm.hidden = true;
    result.replaceChildren();
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
          "descripcion",
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
      const data = await readJson(response);
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
      const data = await readJson(response);
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
// HU00: state changes of the company's documents; automatic ones show as "Sistema".
async function documentHistory(company) {
  const data = await api(`/api/empresas/${company}/historial-documental`);
  const parent = modal("Historial documental");
  parent.append(table(["Fecha", "Documento", "De", "A", "Por", "Comentario"], data.historial.map((h) => [
    fecha(h.fecha, true), h.documento, choiceLabel(h.estado_anterior), choiceLabel(h.estado_nuevo), h.realizado_por, h.comentario,
  ])));
}
// editar_tipo_documento: the legal-document catalog.
function documentTypeForm(record) {
  const parent = modal(record ? "Editar tipo de documento" : "Nuevo tipo de documento");
  const form = el("form", "form-grid");
  if (!record) input(form, "codigo", "Código (p. ej. POLIZA_RESPONSABILIDAD)", "", "text", true);
  input(form, "nombre", "Nombre", record?.nombre || "", "text", true);
  input(form, "descripcion", "Descripción", record?.descripcion || "", "textarea");
  const days = input(form, "vigencia_dias", "Vigencia máxima (días desde la expedición)", record?.vigencia_dias ?? "", "number");
  days.min = 0;
  input(form, "orden", "Orden", record?.orden ?? 0, "number");
  const boxes = [
    ["obligatorio", "Obligatorio"],
    ["aplica_sin_convenio", "Se exige también a empresas sin convenio"],
    ["requiere_permiso_especial", "Solo lo carga un usuario con permiso especial"],
    ["activo", "Activo"],
  ];
  for (const [name, text] of boxes) {
    const wrap = el("div", "form-check full");
    const box = el("input", "form-check-input");
    box.type = "checkbox";
    box.name = name;
    box.id = "field-" + name;
    box.checked = record ? !!record[name] : name !== "requiere_permiso_especial";
    const l = el("label", "form-check-label", text);
    l.htmlFor = box.id;
    wrap.append(box, l);
    form.append(wrap);
  }
  form.append(el("button", "primary full", "Guardar"));
  form.onsubmit = async (e) => {
    e.preventDefault();
    const data = Object.fromEntries(new FormData(form));
    for (const [name] of boxes) data[name] = form.elements[name].checked;
    data.vigencia_dias = data.vigencia_dias === "" ? null : Number(data.vigencia_dias);
    data.orden = Number(data.orden || 0);
    try {
      await mutate(record ? `/api/tipos-documento/${record.id}` : "/api/tipos-documento", data);
    } catch (error) {
      toast(error.message);
    }
  };
  parent.append(form);
}
// seguimiento_avance (HU14).
async function progressPanel(reto) {
  const data = await api(`/api/retos/${reto}/avance`);
  const parent = modal("Avance del reto");
  parent.append(el("p", "fw-bold", `Avance según el último seguimiento: ${data.porcentaje}%`));
  parent.append(el("h6", "fw-bold mt-2", "Bitácora de sesiones"));
  parent.append(table(["Fecha", "Sesión", "Avance", "Avances", "Acuerdos"], data.seguimientos.map((s) => [
    fecha(s.fecha_sesion), choiceLabel(s.tipo_sesion), `${s.porcentaje_avance}%`, s.avances, s.acuerdos,
  ])));
  parent.append(el("h6", "fw-bold mt-3", "Entregables de los equipos"));
  for (const e of data.entregables) {
    const box = el("div", "mb-3");
    box.append(el("p", "mb-1", `${e.titulo} · ${e.estudiante} · ${choiceLabel(e.estado)}`));
    for (const c of e.comentarios) box.append(el("p", "small text-muted mb-1", `${c.autor}: ${c.comentario}`));
    const form = el("form", "d-flex gap-2");
    const text = el("input", "form-control form-control-sm");
    text.name = "comentario";
    text.required = true;
    text.placeholder = "Comentario para el equipo";
    form.append(text, el("button", "secondary", "Comentar"));
    form.onsubmit = async (ev) => {
      ev.preventDefault();
      try {
        await api(`/api/entregables/${e.id}/comentario`, { comentario: text.value });
        toast("Comentario agregado al historial.");
        await progressPanel(reto);
      } catch (error) {
        toast(error.message);
      }
    };
    box.append(form);
    parent.append(box);
  }
  if (!data.entregables.length) parent.append(el("p", "small text-muted", "Aún no hay entregables."));
}
// HU13: the activity log filtered by action, user and dates.
// Display labels for log_actividad.accion codes (values sent to the API stay the codes).
const ACCIONES_LOG = {
  LOGIN: "Inicio de sesión",
  LOGIN_FALLIDO: "Inicio de sesión fallido",
  LOGOUT: "Cierre de sesión",
  CREACION: "Creación",
  CAMBIO_ROL: "Cambio de rol",
  CAMBIO_ESTADO: "Cambio de estado",
  CAMBIO_PERMISOS: "Cambio de permisos",
  IMPORTACION: "Importación",
};
// Titles of the notification events (events.json), for display only.
const EVENTOS_NOTIFICACION = {
  CERTIFICADO_DISPONIBLE: "Tu certificado está disponible",
  COMENTARIO_EMPRESA: "Comentario de la empresa a un entregable",
  CONVENIO_ACTUALIZADO: "Avance en el trámite de tu convenio",
  DOCUMENTO_CARGADO_ADMIN: "Documento legal cargado por una empresa",
  DOCUMENTO_REVISADO: "Documentación revisada",
  EMPRESA_LISTAS_RESTRICTIVAS: "Verificación en listas restrictivas",
  EMPRESA_PARTICIPACION: "Estado de participación de tu empresa",
  ENCUESTA_PENDIENTE: "Encuesta de satisfacción",
  ENTREGABLE_CALIFICADO: "Tu entregable fue calificado",
  ENTREGABLE_RECIBIDO: "Nuevo entregable recibido",
  EQUIPO_ASIGNADO: "Fuiste asignado a un equipo",
  HACKATON_PUBLICADO: "Hackathon publicado",
  HACKATON_RESULTADOS: "Resultados del hackathon",
  INTEGRACION_ENVIADA_REVISION: "Nueva postulación docente a un reto",
  INTEGRACION_REVISADA: "Integración académica revisada",
  LOGISTICA_SOLICITADA: "Nueva solicitud de logística",
  POSTULACION_GESTIONADA: "Respuesta a tu postulación",
  POSTULACION_NUEVA: "Nueva postulación",
  POSTULACION_RECIBIDA: "Recibimos tu postulación",
  PRESUPUESTO_80: "Presupuesto por encima del 80 %",
  RECORDATORIO_FECHA_LIMITE: "Recordatorio de fecha límite",
  RETO_ENVIADO_REVISION: "Reto enviado a revisión",
  RETO_ESTADO_CAMBIADO: "Cambio de estado de tu reto",
  RETO_FINALIZADO: "Reto finalizado",
  RETO_SIN_ACTUALIZAR: "Reto vencido sin actualizaciones",
};
async function activityLog(filters) {
  const query = new URLSearchParams(Object.entries(filters).filter(([, v]) => v)).toString();
  const data = await api(`/api/log-actividad?${query}`);
  const parent = modal("Registro de actividad");
  const form = el("form", "form-grid");
  choiceSelect(form, "accion", "Acción", [["", "Todas"], ...Object.entries(ACCIONES_LOG)], filters.accion || "");
  input(form, "usuario", "Usuario", filters.usuario || "");
  input(form, "desde", "Desde", filters.desde || "", "date");
  input(form, "hasta", "Hasta", filters.hasta || "", "date");
  form.append(el("button", "secondary full", "Filtrar"));
  form.onsubmit = (e) => {
    e.preventDefault();
    activityLog(Object.fromEntries(new FormData(form))).catch(showError);
  };
  parent.append(form, table(["Fecha", "Acción", "Identificador", "Usuario", "Realizado por", "IP", "Detalle"], data.registros.map((r) => [
    fecha(r.creado_en, true), ACCIONES_LOG[r.accion] || choiceLabel(r.accion), r.identificador, r.usuario, r.realizado_por, r.ip, r.detalle,
  ])));
}
// HU15: delivery state of every notification, filterable.
async function deliveryLog(estado) {
  const data = await api(`/api/notificaciones/registro?estado=${encodeURIComponent(estado)}`);
  const parent = modal("Registro de envíos");
  const r = data.resumen;
  parent.append(el("p", "small", `Total: ${r.total} · Enviadas por correo: ${r.enviadas} · Fallidas: ${r.fallidas} · Leídas: ${r.leidas}`));
  const filter = choiceSelect(parent, "estado-envio", "Estado", [["", "Todos"], ["enviado", "Enviado por correo"], ["fallido", "Fallido"], ["leido", "Leído"], ["no_leido", "No leído"], ["solo_app", "Solo en la plataforma"]], estado);
  filter.onchange = () => deliveryLog(filter.value).catch(showError);
  parent.append(table(["Fecha", "Usuario", "Evento", "Título", "Correo", "Leída"], data.registros.map((n) => [
    fecha(n.creada_en, true), n.username, EVENTOS_NOTIFICACION[n.evento] || human(n.evento || ""), n.titulo,
    n.error_envio ? `Fallido: ${n.error_envio}` : n.enviada_por_correo ? "Enviado" : "Solo plataforma",
    n.leida ? "Sí" : "No",
  ])));
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
      const data = await readJson(response);
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
