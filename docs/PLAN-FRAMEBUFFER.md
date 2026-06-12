# PLAN — Extensión Framebuffer (modo gráfico inspirado en el modo 13h de VGA, con resolución 320×240)

> **Encuadre.** Exploración técnica de viabilidad, candidata a ampliación de alcance del proyecto. NO forma parte del alcance comprometido de v0.1 (SPEC.md lo marca como fuera de alcance). Esta extensión se desarrolla en una **rama de git separada** (`feature/framebuffer`); si más adelante un tutor avala su inclusión, se integra al alcance formal; si no, `main` (v0.1) queda intacta.
> **Modelo de desarrollo:** Claude Fable 5, autónomo, sobre este contrato. Prioridad: velocidad DENTRO de las compuertas de verificación (los gates no se saltan — son la red que permite ir rápido sin romper v0.1).

## Objetivo

Añadir una **pantalla gráfica de memoria mapeada** inspirada en el modo 13h de VGA —con resolución 320×240, no los 320×200 del 13h real— a EmuRV, y portar el Tetris a una versión gráfica en tiempo real como demo. Esto materializa el argumento "rendimiento para programas gráficos reales" que la investigación de antecedentes (WIMS) destaca y que v0.1 declara pero no demuestra.

## Decisiones cerradas (no reabrir)

| Parámetro | Valor | Razón |
|---|---|---|
| Resolución | **320×240** | Inspirado en el modo 13h de VGA (el 13h real es 320×200; aquí se adopta 320×240) |
| Color | **Paleta indexada de 256 colores, 1 byte/píxel** | Fiel al hardware emulado; 1/4 del tráfico vs RGB; históricamente correcto frente al Tetris 8086 original |
| Tamaño framebuffer | 76 800 bytes (320×240×1) | — |
| Paleta | 256 entradas RGB888 (768 bytes) en región propia | El programa puede reescribirla |
| Base del framebuffer | **0x9000_0000** (región nueva, no pisa .text/.data/pila) | Documentar en el mapa de memoria |
| Base de la paleta | **0x9002_0000** | — |
| Registro de control | **0x9003_0000** (el programa escribe 1 = "frame listo"; ver nota de divergencia sobre la semántica) | Permite al programa señalar "frame listo" |

> **Nota de divergencia (2026-06-11).** Se deja constancia de dos puntos en que la implementación real (`src-tauri/src/core/mod.rs`) difiere de lo planificado originalmente en este documento; la tabla de arriba ya refleja los valores implementados.
>
> 1. **Direcciones de paleta y control.** El plan decía paleta en **0x9001_0000** y registro de control en **0x9002_0000**; la implementación usa `PALETTE_BASE = 0x9002_0000` y `VIDEO_CONTROL_BASE = 0x9003_0000` (`FRAMEBUFFER_BASE = 0x9000_0000` sí coincide). Motivo inferido del código: los datos del framebuffer ocupan 76 800 bytes (0x12C00), que **no caben en 64 KiB** (0x10000) — la región mapeada del framebuffer es de 0x20000 bytes (`FRAMEBUFFER_SIZE`), por lo que una paleta en 0x9001_0000 habría quedado dentro de la región de video; cada región corrió una "página" de 0x10000 para no solaparse.
> 2. **Semántica del registro de control.** El plan prometía que *escribir 1 = solicitar redibujado / vsync lógico* (un hook de escritura que dispara el refresco — la parte (a) del riesgo RB-2, ex RF2). La implementación real **no tiene hook de escritura**: la publicación del frame ocurre al final de cada tramo de ejecución (`commands.rs` → `publish_frame`) y al entrar en la syscall 5 antes de bloquear por entrada (`syscalls.rs` → `publicar_frame`), leyendo **pasivamente** el registro de control; el refresco efectivo lo marca el timer de ~30 Hz de la UI (es decir, lo que existe es la parte (b) de RB-2). Decisión de sesión 2026-06-11: se corrige la documentación en lugar de agregar el hook, por menor riesgo.

## Riesgos y mitigaciones (aplicar sin pedir permiso, registrar en BITACORA.md)

> **Nomenclatura (2026-06-11):** los riesgos de este plan se renombran de RF1..RF4 a **RB-1..RB-4** ("Riesgo de framebuffer"; RB-N = ex RF-N de este plan) para no colisionar con los requisitos funcionales RF-01..RF-10 de SPEC.md.

- **RB-1 [Probable] Tráfico del puente.** 76 800 bytes/frame por el puente Tauri puede ser el cuello de botella. **Mitigación obligatoria desde el primer commit:** el frame NO se serializa como JSON. Opciones en orden de preferencia: (a) `tauri::ipc::Response` con bytes crudos; (b) canal de eventos con `ArrayBuffer`; (c) memoria compartida si Tauri 2 lo permite en esta versión. Medir los tres si hace falta. El framebuffer se vuelca al canvas con `putImageData` sobre un `ImageData` reusado (no recrear cada frame). **Nunca** pintar píxel por píxel desde JS.
- **RB-2 [Probable] Frecuencia de redibujado.** No redibujar por cada escritura de byte a la VRAM (sería absurdo). El redibujado se dispara por: (a) escritura al registro de control 0x90030000 (el programa señala "frame listo"), o (b) timer de ~30 Hz que vuelca si hubo escrituras desde el último frame (flag sucio). Igual que en v0.1 con los registros: la UI se refresca a ritmo fijo, no por evento de máquina. *(Divergencia 2026-06-11: la implementación final solo materializó la vía (b) — ver la nota de divergencia junto a la tabla de decisiones.)*
- **RB-3 [Suposición] Hook de escritura a memoria.** Mapear la VRAM como región Unicorn normal y leerla al vsync es más simple y probablemente más rápido que un hook de escritura por byte. Preferir lectura por volcado sobre hook de escritura, salvo que la medición diga lo contrario.
- **RB-4 [Seguro] Compatibilidad con v0.1.** Los programas sin gráficos no deben pagar ningún costo: si el programa nunca escribe en la región de video, no se mapea el canvas ni corre el timer de vsync. La pantalla gráfica es un panel que aparece solo cuando se detecta uso de VRAM (o por toggle manual).

## Fases con compuertas

### FB0 — Rama y mapa de memoria
- `git checkout -b feature/framebuffer` desde el `main` actual (v0.1 entregado).
- Mapear en `machine.rs` las tres regiones nuevas (VRAM, paleta, control) con sus tamaños.
- Actualizar el mapa de memoria en la ayuda integrada y en un nuevo `docs/MEMORIA-GRAFICA.md`.
- **Gate FB0:** test que verifica que las regiones existen, son escribibles/legibles y no solapan con .text/.data/pila. `cargo test` verde incluyendo los 35 de v0.1 (no romper nada).

### FB1 — Núcleo del framebuffer (sin UI)
- Lógica de lectura del framebuffer + paleta → buffer RGBA de 320×240 listo para canvas (conversión índice→RGB en Rust, no en JS).
- Paleta por defecto (al estilo de la del modo 13h de VGA: primeros 16 colores CGA/EGA + rampa; recordar que la resolución aquí es 320×240, no la 320×200 del 13h).
- Comando Tauri `read_framebuffer` que devuelve el buffer RGBA como bytes crudos (RB-1).
- **Gate FB1:** test que escribe píxeles conocidos en VRAM + entradas de paleta, llama a la conversión y verifica los bytes RGBA exactos resultantes. Test de rendimiento: conversión de un frame completo < 2 ms.

### FB2 — Panel de pantalla en la UI
- Canvas 320×240 (escalado por CSS con `image-rendering: pixelated`, sin interpolación borrosa).
- Volcado con `putImageData` sobre `ImageData` reusado; timer de vsync ~30 Hz con flag sucio (RB-2).
- El panel aparece al detectar uso de VRAM o por toggle manual en la UI; oculto por defecto (RB-4).
- i18n de los textos nuevos en `es.json`.
- **Gate FB2:** ejemplo mínimo nuevo (`ejemplos/07-franjas.asm`: pinta franjas de color escribiendo en VRAM + paleta) se ve correctamente en el panel. Captura en `docs/capturas/fb2-franjas.png`. Medir FPS efectivos con un programa que reescribe el frame en bucle; registrar en BITACORA.

### FB3 — Tetris gráfico
- Port del Tetris (`ejemplos/08-tetris-grafico.asm`): misma lógica de juego que `06-tetris.asm` (mismos bitmaps, mismo LCG y semilla, misma colisión/líneas/puntaje — reusar lo verificado), reemplazando la capa de salida ASCII por dibujo de celdas en el framebuffer (rectángulos de color por bloque) y la entrada por turnos por entrada en tiempo real si el tiempo lo permite; si no, mantener por turnos pero con render gráfico.
- **Gate FB3:** test de integración que ejecuta una secuencia determinista de piezas y verifica el estado del tablero en memoria (igual que el test de `06-tetris`, reusando el oráculo). Verificación visual: captura del tablero gráfico con piezas colocadas (`docs/capturas/fb3-tetris.png`).

### FB4 — Cierre
- `docs/MEMORIA-GRAFICA.md` completo: mapa de memoria gráfica, formato de paleta, registro de control, ejemplo de uso comentado, tabla de FPS medidos.
- README actualizado con la sección de modo gráfico.
- BITACORA: entrada por cada decisión y medición, con nivel de confianza.
- **Gate FB4:** `cargo test` 100% verde (v0.1 + framebuffer), `cargo clippy` 0 warnings, `cargo tauri build` exitoso, peso del bundle registrado contra el límite de 30 MB. Commit final de la rama. NO se hace merge a main automáticamente: queda como rama lista para revisión (pendiente de aval de tutor para integrarse al alcance).

## Lo que esta extensión NO hace (límites explícitos)
- No toca el alcance documentado de v0.1 (SPEC.md/PLAN.md de v0.1 quedan intactos).
- No mergea a `main` sin decisión humana.
- No agrega datapath/pipeline visual (frontera deliberada frente a Ripes — fuera de discusión).
- No agrega sonido, sprites por hardware, ni aceleración: es un framebuffer lineal simple.
