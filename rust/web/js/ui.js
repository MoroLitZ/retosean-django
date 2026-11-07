"use strict";
/* ═══════════════════════════════════════════════════════════════════════════
   RetosEAN UI — small, dependency-free component helpers for view engineers.
   Everything is namespaced under `ui`, plus two globals: busy() and setPageTitle().

   These consume the design-system classes from components.css and the global
   `icon()`/`el()` helpers defined in core.js (which loads before this file).
   ═══════════════════════════════════════════════════════════════════════════ */

const ui = (() => {
  const h = (tag, className, text) => {
    const n = document.createElement(tag);
    if (className) n.className = className;
    if (text !== undefined) n.textContent = text;
    return n;
  };
  const ic = (name) =>
    typeof icon === "function" ? icon(name) : h("span", "icon");

  /* --- Button ------------------------------------------------ */
  function button(label, opts = {}) {
    const { variant = "secondary", icon: iconName, onClick, type = "button", disabled = false, title } = opts;
    const b = h("button", "btn btn--" + variant);
    b.type = type;
    b.disabled = !!disabled;
    if (title) b.title = title;
    if (iconName) b.append(ic(iconName));
    if (label) b.append(h("span", "", label));
    if (onClick) b.addEventListener("click", onClick);
    return b;
  }

  /* --- Status pill ------------------------------------------- */
  function badge(state, opts = {}) {
    const { dot = true, label } = opts;
    const valid = ["success", "warning", "danger", "info", "neutral", "brand"];
    const cls = valid.includes(state) ? "pill pill--" + state : "pill pill--neutral";
    const b = h("span", cls + (dot ? " dot" : ""));
    if (label !== undefined) b.append(h("span", "", label));
    return b;
  }

  /* --- Card / surface ---------------------------------------- */
  function card(title, body, actions) {
    const el = h("section", "surface");
    if (title) el.append(h("header", "surface__header", title));
    if (body) {
      const b = h("div", "surface__body");
      b.append(body);
      el.append(b);
    }
    if (actions && actions.childNodes.length) {
      const a = h("footer", "dialog-actions");
      a.append(actions);
      el.append(a);
    }
    return el;
  }

  /* --- Form field -------------------------------------------- */
  function field(opts = {}) {
    const {
      name, label: labelText, type = "text", value = "", required = false,
      hint, options, error, placeholder, disabled = false, step, min, max,
      accept, multiple = false, onChange, autocomplete,
    } = opts;
    const wrap = h("div", "field");
    if (type === "textarea") wrap.classList.add("field--full");

    /* Checkbox / switch use a compact layout. */
    if (type === "checkbox" || type === "switch") {
      const id = name ? "field-" + name : "";
      const input = h("input");
      input.type = "checkbox";
      input.checked = !!value;
      input.disabled = !!disabled;
      if (id) input.id = id;
      if (name) input.name = name;

      if (type === "switch") {
        wrap.className = "switch";
        const lbl = h("label");
        if (id) lbl.htmlFor = id;
        const track = h("span", "switch__track");
        lbl.append(input, track);
        if (labelText) lbl.append(h("span", "switch__label", labelText));
        wrap.append(lbl);
      } else {
        wrap.className = "checkbox";
        input.className = "checkbox__input";
        wrap.append(input);
        if (labelText) {
          const textLbl = h("label", "checkbox__label", labelText);
          if (id) textLbl.htmlFor = id;
          wrap.append(textLbl);
        }
      }
      return wrap;
    }

    let control;
    if (type === "select") {
      control = h("select", "select");
      control.append(...(options || []).map((o) => {
        const opt = h("option", "", typeof o === "string" ? o : o.label);
        opt.value = typeof o === "string" ? o : o.value;
        if (String(opt.value) === String(value)) opt.selected = true;
        return opt;
      }));
    } else if (type === "textarea") {
      control = h("textarea", "textarea");
      control.value = value ?? "";
    } else {
      control = h("input", "input");
      control.type = type;
      control.value = value ?? "";
      if (step !== undefined) control.step = step;
      if (min !== undefined) control.min = min;
      if (max !== undefined) control.max = max;
      if (accept) control.accept = accept;
      if (multiple) control.multiple = true;
      if (autocomplete) control.autocomplete = autocomplete;
    }

    if (name) {
      control.id = "field-" + name;
      control.name = name;
    }
    if (required) control.required = true;
    if (disabled) control.disabled = true;
    if (placeholder) control.placeholder = placeholder;
    if (onChange && control.addEventListener) control.addEventListener("input", onChange);

    if (labelText) {
      const lbl = h("label", "field__label");
      lbl.htmlFor = control.id || "";
      lbl.append(h("span", "", labelText));
      if (required) lbl.append(h("span", "req", " *"));
      wrap.append(lbl);
    }
    wrap.append(control);
    if (hint) wrap.append(h("p", "field__hint", hint));
    const err = h("p", "field__error");
    if (control.id) {
      err.id = control.id + "-error";
      control.setAttribute("aria-describedby", err.id);
    }
    if (error) {
      err.append(ic("exclamation-circle-fill"), h("span", "", error));
      control.setAttribute("aria-invalid", "true");
    }
    wrap.append(err);
    return wrap;
  }

  /* --- Table ------------------------------------------------- */
  function table(columns, rows, opts = {}) {
    const { empty = "No hay registros para mostrar.", caption } = opts;
    const wrap = h("div", "table-wrap");
    const tbl = h("table", "data-table is-hoverable");
    if (caption) tbl.append(h("caption", "", caption));
    const thead = h("thead");
    const trh = h("tr");
    for (const c of columns) {
      const key = typeof c === "string" ? c : c.key;
      const label = typeof c === "string" ? c : c.label || key;
      const th = h("th", "", label);
      th.setAttribute("scope", "col");
      if (typeof c === "object" && c.align === "right") th.classList.add("num");
      trh.append(th);
    }
    thead.append(trh);
    tbl.append(thead);
    const tbody = h("tbody");
    if (!rows.length) {
      const tr = h("tr");
      const td = h("td");
      td.colSpan = columns.length || 1;
      const box = h("div", "empty");
      box.append(h("div", "empty__title", empty));
      td.append(box);
      tr.append(td);
      tbody.append(tr);
    }
    for (const row of rows) {
      const tr = h("tr");
      columns.forEach((c, i) => {
        const key = typeof c === "string" ? c : c.key;
        const val = Array.isArray(row) ? row[i] : row[key];
        const td = h("td", "", val === undefined || val === null ? "—" : String(val));
        if (typeof c === "object" && c.align === "right") td.classList.add("num");
        tr.append(td);
      });
      tbody.append(tr);
    }
    tbl.append(tbody);
    wrap.append(tbl);
    return wrap;
  }

  /* --- Empty state ------------------------------------------- */
  function empty(iconName, titleText, hintText, actionNode) {
    const e = h("div", "empty");
    if (iconName) {
      const iconBox = h("div", "empty__icon");
      iconBox.append(ic(iconName));
      e.append(iconBox);
    }
    if (titleText) e.append(h("div", "empty__title", titleText));
    if (hintText) e.append(h("p", "empty__hint", hintText));
    if (actionNode) {
      const actionBox = h("div", "empty__action");
      actionBox.append(actionNode);
      e.append(actionBox);
    }
    return e;
  }

  /* --- Skeleton ---------------------------------------------- */
  function skeleton(n = 3, opts = {}) {
    const { kind = "row" } = opts;
    const box = h("div", "skeleton-wrap");
    box.setAttribute("aria-hidden", "true");
    for (let i = 0; i < n; i++) box.append(h("div", "skeleton skeleton--" + kind));
    return box;
  }

  /* --- Pagination -------------------------------------------- */
  function pagination(opts = {}) {
    const { page = 1, pages = 1, onPage, info } = opts;
    const nav = h("nav", "pagination");
    nav.setAttribute("aria-label", "Paginación");
    const prev = h("button", "pagination__btn", "Anterior");
    prev.prepend(ic("chevron-left"));
    prev.disabled = page <= 1;
    prev.onclick = () => onPage && onPage(page - 1);
    nav.append(prev);

    const start = Math.max(1, page - 2);
    const end = Math.min(pages, start + 4);
    for (let p = start; p <= end; p++) {
      const b = h("button", "pagination__page", String(p));
      if (p === page) b.setAttribute("aria-current", "page");
      b.onclick = () => p !== page && onPage && onPage(p);
      nav.append(b);
    }

    const next = h("button", "pagination__btn", "Siguiente");
    next.append(ic("chevron-right"));
    next.disabled = page >= pages;
    next.onclick = () => onPage && onPage(page + 1);
    nav.append(next);

    if (info) nav.append(h("span", "pagination__info", info));
    return nav;
  }

  /* --- Tabs -------------------------------------------------- */
  function tabs(items, opts = {}) {
    const { onChange } = opts;
    const root = h("div", "tabs-wrap");
    const list = h("div", "tabs");
    list.setAttribute("role", "tablist");
    const panels = h("div", "tabs__panels");
    items.forEach((item, i) => {
      const tab = h("button", "tabs__tab", item.label);
      tab.type = "button";
      tab.id = "tab-" + i;
      tab.setAttribute("role", "tab");
      tab.setAttribute("aria-controls", "panel-" + i);
      tab.setAttribute("aria-selected", String(i === 0));
      tab.tabIndex = i === 0 ? 0 : -1;
      tab.onclick = () => {
        items.forEach((_, j) => {
          const t = document.getElementById("tab-" + j);
          const p = document.getElementById("panel-" + j);
          if (!t || !p) return;
          t.setAttribute("aria-selected", String(i === j));
          t.tabIndex = i === j ? 0 : -1;
          p.hidden = i !== j;
        });
        onChange && onChange(i, item);
      };
      list.append(tab);

      const panel = h("section", "tabs__panel");
      panel.id = "panel-" + i;
      panel.setAttribute("role", "tabpanel");
      panel.setAttribute("aria-labelledby", tab.id);
      panel.hidden = i !== 0;
      panel.append(item.content || "");
      panels.append(panel);
    });
    root.append(list, panels);
    return root;
  }

  /* --- Confirm dialog (returns a Promise<boolean>) ----------- */
  function confirm(message, opts = {}) {
    const {
      title = "¿Estás seguro?",
      confirmText = "Confirmar",
      cancelText = "Cancelar",
      variant = "danger",
    } = opts;
    return new Promise((resolve) => {
      const dlg = h("dialog", "confirm-dialog");
      const top = h("header", "dialog-top");
      const titleEl = h("h2", "dialog-title", title);
      titleEl.id = "confirm-title";
      top.append(titleEl);
      dlg.setAttribute("aria-labelledby", "confirm-title");
      const body = h("div", "dialog-body");
      body.append(h("p", "confirm-message", message));
      const actions = h("footer", "dialog-actions");
      const cancel = h("button", "btn btn--secondary", cancelText);
      cancel.type = "button";
      const ok = h("button", "btn btn--" + variant, confirmText);
      ok.type = "button";
      actions.append(cancel, ok);
      dlg.append(top, body, actions);
      document.body.append(dlg);
      const done = (v) => {
        dlg.close();
        dlg.remove();
        resolve(v);
      };
      cancel.onclick = () => done(false);
      ok.onclick = () => done(true);
      dlg.addEventListener("cancel", () => done(false));
      dlg.showModal();
      ok.focus();
    });
  }

  return { button, badge, card, field, table, empty, skeleton, pagination, tabs, confirm };
})();

/* ── Global helpers ── */

/* Disable a button, show a spinner, and re-enable when the promise settles. */
function busy(button, promise) {
  if (!button) return promise;
  button.disabled = true;
  button.classList.add("is-loading");
  const spinner = document.createElement("span");
  spinner.className = "spinner";
  spinner.setAttribute("aria-hidden", "true");
  button.prepend(spinner);
  return Promise.resolve(promise).finally(() => {
    button.disabled = false;
    button.classList.remove("is-loading");
    spinner.remove();
  });
}

/* Set the page heading, document title and breadcrumbs.
   crumbs is an array of {label, href?}; the last is the current page. */
function setPageTitle(title, crumbs = []) {
  if (typeof title === "string") {
    const heading = document.getElementById("page-heading");
    if (heading) heading.textContent = title;
    const pageTitle = document.getElementById("page-title");
    if (pageTitle) pageTitle.textContent = title;
    document.title = `${title} — Retosean`;
  }
  const bc = document.getElementById("breadcrumbs");
  if (bc && crumbs) {
    bc.replaceChildren();
    const home = document.createElement("li");
    home.className = "breadcrumbs__item";
    const homeLink = document.createElement("a");
    homeLink.href = "#dashboard";
    homeLink.textContent = "Inicio";
    home.append(homeLink);
    bc.append(home);
    crumbs.forEach((c) => {
      const li = document.createElement("li");
      li.className = "breadcrumbs__item";
      if (c.href) {
        const a = document.createElement("a");
        a.href = c.href;
        a.textContent = c.label;
        li.append(a);
      } else {
        const span = document.createElement("span");
        span.setAttribute("aria-current", "page");
        span.textContent = c.label;
        li.append(span);
      }
      bc.append(li);
    });
  }
}

window.ui = ui;
window.busy = busy;
window.setPageTitle = setPageTitle;