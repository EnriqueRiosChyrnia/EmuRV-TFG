# PLAN-RP11 — Carga de ELF externos + desensamblador

Punto RP-11 de `docs/ROADMAP-PRO.md`. Plan por fases con compuertas de verificación:
PROHIBIDO pasar a la fase siguiente con la compuerta en rojo.

## Contexto y evidencia previa (verificada por la coordinadora, 2026-06-12)

- `object = 0.39.1` ya es dependencia (default-features = false, features
  write_core/elf/std) — la LECTURA de ELF es agregar `read_core` al mismo crate:
  cero dependencias nuevas para el import.
- `capstone` 0.14.0 (crates.io, actualizado 2026-01) existe y está mantenido.
  **Riesgo RNF-04:** la lib C de Capstone con todas las arquitecturas pesa
  varios MB; la app va en 22/30 MB. El spike F1 MIDE el delta real (con
  arch-only riscv si el build lo permite) ANTES de comprometerse.
- **Plan B del desensamblador (si Capstone revienta el presupuesto):** decoder
  propio RV32IM+Zicsr — el proyecto YA tiene el ensamblador de ese subconjunto;
  el decoder es el espejo (tabla finita, salida en español, pseudo-instrucciones
  reconocibles). Más trabajo, cero deps, cero riesgo de peso, mensajes
  consistentes con el resto de la herramienta. La decisión la toma la evidencia
  del spike BAJO LA REGLA de conflictos del ROADMAP (RNF-04: nadie afloja el
  límite — si ambas vías fallan, FRENAR y presentar al autor).
- Sin gcc RISC-V local (solo riscv64-elf-gdb). Fixtures de import:
  1. PRIMARIA (CI, siempre): round-trip — `emurv-cli asm` exporta ELF (con
     .symtab desde RP-6) → importarlo → correr → misma salida que el fuente.
  2. OPCIONAL: si `brew install riscv64-elf-gcc` funciona en la máquina,
     compilar UNA fixture real con `-march=rv32im -mabi=ilp32
     -Ttext=0x00400000 -Tdata=0x10010000 -nostdlib` y commitearla como asset
     binario de test con su fuente y Makefile de regeneración. Si brew falla,
     registrar y seguir — la round-trip cubre el contrato.
- Mapa de memoria: .text 0x0040_0000 (+4 MB), .data 0x1001_0000 (región desde
  0x1000_0000 +4 MB), heap 0x1004_0000 (RP-10). Un ELF de gcc con linker script
  por defecto NO cae en esas direcciones — el error didáctico debe decir
  exactamente qué flags de linker usar (los de arriba).
- gdb stub (RP-6) ya depura ELFs propios; sobre un ELF importado debe funcionar
  igual (test en F2).

## Alcance

- SÍ: cargar ELF32 RISC-V (UI y CLI), validaciones didácticas, símbolos del
  .symtab si existen, «modo binario» honesto en la UI, desensamblador (Capstone
  o propio según spike) en panel de memoria + subcomando CLI, docs.
- NO: relocaciones/PIE (solo ET_EXEC con direcciones absolutas), DWARF/líneas
  de fuente, RV64 o extensiones fuera de RV32IM+Zicsr (instrucción no decodificable
  se muestra como `.word 0x...` — el emulador la ejecutará o no según Unicorn),
  edición del binario cargado.

## Diseño acordado (no re-litigar; el spike decide SOLO capstone-vs-propio)

### Carga (núcleo)
- `core/elf_import.rs`: `import_elf(bytes) -> Result<LoadedProgram, String>`
  donde LoadedProgram reusa `Program` (text/data/symbols) + flag `from_binary:
  bool` (sin source_map: vacío). Validaciones con error didáctico cada una:
  magic/clase 32/little-endian/EM_RISCV/ET_EXEC; segmentos PT_LOAD dentro de
  las regiones .text/.data del mapa RARS (fuera → error con los flags de linker
  correctos); entry == TEXT_BASE o, si difiere pero cae en .text, aceptar y
  registrar (Unicorn arranca donde diga e_entry — Machine::entry respeta el
  override). .symtab → Program.symbols (FUNC y OBJECT globales).
- `Machine` NO cambia: recibe un Program como siempre (esa es la prueba de que
  la abstracción de RP-3/RP-6 estaba bien cortada). Lo único nuevo: entry
  variable.

### Modo binario (UI)
- «Abrir» acepta `.elf` (y detección por magic si no tiene extensión): el editor
  pasa a SOLO LECTURA mostrando el listado desensamblado de .text (una línea
  por instrucción: dirección, bytes, mnemónico — generado al cargar), con
  banner «binario importado: sin código fuente». Ensamblar deshabilitado;
  Ejecutar/Paso/Detener/Reiniciar funcionan; breakpoints se ponen sobre las
  LÍNEAS del listado (cada línea = una dirección — el mapeo línea↔addr del
  listado reemplaza al source_map; los breakpoints condicionales y watchpoints
  de RP-7 funcionan igual).
- CLI: `emurv-cli run programa.elf` (detección por magic), `emurv-cli debug
  programa.elf` (gdb sobre binario), `emurv-cli disasm programa.elf` (listado
  a stdout).

### Desensamblador
- Interfaz única `disasm(words: &[u32], base: u32) -> Vec<DisasmLine>` para que
  la elección capstone/propio quede encapsulada en un módulo.
- Panel de memoria: columna nueva de instrucción decodificada SOLO cuando la
  dirección cae en .text (para fuentes normales también — el beneficio es
  general, no solo modo binario).

## Fase 1 — Spike: Capstone vs decoder propio + PoC de lectura ELF

1. Rama de prueba del spike (descartable): agregar capstone, desensamblar 10
   palabras RV32IM conocidas, MEDIR: delta de peso del bundle de la app
   (scripts/check-bundle-size.sh antes/después con `cargo tauri build`), delta
   de tiempo de compilación, ¿se puede compilar arch-only riscv?
2. PoC `object` read: parsear el ELF de round-trip, listar segmentos/símbolos.
3. ¿`brew install riscv64-elf-gcc` anda? (para la fixture opcional).
4. DECISIÓN registrada en BITACORA [Seguro] con los números: capstone si bundle
   ≤ 30 MB con margen ≥ 3 MB; si no, decoder propio; si el decoder propio
   resultara inviable por algo imprevisto, FRENAR (regla de conflictos).

**Compuerta F1:** números medidos + decisión fundamentada en BITACORA + spike
limpio (la rama de prueba no se mergea; lo que entra es la decisión).
Commit `rp11-fase-1: spike capstone vs decoder propio (medición de peso)`.

## Fase 2 — Import de ELF (núcleo + CLI)

`core/elf_import.rs` + feature read de object + entry variable en Machine +
CLI run/debug de .elf. Tests: round-trip completo (asm→ELF→import→correr→misma
salida, con símbolos preservados); cada validación didáctica (magic malo, clase
64, big-endian, otra máquina, segmento fuera de región CON los flags de linker
en el mensaje, ET_DYN rechazado); gdb stub sobre ELF importado (breakpoint por
símbolo del .symtab importado); fixture GNU si F1 la consiguió.
**Compuerta F2:** verdes + suite + clippy ambos.
Commit `rp11-fase-2: import de ELF32 RISC-V con validaciones didácticas`.

## Fase 3 — Desensamblador + modo binario

Módulo disasm según decisión F1; listado en editor read-only + breakpoints por
línea del listado; columna en panel de memoria; `emurv-cli disasm`. Tests:
desensamblado de las ~40 instrucciones del repertorio contra valores conocidos
(las mismas palabras que ensambla nuestro assembler — round-trip de mnemónicos);
palabra inválida → `.word 0x...` sin pánico; breakpoint en línea del listado
detiene en la dirección correcta; salida de `disasm` estable (test de proceso).
**Compuerta F3:** verdes + tsc + clippy + **bundle ≤ 30 MB re-verificado** (la
compuerta del peso es LA compuerta de esta fase).
Commit `rp11-fase-3: desensamblador y modo binario`.

## Fase 4 — Docs y cierre

README (abrir binarios, flags de linker para gcc, subcomando disasm),
docs/DEPURACION.md y DEPURACION-GDB.md (modo binario), Ayuda, BITACORA
consolidada. ROADMAP-PRO y Cap. VI: NO tocar (coordinadora).
**Compuerta F4 (cierre):** cargo test + clippy AMBOS + tsc + ci-local
--skip-bundle + tauri build + bundle ≤ 30 MB.
Commit `rp11-fase-4: docs import y desensamblador`.

## Criterios de aceptación (del ROADMAP, precisados)

- Cargar y correr un ELF round-trip SIEMPRE; uno de gcc real si la toolchain se
  pudo instalar (sino: fixture pendiente documentada con su Makefile listo). ✓ F2.
- Desensamblado verificado contra el repertorio propio (y contra objdump si la
  toolchain apareció). ✓ F3.
- Depuración por dirección + gdb stub sobre el binario cargado. ✓ F2/F3.
- Decisión capstone-vs-propio tomada por medición, con RNF-04 intocable. ✓ F1/F3.
