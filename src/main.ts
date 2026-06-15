// EmuRV — punto de entrada del frontend.
// Conecta toolbar, editor, consola y (en F3) los paneles de estado con
// los comandos y eventos del backend.
import type { UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
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
  crearEstadoEditor,
  getBreakpointLines,
  getDoc,
  setDoc,
  setErrors,
  setExecLine,
  setReadOnly,
  toggleBreakpointAtLine,
  type EditorCallbacks,
} from "./editor";
import {
  crearDocumento,
  indiceActivoTrasCerrar,
  nombreDocumento,
  type Documento,
} from "./documento";
import { BarraPestanas } from "./paneles/pestanas";
import { getLang, setLang, t } from "./i18n";
import { icono } from "./iconos";
// Wordmark de marca (RP19-F3): SVG bi-tono inline; sus clases .logo-emu/.logo-rv
// se pintan con var(--acento)/var(--texto), así se adapta a cada tema.
import wordmark from "./assets/emurv-wordmark.svg?raw";
import { inicializarTema, leerTema, setTema, TEMAS, type Tema } from "./temas";
import { leerXlen, setXlen } from "./arquitectura";
import { guardarWorkspace, leerWorkspace } from "./workspace";
import type { Leccion } from "./lecciones";
import { Ayuda } from "./paneles/ayuda";
import { Consola } from "./paneles/consola";
import { Depuracion, type FilaBreakpoint } from "./paneles/depuracion";
import { Lecciones } from "./paneles/lecciones";
import { Ejemplos } from "./paneles/ejemplos";
import { EditorTema } from "./paneles/editor-tema";
import { desacoplarPanel, botonDesacople } from "./paneles/desacople";
import { depSync, type FilaConCond } from "./paneles/dep-sync";
import { panelSync } from "./paneles/panel-sync";
import { Memoria } from "./paneles/memoria";
import { Registros } from "./paneles/registros";
import { Video } from "./paneles/video";
import { Audio } from "./paneles/audio";

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
import ejRv64 from "../ejemplos/15-rv64.asm?raw";
import ejSonido from "../ejemplos/16-sonido.asm?raw";
import ejSprites from "../ejemplos/17-sprites.asm?raw";

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
  { clave: "ejemplo.rv64", codigo: ejRv64 },
  { clave: "ejemplo.sonido", codigo: ejSonido },
  { clave: "ejemplo.sprites", codigo: ejSprites },
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
  /** Dispositivo de sonido (RP31): reproduce los tonos del programa (WebAudio).
   *  No tiene UI; se conecta solo al evento `emu://sonido`. */
  private audio: Audio;
  private ayuda: Ayuda;
  private lecciones: Lecciones;
  private ejemplos!: Ejemplos;
  private editorTema: EditorTema;
  /** Documentos abiertos (RP30): una pestaña = un documento. Los campos
   *  doc-céntricos (programa/ruta/docLimpio/modoBinario/desactualizado) viven
   *  en el `Documento` activo; `App` los expone con accessors (más abajo) para
   *  no reescribir las ~50 referencias `this.X` y conservar la paridad. */
  private documentos: Documento[] = [];
  private activaIdx = 0;
  private barra!: BarraPestanas;
  /** Callbacks del editor (estables): construyen el `EditorState` de cada
   *  pestaña nueva (1 vista + N estados, ver BITACORA RP30-F1). */
  private editorCbs!: EditorCallbacks;
  /** Último modo seteado (RP30): para bloquear el cambio de pestaña con una
   *  ejecución viva (corriendo/esperando). */
  private modoActual: Modo = "sin_programa";
  /** Id del documento DUEÑO de la única sesión de backend (RP30): el que
   *  ensambló/abrió-binario por última vez. Solo ese documento puede ejecutar
   *  sin re-ensamblar; al activar otra pestaña con `programa`, hay que
   *  re-ensamblar (la sesión es única y sigue a la activa). */
  private sesionDocId: string | null = null;
  /** Depuración está en su ventana aparte (RP22-F3): main DELEGA el envío de
   *  breakpoints a la ventana (que tiene las condiciones) para no pisarlas;
   *  solo le manda la lista por evento. */
  private depDesacoplada = false;
  /** RP27: timer de debounce del guardado de workspace (undefined = sin pendiente). */
  private timerWorkspace: number | undefined;
  private reinicioPedido = false;
  /** Banner del modo binario (RP11): chrome COMPARTIDO del editor (no es
   *  per-documento); se muestra/oculta según el modo del documento activo. */
  private bannerBinario: HTMLElement | null = null;
  /** Cancelaciones de los listen() de Tauri (evita fugas al re-suscribir). */
  private unlisteners: UnlistenFn[] = [];

  private botones: Record<string, HTMLButtonElement> = {};
  private estadoEl: HTMLElement;

  // --- documento activo (RP30): los campos doc-céntricos enrutan a la pestaña
  //     activa, así el resto de App sigue escribiendo `this.programa`, etc. ---
  private get doc(): Documento {
    return this.documentos[this.activaIdx];
  }
  private get programa(): AssembleOk | null {
    return this.doc.programa;
  }
  private set programa(v: AssembleOk | null) {
    this.doc.programa = v;
  }
  /** El texto cambió desde el último ensamblado (line_to_addr obsoleto). */
  private get programaDesactualizado(): boolean {
    return this.doc.programaDesactualizado;
  }
  private set programaDesactualizado(v: boolean) {
    this.doc.programaDesactualizado = v;
  }
  /** Ruta en disco del buffer (RP5): base de resolución de `.include`. */
  private get rutaArchivo(): string | null {
    return this.doc.rutaArchivo;
  }
  private set rutaArchivo(v: string | null) {
    this.doc.rutaArchivo = v;
  }
  /** Contenido del último guardado/carga (RP23): base del «•». */
  private get docLimpio(): string {
    return this.doc.docLimpio;
  }
  private set docLimpio(v: string) {
    this.doc.docLimpio = v;
  }
  /** Modo binario (RP11): listado desensamblado, solo lectura. */
  private get modoBinario(): boolean {
    return this.doc.modoBinario;
  }
  private set modoBinario(v: boolean) {
    this.doc.modoBinario = v;
  }

  constructor() {
    this.construirToolbar();
    this.traducirTitulos();
    this.estadoEl = document.getElementById("estado")!;
    // Callbacks del editor, estables (sirven para todo `EditorState` nuevo).
    this.editorCbs = {
      onBreakpointsChanged: () => this.alCambiarBreakpoints(),
      puedeAgregarBreakpoint: (linea) => this.puedeAgregarBreakpoint(linea),
      onDocChanged: () => {
        this.programaDesactualizado = true;
        this.actualizarTitulo(); // RP25: «•» en vivo al teclear (título + barra)
        this.programarGuardadoWorkspace(); // RP27: persistir el buffer (debounced)
      },
    };
    this.editor = createEditor(document.getElementById("editor")!, ejHola, this.editorCbs);
    // Primera pestaña (RP30): el documento arranca con el ejemplo «Hola mundo».
    this.documentos = [crearDocumento(this.editor.state, { docLimpio: ejHola })];
    this.activaIdx = 0;
    // Barra de pestañas (RP30): entre la toolbar y el editor.
    this.barra = new BarraPestanas(document.getElementById("pestanas")!, {
      onActivar: (i) => this.activarPestana(i),
      onCerrar: (i) => this.cerrarPestana(i),
      onNueva: () => this.nuevaPestana(),
    });
    this.consola = new Consola(document.getElementById("consola")!);
    this.registros = new Registros(document.getElementById("registros")!);
    this.memoria = new Memoria(document.getElementById("memoria")!);
    // RP18-F3: Memoria plegable (abierta por defecto; el alumno la cierra si
    // necesita espacio). Depuración ya se autogestiona (arranca .colapsado
    // en index.html y se expande sola al primer breakpoint, ver abajo).
    // Registros plegable también (pedido del autor): abierto por defecto, el
    // alumno lo cierra si quiere. Sigue siendo el prioritario en el flex.
    this.hacerPanelPlegable("panel-registros");
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
    // Sonido (RP31): sin panel, solo el dispositivo. RP-30 rearquitectura el
    // resto; la huella acá es mínima (instanciar + conectar el evento abajo).
    this.audio = new Audio();
    this.ayuda = new Ayuda();
    this.lecciones = new Lecciones({
      onAbrir: (l) => this.abrirLeccion(l),
      onAbrirMultiarchivo: (l) => void this.abrirLeccionMultiarchivo(l),
    });
    this.ejemplos = new Ejemplos(EJEMPLOS, (i) => this.cargarEjemplo(i));
    // Editor de tema de usuario (RP28): al guardar/cerrar sincroniza el
    // <select> de tema de la toolbar con el tema activo persistido.
    this.editorTema = new EditorTema(() => {
      const sel = document.getElementById("tema") as HTMLSelectElement | null;
      if (sel) sel.value = leerTema();
    });
    this.configurarDesacople();
    this.configurarCierreVentanas();
    this.consola.setOnLine((line) => {
      void api.sendInput(line);
      this.setModo("corriendo", t("estado.corriendo"));
    });
    void this.suscribirEventos();
    this.restaurarWorkspace(); // RP27: retomar el buffer de la sesión anterior
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
    bar.appendChild(this.sep());
    // Sección de archivo en un MENÚ DESPLEGABLE «Archivo ⌄» (RP23): Nuevo ·
    // Abrir · Guardar · Guardar como. La lógica de cada acción no cambia, solo
    // se agrupa para no llenar la barra de botones sueltos.
    const btnArchivo = boton("archivo", "toolbar.archivo", "deemph");
    btnArchivo.insertAdjacentHTML("beforeend", '<span class="caret" aria-hidden="true">⌄</span>');
    btnArchivo.setAttribute("aria-haspopup", "menu");
    btnArchivo.setAttribute("aria-expanded", "false");
    btnArchivo.onclick = () => this.alternarMenuArchivo(btnArchivo);
    bar.appendChild(this.sep());
    // Exportar ELF queda aparte: produce un binario, no guarda el fuente.
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

    // Selector de arquitectura RV32/RV64 (RP32): persiste y aplica al PRÓXIMO
    // ensamblado (no recarga ni reensambla, así el usuario puede cambiarlo sin
    // perder la sesión actual). RV32 (default histórico) va primero.
    const arq = document.createElement("select");
    arq.id = "arquitectura";
    arq.title = t("toolbar.arquitectura");
    for (const valor of ["rv32", "rv64"] as const) {
      const o = document.createElement("option");
      o.value = valor;
      o.textContent = t(`arquitectura.${valor}`);
      if (valor === leerXlen()) o.selected = true;
      arq.appendChild(o);
    }
    arq.onchange = () => {
      setXlen(arq.value === "rv64" ? "rv64" : "rv32");
    };
    bar.appendChild(arq);

    // Editor de tema de usuario (RP28): botón que abre el modal «Editar
    // tema». Guardar crea/actualiza el tema «Personalizado» y lo selecciona.
    const btnEditarTema = boton("tema-editar", "tema.editar");
    btnEditarTema.onclick = () => this.editorTema.abrir(btnEditarTema);
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
    this.agregarDesacople(
      "memoria",
      "panel-memoria",
      "panel.memoria",
      560,
      460,
      () => void this.memoria.refrescar(),
    );
    // Depuración (el delicado): al desacoplar, main DELEGA el envío de
    // breakpoints a la ventana (que tiene las condiciones) y le manda la
    // lista; al reacoplar, re-sincroniza desde el editor y reasume el envío.
    this.agregarDesacople(
      "depuracion",
      "panel-depuracion",
      "panel.depuracion",
      460,
      460,
      () => {
        this.depDesacoplada = false;
        this.sincronizarPanelDepuracion();
        this.enviarBreakpoints();
      },
      () => {
        this.depDesacoplada = true;
        this.emitirDepFilas();
      },
    );
    this.configurarSyncDepuracion();
  }

  /** Construye la lista de breakpoints (línea+addr+condición) y la emite a la
   *  ventana de Depuración desacoplada (RP22-F3). */
  private emitirDepFilas(): void {
    const conAddr = this.programa !== null && !this.programaDesactualizado;
    const filas: FilaConCond[] = getBreakpointLines(this.editor).map((linea) => ({
      linea,
      addr: conAddr ? (this.programa!.line_to_addr[String(linea)] ?? null) : null,
      cond: this.depuracion.condicionDe(linea) ?? "",
    }));
    depSync.emitFilas(filas);
  }

  /** Escucha las acciones de la ventana de Depuración (RP22-F3): pedir el
   *  estado al abrir, quitar un breakpoint (vuelve al editor) y absorber las
   *  condiciones editadas (para no perderlas al reacoplar). Una sola vez. */
  private configurarSyncDepuracion(): void {
    void depSync.onPedir(() => this.emitirDepFilas());
    void depSync.onQuitar((linea) => {
      toggleBreakpointAtLine(this.editor, linea);
      this.alCambiarBreakpoints();
    });
    void depSync.onConds((conds) => {
      for (const c of conds) this.depuracion.setCondicionExterna(c.linea, c.cond);
    });
  }

  /** Cierra las ventanas HIJAS al cerrar la principal (RP23-F1): los paneles
   *  desacoplados (panel-*) y la pantalla quedaban huérfanos. Se toma el
   *  control del cierre (preventDefault), se cierran las hijas por su label
   *  conocida y se destruye la principal — `destroy()` NO re-dispara este
   *  handler, así que no hay bucle. Todo frontend (RP-22 también lo fue). */
  private configurarCierreVentanas(): void {
    const principal = getCurrentWindow();
    const hijas = ["panel-registros", "panel-memoria", "panel-depuracion", "pantalla"];
    void principal.onCloseRequested(async (evento) => {
      evento.preventDefault();
      // RP25/RP30: avisar si se cierra con cambios sin guardar en CUALQUIER
      // pestaña (la ventana principal arrastra a las hijas, así que el aviso va
      // una sola vez, acá).
      if (this.hayAlgunCambioSinGuardar() && !confirm(t("archivo.cerrarConfirm"))) return;
      for (const label of hijas) {
        const w = await WebviewWindow.getByLabel(label);
        if (w) await w.close().catch(() => {});
      }
      await principal.destroy();
    });
  }

  private agregarDesacople(
    id: string,
    seccionId: string,
    claveTitulo: string,
    width: number,
    height: number,
    alReacoplar: () => void,
    alDesacoplar?: () => void,
  ): void {
    const h2 = document.querySelector<HTMLElement>(`#${seccionId} h2`);
    if (!h2) return;
    h2.appendChild(
      botonDesacople(t("desacople.titulo"), () => {
        void desacoplarPanel({
          id,
          seccionId,
          titulo: t(claveTitulo),
          width,
          height,
          alReacoplar,
          alDesacoplar,
        });
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
    // El programa disparó un tono: reproducirlo (RP31). Apagado por defecto:
    // solo llega este evento si el programa usó el dispositivo de sonido.
    this.unlisteners.push(await events.onSonido((ev) => this.audio.reproducir(ev)));
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
      const ok = await api.assemble(getDoc(this.editor), this.rutaArchivo, leerXlen());
      this.programa = ok;
      this.programaDesactualizado = false;
      this.sesionDocId = this.doc.id; // este documento pasa a ser dueño de la sesión
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
      panelSync.notificar(); // ventanas desacopladas: re-sincronizar al nuevo programa
    } catch (e) {
      const errores = e as AsmError[];
      this.programa = null;
      this.sesionDocId = null; // ensamblado fallido: ningún documento corre
      this.memoria.setPrograma(null);
      panelSync.notificar(); // ventanas desacopladas: limpiar (sin programa válido)
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
    // RP22-F3: si Depuración está en su ventana, ella tiene las condiciones y
    // es quien manda set_breakpoints (main solo le pasa la lista); así no se
    // pisan las condiciones. Acoplada, main envía normalmente.
    if (this.depDesacoplada) this.emitirDepFilas();
    else this.enviarBreakpoints();
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
    panelSync.notificar(); // ventanas desacopladas: re-sincronizar al estado reiniciado
  }

  /** Carga una lección normal al editor (buffer sin ruta, como un ejemplo).
   *  La marca como completada al abrirla (progreso persistido, RP15). */
  private abrirLeccion(l: Leccion): void {
    this.salirModoBinario();
    this.setDocLimpio(l.codigo);
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
      this.setDocLimpio(archivos["main.s"]);
      this.programa = null;
      this.rutaArchivo = mainPath; // base_dir para .include (RP5)
      this.setModo("sin_programa", t("leccion.multiarchivoListo", { carpeta }));
      this.lecciones.marcarCompletada(l.numero);
    } catch (e) {
      this.consola.append(`${t("archivo.errorGuardar", { message: String(e) })}\n`, "error");
    }
  }

  /** Abre un archivo del usuario en una PESTAÑA NUEVA (RP30-F3); si ya hay una
   *  pestaña con esa ruta, la enfoca en vez de duplicarla. Un fuente abre una
   *  pestaña editable; un ELF (detección por magic en el backend, RP11) entra
   *  en modo binario con el listado desensamblado, solo lectura. */
  private async abrirArchivo(): Promise<void> {
    if (this.estaVivo()) return;
    const { open } = await import("@tauri-apps/plugin-dialog");
    const path = await open({
      title: t("archivo.tituloAbrir"),
      filters: [{ name: t("archivo.filtro"), extensions: ["asm", "s", "elf"] }],
    });
    if (!path) return;
    // Si ya está abierto (misma ruta), enfocar esa pestaña en vez de duplicar.
    const ya = this.documentos.findIndex((d) => d.rutaArchivo === path);
    if (ya >= 0) {
      this.activarPestana(ya);
      return;
    }
    try {
      const apertura = await api.abrirPrograma(path);
      if (apertura.tipo === "fuente") {
        this.crearYActivarPestana(apertura.contenido);
        this.rutaArchivo = path; // base para .include (RP5)
        this.consola.clear();
        this.actualizarChromeBinario();
        this.sincronizarPanelDepuracion();
        this.setModo("sin_programa", t("archivo.abierto", { path }));
      } else {
        this.crearYActivarPestana(""); // entrarModoBinario llena el listado
        await this.entrarModoBinario(apertura.ok, apertura.listado, path);
      }
      this.programarGuardadoWorkspace();
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
    this.setDocLimpio(listado);
    setReadOnly(this.editor, true);
    this.modoBinario = true;
    this.programa = ok;
    this.programaDesactualizado = false;
    this.rutaArchivo = null;
    this.sesionDocId = this.doc.id; // la sesión de backend es de este documento
    this.botones["ensamblar"].disabled = true;
    this.actualizarChromeBinario(); // banner del modo binario (chrome compartido)
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
    this.setDocLimpio(EJEMPLOS[i].codigo);
    this.programa = null;
    this.rutaArchivo = null; // ejemplo embebido: sin base de .include
    this.setModo("sin_programa", t("estado.sin_programa"));
  }

  /** Carga `contenido` al editor y lo marca como «limpio» (base de la
   *  detección de cambios sin guardar de RP23). */
  private setDocLimpio(contenido: string): void {
    setDoc(this.editor, contenido);
    this.docLimpio = contenido;
  }

  /** ¿El buffer ACTIVO tiene cambios sin guardar? (RP23) En modo binario nunca:
   *  el listado desensamblado es solo lectura. Lo usa el título de la ventana. */
  private hayCambiosSinGuardar(): boolean {
    return !this.modoBinario && getDoc(this.editor) !== this.docLimpio;
  }

  /** ¿ALGUNA pestaña tiene cambios sin guardar? (RP30) Lo usa el aviso al cerrar
   *  la ventana: con varias pestañas, avisa si cualquiera está sucia. */
  private hayAlgunCambioSinGuardar(): boolean {
    return this.documentos.some((d, i) => this.docSucio(d, i === this.activaIdx));
  }

  /** RP27: retoma el buffer de la sesión anterior (contenido + ruta). Si no hay
   *  nada persistido, el editor queda con su default («Hola mundo»). No restaura
   *  modo binario: solo fuentes editables. */
  private restaurarWorkspace(): void {
    const w = leerWorkspace();
    if (!w) return;
    this.setDocLimpio(w.contenido);
    this.rutaArchivo = w.rutaArchivo;
  }

  /** RP27: persiste el buffer actual (no en modo binario: el desensamblado es
   *  solo lectura y no debe pisar el último fuente). */
  private persistirWorkspace(): void {
    if (this.modoBinario) return;
    guardarWorkspace({ contenido: getDoc(this.editor), rutaArchivo: this.rutaArchivo });
  }

  /** RP27: persistencia con debounce para no escribir localStorage por tecla. */
  private programarGuardadoWorkspace(): void {
    if (this.timerWorkspace !== undefined) clearTimeout(this.timerWorkspace);
    this.timerWorkspace = window.setTimeout(() => this.persistirWorkspace(), 400);
  }

  /** RP25: el título de la ventana refleja el archivo actual y antepone «•»
   *  cuando hay cambios sin guardar (convención de editor/IDE). Se invoca al
   *  cargar/guardar (vía setModo y escribirArchivo) y en cada tecla. */
  private actualizarTitulo(): void {
    const nombre = nombreDocumento(this.rutaArchivo, t("titulo.sinTitulo"));
    const punto = this.hayCambiosSinGuardar() ? "• " : "";
    void getCurrentWindow().setTitle(`${punto}${nombre} — EmuRV`);
    this.refrescarBarra(); // RP30: el «•»/nombre de la pestaña activa también
  }

  // ----- pestañas (RP30) -----

  /** ¿La ejecución está viva? (corriendo o esperando entrada). Con ejecución
   *  viva NO se cambia/abre/cierra pestaña (la sesión de backend es única y
   *  sigue a la activa; ver BITACORA RP30-F1). */
  private estaVivo(): boolean {
    return this.modoActual === "corriendo" || this.modoActual === "esperando";
  }

  /** ¿El programa del documento activo corresponde a la sesión de backend viva?
   *  (RP30) Solo entonces se puede Ejecutar/Paso/Reiniciar/Exportar sin volver
   *  a ensamblar. Con una sola pestaña es siempre `this.programa !== null`
   *  (la activa es siempre la dueña), así que la paridad single-tab se conserva. */
  private programaVigente(): boolean {
    return this.programa !== null && this.sesionDocId === this.doc.id;
  }

  /** ¿El documento tiene cambios sin guardar? Para la activa, la fuente de
   *  verdad es el editor; para una inactiva, su `estado` guardado. (RP25 por
   *  pestaña). En modo binario nunca (el listado es solo lectura). */
  private docSucio(doc: Documento, esActiva: boolean): boolean {
    if (doc.modoBinario) return false;
    const contenido = esActiva ? getDoc(this.editor) : doc.estado.doc.toString();
    return contenido !== doc.docLimpio;
  }

  /** Repinta la barra con el estado actual de los documentos. */
  private refrescarBarra(): void {
    if (!this.barra) return; // aún no construida (primeras llamadas del ctor)
    const sinTitulo = t("titulo.sinTitulo");
    this.barra.render(
      this.documentos.map((d, i) => ({
        id: d.id,
        nombre: nombreDocumento(d.rutaArchivo, sinTitulo),
        sucio: this.docSucio(d, i === this.activaIdx),
        cerrable: this.documentos.length > 1,
      })),
      this.activaIdx,
    );
  }

  /** Vuelca el estado vivo del editor (estado + scroll) al documento activo,
   *  antes de intercambiarlo por otro (setState resetea el scroll). */
  private guardarEstadoActivo(): void {
    const a = this.doc;
    a.estado = this.editor.state;
    a.scrollTop = this.editor.scrollDOM.scrollTop;
  }

  /** Carga el `estado` de un documento en la única `EditorView` y restaura su
   *  scroll. NO toca el backend (la sesión sigue a quien ensambló). */
  private cargarEstado(doc: Documento): void {
    this.editor.setState(doc.estado);
    this.editor.scrollDOM.scrollTop = doc.scrollTop;
  }

  /** Muestra/oculta el banner del modo binario (chrome compartido) según el
   *  modo del documento activo. */
  private actualizarChromeBinario(): void {
    if (this.modoBinario) {
      if (!this.bannerBinario) {
        this.bannerBinario = document.createElement("div");
        this.bannerBinario.id = "binario-banner";
        const editorEl = document.getElementById("editor")!;
        editorEl.parentElement!.insertBefore(this.bannerBinario, editorEl);
      }
      this.bannerBinario.textContent = t("binario.banner");
      this.bannerBinario.hidden = false;
    } else if (this.bannerBinario) {
      this.bannerBinario.hidden = true;
    }
  }

  /** Sincroniza el chrome dependiente del documento activo tras un swap:
   *  banner binario, panel de Depuración y modo/estado. readOnly, breakpoints,
   *  errores y línea actual ya viajan en el `EditorState`. La sesión de backend
   *  NO se toca: si el documento entrante tiene `programa` pero no es el dueño
   *  de la sesión viva, se le pide re-ensamblar para ejecutarlo (RP30-F3). */
  private sincronizarUIactiva(): void {
    this.actualizarChromeBinario();
    this.sincronizarPanelDepuracion(); // breakpoints del documento activo
    if (this.programaVigente()) {
      this.setModo("listo", t("estado.ensamblado", { n: this.programa!.text_len }), "ok");
    } else if (this.programa) {
      // Tiene un programa propio, pero la sesión de backend es de otra pestaña.
      this.setModo("sin_programa", t("pestana.reensamblar"));
    } else {
      this.setModo("sin_programa", t("estado.sin_programa"));
    }
  }

  /** Crea un documento alrededor de `contenido`, lo agrega y lo activa,
   *  preservando el estado de la pestaña saliente. NO sincroniza la UI: el
   *  llamante decide el modo/mensaje (cada flujo es distinto). */
  private crearYActivarPestana(contenido: string): Documento {
    this.guardarEstadoActivo();
    const estado = crearEstadoEditor(contenido, this.editorCbs);
    const doc = crearDocumento(estado, { docLimpio: contenido });
    this.documentos.push(doc);
    this.activaIdx = this.documentos.length - 1;
    this.cargarEstado(doc);
    return doc;
  }

  /** Trae al frente la pestaña `idx` preservando el estado de la saliente. */
  private activarPestana(idx: number): void {
    if (idx === this.activaIdx || idx < 0 || idx >= this.documentos.length) return;
    if (this.estaVivo()) return; // no cambiar con ejecución viva
    this.guardarEstadoActivo();
    this.activaIdx = idx;
    this.cargarEstado(this.doc);
    this.sincronizarUIactiva();
  }

  /** Abre una pestaña nueva en blanco (plantilla mínima) y la activa. La usan
   *  «+» y «Nuevo» (RP30-F3: ambos abren pestaña en blanco). */
  private nuevaPestana(): void {
    if (this.estaVivo()) return;
    this.crearYActivarPestana(App.PLANTILLA_NUEVA);
    this.consola.clear();
    this.actualizarChromeBinario(); // doc nuevo: oculta el banner si venía de binario
    this.sincronizarPanelDepuracion();
    this.setModo("sin_programa", t("archivo.nuevo"));
    this.programarGuardadoWorkspace();
  }

  /** Cierra la pestaña `idx`. No cierra la última (siempre queda una). Avisa
   *  antes de descartar cambios sin guardar (RP25 «•», RP30-F3). */
  private cerrarPestana(idx: number): void {
    if (this.estaVivo()) return;
    if (this.documentos.length <= 1) return;
    if (idx < 0 || idx >= this.documentos.length) return;
    const cerrabaActiva = idx === this.activaIdx;
    if (this.docSucio(this.documentos[idx], cerrabaActiva) && !confirm(t("archivo.cerrarConfirm"))) {
      return;
    }
    const cerrado = this.documentos[idx];
    const total = this.documentos.length;
    this.documentos.splice(idx, 1);
    // Si el documento cerrado era el dueño de la sesión, ya no hay dueño.
    if (this.sesionDocId === cerrado.id) this.sesionDocId = null;
    this.activaIdx = indiceActivoTrasCerrar(this.activaIdx, idx, total);
    if (cerrabaActiva) {
      // La activa cambió de documento: hay que cargar su estado en la vista.
      this.cargarEstado(this.doc);
      this.sincronizarUIactiva();
    } else {
      // La activa sigue siendo el mismo documento (su estado vivo está en la
      // vista, no se toca); solo se repinta la barra con el índice corregido.
      this.refrescarBarra();
    }
    this.programarGuardadoWorkspace();
  }

  /** Plantilla mínima de un programa nuevo (RP23): un esqueleto runnable
   *  que termina limpio, para que el principiante tenga de dónde partir. */
  private static readonly PLANTILLA_NUEVA =
    "# Programa nuevo en RISC-V (RV32)\n" +
    "        .text\n" +
    "main:\n" +
    "        li   a7, 10      # servicio 10: terminar\n" +
    "        ecall\n";

  /** Menú desplegable «Archivo» (RP23): popover anclado al botón con Nuevo /
   *  Abrir / Guardar / Guardar como. Toggle (si está abierto, cierra); cierra
   *  al click afuera o Escape. La lógica de cada acción vive en sus métodos. */
  private alternarMenuArchivo(ancla: HTMLElement): void {
    const previo = document.getElementById("menu-archivo");
    if (previo) {
      previo.remove();
      ancla.setAttribute("aria-expanded", "false");
      return;
    }
    const items: { id: string; clave: string; accion: () => void }[] = [
      { id: "nuevo", clave: "toolbar.nuevo", accion: () => this.nuevoArchivo() },
      { id: "abrir", clave: "toolbar.abrir", accion: () => void this.abrirArchivo() },
      { id: "guardar", clave: "toolbar.guardar", accion: () => void this.guardarArchivo() },
      { id: "guardarComo", clave: "toolbar.guardarComo", accion: () => void this.guardarComo() },
    ];
    const menu = document.createElement("div");
    menu.id = "menu-archivo";
    menu.className = "menu-toolbar";
    menu.setAttribute("role", "menu");

    let cerrar = (): void => {};
    const afuera = (e: MouseEvent): void => {
      const t2 = e.target as Node;
      if (!menu.contains(t2) && !ancla.contains(t2)) cerrar();
    };
    const esc = (e: KeyboardEvent): void => {
      if (e.key === "Escape") cerrar();
    };
    cerrar = (): void => {
      menu.remove();
      ancla.setAttribute("aria-expanded", "false");
      document.removeEventListener("click", afuera, true);
      document.removeEventListener("keydown", esc);
    };

    for (const it of items) {
      const b = document.createElement("button");
      b.type = "button";
      b.className = "menu-item";
      b.setAttribute("role", "menuitem");
      b.innerHTML = `${icono(it.id)}<span>${t(it.clave)}</span>`;
      b.onclick = () => {
        cerrar();
        it.accion();
      };
      menu.appendChild(b);
    }

    document.body.appendChild(menu);
    const r = ancla.getBoundingClientRect();
    menu.style.top = `${r.bottom + 4}px`;
    menu.style.left = `${r.left}px`;
    ancla.setAttribute("aria-expanded", "true");
    // registrar los cierres tras este click (si no, el mismo click lo cerraría)
    setTimeout(() => {
      document.addEventListener("click", afuera, true);
      document.addEventListener("keydown", esc);
    }, 0);
    menu.querySelector<HTMLButtonElement>("button")?.focus();
  }

  /** «Nuevo» (RP23 → RP30-F3): abre una pestaña EN BLANCO (plantilla mínima),
   *  igual que «+». Ya no descarta el buffer actual (queda en su pestaña), así
   *  que no necesita confirmar. */
  private nuevoArchivo(): void {
    this.nuevaPestana();
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

  /** «Guardar» INTELIGENTE (RP23): si ya hay una ruta conocida, guarda ahí
   *  directo (sin re-preguntar, como un IDE); si no, se comporta como
   *  «Guardar como». */
  private async guardarArchivo(): Promise<void> {
    if (this.rutaArchivo) await this.escribirArchivo(this.rutaArchivo);
    else await this.guardarComo();
  }

  /** «Guardar como» (RP23): SIEMPRE pide la ruta con el diálogo; al confirmar
   *  guarda y actualiza la ruta actual (base de .include). */
  private async guardarComo(): Promise<void> {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const path = await save({
      title: t("archivo.tituloGuardar"),
      defaultPath: this.rutaArchivo ?? "programa.asm",
      filters: [{ name: t("archivo.filtro"), extensions: ["asm", "s"] }],
    });
    if (!path) return;
    await this.escribirArchivo(path);
  }

  /** Escribe el buffer en `path`: actualiza la ruta actual (base de .include,
   *  RP5), marca el buffer como «limpio» y avisa en la barra de estado. */
  private async escribirArchivo(path: string): Promise<void> {
    try {
      await api.guardarArchivo(path, getDoc(this.editor));
      this.rutaArchivo = path;
      this.docLimpio = getDoc(this.editor); // recién guardado: limpio
      this.actualizarTitulo(); // RP25: quita «•» y muestra el nombre guardado
      this.persistirWorkspace(); // RP27: persistir ruta + contenido recién guardados
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
    this.modoActual = modo; // RP30: para bloquear cambio de pestaña con ejecución viva
    this.estadoEl.textContent = mensaje;
    this.estadoEl.className = clase;
    const b = this.botones;
    // RP30: Ejecutar/Paso/Reiniciar/Exportar exigen que el programa del
    // documento activo SEA el de la sesión de backend viva (con una sola
    // pestaña, equivale a `programa !== null`: la activa es siempre la dueña).
    const puedeCorrer = this.programaVigente();
    const parado = modo === "listo" || modo === "terminado";
    b.ensamblar.disabled = modo === "corriendo" || modo === "esperando";
    b.ejecutar.disabled = !puedeCorrer || !parado || modo === "terminado";
    b.paso.disabled = !puedeCorrer || !parado || modo === "terminado";
    b.detener.disabled = modo !== "corriendo" && modo !== "esperando";
    b.reiniciar.disabled = !puedeCorrer || modo === "corriendo" || modo === "esperando";
    b.exportar.disabled = !puedeCorrer || modo === "corriendo" || modo === "esperando";
    // Menú Archivo (Nuevo/Abrir/Guardar/Guardar como): se deshabilita mientras
    // la ejecución está viva — Abrir reemplazaría el editor y Guardar leería un
    // estado a medias (mismo criterio que Exportar). [Fix: antes referenciaba
    // b.abrir/b.guardar, que dejaron de existir al agrupar el menú → setModo
    // lanzaba TypeError y la ejecución nunca arrancaba.]
    b.archivo.disabled = modo === "corriendo" || modo === "esperando";
    // RP30: la barra de pestañas se bloquea con ejecución viva (no cambiar/
    // abrir/cerrar pestaña mientras corre: la sesión es única y sigue a la activa).
    document.getElementById("pestanas")?.classList.toggle("bloqueada", this.estaVivo());
    this.actualizarTitulo(); // RP25: refresca «•» y nombre tras cargar/cambiar de estado
    this.persistirWorkspace(); // RP27: cada carga/cambio de estado deja el buffer persistido
  }
}

window.addEventListener("DOMContentLoaded", () => {
  inicializarTema(); // aplica el tema guardado antes de construir la UI (RP17)
  new App();
});
