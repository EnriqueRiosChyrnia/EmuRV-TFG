// Editor de tema de usuario (RP28, «Parte B» de RP17): un modal para que el
// usuario arme su propio tema. Reusa el shell de Modal (RP20) y la lógica de
// temas/usuario.ts (persistencia + vars inline) y temas/contraste.ts (aviso
// AA). Un control por token principal: <input type="color"> + entrada hex,
// sincronizados; PREVIEW EN VIVO (cada cambio pinta :root al instante);
// botón restaurar (vuelve al Educativo); aviso —NO bloqueante— si algún par
// crítico no llega a AA 4.5:1; exportar/importar el tema como JSON.
//
// El preview se aplica mientras el modal está abierto; si el usuario cierra
// sin guardar, cerrar() revierte al tema persistido. Guardar persiste y
// selecciona el tema «usuario».
//
// Estilos: el componente inyecta su CSS una vez (clases .et-*) en vez de
// tocar styles.css — así el modal queda autocontenido (RP28 corre en su
// worktree, disjunto del resto; BITACORA). Todo usa var(--…), así el editor
// recolorea con el tema que se está editando.
import { t } from "../i18n";
import { Modal, ICONO_CERRAR } from "./modal";
import { aplicarTema, leerTema, setTema } from "../temas";
import {
  TOKENS_USUARIO,
  type TemaUsuario,
  type TokenUsuario,
  aplicarVarsUsuario,
  esHex,
  guardarTemaUsuario,
  leerTemaUsuario,
  normalizarTema,
  temaUsuarioPorDefecto,
} from "../temas/usuario";
import { paresQueFallan } from "../temas/contraste";

// Agrupación de los tokens en el editor (orden y secciones visibles).
const GRUPOS: Array<{ clave: string; tokens: readonly TokenUsuario[] }> = [
  { clave: "superficies", tokens: ["fondo", "fondo-panel", "fondo-claro", "borde"] },
  { clave: "texto", tokens: ["texto", "texto-suave", "texto-sobre-acento"] },
  { clave: "acento", tokens: ["acento", "ok", "error", "cambio"] },
  { clave: "sintaxis", tokens: ["sx-com", "sx-dir", "sx-lbl", "sx-ins", "sx-str", "sx-num"] },
];

export class EditorTema extends Modal {
  private draft: TemaUsuario = temaUsuarioPorDefecto();
  /** Avisar al toolbar que el tema activo cambió (sincroniza el <select>). */
  private readonly onCambio: () => void;

  constructor(onCambio: () => void) {
    super("modal-cuerpo-tema");
    this.onCambio = onCambio;
    inyectarEstilos();
    this.card.innerHTML = this.plantilla();
    this.cablear();
  }

  // ----- ciclo de vida -----

  override abrir(disparador?: HTMLElement | null): void {
    // Arranca del tema guardado (o copia del Educativo si no hay) y muestra
    // el preview en vivo desde el primer instante.
    this.draft = leerTemaUsuario();
    this.sincronizarInputs();
    this.previsualizar();
    super.abrir(disparador);
  }

  override cerrar(): void {
    super.cerrar();
    // Revertir el preview: reaplica el tema persistido. Si se acaba de
    // guardar, ese tema ya es «usuario» con los valores recién guardados
    // (reafirma, sin parpadeo); si no se guardó, vuelve al tema previo.
    aplicarTema(leerTema());
    this.onCambio();
  }

  // ----- construcción -----

  private plantilla(): string {
    const filas = GRUPOS.map((g) => {
      const rows = g.tokens
        .map(
          (tk) =>
            `<div class="et-fila" data-token="${tk}">` +
            `<label for="et-c-${tk}">${t(`tema.token.${tk}`)}</label>` +
            `<input type="color" class="et-color" id="et-c-${tk}" ` +
            `aria-label="${t(`tema.token.${tk}`)}" />` +
            `<input type="text" class="et-hex" spellcheck="false" ` +
            `aria-label="${t(`tema.token.${tk}`)} (hex)" /></div>`,
        )
        .join("");
      return `<div class="et-grupo"><div class="et-grupo-tit">${t(`tema.grupo.${g.clave}`)}</div>${rows}</div>`;
    }).join("");

    return (
      `<header class="modal-head"><h2>${t("tema.editar")}` +
      `<span class="mh-sub">${t("tema.editar.sub")}</span></h2>` +
      `<button class="modal-x" aria-label="${t("ayuda.cerrar")}">${ICONO_CERRAR}</button></header>` +
      `<div class="modal-scroll et-scroll">${filas}<div class="et-aviso" role="status"></div></div>` +
      `<footer class="et-foot">` +
      `<button class="et-btn" data-act="restaurar">${t("tema.restaurar")}</button>` +
      `<span class="et-foot-sp"></span>` +
      `<button class="et-btn" data-act="exportar">${t("tema.exportar")}</button>` +
      `<button class="et-btn" data-act="importar">${t("tema.importar")}</button>` +
      `<button class="et-btn et-btn-pri" data-act="guardar">${t("tema.guardar")}</button>` +
      `<input type="file" accept="application/json,.json" class="et-file" hidden />` +
      `</footer>`
    );
  }

  private cablear(): void {
    // Edición: color o hex de una fila → actualiza el otro, el draft y el
    // preview. (delegación: un solo listener para todas las filas)
    this.card.querySelector(".et-scroll")!.addEventListener("input", (e) => {
      const el = e.target as HTMLInputElement;
      const fila = el.closest<HTMLElement>(".et-fila");
      if (!fila) return;
      const token = fila.dataset.token as TokenUsuario;
      if (el.classList.contains("et-color")) {
        this.draft[token] = el.value;
        (fila.querySelector(".et-hex") as HTMLInputElement).value = el.value;
        this.previsualizar();
      } else if (el.classList.contains("et-hex")) {
        const v = el.value.trim();
        if (esHex(v)) {
          this.draft[token] = v;
          (fila.querySelector(".et-color") as HTMLInputElement).value = v;
          this.previsualizar();
        }
        // hex incompleto/ inválido: no aplicar (el usuario sigue tipeando)
      }
    });

    // Acciones del pie.
    this.card.querySelector(".et-foot")!.addEventListener("click", (e) => {
      const btn = (e.target as HTMLElement).closest<HTMLElement>("[data-act]");
      if (!btn) return;
      switch (btn.dataset.act) {
        case "restaurar":
          this.restaurar();
          break;
        case "guardar":
          this.guardar();
          break;
        case "exportar":
          this.exportar();
          break;
        case "importar":
          this.card.querySelector<HTMLInputElement>(".et-file")!.click();
          break;
      }
    });
    this.card
      .querySelector(".et-file")!
      .addEventListener("change", (e) => this.importar(e));
  }

  // ----- estado / preview -----

  /** Vuelca el draft a los inputs (color + hex de cada fila). */
  private sincronizarInputs(): void {
    for (const tk of TOKENS_USUARIO) {
      const c = this.card.querySelector<HTMLInputElement>(`#et-c-${tk}`);
      const h = this.card.querySelector<HTMLInputElement>(`.et-fila[data-token="${tk}"] .et-hex`);
      if (c) c.value = this.draft[tk];
      if (h) h.value = this.draft[tk];
    }
  }

  /** Pinta :root con el draft (preview en vivo) y recalcula el aviso AA. */
  private previsualizar(): void {
    document.documentElement.setAttribute("data-tema", "usuario");
    aplicarVarsUsuario(this.draft);
    this.actualizarAvisos();
  }

  /** Aviso de contraste (no bloqueante): pares críticos por debajo de AA. */
  private actualizarAvisos(): void {
    const caja = this.card.querySelector<HTMLElement>(".et-aviso")!;
    const fallas = paresQueFallan(this.draft);
    if (fallas.length === 0) {
      caja.className = "et-aviso et-aviso-ok";
      caja.textContent = t("tema.aviso.ok");
      return;
    }
    caja.className = "et-aviso et-aviso-bad";
    const items = fallas
      .map(
        (f) =>
          `<li>${t("tema.aviso.par", {
            fg: t(`tema.token.${f.fg}`),
            bg: t(`tema.token.${f.bg}`),
            ratio: f.ratio.toFixed(1),
          })}</li>`,
      )
      .join("");
    caja.innerHTML = `<div class="et-aviso-tit">${t("tema.aviso.titulo")}</div><ul>${items}</ul>`;
  }

  // ----- acciones -----

  private restaurar(): void {
    this.draft = temaUsuarioPorDefecto();
    this.sincronizarInputs();
    this.previsualizar();
  }

  private guardar(): void {
    guardarTemaUsuario(this.draft);
    setTema("usuario"); // persiste + aplica el tema desde storage
    this.cerrar(); // reafirma el tema guardado y sincroniza el selector
  }

  private exportar(): void {
    const blob = new Blob([JSON.stringify(this.draft, null, 2)], {
      type: "application/json",
    });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "emurv-tema.json";
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
  }

  private importar(e: Event): void {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    input.value = ""; // permite reimportar el mismo archivo
    if (!file) return;
    const reader = new FileReader();
    reader.onload = (): void => {
      try {
        this.draft = normalizarTema(JSON.parse(String(reader.result)));
        this.sincronizarInputs();
        this.previsualizar();
      } catch {
        const caja = this.card.querySelector<HTMLElement>(".et-aviso")!;
        caja.className = "et-aviso et-aviso-bad";
        caja.textContent = t("tema.importar.error");
      }
    };
    reader.readAsText(file);
  }
}

// CSS del componente, inyectado una vez. Todo en var(--…) para recolorear
// con el tema en edición. Prefijo .et-* (editor-tema) para no colisionar.
function inyectarEstilos(): void {
  if (document.getElementById("estilo-editor-tema")) return;
  const s = document.createElement("style");
  s.id = "estilo-editor-tema";
  s.textContent = `
.modal-card.modal-cuerpo-tema { width: min(560px, 100%); max-height: 88%; }
.et-scroll { padding: 4px 14px 14px; }
.et-grupo { margin-top: 10px; }
.et-grupo-tit {
  font-size: 10.5px; text-transform: uppercase; letter-spacing: .06em;
  color: var(--texto-suave); opacity: .75; margin: 8px 2px 4px;
}
.et-fila {
  display: grid; grid-template-columns: 1fr auto 92px; align-items: center;
  gap: 10px; padding: 4px 2px;
}
.et-fila > label { font-size: 13px; color: var(--texto); }
.et-color {
  -webkit-appearance: none; appearance: none; width: 32px; height: 24px;
  padding: 0; border: 1px solid var(--borde); border-radius: 6px;
  background: none; cursor: pointer;
}
.et-color::-webkit-color-swatch-wrapper { padding: 0; }
.et-color::-webkit-color-swatch { border: none; border-radius: 5px; }
.et-hex {
  font-family: var(--mono); font-size: 12.5px; padding: 5px 8px; width: 100%;
  border: 1px solid var(--borde); border-radius: 7px;
  background: var(--fondo); color: var(--texto);
}
.et-hex:focus, .et-color:focus { outline: 2px solid var(--acento); outline-offset: 1px; }
.et-aviso { margin: 14px 2px 2px; font-size: 12.5px; border-radius: 9px; padding: 9px 11px; line-height: 1.45; }
.et-aviso:empty { display: none; }
.et-aviso-ok { color: var(--ok); background: color-mix(in srgb, var(--ok) 12%, transparent); }
.et-aviso-bad {
  color: var(--texto);
  background: color-mix(in srgb, var(--error) 13%, transparent);
  border: 1px solid color-mix(in srgb, var(--error) 35%, transparent);
}
.et-aviso-tit { font-weight: 600; color: var(--error); margin-bottom: 4px; }
.et-aviso ul { margin: 0; padding-left: 18px; }
.et-foot {
  display: flex; align-items: center; gap: 8px; flex: none;
  padding: 12px 14px; border-top: 1px solid var(--borde);
}
.et-foot-sp { flex: 1; }
.et-btn {
  font-family: inherit; font-size: 12.5px; padding: 7px 12px;
  border-radius: var(--radio); border: 1px solid var(--borde);
  background: var(--fondo-claro); color: var(--texto); cursor: pointer;
}
.et-btn:hover { background: color-mix(in srgb, var(--acento) 12%, var(--fondo-claro)); }
.et-btn-pri {
  background: var(--acento); color: var(--texto-sobre-acento);
  border-color: transparent; font-weight: 600;
}
.et-btn-pri:hover { background: var(--acento); filter: brightness(1.06); }
`;
  document.head.appendChild(s);
}
