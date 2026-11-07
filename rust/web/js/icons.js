"use strict";
/* ═══════════════════════════════════════════════════════════════════════════
   RetosEAN inline SVG icon set
   Hand-written 24×24 stroke icons in the spirit of Lucide/Feather. Each value is
   the *inner* markup of an <svg>; core.js `icon(name)` wraps it with a 24×24
   viewBox, stroke=currentColor, fill=none, and proper a11y attributes.

   Aliasing keeps every icon name currently used by the app rendering no matter
   whether view files use the old Bootstrap-Icons flavour or the graph set above.
   Unknown names fall back to ICONS.circle (a neutral dot) — never breaks.
   ═══════════════════════════════════════════════════════════════════════════ */

const ICONS = {
  /* Navigation / app */
  speedometer2: "<path d='m12 14 4-4'/><path d='M3.34 19a10 10 0 1 1 17.32 0'/>",
  people: "<path d='M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2'/><circle cx='9' cy='7' r='4'/><path d='M22 21v-2a4 4 0 0 0-3-3.87'/><path d='M16 3.13a4 4 0 0 1 0 7.75'/>",
  person: "<path d='M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2'/><circle cx='12' cy='7' r='4'/>",
  "person-circle": "<circle cx='12' cy='12' r='10'/><circle cx='12' cy='10' r='3'/><path d='M7 20.66V19a5 5 0 0 1 10 0v1.66'/>",
  building: "<path d='M6 22V4a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v18Z'/><path d='M6 12H4a2 2 0 0 0-2 2v6a2 2 0 0 0 2 2h2'/><path d='M18 9h2a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2h-2'/><path d='M10 6h4'/><path d='M10 10h4'/><path d='M10 14h4'/><path d='M10 18h4'/>",
  trophy: "<path d='M6 9H4.5a2.5 2.5 0 0 1 0-5H6'/><path d='M18 9h1.5a2.5 2.5 0 0 0 0-5H18'/><path d='M4 22h16'/><path d='M10 14.66V17c0 .55-.47.98-.97 1.21C7.85 18.75 7 20.24 7 22'/><path d='M14 14.66V17c0 .55.47.98.97 1.21 1.18.54 2.03 2.03 2.03 3.79'/><path d='M18 2H6v7a6 6 0 0 0 12 0V2Z'/>",
  "link-45deg": "<path d='M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.71 1.71'/><path d='M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71'/>",
  flag: "<path d='M4 15s1-1 4-1 5 2 8 2 4-1 4-1V3s-1 1-4 1-5-2-8-2-4 1-4 1z'/><line x1='4' x2='4' y1='22' y2='15'/>",
  "bar-chart-line": "<path d='M3 3v18h18'/><path d='M18 17V9'/><path d='M13 17V5'/><path d='M8 17v-3'/>",
  "cash-coin": "<circle cx='12' cy='12' r='10'/><path d='M16 8h-6a2 2 0 0 0 0 4h4a2 2 0 0 1 0 4H8'/><path d='M12 6v2'/><path d='M12 16v2'/>",
  "journal-bookmark-fill": "<path d='M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2Z' fill='currentColor' stroke='none'/><path d='M8 7h6' stroke='#fff'/><path d='M8 11h4' stroke='#fff'/>",
  "plus-circle": "<circle cx='12' cy='12' r='10'/><path d='M8 12h8'/><path d='M12 8v8'/>",
  "person-plus": "<path d='M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2'/><circle cx='9' cy='7' r='4'/><line x1='19' x2='19' y1='8' y2='14'/><line x1='22' x2='16' y1='11' y2='11'/>",
  "person-check": "<path d='M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2'/><circle cx='9' cy='7' r='4'/><path d='m16 11 2 2 4-4'/>",
  "file-earmark-text": "<path d='M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8Z'/><path d='M14 2v6h6'/><path d='M16 13H8'/><path d='M16 17H8'/><path d='M10 9H8'/>",
  "file-earmark-ruled": "<path d='M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8Z'/><path d='M14 2v6h6'/><path d='M16 13H8'/><path d='M16 17H8'/><path d='M10 9H8'/><path d='M8 13v4'/>",
  activity: "<path d='M22 12h-2.48a2 2 0 0 0-1.93 1.46l-2.35 8.36a.25.25 0 0 1-.48 0L9.24 2.18a.25.25 0 0 0-.48 0l-2.35 8.36A2 2 0 0 1 4.49 12H2'/>",
  "chat-square-text": "<path d='M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z'/><path d='M8 7h8'/><path d='M8 11h5'/>",
  "journal-text": "<path d='M8 3H7a2 2 0 0 0-2 2v5a2 2 0 0 1-2 2 2 2 0 0 1 2 2v5a2 2 0 0 0 2 2h1'/><path d='M16 3h1a2 2 0 0 1 2 2v5a2 2 0 0 0 2 2 2 2 0 0 0-2 2v5a2 2 0 0 1-2 2h-1'/><path d='M8 7h3'/><path d='M8 11h3'/>",
  "diagram-3": "<circle cx='18' cy='5' r='3'/><circle cx='6' cy='12' r='3'/><circle cx='18' cy='19' r='3'/><line x1='8.59' x2='15.42' y1='13.51' y2='17.49'/><line x1='15.41' x2='8.59' y1='6.51' y2='10.49'/>",
  "folder-check": "<path d='M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z'/><path d='m9 14 2 2 4-4'/>",
  "folder2-open": "<path d='m6 14 1.45-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.55 6a2 2 0 0 1-1.94 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.93a2 2 0 0 1 1.66.9l.82 1.2a2 2 0 0 0 1.66.9H18a2 2 0 0 1 2 2v2'/>",
  "clipboard2-check": "<rect width='8' height='4' x='8' y='2' rx='1'/><path d='M8 4H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V6a2 2 0 0 0-2-2h-2'/><path d='m9 14 2 2 4-4'/>",
  search: "<circle cx='11' cy='11' r='8'/><path d='m21 21-4.3-4.3'/>",
  send: "<path d='M14.536 21.686a.5.5 0 0 0 .937-.024l6.5-19a.496.496 0 0 0-.635-.635l-19 6.5a.5.5 0 0 0-.024.937l7.93 3.18a2 2 0 0 1 1.112 1.11Z'/><path d='m21.854 2.147-10.94 10.939'/>",
  star: "<polygon points='12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2'/>",
  "star-half": "<path d='M12 2v15.3l-6.2 3.4 1.6-7.2L2 9.2l7.2-.6z' fill='currentColor' stroke='none'/><path d='M12 2l2.8 6.6 7.2.6-5.4 4.6 1.6 7.2-6.2-3.4'/>",
  award: "<circle cx='12' cy='8' r='6'/><path d='M15.477 12.89 17 22l-5-3-5 3 1.523-9.11'/>",
  "lightning-charge": "<path d='M13 2 3 14h7l-1 8 10-12h-7z'/>",
  table: "<path d='M12 3v18'/><rect width='18' height='18' x='3' y='3' rx='2'/><path d='M3 9h18'/><path d='M3 15h18'/>",

  /* Session / auth */
  "box-arrow-left": "<path d='M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4'/><path d='m16 17-5-5 5-5'/><path d='M21 12H9'/>",
  "box-arrow-right": "<path d='M15 3h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4'/><path d='m8 7-5 5 5 5'/><path d='M3 12h13'/>",
  "box-arrow-in-right": "<path d='M15 3h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4'/><path d='m12 17 5-5-5-5'/><path d='M17 12H3'/>",
  lock: "<rect width='18' height='10' x='3' y='11' rx='2'/><path d='M7 11V7a5 5 0 0 1 10 0v4'/>",
  eye: "<path d='M2.06 12.35a1 1 0 0 1 0-.7 10.75 10.75 0 0 1 19.88 0 1 1 0 0 1 0 .7 10.75 10.75 0 0 1-19.88 0'/><circle cx='12' cy='12' r='3'/>",
  "eye-slash": "<path d='M9.88 9.88a3 3 0 1 0 4.24 4.24'/><path d='M10.73 5.08A10.43 10.43 0 0 1 12 5c7 0 10 7 10 7a13.16 13.16 0 0 1-1.67 2.68'/><path d='M6.61 6.61A13.53 13.53 0 0 0 2 12s3 7 10 7a9.74 9.74 0 0 0 5.39-1.61'/><line x1='2' x2='22' y1='2' y2='22'/>",

  /* Notifications / states */
  bell: "<path d='M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9'/><path d='M10.3 21a1.94 1.94 0 0 0 3.4 0'/>",
  "bell-slash": "<path d='M13.73 21a2 2 0 0 1-3.46 0'/><path d='M18.63 13A17.89 17.89 0 0 1 18 8'/><path d='M6.26 6.26A5.86 5.86 0 0 0 6 8c0 7-3 9-3 9h14'/><path d='M18 8a6 6 0 0 0-9.33-5'/><line x1='2' x2='22' y1='2' y2='22'/>",
  "check-circle": "<circle cx='12' cy='12' r='10'/><path d='m9 12 2 2 4-4'/>",
  "check2-circle": "<circle cx='12' cy='12' r='10'/><path d='m9 12 2 2 4-4'/>",
  "check-square": "<rect width='18' height='18' x='3' y='3' rx='2'/><path d='m9 12 2 2 4-4'/>",
  "check2-square": "<rect width='18' height='18' x='3' y='3' rx='2'/><path d='m9 12 2 2 4-4'/>",
  "check-circle-fill": "<circle cx='12' cy='12' r='10' fill='currentColor' stroke='none'/><path d='m8 12.5 2.5 2.5L16 9.5' fill='none' stroke='#fff' stroke-width='2'/>",
  "x-circle": "<circle cx='12' cy='12' r='10'/><path d='m15 9-6 6'/><path d='m9 9 6 6'/>",
  "x-circle-fill": "<circle cx='12' cy='12' r='10' fill='currentColor' stroke='none'/><path d='m9 9 6 6M15 9l-6 6' stroke='#fff' stroke-width='2' fill='none'/>",
  "info-circle-fill": "<circle cx='12' cy='12' r='10' fill='currentColor' stroke='none'/><path d='M12 11v5' stroke='#fff' stroke-width='2' fill='none'/><circle cx='12' cy='8' r='1.2' fill='#fff' stroke='none'/>",
  "exclamation-circle-fill": "<circle cx='12' cy='12' r='10' fill='currentColor' stroke='none'/><path d='M12 7v5' stroke='#fff' stroke-width='2' fill='none'/><circle cx='12' cy='16.5' r='1.2' fill='#fff' stroke='none'/>",
  "exclamation-triangle": "<path d='M10.29 3.86 1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0Z'/><path d='M12 9v4'/><path d='M12 17h.01'/>",
  "exclamation-triangle-fill": "<path d='M10.29 3.86 1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0Z' fill='currentColor' stroke='none'/><path d='M12 9v4' stroke='#fff' stroke-width='2' fill='none'/><circle cx='12' cy='16.5' r='1.2' fill='#fff' stroke='none'/>",

  /* Workflows / actions */
  "play-circle": "<circle cx='12' cy='12' r='10'/><path d='m10 8 6 4-6 4z'/>",
  "hourglass-split": "<path d='M5 22h14'/><path d='M5 2h14'/><path d='M17 22v-6.17a2 2 0 0 0-.59-1.42L12 10l-4.41 4.41A2 2 0 0 0 7 15.83V22'/><path d='M7 2v6.17a2 2 0 0 0 .59 1.42L12 14l4.41-4.41A2 2 0 0 0 17 8.17V2'/>",
  hourglass: "<path d='M5 22h14'/><path d='M5 2h14'/><path d='M17 22v-6.17a2 2 0 0 0-.59-1.42L12 10l-4.41 4.41A2 2 0 0 0 7 15.83V22'/><path d='M7 2v6.17a2 2 0 0 0 .59 1.42L12 14l4.41-4.41A2 2 0 0 0 17 8.17V2'/>",
  mortarboard: "<path d='M22 10 12 5 2 10l10 5 10-5z'/><path d='M6 12v5c0 1.7 2.7 3 6 3s6-1.3 6-3v-5'/><path d='M22 10v6'/>",
  "person-video3": "<path d='m16 13 5.22 3.15a1 1 0 0 1 .78.93V6.92a1 1 0 0 0-.78-.93L16 9'/><rect x='2' y='6' width='14' height='12' rx='2'/>",
  "clock-history": "<path d='M3 12a9 9 0 1 1 9 9'/><path d='M3 3v6h6'/><path d='M12 7v5l3 2'/>",
  "cloud-arrow-up": "<path d='M12 13v8'/><path d='m8 17 4-4 4 4'/><path d='M20.39 18.39A5 5 0 0 0 18 9h-1.26A8 8 0 1 0 3 16.3'/><path d='m16 16 6-6-3-3-6 6'/>",
  "plus-lg": "<path d='M5 12h14'/><path d='M12 5v14'/>",
  download: "<path d='M12 3v12'/><path d='m7 10 5 5 5-5'/><path d='M5 21h14'/>",
  upload: "<path d='M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4'/><path d='m7 10 5-5 5 5'/><path d='M12 5v12'/>",
  "envelope-paper": "<rect width='20' height='16' x='2' y='4' rx='2'/><path d='m22 7-8.97 5.7a1.94 1.94 0 0 1-2.06 0L2 7'/>",
  list: "<path d='M8 6h13'/><path d='M8 12h13'/><path d='M8 18h13'/><path d='M3 6h.01'/><path d='M3 12h.01'/><path d='M3 18h.01'/>",
  funnel: "<path d='M3 4h18l-7 8v6l-4 3v-9z'/>",
  calendar3: "<rect width='18' height='18' x='3' y='4' rx='2'/><path d='M16 2v4'/><path d='M8 2v4'/><path d='M3 10h18'/>",
  "chevron-left": "<path d='m15 18-6-6 6-6'/>",
  "chevron-right": "<path d='m9 18 6-6-6-6'/>",
  x: "<path d='M18 6 6 18'/><path d='m6 6 12 12'/>",
  check: "<path d='M20 6 9 17l-5-5'/>",
  dot: "<circle cx='12' cy='12' r='5' fill='currentColor' stroke='none'/>",
  circle: "<circle cx='12' cy='12' r='8'/>",
};

/* Legacy Bootstrap-icons spellings and graph aliases still referenced by views. */
(function () {
  const alias = (target, name) => {
    if (ICONS[target] && !ICONS[name]) ICONS[name] = ICONS[target];
  };
  alias("bar-chart-line", "bar-chart");
  alias("envelope-paper", "envelope");
  alias("file-earmark-text", "filetype-csv");
  alias("file-earmark-text", "file-earmark-pdf");
  alias("file-earmark-text", "file-earmark-excel");
  alias("check-circle-fill", "check2-circle-fill");
  alias("x", "x-lg");
  alias("info-circle-fill", "info-circle");
})();

/* Expose for classic-script global scope (loaded before core.js) */
window.ICONS = ICONS;