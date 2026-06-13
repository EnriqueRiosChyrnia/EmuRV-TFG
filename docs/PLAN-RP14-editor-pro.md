# PLAN-RP14 — Editor profesional (autocompletar, hover, ir-a-etiqueta)

Punto RP-14 de `docs/ROADMAP-PRO.md`. Plan por fases con compuertas de verificación:
PROHIBIDO pasar a la fase siguiente con la compuerta en rojo.

## Contexto (anclas en código)

- Editor CodeMirror 6: `src/editor/index.ts` (createEditor + breakpoints +
  setErrors + setExecLine) y `src/editor/riscv.ts` (StreamLanguage de resaltado).
- **Deuda detectada (la coordinadora, 2026-06-13):** las listas `MNEMONICS` y
  `REGISTERS` de riscv.ts están en RV32IM ANTIGUO — NO incluyen F/D (RP-12),
  A ni C (RP-13). El resaltado de sintaxis hoy NO reconoce fadd.s, amoadd.w,
  c.addi, registros f. RP-14 las pone al día (es prerrequisito de un
  autocompletado honesto).
- `@codemirror/autocomplete` NO está instalado — dep nueva del mismo ecosistema
  CM6 (peso JS menor; el bundle se re-mide igual, RNF-04).
- `Program.symbols: Record<string,number>` ya viaja en la API (api.ts) tras
  ensamblar — pero las etiquetas se pueden leer del PROPIO buffer con regex
  (`^etiqueta:`), lo que funciona ANTES de ensamblar y sin viaje al backend:
  esa es la fuente para ir-a-etiqueta y autocompletar etiquetas.
- Material para el hover: la Ayuda integrada y las lecciones (RP-9) ya describen
  instrucciones en español — RP-14 consolida eso en un diccionario de datos.

## Alcance

- SÍ: listas de tokens al día (RV32IMAFDC + Zicsr + pseudos); autocompletado de
  mnemónicos, registros (x/ABI/f) y etiquetas del buffer; hover con ayuda por
  instrucción (español); ir-a-definición de etiqueta; resaltar usos de la
  etiqueta bajo el cursor; docs.
- NO: LSP completo, análisis semántico en vivo (errores se siguen viendo al
  Ensamblar, como hoy — RP-14 no re-ensambla mientras tipeás), formateo
  automático, refactor/rename, sugerencias «¿quiso decir…?» en mensajes del
  ENSAMBLADOR (eso vive en el Rust — queda como propuesta separada; el
  autocompletado ya mitiga el typo antes de que ocurra).

## Diseño acordado (no re-litigar)

### Fuente única de tokens (anti-drift)
- `src/editor/isa.ts` NUEVO: arrays exportados `MNEMONICS`, `REGISTERS_X`,
  `REGISTERS_F`, `PSEUDOS` cubriendo RV32IMAFDC+Zicsr — la lista COMPLETA que el
  ensamblador acepta. riscv.ts (resaltado) y el autocompletado consumen de acá:
  una sola fuente en el frontend.
- **Anti-drift con el backend:** test (en emurv-cli o un test TS) que toma cada
  mnemónico de isa.ts, arma una instrucción mínima válida y confirma que el
  ensamblador la acepta (no «instrucción desconocida»). Y a la inversa para una
  muestra: que no falte ninguno. Así la lista del editor no se desincroniza del
  Rust en el futuro. Si el test cruzado TS↔Rust es caro, alternativa: un test
  Rust que exponga la lista de mnemónicos del assembler y un snapshot que isa.ts
  debe igualar — registrar la elección en BITACORA.

### Autocompletado (@codemirror/autocomplete)
- Contexto: primer token de la línea → mnemónicos+pseudos; tras un mnemónico que
  espera registro → registros (filtrando f vs x según la instrucción si es
  barato; si no, todos); tras operando que admite etiqueta (j, la, beq…, %hi) →
  etiquetas del buffer. Cada ítem con `detail` corto (la sintaxis) y `info` (la
  descripción del diccionario). Disparo automático + Ctrl-Espacio.

### Hover (diccionario de datos)
- `src/editor/ayuda-isa.ts` NUEVO: `Record<mnemónico, { sintaxis, descripcion }>`
  en español, una entrada por instrucción del repertorio (reusar/condensar el
  material de la Ayuda y las lecciones — NO inventar semántica; citar la fuente
  en un comentario). `hoverTooltip` de CM6 muestra sintaxis + descripción al
  pasar sobre un mnemónico. Mnemónico sin entrada → sin tooltip (nunca un
  tooltip vacío o erróneo).

### Navegación de etiquetas
- Ir-a-definición: Cmd/Ctrl-click (o tecla configurable) sobre el uso de una
  etiqueta → mueve el cursor a su definición (`^etiqueta:` en el buffer). Si no
  existe → aviso breve, sin romper.
- Resaltar usos: con el cursor sobre una etiqueta, subrayar/marcar todas sus
  apariciones (definición + usos) — decoración CM6, sin tocar el texto.

## Fase 1 — Tokens al día + fuente única

isa.ts con RV32IMAFDC+Zicsr completo; riscv.ts consume de isa.ts (resaltado
ahora reconoce fadd.s/amoadd.w/c.addi/registros f); test anti-drift TS↔Rust.
**Compuerta F1:** resaltado correcto verificado a mano (BITACORA) + test
anti-drift verde + tsc limpio + suite Rust intacta + clippy.
Commit `rp14-fase-1: tokens ISA al día (RV32IMAFDC) y fuente única anti-drift`.

## Fase 2 — Autocompletado

@codemirror/autocomplete + provider contextual (mnemónicos/registros/etiquetas).
Tests: con @codemirror autocomplete se testea el provider como función pura
(dado un estado/posición, devuelve las opciones esperadas) — sin DOM. Casos:
inicio de línea → mnemónicos; tras `add ` → registros; tras `j ` → etiquetas del
buffer; filtrado por prefijo.
**Compuerta F2:** tests del provider verdes + tsc + bundle re-medido ≤ 30 MB
(dep nueva). Commit `rp14-fase-2: autocompletado contextual`.

## Fase 3 — Hover + navegación de etiquetas

ayuda-isa.ts + hoverTooltip + ir-a-definición + resaltar usos. Tests: el
diccionario cubre TODO mnemónico de isa.ts (test que falla si falta una
entrada — anti-drift del hover); función de búsqueda de definición de etiqueta
(pura, dado buffer + nombre → línea); función de usos. Verificación manual del
hover y el click (BITACORA).
**Compuerta F3:** tests verdes + cobertura hover 100% de mnemónicos + tsc.
Commit `rp14-fase-3: hover de ayuda y navegación de etiquetas`.

## Fase 4 — Docs y cierre

README (features del editor), Ayuda integrada (mención de autocompletar/hover/
ir-a-etiqueta + atajos), BITACORA. ROADMAP-PRO y Cap. VI: NO tocar.
**Compuerta F4 (cierre):** cargo test + clippy AMBOS + tsc + ci-local
--skip-bundle + tauri build + bundle ≤ 30 MB.
Commit `rp14-fase-4: docs editor profesional`.

## Criterios de aceptación (del ROADMAP)

- Autocompletado de mnemónicos y registros (lista YA al día, no la vieja). ✓ F1/F2.
- Hover con ayuda por instrucción en español, cobertura 100%. ✓ F3.
- Ir-a-definición de etiquetas + resaltar usos. ✓ F3.
- Bonus: deuda del resaltado desactualizado (sin F/D/A/C) saldada. ✓ F1.
- Anti-drift: la lista del editor no puede desincronizarse del ensamblador
  (test cruzado). ✓ F1.
