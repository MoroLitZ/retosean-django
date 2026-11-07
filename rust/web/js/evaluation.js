"use strict";
async function gradeForm(id, record) {
  const parent = wf_modalLoading("Evaluar entregable", 5);
  let rubricas;
  try {
    ({ rubricas } = await api(`/api/entregables/${id}/rubricas`));
  } catch (error) {
    wf_modalFailed(parent, error);
  }
  parent.removeAttribute("aria-busy");
  parent.replaceChildren();
  const form = el("form", "wf-form");
  const modoWrap = el("div", "field");
  const modoLabel = el("label", "field__label", "Forma de calificar");
  modoLabel.htmlFor = "wf-modo-rubrica";
  const modo = el("select", "select");
  modo.id = "wf-modo-rubrica";
  modoWrap.append(modoLabel, modo);
  modo.append(new Option("Puntaje libre", ""));
  rubricas.forEach((r) => modo.append(new Option(`Rúbrica: ${r.nombre}`, r.id)));
  const libre = el("div", "wf-stack");
  const nota = input(libre, "nota", `Nota (máximo ${record.puntaje_maximo})`, "", "number");
  nota.step = "0.01";
  nota.min = "0";
  const criterios = el("div", "wf-stack");
  const draw = () => {
    const rubrica = rubricas.find((r) => String(r.id) === modo.value);
    libre.hidden = !!rubrica;
    criterios.replaceChildren();
    for (const c of rubrica?.criterios || []) {
      const f = input(criterios, `criterio_${c.id}`, `${c.nombre} (máximo ${c.puntaje_maximo})`, "", "number");
      f.step = "0.01";
      f.min = "0";
      f.dataset.criterio = c.id;
    }
  };
  modo.onchange = draw;
  draw();
  form.append(modoWrap, libre, criterios);
  input(form, "comentario", "Retroalimentación", record.comentario_profesor || "", "textarea");
  let confirmar = null;
  if (record.estado === "CALIFICADO") {
    const wrap = el("div", "checkbox mt-2");
    confirmar = el("input", "checkbox__input");
    confirmar.type = "checkbox";
    confirmar.id = "confirmar-recalificacion";
    const l = el("label", "checkbox__label", `Ya tiene nota (${record.nota}). Confirmo que quiero volver a calificarlo.`);
    l.htmlFor = confirmar.id;
    wrap.append(confirmar, l);
    form.append(wrap);
  }
  form.append(el("p", "field__hint mt-2", "La nota queda sin publicar hasta que publiques las calificaciones del reto."));
  const button = ui.button("Guardar evaluación", { variant: "primary", type: "submit", icon: "check2-circle" });
  const actions = el("div", "wf-actions");
  actions.append(button);
  form.append(actions);
  submitForm(form, button, `/api/entregables/${id}/calificar`, () => ({
    nota: nota.value,
    rubrica_id: modo.value ? Number(modo.value) : null,
    criterios: Object.fromEntries([...criterios.querySelectorAll("input")].map((f) => [f.dataset.criterio, f.value])),
    comentario: form.elements.comentario.value,
    confirmar_recalificacion: !!confirmar?.checked,
  }));
  parent.append(form);
}
function rubricForm(retoId) {
  const parent = modal("Nueva rúbrica");
  const form = el("form", "wf-form");
  const campos = el("div", "wf-stack");
  input(campos, "nombre", "Nombre", "", "text", true);
  input(campos, "descripcion", "Descripción", "", "textarea");
  form.append(campos);
  const criterios = editableRows(form, "Criterios", [
    { name: "nombre", label: "Criterio" },
    { name: "puntaje_maximo", label: "Puntaje máximo", type: "number" },
  ], [{}, {}]);
  const button = ui.button("Crear rúbrica", { variant: "primary", type: "submit", icon: "plus-lg" });
  const actions = el("div", "wf-actions");
  actions.append(button);
  form.append(actions);
  submitForm(form, button, `/api/retos/${retoId}/rubrica`, () => ({
    nombre: form.elements.nombre.value,
    descripcion: form.elements.descripcion.value,
    criterios: criterios().filter((c) => c.nombre),
  }));
  parent.append(form);
}
// ── HU06: report builder (Django reportes/constructor.html) ──
