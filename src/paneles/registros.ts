// Vista de registros (RF-05): x0–x31 + pc con nombres ABI, valores en
// hex y decimal, y resaltado de los registros modificados desde la
// última actualización. Doble click en un valor lo edita en vivo (RP7).
// Al pie, una sección plegable muestra los CSRs de máquina (RP8-F4).
import { api, type MachineState } from "../api";
import { t } from "../i18n";
import { ABI, GRUPOS_ABI, claveRol } from "./registros-meta";

// Colgroups (RP18-F2): anchos de columna fijos para que la tabla no salte.
// Enteros/CSRs comparten nombre/hex/dec; la F da más ancho al «valor».
const COLGRUPO = `<colgroup><col class="registros-col-nombre"><col class="registros-col-hex"><col class="registros-col-dec"></colgroup>`;
const COLGRUPO_F = `<colgroup><col class="registros-col-fnombre"><col class="registros-col-fhex"><col class="registros-col-fval"></colgroup>`;

/** CSRs que expone get_csrs(), en el orden fijo del backend (RP8). */
const CSRS: string[] = [
  "mstatus", "mie", "mtvec", "mscratch", "mepc", "mcause", "mtval", "mip",
];

/** Nombres ABI de los registros f (RP12), f0..f31. */
const FABI: string[] = [
  "ft0", "ft1", "ft2", "ft3", "ft4", "ft5", "ft6", "ft7", "fs0", "fs1",
  "fa0", "fa1", "fa2", "fa3", "fa4", "fa5", "fa6", "fa7",
  "fs2", "fs3", "fs4", "fs5", "fs6", "fs7", "fs8", "fs9", "fs10", "fs11",
  "ft8", "ft9", "ft10", "ft11",
];

/** Interpreta los 64 bits de un registro f (hex de 16 dígitos).
 *  Regla del spike RP12-F1: alto == 0xFFFFFFFF (NaN-boxing de la espec) O
 *  alto == 0 con bajo ≠ 0 (Unicorn no boxea el RESULTADO de las ops .s —
 *  quirk medido) → single; si no → double. Un f64 real con alto == 0 es
 *  un denormal < 1e-308: no aparece en uso didáctico. */
function interpretarF(hex16: string): { texto: string; esF32: boolean } {
  const hi = Number.parseInt(hex16.slice(0, 8), 16) >>> 0;
  const lo = Number.parseInt(hex16.slice(8), 16) >>> 0;
  const dv = new DataView(new ArrayBuffer(8));
  if (hi === 0xffff_ffff || (hi === 0 && lo !== 0)) {
    dv.setUint32(0, lo);
    return { texto: `${dv.getFloat32(0)} (f32)`, esF32: true };
  }
  if (hi === 0 && lo === 0) return { texto: "0 (f64)", esF32: false };
  dv.setUint32(0, hi);
  dv.setUint32(4, lo);
  return { texto: `${dv.getFloat64(0)} (f64)`, esF32: false };
}

/** Texto del usuario → 16 dígitos hex para write_reg_f: «0x…» son bits
 *  crudos; un decimal con punto se codifica según la interpretación
 *  ACTUAL del registro (f32 → NaN-boxed, f64 → bits de double). */
function bitsDesdeTexto(texto: string, esF32: boolean): string | null {
  const v = texto.trim().replace(/ \(f(32|64)\)$/, "");
  if (/^0x[0-9a-f]{1,16}$/i.test(v)) return v.slice(2).padStart(16, "0").toLowerCase();
  const n = Number(v);
  if (Number.isNaN(n) && v !== "NaN") return null;
  const dv = new DataView(new ArrayBuffer(8));
  if (esF32) {
    dv.setFloat32(0, n);
    const lo = dv.getUint32(0).toString(16).padStart(8, "0");
    return `ffffffff${lo}`;
  }
  dv.setFloat64(0, n);
  const hi = dv.getUint32(0).toString(16).padStart(8, "0");
  const lo = dv.getUint32(4).toString(16).padStart(8, "0");
  return `${hi}${lo}`;
}

const hex = (v: number) => `0x${(v >>> 0).toString(16).padStart(8, "0")}`;
/** Valor decimal con signo (los registros se muestran como i32). */
const dec = (v: number) => String(v | 0);

/** Acepta decimal (42, -7) o hexadecimal (0x2a, -0x10); null si no parsea
 *  o se sale del rango de 32 bits. */
function parsearValor(texto: string): number | null {
  const v = texto.trim();
  let n: number;
  if (/^-?\d+$/.test(v)) n = Number.parseInt(v, 10);
  else if (/^-?0x[0-9a-f]+$/i.test(v)) n = Number.parseInt(v, 16);
  else return null;
  if (!Number.isSafeInteger(n) || n < -0x8000_0000 || n > 0xffff_ffff) return null;
  return n;
}

export class Registros {
  private celdas: { hex: HTMLElement; dec: HTMLElement; fila: HTMLElement }[] = [];
  private pcHex: HTMLElement;
  private pcDec: HTMLElement;
  private anterior: number[] | null = null;
  private pcAnterior: number | null = null;
  private errorEl: HTMLElement;
  private csrDetalles: HTMLDetailsElement;
  private csrCeldas: { hex: HTMLElement; dec: HTMLElement }[] = [];
  private fDetalles: HTMLDetailsElement;
  private fCeldas: { hex: HTMLElement; valor: HTMLElement }[] = [];
  private fcsrCelda: { hex: HTMLElement; dec: HTMLElement } | null = null;

  constructor(root: HTMLElement) {
    const tabla = document.createElement("table");
    tabla.className = "registros-tabla";
    tabla.title = t("registros.editarHint");
    tabla.innerHTML = `${COLGRUPO}<thead><tr>
        <th>${t("registros.nombre")}</th>
        <th>${t("registros.hex")}</th>
        <th>${t("registros.dec")}</th>
      </tr></thead>`;
    const cuerpo = document.createElement("tbody");

    const filaPc = document.createElement("tr");
    filaPc.className = "registros-pc";
    filaPc.innerHTML = `<td>pc</td><td class="mono"></td><td class="mono"></td>`;
    this.pcHex = filaPc.children[1] as HTMLElement;
    this.pcDec = filaPc.children[2] as HTMLElement;
    this.hacerEditable(this.pcHex, "pc");
    this.hacerEditable(this.pcDec, "pc");
    cuerpo.appendChild(filaPc);

    for (let i = 0; i < 32; i++) {
      const tr = document.createElement("tr");
      // Separador de grupo ABI (RP18-F2): solo cosmético, el orden no cambia.
      if (GRUPOS_ABI.has(i)) tr.classList.add("grupo");
      tr.innerHTML = `<td>${ABI[i]} <span class="registros-xn">x${i}</span></td>
        <td class="mono"></td><td class="mono"></td>`;
      // Tooltip con el rol del registro en la celda del nombre (RP18-F2).
      (tr.children[0] as HTMLElement).title = t(claveRol(ABI[i]));
      cuerpo.appendChild(tr);
      const hexEl = tr.children[1] as HTMLElement;
      const decEl = tr.children[2] as HTMLElement;
      // x0 también se cablea: el rechazo didáctico del backend lo explica.
      this.hacerEditable(hexEl, ABI[i]);
      this.hacerEditable(decEl, ABI[i]);
      this.celdas.push({ fila: tr, hex: hexEl, dec: decEl });
    }
    tabla.appendChild(cuerpo);
    root.appendChild(tabla);

    // Sección plegable de CSRs (RP8-F4), cerrada por defecto: recién al
    // abrirla se piden valores (cerrada no genera tráfico con el backend).
    this.csrDetalles = document.createElement("details");
    this.csrDetalles.className = "registros-csrs";
    const resumen = document.createElement("summary");
    resumen.textContent = t("registros.csrs.titulo");
    this.csrDetalles.appendChild(resumen);
    const tablaCsr = document.createElement("table");
    tablaCsr.className = "registros-tabla";
    tablaCsr.title = t("registros.editarHint");
    tablaCsr.innerHTML = COLGRUPO;
    const cuerpoCsr = document.createElement("tbody");
    CSRS.forEach((nombre, i) => {
      const tr = document.createElement("tr");
      tr.innerHTML = `<td>${nombre}</td><td class="mono"></td><td class="mono"></td>`;
      const hexEl = tr.children[1] as HTMLElement;
      const decEl = tr.children[2] as HTMLElement;
      this.hacerEditableCsr(hexEl, i);
      this.hacerEditableCsr(decEl, i);
      cuerpoCsr.appendChild(tr);
      this.csrCeldas.push({ hex: hexEl, dec: decEl });
    });
    tablaCsr.appendChild(cuerpoCsr);
    this.csrDetalles.appendChild(tablaCsr);
    this.csrDetalles.addEventListener("toggle", () => {
      if (this.csrDetalles.open) void this.refrescarCsrs();
    });
    root.appendChild(this.csrDetalles);

    // Sección plegable de registros F (RP12), mismo patrón que los CSRs:
    // cerrada por defecto = cero invokes durante una ejecución larga.
    this.fDetalles = document.createElement("details");
    this.fDetalles.className = "registros-csrs";
    const resumenF = document.createElement("summary");
    resumenF.textContent = t("registros.fregs.titulo");
    this.fDetalles.appendChild(resumenF);
    const tablaF = document.createElement("table");
    tablaF.className = "registros-tabla";
    tablaF.title = t("registros.editarHint");
    tablaF.innerHTML = `${COLGRUPO_F}<thead><tr>
        <th>${t("registros.nombre")}</th>
        <th>${t("registros.hex")}</th>
        <th>${t("registros.valor")}</th>
      </tr></thead>`;
    const cuerpoF = document.createElement("tbody");
    FABI.forEach((nombre, i) => {
      const tr = document.createElement("tr");
      tr.innerHTML = `<td>${nombre} <span class="registros-xn">f${i}</span></td>
        <td class="mono"></td><td class="mono"></td>`;
      const hexEl = tr.children[1] as HTMLElement;
      const valEl = tr.children[2] as HTMLElement;
      this.hacerEditableF(hexEl, i);
      this.hacerEditableF(valEl, i);
      cuerpoF.appendChild(tr);
      this.fCeldas.push({ hex: hexEl, valor: valEl });
    });
    // fcsr al pie (solo lectura: flags acumulados + modo de redondeo).
    const trFcsr = document.createElement("tr");
    trFcsr.innerHTML = `<td>fcsr</td><td class="mono"></td><td class="mono"></td>`;
    this.fcsrCelda = {
      hex: trFcsr.children[1] as HTMLElement,
      dec: trFcsr.children[2] as HTMLElement,
    };
    cuerpoF.appendChild(trFcsr);
    tablaF.appendChild(cuerpoF);
    this.fDetalles.appendChild(tablaF);
    this.fDetalles.addEventListener("toggle", () => {
      if (this.fDetalles.open) void this.refrescarFregs();
    });
    root.appendChild(this.fDetalles);

    this.errorEl = document.createElement("div");
    this.errorEl.className = "panel-error";
    this.errorEl.hidden = true;
    root.appendChild(this.errorEl);
    this.limpiar();
  }

  /** Refresca valores y marca los registros que cambiaron (RF-05).
   *  Una celda con edición inline en curso no se pisa (el refresco
   *  periódico durante una ejecución larga destruiría el input). */
  actualizar(estado: MachineState): void {
    const poner = (el: HTMLElement, texto: string) => {
      if (!el.querySelector("input")) el.textContent = texto;
    };
    const regs = estado.registers;
    for (let i = 0; i < 32; i++) {
      const c = this.celdas[i];
      poner(c.hex, hex(regs[i]));
      poner(c.dec, dec(regs[i]));
      const cambio = this.anterior !== null && this.anterior[i] !== regs[i];
      c.fila.classList.toggle("cambiado", cambio);
    }
    poner(this.pcHex, hex(estado.pc));
    poner(this.pcDec, dec(estado.pc));
    this.pcHex.parentElement!.classList.toggle(
      "cambiado",
      this.pcAnterior !== null && this.pcAnterior !== estado.pc,
    );
    this.anterior = [...regs];
    this.pcAnterior = estado.pc;
    void this.refrescarCsrs();
    void this.refrescarFregs();
  }

  /** Estado inicial sin resaltados (tras ensamblar/reiniciar). */
  reiniciar(estado: MachineState): void {
    this.anterior = null;
    this.pcAnterior = null;
    this.actualizar(estado);
  }

  private limpiar(): void {
    this.pcHex.textContent = "—";
    this.pcDec.textContent = "—";
    for (const c of this.celdas) {
      c.hex.textContent = "—";
      c.dec.textContent = "—";
    }
    for (const c of this.csrCeldas) {
      c.hex.textContent = "—";
      c.dec.textContent = "—";
    }
    for (const c of this.fCeldas) {
      c.hex.textContent = "—";
      c.valor.textContent = "—";
    }
    if (this.fcsrCelda) {
      this.fcsrCelda.hex.textContent = "—";
      this.fcsrCelda.dec.textContent = "—";
    }
  }

  // ----- CSRs (RP8-F4) -----

  /** Vuelca los CSRs en la sección plegable. Solo si está abierta (cerrada
   *  = cero tráfico extra, importante con actualizar() a ~10 Hz durante una
   *  ejecución larga) y sin pisar una edición inline en curso (mismo guard
   *  que RP7). Sin sesión de emulación el rechazo se ignora en silencio. */
  private async refrescarCsrs(): Promise<void> {
    if (!this.csrDetalles.open) return;
    if (this.csrDetalles.querySelector("input")) return;
    let valores: number[];
    try {
      valores = await api.getCsrs();
    } catch {
      return; // sin sesión: la tabla conserva los «—»
    }
    for (let i = 0; i < this.csrCeldas.length && i < valores.length; i++) {
      const c = this.csrCeldas[i];
      // Un input abierto mientras esperábamos la respuesta no se pisa.
      if (!c.hex.querySelector("input")) c.hex.textContent = hex(valores[i]);
      if (!c.dec.querySelector("input")) c.dec.textContent = dec(valores[i]);
    }
  }

  // ----- registros F (RP12) -----

  /** Vuelca los registros f en su sección plegable (mismas reglas que
   *  refrescarCsrs: solo abierta, sin pisar ediciones en curso). */
  private async refrescarFregs(): Promise<void> {
    if (!this.fDetalles.open) return;
    if (this.fDetalles.querySelector("input")) return;
    let regs: string[];
    let fcsr: number;
    try {
      [regs, fcsr] = await api.getFregs();
    } catch {
      return; // sin sesión: la tabla conserva los «—»
    }
    for (let i = 0; i < this.fCeldas.length && i < regs.length; i++) {
      const c = this.fCeldas[i];
      if (!c.hex.querySelector("input")) c.hex.textContent = `0x${regs[i]}`;
      if (!c.valor.querySelector("input")) c.valor.textContent = interpretarF(regs[i]).texto;
    }
    if (this.fcsrCelda) {
      this.fcsrCelda.hex.textContent = hex(fcsr);
      this.fcsrCelda.dec.textContent = dec(fcsr);
    }
  }

  /** Edición de un registro f: «0x…» = bits crudos; decimal con punto se
   *  codifica según la interpretación ACTUAL (f32 NaN-boxed o f64). */
  private hacerEditableF(celda: HTMLElement, indice: number): void {
    celda.addEventListener("dblclick", () => {
      if (celda.querySelector("input")) return;
      const original = celda.textContent ?? "";
      if (original === "—") return;
      const hexActual = this.fCeldas[indice].hex.textContent ?? "";
      const esF32 = /^0x[0-9a-f]{16}$/i.test(hexActual)
        ? interpretarF(hexActual.slice(2)).esF32
        : false;
      const input = document.createElement("input");
      input.type = "text";
      input.className = "edicion-inline";
      input.value = original;
      input.spellcheck = false;
      celda.textContent = "";
      celda.appendChild(input);
      input.focus();
      input.select();
      let cerrado = false;
      const cerrar = () => {
        if (cerrado) return;
        cerrado = true;
        celda.textContent = original;
      };
      input.addEventListener("keydown", (e) => {
        if (e.key === "Escape") {
          cerrar();
          return;
        }
        if (e.key !== "Enter") return;
        const bits = bitsDesdeTexto(input.value, esF32);
        if (bits === null) {
          this.mostrarError(t("edicion.valorFInvalido"));
          return;
        }
        void (async () => {
          try {
            await api.writeRegF(indice, bits);
            cerrar();
            this.errorEl.hidden = true;
            await this.refrescarFregs();
          } catch (err) {
            cerrar();
            this.mostrarError(String(err));
          }
        })();
      });
      input.addEventListener("blur", () => cerrar());
    });
  }

  // ----- edición en vivo (RP7; los CSRs de RP8 reusan el mismo flujo) -----

  private hacerEditable(celda: HTMLElement, reg: string): void {
    celda.addEventListener("dblclick", () =>
      this.editar(
        celda,
        (val) => api.writeReg(reg, val),
        async () => this.actualizar(await api.getState()),
      ),
    );
  }

  private hacerEditableCsr(celda: HTMLElement, indice: number): void {
    celda.addEventListener("dblclick", () =>
      this.editar(
        celda,
        (val) => api.writeCsr(indice, val),
        () => this.refrescarCsrs(),
      ),
    );
  }

  /** Reemplaza la celda por un input inline prellenado: Enter aplica,
   *  Esc o blur cancelan. El valor mostrado solo cambia vía `refrescar`
   *  tras una escritura aceptada. */
  private editar(
    celda: HTMLElement,
    escribir: (val: number) => Promise<void>,
    refrescar: () => Promise<void>,
  ): void {
    if (celda.querySelector("input")) return;
    const original = celda.textContent ?? "";
    if (original === "—") return; // sin máquina todavía: nada que editar
    const input = document.createElement("input");
    input.type = "text";
    input.className = "edicion-inline";
    input.value = original;
    input.spellcheck = false;
    celda.textContent = "";
    celda.appendChild(input);
    input.focus();
    input.select();
    let cerrado = false;
    const cerrar = () => {
      if (cerrado) return;
      cerrado = true;
      celda.textContent = original;
    };
    input.addEventListener("keydown", (e) => {
      if (e.key === "Escape") {
        cerrar();
        return;
      }
      if (e.key !== "Enter") return;
      const val = parsearValor(input.value);
      if (val === null) {
        // Valor ilegible: se avisa y el input queda para corregirlo.
        this.mostrarError(t("edicion.valorInvalido"));
        return;
      }
      void this.aplicar(val, cerrar, escribir, refrescar);
    });
    input.addEventListener("blur", () => cerrar());
  }

  private async aplicar(
    val: number,
    cerrar: () => void,
    escribir: (val: number) => Promise<void>,
    refrescar: () => Promise<void>,
  ): Promise<void> {
    try {
      await escribir(val);
      cerrar();
      this.errorEl.hidden = true;
      // Refresco por el flujo normal: el resaltado de cambios (RF-05)
      // marca el registro recién editado.
      await refrescar();
    } catch (e) {
      cerrar(); // rechazo (corriendo, x0, …): el valor no cambia
      this.mostrarError(String(e));
    }
  }

  private mostrarError(mensaje: string): void {
    this.errorEl.textContent = mensaje;
    this.errorEl.hidden = false;
  }
}
