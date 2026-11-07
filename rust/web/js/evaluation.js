"use strict";
async function gradeForm(id, record) {
  const { rubricas } = await api(`/api/entregables/${id}/rubricas`);
  const parent = modal("Evaluar entregable");
  const form = el("form");
  const modo = el("select", "form-select mb-2");
  modo.setAttribute("aria-label", "Forma de calificar");
  modo.append(new Option("Puntaje libre", ""));
  rubricas.forEach((r) => modo.append(new Option(`Rúbrica: ${r.nombre}`, r.id)));
  const libre = el("div");
  const nota = input(libre, "nota", `Nota (máximo ${record.puntaje_maximo})`, "", "number");
  nota.step = "0.01";
  nota.min = "0";
  const criterios = el("div");
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
  form.append(modo, libre, criterios);
  input(form, "comentario", "Retroalimentación", record.comentario_profesor || "", "textarea");
  let confirmar = null;
  if (record.estado === "CALIFICADO") {
    const wrap = el("div", "form-check mt-2");
    confirmar = el("input", "form-check-input");
    confirmar.type = "checkbox";
    confirmar.id = "confirmar-recalificacion";
    const l = el("label", "form-check-label", `Ya tiene nota (${record.nota}). Confirmo que quiero volver a calificarlo.`);
    l.htmlFor = confirmar.id;
    wrap.append(confirmar, l);
    form.append(wrap);
  }
  form.append(el("p", "small text-muted mt-2 mb-0", "La nota queda sin publicar hasta que publiques las calificaciones del reto."));
  const button = el("button", "primary", "Guardar evaluación");
  form.append(button);
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
  const form = el("form");
  input(form, "nombre", "Nombre", "", "text", true);
  input(form, "descripcion", "Descripción", "", "textarea");
  const criterios = editableRows(form, "Criterios", [
    { name: "nombre", label: "Criterio" },
    { name: "puntaje_maximo", label: "Puntaje máximo", type: "number" },
  ], [{}, {}]);
  const button = el("button", "primary", "Crear rúbrica");
  form.append(el("div", "mt-3"), button);
  submitForm(form, button, `/api/retos/${retoId}/rubrica`, () => ({
    nombre: form.elements.nombre.value,
    descripcion: form.elements.descripcion.value,
    criterios: criterios().filter((c) => c.nombre),
  }));
  parent.append(form);
}
// ── HU06: report builder (Django reportes/constructor.html) ──
