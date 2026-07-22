# PLAN-RT4 — Benchmark de rendimiento vs QEMU, Spike, RARS + caso Tetris vs emu8086

Punto RT-4 de `docs/ROADMAP-PRO.md` (fase de cierre de tesis). Plan por fases
con compuertas: PROHIBIDO pasar a la siguiente con la compuerta en rojo.

**Prerrequisito:** RT-2 y RT-3 COMPLETOS (2026-07-22). Cumplido.

**Destino inmediato:** tabla citable del Cap. V que ubique el rendimiento de
EmuRV en el mapa de emuladores: contra los emuladores de sistema serios
(QEMU, Spike), contra el emulador educativo de referencia RISC-V (RARS) y
contra el emulador que EmuRV viene a reemplazar (emu8086), más el caso de
aplicación en tiempo real (Tetris). Tesis del capítulo: EmuRV hereda
rendimiento de clase QEMU (su motor Unicorn ES el JIT de QEMU) estando en la
categoría didáctica de RARS/emu8086 — órdenes de magnitud por encima de ellos.

## Alcance

- SÍ: kernels de benchmark propios (mismo algoritmo en todos los targets),
  medición reproducible por script (repeticiones + mediana), comparadores
  QEMU user-mode, Spike y RARS en esta máquina (macOS ARM, misma condición
  para todos), caso Tetris con la instrumentación FPS existente, comparación
  documental con emu8086, docs + BITACORA.
- NO: correr emu8086 (requiere Windows; comparación documental con fuentes,
  y si RT-5 da acceso a máquina Windows real se podrá medir ahí — nota en el
  doc); benchmarks de la UI completa (el objeto es el motor); CoreMark/Embench
  quedan OPCIONALES con compuerta de esfuerzo (F5): el port bare-metal exige
  harness de clock/printf por target y no cambia la conclusión de orden de
  magnitud.

## Diseño acordado (no re-litigar)

### Metodología
- **Métrica principal: M instr/s (MIPS)** — wall-time de workloads con cuenta
  de instrucciones NOMINAL conocida (bucles deterministas: N iteraciones ×
  instrucciones/iteración, contadas sobre el listado). Así no dependemos del
  contador interno de cada emulador (QEMU user-mode no expone uno).
- **Mismo algoritmo en todos los targets.** El núcleo de cada kernel es un
  .s compartido; cada target lo envuelve con su convención de arranque/salida:
  - **EmuRV**: `emurv-cli run` (syscalls estilo RARS) — camino headless real
    del producto.
  - **RARS**: EL MISMO fuente que EmuRV (los syscalls de EmuRV son
    compatibles RARS a propósito) — comparación limpia sin reescritura.
  - **QEMU (`qemu-riscv32` user-mode)** y **Spike**: wrapper bare-metal
    mínimo (entrada + `exit` por syscall Linux / HTIF tohost). Compilados
    con riscv64-elf-gcc (RT-2/RT-3).
- **Medición:** N ≥ 5 repeticiones, mediana, dispersión reportada; workloads
  dimensionados para ≥ 1 s en el emulador MÁS RÁPIDO (≥ ~500M instr).
  Se registra hardware/OS/versiones exactas.
- **Honestidad metodológica en el doc:** EmuRV corre por tramos
  (interrumpibilidad del teclado, RP/TEC) — se mide en esa condición real, no
  en un modo "benchmark" artificial; QEMU/Spike no pagan ese costo y se dice.

### Kernels (4)
1. `aritmetica` — enteros en registros (add/mul/xor dependientes), sin memoria.
2. `memoria` — recorrido y suma de arreglo (lw/sw estilo criba/copia).
3. `llamadas` — call/ret con pila (fibonacci recursivo acotado o similar).
4. `mixto` — criba de Eratóstenes o fibonacci iterativo grande (mezcla real).

Verificación de equivalencia (compuerta F2): cada kernel imprime/retorna un
resultado final que debe coincidir EXACTO en los 4 targets, y la cuenta
nominal de instrucciones se documenta por kernel.

### Comparadores (versiones pineadas al ejecutar)
- QEMU: `brew install qemu` → `qemu-riscv32` (user-mode; TCG JIT, la misma
  tecnología que Unicorn/EmuRV — comparación motor vs motor).
- Spike: `riscv-isa-sim` (brew tap riscv-software-src o build cmake) —
  intérprete de referencia.
- RARS: jar de release pineada (1.6) con Java presente (OpenJDK 25); se mide
  en modo consola `java -jar rars.jar nc <archivo.asm>` y se descuenta el
  arranque de la JVM (corrida con programa vacío).
- emu8086: documental — manual/fuentes sobre su naturaleza interpretada y
  velocidad; tabla lo ubica con esa salvedad explícita.

### Caso Tetris (tiempo real)
- EmuRV: `ejemplos/10-tetris-tiempo-real.asm` con la instrumentación FPS de
  RP/FB (fb4_fps) — FPS sostenidos y MIPS durante juego real.
- emu8086: el caso equivalente (juego en tiempo real con teclado) es
  IMPOSIBLE o degradado en emu8086 por su velocidad interpretada — argumento
  central de la motivación de la tesis, documentado con fuentes y con la
  experiencia registrada en el Cap. I/II.

## Fases

### F1 — Comparadores instalados y humo
- brew qemu; Spike (tap o source); RARS 1.6 jar (sha256 anotado); hyperfine
  (o `time` + script propio si hyperfine no aporta).
- Humo: cada emulador corre un hola-mundo/bucle trivial de su target.
- **Compuerta:** los 3 comparadores ejecutan y terminan con código 0.

### F2 — Kernels + wrappers + equivalencia
- `tests-riscv/bench/` (o `bench/`): núcleos .s compartidos + wrappers por
  target + Makefile/script de build.
- **Compuerta:** resultado final idéntico en los 4 targets por kernel;
  cuenta nominal de instrucciones documentada.

### F3 — Harness de medición + corrida completa
- `scripts/bench-rt4.sh`: repeticiones, mediana, CSV + tabla markdown;
  registra versiones y hardware.
- **Compuerta:** CSV completo (4 kernels × 4 emuladores) con dispersión
  razonable (mediana estable); sanity: EmuRV ≥ RNF-03 (10 MIPS) por margen.
- Corrida en condiciones controladas (enchufado, sin carga; se anota).

### F4 — Caso Tetris / tiempo real
- Medir FPS/MIPS del Tetris RT en EmuRV (instrumentación existente, corrida
  automatizada si el harness fb4 lo permite; si exige interacción humana se
  hace una corrida manual documentada con captura).
- Sección emu8086 documental con fuentes.
- **Compuerta:** cifras de EmuRV registradas + comparación documental con
  citas verificables.

### F5 — (OPCIONAL, compuerta de esfuerzo) CoreMark
- Intentar port bare-metal (harness clock por rdcycle/rdtime o contador del
  wrapper). SI el port supera ~medio día de trabajo o exige tocar el core:
  se difiere con nota (los kernels ya dan la conclusión de orden de
  magnitud) — sin culpa, está previsto acá.
- **Compuerta:** CoreMark/s en ≥ 2 emuladores o diferimiento documentado.

### F6 — Cierre
- `docs/RESULTADOS-BENCHMARK.md` → tabla MIPS por kernel × emulador,
  gráfico si aporta, caso Tetris, limitaciones (single-run macOS ARM, wall
  time, cuenta nominal), reproducción.
- BITACORA (decisiones + números clave), ROADMAP-PRO RT-4 → COMPLETO,
  commits `rt4-fase-M: ...` (sin trailer co-autor), CI verde (nada de esto
  entra a CI — solo docs y scripts).
- **Compuerta:** docs sincronizadas, CI verde.

## Riesgos y plan B

| Riesgo | Plan B |
|---|---|
| Spike no compila en macOS ARM | Docker linux/arm64 solo para Spike (ya hay Docker); se anota la asimetría de plataforma |
| qemu-riscv32 user-mode: syscalls Linux en wrapper bare | Wrapper con `ecall` Linux mínimo (write/exit) — 20 líneas; alternativa qemu-system con semihosting |
| RARS demasiado lento para 500M instr | Workload reducido para RARS con factor documentado (misma métrica MIPS, menos iteraciones) |
| Arranque JVM distorsiona a RARS | Medición con programa vacío restada; se reporta bruto y neto |
| Variabilidad wall-time en laptop | Mediana de ≥5, condiciones anotadas, dispersión publicada |
| El contador de tramos de EmuRV penaliza vs QEMU puro | Se reporta como está (condición real del producto) y se explica el porqué didáctico |
