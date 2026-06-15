# PLAN-RP31 — Sprites + Sonido (sobre el framebuffer)

Punto RP-31 de `docs/ROADMAP-PRO.md`. Worktree propio (`rp31/sprites-sonido`).
Plan por fases con compuertas: PROHIBIDO avanzar con la compuerta en rojo.
**Huella mínima en `main.ts`** (1-2 líneas de instanciación): RP-30 lo
rearquitectura después y un footprint chico evita pelea de merge.

**Decisión del autor:** dar a los programas un dispositivo de **sonido** y
soporte de **sprites** para que el emulador sirva para hacer cosas tipo juego
(uso "profesional/creativo"), espejando el modelo de video que ya existe.

## Contexto (anclas verificadas)

- Video YA existe como MMIO: `FRAMEBUFFER_BASE 0x9000_0000` (320×240 indexado),
  `PALETTE_BASE 0x9002_0000` (256×RGB), `VIDEO_CONTROL_BASE 0x9003_0000`
  (`src-tauri/src/core/mod.rs:79-97`). `KEYBOARD_BASE 0xFFFF_0000`.
- Patrón de dispositivo (a espejar para sonido): el hilo de emulación PUBLICA en
  un estado compartido (`core/video.rs` `VideoShared`: rgba/seq/in_use); el run
  loop detecta uso y emite un evento Tauri (`commands.rs` `emu://video-activo`);
  el frontend CONSUME por comando (`read_framebuffer`) sin tocar el hilo. El
  panel de video se muestra solo si hubo uso real (RF4).
- Frontend: `src/paneles/video.ts` (canvas 320×240, volcado por frame).
- NO existe NADA de audio hoy (grep audio/sonido/AudioContext = 0 en src/src-tauri).

## Alcance

- SÍ **sonido**: una región MMIO de sonido nueva (no pisar video/teclado) donde
  el programa escribe frecuencia / duración / forma de onda y dispara un tono;
  el backend detecta el disparo y emite un evento; el frontend lo reproduce con
  WebAudio (`AudioContext` + `OscillatorNode`). Apagado por defecto (como video:
  solo suena si el programa usa el dispositivo). Ejemplo `.asm` que suena.
- SÍ **sprites**: la arquitectura la FIJA el spike F1, eligiendo el camino más
  barato y fiel:
  (a) overlay de sprites por hardware: una tabla de sprites en MMIO (x, y,
      índice de patrón, flags) + memoria de patrones, compuesta sobre el
      framebuffer en Rust ANTES de convertir a RGBA; o
  (b) un helper/macro de "blit" documentado + ejemplo que dibuja un sprite con
      escrituras al framebuffer ya existente (sin MMIO nueva).
  Preferir (a) si el costo es razonable (más "profesional"); si resulta
  desproporcionado, (b) es aceptable y se documenta la decisión.
- NO: música/secuenciador, mixing multicanal complejo (un canal de tono basta
  para lo educativo; registrar si hace falta más), formatos de audio (WAV/MP3),
  aceleración por GPU. Mantener el peso del bundle ≤30 MB (WebAudio es nativo
  del runtime, cero dependencias npm).

## Diseño acordado (el QUÉ; el CÓMO exacto lo cierra el spike)

- **Sonido — backend:** región `SOUND_BASE` nueva (p. ej. `0x9004_0000`, lejos de
  video/teclado; elegir y documentar). Registros: frecuencia (Hz), duración
  (ms), forma de onda (0=cuadrada/1=senoidal/2=triángulo/3=sierra), y un
  registro de disparo. `SoundShared` (espejo de `VideoShared`) con una cola de
  eventos de sonido + `in_use`. El run loop la drena y emite
  `emu://sonido { freq, dur_ms, wave }`. Mapear la región en Unicorn como las de
  video. Errores didácticos coherentes con el resto.
- **Sonido — frontend:** módulo `src/paneles/audio.ts` (clase `Audio`): un único
  `AudioContext` (creado tras gesto del usuario / primera reproducción para no
  violar la política de autoplay), `events.onSonido` → crea un `OscillatorNode`
  con la forma de onda y un `GainNode` con envolvente corta para que no chasquee;
  se autodetiene a `dur_ms`. Respeta silencio si no hay soporte.
- **Sprites:** según (a)/(b) del spike. Si (a): `core/sprites.rs` compone la capa
  sobre el indexado antes de `FramebufferRGBA::from_indexed`; transparencia por
  índice 0 (o color-key configurable). Documentar el mapa MMIO.
- **Integración main.ts:** SOLO instanciar la clase `Audio` junto a `Video`
  (~línea 150, donde se crea `this.video`) y conectar el evento. Nada de tocar
  el modelo de buffer/editor (territorio de RP-30).
- **i18n es/en:** claves nuevas al final de ambos JSON (etiqueta del panel/aviso
  de sonido si lo hubiera), anti-drift verde.

## Fase 1 — Spike: dispositivo de sonido + decisión de sprites

`tests/spike_sonido.rs` (o ampliar un spike existente): (1) mapear `SOUND_BASE`
en Unicorn y leer/escribir sus registros; (2) un programa mínimo que escribe
freq/dur/disparo y verificar que `SoundShared` registra el evento; (3) confirmar
que video y teclado siguen idénticos (regiones intactas). DECIDIR en BITACORA
[Seguro]: layout MMIO de sonido y camino de sprites (a vs b) con su costo.
**Compuerta F1:** spike verde + decisiones fundadas en BITACORA; suite actual
intacta. Commit `rp31-fase-1: spike sonido + decision de sprites`.

## Fase 2 — Backend de sonido (MMIO + evento), regiones intactas

`core/sound.rs` (`SoundShared`), mapeo MMIO, detección de disparo en el run loop,
`emu://sonido`. Tests: round-trip de un evento de sonido; `in_use` arranca en
false; video/teclado intactos.
**Compuerta F2:** suite completa verde + tests de sonido + clippy. Commit
`rp31-fase-2: dispositivo de sonido MMIO + evento`.

## Fase 3 — Frontend de audio (WebAudio)

`src/paneles/audio.ts` (clase `Audio`), `events.onSonido`, envolvente
anti-click, `AudioContext` perezoso (gesto de usuario). Instanciación mínima en
`main.ts`. Tests UI de lo testeable (p. ej. mapping wave→type, cálculo de
envolvente como función pura).
**Compuerta F3:** tsc + suite + reproduce un tono al ejecutar el ejemplo. Commit
`rp31-fase-3: reproduccion de sonido (WebAudio)`.

## Fase 4 — Sprites (según decisión F1)

(a) `core/sprites.rs` + composición sobre el framebuffer + tests de composición
(sprite en (x,y), transparencia, fuera de pantalla recortado); o (b) helper +
ejemplo documentado. RV32 y RV64 (RP-32 ya está) deben ensamblar el ejemplo.
**Compuerta F4:** suite + clippy + (si a) tests de composición; un sprite se ve
en pantalla.
Commit `rp31-fase-4: sprites sobre el framebuffer`.

## Fase 5 — Ejemplos, docs y cierre

Ejemplo `ejemplos/NN-sonido.asm` y `ejemplos/NN-sprites.asm` (o uno combinado
tipo mini-juego); documentar el mapa MMIO de sonido y sprites en
`docs/` (donde vive la doc del framebuffer/teclado); BITACORA consolidada.
**Compuerta F5 (cierre):** cargo test + clippy AMBOS crates + tsc + ci-local
--skip-bundle + tauri build + bundle ≤30 MB + verificación manual (suena + se ve
el sprite). Commit `rp31-fase-5: ejemplos y docs de sprites/sonido`.

## Criterios de aceptación

- Un programa puede emitir sonido vía MMIO y se escucha (WebAudio). ✓ F2-F3.
- Soporte de sprites (overlay HW o blit documentado) con ejemplo visible. ✓ F4.
- Video y teclado intactos; regiones MMIO no se pisan. ✓ todas.
- Bundle ≤30 MB; cero dependencias npm nuevas. ✓ F5.
- Huella en main.ts mínima (no toca el modelo de buffer; deja RP-30 limpio). ✓.
- Si sprites-HW resulta desproporcionado, blit documentado + decisión en
  BITACORA también cierra el punto. ✓ F1/F4.
