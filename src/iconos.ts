// Iconos SVG inline para la toolbar (RP17-F2). Tomados de Lucide
// (https://lucide.dev), licencia ISC — copiados como path data, CERO
// dependencia npm. Son decorativos: el botón conserva su texto (que da la
// etiqueta accesible), así que el <svg> va con aria-hidden.

/// Path(s) de cada icono (viewBox 0 0 24 24, trazo, sin relleno).
const PATHS: Record<string, string> = {
  // package — Ensamblar (construir el binario)
  ensamblar:
    '<path d="M11 21.73a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73z"/><path d="M12 22V12"/><path d="m3.3 7 8.7 5 8.7-5"/><path d="m7.5 4.27 9 5.15"/>',
  // play — Ejecutar
  ejecutar: '<polygon points="6 3 20 12 6 21 6 3"/>',
  // chevron-right — Paso
  paso: '<path d="m9 18 6-6-6-6"/>',
  // square — Detener
  detener: '<rect width="18" height="18" x="3" y="3" rx="2"/>',
  // rotate-ccw — Reiniciar
  reiniciar: '<path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/>',
  // graduation-cap — Lecciones
  lecciones:
    '<path d="M21.42 10.922a1 1 0 0 0-.019-1.838L12.83 5.18a2 2 0 0 0-1.66 0L2.6 9.08a1 1 0 0 0 0 1.832l8.57 3.908a2 2 0 0 0 1.66 0z"/><path d="M22 10v6"/><path d="M6 12.5V16a6 3 0 0 0 12 0v-3.5"/>',
  // monitor — Pantalla
  video:
    '<rect width="20" height="14" x="2" y="3" rx="2"/><line x1="8" x2="16" y1="21" y2="21"/><line x1="12" x2="12" y1="17" y2="21"/>',
  // file-plus — Nuevo (programa en blanco)
  nuevo:
    '<path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/><path d="M9 15h6"/><path d="M12 18v-6"/>',
  // folder-open — Abrir
  abrir:
    '<path d="m6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2"/>',
  // save — Guardar
  guardar:
    '<path d="M15.2 3a2 2 0 0 1 1.4.6l3.8 3.8a2 2 0 0 1 .6 1.4V19a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z"/><path d="M17 21v-7a1 1 0 0 0-1-1H8a1 1 0 0 0-1 1v7"/><path d="M7 3v4a1 1 0 0 0 1 1h7"/>',
  // save-all — Guardar como (variante de save)
  guardarComo:
    '<path d="M10 2v3a1 1 0 0 1-1 1H6"/><path d="M17 21v-7a1 1 0 0 0-1-1H8a1 1 0 0 0-1 1v7"/><path d="M4 6v13a2 2 0 0 0 2 2h11a2 2 0 0 0 2-2V7.5L16.5 4H6a2 2 0 0 0-2 2"/><path d="M7 8h7"/>',
  // download — Exportar ELF
  exportar:
    '<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" x2="12" y1="15" y2="3"/>',
  // circle-help — Ayuda
  ayuda:
    '<circle cx="12" cy="12" r="10"/><path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3"/><path d="M12 17h.01"/>',
};

/// Devuelve el SVG (string) de un icono, o "" si no existe (el botón queda
/// solo con texto, sin romper).
export function icono(nombre: string): string {
  const paths = PATHS[nombre];
  if (!paths) return "";
  // El play (Ejecutar) va relleno por convención (RP19): clase .ico-fill,
  // que en styles.css lo pinta con currentColor sin trazo.
  const clase = nombre === "ejecutar" ? "ico ico-fill" : "ico";
  return (
    `<svg class="${clase}" viewBox="0 0 24 24" fill="none" stroke="currentColor" ` +
    `stroke-width="2" stroke-linecap="round" stroke-linejoin="round" ` +
    `aria-hidden="true" focusable="false">${paths}</svg>`
  );
}
