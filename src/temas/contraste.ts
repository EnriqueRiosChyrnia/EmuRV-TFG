// Contraste WCAG (RP28): lógica PURA de luminancia/ratio y los pares
// críticos, extraída de contraste.test.ts para que la comparta el AVISO en
// vivo del editor de tema de usuario (RP28) y el test de los 5 temas
// built-in. Fuente única: si cambia la fórmula o los pares, cambia para
// ambos. Sin acceso a archivos ni DOM → corre en el navegador y en node:test.

/// Luminancia relativa WCAG de un color `#rrggbb` (sRGB linealizado).
export function luminancia(hex: string): number {
  const c = hex.replace("#", "");
  const v = [0, 2, 4]
    .map((i) => parseInt(c.slice(i, i + 2), 16) / 255)
    .map((x) => (x <= 0.03928 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4));
  return 0.2126 * v[0] + 0.7152 * v[1] + 0.0722 * v[2];
}

/// Ratio de contraste WCAG entre dos colores (1:1 a 21:1).
export function ratio(a: string, b: string): number {
  const la = luminancia(a);
  const lb = luminancia(b);
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

/// Umbral WCAG AA para texto normal.
export const AA = 4.5;

// Pares críticos texto/fondo (nombres de token SIN el prefijo `--`). Mismo
// conjunto que verificaba contraste.test.ts para los temas built-in.
export const PARES: ReadonlyArray<readonly [string, string]> = [
  ["texto", "fondo"],
  ["texto", "fondo-panel"],
  ["texto", "fondo-claro"],
  ["texto-suave", "fondo-panel"],
  ["texto-suave", "fondo"],
  ["error", "fondo-panel"],
  ["error", "fondo"],
  ["acento", "fondo"],
  ["acento", "fondo-panel"],
  ["ok", "fondo"],
  ["cambio", "fondo-panel"],
];

// Sintaxis del editor: cada --sx-* es texto que CodeMirror pinta sobre el
// fondo del editor (= --fondo). La legibilidad del código es tan
// innegociable como la de la UI.
export const SINTAXIS: readonly string[] = [
  "sx-com",
  "sx-dir",
  "sx-lbl",
  "sx-ins",
  "sx-str",
  "sx-num",
];

/** Un par que no llega al umbral, con su ratio para el mensaje de aviso. */
export interface ParAviso {
  fg: string;
  bg: string;
  ratio: number;
}

/// Dado un mapa token→`#hex` (claves SIN `--`), devuelve los pares críticos
/// (UI + sintaxis sobre --fondo) cuyo contraste queda por debajo de `umbral`.
/// No bloquea: es el insumo del aviso «este color puede ser difícil de leer».
export function paresQueFallan(
  tk: Record<string, string>,
  umbral: number = AA,
): ParAviso[] {
  const fallas: ParAviso[] = [];
  const mira = (fg: string, bg: string): void => {
    const a = tk[fg];
    const b = tk[bg];
    if (!a || !b) return; // token ausente: no opinamos
    const r = ratio(a, b);
    if (r < umbral) fallas.push({ fg, bg, ratio: r });
  };
  for (const [fg, bg] of PARES) mira(fg, bg);
  for (const sx of SINTAXIS) mira(sx, "fondo");
  return fallas;
}
