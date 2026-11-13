import { t } from "../i18n";
import "./inicio.css";

type Intro = { play(): Promise<void>; finish(): void; destroy(): void };
let abierto = false;

/** La presentación es independiente de la inicialización del simulador. */
export async function mostrarInicio(): Promise<void> {
  if (abierto || matchMedia("(prefers-reduced-motion: reduce)").matches) return;
  abierto = true;
  let overlay: HTMLElement | undefined;
  let intro: Intro | undefined;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const app = document.getElementById("app");
  const foco = document.activeElement as HTMLElement | null;
  const alTeclado = (e: KeyboardEvent) => {
    if (e.key === "Escape") { e.preventDefault(); cerrar(); }
    if (e.key === "Tab" && overlay) {
      e.preventDefault(); overlay.focus();
    }
  };
  const cerrar = () => {
    if (timer) clearTimeout(timer);
    document.removeEventListener("keydown", alTeclado, true);
    intro?.destroy();
    overlay?.remove();
    if (app) app.inert = false;
    if (overlay) {
      if (foco && foco !== document.body && foco.isConnected) foco.focus();
      else document.querySelector<HTMLElement>(".cm-content")?.focus();
    }
    abierto = false;
  };
  try {
    // Se conserva el reproductor original junto a los medios y fuentes de marca.
    // public/ evita transformar sus rutas de audio en la compilación de Vite.
    const url = new URL("marca/emurv/inicio/player.js", document.baseURI).href;
    const { EmuRVIntro } = await import(/* @vite-ignore */ url);
    overlay = document.createElement("section");
    overlay.className = "marca-inicio";
    overlay.setAttribute("role", "dialog");
    overlay.setAttribute("aria-modal", "true");
    overlay.setAttribute("aria-label", t("inicio.titulo"));
    const scene = document.createElement("div");
    scene.className = "marca-inicio-escena";
    overlay.tabIndex = -1;
    overlay.append(scene);
    document.body.append(overlay);
    if (app) app.inert = true;
    document.addEventListener("keydown", alTeclado, true);
    overlay.focus();
    intro = new EmuRVIntro(scene, {
      theme: "dark", variant: "ensamble", soundEnabled: true,
      onComplete: cerrar,
    }) as Intro;
    // La denegación de audio, una pestaña oculta o un medio dañado nunca
    // deben impedir el acceso al editor. No se espera a cargar recursos de red.
    timer = setTimeout(cerrar, 6500);
    void intro.play().catch(cerrar);
  } catch (error) {
    console.warn("No se pudo mostrar la presentación de EmuRV", error);
    cerrar();
  }
}
