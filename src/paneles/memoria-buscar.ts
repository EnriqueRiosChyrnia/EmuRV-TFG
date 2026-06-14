// Lógica PURA de la búsqueda en el panel de Memoria (RP26): parsear el
// texto buscado a una secuencia de bytes y localizarla en memoria leyendo
// por bloques. Sin DOM ni dependencias de Tauri, para poder testear los
// bordes (no encontrado, patrón partido entre bloques) con node:test.

/** Tipo de valor que se busca: byte (1 hex), word (32 bits, little-endian
 *  como se almacena en RV32) o texto ASCII. */
export type TipoBusqueda = "byte" | "word" | "ascii";

/** Lector de memoria: bytes en [addr, addr+len) o lanza si la zona no está
 *  mapeada (mismo contrato que `api.readMem`). Inyectable para tests. */
export type LectorMem = (addr: number, len: number) => Promise<number[]>;

/** Trozo del escaneo: leer por bloques (no de una) evita congelar la UI y
 *  acota cada `readMem` a un payload razonable. */
export const BLOQUE = 0x4000; // 16 KiB

/** Convierte el texto de búsqueda a la secuencia de bytes a localizar.
 *  - byte: hex de 1 byte (0..0xFF), con o sin prefijo `0x` (p. ej. `2A`).
 *  - word: hex de 32 bits → 4 bytes LITTLE-ENDIAN (p. ej. `0000002A` →
 *    `2A 00 00 00`, tal como queda en memoria).
 *  - ascii: cada carácter → su byte (solo 0..0xFF; p. ej. `Hola`).
 *  Devuelve null si el texto no es válido para el tipo. */
export function parsearPatron(texto: string, tipo: TipoBusqueda): number[] | null {
  const v = texto.trim();
  if (v === "") return null;
  if (tipo === "ascii") {
    const bytes: number[] = [];
    for (const ch of v) {
      const c = ch.codePointAt(0)!;
      if (c > 0xff) return null; // fuera de un solo byte (no ASCII/latin-1)
      bytes.push(c);
    }
    return bytes;
  }
  // byte / word: hexadecimal, con o sin prefijo 0x
  const hex = v.replace(/^0x/i, "");
  if (!/^[0-9a-f]+$/i.test(hex)) return null;
  const n = Number.parseInt(hex, 16);
  if (tipo === "byte") {
    if (n > 0xff) return null;
    return [n & 0xff];
  }
  if (n > 0xffff_ffff) return null;
  const u = n >>> 0;
  return [u & 0xff, (u >>> 8) & 0xff, (u >>> 16) & 0xff, (u >>> 24) & 0xff];
}

/** Primera aparición de `patron` en `bytes` a partir del índice `desde`.
 *  Devuelve el offset o -1 (patrón vacío o más largo que el buffer → -1). */
export function buscarPatron(bytes: number[], patron: number[], desde = 0): number {
  if (patron.length === 0 || patron.length > bytes.length) return -1;
  const ultimo = bytes.length - patron.length;
  for (let i = Math.max(0, desde); i <= ultimo; i++) {
    let ok = true;
    for (let j = 0; j < patron.length; j++) {
      if (bytes[i + j] !== patron[j]) {
        ok = false;
        break;
      }
    }
    if (ok) return i;
  }
  return -1;
}

/** Escaneo HACIA ADELANTE por bloques en [inicio, fin) desde la dirección
 *  `ancla`. Cada bloque solapa (patron.length-1) bytes con el siguiente para
 *  no perder una coincidencia partida entre dos lecturas. Devuelve la
 *  dirección absoluta de la 1ª coincidencia o null. Un bloque que no se puede
 *  leer (zona no mapeada) corta el escaneo (fin del rango útil). */
export async function buscarAdelante(
  leer: LectorMem,
  inicio: number,
  fin: number,
  patron: number[],
  ancla: number,
  tam = BLOQUE,
): Promise<number | null> {
  if (patron.length === 0 || inicio >= fin) return null;
  const paso = Math.max(1, tam - (patron.length - 1));
  let addr = Math.max(inicio, ancla);
  while (addr < fin) {
    const len = Math.min(tam, fin - addr);
    if (len < patron.length) break; // lo que queda no alcanza para el patrón
    let bytes: number[];
    try {
      bytes = await leer(addr, len);
    } catch {
      return null; // zona no mapeada → fin del rango útil
    }
    const off = buscarPatron(bytes, patron, 0);
    if (off >= 0) return (addr + off) >>> 0;
    if (addr + len >= fin) break; // ya cubrimos hasta fin
    addr += paso;
  }
  return null;
}

/** Escaneo HACIA ATRÁS en [inicio, fin): última coincidencia cuya dirección
 *  sea estrictamente menor que `ancla`. Reusa `buscarAdelante` (misma lógica
 *  de bloques/solape) avanzando hasta pasarse de `ancla`. Devuelve la
 *  dirección absoluta o null. */
export async function buscarAtras(
  leer: LectorMem,
  inicio: number,
  fin: number,
  patron: number[],
  ancla: number,
  tam = BLOQUE,
): Promise<number | null> {
  if (patron.length === 0) return null;
  let ultima: number | null = null;
  let actual = inicio;
  while (actual < ancla) {
    const m = await buscarAdelante(leer, inicio, fin, patron, actual, tam);
    if (m === null || m >= ancla) break;
    ultima = m;
    actual = m + 1;
  }
  return ultima;
}
