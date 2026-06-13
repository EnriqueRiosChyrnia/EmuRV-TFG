// Autocompletado contextual del editor (RP14-F2). La lógica vive en una
// función PURA (`opcionesAutocompletado`) testeable sin DOM; el adapter de
// CodeMirror (`fuenteAutocompletado`) la envuelve. RP-14 NO re-ensambla
// mientras tipeás: el autocompletado solo lee el buffer.
import type { CompletionContext, CompletionResult } from "@codemirror/autocomplete";
import { MNEMONICS, PSEUDOS, REGISTERS_F, REGISTERS_X } from "./isa";

/// Categoría de una sugerencia (define el ícono/orden en CM y el color).
export type TipoOpcion = "instruccion" | "pseudo" | "registro" | "etiqueta";

export interface Opcion {
  texto: string;
  tipo: TipoOpcion;
}

export interface ResultadoAutocompletado {
  /// Offset (relativo al comienzo de `lineaAntes`) donde empieza el token
  /// que se está completando — lo que CM debe reemplazar.
  desde: number;
  opciones: Opcion[];
}

/// Mnemónicos que admiten una etiqueta como operando (saltos, ramas,
/// carga de dirección). Tras uno de estos, además de registros, se ofrecen
/// las etiquetas del buffer.
const OPS_CON_ETIQUETA = new Set([
  "j", "call", "tail", "la", "lla", "jal",
  "beq", "bne", "blt", "bge", "bltu", "bgeu",
  "beqz", "bnez", "blez", "bgez", "bltz", "bgtz",
  "bgt", "ble", "bgtu", "bleu",
  "c.j", "c.beqz", "c.bnez",
]);

/// Extrae las etiquetas definidas en el documento (`etiqueta:` al inicio
/// de línea, tolerando indentación). Fuente local: funciona ANTES de
/// ensamblar y sin viaje al backend.
export function etiquetasDelBuffer(doc: string): string[] {
  const re = /^[ \t]*([A-Za-z_$][\w$]*)[ \t]*:/gm;
  const out = new Set<string>();
  let m: RegExpExecArray | null;
  while ((m = re.exec(doc)) !== null) out.add(m[1]);
  return [...out];
}

/// Núcleo PURO: dado el texto de la línea hasta el cursor y el documento
/// completo, decide qué ofrecer. Devuelve null si no hay nada que sugerir
/// (p. ej. se está escribiendo un número inmediato).
export function opcionesAutocompletado(
  lineaAntes: string,
  doc: string,
): ResultadoAutocompletado | null {
  // Token actual: la corrida de caracteres de identificador pegada al
  // cursor (incluye el punto de fadd.s / c.addi).
  const tokenActual = /[A-Za-z_$.][\w.$]*$/.exec(lineaAntes)?.[0] ?? "";
  const desde = lineaAntes.length - tokenActual.length;
  const prefijo = tokenActual.toLowerCase();

  // Un inmediato (empieza con dígito o signo) no se autocompleta.
  if (/[-+0-9]$/.test(lineaAntes) && tokenActual === "") return null;

  // ¿Estamos en el PRIMER token de la línea? (solo espacios antes del token)
  const antesDelToken = lineaAntes.slice(0, desde);
  const esPrimerToken = /^[ \t]*$/.test(antesDelToken);

  if (esPrimerToken) {
    const opciones: Opcion[] = [
      ...MNEMONICS.map((t): Opcion => ({ texto: t, tipo: "instruccion" })),
      ...PSEUDOS.map((t): Opcion => ({ texto: t, tipo: "pseudo" })),
    ];
    return { desde, opciones: filtrar(opciones, prefijo) };
  }

  // Hay un mnemónico al inicio: lo identificamos para decidir operandos.
  const mnem = /^[ \t]*([A-Za-z_$.][\w.$]*)/.exec(lineaAntes)?.[1]?.toLowerCase() ?? "";
  const opciones: Opcion[] = [
    ...REGISTERS_X.map((t): Opcion => ({ texto: t, tipo: "registro" })),
    ...REGISTERS_F.map((t): Opcion => ({ texto: t, tipo: "registro" })),
  ];
  if (OPS_CON_ETIQUETA.has(mnem)) {
    for (const e of etiquetasDelBuffer(doc)) {
      opciones.push({ texto: e, tipo: "etiqueta" });
    }
  }
  return { desde, opciones: filtrar(opciones, prefijo) };
}

/// Filtra por prefijo (case-insensitive) y conserva el orden de origen.
function filtrar(opciones: Opcion[], prefijo: string): Opcion[] {
  if (prefijo === "") return opciones;
  return opciones.filter((o) => o.texto.toLowerCase().startsWith(prefijo));
}

/// Adapter de CodeMirror: traduce el CompletionContext al núcleo puro.
/// `info` (descripción larga) se enriquece con el diccionario de hover en
/// la fase 3; acá `detail` lleva la categoría.
const DETALLE: Record<TipoOpcion, string> = {
  instruccion: "instrucción",
  pseudo: "pseudo",
  registro: "registro",
  etiqueta: "etiqueta",
};

export function fuenteAutocompletado(
  ctx: CompletionContext,
  info?: (mnem: string) => string | undefined,
): CompletionResult | null {
  const linea = ctx.state.doc.lineAt(ctx.pos);
  const lineaAntes = ctx.state.sliceDoc(linea.from, ctx.pos);
  const doc = ctx.state.doc.toString();
  const res = opcionesAutocompletado(lineaAntes, doc);
  if (!res || res.opciones.length === 0) {
    return null;
  }
  const from = linea.from + res.desde;
  // Disparo automático solo si hay un token en curso; con Ctrl-Espacio
  // (ctx.explicit) se ofrece aunque el token esté vacío.
  if (from === ctx.pos && !ctx.explicit) return null;
  return {
    from,
    options: res.opciones.map((o) => ({
      label: o.texto,
      type:
        o.tipo === "instruccion" || o.tipo === "pseudo"
          ? "keyword"
          : o.tipo === "registro"
            ? "variable"
            : "constant",
      detail: DETALLE[o.tipo],
      info:
        (o.tipo === "instruccion" || o.tipo === "pseudo") && info
          ? info(o.texto)
          : undefined,
    })),
  };
}
