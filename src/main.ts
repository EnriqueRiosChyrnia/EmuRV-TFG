// EmuRV — punto de entrada del frontend.
// Conecta toolbar, editor, consola y (en F3) los paneles de estado con
// los comandos y eventos del backend.
import type { UnlistenFn } from "@tauri-apps/api/event";
import type { EditorView } from "@codemirror/view";
import {
  api,
  events,
  type AsmError,
  type AssembleOk,
  type BpSpec,
  type StoppedEvent,
} from "./api";
import {
  createEditor,
  getBreakpointLines,
  getDoc,
  setDoc,
  setErrors,
  setExecLine,
  setReadOnly,
  toggleBreakpointAtLine,
} from "./editor";
import { t } from "./i18n";
import { Ayuda } from "./paneles/ayuda";
import { Consola } from "./paneles/consola";
import { Depuracion, type FilaBreakpoint } from "./paneles/depuracion";
import { Memoria } from "./paneles/memoria";
import { Registros } from "./paneles/registros";
import { Video } from "./paneles/video";

import ejContador from "../ejemplos/03-contador.asm?raw";
import ejFibonacci from "../ejemplos/04-fibonacci.asm?raw";
import ejHola from "../ejemplos/01-hola-mundo.asm?raw";
import ejPila from "../ejemplos/05-pila-funciones.asm?raw";
import ejSuma from "../ejemplos/02-suma-interactiva.asm?raw";
import ejFranjas from "../ejemplos/07-franjas.asm?raw";
import ejTetrisGrafico from "../ejemplos/08-tetris-grafico.asm?raw";
import ejEcoTeclado from "../ejemplos/09-eco-teclado.asm?raw";
import ejTetrisRt from "../ejemplos/10-tetris-tiempo-real.asm?raw";
import ejTrap from "../ejemplos/12-trap-handler.asm?raw";
import ejFlotantes from "../ejemplos/13-flotantes.asm?raw";

const EJEMPLOS: { clave: string; codigo: string }[] = [
  { clave: "ejemplo.hola", codigo: ejHola },
  { clave: "ejemplo.suma", codigo: ejSuma },
  { clave: "ejemplo.contador", codigo: ejContador },
  { clave: "ejemplo.fibonacci", codigo: ejFibonacci },
  { clave: "ejemplo.pila", codigo: ejPila },
  { clave: "ejemplo.franjas", codigo: ejFranjas },
  { clave: "ejemplo.tetris", codigo: ejTetrisGrafico },
  { clave: "ejemplo.eco", codigo: ejEcoTeclado },
  { clave: "ejemplo.tetrisRt", codigo: ejTetrisRt },
  { clave: "ejemplo.trap", codigo: ejTrap },
  { clave: "ejemplo.flotantes", codigo: ejFlotantes },
];

/** Estado de la UI respecto del programa. */
type Modo = "sin_programa" | "listo" | "corriendo" | "esperando" | "terminado";

class App {
  private editor: EditorView;
  private consola: Consola;
  private registros: Registros;
  private memoria: Memoria;
  private depuracion: Depuracion;
  private video: Video;
  private ayuda: Ayuda;
  private programa: AssembleOk | null = null;
  /** El texto del editor cambió desde el último ensamblado exitoso:
   *  line_to_addr ya no es confiable para validar breakpoints. */
  private programaDesactualizado = false;
  /** Ruta en disco del buffer (RP5): base de resolución de .include.
   *  null = buffer nunca guardado (ejemplos embebidos incluidos). */
  private rutaArchivo: string | null = null;
  private reinicioPedido = false;
  /** Modo binario (RP11): el editor muestra el listado desensamblado de
   *  un ELF importado, solo lectura; Ensamblar queda deshabilitado. */
  private modoBinario = false;
  private bannerBinario: HTMLElement | null = null;
  /** Cancelaciones de los listen() de Tauri (evita fugas al re-suscribir). */
  private unlisteners: UnlistenFn[] = [];

  private botones: Record<string, HTMLButtonElement> = {};
  private estadoEl: HTMLElement;

  constructor() {
    this.construirToolbar();
    this.traducirTitulos();
    this.estadoEl = document.getElementById("estado")!;
    this.editor = createEditor(
      document.getElementById("editor")!,
      ejHola,
      () => this.alCambiarBreakpoints(),
      (linea) => this.puedeAgregarBreakpoint(linea),
      () => {
        this.programaDesactualizado = true;
      },
    );
    this.consola = new Consola(document.getElementById("consola")!);
    this.registros = new Registros(document.getElementById("registros")!);
    this.memoria = new Memoria(document.getElementById("memoria")!);
    this.depuracion = new Depuracion(document.getElementById("depuracion")!, {
      // Condición editada: re-mandar el juego completo (el rechazo del
      // backend vuelve al div de error del panel vía enviarBreakpoints).
      onCondicionCambiada: () => this.enviarBreakpoints(),
      // Quitar desde el panel = quitar el punto del margen del editor.
      onQuitarBreakpoint: (linea) => {
        toggleBreakpointAtLine(this.editor, linea);
        this.alCambiarBreakpoints();
      },
    });
    this.video = new Video(
      document.getElementById("panel-video")!,
      document.getElementById("video-canvas") as HTMLCanvasElement,
      document.getElementById("video-desacoplar")!,
    );
    this.ayuda = new Ayuda();
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
        this.salirModoBinario();
        setDoc(this.editor, EJEMPLOS[i].codigo);
        this.programa = null;
        this.rutaArchivo = null; // ejemplo embebido: sin base de .include
        this.setModo("sin_programa", t("estado.sin_programa"));
      }
      sel.value = "";
    };
    bar.appendChild(sel);

    const espacio = document.createElement("span");
    espacio.className = "espacio";
    bar.appendChild(espacio);
    boton("video", "toolbar.video").onclick = () => this.video.alternar();
    boton("abrir", "toolbar.abrir").onclick = () => void this.abrirArchivo();
    boton("guardar", "toolbar.guardar").onclick = () => void this.guardarArchivo();
    boton("exportar", "toolbar.exportar").onclick = () => void this.exportarElf();
    boton("ayuda", "toolbar.ayuda").onclick = () => this.ayuda.abrir();
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
    document.querySelectorAll<HTMLElement>("[data-i18n-title]").forEach((el) => {
      el.title = t(el.dataset.i18nTitle!);
    });
  }

  // ----- eventos del backend -----

  private async suscribirEventos(): Promise<void> {
    // Idempotencia: si se vuelve a suscribir, primero se cancelan las
    // suscripciones anteriores para no duplicar listeners.
    this.desuscribirEventos();
    this.unlisteners.push(await events.onConsole((text) => this.consola.append(text)));
    this.unlisteners.push(await events.onConsoleClear(() => this.consola.clear()));
    this.unlisteners.push(
      await events.onInputRequest(() => {
        this.consola.setInputMode(true);
        this.setModo("esperando", t("estado.esperando_entrada"));
      }),
    );
    this.unlisteners.push(await events.onStopped((ev) => this.alDetenerse(ev)));
    // Refresco periódico (~10 Hz) durante ejecución larga (RNF-03):
    // solo registros; la memoria se relee al detenerse.
    this.unlisteners.push(await events.onRegisters((st) => this.registros.actualizar(st)));
    // El programa usa la pantalla gráfica: mostrar el panel (RB-4) y
    // arrancar el volcado a ~30 Hz mientras corre (RB-2).
    this.unlisteners.push(
      await events.onVideoActivo(() => {
        this.video.mostrar();
        this.video.iniciarTimer();
      }),
    );
  }

  /** Cancela todas las suscripciones a eventos del backend. */
  private desuscribirEventos(): void {
    for (const un of this.unlisteners) un();
    this.unlisteners = [];
  }

  private alDetenerse(ev: StoppedEvent): void {
    this.consola.setInputMode(false);
    // Último volcado de la pantalla con el estado final del frame.
    this.video.detenerTimerYRefrescar();
    // Las vistas se refrescan al detenerse (pausa/paso/breakpoint/fin),
    // jamás por instrucción (RNF-03).
    if (this.reinicioPedido) {
      this.registros.reiniciar(ev.state);
    } else {
      this.registros.actualizar(ev.state);
    }
    // En un watchpoint la vista navega a la dirección del acceso (no se
    // dispara el refresco genérico en paralelo: ambos son async y el
    // formateo usa la dirección vigente al resolver — se pisarían).
    if (ev.reason.kind === "watchpoint") {
      void this.memoria.irA(ev.reason.addr);
    } else {
      void this.memoria.refrescar();
    }
    // Línea actual resaltada mientras el programa sigue vivo (RF-04);
    // "watchpoint" cuenta como vivo, igual que breakpoint/pausa.
    const sigueVivo = ev.reason.kind !== "exited" && ev.reason.kind !== "runtime_error";
    setExecLine(this.editor, sigueVivo ? ev.state.line : null);
    const linea = ev.state.line ?? 0;
    switch (ev.reason.kind) {
      case "exited":
        this.setModo("terminado", t("estado.terminado", { code: ev.reason.code }), "ok");
        break;
      case "breakpoint":
        this.setModo("listo", t("estado.breakpoint", { line: linea }));
        break;
      case "watchpoint": {
        const accion =
          ev.reason.access === "read"
            ? t("estado.watchpoint.leyo")
            : ev.reason.access === "write"
              ? t("estado.watchpoint.escribio")
              : t("estado.watchpoint.accedio");
        const hex8 = (v: number) => (v >>> 0).toString(16).padStart(8, "0");
        this.setModo(
          "listo",
          t("estado.watchpoint", {
            accion,
            value: hex8(ev.reason.value),
            addr: hex8(ev.reason.addr),
            line: linea,
          }),
        );
        break;
      }
      case "ebreak_pause":
        // Pausa por software (instrucción ebreak): el pc ya quedó en la
        // instrucción siguiente; Ejecutar/Paso continúan desde ahí.
        this.setModo("listo", t("estado.ebreak", { line: linea }));
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
    if (this.modoBinario) return; // el botón está deshabilitado; cinturón
    try {
      const ok = await api.assemble(getDoc(this.editor), this.rutaArchivo);
      this.programa = ok;
      this.programaDesactualizado = false;
      this.consola.clear();
      this.consola.setInputMode(false);
      setErrors(this.editor, []);
      setExecLine(this.editor, null);
      this.memoria.setPrograma(ok);
      this.registros.reiniciar(await api.getState());
      await this.memoria.irA(ok.data_base);
      void this.cargarDisasmEnMemoria(); // columna de instrucción (RP11)
      // Los puntos del margen que cayeron en líneas sin código ejecutable
      // no representan un breakpoint real: se quitan y se avisa por consola
      // en vez de filtrarlos en silencio.
      for (const linea of getBreakpointLines(this.editor)) {
        if (ok.line_to_addr[String(linea)] === undefined) {
          toggleBreakpointAtLine(this.editor, linea);
          this.consola.append(`${t("breakpoint.lineaSinCodigo", { linea })}\n`, "error");
        }
      }
      this.sincronizarPanelDepuracion(); // direcciones nuevas en el panel
      this.enviarBreakpoints(); // los del margen aplican a la sesión nueva
      void this.depuracion.reaplicarWatchpoints(); // ídem watchpoints (RP7)
      this.setModo("listo", t("estado.ensamblado", { n: ok.text_len }), "ok");
    } catch (e) {
      const errores = e as AsmError[];
      this.programa = null;
      this.memoria.setPrograma(null);
      this.consola.clear();
      setErrors(this.editor, errores);
      setExecLine(this.editor, null);
      this.sincronizarPanelDepuracion(); // sin programa: direcciones fuera
      for (const err of errores) {
        this.consola.append(
          `${err.file ? `${err.file}: ` : ""}${t("error.posicion", { line: err.line, col: err.col, msg: err.msg })}\n`,
          "error",
        );
      }
      this.setModo("sin_programa", t("estado.errores", { n: errores.length }), "error");
    }
  }

  /** Un breakpoint nuevo solo vale en líneas con código ejecutable.
   *  Si no hay programa ensamblado, o el texto cambió desde el último
   *  ensamblado (line_to_addr quedó obsoleto), no hay forma confiable de
   *  saberlo y se permite; con programa vigente, se rechaza y se avisa. */
  private puedeAgregarBreakpoint(linea: number): boolean {
    if (!this.programa || this.programaDesactualizado) return true;
    if (this.programa.line_to_addr[String(linea)] !== undefined) return true;
    this.consola.append(`${t("breakpoint.lineaSinCodigo", { linea })}\n`, "error");
    return false;
  }

  /** Cambió el juego de breakpoints (margen del editor o botón quitar del
   *  panel): sincronizar la lista del panel y reenviar al backend. */
  private alCambiarBreakpoints(): void {
    this.sincronizarPanelDepuracion();
    this.enviarBreakpoints();
  }

  /** Refleja los puntos del margen en el panel de Depuración (RP7-F4):
   *  línea + dirección si hay programa ensamblado vigente. */
  private sincronizarPanelDepuracion(): void {
    const conAddr = this.programa !== null && !this.programaDesactualizado;
    const filas: FilaBreakpoint[] = getBreakpointLines(this.editor).map((linea) => ({
      linea,
      addr: conAddr ? (this.programa!.line_to_addr[String(linea)] ?? null) : null,
    }));
    this.depuracion.setFilasBreakpoints(filas);
  }

  /** Traduce las líneas con breakpoint a direcciones (con su condición del
   *  panel de Depuración) y las manda al backend. Las líneas sin código
   *  generado (comentarios, datos) se ignoran. Una condición mal escrita
   *  vuelve como rechazo didáctico y se muestra en el panel; el texto
   *  queda en el input para corregirlo. */
  private enviarBreakpoints(): void {
    if (!this.programa) return;
    const bps: BpSpec[] = [];
    for (const linea of getBreakpointLines(this.editor)) {
      const addr = this.programa.line_to_addr[String(linea)];
      if (addr !== undefined) bps.push({ addr, cond: this.depuracion.condicionDe(linea) });
    }
    api
      .setBreakpoints(bps)
      .then(() => this.depuracion.limpiarError())
      .catch((e) => this.depuracion.mostrarError(String(e)));
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

  /** Abre un archivo del usuario: un fuente reemplaza el contenido del
   *  editor; un ELF (detección por magic en el backend, RP11) entra en
   *  modo binario con el listado desensamblado, solo lectura. */
  private async abrirArchivo(): Promise<void> {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const path = await open({
      title: t("archivo.tituloAbrir"),
      filters: [{ name: t("archivo.filtro"), extensions: ["asm", "s", "elf"] }],
    });
    if (!path) return;
    try {
      const apertura = await api.abrirPrograma(path);
      if (apertura.tipo === "fuente") {
        this.salirModoBinario();
        setDoc(this.editor, apertura.contenido);
        this.programa = null;
        this.rutaArchivo = path; // base para .include (RP5)
        this.setModo("sin_programa", t("archivo.abierto", { path }));
      } else {
        await this.entrarModoBinario(apertura.ok, apertura.listado, path);
      }
    } catch (e) {
      this.consola.append(`${t("archivo.errorAbrir", { message: String(e) })}\n`, "error");
    }
  }

  /** Modo binario (RP11): la sesión ya quedó creada en el backend (como
   *  un Ensamblar exitoso); acá se prepara la UI: listado solo-lectura,
   *  banner y Ensamblar deshabilitado. Breakpoints, Paso, Ejecutar y el
   *  resaltado de línea funcionan igual: cada línea del listado es una
   *  dirección (line_to_addr viene armado del listado). */
  private async entrarModoBinario(ok: AssembleOk, listado: string, path: string): Promise<void> {
    setDoc(this.editor, listado);
    setReadOnly(this.editor, true);
    this.modoBinario = true;
    this.programa = ok;
    this.programaDesactualizado = false;
    this.rutaArchivo = null;
    this.botones["ensamblar"].disabled = true;
    if (!this.bannerBinario) {
      this.bannerBinario = document.createElement("div");
      this.bannerBinario.id = "binario-banner";
      const editorEl = document.getElementById("editor")!;
      editorEl.parentElement!.insertBefore(this.bannerBinario, editorEl);
    }
    this.bannerBinario.textContent = t("binario.banner");
    this.bannerBinario.hidden = false;
    this.consola.clear();
    this.consola.setInputMode(false);
    setErrors(this.editor, []);
    setExecLine(this.editor, null);
    this.memoria.setPrograma(ok);
    void this.cargarDisasmEnMemoria();
    this.registros.reiniciar(await api.getState());
    await this.memoria.irA(ok.text_base);
    this.sincronizarPanelDepuracion();
    this.enviarBreakpoints();
    void this.depuracion.reaplicarWatchpoints();
    this.setModo("listo", t("binario.abierto", { path }), "ok");
  }

  /** Vuelta al modo fuente (abrir un .s o cargar un ejemplo). */
  private salirModoBinario(): void {
    if (!this.modoBinario) return;
    this.modoBinario = false;
    setReadOnly(this.editor, false);
    if (this.bannerBinario) this.bannerBinario.hidden = true;
    this.botones["ensamblar"].disabled = false;
    this.memoria.setDisasm(null);
  }

  /** Columna de instrucción del panel Memoria (RP11): vale para fuentes
   *  normales y binarios por igual — el beneficio es general. */
  private async cargarDisasmEnMemoria(): Promise<void> {
    try {
      this.memoria.setDisasm(await api.getDisasm());
    } catch {
      this.memoria.setDisasm(null);
    }
  }

  /** Guarda el contenido actual del editor como archivo .asm. */
  private async guardarArchivo(): Promise<void> {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const path = await save({
      title: t("archivo.tituloGuardar"),
      defaultPath: "programa.asm",
      filters: [{ name: t("archivo.filtro"), extensions: ["asm", "s"] }],
    });
    if (!path) return;
    try {
      await api.guardarArchivo(path, getDoc(this.editor));
      this.rutaArchivo = path; // base para .include (RP5)
      this.estadoEl.textContent = t("archivo.guardado", { path });
      this.estadoEl.className = "ok";
    } catch (e) {
      this.consola.append(`${t("archivo.errorGuardar", { message: String(e) })}\n`, "error");
    }
  }

  /** Exporta el programa ensamblado como ELF32 RISC-V (RF-08). */
  private async exportarElf(): Promise<void> {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const path = await save({
      title: t("exportar.titulo"),
      defaultPath: "programa.elf",
      filters: [{ name: "ELF", extensions: ["elf"] }],
    });
    if (!path) return;
    try {
      await api.exportElf(path);
      this.estadoEl.textContent = t("exportar.ok", { path });
      this.estadoEl.className = "ok";
    } catch (e) {
      this.estadoEl.textContent = t("exportar.error", { message: String(e) });
      this.estadoEl.className = "error";
    }
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
    b.exportar.disabled = !hayPrograma || modo === "corriendo" || modo === "esperando";
    // Abrir reemplazaría el editor (y Guardar leería un estado a medias)
    // con el hilo de ejecución vivo: mismo criterio que Exportar.
    b.abrir.disabled = modo === "corriendo" || modo === "esperando";
    b.guardar.disabled = modo === "corriendo" || modo === "esperando";
  }
}

window.addEventListener("DOMContentLoaded", () => {
  new App();

});
