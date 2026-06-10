// EmuRV — punto de entrada del frontend.
// Conecta toolbar, editor, consola y (en F3) los paneles de estado con
// los comandos y eventos del backend.
import type { EditorView } from "@codemirror/view";
import { api, events, type AsmError, type AssembleOk, type StoppedEvent } from "./api";
import { createEditor, getDoc, setDoc } from "./editor";
import { t } from "./i18n";
import { Consola } from "./paneles/consola";

import ejContador from "../ejemplos/03-contador.asm?raw";
import ejFibonacci from "../ejemplos/04-fibonacci.asm?raw";
import ejHola from "../ejemplos/01-hola-mundo.asm?raw";
import ejPila from "../ejemplos/05-pila-funciones.asm?raw";
import ejSuma from "../ejemplos/02-suma-interactiva.asm?raw";

const EJEMPLOS: { clave: string; codigo: string }[] = [
  { clave: "ejemplo.hola", codigo: ejHola },
  { clave: "ejemplo.suma", codigo: ejSuma },
  { clave: "ejemplo.contador", codigo: ejContador },
  { clave: "ejemplo.fibonacci", codigo: ejFibonacci },
  { clave: "ejemplo.pila", codigo: ejPila },
];

/** Estado de la UI respecto del programa. */
type Modo = "sin_programa" | "listo" | "corriendo" | "esperando" | "terminado";

class App {
  private editor: EditorView;
  private consola: Consola;
  private programa: AssembleOk | null = null;
  private reinicioPedido = false;

  private botones: Record<string, HTMLButtonElement> = {};
  private estadoEl: HTMLElement;

  constructor() {
    this.construirToolbar();
    this.traducirTitulos();
    this.estadoEl = document.getElementById("estado")!;
    this.editor = createEditor(document.getElementById("editor")!, ejHola);
    this.consola = new Consola(document.getElementById("consola")!);
    this.consola.setOnLine((line) => {
      void api.sendInput(line);
      this.setModo("corriendo", t("estado.corriendo"));
    });
    void this.suscribirEventos();
    this.setModo("sin_programa", t("estado.sin_programa"));
  }

  // ----- construcción de la UI -----

  private construirToolbar(): void {
    const bar = document.getElementById("toolbar")!;
    const boton = (id: string, clave: string, clase = "") => {
      const b = document.createElement("button");
      b.id = id;
      b.textContent = t(clave);
      if (clase) b.className = clase;
      bar.appendChild(b);
      this.botones[id] = b;
      return b;
    };
    boton("ensamblar", "toolbar.ensamblar", "principal").onclick = () => void this.ensamblar();
    bar.appendChild(this.sep());
    boton("ejecutar", "toolbar.ejecutar").onclick = () => void this.ejecutar();
    boton("paso", "toolbar.paso").onclick = () => void api.step();
    boton("detener", "toolbar.detener").onclick = () => void api.stop();
    boton("reiniciar", "toolbar.reiniciar").onclick = () => void this.reiniciar();
    bar.appendChild(this.sep());

    const sel = document.createElement("select");
    sel.id = "ejemplos";
    const ph = document.createElement("option");
    ph.textContent = t("toolbar.ejemplos");
    ph.value = "";
    sel.appendChild(ph);
    EJEMPLOS.forEach((e, i) => {
      const o = document.createElement("option");
      o.value = String(i);
      o.textContent = t(e.clave);
      sel.appendChild(o);
    });
    sel.onchange = () => {
      const i = Number(sel.value);
      if (!Number.isNaN(i) && sel.value !== "") {
        setDoc(this.editor, EJEMPLOS[i].codigo);
        this.programa = null;
        this.setModo("sin_programa", t("estado.sin_programa"));
      }
      sel.value = "";
    };
    bar.appendChild(sel);

    const espacio = document.createElement("span");
    espacio.className = "espacio";
    bar.appendChild(espacio);
    // Exportar ELF y Ayuda se habilitan en la fase F5.
    boton("exportar", "toolbar.exportar").disabled = true;
    boton("ayuda", "toolbar.ayuda").disabled = true;
  }

  private sep(): HTMLElement {
    const s = document.createElement("span");
    s.className = "separador";
    return s;
  }

  private traducirTitulos(): void {
    document.querySelectorAll<HTMLElement>("[data-i18n]").forEach((el) => {
      el.textContent = t(el.dataset.i18n!);
    });
  }

  // ----- eventos del backend -----

  private async suscribirEventos(): Promise<void> {
    await events.onConsole((text) => this.consola.append(text));
    await events.onInputRequest(() => {
      this.consola.setInputMode(true);
      this.setModo("esperando", t("estado.esperando_entrada"));
    });
    await events.onStopped((ev) => this.alDetenerse(ev));
    // emu://registros se consume en F3 (paneles de estado).
  }

  private alDetenerse(ev: StoppedEvent): void {
    this.consola.setInputMode(false);
    const linea = ev.state.line ?? 0;
    switch (ev.reason.kind) {
      case "exited":
        this.setModo("terminado", t("estado.terminado", { code: ev.reason.code }), "ok");
        break;
      case "breakpoint":
        this.setModo("listo", t("estado.breakpoint", { line: linea }));
        break;
      case "step_done":
        this.setModo(
          "listo",
          ev.state.line ? t("estado.paso", { line: linea }) : t("estado.paso_sin_linea"),
        );
        break;
      case "paused":
        if (this.reinicioPedido) {
          this.reinicioPedido = false;
          this.setModo("listo", t("estado.reiniciado"));
        } else {
          this.setModo("listo", t("estado.pausado"));
        }
        break;
      case "runtime_error":
        this.consola.append(`\n${ev.reason.message}\n`, "error");
        this.setModo(
          "terminado",
          t("estado.error_ejecucion", { message: ev.reason.message }),
          "error",
        );
        break;
    }
  }

  // ----- acciones -----

  private async ensamblar(): Promise<void> {
    try {
      const ok = await api.assemble(getDoc(this.editor));
      this.programa = ok;
      this.consola.clear();
      this.consola.setInputMode(false);
      this.setModo("listo", t("estado.ensamblado", { n: ok.text_len }), "ok");
    } catch (e) {
      const errores = e as AsmError[];
      this.programa = null;
      this.consola.clear();
      for (const err of errores) {
        this.consola.append(`línea ${err.line}, col ${err.col}: ${err.msg}\n`, "error");
      }
      this.setModo("sin_programa", t("estado.errores", { n: errores.length }), "error");
    }
  }

  private async ejecutar(): Promise<void> {
    this.setModo("corriendo", t("estado.corriendo"));
    await api.run();
  }

  private async reiniciar(): Promise<void> {
    this.reinicioPedido = true;
    this.consola.setInputMode(false);
    await api.reset();
  }

  // ----- estado de la UI -----

  private setModo(modo: Modo, mensaje: string, clase: "" | "ok" | "error" = ""): void {
    this.estadoEl.textContent = mensaje;
    this.estadoEl.className = clase;
    const b = this.botones;
    const hayPrograma = this.programa !== null;
    const parado = modo === "listo" || modo === "terminado";
    b.ensamblar.disabled = modo === "corriendo" || modo === "esperando";
    b.ejecutar.disabled = !hayPrograma || !parado || modo === "terminado";
    b.paso.disabled = !hayPrograma || !parado || modo === "terminado";
    b.detener.disabled = modo !== "corriendo" && modo !== "esperando";
    b.reiniciar.disabled = !hayPrograma || modo === "corriendo" || modo === "esperando";
  }
}

window.addEventListener("DOMContentLoaded", () => {
  new App();
});
