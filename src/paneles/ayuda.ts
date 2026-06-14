// Ayuda integrada (RF-07/RF-09), rediseñada a un modal de 2 columnas
// (RP20): índice agrupado (ay-nav) + cuerpo scrolleable (ay-cont) con
// scroll-spy. El CONTENIDO sale de las mismas claves i18n `ayuda.*` de
// siempre (RP-20 reorganiza la PRESENTACIÓN, no reescribe textos). Usa el
// shell común de modales (modal.ts) para overlay/Escape/foco.
import { t } from "../i18n";
import { Modal, ICONO_CERRAR } from "./modal";
import { seccionActiva, type SeccionPos } from "./scroll-spy";
// Wordmark bi-tono (RP19) reusado en el «Acerca de» (se adapta al tema).
import wordmark from "../assets/emurv-wordmark.svg?raw";

// Filas de la tabla de syscalls: a7, clave i18n del servicio, argumentos
// (notación técnica, igual que antes — no es texto traducible).
const SYSCALLS: Array<[string, string, string]> = [
  ["1", "ayuda.syscalls.s1", "a0 = valor"],
  ["2", "ayuda.syscalls.s2", "fa0 = valor (f32)"],
  ["3", "ayuda.syscalls.s3", "fa0 = valor (f64)"],
  ["4", "ayuda.syscalls.s4", "a0 = dirección (NUL)"],
  ["5", "ayuda.syscalls.s5", "→ a0 = valor leído"],
  ["6", "ayuda.syscalls.s6", "→ fa0 (f32)"],
  ["7", "ayuda.syscalls.s7", "→ fa0 (f64)"],
  ["8", "ayuda.syscalls.s8", "a0 = buffer, a1 = tamaño"],
  ["9", "ayuda.syscalls.s9", "a0 = bytes → a0 = dir"],
  ["10", "ayuda.syscalls.s10", "—"],
  ["11", "ayuda.syscalls.s11", "a0 = código de carácter"],
  ["30", "ayuda.syscalls.s30", "→ a0 bajos, a1 altos"],
  ["32", "ayuda.syscalls.s32", "a0 = milisegundos"],
  ["60", "ayuda.syscalls.s60", "—"],
  ["93", "ayuda.syscalls.s93", "a0 = código de salida"],
];

const DIRECTIVAS = [
  ".text", ".data", ".word", ".half", ".byte", ".ascii", ".asciiz", ".string",
  ".space", ".align", ".globl", ".include", ".eqv", ".equ", ".macro", ".end_macro",
];

// Índice: grupos (clave i18n del título) → secciones (id, clave del rótulo).
const NAV: Array<{ grupo: string; items: Array<[string, string]> }> = [
  { grupo: "ayuda.grupo.empezar", items: [["ay-ciclo", "ayuda.ciclo.titulo"], ["ay-ejemplos", "ayuda.ejemplos.titulo"]] },
  {
    grupo: "ayuda.grupo.referencia",
    items: [
      ["ay-syscalls", "ayuda.syscalls.titulo"],
      ["ay-memoria", "ayuda.memoria.titulo"],
      ["ay-teclado", "ayuda.teclado.titulo"],
      ["ay-directivas", "ayuda.directivas.titulo"],
    ],
  },
  {
    grupo: "ayuda.grupo.herramientas",
    items: [
      ["ay-editor", "ayuda.editor.titulo"],
      ["ay-depuracion", "ayuda.depuracion.titulo"],
      ["ay-idioma", "ayuda.idiomaTemas.titulo"],
    ],
  },
  { grupo: "ayuda.grupo.proyecto", items: [["ay-about", "ayuda.about.titulo"]] },
];

export class Ayuda {
  private modal: Modal;
  private nav!: HTMLElement;
  private cont!: HTMLElement;

  constructor() {
    this.modal = new Modal("modal-cuerpo-ayuda");
    this.modal.card.innerHTML = `
      <header class="modal-head">
        <h2>${t("ayuda.titulo")}<span class="mh-sub">${t("ayuda.subtitulo")}</span></h2>
        <button class="modal-x" aria-label="${t("ayuda.cerrar")}">${ICONO_CERRAR}</button>
      </header>
      <div class="ay-body">
        <nav class="ay-nav">${this.navHTML()}</nav>
        <div class="ay-cont">${this.contHTML()}</div>
      </div>`;
    this.nav = this.modal.card.querySelector(".ay-nav")!;
    this.cont = this.modal.card.querySelector(".ay-cont")!;

    // Click en el índice → scroll suave a la sección + marcar activa.
    this.nav.addEventListener("click", (e) => {
      const btn = (e.target as HTMLElement).closest<HTMLElement>("button[data-sec]");
      if (!btn) return;
      const dest = this.cont.querySelector<HTMLElement>(`#${btn.dataset.sec}`);
      if (dest) this.cont.scrollTo({ top: dest.offsetTop - this.cont.offsetTop - 6, behavior: "smooth" });
      this.marcarActiva(btn.dataset.sec!);
    });
    // Scroll a mano → actualizar la activa (scroll-spy).
    this.cont.addEventListener("scroll", () => {
      const secs: SeccionPos[] = [...this.cont.querySelectorAll<HTMLElement>("section[id]")].map((s) => ({
        id: s.id,
        top: s.offsetTop - this.cont.offsetTop,
      }));
      const id = seccionActiva(this.cont.scrollTop, secs);
      if (id) this.marcarActiva(id);
    });
  }

  private marcarActiva(sec: string): void {
    this.nav.querySelectorAll("button[data-sec]").forEach((b) => b.classList.remove("activo"));
    this.nav.querySelector(`button[data-sec="${sec}"]`)?.classList.add("activo");
  }

  private navHTML(): string {
    return NAV.map(
      (g, gi) =>
        `<div class="ay-nav-tit">${t(g.grupo)}</div>` +
        g.items
          .map(
            ([id, clave], i) =>
              `<button data-sec="${id}"${gi === 0 && i === 0 ? ' class="activo"' : ""}>${t(clave)}</button>`,
          )
          .join(""),
    ).join("");
  }

  private contHTML(): string {
    const syscallRows = SYSCALLS.map(
      ([a7, clave, args]) =>
        `<tr><td class="col-num"><span class="ay-num">${a7}</span></td><td class="ay-serv">${t(clave)}</td><td class="ay-arg">${args}</td></tr>`,
    ).join("");
    const chips = DIRECTIVAS.map((d) => `<span class="ay-chip">${d}</span>`).join("");

    return `
      <section id="ay-ciclo">
        <h3>${t("ayuda.ciclo.titulo")}</h3>
        <p>${t("ayuda.ciclo.texto")}</p>
        <p>${t("ayuda.lecciones")}</p>
        <p class="compacto">${t("ayuda.binario")}</p>
      </section>

      <section id="ay-ejemplos">
        <h3>${t("ayuda.ejemplos.titulo")}</h3>
        <p>${t("ayuda.ejemplos.texto")}</p>
      </section>

      <section id="ay-syscalls">
        <h3>${t("ayuda.syscalls.titulo")}</h3>
        <p class="compacto">${t("ayuda.syscalls.texto")}</p>
        <table class="ay-tabla">
          <thead><tr><th class="col-num">a7</th><th>${t("ayuda.syscalls.servicio")}</th><th>${t("ayuda.syscalls.args")}</th></tr></thead>
          <tbody>${syscallRows}</tbody>
        </table>
        <pre class="ayuda-ejemplo">    <span class="c-ins">li</span>   a7, <span class="c-num">4</span>        <span class="c-com"># servicio: imprimir cadena</span>
    <span class="c-ins">la</span>   a0, mensaje  <span class="c-com"># argumento</span>
    <span class="c-ins">ecall</span>             <span class="c-com"># llamada al sistema</span></pre>
        <div class="ay-nota"><span class="et">ebreak</span>${t("ayuda.ebreak")}</div>
        <div class="ay-nota"><span class="et">Avanzado</span>${t("ayuda.traps")}</div>
        <p class="compacto">${t("ayuda.flotantes")}</p>
        <p class="compacto">${t("ayuda.atomicas")}</p>
      </section>

      <section id="ay-memoria">
        <h3>${t("ayuda.memoria.titulo")}</h3>
        <table class="ay-tabla">
          <tbody>
            <tr><td><span class="ay-addr">0x00400000</span></td><td>${t("ayuda.memoria.text")}</td></tr>
            <tr><td><span class="ay-addr">0x10010000</span></td><td>${t("ayuda.memoria.data")}</td></tr>
            <tr><td><span class="ay-addr">0x7FFFFFF0</span></td><td>${t("ayuda.memoria.sp")}</td></tr>
          </tbody>
        </table>
        <p>${t("ayuda.memoria.buscar")}</p>
      </section>

      <section id="ay-teclado">
        <h3>${t("ayuda.teclado.titulo")}</h3>
        <p>${t("ayuda.teclado.texto")}</p>
        <table class="ay-tabla">
          <tbody>
            <tr><td><span class="ay-addr">0xFFFF0000</span></td><td>${t("ayuda.teclado.control")}</td></tr>
            <tr><td><span class="ay-addr">0xFFFF0004</span></td><td>${t("ayuda.teclado.dato")}</td></tr>
          </tbody>
        </table>
        <p class="compacto">${t("ayuda.teclado.codigos")}</p>
        <table class="ay-tabla">
          <thead><tr><th class="col-num">${t("ayuda.teclado.colCodigo")}</th><th>${t("ayuda.teclado.colTecla")}</th></tr></thead>
          <tbody>
            <tr><td class="ay-arg">32–126</td><td>${t("ayuda.teclado.ascii")}</td></tr>
            <tr><td class="ay-arg">10</td><td>Enter</td></tr>
            <tr><td class="ay-arg">27</td><td>Escape</td></tr>
            <tr><td class="ay-arg">8</td><td>Backspace</td></tr>
            <tr><td class="ay-arg">0x80–0x83</td><td>${t("ayuda.teclado.flechas")}</td></tr>
          </tbody>
        </table>
        <pre class="ayuda-ejemplo">espera:
    <span class="c-ins">li</span>   t0, <span class="c-num">0xFFFF0000</span>   <span class="c-com"># RECEIVER_CONTROL</span>
    <span class="c-ins">lw</span>   t1, <span class="c-num">0</span>(t0)        <span class="c-com"># bit 0 = hay tecla</span>
    <span class="c-ins">beqz</span> t1, espera       <span class="c-com"># nada todavía</span>
    <span class="c-ins">lw</span>   a0, <span class="c-num">4</span>(t0)        <span class="c-com"># leer consume la tecla</span></pre>
      </section>

      <section id="ay-directivas">
        <h3>${t("ayuda.directivas.titulo")}</h3>
        <div class="ay-chips">${chips}</div>
        <p class="compacto">${t("ayuda.directivas.rp5")}</p>
      </section>

      <section id="ay-editor">
        <h3>${t("ayuda.editor.titulo")}</h3>
        <p>${t("ayuda.editor.texto")}</p>
      </section>

      <section id="ay-depuracion">
        <h3>${t("ayuda.depuracion.titulo")}</h3>
        <p>${t("ayuda.breakpoints.texto")}</p>
        <p class="compacto">${t("ayuda.depuracion.texto")}</p>
      </section>

      <section id="ay-idioma">
        <h3>${t("ayuda.idiomaTemas.titulo")}</h3>
        <p>${t("ayuda.idioma.texto")}</p>
        <p class="compacto">${t("ayuda.temas.texto")}</p>
        <p class="compacto">${t("ayuda.temas.usuario")}</p>
      </section>

      <section id="ay-about">
        <h3>${t("ayuda.about.titulo")}</h3>
        <div class="ay-about-id">
          ${wordmark}
          <div class="tagline">Entorno educativo para programar en ensamblador RISC-V (RV32IMAFDC), en español. <span class="ay-ver">v0.1</span></div>
        </div>

        <div class="ay-autor">
          <div class="nombre">Enrique Emmanuel Ríos Chyrnia</div>
          <div class="rol">Estudiante de Ingeniería Informática · Paraguay</div>
        </div>

        <div class="ay-nota"><span class="et">IA</span>EmuRV fue desarrollado con asistencia de modelos Claude (Anthropic) sobre un contrato de especificación/arquitectura/plan, con verificación por pruebas automatizadas.</div>

        <div class="ay-cred-grupo">Núcleo y app de escritorio (Rust)</div>
        <ul class="ay-cred">
          <li><span class="nom"><b>Rust</b> <span class="desc">— lenguaje del núcleo</span></span></li>
          <li><span class="nom"><b>Tauri 2</b> <span class="desc">— app de escritorio nativa liviana</span></span><span class="ay-lic">MIT/Apache-2.0</span></li>
          <li><span class="nom"><b>Unicorn Engine 2.1.5</b> <span class="desc">— motor de emulación de CPU</span></span><span class="ay-lic">GPLv2</span></li>
          <li><span class="nom"><b>Capstone 0.14</b> <span class="desc">— desensamblador</span></span><span class="ay-lic">BSD-3-Clause</span></li>
          <li><span class="nom"><b>object 0.39</b> <span class="desc">— ejecutables ELF</span></span><span class="ay-lic">MIT/Apache-2.0</span></li>
          <li><span class="nom"><b>udbserver</b> <span class="desc">— servidor GDB remoto (usa gdbstub)</span></span><span class="ay-lic">MIT</span></li>
          <li><span class="nom"><b>serde · serde_json · thiserror</b> <span class="desc">— utilidades</span></span><span class="ay-lic">MIT/Apache-2.0</span></li>
        </ul>
        <div class="ay-nota"><span class="et">Licencia</span>La GPLv2 de Unicorn Engine condiciona la licencia del proyecto (decisión pendiente en el Cap. VI de la memoria).</div>

        <div class="ay-cred-grupo">Interfaz (TypeScript)</div>
        <ul class="ay-cred">
          <li><span class="nom"><b>TypeScript 5.6</b> <span class="desc">— lenguaje</span></span><span class="ay-lic">Apache-2.0</span></li>
          <li><span class="nom"><b>Vite 6</b> <span class="desc">— empaquetado</span></span><span class="ay-lic">MIT</span></li>
          <li><span class="nom"><b>CodeMirror 6</b> <span class="desc">— editor de código</span></span><span class="ay-lic">MIT</span></li>
          <li><span class="nom"><b>@tauri-apps/api</b> <span class="desc">— puente con el backend (+ plugins dialog/opener)</span></span><span class="ay-lic">MIT/Apache-2.0</span></li>
        </ul>

        <div class="ay-cred-grupo">Tipografía e iconos</div>
        <ul class="ay-cred">
          <li><span class="nom"><b>IBM Plex Sans</b> <span class="desc">— tipografía de interfaz</span></span><span class="ay-lic">OFL 1.1</span></li>
          <li><span class="nom"><b>JetBrains Mono</b> <span class="desc">— fuente monoespaciada</span></span><span class="ay-lic">OFL 1.1</span></li>
          <li><span class="nom"><b>Lucide</b> <span class="desc">— iconos SVG de la barra</span></span><span class="ay-lic">ISC</span></li>
        </ul>
      </section>`;
  }

  abrir(disparador?: HTMLElement | null): void {
    this.modal.abrir(disparador);
  }

  cerrar(): void {
    this.modal.cerrar();
  }
}
