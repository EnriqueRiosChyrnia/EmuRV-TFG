# PLAN-RP7 — Watchpoints, breakpoints condicionales y edición en vivo

Punto RP-7 de `docs/ROADMAP-PRO.md`. Plan por fases con compuertas de verificación:
PROHIBIDO pasar a la fase siguiente con la compuerta en rojo.

## Contexto (anclas en código)

- Breakpoints hoy: `Machine::set_breakpoints` (machine.rs:473) — hook de código
  instalado SOLO cuando hay breakpoints (con hook el JIT cae órdenes de magnitud:
  1146 → 90 M instr/s, medido en RNF-03). Ese patrón instalar-solo-si-hace-falta
  es LEY para todo hook nuevo de este plan.
- `last_bp`/`suppress_bp` (RP-1 F4): la supresión del breakpoint del pc de
  arranque depende de la causa real de la última parada — las condiciones nuevas
  deben integrarse sin romperlo (sus tests existen y vigilan).
- Sin escritura expuesta: `Machine` no tiene `write_reg`/`write_mem` públicos;
  el panel de registros es read-only; comandos Tauri existentes en commands.rs
  (set_breakpoints:574, read_mem:613, get_state:628 — el patrón a seguir).
- Teclado MMIO (TEC2): página 0xFFFF0000 con hook MEM_READ propio — los
  watchpoints NO deben tocarla (conflicto semántico: leer consume).
- Limitación TEC6 vigente: durante un sleep (syscall 32) el hilo de emulación no
  atiende comandos — las ediciones en vivo se encolan hasta que despierte.
  Documentar, no pelear.
- gdb (RP-6) ya ofrece watchpoints/condiciones para usuarios avanzados — el valor
  de RP-7 es llevarlos a la UI DIDÁCTICA con mensajes en español. Mantener esa
  vara: cada mensaje nuevo es para un estudiante, no para un ingeniero.

## Alcance

- SÍ: watchpoints de lectura/escritura sobre datos, breakpoints con condición
  simple, edición de registros y memoria con la máquina pausada, panel de
  depuración en la UI, comandos Tauri nuevos, tests núcleo + proceso.
- NO: condiciones compuestas (&&/||), watchpoints con condición, expresiones
  aritméticas en condiciones, edición de .text (rechazo didáctico), historial de
  cambios/undo, exposición de nada de esto por el stub gdb (gdb ya tiene lo suyo).

## Diseño acordado (no re-litigar)

### Watchpoints (núcleo)
- `Watchpoint { addr: u32, len: u8 (1|2|4), kind: WatchKind (Read|Write|Both) }`.
- `Machine::set_watchpoints(Vec<Watchpoint>)` — semántica reemplazar-todo, igual
  que set_breakpoints. Hooks MEM_READ/MEM_WRITE instalados SOLO si hay
  watchpoints, removidos al vaciar (espejo exacto del bp_hook; flush de TB al
  instalar/retirar, lección del bug de caché JIT).
- Regiones válidas: .data, pila, framebuffer y paleta. Página del teclado MMIO →
  error didáctico («esa dirección es del teclado; mirala con el panel de
  registros del dispositivo»). Direcciones fuera de toda región → error claro.
- `StopReason::Watchpoint { addr, kind, value, pc }` — value = valor escrito (en
  Write) o leído (en Read). UI: «Watchpoint: el programa escribió 0x2A en
  0x10010004 (línea N)» — línea vía source_map del pc.
- Las lecturas del HOST (panel de memoria, gdb `m`) NO disparan watchpoints
  (mem_read de la API no pasa por hooks — la garantía de TEC2; test que lo fije).

### Breakpoints condicionales (núcleo)
- Gramática mínima: `REG OP VALOR` — REG = nombre ABI (t0, a1, sp…) o xN;
  OP ∈ {==, !=, <, <=, >, >=} con comparación SIGNED (i32); VALOR = inmediato
  decimal o hex 0x… o un segundo registro. Espacios libres. Nada más.
- Parseo AL DEFINIR la condición (no en el hook): error didáctico en español con
  posición («condición inválida: «t9» no es un registro; los temporales van de
  t0 a t6»). El hook evalúa un AST ya validado — costo por instrucción mínimo.
- API: `set_breakpoints` pasa a recibir `Vec<BreakpointSpec { addr, cond:
  Option<CondAst> }>`; el comando Tauri acepta `{ addr, cond: Option<String> }`
  y parsea antes de mandar al hilo (el error didáctico llega al panel SIN tocar
  la máquina). Breakpoint sin condición = comportamiento actual intacto.
- Interacción con `suppress_bp`/`last_bp`: un breakpoint condicional cuya
  condición es FALSA no cuenta como parada — no setea last_bp, no se suprime
  nada. Los tests de RP-1 F4 + nuevos casos lo fijan.

### Edición en vivo (núcleo)
- `Machine::write_reg(n: u8, val: u32)` — n ∈ 1..=31 más pc; escribir x0 → error
  didáctico («x0 es cero por diseño del ISA»). Escribir pc: permitido, resetea
  `last_bp` (el siguiente Run no debe suprimir nada — coherencia con RP-1 F4).
- `Machine::write_mem(addr: u32, bytes: &[u8])` — regiones válidas: .data, pila,
  framebuffer, paleta. .text → rechazo didáctico («el programa es inmutable
  mientras corre; editá el fuente y re-ensamblá»). Página MMIO → rechazo (como
  watchpoints). Sin necesidad de flush de TB (no se toca código).
- Comandos Tauri `write_reg` / `write_mem` servidos SOLO con la máquina pausada
  (estado listo/pausado/breakpoint — el Session ya conoce su estado; corriendo →
  error «pausá la ejecución para editar»).

### UI (panel nuevo + ediciones inline)
- Panel «Depuración» (colapsable, junto a los existentes): lista de breakpoints
  (línea — vía source_map — + campo de condición editable + quitar) y lista de
  watchpoints (dirección, tamaño, tipo R/W/RW, agregar/quitar). Errores de
  condición/dirección mostrados ahí mismo en español.
- Panel de registros: doble click en un valor con la máquina pausada → input
  inline (acepta decimal o 0x…), Enter aplica vía write_reg, Esc cancela, error
  didáctico bajo el panel. El resaltado de cambio existente se reusa para marcar
  el registro editado.
- Panel de memoria: doble click en una palabra → mismo flujo con write_mem.
- Al detenerse por watchpoint: el panel de memoria navega a la dirección y el
  estado muestra el mensaje didáctico. i18n: todas las cadenas nuevas en es.json.

## Fase 1 — Watchpoints (núcleo)

1. Tipos + set_watchpoints + hooks condicionales + StopReason::Watchpoint +
   validación de regiones.
2. Tests: write watch dispara con valor y pc correctos; read watch ídem; Both;
   len 1/2/4 (solapamiento parcial: escribir 4 bytes que pisan un watch de 1);
   host read_mem NO dispara; página MMIO rechazada; sin watchpoints → cero hooks
   (introspección con flag interno o midiendo: variante del test rnf03 confirma
   velocidad de corrida libre intacta); con watchpoint activo: medir y registrar
   en BITACORA (≥ 10 M instr/s del RNF-03 como gate, igual que breakpoints).

**Compuerta F1:** tests verdes + suite completa intacta + clippy + medición
registrada. Commit `rp7-fase-1: watchpoints con hooks condicionales`.

## Fase 2 — Breakpoints condicionales (núcleo)

1. Parser de condición (módulo chico, p. ej. core/cond.rs) con errores didácticos
   posicionados; AST mínimo; eval en el hook de código existente.
2. set_breakpoints → BreakpointSpec manteniendo el camino sin condición intacto
   (los ~10 usos en tests existentes siguen compilando vía From<u32> o helper).
3. Tests: condición verdadera para, falsa pasa de largo (y el bucle sigue N
   iteraciones hasta que sea verdadera — el caso didáctico estrella: parar
   «cuando t0 == 5» en un bucle); registro contra registro; hex negativo signed;
   errores de parseo (registro inexistente, operador malo, basura al final);
   interacción con last_bp (condicional falso no suprime nada — extender los
   tests de RP-1 F4); condicional + watchpoint conviviendo.

**Compuerta F2:** todo verde + clippy. Commit `rp7-fase-2: breakpoints condicionales`.

## Fase 3 — Edición en vivo (núcleo)

1. write_reg/write_mem con las validaciones del diseño.
2. Tests: editar t0 pausado → la siguiente instrucción usa el valor nuevo (el
   criterio del ROADMAP, literal); editar pc → ejecución continúa ahí y last_bp
   reseteado; x0 rechazado; .text rechazado; MMIO rechazado; write_mem en .data
   visible por el programa y por read_mem; en pila; bytes/half/word.

**Compuerta F3:** todo verde + clippy. Commit `rp7-fase-3: edición en vivo de registros y memoria`.

## Fase 4 — Comandos, API TS y panel de depuración

1. Comandos Tauri: set_breakpoints extendido (compatible), set_watchpoints,
   write_reg, write_mem — con el guard de máquina pausada. API TS (src/api.ts)
   + tipos.
2. UI según el diseño: panel Depuración, edición inline en registros y memoria,
   navegación del panel de memoria al frenar por watchpoint, i18n completo.
3. tsc --noEmit limpio; prueba manual guiada (la app levanta, los flujos
   funcionan) — capturas para BITACORA si el flujo de capturas del repo aplica.

**Compuerta F4:** tsc limpio + suite Rust intacta + flujos manuales verificados y
anotados en BITACORA. Commit `rp7-fase-4: panel de depuración y edición inline`.

## Fase 5 — Documentación y cierre

1. docs/DEPURACION.md (o sección en el README si queda corto): watchpoints,
   condiciones (gramática exacta con ejemplos), edición en vivo, límites (TEC6
   sleep, .text inmutable, MMIO) y cuándo conviene saltar a gdb (RP-6).
2. Ayuda integrada: párrafo nuevo. ENSAMBLADOR.md no se toca (nada de sintaxis).
3. BITACORA: decisiones + mediciones + niveles de confianza.
4. `docs/ROADMAP-PRO.md`: NO tocar — lo gestiona la sesión coordinadora.

**Compuerta F5 (cierre):** cargo test + clippy verdes AMBOS crates + tsc limpio +
`scripts/ci-local.sh --skip-bundle` verde + `cargo tauri build` + bundle ≤ 30 MB.
Commit `rp7-fase-5: docs depuración didáctica`.

## Criterios de aceptación (del ROADMAP)

- Tests núcleo para watchpoint de escritura Y de lectura. ✓ F1.
- Breakpoints con condición simple (t0 == 5 en bucle). ✓ F2.
- UI: panel de breakpoints con condición editable. ✓ F4.
- Edición de registro refleja en la siguiente instrucción ejecutada. ✓ F3 (test
  literal) + F4 (flujo UI).
- Extra fijado por este plan: velocidad de corrida libre intacta sin
  watchpoints/condiciones (el patrón instalar-solo-si-hace-falta, medido). ✓ F1.
