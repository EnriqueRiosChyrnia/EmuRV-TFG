// Navegador del currículo (RP15): las 10 lecciones de lecciones/ embebidas
// con ?raw (mismo patrón que los ejemplos). Los metadatos NO se duplican a
// mano: se parsean de la cabecera (parser.ts) — fuente única, anti-drift.
import l01 from "../../lecciones/01-registros/leccion.s?raw";
import l02 from "../../lecciones/02-aritmetica/leccion.s?raw";
import l03 from "../../lecciones/03-memoria/leccion.s?raw";
import l04 from "../../lecciones/04-condicionales/leccion.s?raw";
import l05 from "../../lecciones/05-bucles/leccion.s?raw";
import l06 from "../../lecciones/06-funciones/leccion.s?raw";
import l07 from "../../lecciones/07-syscalls/leccion.s?raw";
// La 08 es multi-archivo: main.s + sus dos includes (necesita disco para
// el .include — se resuelve al abrirla, ver RP-5 / caso multiarchivo).
import l08main from "../../lecciones/08-multiarchivo/main.s?raw";
import l08utiles from "../../lecciones/08-multiarchivo/utiles.s?raw";
import l08constantes from "../../lecciones/08-multiarchivo/constantes.s?raw";
import l09 from "../../lecciones/09-mmio/leccion.s?raw";
import l10 from "../../lecciones/10-traps/leccion.s?raw";
import { parseLeccion, type MetaLeccion } from "./parser";

export type { MetaLeccion } from "./parser";
export { parseLeccion } from "./parser";

/// Una lección lista para el navegador: metadatos + el/los archivo(s).
export interface Leccion extends MetaLeccion {
  /// Carpeta canónica (la de lecciones/), para nombrar al guardar.
  carpeta: string;
  /// Archivo principal a cargar en el editor.
  codigo: string;
  /// true si necesita disco (.include): la 08.
  multiarchivo: boolean;
  /// Para el caso multi-archivo: nombre→contenido de TODOS sus archivos.
  archivos?: Record<string, string>;
}

function armar(
  carpeta: string,
  texto: string,
  extra?: { archivos: Record<string, string> },
): Leccion {
  const meta = parseLeccion(texto);
  return {
    ...meta,
    carpeta,
    codigo: texto,
    multiarchivo: extra !== undefined,
    archivos: extra?.archivos,
  };
}

/// El currículo ordenado (orden de aprendizaje, no alfabético).
export const LECCIONES: Leccion[] = [
  armar("01-registros", l01),
  armar("02-aritmetica", l02),
  armar("03-memoria", l03),
  armar("04-condicionales", l04),
  armar("05-bucles", l05),
  armar("06-funciones", l06),
  armar("07-syscalls", l07),
  armar("08-multiarchivo", l08main, {
    archivos: { "main.s": l08main, "utiles.s": l08utiles, "constantes.s": l08constantes },
  }),
  armar("09-mmio", l09),
  armar("10-traps", l10),
];
