# BITÁCORA — EmuRV v0.1

Registro de decisiones técnicas del prototipo. Cada entrada incluye fecha, decisión, alternativas consideradas y nivel de confianza [Seguro / Probable / Suposición].

---

## Fase 0 — Bootstrap y spike de riesgo

### 2026-06-09 — Verificación de toolchain

| Herramienta | Estado | Versión |
|---|---|---|
| rustc / cargo | ✅ | 1.96.0 (estable, aarch64-apple-darwin) |
| cmake | ✅ | 4.3.3 (requisito de `unicorn-engine`, riesgo R2 descartado) |
| node / npm | ✅ | v24.11.1 / 11.6.2 |
| tauri-cli | instalado en F0 | `cargo install tauri-cli` |
| qemu-riscv32 | ❌ no disponible | ver nota F5 |

Nota: `qemu-riscv32` (emulación de modo usuario) no está empaquetado para macOS en Homebrew — el modo `linux-user` de QEMU solo se compila en hosts Linux. La validación de ejecución del ELF exportado con qemu (parte opcional del Gate F5: "si hay qemu-riscv32") quedará documentada como no aplicable en macOS; la validación se hará con `file` + test unitario del header + carga en herramientas alternativas si están disponibles. [Seguro]

### 2026-06-09 — SPIKE R1: Keystone NO soporta RISC-V → Plan B: ensamblador propio

**Decisión: descartar Keystone y construir un ensamblador RV32IM propio en Rust** (lexer + parser + encoder de formatos R/I/S/B/U/J + pseudo-instrucciones + directivas).

**Evidencia del spike** (duración real: ~5 minutos de los 45 presupuestados — el resultado fue categórico):

1. `brew install keystone` → instala Keystone **0.9.2_1** (última release oficial, oct. 2020).
2. Inspección de `$(brew --prefix keystone)/include/keystone/keystone.h`: el enum `ks_arch` contiene exactamente `ARM, ARM64, MIPS, X86, PPC, SPARC, SYSTEMZ, HEXAGON, EVM`. **No existe `KS_ARCH_RISCV`.**
3. `grep -ri riscv` sobre todos los headers instalados: **cero resultados**.
4. Bindings Rust disponibles (`keystone` 0.9.0, `keystone-engine` 0.1.0, `hexpatch-keystone` 0.9.4) envuelven la misma librería C, por lo que ninguno puede ofrecer una arquitectura que la librería no tiene.

El riesgo R1 de ARQUITECTURA.md preveía "soporte históricamente incompleto"; la realidad es peor: el soporte es **inexistente** en cualquier release. No fue necesario intentar ensamblar las 10 instrucciones de prueba: la arquitectura no se puede ni seleccionar. Decisión irreversible según contrato (decidir en fase 0 y no volver atrás).

**Alternativas consideradas:**
- *Keystone desde fork/PR no fusionado con RISC-V*: descartado — depender de un fork muerto sin releases es deuda técnica inaceptable para una tesis. [Seguro]
- *Invocar un toolchain GNU externo (`riscv64-elf-as`)*: descartado — rompe el requisito de app autocontenida, complica el bundle (<30 MB) y los mensajes de error serían los de GNU as en inglés. [Seguro]
- *Ensamblador propio (elegida)*: costo ~1 fase extra de trabajo, absorbido en F1. Beneficios directos para los RF: mensajes de error por línea/columna en español de calidad didáctica (RF-02) y control total del source_map (RF-04). [Seguro]

**Nivel de confianza: [Seguro]** — evidencia directa de headers de la última release oficial.

### 2026-06-09 — Frontend: Vite + TypeScript vanilla + CodeMirror 6 (sin framework)

**Decisión:** frontend en TypeScript estricto sin framework de componentes, con Vite como bundler y CodeMirror 6 como editor.

**Alternativas:** Svelte (permitido por CLAUDE.md) aportaría reactividad declarativa, pero la UI de EmuRV son 4 paneles con actualizaciones controladas por eventos de emulación — un patrón imperativo simple. Un framework agregaría peso al bundle (RNF-04) y una capa de indirección sin beneficio proporcional. CodeMirror 6 es la única pieza de UI compleja y es independiente de frameworks. [Probable]

### 2026-06-09 — Gate F0: SUPERADO

- Scaffold Tauri 2 + Vite + TS vanilla generado con `create-tauri-app` (identificador `py.edu.fiuni.emurv`, producto `EmuRV`).
- Estructura de carpetas de ARQUITECTURA.md creada (`src-tauri/src/core`, `src/editor`, `src/paneles`, `src/i18n`, `ejemplos/`).
- `cargo tauri dev`: compilación limpia (347 crates) y proceso `emurv` corriendo como aplicación GUI en primer plano (verificado vía System Events: aparece en la lista de procesos con `background only = false`, es decir, con ventana). La inspección del título exacto de la ventana requiere permisos de accesibilidad de macOS que no corresponde otorgar en una sesión autónoma; la evidencia de proceso GUI activo + arranque sin errores se considera suficiente. [Seguro]
- Decisión R1 registrada arriba. Gate F0 cumplido.

