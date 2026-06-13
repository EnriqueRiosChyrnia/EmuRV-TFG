// Parser de la cabecera de plantilla fija de las lecciones (RP15). PURO y
// SIN imports de Vite (`?raw`), para que corra bajo node:test/tsx además
// de en el bundle. El navegador (index.ts) lo usa sobre las lecciones
// embebidas; el test lo usa sobre los .s leídos del disco — misma fuente.

/// Metadatos extraídos de la cabecera de una lección.
export interface MetaLeccion {
  numero: number;
  titulo: string;
  objetivos: string[];
  paneles: string;
}

/// Quita el prefijo «# » (o «#») de una línea de comentario de la cabecera.
function descomentar(linea: string): string {
  return linea.replace(/^#\s?/, "");
}

/// Parsea la cabecera de plantilla fija de RP-9:
///   `# Lección NN — TÍTULO`
///   `# OBJETIVOS — …` seguido de bullets `#   * …` (con continuaciones)
///   `# PANELES PARA OBSERVAR: …` (posiblemente multilínea)
///   cerrado por `# CÓMO USARLA:` o el separador `# ----`.
/// Es la fuente única de los metadatos del navegador: si una lección no
/// cumple la plantilla (sin título u objetivos), el test anti-drift falla.
export function parseLeccion(texto: string): MetaLeccion {
  const lineas = texto.split("\n");
  let numero = 0;
  let titulo = "";
  const objetivos: string[] = [];
  let paneles = "";

  let modo: "ninguno" | "objetivos" | "paneles" = "ninguno";
  for (const cruda of lineas) {
    const linea = cruda.trimEnd();
    const mt = /^#\s*Lección\s+(\d+)\s*[—-]\s*(.+)$/.exec(linea);
    if (mt) {
      numero = Number.parseInt(mt[1], 10);
      titulo = mt[2].trim();
      continue;
    }
    if (/^#\s*OBJETIVOS\b/.test(linea)) {
      modo = "objetivos";
      continue;
    }
    const mp = /^#\s*PANELES PARA OBSERVAR:\s*(.*)$/.exec(linea);
    if (mp) {
      modo = "paneles";
      paneles = mp[1].trim();
      continue;
    }
    if (/^#\s*CÓMO USARLA:/.test(linea)) {
      modo = "ninguno";
      continue;
    }
    if (/^#\s*-{3,}/.test(linea)) {
      // El separador cierra la sección de paneles si seguía abierta.
      if (modo === "paneles") modo = "ninguno";
      continue;
    }
    if (modo === "objetivos") {
      const cuerpo = descomentar(linea);
      const m = /^\s*\*\s+(.+)$/.exec(cuerpo);
      if (m) {
        objetivos.push(m[1].trim());
      } else if (objetivos.length > 0 && cuerpo.trim() !== "") {
        objetivos[objetivos.length - 1] += " " + cuerpo.trim();
      }
    } else if (modo === "paneles") {
      const cuerpo = descomentar(linea).trim();
      if (cuerpo !== "") paneles += (paneles ? " " : "") + cuerpo;
    }
  }
  return { numero, titulo, objetivos, paneles };
}
