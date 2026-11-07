"use strict";
function authField(parent, name, label, type = "text", options = null, value = "") {
  const wrap = el("div");
  const l = el("label", "form-label", label);
  l.htmlFor = "auth-" + name;
  let field;
  if (options) {
    field = el("select", "form-select");
    options.forEach(([v, t]) => field.append(new Option(t, v)));
  } else {
    field = el("input", "form-control");
    field.type = type;
  }
  field.id = "auth-" + name;
  field.name = name;
  // An empty value would deselect a <select>'s first option.
  if (value) field.value = value;
  if (type === "password") field.autocomplete = "new-password";
  wrap.append(l, field);
  parent.append(wrap);
  return field;
}
function authForm(title, subtitle, build, submitLabel, onSubmit, footer) {
  const box = $("auth-alt");
  box.replaceChildren(el("h2", "", title), el("p", "auth-subtitle", subtitle));
  const error = el("div", "alert alert-danger py-2 mb-3");
  error.hidden = true;
  const form = el("form");
  form.noValidate = true;
  build(form);
  const button = el("button", "btn btn-auth w-100 mt-4", submitLabel);
  form.append(button);
  form.onsubmit = async (e) => {
    e.preventDefault();
    error.hidden = true;
    button.disabled = true;
    try {
      await onSubmit(Object.fromEntries(new FormData(form)));
    } catch (err) {
      error.textContent = err.message;
      error.hidden = false;
    } finally {
      button.disabled = false;
    }
  };
  box.append(error, form);
  if (footer) box.append(footer);
}
function backToLogin(text = "¿Ya tienes cuenta?") {
  const p = el("p", "text-center mt-3 mb-0 auth-note", text + " ");
  const a = el("a", "text-link-green", "Inicia sesión aquí");
  a.href = "#";
  p.append(a);
  return p;
}
async function authRoute() {
  const hash = location.hash.slice(1);
  const alt = hash.startsWith("registro") || hash === "recuperar" || hash.startsWith("restablecer/");
  $("auth-login").hidden = alt;
  $("auth-alt").hidden = !alt;
  if (!alt) return;
  if (hash === "registro") {
    const box = $("auth-alt");
    box.replaceChildren(el("h2", "", "Crea tu cuenta"), el("p", "auth-subtitle", "Elige cómo vas a participar en RetosEAN"));
    for (const [target, title, text] of [
      ["registro/academico", "Comunidad EAN", "Si eres Estudiante o Profesor de la Universidad EAN, usa esta opción para validar tu correo institucional."],
      ["registro/empresa", "Empresas / Organizaciones", "Si eres un Aliado Comercial externo y deseas proponer retos o validar documentación legal, crea tu perfil corporativo aquí."],
    ]) {
      const a = el("a", "auth-choice text-decoration-none text-dark");
      a.href = "#" + target;
      a.append(el("div", "fw-bold", title), el("div", "small text-muted", text));
      box.append(a);
    }
    box.append(backToLogin());
    return;
  }
  const register = async (data) => {
    for (const k of ["programa_id", "facultad_id", "semestre"]) data[k] = data[k] ? Number(data[k]) : null;
    const session = await api("/api/registro", data);
    if (session.pendiente_aprobacion) {
      // A self-registered professor waits for admin approval: no session yet.
      $("auth-alt").replaceChildren(el("h2", "", "Cuenta pendiente de aprobación"),
        el("p", "auth-subtitle", session.mensaje), backToLogin(""));
      return;
    }
    csrf = session.csrf;
    location.hash = "dashboard";
    await start();
  };
  if (hash === "registro/academico" || hash === "registro/empresa") {
    const academic = hash.endsWith("academico");
    const options = await api("/api/registro/opciones");
    authForm(academic ? "Registro Comunidad EAN" : "Registro de Empresa",
      academic ? "Usa tu correo institucional @universidadean.edu.co" : "Crea el perfil corporativo de tu organización",
      (form) => {
        const hidden = el("input");
        hidden.type = "hidden";
        hidden.name = "tipo";
        hidden.value = academic ? "academico" : "empresa";
        form.append(hidden);
        if (academic) {
          const rol = authField(form, "rol", "Rol", "text", [["ESTUDIANTE", "Estudiante"], ["PROFESOR", "Profesor"]]);
          authField(form, "first_name", "Nombre");
          authField(form, "last_name", "Apellido");
          const programa = authField(form, "programa_id", "Programa", "text", [["", "Seleccione un programa"], ...options.programas.map((p) => [p.value, p.label])]).parentElement;
          const semestre = authField(form, "semestre", "Semestre", "text", Array.from({ length: 10 }, (_, i) => [String(i + 1), String(i + 1)])).parentElement;
          const facultad = authField(form, "facultad_id", "Facultad", "text", [["", "Seleccione una facultad"], ...options.facultades.map((f) => [f.value, f.label])]).parentElement;
          const especialidad = authField(form, "especialidad", "Especialidad", "text", [["", "Seleccione una especialidad"], ...options.especialidades.map((e) => [e, e])]).parentElement;
          const toggle = () => {
            const student = rol.value === "ESTUDIANTE";
            programa.hidden = semestre.hidden = !student;
            facultad.hidden = especialidad.hidden = student;
          };
          rol.onchange = toggle;
          toggle();
        } else {
          authField(form, "nit", "NIT de la Empresa");
          authField(form, "razon_social", "Razón social");
          authField(form, "sector_industrial", "Sector Industrial");
          authField(form, "tipo_convocatoria", "Tipo de convocatoria", "text", options.convocatorias);
          authField(form, "area_responsable", "Área responsable");
          authField(form, "sitio_web", "Sitio web", "url");
        }
        authField(form, "username", academic ? "Usuario" : "Usuario Corporativo");
        authField(form, "email", academic ? "Correo electrónico" : "Correo electrónico corporativo", "email");
        authField(form, "password", "Contraseña (mínimo 12 caracteres)", "password");
        authField(form, "password2", "Confirmar contraseña", "password");
      }, "Crear cuenta", register, backToLogin());
    return;
  }
  if (hash === "recuperar") {
    authForm("Recuperar contraseña", "Te enviaremos un enlace para elegir una nueva contraseña",
      (form) => authField(form, "email", "Correo electrónico", "email"),
      "Enviar enlace", async (data) => {
        await api("/api/password/reset", data);
        const box = $("auth-alt");
        box.replaceChildren(el("h2", "", "Revisa tu correo"),
          el("p", "auth-subtitle", "Si existe una cuenta con ese correo, te enviamos un enlace para restablecer la contraseña. El enlace vence en 3 días."),
          backToLogin("¿Recordaste tu contraseña?"));
      }, backToLogin("¿Recordaste tu contraseña?"));
    return;
  }
  const token = hash.slice("restablecer/".length);
  authForm("Elige una nueva contraseña", "Usa al menos 12 caracteres",
    (form) => {
      authField(form, "nueva", "Nueva contraseña", "password");
      authField(form, "nueva2", "Confirmar contraseña", "password");
    },
    "Cambiar contraseña", async (data) => {
      await api("/api/password/reset/confirm", { ...data, token });
      const box = $("auth-alt");
      box.replaceChildren(el("h2", "", "Contraseña actualizada"),
        el("p", "auth-subtitle", "Ya puedes iniciar sesión con tu nueva contraseña."), backToLogin(""));
    }, backToLogin());
}
// ── HU12: grading by free score or rubric; grades published per reto ──
