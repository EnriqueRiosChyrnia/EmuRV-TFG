# Resultados: benchmark de rendimiento (RT-4)

Rendimiento del motor de EmuRV contra QEMU, Spike y RARS con el **mismo
algoritmo en los cuatro emuladores**, más el caso de aplicación en tiempo
real (Tetris) y la comparación con emu8086. Material citable del Capítulo V.

- **Máquina:** Apple M4 Pro, macOS 26.5, corriendo enchufado y sin carga.
- **Versiones:** EmuRV 0.1.0 (emurv-cli release; motor Unicorn 2.1.5) ·
  QEMU 11.0.2 (`qemu-system-riscv32 -M virt`, semihosting) · Spike 1.1.1-dev
  (brew riscv-isa-sim) · RARS 1.6 (OpenJDK 25; arranque JVM de 0,133 s
  descontado).
- **Metodología:** 4 kernels RV32IM (`bench/kernels/`) con cuenta NOMINAL de
  instrucciones documentada en el fuente; ~495–500 M instrucciones por
  corrida (RARS: ÷100, mismo cómputo de MIPS); wall-time, mediana de 5
  repeticiones (mín/máx publicados en el CSV); equivalencia verificada:
  los 4 emuladores producen el MISMO resultado numérico por kernel.
- **Reproducción:** `scripts/bench-rt4.sh build && verify && run` ·
  CSV archivado en `docs/evidencia/rt4-benchmark.csv`.
- **Fecha:** 2026-07-22.

## Tabla: M instr/s (mediana de 5)

Con el **tramo adaptativo** (2026-07-22, misma fecha): el lazo de ejecución
apunta a ~40 ms de pared por chequeo de interrumpibilidad en vez de un count
fijo de 1 M instrucciones; con la pantalla gráfica activa vuelve al piso
(cadencia de frames). El teclado no depende del tramo (cola MMIO asíncrona).

| Kernel | EmuRV | QEMU (TCG) | Spike | RARS |
|---|---:|---:|---:|---:|
| aritmética (ALU pura) | **718,4** | 3 289,5 | 881,8 | 6,0 |
| memoria (lw/sw 4 KiB) | **257,1** | 4 342,1 | 992,0 | 5,6 |
| llamadas (call/ret + pila) | **299,4** | 1 090,5 | 982,1 | 5,8 |
| mixto (LCG + memoria + mul) | **160,2** | 4 805,8 | 992,0 | 5,7 |

Con el tramo fijo original de 1 M (medición previa del mismo día, archivada
en el historial de git de este CSV): EmuRV 129,5–308,5 — el tramo adaptativo
recuperó entre 1,2× y 2,3× sin perder interrumpibilidad perceptible.

## Lectura

- **EmuRV vs RARS (su categoría: emulador educativo RISC-V): 28–120× más
  rápido.** Es la comparación de posicionamiento: misma función didáctica,
  motores de distinta clase (JIT heredado de QEMU vía Unicorn vs intérprete
  Java).
- **EmuRV está en el mismo orden de magnitud que Spike**, el simulador ISS
  de referencia del ecosistema (81 % de Spike en ALU pura; ×0,16–0,81 según
  kernel), y 0,7–1,5 órdenes por debajo de QEMU puro. El costo restante es
  deliberado: chequeo de interrumpibilidad cada ~40 ms, hooks de
  observación didáctica (teclado MMIO, video, syscalls) y el wall-time
  incluye arranque de proceso + ensamblado del fuente. Techo interno del
  motor sin tramos (RNF-03, misma máquina): ~1 500 M instr/s.
- **Dispersión:** con el tramo adaptativo la varianza de EmuRV bajó
  drásticamente en ALU (0,688–0,706 s); en los kernels con memoria persiste
  algo de calentamiento del JIT de Unicorn (mín/máx en el CSV). La mediana
  es la cifra reportada.
- **Cumplimiento RNF-03 (≥10 M instr/s): 16–72× por encima** del requisito
  en el peor kernel.

## Caso Tetris / tiempo real

Con el ejemplo de carga máxima gráfica (repintado COMPLETO del framebuffer
320×240 por frame + cierre de frame por syscall, el camino de producción):

- **797 FPS publicados del lado Rust** (1 000 frames en 1,255 s, release,
  misma máquina; `cargo test --release --test fb4_fps -- --ignored`).
  La UI consume ~30 Hz: el motor tiene ~26× de margen sobre lo que la
  pantalla necesita — el Tetris en tiempo real (TEC-5) corre sin despeinarse.
- **emu8086** (el emulador que EmuRV reemplaza en la asignatura):
  comparación **documental** — es un intérprete paso a paso orientado a
  depuración, con actualización de UI por instrucción; no publica cifras de
  rendimiento y correrlo exige Windows (si RT-5 habilita una máquina real,
  puede medirse ahí — nota pendiente). La experiencia práctica que motiva
  esta tesis (Cap. I): juegos en tiempo real tipo Tetris resultan inviables
  en emu8086, mientras que en EmuRV son un ejemplo de la lección 10 con
  margen de 26× en el peor caso.

## Limitaciones declaradas

- Cuenta de instrucciones **nominal** (bucles deterministas contados sobre el
  listado), no retirada por contador de hardware: idéntico criterio para los
  cuatro emuladores.
- Wall-time de proceso completo: incluye arranque (~ms) y, en EmuRV, el
  ensamblado del fuente; en QEMU, el boot de la máquina `virt`. A la escala
  de ≥0,5 s por corrida no altera el orden de magnitud de ninguna columna.
- QEMU corre en system-mode (los emuladores user-mode no existen en el brew
  de macOS); Spike y QEMU ejecutan ELF bare-metal, EmuRV y RARS el MISMO
  fuente ensamblador (syscalls compatibles).
- Microkernels sintéticos, no CoreMark: el port bare-metal de CoreMark por
  target quedó diferido por la compuerta de esfuerzo del PLAN-RT4 (F5) — la
  conclusión de orden de magnitud no cambia y el harness queda listo para
  agregarlo como trabajo futuro.
- Una sola máquina (Apple Silicon M4 Pro). Los cocientes entre emuladores
  son lo citable, no los absolutos.
