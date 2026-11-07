"use strict";
/* RetosEAN — create/edit forms: field rendering, validation, file uploads and
   submit flows. Consumes the design-system helpers (ui.*, icon, toast, api,
   modal, mutate, busy) from ui.js / core.js. */

/* ═══════════════════════════════════════════════════════════════════════════
   Local helpers (forms area). Prefixed with `forms_` to avoid clashing with
   other shared-scope globals.
   ═══════════════════════════════════════════════════════════════════════════ */

// Normalise an <option> spec to {value,label,selected}. Accepted forms:
// a bare string (label via choiceLabel), a {value,label,selected?} object, or
// a [value,label] tuple.
function forms_option(v) {
  if (Array.isArray(v)) return { value: v[0], label: v[1], selected: false };
  if (typeof v === "object") {
    return {
      value: v.value,
      label: v.label,
      selected: !!v.selected,
      disabled: !!v.disabled,
    };
  }
  return { value: v, label: choiceLabel(v), selected: false };
}

// A labelled <select> built on the design-system classes, with inline error
// and optional multiple selection.
function forms_selectField(name, label, options, opts = {}) {
  const { required = false, multiple = false, hint = "", value, onInput } = opts;
  const wrap = el("div", "field");
  const id = "field-" + name;
  const l = el("label", "field__label", label);
  l.htmlFor = id;
  if (required) l.append(el("span", "req", " *"));
  const select = el("select", "select");
  select.id = id;
  select.name = name;
  select.required = required;
  select.multiple = multiple;
  for (const v of options) {
    const o = forms_option(v);
    const option = document.createElement("option");
    option.value = o.value;
    option.textContent = o.label;
    option.selected =
      o.selected || (value !== undefined && String(o.value) === String(value));
    if (o.disabled) option.disabled = true;
    select.append(option);
  }
  const err = el("p", "field__error");
  err.id = id + "-error";
  select.setAttribute("aria-describedby", err.id);
  wrap.append(l, select);
  if (hint) wrap.append(el("p", "field__hint", hint));
  wrap.append(err);
  if (onInput) select.addEventListener("input", onInput);
  return wrap;
}

// A checkbox using the .checkbox design-system classes. Optional description
// renders stacked under the label.
function forms_checkbox(name, label, opts = {}) {
  const { checked = false, required = false, description = "", value, id } = opts;
  const wrap = el("div", "checkbox field--full");
  const boxId = id || (name ? "field-" + name : "");
  const box = el("input", "checkbox__input");
  box.type = "checkbox";
  if (boxId) box.id = boxId;
  if (name) box.name = name;
  if (value !== undefined) box.value = value;
  box.checked = !!checked;
  box.required = !!required;
  const lab = el("label", "checkbox__label", label);
  if (boxId) lab.htmlFor = boxId;
  if (required) lab.append(el("span", "req", " *"));
  wrap.append(box);
  if (description) {
    const text = el("div", "min-w-0");
    text.append(lab, el("div", "small text-muted", description));
    wrap.append(text);
  } else {
    wrap.append(lab);
  }
  return wrap;
}

function forms_formatSize(bytes) {
  if (!Number.isFinite(bytes)) return "";
  if (bytes < 1024) return bytes + " B";
  if (bytes < 1024 * 1024) return Math.round(bytes / 1024) + " KB";
  return (bytes / 1024 / 1024).toFixed(1) + " MB";
}

// Validate a file input's extension (from its accept list) and size. Returns an
// error message or "" when valid.
function forms_validateFile(input, maxMb) {
  const f = input.files && input.files[0];
  if (!f) return input.required ? "Selecciona un archivo." : "";
  const exts = (input.accept || "")
    .split(",")
    .map((s) => s.trim().replace(/^\./, ""))
    .filter(Boolean);
  if (exts.length) {
    const ext = String(f.name.split(".").pop() || "").toLowerCase();
    if (!exts.some((e) => e.toLowerCase() === ext))
      return "Formato no admitido. Usa: " + input.accept.replace(/,/g, ", ");
  }
  if (maxMb && f.size > maxMb * 1024 * 1024)
    return `El archivo supera el límite de ${maxMb} MB.`;
  return "";
}

// A file field with dropzone styling, selected-file meta and an indeterminate
// upload progress bar.
function forms_fileField(name, label, opts = {}) {
  const { accept = "", required = false, hint = "" } = opts;
  const wrap = el("div", "field field--full");
  const id = "field-" + name;
  const l = el("label", "field__label", label);
  l.htmlFor = id;
  if (required) l.append(el("span", "req", " *"));
  const fileBox = el("div", "file");
  const input = el("input", "input");
  input.type = "file";
  input.id = id;
  input.name = name;
  if (accept) input.accept = accept;
  input.required = !!required;
  fileBox.append(input);
  const err = el("p", "field__error");
  err.id = id + "-error";
  input.setAttribute("aria-describedby", err.id);
  const meta = el("p", "small text-muted mt-1");
  meta.hidden = true;
  const progress = el("div", "progress forms-progress mt-2");
  progress.hidden = true;
  progress.setAttribute("role", "progressbar");
  progress.setAttribute("aria-label", "Subiendo archivo");
  progress.append(el("div", "progress__bar"));
  input.addEventListener("change", () => {
    const f = input.files && input.files[0];
    if (!f) {
      meta.hidden = true;
      meta.textContent = "";
    } else {
      meta.hidden = false;
      meta.textContent = `${f.name} · ${forms_formatSize(f.size)}`;
    }
    forms_setError(wrap, "");
  });
  wrap.append(l, fileBox, meta, progress);
  if (hint) wrap.append(el("p", "field__hint", hint));
  wrap.append(err);
  return wrap;
}

// Set or clear the inline validation error for a built field.
function forms_setError(field, message) {
  const control = field.querySelector(
    ".input, .select, .textarea, .checkbox__input",
  );
  const err = field.querySelector(".field__error");
  if (!err) return;
  err.replaceChildren();
  if (message) {
    err.append(el("span", "", message));
    if (control) control.setAttribute("aria-invalid", "true");
  } else if (control) {
    control.removeAttribute("aria-invalid");
  }
}

// Toggle the indeterminate upload progress bar inside a file field.
function forms_setProgress(field, busy) {
  const p = field.querySelector(".forms-progress");
  if (!p) return;
  p.hidden = !busy;
  p.classList.toggle("progress--busy", !!busy);
}

// A row of action buttons at the foot of a form (spans both grid columns).
function forms_actions(...nodes) {
  const a = el("div", "forms-actions");
  for (const n of nodes) a.append(n);
  return a;
}

// An inline status message (alert) with a matching icon and role.
function forms_alert(text, variant = "danger") {
  const a = el("div", "alert alert--" + variant);
  a.setAttribute(
    "role",
    variant === "danger" || variant === "warning" ? "alert" : "status",
  );
  const icons = {
    danger: "exclamation-circle-fill",
    warning: "exclamation-triangle-fill",
    info: "info-circle-fill",
    success: "check-circle-fill",
  };
  a.append(icon(icons[variant] || "info-circle-fill"), el("span", "", text));
  return a;
}

/* ═══════════════════════════════════════════════════════════════════════════
   Forms
   ═══════════════════════════════════════════════════════════════════════════ */

function importForm() {
  const parent = modal("Importación Masiva de Usuarios");
  parent.append(
    el(
      "p",
      "small text-muted mb-1",
      "Columnas: username, email, first_name, last_name, rol, telefono, password. Obligatorias: username, email, rol.",
    ),
    el(
      "p",
      "small text-muted mb-3",
      "Sin contraseña, la cuenta recibe por correo un enlace para definirla.",
    ),
  );
  const template = el("a", "btn btn--secondary mb-3");
  template.append(icon("download"), el("span", "", "Descargar plantilla CSV"));
  template.href = "/api/usuarios/plantilla";

  const form = el("form");
  const fileField = forms_fileField(
    "archivo",
    "Archivo .csv o .xlsx (máximo 1000 filas)",
    { accept: ".csv,.xlsx,.xlsm", required: true, hint: "Máximo 1000 filas por archivo." },
  );
  form.append(fileField);
  const file = fileField.querySelector("input");

  const result = el("div", "field--full mt-3");

  const send = async (accion) => {
    const data = new FormData();
    data.append("accion", accion);
    data.append("archivo", file.files[0]);
    const response = await fetch("/api/usuarios/importar", {
      method: "POST",
      headers: { "X-CSRF-Token": csrf },
      body: data,
    });
    const body = await readJson(response);
    if (!response.ok) throw new Error(body.error || "No se pudo leer el archivo");
    return body;
  };

  const preview = el("button", "btn btn--secondary");
  preview.type = "button";
  preview.append(icon("eye"), el("span", "", "Previsualizar"));

  const confirm = el("button", "btn btn--primary ms-2");
  confirm.type = "button";
  confirm.append(icon("check2-circle"), el("span", "", "Confirmar importación"));
  confirm.hidden = true;

  preview.onclick = () =>
    busy(
      preview,
      (async () => {
        const error = forms_validateFile(file);
        if (error) {
          forms_setError(fileField, error);
          return;
        }
        try {
          const data = await send("previsualizar");
          result.replaceChildren();
          if (data.error_global) {
            result.append(forms_alert(data.error_global));
            confirm.hidden = true;
            return;
          }
          const rows = [...data.validas, ...data.invalidas].sort(
            (a, b) => a.fila - b.fila,
          );
          result.append(
            el(
              "p",
              "small fw-semibold",
              `${data.validas.length} fila(s) válida(s) · ${data.invalidas.length} con errores`,
            ),
            ui.table(
              ["Fila", "Usuario", "Correo", "Nombre", "Rol", "Estado"],
              rows.map((r) => [
                r.fila,
                r.datos.username,
                r.datos.email,
                `${r.datos.first_name || ""} ${r.datos.last_name || ""}`.trim(),
                r.datos.rol,
                r.errores.length ? r.errores.join(" ") : "Lista para crear",
              ]),
              { empty: "Sin filas para previsualizar." },
            ),
          );
          confirm.hidden = data.validas.length === 0;
        } catch (e) {
          toast(e.message, "error");
        }
      })(),
    );

  confirm.onclick = () =>
    busy(
      confirm,
      (async () => {
        try {
          const data = await send("confirmar");
          if (data.error_global) {
            result.replaceChildren(forms_alert(data.error_global));
            confirm.hidden = true;
            return;
          }
          $("detail-dialog").close();
          toast(
            `${data.creados} usuario(s) creado(s) correctamente.` +
              (data.omitidas
                ? ` ${data.omitidas} fila(s) se omitieron por errores de validación.`
                : ""),
            "success",
          );
          await loadRecords();
        } catch (e) {
          toast(e.message, "error");
        }
      })(),
    );

  file.addEventListener("change", () => {
    confirm.hidden = true;
    result.replaceChildren();
  });

  form.append(forms_actions(preview, confirm), result);
  parent.append(template, form);
}

function simpleForm(title, path, fields, transform = (data) => data) {
  const parent = modal(title);
  const form = el("form", "form-grid");
  for (const [name, label, type = "text", value = ""] of fields) {
    if (type === "select" || type === "multiselect" || type === "optional") {
      form.append(
        forms_selectField(name, label, value, {
          required: type === "select",
          multiple: type === "multiselect",
        }),
      );
    } else {
      form.append(
        ui.field({
          name,
          label,
          type,
          value,
          required: ![
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
          step: type === "number" ? "0.01" : undefined,
          min: type === "number" ? "0" : undefined,
        }),
      );
    }
  }
  const button = ui.button("Guardar", {
    variant: "primary",
    icon: "check",
    type: "submit",
  });
  form.append(forms_actions(button));
  form.addEventListener("submit", (e) => {
    e.preventDefault();
    busy(
      button,
      (async () => {
        try {
          const data = Object.fromEntries(new FormData(form));
          for (const [name, , type] of fields)
            if (type === "multiselect")
              data[name] = Array.from(form.elements[name].selectedOptions).map(
                (o) => o.value,
              );
          await mutate(path, transform(data));
        } catch (error) {
          toast(error.message, "error");
        }
      })(),
    );
  });
  parent.append(form);
}

function deliveryForm(id) {
  const parent = modal("Entregar un archivo");
  const form = el("form");
  const titulo = ui.field({
    name: "titulo",
    label: "Título del entregable",
    type: "text",
    required: true,
  });
  titulo.querySelector(".input").maxLength = 180;
  form.append(titulo);
  form.append(
    ui.field({ name: "comentario", label: "Comentario", type: "textarea" }),
  );
  const fileField = forms_fileField(
    "archivo",
    "Archivo (PDF, PNG, JPG o TXT; máximo 9 MB)",
    { accept: ".pdf,.png,.jpg,.jpeg,.txt", required: true },
  );
  form.append(fileField);
  const file = fileField.querySelector("input");
  const button = ui.button("Enviar entregable", {
    variant: "primary",
    icon: "upload",
    type: "submit",
  });
  form.append(forms_actions(button));
  form.addEventListener("submit", (e) => {
    e.preventDefault();
    const error = forms_validateFile(file, 9);
    if (error) {
      forms_setError(fileField, error);
      return;
    }
    forms_setProgress(fileField, true);
    busy(
      button,
      (async () => {
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
          toast("Archivo entregado", "success");
          await loadRecords();
        } catch (error) {
          toast(error.message, "error");
        } finally {
          forms_setProgress(fileField, false);
        }
      })(),
    );
  });
  parent.append(form);
}

function documentForm(url) {
  const parent = modal("Cargar documento legal");
  const form = el("form");
  form.append(
    ui.field({
      name: "fecha_expedicion",
      label: "Fecha de expedición",
      type: "date",
      required: true,
    }),
  );
  const fileField = forms_fileField(
    "archivo",
    "Documento PDF (máximo 9 MB)",
    { accept: ".pdf", required: true },
  );
  form.append(fileField);
  const file = fileField.querySelector("input");
  const button = ui.button("Cargar documento", {
    variant: "primary",
    icon: "upload",
    type: "submit",
  });
  form.append(forms_actions(button));
  form.addEventListener("submit", (e) => {
    e.preventDefault();
    const error = forms_validateFile(file, 9);
    if (error) {
      forms_setError(fileField, error);
      return;
    }
    busy(
      button,
      (async () => {
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
          toast("Documento enviado a revisión", "success");
          await loadRecords();
        } catch (error) {
          toast(error.message, "error");
        }
      })(),
    );
  });
  parent.append(form);
}

async function permissionsForm(id) {
  const parent = modal("Permisos especiales");
  parent.append(ui.skeleton(4, { kind: "row" }));
  let data;
  try {
    data = await api(`/api/usuarios/${id}/permisos`);
  } catch (e) {
    $("detail-dialog").close();
    throw e;
  }
  parent.replaceChildren();
  const form = el("form");
  for (const p of data.permisos) {
    form.append(
      forms_checkbox(null, p.nombre, {
        id: "perm-" + p.codigo,
        value: p.codigo,
        checked: p.otorgado,
        description: p.descripcion,
      }),
    );
  }
  const button = ui.button("Guardar permisos", {
    variant: "primary",
    icon: "check",
    type: "submit",
  });
  form.append(forms_actions(button));
  form.addEventListener("submit", (e) => {
    e.preventDefault();
    busy(
      button,
      (async () => {
        const codigos = Array.from(form.querySelectorAll("input:checked")).map(
          (b) => b.value,
        );
        await mutate(`/api/usuarios/${id}/permisos`, { codigos });
      })(),
    );
  });
  parent.append(form);
}

// HU00: state changes of the company's documents; automatic ones show as "Sistema".
async function documentHistory(company) {
  const parent = modal("Historial documental");
  parent.append(ui.skeleton(4, { kind: "row" }));
  let data;
  try {
    data = await api(`/api/empresas/${company}/historial-documental`);
  } catch (e) {
    $("detail-dialog").close();
    throw e;
  }
  parent.replaceChildren();
  parent.append(
    ui.table(
      ["Fecha", "Documento", "De", "A", "Por", "Comentario"],
      data.historial.map((h) => [
        fecha(h.fecha, true),
        h.documento,
        choiceLabel(h.estado_anterior),
        choiceLabel(h.estado_nuevo),
        h.realizado_por,
        h.comentario,
      ]),
      { empty: "Sin movimientos en el historial documental." },
    ),
  );
}

// editar_tipo_documento: the legal-document catalog.
function documentTypeForm(record) {
  const parent = modal(
    record ? "Editar tipo de documento" : "Nuevo tipo de documento",
  );
  const form = el("form", "form-grid");
  if (!record)
    form.append(
      ui.field({
        name: "codigo",
        label: "Código (p. ej. POLIZA_RESPONSABILIDAD)",
        type: "text",
        required: true,
      }),
    );
  form.append(
    ui.field({
      name: "nombre",
      label: "Nombre",
      type: "text",
      value: record?.nombre || "",
      required: true,
    }),
  );
  form.append(
    ui.field({
      name: "descripcion",
      label: "Descripción",
      type: "textarea",
      value: record?.descripcion || "",
    }),
  );
  form.append(
    ui.field({
      name: "vigencia_dias",
      label: "Vigencia máxima (días desde la expedición)",
      type: "number",
      value: record?.vigencia_dias ?? "",
      min: "0",
    }),
  );
  form.append(
    ui.field({
      name: "orden",
      label: "Orden",
      type: "number",
      value: record?.orden ?? 0,
    }),
  );
  const boxes = [
    ["obligatorio", "Obligatorio"],
    ["aplica_sin_convenio", "Se exige también a empresas sin convenio"],
    ["requiere_permiso_especial", "Solo lo carga un usuario con permiso especial"],
    ["activo", "Activo"],
  ];
  for (const [name, text] of boxes) {
    form.append(
      forms_checkbox(name, text, {
        checked: record ? !!record[name] : name !== "requiere_permiso_especial",
      }),
    );
  }
  const button = ui.button("Guardar", {
    variant: "primary",
    icon: "check",
    type: "submit",
  });
  form.append(forms_actions(button));
  form.addEventListener("submit", (e) => {
    e.preventDefault();
    busy(
      button,
      (async () => {
        const data = Object.fromEntries(new FormData(form));
        for (const [name] of boxes) data[name] = form.elements[name].checked;
        data.vigencia_dias =
          data.vigencia_dias === "" ? null : Number(data.vigencia_dias);
        data.orden = Number(data.orden || 0);
        try {
          await mutate(
            record ? `/api/tipos-documento/${record.id}` : "/api/tipos-documento",
            data,
          );
        } catch (error) {
          toast(error.message, "error");
        }
      })(),
    );
  });
  parent.append(form);
}

// seguimiento_avance (HU14).
async function progressPanel(reto) {
  const parent = modal("Avance del reto");
  parent.append(ui.skeleton(4, { kind: "row" }));
  let data;
  try {
    data = await api(`/api/retos/${reto}/avance`);
  } catch (e) {
    $("detail-dialog").close();
    throw e;
  }
  parent.replaceChildren();
  parent.append(
    el(
      "p",
      "fw-bold",
      `Avance según el último seguimiento: ${data.porcentaje}%`,
    ),
  );
  parent.append(el("h6", "fw-bold mt-2", "Bitácora de sesiones"));
  parent.append(
    ui.table(
      ["Fecha", "Sesión", "Avance", "Avances", "Acuerdos"],
      data.seguimientos.map((s) => [
        fecha(s.fecha_sesion),
        choiceLabel(s.tipo_sesion),
        `${s.porcentaje_avance}%`,
        s.avances,
        s.acuerdos,
      ]),
      { empty: "Aún no hay sesiones registradas." },
    ),
  );
  parent.append(el("h6", "fw-bold mt-3", "Entregables de los equipos"));
  for (const e of data.entregables) {
    const box = el("section", "surface mb-3");
    const body = el("div", "surface__body");
    const head = el("div", "cluster cluster--between");
    head.append(
      el("span", "fw-semibold", `${e.titulo} · ${e.estudiante}`),
      ui.badge(badgeClass(e.estado).replace("pill--", ""), {
        label: choiceLabel(e.estado),
      }),
    );
    body.append(head);
    for (const c of e.comentarios)
      body.append(el("p", "small text-muted mb-1", `${c.autor}: ${c.comentario}`));
    const form = el("form", "forms-comment mt-2");
    const text = el("input", "input input--sm");
    text.name = "comentario";
    text.required = true;
    text.placeholder = "Comentario para el equipo";
    text.setAttribute("aria-label", "Comentario para el equipo");
    const submit = ui.button("Comentar", {
      variant: "secondary",
      icon: "chat-square-text",
      type: "submit",
    });
    form.append(text, submit);
    form.addEventListener("submit", (ev) => {
      ev.preventDefault();
      busy(
        submit,
        (async () => {
          try {
            await api(`/api/entregables/${e.id}/comentario`, {
              comentario: text.value,
            });
            toast("Comentario agregado al historial.", "success");
            await progressPanel(reto);
          } catch (error) {
            toast(error.message, "error");
          }
        })(),
      );
    });
    body.append(form);
    box.append(body);
    parent.append(box);
  }
  if (!data.entregables.length)
    parent.append(
      ui.empty(
        "folder2-open",
        "Aún no hay entregables",
        "Los equipos no han cargado entregables todavía.",
      ),
    );
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
  const parent = modal("Registro de actividad");
  parent.append(ui.skeleton(4, { kind: "row" }));
  let data;
  try {
    const query = new URLSearchParams(
      Object.entries(filters).filter(([, v]) => v),
    ).toString();
    data = await api(`/api/log-actividad?${query}`);
  } catch (e) {
    $("detail-dialog").close();
    throw e;
  }
  parent.replaceChildren();
  const form = el("form", "form-grid");
  form.append(
    forms_selectField(
      "accion",
      "Acción",
      [["", "Todas"], ...Object.entries(ACCIONES_LOG)],
      { value: filters.accion || "" },
    ),
  );
  form.append(ui.field({ name: "usuario", label: "Usuario", value: filters.usuario || "" }));
  form.append(ui.field({ name: "desde", label: "Desde", type: "date", value: filters.desde || "" }));
  form.append(ui.field({ name: "hasta", label: "Hasta", type: "date", value: filters.hasta || "" }));
  const apply = ui.button("Filtrar", {
    variant: "secondary",
    icon: "funnel",
    type: "submit",
  });
  form.append(forms_actions(apply));
  form.addEventListener("submit", (e) => {
    e.preventDefault();
    activityLog(Object.fromEntries(new FormData(form))).catch(showError);
  });
  parent.append(
    form,
    ui.table(
      [
        "Fecha",
        "Acción",
        "Identificador",
        "Usuario",
        "Realizado por",
        "IP",
        "Detalle",
      ],
      data.registros.map((r) => [
        fecha(r.creado_en, true),
        ACCIONES_LOG[r.accion] || choiceLabel(r.accion),
        r.identificador,
        r.usuario,
        r.realizado_por,
        r.ip,
        r.detalle,
      ]),
      { empty: "No hay actividad registrada con esos filtros." },
    ),
  );
}

// HU15: delivery state of every notification, filterable.
async function deliveryLog(estado) {
  const parent = modal("Registro de envíos");
  parent.append(ui.skeleton(4, { kind: "row" }));
  let data;
  try {
    data = await api(
      `/api/notificaciones/registro?estado=${encodeURIComponent(estado)}`,
    );
  } catch (e) {
    $("detail-dialog").close();
    throw e;
  }
  parent.replaceChildren();
  const r = data.resumen;
  parent.append(
    el(
      "p",
      "small",
      `Total: ${r.total} · Enviadas por correo: ${r.enviadas} · Fallidas: ${r.fallidas} · Leídas: ${r.leidas}`,
    ),
  );
  const filterField = forms_selectField(
    "estado-envio",
    "Estado",
    [
      ["", "Todos"],
      ["enviado", "Enviado por correo"],
      ["fallido", "Fallido"],
      ["leido", "Leído"],
      ["no_leido", "No leído"],
      ["solo_app", "Solo en la plataforma"],
    ],
    { value: estado },
  );
  const filter = filterField.querySelector("select");
  filter.addEventListener("change", () =>
    deliveryLog(filter.value).catch(showError),
  );
  parent.append(filterField);
  parent.append(
    ui.table(
      ["Fecha", "Usuario", "Evento", "Título", "Correo", "Leída"],
      data.registros.map((n) => [
        fecha(n.creada_en, true),
        n.username,
        EVENTOS_NOTIFICACION[n.evento] || human(n.evento || ""),
        n.titulo,
        n.error_envio
          ? `Fallido: ${n.error_envio}`
          : n.enviada_por_correo
            ? "Enviado"
            : "Solo plataforma",
        n.leida ? "Sí" : "No",
      ]),
      { empty: "Sin envíos registrados." },
    ),
  );
}

async function changeState(id, estado, comentario = "") {
  await mutate(`/api/retos/${id}/estado`, { estado, comentario });
}

function stateForm(id, estado) {
  const parent = modal(choice("retos", "estado", estado));
  const form = el("form");
  form.append(
    ui.field({
      name: "comentario",
      label: "Motivo / comentario",
      type: "textarea",
      required: true,
    }),
  );
  const submit = ui.button("Confirmar", {
    variant: "primary",
    icon: "check",
    type: "submit",
  });
  form.append(forms_actions(submit));
  form.addEventListener("submit", (e) => {
    e.preventDefault();
    busy(
      submit,
      (async () => {
        try {
          const data = Object.fromEntries(new FormData(form));
          await changeState(id, estado, data.comentario);
        } catch (error) {
          toast(error.message, "error");
        }
      })(),
    );
  });
  parent.append(form);
}

function challengeForm(record = {}) {
  const parent = modal(record.id ? "Editar reto" : "Crear un reto");
  const form = el("form", "form-grid");
  const titleField = ui.field({
    name: "titulo",
    label: "Título",
    type: "text",
    value: record.titulo,
    required: true,
  });
  titleField.querySelector(".input").maxLength = 180;
  form.append(titleField);
  if (!record.id) {
    form.append(
      forms_selectField(
        "tipo",
        "Tipo",
        ["reto", "hackathon"].map((name) => ({
          value: name,
          label: choice("retos", "tipo", name),
        })),
      ),
    );
  }
  const areaField = ui.field({
    name: "area",
    label: "Área",
    type: "text",
    value: record.area || "Tecnología",
    required: true,
  });
  areaField.querySelector(".input").maxLength = 120;
  form.append(areaField);
  form.append(
    ui.field({
      name: "descripcion",
      label: "Descripción",
      type: "textarea",
      value: record.descripcion,
      required: true,
    }),
  );
  form.append(
    ui.field({
      name: "nivel_academico",
      label: "Nivel académico",
      type: "text",
      value: record.nivel_academico || "Pregrado",
    }),
  );
  form.append(
    ui.field({
      name: "fecha_inicio_tentativa",
      label: "Inicio tentativo",
      type: "date",
      value: record.fecha_inicio_tentativa,
      required: true,
    }),
  );
  form.append(
    ui.field({
      name: "fecha_fin_tentativa",
      label: "Fin tentativo",
      type: "date",
      value: record.fecha_fin_tentativa,
      required: true,
    }),
  );
  form.append(
    ui.field({
      name: "fecha_limite_postulacion",
      label: "Límite de postulación",
      type: "date",
      value: record.fecha_limite_postulacion,
      required: true,
    }),
  );
  const button = ui.button("Guardar borrador", {
    variant: "primary",
    icon: "check",
    type: "submit",
  });
  form.append(forms_actions(button));
  form.addEventListener("submit", (e) => {
    e.preventDefault();
    busy(
      button,
      (async () => {
        try {
          const data = Object.fromEntries(new FormData(form));
          data.tipo = record.tipo || data.tipo || "reto";
          for (const key of [
            "fecha_inicio_tentativa",
            "fecha_fin_tentativa",
            "fecha_limite_postulacion",
          ])
            data[key] ||= null;
          await mutate(
            record.id ? `/api/retos/${record.id}` : "/api/retos",
            data,
          );
        } catch (error) {
          toast(error.message, "error");
        }
      })(),
    );
  });
  parent.append(form);
}

async function applicationForm(id) {
  // PostulacionForm: catalog program, optional PDF CV, required terms.
  const parent = modal("Postularme al reto");
  parent.append(ui.skeleton(4, { kind: "row" }));
  let options;
  try {
    options = await api("/api/registro/opciones");
  } catch (e) {
    $("detail-dialog").close();
    throw e;
  }
  parent.replaceChildren();
  const form = el("form", "form-grid");
  form.append(
    ui.field({
      name: "motivacion",
      label: "¿Por qué quieres participar?",
      type: "textarea",
      required: true,
    }),
  );
  form.append(
    ui.field({ name: "habilidades", label: "Habilidades", type: "textarea" }),
  );
  form.append(
    forms_selectField("programa", "Programa académico", [
      ["", "Selecciona tu programa académico"],
      ...options.programas.map((p) => [p.nombre, p.label]),
    ]),
  );
  form.append(
    ui.field({
      name: "semestre",
      label: "Semestre",
      type: "number",
      value: "1",
      min: "1",
      max: "12",
      required: true,
    }),
  );
  const cvField = forms_fileField(
    "hoja_de_vida",
    "Hoja de vida o portafolio (PDF, opcional)",
    { accept: ".pdf" },
  );
  form.append(cvField);
  const cv = cvField.querySelector("input");
  form.append(
    forms_checkbox(
      "acepta_terminos",
      "Acepto los términos y condiciones del reto y el tratamiento de mis datos personales",
      { required: true },
    ),
  );
  const button = ui.button("Enviar postulación", {
    variant: "primary",
    icon: "send",
    type: "submit",
  });
  form.append(forms_actions(button));
  form.addEventListener("submit", (e) => {
    e.preventDefault();
    const error = forms_validateFile(cv);
    if (error) {
      forms_setError(cvField, error);
      return;
    }
    busy(
      button,
      (async () => {
        try {
          const response = await fetch(`/api/retos/${id}/postular`, {
            method: "POST",
            headers: { "X-CSRF-Token": csrf },
            body: new FormData(form),
          });
          const data = await readJson(response);
          if (!response.ok)
            throw new Error(data.error || "No se pudo enviar la postulación");
          $("detail-dialog").close();
          toast(
            "¡Tu postulación fue enviada! La empresa fue notificada y revisará tu solicitud.",
            "success",
          );
          await loadRecords();
        } catch (error) {
          toast(error.message, "error");
        }
      })(),
    );
  });
  parent.append(form);
}