# PLAN-RP9 — Currículo educativo progresivo (lecciones)

Punto RP-9 de `docs/ROADMAP-PRO.md`. Plan por fases con compuertas de verificación:
PROHIBIDO pasar a la fase siguiente con la compuerta en rojo.

**Prerrequisito:** RP-8 DOCUMENTADO (la lección 10 usa traps/CSRs y el ejemplo
12-trap-handler.asm). NO arrancar este plan antes.

**Destino inmediato:** estas lecciones son el guion de prueba del tester externo
(primo del autor) — cada lección debe poder seguirse SIN ayuda del autor: el
texto de la lección es el único docente presente.

## Alcance

- SÍ: estructura `lecciones/`, 10 lecciones con formato fijo, índice con mapa de
  aprendizaje, tests CLI de cada lección, referencias cruzadas con los ejemplos
  existentes, docs.
- NO: UI de lecciones (RP-15, fase visual), gamificación, progreso persistido,
  traducciones. Los ejemplos existentes NO se mueven ni renombran (la UI los
  embebe — romperlos rompe la app).

## Diseño acordado (no re-litigar)

### Estructura
```
lecciones/
  README.md            ← índice: mapa de aprendizaje, orden, prerrequisitos
                          por lección, tabla qué-aprendés-dónde
  01-registros/
    leccion.s          ← EL archivo que el estudiante abre y corre
  02-aritmetica/leccion.s
  ...
  08-multiarchivo/     ← esta tiene main.s + utiles.s + constantes.s
  ...
  10-traps/leccion.s
```

### Formato fijo de `leccion.s` (plantilla obligatoria, igual en las 10)
```asm
# ==========================================================
# Lección NN — TÍTULO
# ==========================================================
# OBJETIVOS — al terminar esta lección vas a poder:
#   * ...
#   * ...
# PANELES PARA OBSERVAR: <cuáles paneles de EmuRV mirar y qué buscar>
# CÓMO USARLA: Ensamblá y ejecutá PASO A PASO (botón «Paso»),
#   leyendo los comentarios. Cada bloque te dice qué mirar.
# ----------------------------------------------------------
<programa, bloques cortos, comentario ANTES de cada bloque>
# ----------------------------------------------------------
# EJERCICIOS (modificá el código y volvé a ensamblar):
#   1. <variante concreta — qué cambiar y qué debería pasar>
#   2. ...
#   3. (desafío) ...
# ==========================================================
```
Reglas: ≤ ~120 líneas de programa por lección (sin contar ejercicios/cabecera);
salida determinista por stdout (testeable); terminología EXACTA de la UI de
EmuRV (los nombres de los paneles y botones como aparecen en es.json);
vocabulario para principiante absoluto, español rioplatense neutro como el
resto del proyecto.

### Las 10 lecciones (contenido fijado)
1. **Registros y movimiento de datos** — li, mv, la; los 32 registros y la
   convención de nombres ABI; imprimir un entero. Observar: panel REGISTROS,
   resaltado de cambios al dar Paso.
2. **Aritmética y lógica** — add/sub/mul/div/rem, and/or/xor, sll/srl/sra;
   complemento a 2 DEMOSTRADO (0x7FFFFFFF + 1 ante tus ojos); diferencia
   srl/sra con número negativo.
3. **Memoria** — .data, .word/.byte/.asciiz, lw/sw/lb/lbu/sb; endianness
   demostrado (escribir word, leer sus 4 bytes); panel MEMORIA con navegación
   a .data (0x10010000).
4. **Condicionales** — beq/bne/blt/bge (+ versiones u); construir if/else;
   máximo de dos números; por qué no hay «if» en asm.
5. **Bucles** — while/for con saltos hacia atrás; suma 1..N; bucle anidado
   (tablita de multiplicar); breakpoint CONDICIONAL como herramienta (parar
   cuando t0 == 5 — usa RP-7 didácticamente).
6. **Funciones y pila** — jal/jalr/ret, ra, convención a0-a7, sp y el frame;
   prólogo/epílogo; una función hoja y una que llama a otra (por qué se
   guarda ra); referencia al ejemplo 05-pila-funciones para profundizar
   (factorial recursivo). Observar: pila en el panel MEMORIA (botón pila).
7. **Syscalls y E/S** — tabla RARS (1, 4, 5, 10, 11) + tiempo (30, 32);
   programa interactivo con validación; el ciclo leer-procesar-imprimir.
   NOTA: solo syscalls EXISTENTES hoy — nada de 8/9/93 (eso es RP-10; cuando
   exista, esta lección se amplía).
8. **Constantes, includes y macros** — mini proyecto multi-archivo (.eqv +
   .include + una macro con parámetros); por qué dividir el código; el
   repertorio RP-5 completo en acción. Referencia al ejemplo 11-multiarchivo.
9. **E/S proyectada en memoria y tiempo real** — el teclado MMIO (0xFFFF0000,
   CONTROL/DATA, leer-consume) + framebuffer (pintar un par de píxeles) +
   syscall 32 para animar. El programa hace poll con límite de vueltas y
   TERMINA SOLO con salida determinista si nadie teclea (testeable headless;
   con la app abierta, tecleás y reacciona). Referencias a ejemplos 07-10.
10. **Traps e interrupciones (avanzada)** — qué es una excepción; mtvec/mepc/
    mcause; instalar un handler, dispararlo con una ecall desconocida, volver
    con mret (sobre la base del ejemplo 12-trap-handler.asm de RP-8); panel
    CSRs en modo avanzado. Cerrar con: esto es la semilla de un sistema
    operativo.

### Tests (CI)
`emurv-cli/tests/lecciones.rs`: cada lección corre por CLI con stdin fijo donde
aplique y stdout EXACTO esperado; lección 9 con su salida determinista sin
teclas; lección 10 igual (el handler corre solo). Las lecciones quedan así
vigiladas por la CI para siempre: si un cambio futuro rompe una lección, la
compuerta lo caza.

## Fase 1 — Estructura + lecciones 1-3

Plantilla, índice esqueleto, lecciones 1-3 completas con tests.
**Compuerta F1:** 3 lecciones corren por CLI con salida exacta; revisión de
calidad didáctica contra la regla de formato (autorrevisión documentada en
BITACORA: ¿un principiante absoluto puede seguirla solo?); suite + clippy.
Commit `rp9-fase-1: estructura y lecciones 1-3`.

## Fase 2 — Lecciones 4-7

**Compuerta F2:** ídem F1 para las 4 nuevas; la lección 5 demuestra el
breakpoint condicional en sus instrucciones (texto, no solo código).
Commit `rp9-fase-2: lecciones 4-7`.

## Fase 3 — Lecciones 8-9 + índice completo

**Compuerta F3:** lección 8 multi-archivo corre por CLI; lección 9 determinista
headless Y verificada a mano con la app (teclear → reacciona — anotar en
BITACORA); README de lecciones completo (mapa, prerrequisitos, tabla).
Commit `rp9-fase-3: lecciones 8-9 e índice`.

## Fase 4 — Lección 10 + integración + docs

1. Lección 10 sobre el material de RP-8.
2. README del repo: sección «Aprendé RISC-V con EmuRV» enlazando lecciones/.
3. Ayuda integrada: mención de la carpeta de lecciones.
4. Capítulo IV/VI de la memoria NO se tocan acá (la coordinadora decide si las
   lecciones se citan en la tesis al cierre).
5. BITACORA consolidada.

**Compuerta F4 (cierre):** 10/10 lecciones con tests verdes + suite completa
AMBOS crates + clippy + tsc + ci-local.sh --skip-bundle + cargo tauri build +
bundle ≤ 30 MB.
Commit `rp9-fase-4: lección de traps, índice y docs`.

## Criterios de aceptación (del ROADMAP)

- `lecciones/NN-tema/` con ≥ 8 módulos. ✓ (son 10).
- Cada lección: objetivos, programa comentado, ejercicios, salida esperada. ✓
  (plantilla obligatoria).
- Todas corren por CLI con tests. ✓ F1-F4.
- Índice con mapa de aprendizaje. ✓ F3.
- Ejemplos actuales intactos. ✓ (regla de alcance).
- Sin promesas de threads/paralelismo real (honestidad RV32IM mono-hart) —
  lección 9 = concurrencia aparente, lección 10 = traps. ✓ (diseño).
