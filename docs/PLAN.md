# PLAN — fases con compuertas de verificación

Regla general: ninguna compuerta (gate) se salta. Si un gate no pasa tras 3 intentos serios, documentá el bloqueo en BITACORA.md y avanzá con lo que no dependa de él.

## F0 — Bootstrap y spike de riesgo
- Scaffold Tauri 2 + TS, git init, estructura de carpetas según ARQUITECTURA.md.
- Verificar toolchain (rust, cmake, node).
- **Spike Keystone RV32IM (R1)**: decidir Keystone vs ensamblador propio. Registrar decisión.
- **Gate F0**: `cargo tauri dev` abre ventana vacía + decisión R1 registrada en BITACORA.md.

## F1 — Núcleo sin GUI (lo más importante del proyecto)
- assembler.rs (con source_map), machine.rs, syscalls.rs.
- Los 5 programas de ejemplo escritos y ensamblando.
- **Gate F1**: tests Rust que ensamblan y ejecutan los 5 ejemplos verificando estado final de registros y salida de consola (RNF-06). Todo verde.

## F2 — Puente Tauri + editor
- Comandos Tauri, editor CodeMirror con resaltado RV32, botones Ensamblar / Ejecutar / Paso / Reiniciar.
- **Gate F2**: hola mundo se ensambla y ejecuta desde la GUI mostrando salida en consola.

## F3 — Paneles de estado
- Vista de registros con resaltado de cambios, vista de memoria navegable, consola con entrada bloqueante (a7=5).
- **Gate F3**: el ejemplo interactivo (suma con entrada de usuario) funciona de punta a punta; fibonacci paso a paso muestra registros cambiando.

## F4 — Breakpoints y errores didácticos
- Breakpoints en margen del editor, línea actual resaltada durante paso a paso, errores de ensamblado en español anclados a línea.
- **Gate F4**: demo manual documentada en BITACORA.md (breakpoint en mitad de un bucle, inspección, continuar).

## F5 — Export ELF + ayuda
- elf_export.rs, botón "Exportar ELF", panel de ayuda con la tabla de syscalls y los ejemplos.
- **Gate F5**: `file` reporta ELF 32-bit RISC-V válido; si hay qemu-riscv32, validación de ejecución; test unitario del header ELF.

## F6 — Pulido y entrega
- i18n externalizado completo, icono mínimo, `cargo tauri build` (bundle .app), README.md de usuario (instalación + primer programa en 5 minutos).
- **Gate FINAL**: checklist RF-01..RF-10 en BITACORA.md, cada ítem con evidencia (nombre del test o descripción de verificación manual). `cargo test` verde, clippy limpio, bundle generado y peso registrado contra RNF-04.
