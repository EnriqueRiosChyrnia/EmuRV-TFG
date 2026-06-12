# SPEC — EmuRV v0.1 (MVP macOS)

Nombre provisional: **EmuRV** (se definirá nombre final en fase de pulido; no gastar tiempo en esto).
Usuario objetivo: estudiante de FIUNI sin experiencia previa en ensamblador.
Principio rector: UX familiar al ciclo de emu8086 — **editar → ensamblar → ejecutar / paso a paso → observar registros y memoria** — con respuesta instantánea.

## Requisitos funcionales (RF)

- **RF-01** Editor de código ensamblador RV32 con resaltado de sintaxis y números de línea.
- **RF-02** Ensamblado de **RV32IM** con reporte de errores **por línea y en español** (línea, columna si es posible, mensaje claro para principiante).
- **RF-03** Ejecución completa ("Ejecutar") y paso a paso ("Paso") sobre Unicorn Engine.
- **RF-04** Breakpoints por línea, toggle en el margen del editor; la ejecución continua se detiene al alcanzarlos y resalta la línea actual.
- **RF-05** Vista de registros x0–x31 + pc: nombres ABI (zero, ra, sp, gp, tp, t0–t6, s0–s11, a0–a7), valores en hex y decimal, **resaltado de los registros modificados en el último paso**.
- **RF-06** Vista de memoria: hexdump navegable por dirección, accesos rápidos a .text, .data y zona de pila (sp).
- **RF-07** Consola integrada con syscalls estilo RARS vía `ecall` (selector en a7): 1=print int, 4=print string (NUL-terminated en a0), 5=read int, 10=exit, 11=print char. Subconjunto mínimo, documentado en la ayuda integrada.
- **RF-08** Exportar ELF: generar un **ELF32 RISC-V estándar** del programa ensamblado, ejecutable en `qemu-riscv32` y cargable en otras herramientas (p. ej. Ripes).
- **RF-09** Ejemplos incluidos (mínimo 5, en español): hola mundo, suma con entrada de usuario, bucle/contador, fibonacci, uso de pila/funciones. Accesibles desde un menú "Ejemplos".
- **RF-10** Botón "Reiniciar": vuelve la máquina al estado post-ensamblado (registros y memoria iniciales).

> **Nota (2026-06-12, RP-1 del roadmap):** la instrucción `ebreak` queda soportada como **pausa por software** (complemento de RF-03/RF-04): pausa la ejecución mostrando el estado y al reanudar se continúa en la instrucción siguiente, convención RARS/gdb. Declarado recién después del fix correspondiente (BITACORA RP1, tests `rp1_ebreak.rs`).

## Requisitos no funcionales (RNF)

- **RNF-01** Arranque de la app < 2 s en Apple Silicon.
- **RNF-02** Ensamblar y cargar un programa de 500 líneas < 200 ms.
- **RNF-03** Ejecución continua ≥ 10 M instr/s efectivas. Unicorn lo entrega de sobra: el riesgo real es la UI. Las vistas se refrescan solo al pausar, terminar, alcanzar breakpoint o a intervalo fijo (~10 Hz) durante ejecución larga; **jamás** por instrucción.
- **RNF-04** Bundle macOS < 30 MB.
- **RNF-05** Toda la UI en español; textos en archivos i18n.
- **RNF-06** Un test de integración por cada ejemplo de RF-09: ensamblar + ejecutar + verificar estado final de registros/salida.

## Explícitamente FUERA de alcance en v0.1

- Visualización de datapath/pipeline (decisión estratégica de tesis: ese terreno es de Ripes; no competimos ahí).
- RV64, extensiones F/D/A/C.
- Carga/depuración de ELF externos (solo export).
- Modo gráfico/VGA, interrupciones de hardware, MMIO más allá de la consola.
- Builds de Windows/Linux (solo dejar la configuración multiplataforma de Tauri intacta).
- Autenticación, telemetría, actualizaciones automáticas.
