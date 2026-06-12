# ARQUITECTURA — decisiones técnicas

> **Nota de estado (actualizado 2026-06-10).** Este documento registra las decisiones de arquitectura *previstas* al planificar el proyecto. Dos de ellas cambiaron durante la implementación, con evidencia registrada en BITACORA.md. Las divergencias están marcadas en línea con el prefijo **⚠ DIVERGENCIA** y detalladas en `ENSAMBLADOR.md`. El resto del documento se mantiene como fue verificado. *(2026-06-12: la fase de teclado agregó notas fechadas en §Estructura del repositorio y §Diseños clave 3, y la sección nueva «Dispositivos emulados».)*

## Stack (decisiones cerradas del brief de tesis — no reabrir)

| Componente | Elección | Crate / nota |
|---|---|---|
| Núcleo de emulación | Unicorn Engine (RISC-V 32) | `unicorn-engine` |
| Ensamblador | ~~Keystone~~ → **ensamblador propio RV32IM** | ⚠ DIVERGENCIA — Keystone no soporta RISC-V (spike R1). Ver ENSAMBLADOR.md |
| Desensamblador | Capstone (no incorporado en v0.1) | ⚠ DIVERGENCIA — ningún RF lo requiere; queda designado para uso futuro |
| Shell de aplicación | Tauri 2.x | Rust backend + frontend web |
| Escritura de ELF | crate `object` (módulo write) | `goblin` solo lee; usar `object::write` |
| Editor | CodeMirror 6 | modo RISC-V propio, ver §Riesgos R3 |

## Estructura del repositorio

```
/src-tauri          Rust
  /src/core
    assembler.rs    ensamblado + source_map
    machine.rs      Unicorn: memoria, ejecución, hooks, breakpoints
    syscalls.rs     convención RARS (a7)
    elf_export.rs   ELF32 EM_RISCV con `object`
    video.rs        framebuffer (añadido en la fase framebuffer, 2026-06; no estaba en el plan original)
    keyboard.rs     cola del dispositivo de teclado MMIO (añadido en la fase de teclado, 2026-06-12)
  /src/commands.rs  comandos Tauri (assemble, run, step, reset, read_mem, export_elf, send_input)
  /tests            integración (RNF-06)
/src                frontend TS
  /editor  /paneles (registros, memoria, consola; video y ayuda añadidos en la fase framebuffer, 2026-06)  /i18n/es.json
/ejemplos           .asm de RF-09
/docs               esta carpeta
```

*(Corrección 2026-06-11: el árbol original ubicaba `/tests` en la raíz del repositorio; los tests de integración están y estuvieron siempre en `src-tauri/tests`, que es donde cargo los ejecuta.)*

*(Actualización 2026-06-12, fase de teclado: además de `core/keyboard.rs`, se sumaron en el frontend `src/teclado.ts` (módulo único de mapeo navegador→códigos del dispositivo, compartido por el panel embebido y `src/pantalla.ts`); en `src-tauri/tests` los spikes `spike_count.rs` y `spike_mmio.rs` y las suites `tec1_detener.rs`, `tec2_teclado.rs`, `tec3_tiempo.rs`, `tec5_eco.rs` y `tec5_tetris_rt.rs`; y en `/ejemplos` los programas `09-eco-teclado.asm` y `10-tetris-tiempo-real.asm`. `commands.rs` ganó el comando `push_key`. Ver `TECLADO.md` y BITÁCORA TEC0–TEC5.)*

## Diseños clave

1. **source_map**: el ensamblado produce `bytes + tabla línea↔dirección` (y dirección↔línea). Es la pieza que une los breakpoints del editor con el pc de Unicorn y la que permite resaltar la línea actual. Diseñarla primero: todo lo demás depende de ella.

2. **Mapa de memoria estilo RARS** (compatibilidad pedagógica con material existente en español):
   - `.text` base **0x0040_0000** (4 MB mapeados)
   - `.data` base **0x1001_0000** (4 MB; heap crece después de .data)
   - pila: **sp inicial = 0x7FFF_FFF0**, región de 8 MB mapeada debajo
   - e_entry del ELF exportado = 0x0040_0000

3. **Modelo de ejecución**: hilo Rust dedicado para la emulación; la UI recibe eventos Tauri (`paused`, `exited`, `output`, `error`, `registers_update`). "Paso" = `emu_start` con count=1 desde pc actual. "Ejecutar" = emu_start con hook de código que compara pc contra el set de breakpoints (usar hook solo si hay breakpoints activos; si no, correr libre — es la diferencia entre 1 M y 100 M instr/s).

   > **⚠ Nota (2026-06-12, fase de teclado).** "Ejecutar" ya no es una sola llamada larga a `emu_start`: corre en **tramos acotados por cantidad de instrucciones** (`RUN_SLICE_COUNT = 1_000_000` ≈ 2–3 ms por tramo, `Machine::run_slice`). Motivo: el timeout de Unicorn **no corta bucles apretados sin syscalls** (un `loop: j loop` dejaba «Detener» sin responder; hallazgo de la Etapa 4, resuelto en BITÁCORA TEC0/TEC1 — el count sí corta, con exactitud verificada y ~4,2 µs de overhead por re-entrada). La señal de stop y las órdenes auxiliares (breakpoints, lecturas de memoria) se atienden **entre tramos**, con latencia de «Detener» de ~2–3 ms; el refresco de registros a la UI quedó **desacoplado** del tramo, a cadencia de ~100 ms (emitir por tramo serían ~500 eventos/s). RNF-03 re-medido con slicing: 302,5–397,0 M instr/s. El criterio "hook de breakpoints solo cuando hay breakpoints" sigue vigente sin cambios.

4. **Syscalls**: hook de interrupción de Unicorn captura `ecall`, lee a7 y despacha. `read int` (a7=5): el hilo de emulación se bloquea en un canal esperando que la UI envíe input (comando `send_input`); la consola de la UI entra en modo entrada.

5. **Export ELF**: mismas secciones del ensamblado emitidas como ELF32 little-endian, `e_machine = EM_RISCV`, secciones .text/.data con sus direcciones del mapa. Validación mínima: `file salida.elf` debe reportar "ELF 32-bit LSB executable, UCB RISC-V". Si `qemu-riscv32` está instalado, validar ejecución real de hola mundo (con la salvedad de que las syscalls RARS no son syscalls Linux: para qemu, validar un ejemplo que termine con exit code, no con print).

## Dispositivos emulados (sección añadida 2026-06-12)

Además de la pantalla gráfica de la fase framebuffer (framebuffer + paleta + registro de control en `0x9000_0000`–`0x9003_0000`, ver `MEMORIA-GRAFICA.md`), la fase de teclado agregó:

- **Teclado MMIO** (página `0xFFFF_0000`, 4 KB): `RECEIVER_CONTROL` (+0, bit 0 = hay tecla; bit 1 reservado, sin interrupciones) y `RECEIVER_DATA` (+4, leerlo consume), **compatibles con RARS/MARS en direcciones y semántica** para reutilizar material didáctico existente. Mecanismo: hook `MEM_READ` permanente sobre los 8 bytes con página de respaldo real (el visor de memoria de la UI no consume teclas); cola de 64 con descarte del más viejo, alimentada por el comando `push_key` desde la UI. Documentación completa —mapa, semántica exacta, códigos, syscalls 30/32, decisiones del spike con cifras y limitaciones— en **`TECLADO.md`**.

## §Riesgos y planes B (aplicar sin pedir permiso, registrando en BITACORA.md)

- **R1 [RESUELTO — confirmado peor que lo previsto]** El soporte RISC-V de Keystone no es incompleto: es **inexistente** en toda release oficial (verificado en headers de Keystone 0.9.2: el enum `ks_arch` no contiene `KS_ARCH_RISCV`). **Plan B aplicado: ensamblador propio RV32IM** (lexer + parser + encoder R/I/S/B/U/J + pseudo-instrucciones + directivas). Resultado: mensajes de error en español de calidad didáctica (RF-02) y control total del source_map (RF-04). Documentado en ENSAMBLADOR.md y BITACORA.md.
- **R2 [RESUELTO — materializado distinto]** `unicorn-engine` compila la librería C: requiere cmake **y pkg-config**. La falta de pkg-config (no de cmake) produjo errores C desconcertantes (`Int128`, `PROT_EXEC`) porque el `configure` de qemu abortaba sin propagar el fallo. Solución: `brew install pkg-config`. Registrado en README como requisito de build.
- **R3 [Seguro]** No existe modo CodeMirror oficial para RISC-V: definir un `StreamLanguage` simple propio (regex de mnemónicos, registros ABI, directivas, etiquetas, comentarios `#`).
- **R4 [Suposición]** El bloqueo del hilo de emulación esperando input podría interactuar mal con el runtime de Tauri si se implementa en el hilo principal. Mitigación: la emulación SIEMPRE vive en su propio hilo, comunicación solo por canales.
