"use strict";
// ── Detalle de registro dentro del <dialog> (openRecord) ──
// Presentación con el design system: esqueleto mientras carga, ficha
// clave/valor, badges de estado, acciones y tablas responsivas.
// Aviso visible dentro del diálogo cuando falla una consulta secundaria: el
// resto del registro se conserva en vez de dejar la ficha a medias (FE-6).
function detail_loadError(message) {
  const note = el("div", "alert alert--danger small");
  note.setAttribute("role", "alert");
  note.append(icon("exclamation-triangle-fill"), el("span", "", message));
  return note;
}
async function openRecord(id) {
  const parent = modal("Cargando…");
  parent.setAttribute("aria-busy", "true");
  parent.replaceChildren(ui.skeleton(6, { kind: "row" }));
  let record;
  try {
    record = await api(`/api/records/${selected}/${id}`);
  } catch (error) {
    parent.removeAttribute("aria-busy");
    $("detail-dialog").close();
    throw error;
  }
  const heading =
    record.titulo ||
    record.razon_social ||
    record.nombre ||
    (record.reto && `${capfirst(meta(selected)?.verbose_name || selected)}: ${record.reto}`) ||
    `${capfirst(meta(selected)?.verbose_name || selected)} #${id}`;
  $("dialog-title").textContent = heading;
  parent.removeAttribute("aria-busy");
  parent.replaceChildren();

  const dl = document.createElement("dl");
  dl.className = "details";
  // Model field order, related names instead of raw foreign keys, codigo instead of its parts.
  const keys = (meta(selected)?.fields || [])
    .map((f) => (f.related_table && f.name.slice(0, -3) in record ? f.name.slice(0, -3) : f.name))
    .flatMap((k) => (selected === "retos" && k === "consecutivo" ? ["codigo"] : [k]))
    .filter((k) => k in record && !["id", "eliminado_en", "anio_consecutivo"].includes(k));
  for (const key of keys) {
    const value = record[key];
    const f = field(selected, key);
    const dt = document.createElement("dt");
    dt.textContent =
      key === "codigo" && selected === "retos"
        ? "Consecutivo"
        : capfirst(f?.verbose_name || field(selected, key + "_id")?.verbose_name || key);
    const dd = document.createElement("dd");
    const chosen = f?.choices.length ? choice(selected, key, value) : display(value);
    if (value && f?.type === "FileField") {
      const a = document.createElement("a");
      a.className = "btn btn--secondary btn--sm";
      a.href = `/api/archivo/${selected}/${id}/${key}`;
      a.append(icon("download"), el("span", "", "Descargar archivo"));
      dd.append(a);
    } else if (value != null && value !== "" && key.includes("estado")) {
      dd.append(el("span", "badge " + badgeClass(value) + " dot", chosen));
    } else {
      dd.textContent = chosen;
    }
    dl.append(dt, dd);
  }
  parent.append(dl);

  const actions = document.createElement("div");
  actions.className = "detail-actions";
  parent.append(actions);

  if (selected === "retos") {
    if (
      user.rol === "EMPRESA" &&
      ["borrador", "rechazado"].includes(record.estado)
    )
      action(actions, "Editar reto", () => challengeForm(record));
    if (user.rol === "EMPRESA" && record.estado === "borrador")
      action(
        actions,
        "Enviar a revisión",
        () => changeState(id, "en_revision"),
        true,
      );
    if (admin()) {
      if (record.estado === "en_revision") {
        action(actions, "Aprobar", () => changeState(id, "aprobado"), true);
        action(actions, "Rechazar", () => stateForm(id, "rechazado"));
      }
      if (["abierto", "pausado"].includes(record.estado))
        action(actions, "Iniciar / reanudar", () =>
          changeState(id, "en_curso"),
        );
      if (record.estado === "en_curso")
        action(actions, "Pausar", () => changeState(id, "pausado"));
      if (!["finalizado", "cancelado"].includes(record.estado))
        action(actions, "Cancelar reto", () => stateForm(id, "cancelado"));
    }
    if (user.rol === "ESTUDIANTE") {
      action(actions, "Alternar favorito", () =>
        mutate(`/api/retos/${id}/favorito`, {}),
      );
      if (record.estado === "abierto")
        action(actions, "Postularme", () => applicationForm(id), true);
      if (["abierto", "en_curso"].includes(record.estado))
        action(actions, "Entregar archivo", () => deliveryForm(id));
    }
    if (user.rol === "PROFESOR")
      action(actions, "Crear equipo", async () => {
        const options = await api(`/api/retos/${id}/opciones`);
        teamFields("Configurar Equipo Académico", `/api/retos/${id}/equipo`, "", options);
      });
    if (admin() && record.tipo === "hackathon")
      action(actions, "Crear hackathon", () =>
        simpleForm("Configurar hackathon", `/api/retos/${id}/hackathon`, [
          ["reglas", "Reglas", "textarea"],
        ]),
      );
    if (
      user.rol === "PROFESOR" &&
      ["aprobado", "abierto", "en_curso", "pausado"].includes(record.estado)
    )
      action(actions, "Integrar a mi curso", () => integrationForm(id));
    // Only the integrating professor (crear_rubrica); the server checks ownership.
    if (user.rol === "PROFESOR" && ["abierto", "en_curso", "pausado"].includes(record.estado))
      action(actions, "Crear rúbrica", () => rubricForm(id));
    // _puede_ver_seguimiento: admin, the owning company or the integrated professor.
    if (admin() || user.rol === "PROFESOR" || (user.rol === "EMPRESA" && record.empresa_id === user.id))
      action(actions, "Registrar seguimiento", () => progressForm(id));
    // HU14: the company's read-only progress view, with comments on deliverables.
    if (user.rol === "EMPRESA" && record.empresa_id === user.id && ["aprobado", "abierto", "en_curso", "pausado", "finalizado"].includes(record.estado))
      action(actions, "Ver avance", () => progressPanel(id));
    if (user.rol === "EMPRESA" && record.empresa_id === user.id && record.estado === "borrador")
      action(actions, "Eliminar reto", async () => {
        if (await ui.confirm("¿Eliminar este reto en borrador?", { title: "Eliminar reto", confirmText: "Eliminar" }))
          await mutate(`/api/retos/${id}/eliminar`, {});
      });
    if (admin() || (user.rol === "EMPRESA" && record.empresa_id === user.id && ["borrador", "rechazado"].includes(record.estado)))
      action(actions, "Adjuntar archivos de soporte", () => filesForm(`/api/retos/${id}/archivos`, "Archivos de soporte"));
    if (admin() && !["borrador", "cancelado"].includes(record.estado))
      action(actions, "Definir presupuesto", () =>
        simpleForm("Presupuesto", `/api/retos/${id}/presupuesto`, [
          ["monto_total", "Monto total (COP)", "number", "0.00"],
        ]),
      );
    // HU08: agenda, acta, final artifacts, recognitions and logistics.
    if (["ADMIN", "EMPRESA", "PROFESOR"].includes(user.rol) || admin())
      if (["aprobado", "abierto", "en_curso", "pausado", "finalizado"].includes(record.estado))
        action(actions, "Gestionar cierre", () => closurePanel(id));
    if ((admin() || user.rol === "PROFESOR") && record.estado === "en_curso")
      action(
        actions,
        "Cerrar y emitir certificados",
        () =>
          simpleForm(
            "Cierre formal",
            `/api/retos/${id}/cerrar`,
            [
              ["notas", "Notas del cierre", "textarea"],
              ["horas", "Horas certificadas", "number", "40"],
            ],
            (data) => ({ ...data, horas: Number(data.horas) }),
          ),
        true,
      );
  }
  if (
    selected === "notificaciones" &&
    record.usuario_id === user.id &&
    !record.leida
  )
    action(
      actions,
      "Marcar como leída",
      () => mutate(`/api/notificaciones/${id}/leer`, {}),
      true,
    );
  if (selected === "postulaciones" && record.estado === "PENDIENTE") {
    if (user.rol === "ESTUDIANTE")
      action(actions, "Retirar postulación", () =>
        mutate(`/api/postulaciones/${id}/estado`, { estado: "RETIRADA" }),
      );
    if (admin() || user.rol === "PROFESOR" || user.rol === "EMPRESA") {
      action(
        actions,
        "Aceptar",
        () => mutate(`/api/postulaciones/${id}/estado`, { estado: "ACEPTADA" }),
        true,
      );
      action(actions, "Rechazar", () =>
        mutate(`/api/postulaciones/${id}/estado`, { estado: "RECHAZADA" }),
      );
    }
  }
  if (
    selected === "usuarios" &&
    admin() &&
    record.id !== user.id &&
    record.rol !== "ADMIN"
  )
    action(
      actions,
      record.activo ? "Desactivar usuario" : "Activar usuario",
      () => mutate(`/api/usuarios/${id}/estado`, { activo: !record.activo }),
    );
  if (selected === "usuarios" && admin() && record.id !== user.id) {
    // HU13: role and per-module permissions.
    action(actions, "Cambiar rol", () =>
      simpleForm("Cambiar rol", `/api/usuarios/${id}/rol`, [
        ["rol", "Nuevo rol", "select", ["ADMIN", "EMPRESA", "PROFESOR", "ESTUDIANTE", "EXPERTO"].map((v) => ({ value: v, label: choiceLabel(v) }))],
      ]),
    );
    action(actions, "Permisos especiales", () => permissionsForm(id));
  }
  if (selected === "certificados")
    action(actions, "Verificar certificado", async () => {
      const data = await api(`/api/verificar/${record.codigo_verificacion}`);
      toast(`Certificado válido: ${data.participante} · ${data.reto}`);
    });
  if (selected === "certificados") {
    const a = document.createElement("a");
    a.className = "btn btn--primary btn--sm";
    a.href = `/api/certificados/${id}/pdf`;
    a.append(icon("award"), el("span", "", "Descargar certificado PDF"));
    actions.append(a);
  }
  if (selected === "integraciones_academicas") {
    let team = null;
    try {
      team = await api(`/api/integraciones/${id}/equipo`);
      parent.insertBefore(teamTables(team), actions);
    } catch {
      parent.insertBefore(detail_loadError("No se pudo cargar el equipo de la integración."), actions);
    }
    const editable =
      team &&
      (["borrador", "rechazada"].includes(record.estado) ||
        ["abierto", "en_curso"].includes(record.reto_estado));
    if (user.rol === "PROFESOR" && record.profesor_id === user.id && editable) {
      action(actions, "Editar integración", () => integrationForm(record.reto_id, record));
      action(actions, "Editar equipo y cronograma", () => teamForm(id, team));
    }
    if (user.rol === "PROFESOR" && record.profesor_id === user.id) {
      if (record.estado === "borrador")
        action(
          actions,
          "Enviar a revisión",
          () =>
            mutate(`/api/integraciones/${id}/estado`, {
              estado: "en_revision",
            }),
          true,
        );
      if (record.estado === "aprobada")
        action(
          actions,
          "Publicar integración",
          () =>
            mutate(`/api/integraciones/${id}/estado`, { estado: "publicada" }),
          true,
        );
    }
    // Admins and the company that owns the reto review it (HU14).
    if ((admin() || user.rol === "EMPRESA") && record.estado === "en_revision") {
      action(
        actions,
        "Aprobar integración",
        () =>
          simpleForm(
            "Aprobar integración",
            `/api/integraciones/${id}/estado`,
            [["comentario", "Comentario", "textarea"]],
            (data) => ({ ...data, estado: "aprobada" }),
          ),
        true,
      );
      action(actions, "Rechazar integración", () =>
        simpleForm(
          "Rechazar integración",
          `/api/integraciones/${id}/estado`,
          [["comentario", "Motivo", "textarea"]],
          (data) => ({ ...data, estado: "rechazada" }),
        ),
      );
    }
  }
  if (selected === "presupuestos") {
    const summary = await api(`/api/presupuestos/${id}/resumen`).catch(() => null);
    if (!summary) {
      parent.insertBefore(detail_loadError("No se pudo cargar el resumen del presupuesto."), actions);
    } else {
      const p = document.createElement("p");
      p.className = "text-soft small";
      p.textContent = `Comprometido: ${cop(summary.comprometido)} · Ejecutado: ${cop(summary.ejecutado)} · Disponible: ${cop(summary.disponible)} · ${summary.porcentaje}% asignado${summary.alerta ? " · Umbral de alerta alcanzado" : ""}`;
      parent.prepend(p);
      // HU07: expenses with their state; admins execute commitments or remove them.
      const list = el("div", "mb-3");
      list.append(
        ui.table(
          [
            { label: "Fecha" },
            { label: "Categoría" },
            { label: "Estado" },
            { label: "Descripción" },
            { label: "Monto", align: "right" },
          ],
          summary.gastos.map((g) => [fecha(g.creado_en), choiceLabel(g.categoria), choiceLabel(g.estado), g.descripcion, cop(g.monto)]),
          { empty: "Aún no se registran gastos.", caption: "Gastos del presupuesto" },
        ),
      );
      if (admin())
        for (const g of summary.gastos) {
          const line = el("div", "cluster cluster--sm small mb-2");
          line.append(el("span", "me-1 fw-semibold", `${choiceLabel(g.categoria)} ${cop(g.monto)}:`));
          if (g.estado === "COMPROMETIDO")
            action(line, "Marcar ejecutado", () => mutate(`/api/gastos/${g.id}/ejecutar`, {}));
          action(line, "Eliminar", async () => {
            if (await ui.confirm("¿Eliminar este gasto?", { title: "Eliminar gasto", confirmText: "Eliminar" }))
              await mutate(`/api/gastos/${g.id}/eliminar`, {});
          });
          list.append(line);
        }
      parent.insertBefore(list, actions);
      for (const [format, text] of [["xlsx", "Exportar gastos (Excel)"], ["pdf", "Exportar gastos (PDF)"]]) {
        const a = document.createElement("a");
        a.className = "btn btn--secondary btn--sm";
        a.href = `/api/retos/${summary.reto}/presupuesto/${format}`;
        a.append(icon("download"), el("span", "", text));
        actions.append(a);
      }
      if (admin())
        action(
          actions,
          "Registrar gasto",
          () =>
            simpleForm("Nuevo gasto", `/api/presupuestos/${id}/gastos`, [
              [
                "categoria",
                "Categoría",
                "select",
                ["REFRIGERIOS", "MATERIALES", "IMPRESIONES", "ESPACIOS", "OTROS"],
              ],
              ["monto", "Monto (COP)", "number", "0.00"],
              ["estado", "Estado", "select", [{ value: "EJECUTADO", label: "Ejecutado (pagado)" }, { value: "COMPROMETIDO", label: "Comprometido (reservado)" }]],
              ["descripcion", "Descripción", "textarea"],
            ]),
          true,
        );
    }
  }
  if (selected === "entregables" && user.rol === "PROFESOR") {
    action(actions, record.estado === "CALIFICADO" ? "Volver a calificar" : "Calificar", () => gradeForm(id, record), true);
    if (record.estado === "CALIFICADO" && !record.calificacion_publicada)
      action(actions, "Publicar calificaciones del reto", () =>
        mutate(`/api/retos/${record.reto_id}/publicar-calificaciones`, {}));
  }
  if (
    selected === "cierre_encuestasatisfaccion" &&
    !record.respondida_en &&
    record.participante_id === user.id
  )
    action(
      actions,
      "Responder encuesta",
      () =>
        simpleForm(
          "Tu experiencia",
          `/api/encuestas/${id}`,
          [
            [
              "calificacion",
              "Calificación de 1 a 5",
              "select",
              ["5", "4", "3", "2", "1"],
            ],
            ["comentario", "Comentario", "textarea"],
          ],
          (data) => ({ ...data, calificacion: Number(data.calificacion) }),
        ),
      true,
    );
  if (selected === "preferencias_notificacion" && record.usuario_id === user.id)
    action(actions, "Editar preferencias", () =>
      simpleForm(
        "Preferencias",
        `/api/preferencias`,
        [
          ["recibir_email", "Recibir correo", "select", ["Sí", "No"]],
          [
            "recibir_recordatorios",
            "Recibir recordatorios",
            "select",
            ["Sí", "No"],
          ],
          [
            "eventos_silenciados",
            "Eventos silenciados (separados por coma)",
            "text",
            (record.eventos_silenciados || []).join(","),
          ],
        ],
        (data) => ({
          recibir_email: data.recibir_email === "Sí",
          recibir_recordatorios: data.recibir_recordatorios === "Sí",
          eventos_silenciados: data.eventos_silenciados
            .split(",")
            .map((s) => s.trim())
            .filter(Boolean),
        }),
      ),
    );
  if (selected === "empresas" && admin()) {
    action(actions, "Revisar empresa", () =>
      simpleForm("Revisión de empresa", `/api/empresas/${id}/revisar`, [
        [
          "estado_validacion",
          "Validación",
          "select",
          ["PENDIENTE", "EN_REVISION", "VERIFICADA", "RECHAZADA"],
        ],
        [
          "estado_listas_restrictivas",
          "Listas restrictivas",
          "select",
          ["PENDIENTE", "APROBADO", "RECHAZADO"],
        ],
        ["observaciones", "Observaciones", "textarea"],
      ]),
    );
    action(actions, "Tramitar convenio", () =>
      simpleForm(
        "Convenio",
        `/api/empresas/${id}/convenio`,
        [
          [
            "estado",
            "Nuevo estado",
            "select",
            ["RADICADO", "EN_JURIDICA", "FIRMADO"],
          ],
          ["fecha_vencimiento", "Fecha de vencimiento", "date"],
          ["comentario", "Comentario", "textarea"],
        ],
        (data) => ({
          ...data,
          fecha_vencimiento: data.fecha_vencimiento || null,
        }),
      ),
    );
  }
  if (selected === "empresas" && (admin() || user.rol === "EMPRESA"))
    action(actions, "Historial documental", () => documentHistory(id));
  if (selected === "tipos_documento_legal" && admin())
    action(actions, "Editar tipo de documento", () => documentTypeForm(record));
  if (selected === "empresas" && admin())
    action(
      actions,
      record.participacion_bloqueada ? "Habilitar participación" : "Bloquear participación",
      () =>
        record.participacion_bloqueada
          ? mutate(`/api/empresas/${id}/bloqueo`, { bloquear: false })
          : simpleForm("Bloquear participación", `/api/empresas/${id}/bloqueo`, [["motivo", "Motivo del bloqueo", "textarea"]], (data) => ({ ...data, bloquear: true })),
    );
  if (selected === "empresas" && user.permisos.includes("empresas.cargar_listas_informa"))
    action(actions, "Cargar Listas Informa", () => documentForm(`/api/empresas/${id}/documentos/LISTAS_INFORMA/cargar`));
  if (selected === "empresas" && user.rol === "EMPRESA" && !record.renuncio_a_convenio)
    action(actions, "Participar sin convenio", async () => {
      const data = await api("/api/empresa/modo-convenio", { opcion: "no_convenio" });
      toast(data.motivos.length ? "Participarás sin convenio: no necesitas la minuta ni el convenio firmado. Carga el resto de documentos para poder publicar retos." : "Has seleccionado participar sin convenio.");
      $("detail-dialog").close();
      await loadRecords();
    });
  if (selected === "equipos_reto_academico" && user.rol === "PROFESOR") {
    const team = await api(`/api/equipos-academicos/${id}`).catch(() => null);
    if (team) {
      action(actions, "Editar equipo", () =>
        teamFields("Editar Equipo Académico", `/api/equipos-academicos/${id}`, team.nombre_equipo, team.opciones),
      );
      action(actions, "Eliminar equipo", async () => {
        if (await ui.confirm(`¿Eliminar el equipo "${team.nombre_equipo}"? También se elimina su equipo operativo.`, { title: "Eliminar equipo", confirmText: "Eliminar" }))
          await mutate(`/api/equipos-academicos/${id}/eliminar`, {});
      });
    } else {
      parent.insertBefore(detail_loadError("No se pudo cargar el equipo académico."), actions);
    }
  }
  if (selected === "unidades_estudio" && (admin() || user.rol === "PROFESOR")) {
    action(actions, "Editar unidad", () => unitForm(record));
    action(actions, record.activo ? "Desactivar unidad" : "Activar unidad", async () => {
      const data = await api(`/api/unidades/${id}/activo`, { activo: !record.activo });
      $("detail-dialog").close();
      toast(data.mensaje);
      await loadRecords();
    });
  }
  if (selected === "documentos_empresas" && admin())
    action(actions, "Revisar documento", () =>
      simpleForm("Documento legal", `/api/documentos/${id}/revisar`, [
        ["estado", "Decisión", "select", ["VERIFICADO", "RECHAZADO"]],
        ["comentario", "Motivo / comentario", "textarea"],
      ]),
    );
  if (selected === "documentos_empresas" && user.rol === "EMPRESA")
    action(actions, "Cargar documento", () => documentForm(`/api/documentos/${id}/cargar`));
  if (selected === "hackathons") {
    // HU09: logistics, guests, automatic close and the final report.
    const event = await api(`/api/hackathons/${id}/logistica`).catch(() => null);
    if (event) {
      const info = el("div", "mb-3");
      info.append(
        el("h6", "fw-bold mt-2", "Logística del evento"),
        el("p", "small mb-1", `Modalidad: ${label(MODALIDADES, event.modalidad)} · Espacio: ${event.lugar || "Por definir"}`),
      );
      if (event.enlace_transmision) {
        // Enlace del servidor: solo http(s) o relativas; cualquier otro
        // esquema se muestra como texto y nunca como enlace clicable (FE-1).
        const href = safeHref(event.enlace_transmision);
        if (href) {
          const link = el("a", "small", event.enlace_transmision);
          link.href = href;
          link.target = "_blank";
          link.rel = "noopener";
          info.append(link);
        } else {
          info.append(el("p", "small mb-1", `Transmisión: ${event.enlace_transmision}`));
        }
      }
      if (event.cierre && !record.resultados_publicados)
        info.append(el("p", "small text-muted mt-2 mb-1", `Los resultados se publican automáticamente el ${fecha(event.cierre, true)}.`));
      if (event.invitados.length)
        info.append(
          ui.table(
            ["Invitado", "Tipo", "Organización", "Confirmado"],
            event.invitados.map((g) => [g.nombre, label(TIPOS_INVITADO, g.tipo), g.organizacion, g.confirmado ? "Sí" : "Pendiente"]),
            { empty: "Sin invitados.", caption: "Invitados del evento" },
          ),
        );
      parent.insertBefore(info, actions);
      if (event.puede_gestionar && !["finalizado", "cancelado"].includes(record.estado))
        action(actions, "Logística e invitados", () => logisticsForm(id, event));
      if (event.puede_exportar)
        for (const [format, text] of [["pdf", "Informe final PDF"], ["xlsx", "Informe final Excel"]]) {
          const a = document.createElement("a");
          a.className = "btn btn--secondary btn--sm";
          a.href = `/api/hackathons/${id}/informe/${format}`;
          a.append(icon("download"), el("span", "", text));
          actions.append(a);
        }
    } else {
      parent.insertBefore(detail_loadError("No se pudo cargar la logística del evento."), actions);
    }
    if (admin() || user.rol === "PROFESOR") {
      const targets =
        {
          borrador: ["publicado", "cancelado"],
          publicado: ["en_curso", "cancelado"],
          en_curso: ["finalizado", "cancelado"],
        }[record.estado] || [];
      for (const estado of targets)
        action(actions, choice("hackathons", "estado", estado), () =>
          mutate(`/api/hackathons/${id}/estado`, { estado }),
        );
      if (!["finalizado", "cancelado"].includes(record.estado)) {
        action(actions, "Asignar jurado", async () => {
          const options = await api(`/api/hackathons/${id}/opciones`);
          simpleForm(
            "Asignar jurado",
            `/api/hackathons/${id}/jurado`,
            [
              ["usuario_id", "Experto o docente", "select", options.jurors],
              ["especialidad", "Especialidad"],
            ],
            (data) => ({ ...data, usuario_id: Number(data.usuario_id) }),
          );
        });
        action(actions, "Configurar etapa", () =>
          simpleForm(
            "Etapa del hackathon",
            `/api/hackathons/${id}/etapa`,
            [
              [
                "tipo",
                "Tipo",
                "select",
                ["inscripcion", "desarrollo", "presentacion", "premiacion"],
              ],
              ["titulo", "Título"],
              ["inicia_en", "Inicio", "datetime-local"],
              ["termina_en", "Fin", "datetime-local"],
              ["orden", "Orden", "number", "0"],
            ],
            (data) => ({
              ...data,
              orden: Number(data.orden),
              inicia_en: data.inicia_en
                ? new Date(data.inicia_en).toISOString()
                : null,
              termina_en: data.termina_en
                ? new Date(data.termina_en).toISOString()
                : null,
            }),
          ),
        );
      }
    }
    if (
      user.rol === "ESTUDIANTE" &&
      ["publicado", "en_curso"].includes(record.estado)
    )
      action(actions, "Inscribir mi equipo", async () => {
        const options = await api(`/api/hackathons/${id}/opciones`);
        simpleForm(
          "Inscripción",
          `/api/hackathons/${id}/inscribir`,
          [["equipo_id", "Mi equipo", "select", options.teams]],
          (data) => ({ equipo_id: Number(data.equipo_id) }),
        );
      });
    if (
      ["EXPERTO", "PROFESOR"].includes(user.rol) &&
      ["publicado", "en_curso"].includes(record.estado)
    )
      action(actions, "Votar como jurado", async () => {
        const options = await api(`/api/hackathons/${id}/opciones`);
        simpleForm(
          "Votación",
          `/api/hackathons/${id}/votar`,
          [
            ["equipo_id", "Equipo", "select", options.teams],
            ["puntaje", "Puntaje de 0 a 5", "number"],
            ["comentario", "Comentario", "textarea"],
          ],
          (data) => ({ ...data, equipo_id: Number(data.equipo_id) }),
        );
      });
    if (record.resultados_publicados || admin())
      action(actions, "Ver ranking", async () => {
        const data = await api(`/api/hackathons/${id}/ranking`);
        const content = modal("Ranking del hackathon");
        if (!data.ranking.length) {
          content.append(ui.empty("star", "Sin votos", "Todavía no se han registrado votos para este hackathon."));
          return;
        }
        const ol = document.createElement("ol");
        for (const row of data.ranking) {
          const li = document.createElement("li");
          li.textContent = `${row.equipo} · ${row.promedio} puntos · ${row.votos} ${row.votos === 1 ? "voto" : "votos"}`;
          ol.append(li);
        }
        content.append(ol);
      });
  }
  if (
    selected === "inscripciones_hackaton" &&
    (admin() || user.rol === "PROFESOR")
  )
    for (const estado of ["ACEPTADA", "RECHAZADA"])
      if (record.estado !== estado)
        action(actions, choice("inscripciones_hackaton", "estado", estado), () =>
          mutate(`/api/inscripciones/${id}/estado`, { estado }),
        );
}