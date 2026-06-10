# ARQUITECTURA — decisiones técnicas

## Stack (decisiones cerradas del brief de tesis — no reabrir)

| Componente | Elección | Crate / nota |
|---|---|---|
| Núcleo de emulación | Unicorn Engine (RISC-V 32) | `unicorn-engine` |
| Ensamblador | Keystone | ver §Riesgos R1 — verificar ANTES de todo |
| Desensamblador | Capstone | `capstone` |
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
  /src/commands.rs  comandos Tauri (assemble, run, step, reset, read_mem, export_elf, send_input)
/src                frontend TS
  /editor  /paneles (registros, memoria, consola)  /i18n/es.json
/ejemplos           .asm de RF-09
/docs               esta carpeta
/tests              integración (RNF-06)
```

## Diseños clave

1. **source_map**: el ensamblado produce `bytes + tabla línea↔dirección` (y dirección↔línea). Es la pieza que une los breakpoints del editor con el pc de Unicorn y la que permite resaltar la línea actual. Diseñarla primero: todo lo demás depende de ella.

2. **Mapa de memoria estilo RARS** (compatibilidad pedagógica con material existente en español):
   - `.text` base **0x0040_0000** (4 MB mapeados)
   - `.data` base **0x1001_0000** (4 MB; heap crece después de .data)
   - pila: **sp inicial = 0x7FFF_FFF0**, región de 8 MB mapeada debajo
   - e_entry del ELF exportado = 0x0040_0000

3. **Modelo de ejecución**: hilo Rust dedicado para la emulación; la UI recibe eventos Tauri (`paused`, `exited`, `output`, `error`, `registers_update`). "Paso" = `emu_start` con count=1 desde pc actual. "Ejecutar" = emu_start con hook de código que compara pc contra el set de breakpoints (usar hook solo si hay breakpoints activos; si no, correr libre — es la diferencia entre 1 M y 100 M instr/s).

4. **Syscalls**: hook de interrupción de Unicorn captura `ecall`, lee a7 y despacha. `read int` (a7=5): el hilo de emulación se bloquea en un canal esperando que la UI envíe input (comando `send_input`); la consola de la UI entra en modo entrada.

5. **Export ELF**: mismas secciones del ensamblado emitidas como ELF32 little-endian, `e_machine = EM_RISCV`, secciones .text/.data con sus direcciones del mapa. Validación mínima: `file salida.elf` debe reportar "ELF 32-bit LSB executable, UCB RISC-V". Si `qemu-riscv32` está instalado, validar ejecución real de hola mundo (con la salvedad de que las syscalls RARS no son syscalls Linux: para qemu, validar un ejemplo que termine con exit code, no con print).

## §Riesgos y planes B (aplicar sin pedir permiso, registrando en BITACORA.md)

- **R1 [Probable]** El soporte RISC-V de Keystone es históricamente incompleto y sus bindings Rust están desactualizados. **Spike obligatorio en fase 0 (máx. 45 min)**: intentar ensamblar 10 instrucciones RV32IM representativas (incluyendo pseudo-instrucciones `li`, `la`, `j`) con Keystone. Si falla o el soporte es parcial → **Plan B: ensamblador propio RV32IM** (lexer + parser + encoder de los formatos R/I/S/B/U/J + pseudo-instrucciones + directivas .text/.data/.word/.asciiz/.space). Costo: ~1 fase extra. Beneficio colateral real para la tesis: mensajes de error en español de calidad superior y control total del source_map. Decidir en fase 0 y NO volver atrás.
- **R2 [Probable]** `unicorn-engine` compila la librería C: requiere cmake y toolchain. Verificar en fase 0 (`brew install cmake` si falta).
- **R3 [Seguro]** No existe modo CodeMirror oficial para RISC-V: definir un `StreamLanguage` simple propio (regex de mnemónicos, registros ABI, directivas, etiquetas, comentarios `#`).
- **R4 [Suposición]** El bloqueo del hilo de emulación esperando input podría interactuar mal con el runtime de Tauri si se implementa en el hilo principal. Mitigación: la emulación SIEMPRE vive en su propio hilo, comunicación solo por canales.
