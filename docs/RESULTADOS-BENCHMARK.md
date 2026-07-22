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

| Kernel | EmuRV | QEMU (TCG) | Spike | RARS |
|---|---:|---:|---:|---:|
| aritmética (ALU pura) | **308,5** | 3 496,5 | 892,9 | 6,0 |
| memoria (lw/sw 4 KiB) | **139,5** | 4 090,9 | 951,9 | 4,8 |
| llamadas (call/ret + pila) | **162,6** | 1 069,3 | 950,0 | 5,5 |
| mixto (LCG + memoria + mul) | **129,5** | 4 304,3 | 964,9 | 5,5 |

## Lectura

- **EmuRV vs RARS (su categoría: emulador educativo RISC-V): 23–64× más
  rápido.** Es la comparación de posicionamiento: misma función didáctica,
  motores de distinta clase (JIT heredado de QEMU vía Unicorn vs intérprete
  Java).
- **EmuRV está en el mismo orden de magnitud que Spike**, el simulador ISS
  de referencia del ecosistema (×0,15–0,35), y 1–1,5 órdenes por debajo de
  QEMU puro. Ese costo es explicable y deliberado: EmuRV ejecuta por
  **tramos de 1 M instrucciones** para mantener la interrumpibilidad del
  teclado/depurador (TEC/RP), paga la publicación de estado didáctico y el
  wall-time incluye arranque de proceso + ensamblado del fuente. El techo
  interno del motor sin tramos, medido por RNF-03 en la misma máquina, es
  ~1 500 M instr/s (y ~327 M instr/s con tramos) — coherente con la columna
  EmuRV.
- **Dispersión:** EmuRV muestra la mayor varianza (p.ej. memoria mín 1,98 s
  / máx 6,86 s; el mínimo implica ráfagas de ~250 M instr/s) — calentamiento
  del JIT de Unicorn entre corridas. La mediana es la cifra reportada; el
  CSV trae mín/máx.
- **Cumplimiento RNF-03 (≥10 M instr/s): 13–30× por encima** del requisito
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
