# Optimización de memoria de la versión Rust (noviembre de 2026)

Commits: `c48e05f59` (cambios principales), `a8dcef6d9` (SPA: respuestas de detalle obsoletas),
`8851085c4` (endurecimiento tras la revisión independiente). Línea base: `dbf4b2340`.

## Método

- Arnés reproducible (Python + psutil): base de datos nueva (`init-db`, `seed-demo` y 6 000
  notificaciones adicionales), binario release, muestreo del working set y de la memoria privada del
  proceso cada 50 ms, fases fijas: reposo 5 s; generador de carga incluido (`examples/stress.rs`,
  8 s por nivel, 1/100/500/1 000 conexiones); 30 exportaciones concurrentes (10 XLSX, 10 PDF,
  10 CSV de 5 000 filas); 20 reportes del constructor; 4 previsualizaciones de importación de
  ~5 MB; 4 cargas concurrentes de 2 × 45 MB; 8 descargas concurrentes de 45 MB; reposo 10 s.
- 3 corridas por variante, intercaladas, con la máquina sin otra carga (Windows, 24 núcleos,
  PostgreSQL 18 local). Se reporta la mediana y el rango.
- Variantes medidas: base, solo perfil release, versión optimizada y versión optimizada con
  mimalloc (descartada, ver abajo).

## Resultados (pico de working set, MB; mediana de 3 corridas)

| Fase | Antes | Después | Cambio |
|---|---:|---:|---:|
| Reposo | 15,0 | 14,7 | −2 % |
| Carga HTTP (hasta 1 000 conexiones) | 89,1 | 88,5 | −1 % |
| 30 exportaciones concurrentes | 317,5 | 146,3 | −54 % |
| Reportes del constructor | 42,8 | 39,0 | −9 % |
| 4 importaciones de 5 MB | 152,7 | 119,9 | −21 % |
| 4 cargas de 2 × 45 MB | 527,4 | 82,7 | −84 % |
| 8 descargas de 45 MB | 375,3 | 48,6 | −87 % |
| Memoria retenida 10 s después | 122,1 | 39,0 | −68 % |

Memoria privada: cargas 610,9 → 76,3 MB (−88 %), descargas 479,8 → 40,5 MB (−92 %),
exportaciones 342,7 → 140,7 MB (−59 %), retenida 119,1 → 28,9 MB (−76 %).
Binario: 16,8 MB → 13,8 MB (−18 %).

Rendimiento sin cambios dentro del ruido: 1 157 frente a 1 134 sol/s con 1 conexión y
~6 400–6 600 sol/s entre 100 y 1 000 conexiones, con p50 y p99 equivalentes. Los errores de conexión
esporádicos a 1 000 conexiones aparecen en ambas versiones (repetición aislada: base 0/0/138/48,
optimizada 54/0/0/0) y provienen del cliente y de la pila TCP de Windows, no del servidor. Las
30 exportaciones tardan más en conjunto (0,8 → 1,5 s) porque ahora se atienden de 4 en 4.

SPA (Chromium, 25 ciclos tablero → retos → detalle → cerrar, tras recolección de basura):
heap JS 5,15 → 4,43 MB, listeners 112 → 90; tras cerrar sesión, heap 5,18 → 4,38 MB y listeners
108 → 78; sin lienzos de gráficas retenidos tras cerrar sesión (4 → 0).

## Cambios

- Cargas (`media::read_with_files`, `save_attachments`): cada archivo se escribe por fragmentos en un
  temporal privado (`.<uuid>.part`) mientras llega; el tipo se valida de forma incremental (bytes
  mágicos y UTF-8 completo sin NUL, incluidos caracteres partidos entre fragmentos) y el archivo se
  renombra a su clave final solo si pasa la validación. Un guardián elimina el temporal en cualquier
  error; los temporales huérfanos de una caída se borran al arrancar.
- Descargas (`media::download`): se transmiten en bloques de 64 KiB con `Content-Length`.
- Documentos pequeños (`limited_bytes`): un único búfer del tamaño exacto en lugar de un vector que
  duplica su capacidad.
- Exportaciones (`resources::export`, `reports::export`, `pdf_lines`): se mueven los registros fuera
  del JSON y se libera el árbol de inmediato; CSV escrito en un solo búfer; PDF sin segunda copia de
  las líneas. La neutralización de fórmulas quedó en una sola función (`domain::csv_text`).
- Importación: se libera el archivo crudo tras decodificarlo y las celdas se mueven en lugar de
  copiarse.
- Concurrencia acotada: 4 turnos para exportaciones, portafolio, constructor de reportes e
  importación (`heavy_permit`), tomados después de la autorización (la importación, además, después
  de leer el cuerpo) y con espera máxima de 30 s (503 al vencer). Limita la memoria bajo ráfagas y
  reduce la superficie de denegación de servicio.
- `lifecycle::progress`: si falla cualquier inserción tras guardar los adjuntos, los archivos se
  eliminan (antes solo se eliminaban si fallaba el commit).
- Perfil release: LTO completo, una unidad de generación y sin información de depuración;
  `panic = "unwind"` se mantiene para que un fallo en una solicitud no detenga el servidor.
- SPA: destrucción de gráficas al salir del tablero y al cerrar sesión, limpieza del diálogo de
  detalle al cerrarse, liberación de vistas al cerrar sesión y descarte de respuestas de detalle
  obsoletas (también evita que una respuesta lenta sobrescriba otro registro).

## Descartado

- mimalloc: más memoria en todas las fases (retenida 249 MB frente a 39 MB) y una dependencia en C
  adicional; sin beneficio que lo justifique.
- Reducir iteraciones de PBKDF2, `panic = "abort"` o eliminar límites: debilitarían la seguridad.

## Seguridad y verificación

- Mismos límites y mensajes: 50 MB por archivo, 10 archivos y 100 MB por solicitud, 1 MB por campo
  de texto, límites de cuerpo por ruta; misma validación de contenido.
- Salidas equivalentes: CSV y PDF idénticos byte a byte frente a la versión anterior; en XLSX solo
  difiere la fecha de creación (`docProps/core.xml`).
- Pruebas: 508 aprobadas, 0 fallidas (`cargo test --locked -- --include-ignored`), con las nuevas
  `tests/qa_memoria.rs` (ida y vuelta byte a byte, Content-Length, contenido inválido o disfrazado,
  límites, ausencia de temporales, limpieza al arrancar, partes vacías) y
  `tests/qa_memoria_export.rs`; prueba unitaria que compara el clasificador incremental con el
  original en todos los puntos de corte. `cargo fmt --check`, `cargo clippy -- -D warnings` y
  `node --check` limpios.
- Revisión independiente (Qwen 3.8 Max) del diff: sin vulnerabilidades altas ni críticas; se
  aplicaron sus hallazgos válidos (espera acotada, turno de importación tras leer el cuerpo,
  temporales solo para partes no vacías, limpieza al arrancar) y se rechazaron dos afirmaciones
  incorrectas (celdas nulas del CSV y alcance de `content_class`).
