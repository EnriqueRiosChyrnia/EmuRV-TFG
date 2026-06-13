# PLAN-RP12 — Extensiones F/D (coma flotante)

Punto RP-12 de `docs/ROADMAP-PRO.md`. Plan por fases con compuertas de verificación:
PROHIBIDO pasar a la fase siguiente con la compuerta en rojo.

## Contexto y evidencia previa (verificada por la coordinadora, 2026-06-12)

- Unicorn 2.1.5 expone los registros FP en los bindings: F0=57..F31=88, FCSR=141.
- Tabla RARS verificada (wiki oficial): PrintFloat=2 (fa0), PrintDouble=3 (fa0),
  ReadFloat=6 (→fa0), ReadDouble=7 (→fa0). RandFloat/RandDouble/diálogos NO
  entran (fuera de alcance, igual que en el repertorio entero).
- **gcc RISC-V quedó instalado en RP-11** (`riscv64-elf-gcc` vía brew, con `as` y
  `objdump`): los encodings F/D del ensamblador propio se verifican contra GNU
  — el oráculo externo que ENSAMBLADOR.md §2 lamentaba no tener. Usarlo.
- Capstone (RP-11) ya decodifica F/D → el desensamblador cubre las instrucciones
  nuevas gratis (verificar, no asumir).
- **Trampa conocida que el spike DEBE responder primero:** la espec privilegiada
  exige `mstatus.FS != Off` para ejecutar instrucciones FP — si QEMU/Unicorn
  arranca con FS=Off, todo fadd.s da instrucción ilegal. El spike determina si
  hay que inicializar mstatus.FS en Machine::new/load (y RP-8 dejó la
  infraestructura de CSRs para hacerlo y testearlo).
- Registros f en RV32 con D: 64 bits con NaN-boxing para valores single — el
  panel de UI debe interpretar eso (mostrar hex64 + valor decodificado).

## Alcance

- SÍ: RV32F y RV32D completos en el ensamblador (loads/stores, aritmética,
  fma, conversiones, comparaciones, sign-inject, fclass, fmv), registros f con
  nombres ABI (ft/fs/fa), syscalls 2/3/6/7, panel de registros flotantes,
  ejemplo nuevo, encodings verificados contra GNU as, docs.
- NO: RandFloat/RandDouble (43/44) y diálogos; redondeos explícitos por sufijo
  en v1 (rm = dyn fijo, como emite GNU por defecto — sufijos de rounding mode
  son error didáctico «todavía no soportado»); lección nueva de floats
  (propuesta para ampliar el currículo después — registrar).

## Diseño acordado (no re-litigar)

- **Ensamblador:** registros f0-f31 + ABI (ft0-11, fs0-11, fa0-7) en tabla
  propia (chequeo de choques nombre-entero/nombre-float con error didáctico:
  «fadd.s necesita registros f, usaste t0»); instrucciones F: flw/fsw,
  fadd.s/fsub.s/fmul.s/fdiv.s/fsqrt.s, fmin.s/fmax.s, fmadd.s/fmsub.s/
  fnmadd.s/fnmsub.s (formato R4 — funct2), fcvt.w.s/fcvt.wu.s/fcvt.s.w/
  fcvt.s.wu, fmv.x.w/fmv.w.x, feq.s/flt.s/fle.s, fsgnj.s/fsgnjn.s/fsgnjx.s,
  fclass.s; D análogas (.d) + fcvt.s.d/fcvt.d.s, fmv solo en RV32 vía memoria
  (fmv.x.d NO existe en RV32 — error didáctico que lo explique). Pseudos:
  fmv.s, fabs.s, fneg.s (sgnj-based), flw/fsw con etiqueta (patrón %hi/%lo de
  RP-5), ídem .d.
- **Oráculo GNU:** test de encoding que ensambla el repertorio completo con
  nuestro assembler Y con riscv64-elf-as (-march=rv32imfd) y compara palabra a
  palabra (script en tests/fixtures con Makefile, mismo patrón que rp11). Si
  el runner no tiene la toolchain, esos tests se marcan #[ignore] con
  explicación (CI macOS la tendrá si brew la instaló; la matriz experimental no).
- **Máquina:** mstatus.FS inicializado según hallazgo del spike (en load(),
  comentado con la cita de la espec); registers_f() → [u64; 32] + fcsr;
  write_reg_f (edición en vivo, guard de RP-7 reusado).
- **Syscalls:** 2/3 imprimen fa0 (formato Rust {} de f32/f64 — divergencia de
  formato con Java/RARS REGISTRADA como limitación conocida, con test que fija
  NUESTRO formato); 6/7 leen línea→parse con reintento didáctico (patrón
  read_int); NaN-boxing correcto al escribir fa0 en 6 (single en reg de 64).
- **UI:** panel «Registros F (flotante)» plegable (patrón panel CSRs de RP-8,
  cerrado = cero costo): nombre ABI, hex de 64, valor interpretado (si
  NaN-boxed → f32, sino f64), edición inline pausado (acepta decimal con punto,
  hex crudo con 0x). i18n.
- **Desensamblador:** verificar que capstone arch_riscv decodifica F/D (debería
  — es la misma arch); test con palabras F/D conocidas.

## Fase 1 — Spike FP en Unicorn

`tests/spike_fp.rs`: (1) ¿fadd.s codificada a mano ejecuta o da ILLEGAL con el
estado actual? (2) si ILLEGAL: setear mstatus.FS=Initial vía CSR (RP-8) y
reintentar — documentar el valor exacto necesario; (3) round-trip host:
reg_write(F0)→guest lee, guest escribe→reg_read(F1); (4) NaN-boxing: flw deja
los 32 altos en unos; (5) fadd.d funciona; (6) FCSR accesible.
**Compuerta F1:** todas respondidas en BITACORA [Seguro] + decisión de
inicialización de FS. Commit `rp12-fase-1: spike FP (mstatus.FS y registros)`.

## Fase 2 — Ensamblador F + D con oráculo GNU

Repertorio completo + tabla de registros f + errores didácticos + pseudos +
test palabra-a-palabra contra riscv64-elf-as + tests de ejecución (cargar dos
floats, operar, comparar bits exactos IEEE 754 — valores con representación
exacta para no pelear con redondeo: 1.5, 0.25, etc.) + fmv.x.d rechazado con
explicación + sufijo de rounding rechazado didáctico.
**Compuerta F2:** encodings == GNU palabra a palabra + suite + clippy.
Commit `rp12-fase-2: ensamblador F/D verificado contra GNU as`.

## Fase 3 — Syscalls float + panel UI + edición

Syscalls 2/3/6/7 (con NaN-boxing en 6) + panel Registros F plegable + edición
inline + write_reg_f con guard. Tests: print de 1.5/-0.0/NaN/inf (formato
fijado), read con basura→reintento didáctico, round-trip 6→2, panel verificado
a mano (BITACORA).
**Compuerta F3:** verdes + tsc + clippy. Commit `rp12-fase-3: syscalls float y panel de registros F`.

## Fase 4 — Ejemplo, disasm, docs y cierre

1. `ejemplos/13-flotantes.asm`: promedio de notas leídas con 6, imprime con 2
   — didáctico, comentado, en el menú de la app + test CLI.
2. Test de disasm sobre palabras F/D (capstone las cubre — verificar).
3. ENSAMBLADOR.md (sección F/D + tabla de registros f + nota del oráculo GNU
   ahora disponible), Ayuda (syscalls 2/3/6/7 + panel F), README, BITACORA.
4. Propuesta registrada: lección de floats para el currículo (no se hace acá).
5. ROADMAP-PRO y Cap. VI: NO tocar (coordinadora).
**Compuerta F4 (cierre):** cargo test + clippy AMBOS + tsc + ci-local
--skip-bundle + tauri build + bundle ≤ 30 MB.
Commit `rp12-fase-4: ejemplo flotantes y docs F/D`.

## Criterios de aceptación (del ROADMAP, precisados)

- f0–f31 ABI en el ensamblador, repertorio F y D completo (sin rm explícito). ✓ F2.
- Encodings verificados contra GNU as palabra a palabra. ✓ F2 (criterio NUEVO,
  habilitado por la toolchain de RP-11 — más fuerte que el original).
- Panel de registros flotantes con NaN-boxing bien interpretado. ✓ F3.
- Syscalls 2/3/6/7 RARS con formato documentado. ✓ F3.
- Ejemplo funcionando por UI y CLI; IEEE 754 testeado con valores exactos. ✓ F2/F4.
