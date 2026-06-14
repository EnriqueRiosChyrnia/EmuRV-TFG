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
import { getLang, setLang, t } from "./i18n";
import { icono } from "./iconos";
// Wordmark de marca (RP19-F3): SVG bi-tono inline; sus clases .logo-emu/.logo-rv
// se pintan con var(--acento)/var(--texto), así se adapta a cada tema.
import wordmark from "./assets/emurv-wordmark.svg?raw";
import { inicializarTema, leerTema, setTema, TEMAS, type Tema } from "./temas";
import type { Leccion } from "./lecciones";
import { Ayuda } from "./paneles/ayuda";
import { Consola } from "./paneles/consola";
import { Depuracion, type FilaBreakpoint } from "./paneles/depuracion";
import { Lecciones } from "./paneles/lecciones";
import { Ejemplos } from "./paneles/ejemplos";
import { desacoplarPanel, botonDesacople } from "./paneles/desacople";
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
import ejAtomicas from "../ejemplos/14-atomicas.asm?raw";

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
  { clave: "ejemplo.atomicas", codigo: ejAtomicas },
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
  private lecciones: Lecciones;
  private ejemplos!: Ejemplos;
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
    // RP18-F3: Memoria plegable (abierta por defecto; el alumno la cierra si
    // necesita espacio). Depuración ya se autogestiona (arranca .colapsado
    // en index.html y se expande sola al primer breakpoint, ver abajo).
    this.hacerPanelPlegable("panel-memoria");
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
    this.lecciones = new Lecciones({
      onAbrir: (l) => this.abrirLeccion(l),
      onAbrirMultiarchivo: (l) => void this.abrirLeccionMultiarchivo(l),
    });
    this.ejemplos = new Ejemplos(EJEMPLOS, (i) => this.cargarEjemplo(i));
    this.configurarDesacople();
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
    // Wordmark de marca al inicio, antes del primer separador (RP19-F3).
    const marca = document.createElement("span");
    marca.className = "toolbar-marca";
    marca.innerHTML = wordmark;
    bar.appendChild(marca);
    bar.appendChild(this.sep());
    const boton = (id: string, clave: string, clase = "", parent: HTMLElement = bar) => {
      const b = document.createElement("button");
      b.id = id;
      const texto = t(clave);
      // Icono SVG inline (RP17-F2) + texto. El icono es decorativo
      // (aria-hidden); el texto da la etiqueta. aria-label explícito por las
      // dudas de lectores de pantalla.
      const svg = icono(id);
      b.innerHTML = svg ? `${svg}<span>${texto}</span>` : texto;
      b.setAttribute("aria-label", texto);
      if (clase) b.className = clase;
      parent.appendChild(b);
      this.botones[id] = b;
      return b;
    };
    boton("ensamblar", "toolbar.ensamblar", "principal").onclick = () => void this.ensamblar();
    bar.appendChild(this.sep());
    // Grupo de ejecución segmentado (RP18-F3): los 4 botones de ejecución
    // como UN bloque, no 4 cajas sueltas.
    const grupoEjec = document.createElement("div");
    grupoEjec.className = "grupo-ejec";
    boton("ejecutar", "toolbar.ejecutar", "", grupoEjec).onclick = () => void this.ejecutar();
    boton("paso", "toolbar.paso", "", grupoEjec).onclick = () => void api.step();
    boton("detener", "toolbar.detener", "", grupoEjec).onclick = () => void api.stop();
    boton("reiniciar", "toolbar.reiniciar", "", grupoEjec).onclick = () => void this.reiniciar();
    bar.appendChild(grupoEjec);
    bar.appendChild(this.sep());

    // Ejemplos (RP20-F3): botón que abre el modal con la lista (antes era un
    // <select>). Cargar un ejemplo hace lo mismo que antes (cargarEjemplo).
    const btnEjemplos = boton("ejemplos", "toolbar.ejemplos");
    btnEjemplos.onclick = () => this.ejemplos.abrir(btnEjemplos);

    const espacio = document.createElement("span");
    espacio.className = "espacio";
    bar.appendChild(espacio);
    // Secundarios atenuados (RP18-F3): no compiten con Ensamblar (.principal).
    const btnLecciones = boton("lecciones", "toolbar.lecciones", "deemph");
    btnLecciones.onclick = () => this.lecciones.abrir(btnLecciones);
    boton("video", "toolbar.video", "deemph").onclick = () => this.video.alternar();
    boton("abrir", "toolbar.abrir", "deemph").onclick = () => void this.abrirArchivo();
    boton("guardar", "toolbar.guardar", "deemph").onclick = () => void this.guardarArchivo();
    boton("exportar", "toolbar.exportar", "deemph").onclick = () => void this.exportarElf();
    const btnAyuda = boton("ayuda", "toolbar.ayuda", "deemph");
    btnAyuda.onclick = () => this.ayuda.abrir(btnAyuda);

    // Selector de idioma (RP16): es/en. Cambiar persiste y recarga la
    // ventana (la UI se construye al cargar; recargar evita estado a medio
    // traducir). El español va primero (lengua primaria).
    const idioma = document.createElement("select");
    idioma.id = "idioma";
    idioma.title = t("toolbar.idioma");
    for (const [valor, etiqueta] of [
      ["es", "ES"],
      ["en", "EN"],
    ] as const) {
      const o = document.createElement("option");
      o.value = valor;
      o.textContent = etiqueta;
      if (valor === getLang()) o.selected = true;
      idioma.appendChild(o);
    }
    idioma.onchange = () => {
      const v = idioma.value === "en" ? "en" : "es";
      if (v !== getLang()) {
        setLang(v);
        location.reload();
      }
    };
    bar.appendChild(idioma);

    // Selector de tema (RP17-F3): aplica EN VIVO, sin recargar (variables
    // CSS). El default «Educativo» va primero.
    const tema = document.createElement("select");
    tema.id = "tema";
    tema.title = t("toolbar.tema");
    for (const valor of TEMAS) {
      const o = document.createElement("option");
      o.value = valor;
      o.textContent = t(`tema.${valor}`);
      if (valor === leerTema()) o.selected = true;
      tema.appendChild(o);
    }
    tema.onchange = () => {
      setTema(tema.value as Tema);
    };
    bar.appendChild(tema);
  }

  private sep(): HTMLElement {
    const s = document.createElement("span");
    s.className = "separador";
    return s;
  }

  /** Hace plegable el título de un panel: click en el <h2> alterna
   *  `.colapsado` (CSS oculta el cuerpo y, en el lateral flex, encoge el
   *  panel). Arranca como esté en index.html. (RP18-F3) */
  private hacerPanelPlegable(panelId: string): void {
    const panel = document.getElementById(panelId);
    const titulo = panel?.querySelector<HTMLElement>("h2");
    if (!panel || !titulo) return;
    titulo.classList.add("panel-titulo-clickeable");
    titulo.addEventListener("click", () => panel.classList.toggle("colapsado"));
  }

  /** Botón ⧉ en cada panel desacoplable (RP22): abre el panel en su ventana
   *  (modo MOVER); al reacoplar refresca el panel embebido con el estado vivo
   *  (las ediciones desde la ventana no emiten evento). Se cablea Registros
   *  acá (F1); Memoria/Depuración se suman en F2/F3. */
  private configurarDesacople(): void {
    this.agregarDesacople("registros", "panel-registros", "panel.registros", 380, 580, () => {
      void api
        .getState()
        .then((st) => this.registros.actualizar(st))
        .catch(() => {});
    });
    this.agregarDesacople("memoria", "panel-memoria", "panel.memoria", 560, 460, () => {
      void this.memoria.refrescar();
    });
  }

  private agregarDesacople(
    id: string,
    seccionId: string,
    claveTitulo: string,
    width: number,
    height: number,
    alReacoplar: () => void,
  ): void {
    const h2 = document.querySelector<HTMLElement>(`#${seccionId} h2`);
    if (!h2) return;
    h2.appendChild(
      botonDesacople(t("desacople.titulo"), () => {
        void desacoplarPanel({ id, seccionId, titulo: t(claveTitulo), width, height, alReacoplar });
      }),
    );
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
    // RP18-F3: progressive disclosure — la Depuración arranca plegada; en
    // cuanto el alumno pone un breakpoint, se revela (ahí empieza a importar).
    if (getBreakpointLines(this.editor).length > 0) {
      document.getElementById("panel-depuracion")?.classList.remove("colapsado");
    }
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

  /** Carga una lección normal al editor (buffer sin ruta, como un ejemplo).
   *  La marca como completada al abrirla (progreso persistido, RP15). */
  private abrirLeccion(l: Leccion): void {
    this.salirModoBinario();
    setDoc(this.editor, l.codigo);
    this.programa = null;
    this.rutaArchivo = null; // lección embebida: sin base de .include
    this.setModo("sin_programa", t("leccion.abierta", { titulo: l.titulo }));
    this.lecciones.marcarCompletada(l.numero);
  }

  /** Caso multi-archivo (08): el `.include` necesita disco (RP5). Se ofrece
   *  guardar los 3 archivos en una carpeta y abrir main.s CON esa ruta base;
   *  si el usuario cancela, aviso didáctico explicando por qué. */
  private async abrirLeccionMultiarchivo(l: Leccion): Promise<void> {
    const archivos = l.archivos;
    if (!archivos) return;
    const { open } = await import("@tauri-apps/plugin-dialog");
    const carpeta = await open({
      title: t("leccion.elegirCarpeta"),
      directory: true,
    });
    if (typeof carpeta !== "string") {
      // Canceló: aviso didáctico, no se abre nada roto.
      this.consola.clear();
      this.consola.append(`${t("leccion.multiarchivoCancelado")}\n`, "error");
      return;
    }
    try {
      const sep = carpeta.includes("\\") ? "\\" : "/";
      let mainPath = "";
      for (const [nombre, contenido] of Object.entries(archivos)) {
        const ruta = `${carpeta}${sep}${nombre}`;
        await api.guardarArchivo(ruta, contenido);
        if (nombre === "main.s") mainPath = ruta;
      }
      this.salirModoBinario();
      setDoc(this.editor, archivos["main.s"]);
      this.programa = null;
      this.rutaArchivo = mainPath; // base_dir para .include (RP5)
      this.setModo("sin_programa", t("leccion.multiarchivoListo", { carpeta }));
      this.lecciones.marcarCompletada(l.numero);
    } catch (e) {
      this.consola.append(`${t("archivo.errorGuardar", { message: String(e) })}\n`, "error");
    }
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

  /** Carga un ejemplo embebido al editor (RP20-F3, lo invoca el modal de
   *  Ejemplos). Misma lógica que el viejo <select> de la toolbar. */
  private cargarEjemplo(i: number): void {
    this.salirModoBinario();
    setDoc(this.editor, EJEMPLOS[i].codigo);
    this.programa = null;
    this.rutaArchivo = null; // ejemplo embebido: sin base de .include
    this.setModo("sin_programa", t("estado.sin_programa"));
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
  inicializarTema(); // aplica el tema guardado antes de construir la UI (RP17)
  new App();
});
