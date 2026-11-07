"use strict";
// ── Autenticación ──
// El formulario de inicio de sesión es estático en index.html. Aquí se
// renderizan dentro de #auth-alt: el selector de tipo de cuenta, el registro
// académico/empresa, la recuperación y el restablecimiento de contraseña.

// Construye un campo del design system (.field) con etiqueta, marcador de campo
// obligatorio, pista opcional, error inline y cableado aria. Los campos de
// contraseña incluyen un botón de mostrar/ocultar. Devuelve el control para que
// quien lo invoque pueda leer su valor u ocultarlo vía control.parentElement.
function authField(parent, name, label, spec = {}) {
  const {
    type = "text", options = null, value = "", required = false,
    hint = null, validate = null, autocomplete = null, placeholder = null,
  } = spec;
  const wrap = el("div", "field");
  const id = "auth-" + name;
  let control;
  if (options) {
    control = el("select", "select");
    for (const opt of options) {
      const [v, t] = Array.isArray(opt) ? opt : [opt.value, opt.label];
      const o = el("option", "", t);
      o.value = String(v);
      control.append(o);
    }
  } else {
    control = el("input", "input");
    control.type = type;
  }
  control.id = id;
  control.name = name;
  if (value) control.value = value;
  if (required) control.required = true;
  if (placeholder) control.placeholder = placeholder;
  if (autocomplete) control.autocomplete = autocomplete;
  else if (type === "password") control.autocomplete = "new-password";

  const lbl = el("label", "field__label", label);
  lbl.htmlFor = id;
  if (required) lbl.append(el("span", "req", " *"));
  wrap.append(lbl);

  if (type === "password") {
    const group = el("div", "input-group");
    group.append(control);
    const toggle = el("button", "input-group-text auth-toggle");
    toggle.type = "button";
    toggle.setAttribute("aria-label", "Mostrar la contraseña");
    toggle.setAttribute("aria-pressed", "false");
    toggle.append(icon("eye"));
    toggle.onclick = () => {
      const show = control.type === "password";
      control.type = show ? "text" : "password";
      toggle.replaceChildren(icon(show ? "eye-slash" : "eye"));
      toggle.setAttribute("aria-pressed", String(show));
      toggle.setAttribute(
        "aria-label",
        show ? "Ocultar la contraseña" : "Mostrar la contraseña",
      );
    };
    group.append(toggle);
    wrap.append(group);
  } else {
    wrap.append(control);
  }

  if (hint) wrap.append(el("p", "field__hint", hint));
  const error = el("p", "field__error");
  error.id = id + "-error";
  control.setAttribute("aria-describedby", error.id);
  wrap.append(error);

  if (parent._authFields)
    parent._authFields.push({ control, error, required, validate, wrap });
  parent.append(wrap);
  return control;
}

// Renderiza un formulario de autenticación y se encarga de la validación
// inline, el estado de envío (busy) y los errores de la API.
function authForm(title, subtitle, build, submitLabel, onSubmit, footer) {
  const box = $("auth-alt");
  box.replaceChildren(el("h2", "", title), el("p", "auth-subtitle", subtitle));
  const error = el("div", "alert alert--danger");
  error.hidden = true;
  const form = el("form", "auth-form");
  form.noValidate = true;
  form._authFields = [];
  build(form);
  const button = el("button", "btn btn--primary w-100 auth-submit", submitLabel);
  button.type = "submit";
  form.append(button);
  form.onsubmit = async (e) => {
    e.preventDefault();
    error.hidden = true;
    let firstBad = null;
    for (const f of form._authFields) {
      if (f.wrap.hidden) continue;
      const v = f.control.value;
      let msg = "";
      if (f.required && v.trim() === "") msg = "Este campo es obligatorio.";
      else if (v.trim() !== "" && f.validate) msg = f.validate(v) || "";
      if (msg) {
        f.error.replaceChildren(icon("exclamation-circle-fill"), el("span", "", msg));
        f.control.setAttribute("aria-invalid", "true");
        firstBad = firstBad || f.control;
      } else {
        f.error.replaceChildren();
        f.control.removeAttribute("aria-invalid");
      }
    }
    if (firstBad) {
      firstBad.focus();
      return;
    }
    const data = Object.fromEntries(new FormData(form));
    busy(button, onSubmit(data)).catch((err) => {
      error.replaceChildren(
        icon("exclamation-triangle-fill"),
        el("span", "", err.message),
      );
      error.hidden = false;
    });
  };
  box.append(error, form);
  if (footer) box.append(footer);
}

function backToLogin(text = "¿Ya tienes cuenta?") {
  const p = el("p", "text-center mt-3 mb-0 auth-note", text + " ");
  const a = el("a", "text-link", "Inicia sesión aquí");
  a.href = "#";
  p.append(a);
  return p;
}

// Pantalla de resultado (éxito / pendiente) centrada dentro de #auth-alt.
function authResult(title, message, iconName, footer) {
  const box = $("auth-alt");
  box.replaceChildren();
  const wrap = el("div", "auth-result");
  const icn = el("div", "auth-result__icon");
  icn.append(icon(iconName || "check-circle-fill"));
  wrap.append(icn, el("h2", "", title), el("p", "auth-subtitle", message));
  box.append(wrap);
  if (footer) box.append(footer);
}

async function authRoute() {
  const hash = location.hash.slice(1);
  const alt =
    hash.startsWith("registro") ||
    hash === "recuperar" ||
    hash.startsWith("restablecer/");
  $("auth-login").hidden = alt;
  $("auth-alt").hidden = !alt;
  if (!alt) return;
  if (hash === "registro") {
    const box = $("auth-alt");
    box.replaceChildren(
      el("h2", "", "Crea tu cuenta"),
      el("p", "auth-subtitle", "Elige cómo vas a participar en RetosEAN"),
    );
    for (const [target, title, text, iconName] of [
      [
        "registro/academico",
        "Comunidad EAN",
        "Si eres Estudiante o Profesor de la Universidad EAN, usa esta opción para validar tu correo institucional.",
        "people",
      ],
      [
        "registro/empresa",
        "Empresas / Organizaciones",
        "Si eres un Aliado Comercial externo y deseas proponer retos o validar documentación legal, crea tu perfil corporativo aquí.",
        "building",
      ],
    ]) {
      const a = el("a", "auth-choice");
      a.href = "#" + target;
      const inner = el("span", "auth-choice__wrap");
      const icn = el("span", "auth-choice__icon");
      icn.append(icon(iconName));
      const body = el("span", "auth-choice__body");
      body.append(
        el("span", "auth-choice__title", title),
        el("span", "auth-choice__desc", text),
      );
      const arrow = el("span", "auth-choice__arrow");
      arrow.append(icon("chevron-right"));
      inner.append(icn, body, arrow);
      a.append(inner);
      box.append(a);
    }
    box.append(backToLogin());
    return;
  }
  const register = async (data) => {
    for (const k of ["programa_id", "facultad_id", "semestre"])
      data[k] = data[k] ? Number(data[k]) : null;
    const session = await api("/api/registro", data);
    if (session.pendiente_aprobacion) {
      // Un profesor auto-registrado espera aprobación del administrador.
      authResult(
        "Cuenta pendiente de aprobación",
        session.mensaje,
        "hourglass",
        backToLogin(""),
      );
      return;
    }
    csrf = session.csrf;
    location.hash = "dashboard";
    await start();
  };
  const emailOk = (v) =>
    /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(v) ? "" : "Ingresa un correo válido.";
  if (hash === "registro/academico" || hash === "registro/empresa") {
    const academic = hash.endsWith("academico");
    let options;
    try {
      options = await api("/api/registro/opciones");
    } catch (error) {
      // Sin opciones no hay formulario: error visible en el panel en vez de
      // dejarlo vacío (FE-5).
      const box = $("auth-alt");
      const alert = el("div", "alert alert--danger mt-3");
      alert.setAttribute("role", "alert");
      alert.append(
        icon("exclamation-triangle-fill"),
        el("span", "", error.message || "No se pudo cargar el formulario de registro. Intenta de nuevo."),
      );
      box.replaceChildren(
        el("h2", "", academic ? "Registro Comunidad EAN" : "Registro de Empresa"),
        alert,
        backToLogin(""),
      );
      return;
    }
    authForm(
      academic ? "Registro Comunidad EAN" : "Registro de Empresa",
      academic
        ? "Usa tu correo institucional @universidadean.edu.co"
        : "Crea el perfil corporativo de tu organización",
      (form) => {
        const hidden = el("input");
        hidden.type = "hidden";
        hidden.name = "tipo";
        hidden.value = academic ? "academico" : "empresa";
        form.append(hidden);
        if (academic) {
          const rol = authField(form, "rol", "Rol", {
            type: "select",
            options: [
              ["ESTUDIANTE", "Estudiante"],
              ["PROFESOR", "Profesor"],
            ],
            required: true,
          });
          authField(form, "first_name", "Nombre", { required: true });
          authField(form, "last_name", "Apellido", { required: true });
          const programa = authField(form, "programa_id", "Programa", {
            type: "select",
            options: [
              ["", "Seleccione un programa"],
              ...options.programas.map((p) => [p.value, p.label]),
            ],
            required: true,
          }).parentElement;
          const semestre = authField(form, "semestre", "Semestre", {
            type: "select",
            options: Array.from({ length: 10 }, (_, i) => [
              String(i + 1),
              String(i + 1),
            ]),
          }).parentElement;
          const facultad = authField(form, "facultad_id", "Facultad", {
            type: "select",
            options: [
              ["", "Seleccione una facultad"],
              ...options.facultades.map((f) => [f.value, f.label]),
            ],
            required: true,
          }).parentElement;
          const especialidad = authField(form, "especialidad", "Especialidad", {
            type: "select",
            options: [
              ["", "Seleccione una especialidad"],
              ...options.especialidades.map((e) => [e, e]),
            ],
            required: true,
          }).parentElement;
          const toggle = () => {
            const student = rol.value === "ESTUDIANTE";
            programa.hidden = semestre.hidden = !student;
            facultad.hidden = especialidad.hidden = student;
          };
          rol.onchange = toggle;
          toggle();
        } else {
          authField(form, "nit", "NIT de la Empresa", { required: true });
          authField(form, "razon_social", "Razón social", { required: true });
          authField(form, "sector_industrial", "Sector industrial");
          authField(form, "tipo_convocatoria", "Tipo de convocatoria", {
            type: "select",
            options: options.convocatorias,
          });
          authField(form, "area_responsable", "Área responsable");
          authField(form, "sitio_web", "Sitio web", { type: "url" });
        }
        authField(form, "username", academic ? "Usuario" : "Usuario Corporativo", {
          required: true,
        });
        authField(
          form,
          "email",
          academic ? "Correo electrónico" : "Correo electrónico corporativo",
          { type: "email", required: true, validate: emailOk },
        );
        const password = authField(form, "password", "Contraseña", {
          type: "password",
          required: true,
          hint: "Mínimo 12 caracteres.",
        });
        authField(form, "password2", "Confirmar contraseña", {
          type: "password",
          required: true,
          validate: (v) =>
            v !== password.value ? "Las contraseñas no coinciden." : "",
        });
      },
      "Crear cuenta",
      register,
      backToLogin(),
    );
    return;
  }
  if (hash === "recuperar") {
    authForm(
      "Recuperar contraseña",
      "Te enviaremos un enlace para elegir una nueva contraseña",
      (form) =>
        authField(form, "email", "Correo electrónico", {
          type: "email",
          required: true,
          validate: emailOk,
        }),
      "Enviar enlace",
      async (data) => {
        await api("/api/password/reset", data);
        authResult(
          "Revisa tu correo",
          "Si existe una cuenta con ese correo, te enviamos un enlace para restablecer la contraseña. El enlace vence en 3 días.",
          "envelope",
          backToLogin("¿Recordaste tu contraseña?"),
        );
      },
      backToLogin("¿Recordaste tu contraseña?"),
    );
    return;
  }
  const token = hash.slice("restablecer/".length);
  authForm(
    "Elige una nueva contraseña",
    "Usa al menos 12 caracteres",
    (form) => {
      const nueva = authField(form, "nueva", "Nueva contraseña", {
        type: "password",
        required: true,
        hint: "Mínimo 12 caracteres.",
        validate: (v) =>
          v.length >= 12 ? "" : "La contraseña debe tener al menos 12 caracteres.",
      });
      authField(form, "nueva2", "Confirmar contraseña", {
        type: "password",
        required: true,
        validate: (v) => (v !== nueva.value ? "Las contraseñas no coinciden." : ""),
      });
    },
    "Cambiar contraseña",
    async (data) => {
      await api("/api/password/reset/confirm", { ...data, token });
      authResult(
        "Contraseña actualizada",
        "Ya puedes iniciar sesión con tu nueva contraseña.",
        "check-circle-fill",
        backToLogin(""),
      );
    },
    backToLogin(),
  );
}