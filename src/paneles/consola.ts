// Consola integrada (RF-07): salida del programa y entrada bloqueante
// para la syscall 5 (el modo entrada se activa en la fase F3).
import { t } from "../i18n";

export class Consola {
  private out: HTMLElement;
  private form: HTMLFormElement;
  private input: HTMLInputElement;
  private onLine: (line: string) => void = () => {};

  constructor(root: HTMLElement) {
    root.innerHTML = `
      <button type="button" class="consola-limpiar" data-rol="limpiar"
              title="${t("consola.limpiar.titulo")}">${t("consola.limpiar")}</button>
      <div class="consola-salida" data-rol="salida"></div>
      <form class="consola-entrada" data-rol="form">
        <span class="consola-prompt">›</span>
        <input type="text" autocomplete="off" spellcheck="false"
               placeholder="${t("consola.placeholder")}" disabled />
      </form>`;
    this.out = root.querySelector("[data-rol=salida]")!;
    this.form = root.querySelector("[data-rol=form]")!;
    this.input = this.form.querySelector("input")!;
    // Botón Limpiar (RP10): siempre visible, con o sin sesión — solo borra
    // el texto del panel; la máquina ni se entera.
    root.querySelector<HTMLButtonElement>("[data-rol=limpiar]")!.addEventListener("click", () => {
      this.clear();
    });
    this.form.addEventListener("submit", (e) => {
      e.preventDefault();
      const line = this.input.value;
      this.append(`${line}\n`, "eco");
      this.input.value = "";
      this.setInputMode(false);
      this.onLine(line);
    });
  }

  /** Agrega texto a la salida (clase opcional para eco/errores). */
  append(text: string, clase?: string): void {
    const span = document.createElement("span");
    if (clase) span.classList.add(`consola-${clase}`);
    span.textContent = text;
    this.out.appendChild(span);
    this.out.scrollTop = this.out.scrollHeight;
  }

  clear(): void {
    this.out.textContent = "";
  }

  /** Activa/desactiva el modo entrada (syscall 5). */
  setInputMode(on: boolean): void {
    this.input.disabled = !on;
    this.form.classList.toggle("activa", on);
    if (on) this.input.focus();
  }

  setOnLine(cb: (line: string) => void): void {
    this.onLine = cb;
  }
}
