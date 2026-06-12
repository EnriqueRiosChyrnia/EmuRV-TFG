// Panel de ayuda integrada (RF-07: tabla de syscalls documentada; RF-09:
// referencia de los ejemplos). Overlay modal sobre la app.
import { t } from "../i18n";

export class Ayuda {
  private overlay: HTMLElement;

  constructor() {
    this.overlay = document.createElement("div");
    this.overlay.id = "ayuda-overlay";
    this.overlay.hidden = true;
    // Semántica de diálogo modal para lectores de pantalla.
    this.overlay.setAttribute("role", "dialog");
    this.overlay.setAttribute("aria-modal", "true");
    this.overlay.innerHTML = `
      <div class="ayuda-caja">
        <header>
          <h2>${t("ayuda.titulo")}</h2>
          <button class="ayuda-cerrar" aria-label="${t("ayuda.cerrar")}">✕</button>
        </header>
        <div class="ayuda-cuerpo">
          <section>
            <h3>${t("ayuda.ciclo.titulo")}</h3>
            <p>${t("ayuda.ciclo.texto")}</p>
            <p>${t("ayuda.lecciones")}</p>
            <p>${t("ayuda.binario")}</p>
          </section>
          <section>
            <h3>${t("ayuda.syscalls.titulo")}</h3>
            <p>${t("ayuda.syscalls.texto")}</p>
            <table>
              <thead><tr><th>a7</th><th>${t("ayuda.syscalls.servicio")}</th><th>${t("ayuda.syscalls.args")}</th></tr></thead>
              <tbody>
                <tr><td>1</td><td>${t("ayuda.syscalls.s1")}</td><td>a0 = valor</td></tr>
                <tr><td>4</td><td>${t("ayuda.syscalls.s4")}</td><td>a0 = dirección (terminada en NUL)</td></tr>
                <tr><td>5</td><td>${t("ayuda.syscalls.s5")}</td><td>→ a0 = valor leído</td></tr>
                <tr><td>8</td><td>${t("ayuda.syscalls.s8")}</td><td>a0 = buffer, a1 = tamaño</td></tr>
                <tr><td>9</td><td>${t("ayuda.syscalls.s9")}</td><td>a0 = bytes → a0 = dirección</td></tr>
                <tr><td>10</td><td>${t("ayuda.syscalls.s10")}</td><td>—</td></tr>
                <tr><td>11</td><td>${t("ayuda.syscalls.s11")}</td><td>a0 = código de carácter</td></tr>
                <tr><td>30</td><td>${t("ayuda.syscalls.s30")}</td><td>→ a0 = ms (bits bajos), a1 = ms (bits altos)</td></tr>
                <tr><td>32</td><td>${t("ayuda.syscalls.s32")}</td><td>a0 = milisegundos</td></tr>
                <tr><td>60</td><td>${t("ayuda.syscalls.s60")}</td><td>—</td></tr>
                <tr><td>93</td><td>${t("ayuda.syscalls.s93")}</td><td>a0 = código de salida</td></tr>
              </tbody>
            </table>
            <pre class="ayuda-ejemplo">    li   a7, 4        # servicio: imprimir cadena
    la   a0, mensaje  # argumento
    ecall             # llamada al sistema</pre>
            <p>${t("ayuda.ebreak")}</p>
            <p>${t("ayuda.traps")}</p>
          </section>
          <section>
            <h3>${t("ayuda.memoria.titulo")}</h3>
            <table>
              <tbody>
                <tr><td class="mono">0x00400000</td><td>${t("ayuda.memoria.text")}</td></tr>
                <tr><td class="mono">0x10010000</td><td>${t("ayuda.memoria.data")}</td></tr>
                <tr><td class="mono">0x7FFFFFF0</td><td>${t("ayuda.memoria.sp")}</td></tr>
              </tbody>
            </table>
          </section>
          <section>
            <h3>${t("ayuda.teclado.titulo")}</h3>
            <p>${t("ayuda.teclado.texto")}</p>
            <table>
              <tbody>
                <tr><td class="mono">0xFFFF0000</td><td>${t("ayuda.teclado.control")}</td></tr>
                <tr><td class="mono">0xFFFF0004</td><td>${t("ayuda.teclado.dato")}</td></tr>
              </tbody>
            </table>
            <p>${t("ayuda.teclado.codigos")}</p>
            <table>
              <thead><tr><th>${t("ayuda.teclado.colCodigo")}</th><th>${t("ayuda.teclado.colTecla")}</th></tr></thead>
              <tbody>
                <tr><td class="mono">32–126</td><td>${t("ayuda.teclado.ascii")}</td></tr>
                <tr><td class="mono">10</td><td>Enter</td></tr>
                <tr><td class="mono">27</td><td>Escape</td></tr>
                <tr><td class="mono">8</td><td>Backspace</td></tr>
                <tr><td class="mono">0x80–0x83</td><td>${t("ayuda.teclado.flechas")}</td></tr>
              </tbody>
            </table>
            <pre class="ayuda-ejemplo">espera:
    li   t0, 0xFFFF0000   # RECEIVER_CONTROL del teclado
    lw   t1, 0(t0)        # bit 0 = hay tecla pendiente
    beqz t1, espera       # nada todavía: seguir esperando
    lw   a0, 4(t0)        # RECEIVER_DATA: leer consume la tecla</pre>
          </section>
          <section>
            <h3>${t("ayuda.directivas.titulo")}</h3>
            <p class="mono">.text .data .word .half .byte .ascii .asciiz .string .space .align .globl .include .eqv .equ .macro .end_macro</p>
            <p>${t("ayuda.directivas.rp5")}</p>
          </section>
          <section>
            <h3>${t("ayuda.ejemplos.titulo")}</h3>
            <p>${t("ayuda.ejemplos.texto")}</p>
          </section>
          <section>
            <h3>${t("ayuda.breakpoints.titulo")}</h3>
            <p>${t("ayuda.breakpoints.texto")}</p>
            <p>${t("ayuda.depuracion.texto")}</p>
          </section>
        </div>
      </div>`;
    document.body.appendChild(this.overlay);
    this.overlay.querySelector<HTMLButtonElement>(".ayuda-cerrar")!.onclick = () => this.cerrar();
    this.overlay.addEventListener("click", (e) => {
      if (e.target === this.overlay) this.cerrar();
    });
  }

  /** Cierra con Escape; se registra al abrir y se quita al cerrar
   *  para no dejar un listener global colgado mientras está oculto. */
  private onKeydown = (e: KeyboardEvent): void => {
    if (e.key === "Escape") this.cerrar();
  };

  abrir(): void {
    this.overlay.hidden = false;
    document.addEventListener("keydown", this.onKeydown);
  }

  cerrar(): void {
    this.overlay.hidden = true;
    document.removeEventListener("keydown", this.onKeydown);
  }
}
