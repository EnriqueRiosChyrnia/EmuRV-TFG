// Dispositivo de SONIDO (RP31): reproduce los tonos que dispara el programa
// vía MMIO. El backend emite `emu://sonido { freq, dur_ms, wave }` por cada
// disparo (espejo del modelo de video) y esta clase lo toca con WebAudio
// (nativo del runtime: cero dependencias npm). Apagado por defecto, igual que
// el panel de video: si el programa no usa el dispositivo, no suena nada y ni
// se crea el AudioContext.
import type { SonidoEvent } from "../api";

/** Duración del ataque/liberación de la envolvente (s): rampas cortas que
 *  evitan el «clic» de empezar/cortar una onda en seco. */
const ATAQUE_S = 0.005;
const LIBERACION_S = 0.005;
/** Pico de ganancia: volumen moderado (un tono cuadrado a ganancia 1 satura
 *  y molesta). 0.2 es audible sin ser agresivo. */
const GANANCIA_PICO = 0.2;

/** Mapea el registro de forma de onda del dispositivo al tipo de oscilador de
 *  WebAudio. 0=cuadrada, 1=senoidal, 2=triángulo, 3=sierra; cualquier otro
 *  valor cae en cuadrada (el default histórico de los chips de sonido). */
export function waveType(wave: number): OscillatorType {
  switch (wave) {
    case 1:
      return "sine";
    case 2:
      return "triangle";
    case 3:
      return "sawtooth";
    case 0:
    default:
      return "square";
  }
}

/** Plan de envolvente (en segundos) para un tono de `durMs` ms. Función PURA
 *  (testeable sin AudioContext): el ataque y la liberación no pueden superar
 *  la mitad de la duración cada uno, así un tono muy corto encoge sus rampas
 *  en vez de solaparlas (que sonaría como un clic). */
export interface PlanEnvolvente {
  /** Duración del ataque (s). */
  ataque: number;
  /** Duración de la liberación (s). */
  liberacion: number;
  /** Ganancia pico (meseta entre ataque y liberación). */
  pico: number;
  /** Duración total efectiva del tono (s). */
  total: number;
}

export function planEnvolvente(durMs: number): PlanEnvolvente {
  const total = Math.max(0, durMs) / 1000;
  const mitad = total / 2;
  return {
    ataque: Math.min(ATAQUE_S, mitad),
    liberacion: Math.min(LIBERACION_S, mitad),
    pico: GANANCIA_PICO,
    total,
  };
}

/** Constructor de AudioContext, tolerante al prefijo `webkit` de WebKit viejo
 *  (Safari) y a entornos sin WebAudio (devuelve undefined → silencio). */
type AudioContextCtor = typeof AudioContext;
function ctorAudioContext(): AudioContextCtor | undefined {
  if (typeof window === "undefined") return undefined;
  return (
    window.AudioContext ??
    (window as unknown as { webkitAudioContext?: AudioContextCtor }).webkitAudioContext
  );
}

export class Audio {
  /** Único AudioContext de la app, creado PEREZOSAMENTE en el primer tono.
   *  Para entonces el usuario ya tocó «Ejecutar» (gesto previo), así que la
   *  política de autoplay permite reanudarlo. Antes de eso queda en null y no
   *  se crea nada. */
  private ctx: AudioContext | null = null;
  /** WebAudio no disponible en este entorno: se respeta el silencio sin
   *  reintentar crear el contexto en cada tono. */
  private sinSoporte = false;

  /** Crea (o devuelve) el AudioContext; null si el entorno no tiene WebAudio. */
  private asegurarContexto(): AudioContext | null {
    if (this.ctx) return this.ctx;
    if (this.sinSoporte) return null;
    const Ctor = ctorAudioContext();
    if (!Ctor) {
      this.sinSoporte = true;
      return null;
    }
    try {
      this.ctx = new Ctor();
    } catch {
      this.sinSoporte = true;
      return null;
    }
    return this.ctx;
  }

  /** Reproduce un tono. Silencio si el tono es vacío (duración o frecuencia no
   *  positivas) o si no hay soporte de WebAudio. */
  reproducir(ev: SonidoEvent): void {
    const plan = planEnvolvente(ev.dur_ms);
    if (plan.total <= 0 || ev.freq <= 0) return;
    const ctx = this.asegurarContexto();
    if (!ctx) return;
    // El contexto puede arrancar «suspended» (creado fuera del gesto): como el
    // usuario ya tocó «Ejecutar», reanudarlo está permitido.
    void ctx.resume();

    const t0 = ctx.currentTime;
    const osc = ctx.createOscillator();
    osc.type = waveType(ev.wave);
    osc.frequency.value = ev.freq;

    // Envolvente lineal: 0 → pico (ataque), meseta, pico → 0 (liberación).
    const gain = ctx.createGain();
    gain.gain.setValueAtTime(0, t0);
    gain.gain.linearRampToValueAtTime(plan.pico, t0 + plan.ataque);
    const inicioLiberacion = t0 + Math.max(plan.ataque, plan.total - plan.liberacion);
    gain.gain.setValueAtTime(plan.pico, inicioLiberacion);
    gain.gain.linearRampToValueAtTime(0, t0 + plan.total);

    osc.connect(gain).connect(ctx.destination);
    osc.start(t0);
    osc.stop(t0 + plan.total);
    // Liberar los nodos al terminar (no se acumulan a lo largo de la sesión).
    osc.onended = () => {
      osc.disconnect();
      gain.disconnect();
    };
  }
}
