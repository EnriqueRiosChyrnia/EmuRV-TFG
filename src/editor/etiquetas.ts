// Navegación de etiquetas en el buffer (RP14-F3): ir-a-definición y
// resaltar usos. Funciones PURAS sobre el texto — sin viaje al backend,
// funcionan ANTES de ensamblar. El adapter CM6 (en index.ts) las usa para
// el Cmd/Ctrl-click y la decoración.

/// Posición de la DEFINICIÓN de una etiqueta (`nombre:` al inicio de línea,
/// tolerando indentación), como offset en el documento. null si no existe.
export function definicionDeEtiqueta(doc: string, nombre: string): number | null {
  const re = new RegExp(`^[ \\t]*(${escaparRegex(nombre)})[ \\t]*:`, "m");
  const m = re.exec(doc);
  return m ? m.index + m[0].indexOf(nombre) : null;
}

/// Todos los rangos [from, to) donde aparece la etiqueta `nombre` como
/// palabra completa (definición y usos). Sirve para resaltar.
export function usosDeEtiqueta(doc: string, nombre: string): Array<{ from: number; to: number }> {
  const re = new RegExp(`\\b${escaparRegex(nombre)}\\b`, "g");
  const out: Array<{ from: number; to: number }> = [];
  let m: RegExpExecArray | null;
  while ((m = re.exec(doc)) !== null) {
    out.push({ from: m.index, to: m.index + nombre.length });
  }
  return out;
}

/// La palabra-identificador en la posición `pos` del documento (o null si
/// ahí no hay un identificador). Sirve para saber sobre qué etiqueta está
/// el cursor/click.
export function palabraEn(doc: string, pos: number): { texto: string; from: number; to: number } | null {
  if (pos < 0 || pos > doc.length) return null;
  const esId = (c: string) => /[A-Za-z0-9_$]/.test(c);
  let from = pos;
  while (from > 0 && esId(doc[from - 1])) from--;
  let to = pos;
  while (to < doc.length && esId(doc[to])) to++;
  if (from === to) return null;
  const texto = doc.slice(from, to);
  if (!/^[A-Za-z_$]/.test(texto)) return null;
  return { texto, from, to };
}

function escaparRegex(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
