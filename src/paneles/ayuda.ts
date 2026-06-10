// Panel de ayuda integrada (RF-07: tabla de syscalls documentada; RF-09:
// referencia de los ejemplos). Overlay modal sobre la app.
import { t } from "../i18n";

export class Ayuda {
  private overlay: HTMLElement;

  constructor() {
    this.overlay = document.createElement("div");
    this.overlay.id = "ayuda-overlay";
    this.overlay.hidden = true;
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
                <tr><td>10</td><td>${t("ayuda.syscalls.s10")}</td><td>—</td></tr>
                <tr><td>11</td><td>${t("ayuda.syscalls.s11")}</td><td>a0 = código de carácter</td></tr>
              </tbody>
            </table>
            <pre class="ayuda-ejemplo">    li   a7, 4        # servicio: imprimir cadena
    la   a0, mensaje  # argumento
    ecall             # llamada al sistema</pre>
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
            <h3>${t("ayuda.directivas.titulo")}</h3>
            <p class="mono">.text .data .word .half .byte .ascii .asciiz .string .space .align .globl</p>
          </section>
          <section>
            <h3>${t("ayuda.ejemplos.titulo")}</h3>
            <p>${t("ayuda.ejemplos.texto")}</p>
          </section>
          <section>
            <h3>${t("ayuda.breakpoints.titulo")}</h3>
            <p>${t("ayuda.breakpoints.texto")}</p>
          </section>
        </div>
      </div>`;
    document.body.appendChild(this.overlay);
    this.overlay.querySelector<HTMLButtonElement>(".ayuda-cerrar")!.onclick = () => this.cerrar();
    this.overlay.addEventListener("click", (e) => {
      if (e.target === this.overlay) this.cerrar();
    });
    document.addEventListener("keydown", (e) => {
      if (e.key === "Escape") this.cerrar();
    });
  }

  abrir(): void {
    this.overlay.hidden = false;
  }

  cerrar(): void {
    this.overlay.hidden = true;
  }
}
