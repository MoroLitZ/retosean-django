"use strict";
// Marca o limpia el error inline de los campos del login estático (#login-form).
function auth_loginError(input, message) {
  const wrap = input.closest(".field");
  let err = wrap.querySelector(".field__error");
  if (!err) {
    err = el("p", "field__error");
    err.id = input.id + "-error";
    input.setAttribute("aria-describedby", err.id);
    wrap.append(err);
  }
  if (message) {
    err.replaceChildren(icon("exclamation-circle-fill"), el("span", "", message));
    input.setAttribute("aria-invalid", "true");
  } else {
    err.replaceChildren();
    input.removeAttribute("aria-invalid");
  }
}
$("login-form").onsubmit = (e) => {
  e.preventDefault();
  $("login-error").hidden = true;
  const submit = e.submitter || e.target.querySelector('button[type="submit"]');
  let firstBad = null;
  for (const name of ["username", "password"]) {
    const input = e.target.elements[name];
    const message = input.value.trim() ? "" : "Este campo es obligatorio.";
    auth_loginError(input, message);
    if (message && !firstBad) firstBad = input;
  }
  if (firstBad) {
    firstBad.focus();
    return;
  }
  const data = Object.fromEntries(new FormData(e.target));
  busy(submit, api("/api/login", data))
    .then((response) => {
      csrf = response.csrf;
      $("password").value = "";
      return start();
    })
    .catch((error) => {
      $("login-error").replaceChildren(
        icon("exclamation-circle-fill"),
        el("span", "", error.message),
      );
      $("login-error").hidden = false;
    });
};
async function logout() {
  try {
    await api("/api/logout", {});
    showLogin();
  } catch (error) {
    showError(error);
  }
}
$("logout").onclick = logout;
$("close-dialog").onclick = () => $("detail-dialog").close();
$("new-challenge").onclick = () => {
  if (selected === "usuarios")
    simpleForm(
      "Crear usuario",
      "/api/usuarios",
      [
        ["username", "Usuario"],
        ["password", "Contraseña inicial (mínimo 12 caracteres)", "password"],
        ["email", "Correo", "email"],
        ["first_name", "Nombre"],
        ["last_name", "Apellido"],
        [
          "rol",
          "Rol",
          "select",
          ["ESTUDIANTE", "PROFESOR", "EMPRESA", "EXPERTO"],
        ],
        ["nit", "NIT (solo empresa)"],
        ["razon_social", "Razón social (solo empresa)"],
      ],
      (data) => ({ ...data, programa_id: null, facultad_id: null }),
    );
  else if (selected === "retos") challengeForm();
  else catalogForm().catch(showError);
};
async function catalogForm() {
  const fields = [["nombre", "Nombre"]];
  if (selected === "facultades") fields.push(["codigo", "Código"]);
  if (selected === "ecosistemas")
    fields.push(["descripcion", "Descripción", "textarea"]);
  if (selected === "programas") {
    const data = { records: await allRecords("facultades") };
    fields.push([
      "facultad_id",
      "Facultad",
      "select",
      data.records.map((r) => ({ value: r.id, label: r.nombre })),
    ]);
  }
  if (selected === "unidades_estudio") return unitForm();
  simpleForm(
    "Agregar a " + title(selected),
    `/api/catalogos/${selected}`,
    fields,
    (data) => ({
      ...data,
      programa_id: data.programa_id ? Number(data.programa_id) : null,
      facultad_id: data.facultad_id ? Number(data.facultad_id) : null,
    }),
  );
}
async function unitForm(record = null) {
  const programs = { records: (await allRecords("programas")).sort((a, b) => String(a.nombre).localeCompare(String(b.nombre), "es")) };
  const parent = modal(record ? "Editar Unidad de Estudio" : "Crear Unidad de Estudio");
  const form = el("form", "form-grid");
  input(form, "codigo", "Código", record?.codigo || "", "text", true);
  input(form, "nombre", "Nombre de la unidad", record?.nombre || "", "text", true);
  choiceSelect(form, "programa", "Programa académico", [["", "Seleccione"], ...programs.records.map((p) => [String(p.id), p.nombre])], record ? String(record.programa_id) : "");
  input(form, "periodo", "Periodo (ej: 2026-2)", record?.periodo || "");
  input(form, "ciclo", "Ciclo / Semestre", record?.ciclo || "");
  const file = input(form, "archivo", record?.archivo ? "Reemplazar archivo (PDF)" : "Archivo (PDF con info completa)", "", "file");
  file.accept = ".pdf";
  form.append(el("button", "btn btn--primary full", "Guardar"));
  form.onsubmit = (e) => sendUnit(e, form, record ? `/api/unidades/${record.id}` : "/api/unidades");
  parent.append(form);
  if (!record) {
    parent.append(el("h6", "fw-bold mt-3", "Carga masiva (CSV)"));
    parent.append(el("p", "small text-muted", "Columnas: codigo, nombre, programa (nombre exacto del catálogo), periodo y ciclo opcionales."));
    const bulk = el("form", "form-grid");
    const accion = el("input");
    accion.type = "hidden";
    accion.name = "accion";
    accion.value = "cargar_csv";
    bulk.append(accion);
    input(bulk, "archivo_csv", "Archivo CSV", "", "file", true).accept = ".csv";
    bulk.append(el("button", "btn btn--secondary full", "Cargar CSV"));
    bulk.onsubmit = (e) => sendUnit(e, bulk, "/api/unidades");
    parent.append(bulk);
  }
}
async function sendUnit(e, form, url) {
  e.preventDefault();
  const button = e.submitter || form.querySelector('button[type="submit"]');
  const fileInput = form.querySelector('input[type="file"]');
  if (fileInput && !fileInput.files[0]) {
    toast("Elige un archivo para continuar.");
    if (typeof fileInput.focus === "function") fileInput.focus();
    return;
  }
  try {
    const response = await busy(
      button,
      fetch(url, { method: "POST", headers: { "X-CSRF-Token": csrf }, body: new FormData(form) }),
    );
    const data = await readJson(response);
    if (!response.ok) throw new Error(data.error || "No se pudo guardar");
    $("detail-dialog").close();
    toast(data.mensaje);
    await loadRecords();
  } catch (error) {
    toast(error.message);
  }
}
function teamFields(heading, url, name, options) {
  simpleForm(
    heading,
    url,
    [
      ["nombre_equipo", "Nombre del equipo", "text", name],
      ["profesores", "Profesores", "multiselect", options.profesores],
      ["estudiantes", "Estudiantes", "multiselect", options.estudiantes],
      ["expertos_invitados", "Expertos invitados", "multiselect", options.expertos_invitados],
    ],
    (data) => ({
      ...data,
      profesores: data.profesores.map(Number),
      estudiantes: data.estudiantes.map(Number),
      expertos_invitados: data.expertos_invitados.map(Number),
    }),
  );
}
async function profileForm() {
  const [current, options] = await Promise.all([api("/api/perfil"), api("/api/registro/opciones")]);
  const pick = (list, value) => [{ value: "", label: "Seleccione" }, ...list.map((o) => ({ ...o, selected: String(o.value) === String(value ?? "") }))];
  const fields = [
    ["first_name", "Nombre", "text", current.first_name],
    ["last_name", "Apellido", "text", current.last_name],
    ["email", "Correo", "email", current.email],
    ["telefono", "Teléfono", "text", current.telefono],
  ];
  // EditarPerfilForm: role fields, all optional.
  if (user.rol === "ESTUDIANTE")
    fields.push(
      ["primera_carrera", "Carrera / Programa", "optional", pick(options.programas, current.primera_carrera)],
      ["semestre", "Semestre", "optional", pick([...Array(10)].map((_, i) => ({ value: String(i + 1), label: String(i + 1) })), current.semestre)],
    );
  if (user.rol === "PROFESOR")
    fields.push(
      ["facultad", "Facultad", "optional", pick(options.facultades, current.facultad)],
      ["especialidad", "Especialidad", "optional", pick(options.especialidades.map((e) => ({ value: e, label: e })), current.especialidad)],
    );
  if (user.rol === "EMPRESA")
    fields.push(
      ["razon_social", "Razón Social", "text", current.razon_social],
      ["sector_industrial", "Sector Industrial", "text", current.sector_industrial],
      ["tipo_convocatoria", "Tipo de convocatoria", "optional", pick([{ value: "RETO", label: "Reto" }, { value: "HACKATHON", label: "Hackathon" }, { value: "AMBOS", label: "Reto y hackathon" }], current.tipo_convocatoria)],
      ["area_responsable", "Área responsable", "text", current.area_responsable],
      ["sitio_web", "Sitio web", "url", current.sitio_web],
    );
  simpleForm("Mi Perfil", "/api/perfil", fields, (data) => ({
    ...data,
    primera_carrera: data.primera_carrera ? Number(data.primera_carrera) : null,
    semestre: data.semestre ? Number(data.semestre) : null,
    facultad: data.facultad ? Number(data.facultad) : null,
  }));
  const extra = el("p", "mt-3 mb-0");
  const change = el("button", "btn btn--secondary", "Cambiar contraseña");
  change.type = "button";
  change.onclick = () => {
    simpleForm(
      "Cambiar contraseña",
      "/api/password",
      [
        ["actual", "Contraseña actual", "password"],
        ["nueva", "Contraseña nueva (mínimo 12 caracteres)", "password"],
      ],
    );
    $("dialog-content").prepend(el("p", "small text-muted", "Deberás iniciar sesión nuevamente después del cambio."));
  };
  extra.append(change);
  $("dialog-content").append(extra);
}
// Label of a bare choice value used by the action forms (first model that defines it).
function choiceLabel(value) {
  for (const r of resources)
    for (const f of r.fields)
      for (const [k, v] of f.choices) if (k === value) return v;
  return human(value);
}
$("toggle-password").onclick = () => {
  const input = $("password");
  const show = input.type === "password";
  input.type = show ? "text" : "password";
  const btn = $("toggle-password");
  btn.replaceChildren(icon(show ? "eye-slash" : "eye"));
  btn.setAttribute("aria-label", show ? "Ocultar la contraseña" : "Mostrar la contraseña");
  btn.setAttribute("aria-pressed", String(show));
};
$("import-users").onclick = importForm;
$("bell").onclick = () => {
  $("bell-menu").hidden = !$("bell-menu").hidden;
  $("bell").setAttribute("aria-expanded", String(!$("bell-menu").hidden));
};
$("sidebar-toggle").onclick = () => {
  $("sidebar").classList.toggle("open");
  $("sidebar-overlay").classList.toggle("show");
};
$("sidebar-overlay").onclick = () => {
  $("sidebar").classList.remove("open");
  $("sidebar-overlay").classList.remove("show");
};
$("periodo").onchange = () => $("periodo-form").requestSubmit();
$("periodo-form").onsubmit = (e) => {
  e.preventDefault();
  const params = new URLSearchParams(new FormData(e.target));
  for (const [k, v] of [...params]) if (!v) params.delete(k);
  dashboardQuery = "?" + params;
  loadDashboard().catch(showError);
};
$("search-form").onsubmit = (e) => {
  e.preventDefault();
  query = $("search").value;
  page = 1;
  loadRecords().catch(showError);
};
$("previous").onclick = () => {
  if (page > 1) {
    page--;
    loadRecords().catch(showError);
  }
};
$("next").onclick = () => {
  if (page * 20 < total) {
    page++;
    loadRecords().catch(showError);
  }
};
window.addEventListener("hashchange", () => route().catch(showError));
start().catch(() => showLogin());
