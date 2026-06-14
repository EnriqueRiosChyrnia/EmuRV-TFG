// Host genérico de un panel desacoplado a su propia ventana (RP22). Mismo
// patrón que pantalla.ts, pero parametrizado por `?panel=registros|memoria|
// depuracion`: instancia la MISMA clase TS del panel (sin duplicar lógica),
// se suscribe a los eventos del backend y se refresca solo. La data y la
// edición usan los comandos que ya existen (get_state/read_mem/write_*).
//
// Tema e idioma: la ventana lee el tema (localStorage emurv.tema.v1) y el
// idioma (emurv.lang.v1, vía el módulo i18n que se autocarga) AL ABRIR, y
// los propaga en vivo si la principal los cambia (evento `storage`, mismo
// origen — best-effort; al abrir siempre es correcto).
import { inicializarTema } from "./temas";
import { t } from "./i18n";
import { api, events, type MachineState, type AssembleOk } from "./api";
import { Registros } from "./paneles/registros";
import { Memoria } from "./paneles/memoria";
import { Depuracion } from "./paneles/depuracion";
import { depSync, type FilaConCond } from "./paneles/dep-sync";
import { panelSync } from "./paneles/panel-sync";

inicializarTema(); // data-tema desde localStorage (emurv.tema.v1)

const which = new URLSearchParams(location.search).get("panel") ?? "registros";
const CLAVE_TITULO: Record<string, string> = {
  registros: "panel.registros",
  memoria: "panel.memoria",
  depuracion: "panel.depuracion",
};
const claveTitulo = CLAVE_TITULO[which] ?? "panel.registros";

// Armar el contenedor del panel (mismo markup que el lateral: .panel > h2 +
// .panel-cuerpo) para que el CSS de los paneles aplique igual.
const host = document.getElementById("panel-host")!;
const seccion = document.createElement("section");
seccion.className = "panel";
seccion.innerHTML = `<h2>${t(claveTitulo)}</h2><div class="panel-cuerpo"></div>`;
const cuerpo = seccion.querySelector<HTMLElement>(".panel-cuerpo")!;
host.appendChild(seccion);
document.title = `EmuRV — ${t(claveTitulo)}`;

if (which === "registros") {
  // La MISMA clase Registros del lateral, en este documento. Se refresca por
  // emu://registros (durante la ejecución) y emu://detenido (al pausar), y al
  // abrir pide el estado vigente. Editar (doble-click) usa write_reg.
  const reg = new Registros(cuerpo);
  const pintar = (st: MachineState): void => reg.actualizar(st);
  void events.onRegisters(pintar);
  void events.onStopped((ev) => reg.actualizar(ev.state));
  // Ensamblar/Reiniciar reinician el estado sin emitir emu://detenido: la
  // ventana se re-sincroniza con la señal de panelSync. (Fix post-RP22.)
  void panelSync.onResync(() => void api.getState().then(pintar).catch(() => {}));
  void api.getState().then(pintar).catch(() => {
    // sin sesión de emulación todavía: la tabla queda en «—»
  });
}

if (which === "memoria") {
  // La MISMA clase Memoria, con read_mem/write_mem y navegación. Para la
  // columna de instrucción desensamblada (RP11) la clase necesita el
  // «programa» (rango .text): se sintetiza de las claves de get_disasm
  // (que SON las direcciones de .text), con data_base/sp_init estándar —
  // así no hace falta un comando nuevo ni tocar la clase.
  const mem = new Memoria(cuerpo);
  const cargarDisasm = async (): Promise<void> => {
    try {
      const d = await api.getDisasm();
      const addrs = Object.keys(d).map(Number).filter((n) => !Number.isNaN(n));
      if (addrs.length > 0) {
        const tb = addrs.reduce((a, b) => Math.min(a, b));
        const tlen = addrs.reduce((a, b) => Math.max(a, b)) - tb + 4;
        const prog: AssembleOk = {
          text_base: tb,
          text_len: tlen,
          data_base: 0x1001_0000,
          sp_init: 0x7fff_fff0,
          data_len: 0,
          symbols: {},
          line_to_addr: {},
          addr_to_line: {},
        };
        mem.setPrograma(prog);
        mem.setDisasm(d); // setDisasm ya refresca
      } else {
        void mem.refrescar();
      }
    } catch {
      void mem.refrescar(); // sin sesión: dump en blanco / «—»
    }
  };
  void cargarDisasm();
  // Re-sincroniza el programa + relee la memoria. cargarDisasm() (no solo
  // refrescar()) porque si la ventana se abrió sin programa, this.programa
  // quedaba null y refrescar() dejaba la memoria en blanco. (Fix post-RP22.)
  // - onStopped: paradas de ejecución (pausa/paso/breakpoint/fin).
  // - panelSync: cambios FUERA de ejecución (Ensamblar/Reiniciar), que NO
  //   emiten emu://detenido — era el caso del bug: ensamblar con la ventana
  //   desacoplada no actualizaba la memoria hasta reacoplar.
  void events.onStopped(() => void cargarDisasm());
  void panelSync.onResync(() => void cargarDisasm());
}

if (which === "depuracion") {
  // El delicado (RP22-F3). Los breakpoints se CREAN en el gutter del editor
  // (ventana principal); acá vive la LISTA. main manda las filas por evento
  // (app://dep-filas); esta ventana, que tiene las condiciones, es la que
  // ENVÍA set_breakpoints (así no se pisan). Quitar y condiciones vuelven a
  // main por eventos. Los watchpoints los maneja la clase (set_watchpoints).
  let filas: FilaConCond[] = [];
  let dep: Depuracion;
  const enviar = (): void => {
    const bps = filas
      .filter((f) => f.addr !== null)
      .map((f) => ({ addr: f.addr as number, cond: dep.condicionDe(f.linea) }));
    api
      .setBreakpoints(bps)
      .then(() => dep.limpiarError())
      .catch((e: unknown) => dep.mostrarError(String(e)));
  };
  dep = new Depuracion(cuerpo, {
    onCondicionCambiada: () => {
      enviar(); // aplicar al backend con las condiciones de ESTA ventana
      // y reportarlas a main para que las absorba (no se pierden al reacoplar)
      depSync.emitConds(filas.map((f) => ({ linea: f.linea, cond: dep.condicionDe(f.linea) ?? "" })));
    },
    onQuitarBreakpoint: (linea) => depSync.quitar(linea), // main lo saca del editor
  });
  void depSync.onFilas((nuevas) => {
    filas = nuevas;
    dep.setFilasBreakpoints(nuevas.map((f) => ({ linea: f.linea, addr: f.addr })));
    for (const f of nuevas) dep.setCondicionExterna(f.linea, f.cond);
    enviar(); // un breakpoint nuevo del editor (estando desacoplado) se aplica
  });
  depSync.pedir(); // handshake: pedir a main el estado actual al abrir
}

// Propagación en vivo de tema/idioma (best-effort, mismo origen): si la
// ventana principal cambia el tema, se re-aplica sin recargar; si cambia el
// idioma, se recarga (la UI se arma con t() al construirse).
window.addEventListener("storage", (e) => {
  if (e.key === "emurv.tema.v1") inicializarTema();
  if (e.key === "emurv.lang.v1") location.reload();
});
