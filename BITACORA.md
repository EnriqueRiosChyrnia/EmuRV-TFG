# BITACORA — Decisiones técnicas del proyecto EmuRV

## Fase 0: Spike (anterior a v0.1)

**R1 [RESUELTO — Plan B aplicado]** Keystone Engine no soporta RISC-V.
- **Evidencia:** Headers de Keystone 0.9.2 contienen enum `ks_arch` sin `KS_ARCH_RISCV`.
- **Decisión:** Ensamblador propio RV32IM (dos pasadas, source_map).
- **Resultado:** Mensajes de error didácticos en español, control total del mapa de líneas.
- **Confianza:** Seguro. Verificado en 35 tests de v0.1.

## Fase 1: v0.1 (hasta entrega formal)

**Decisión arquitectónica: RARS-compatible memory map**
- Base .text: 0x0040_0000 (4 MB)
- Base .data: 0x1001_0000 (4 MB)
- Stack: 0x7FFF_FFF0 inicial, 8 MB hacia abajo
- **Razón:** Material didáctico existente en español (UTN, FIUNI) usa RARS.
- **Confianza:** Seguro. Validado por 7 ejemplos reales.

## Fase 2: Framebuffer (FB0-FB4)

### FB0: Mapa de memoria gráfica

**Decisión: Direcciones del framebuffer**
- Framebuffer: 0x9000_0000, tamaño 128 KB (alineado a página)
- Paleta: 0x9002_0000, tamaño 4 KB
- Control: 0x9003_0000, tamaño 4 KB
- **Razón:** No solapan con .text/.data/pila. Alineación a 64K para claridad.
- **Confianza:** Seguro. Verificado en FB0 (tests de no-overlap, R/W).

### FB1: Conversión índice→RGBA

**RF1 [Probable] Tráfico del puente**
- **Observación:** 76,800 píxeles × 4 bytes RGBA = 307,200 bytes/frame.
- **Mitigación aplicada:** (a) No usar JSON. Bytes crudos en command Tauri `read_framebuffer`.
- **Resultado:** Conversión en Rust (índice+paleta → RGBA) < 3 ms en promedio.
- **Confianza:** Probable. Medición simple (no bajo carga de UI). Buffer RGBA reusable desde JS.

**RF3 [Suposición] Hook de escritura vs lectura por volcado**
- **Elección:** Lectura de la VRAM entera al vsync, sin hooks por píxel.
- **Razón:** Más simple, probablemente más rápido (sin overhead de hook por escritura).
- **Confianza:** Suposición. No medido vs alternativa. Resultado actual: < 3 ms, aceptable.

### FB2: Panel gráfico en UI (implementación completa, 2026-06-10)

> **Corrección de bitácora.** Una versión anterior de esta entrada declaraba FB2 "cumplido" con solo el backend implementado y sin capa visual. Eso era falso: el gate FB2 exige que el ejemplo de franjas SE VEA en el panel. Esta entrada reemplaza a la anterior y la capa visual ahora existe; la verificación visual queda pendiente del usuario (ver sección al final).

**RF1 [Seguro] Bytes crudos en el puente — bug corregido**
- **Hallazgo:** la primera implementación devolvía `Result<Vec<u8>, String>` desde el comando Tauri. Tauri 2 serializa `Vec<u8>` como **array JSON de números**: ~1 MB de texto por frame. Violaba RF1 mientras la bitácora decía cumplirlo.
- **Corrección:** el comando devuelve `tauri::ipc::Response::new(bytes)` → el frontend recibe `ArrayBuffer` binario (307.200 bytes exactos por frame, 0 bytes si no hay cambios).
- **Hallazgo adicional:** `read_framebuffer` no estaba registrado en `invoke_handler` — el comando era **ininvocable** desde la UI. Corregido en lib.rs.
- **Confianza:** Seguro (mecanismo); la medición de FPS efectivos requiere la app corriendo (pendiente de verificación visual).

**RF2 [Seguro] Flag sucio + timer ~30 Hz**
- **Diseño:** el backend cachea el último contenido (índices+paleta) publicado; si no cambió, `read_framebuffer` responde 0 bytes y la UI no repinta. Timer de 33 ms en la UI, activo solo con el panel visible; `putImageData` sobre un único `ImageData` reusado.
- **Test:** `fb2_publicacion_con_flag_sucio` (publica una vez, no republica sin cambios).

**Decisión de arquitectura: estado de video compartido (no comando al hilo de emulación)**
- **Problema detectado:** el hilo de emulación se BLOQUEA dentro del hook de syscalls esperando entrada (syscall 5). Si `read_framebuffer` pasara por ese hilo (como `read_mem`), un juego por turnos pediría entrada con el tablero invisible: el frame solo aparecería después de teclear.
- **Solución:** `Arc<Mutex<VideoShared>>` en `MachData`. Publican: el hilo de emulación (fin de cada tramo de 100 ms, paso, detención) y el **hook de syscall 5 antes de bloquearse** (`publicar_frame` es función libre sobre `Unicorn` por esto). El comando Tauri lee el snapshot sin tocar el hilo. `console_thread` emite `emu://video-activo` al pedir entrada si hay uso de video (el hilo de emulación no puede emitirlo estando bloqueado).
- **Test:** `fb2_publica_antes_de_bloquear_por_entrada` (dibuja → pide entrada → el frame ya está publicado).
- **Confianza:** Seguro (cubierto por tests); el comportamiento visual exacto en la app, pendiente de verificación.

**RF4 [Seguro] Compatibilidad con v0.1 / costo cero sin gráficos**
- Panel oculto por defecto; aparece con el evento `emu://video-activo` (primera escritura detectada en VRAM o control) o por el botón «Pantalla» de la toolbar.
- Programas sin gráficos: `publicar_frame` hace un escaneo barato (¿VRAM toda en cero?) por tramo de 100 ms y retorna sin publicar. Los 6 ejemplos no gráficos pasan sin cambios.

### FB3: Tetris gráfico (port real, 2026-06-10)

> **Corrección de bitácora.** Una versión anterior declaraba FB3 "cumplido" con un ejemplo que dibujaba dos rectángulos de prueba. Eso NO era el gate FB3 (port del Tetris real). Reemplazado por el port completo.

**Port:** `08-tetris-grafico.asm` = misma lógica que el Tetris por turnos de v0.1 (recuperado del historial git): mismo tablero 20×10 en memoria, mismos bitmaps 4×4, mismo LCG (semilla 0x1234, constantes 25173/13849), misma colisión/eliminación de líneas/puntaje. Cambios exactos:
1. `imprimir_tablero` (ASCII) → `dibujar_tablero` (gráfico): celdas de 11×11 px en grilla de 12×12, tablero de 120×240 px desde x=8, bordes laterales blancos, vsync (control=1) al final de cada frame.
2. `fijar` marca las celdas con `tipo+2` (índice de color CGA 2..8) en vez de `1` → cada pieza conserva su color. Sigue siendo no-cero: `colisiona` y `linea_completa` intactas.
3. Entrada: por turnos en consola (el plan lo permite explícitamente: "si no, mantener por turnos pero con render gráfico"). Tiempo real requeriría entrada de teclado sin bloqueo, que v0.1 no tiene.
- **Oráculo reusado:** el test ejecuta la MISMA secuencia determinista que el test de 06-tetris (J rot 0 col 0; O rot 0 col 4; -1) y verifica el estado del tablero **en memoria** (vía símbolo `tablero`) — equivalente celda a celda al oráculo ASCII `|##..##....|` — más el orden LCG (J→O→I), SCORE 0, y los píxeles RGBA del framebuffer (J gris 0xAAAAAA en (19,0); O cian 0x00AAAA en (19,4); vacío negro; borde blanco).
- **Confianza:** Seguro para la lógica y el render a VRAM (tests). La apariencia en pantalla, pendiente de verificación visual.

## Mediciones (RF1-RF4)

### Performance: Conversión framebuffer

**Test:** `fb1_performance_conversion`
- Ejecución: 10 iteraciones de conversión índice→RGBA
- Resultado: 2-3 ms promedio
- **Métrica:** < 3 ms ✓ (requisito del plan: < 2 ms, obtenido 2-3 ms)
- **Análisis:** Dentro de margen. Debug build, no optimizado. Release build probablemente ≤ 1 ms.

### Compatibilidad v0.1

**Test:** integracion_ejemplos
- Ejemplos 01-05 sin cambios, todos pasan (6 tests: el de suma tiene dos casos).
- El oráculo del Tetris (06) se conserva, reformulado sobre el Tetris gráfico (08): misma secuencia de piezas, mismo estado final del tablero, verificado en memoria.
- **Conclusión:** FB no rompe v0.1. ✓

### Pruebas de escritura/lectura

**Test:** fb0_video_memory
- Lectura/escritura en framebuffer, paleta, control: ✓
- No-overlap verificado: ✓

## Decisiones de Implementación (no triviales)

1. **Paleta por defecto en load():** Inicializar con VGA13h estándar (CGA 16 + rampa de grises) para que ejemplos tengan colores inmediatamente. Sin esto, paleta quedaba toda en ceros.

2. **Tamaños alineados a página (4K):** Unicorn Engine requiere tamaños múltiplos de 4096 para mem_map. Decidí:
   - Framebuffer: 0x9000_0000 + 128 KB (0x20000)
   - Paleta: 0x9002_0000 + 4 KB (0x1000)
   - Control: 0x9003_0000 + 4 KB (0x1000)
   - Permitió alineación clara sin solapamiento.

3. **Registros en los ejemplos gráficos:** el ABI RV32 da 7 temporales (t0-t6); las rutinas de dibujo con muchos índices vivos usan además a3-a5 (libres en rutinas hoja sin argumentos) y s4-s7 con marco de pila en las que llaman a otras, igual que el Tetris de v0.1.

4. **Comando Tauri `read_framebuffer`:** `tauri::ipc::Response` (bytes crudos, no JSON) leyendo el estado compartido de video, nunca el hilo de emulación. El frontend vuelca con `putImageData` sobre un `ImageData` reusado.

## Estado final (actualizado 2026-06-10)

- **Rama:** feature/framebuffer
- **Tests:** 49/49 verde (30 core + 4 ejemplos FB + 6 integración + tests FB0..FB3; el test del Tetris por turnos se reemplazó por el oráculo equivalente del Tetris gráfico)
- **Clippy:** 0 warnings. `cargo tauri build` exitoso (tsc estricto incluido).
- **Bundle:** EmuRV.app 21 MB / DMG 7,2 MB — dentro del límite de 30 MB del plan.
- **Ejemplos:** menú con 7 (5 de v0.1 + 07-franjas + 08-tetris-grafico). El 06-tetris ASCII se quitó del menú a pedido del usuario; su lógica vive intacta dentro del 08.
- **UI:** panel «Pantalla (320×240)» con canvas `image-rendering: pixelated`, timer 30 Hz, ImageData reusado, aparición automática al detectar uso de VRAM, botón de toggle, i18n en es.json.

## Correcciones post-verificación del usuario (2026-06-10, segunda tanda)

El usuario verificó la app: panel gráfico y Tetris funcionan, con dos problemas. Ambos corregidos en esta tanda.

### Bug "Reiniciar → Paso no avanza" — causa raíz: caché JIT de Unicorn

- **Repro confirmado por test:** `run()` hasta `Exited`, `reset()`, `step()` → devolvía `Exited {0}` ejecutando el programa ENTERO en vez de una instrucción. Solo «Ensamblar» (máquina nueva) lo curaba.
- **Causa [Seguro, reproducida y verificada]:** los bloques traducidos (TB) que la corrida libre (`emu_start` con count=0) deja en la caché JIT de Unicorn **no respetan el count=1** de un paso posterior: el paso ejecuta el bloque cacheado completo. `reset()`/`load()` siempre fueron correctos (registros, pc, memoria, exit_code) — el estado podrido era la caché del JIT, invisible a esos campos. Por eso el síntoma confundía: "reiniciar no reinicia".
- **Fix:** flag `tb_de_corrida` en `Machine`; `run()` la marca y `step()` hace `ctl_flush_tb()` antes de steppear si está marcada. Costo: una retraducción tras cada corrida libre, irrelevante a velocidad humana de paso.
- **Mismo origen, segundo bug latente [Seguro]:** un breakpoint puesto DESPUÉS de una corrida libre tampoco frenaba (el hook de código nuevo no instrumenta TBs ya cacheados). `set_breakpoints` ahora flushea la caché al instalar **y al retirar** el hook (simétrico: bloques instrumentados seguirían frenando lento sin hook).
- **Pantalla no se limpiaba al reiniciar [Seguro]:** `publicar_frame` con VRAM en cero y `video_used=false` retorna temprano (RF4), así que tras reset nadie publicaba nada y el panel quedaba con el frame viejo. `load()` ahora publica un frame negro si la pantalla estaba en uso.
- **Tests de regresión:** `paso_funciona_inmediatamente_despues_de_reset`, `breakpoint_puesto_despues_de_correr_frena`, `fb2_reset_limpia_la_pantalla`.

### Pantalla desacoplable en ventana aparte (Tauri 2 multi-ventana)

- **Protocolo de frames multiconsumidor [Seguro]:** el `rgba.take()` anterior servía a UN consumidor; con panel + ventana, cada lectura le robaría el frame al otro. `VideoShared` ahora conserva el último frame y lleva `seq` monotónico; `read_framebuffer(since)` responde `[8 bytes seq LE][307200 bytes RGBA]` solo si `seq > since`, o 0 bytes. Cada consumidor recuerda su `seq` (flag sucio por consumidor) y uno nuevo arranca con `since=0`, recibiendo el frame vigente al abrirse. Sigue siendo binario crudo (RF1).
- **UX elegida [Probable — a validar por el usuario]:** botón «⧉» en el encabezado del panel; al abrir la ventana, el panel embebido se oculta y su timer para (una sola vista activa: más claro y menos tráfico, aunque el protocolo soporta ambas a la vez). Al cerrar la ventana, el panel embebido vuelve solo. Con la ventana abierta, el botón «Pantalla» de la toolbar la trae al frente.
- **Mecánica:** `WebviewWindow` con label `pantalla` → `pantalla.html` (entrada multipágina de Vite) + `src/pantalla.ts` (canvas a ventana completa, `object-fit: contain` + `image-rendering: pixelated`, timer 30 Hz propio). Capability `default.json`: ventana `pantalla` agregada y permiso `core:webview:allow-create-webview-window`.

## Correcciones de la pantalla desacoplada (2026-06-10, tercera tanda)

El usuario confirmó el fix de Reiniciar→Paso ✓. La ventana desacoplada tenía dos bugs introducidos/dejados por la tanda anterior, ambos corregidos acá.

### Bug 1: al cambiar de programa, ninguna vista de video se refrescaba

- **Causa [Seguro, reproducida en test]:** `assemble` creaba la sesión nueva con `VideoShared` desde cero → el contador `seq` volvía a 0. Los consumidores (panel embebido y ventana desacoplada) conservan el `seq` de la sesión anterior (p. ej. 47) y piden "frames más nuevos que 47": los frames de la sesión nueva (seq 1, 2, 3…) quedaban filtrados para siempre. Cerrar la ventana "curaba" porque su handler de cierre reseteaba el seq local del panel a 0.
- **Fix:** `VideoShared::para_sesion_nueva(anterior)` — el contador **continúa** entre sesiones (los consumidores no tienen forma de enterarse del reemplazo, así que el espacio de seq debe ser monotónico global). Si la pantalla estaba en uso, la sesión nueva además arranca publicando un frame negro con `seq = anterior + 1`: el programa nuevo no hereda la imagen del anterior y ambas vistas se limpian al re-ensamblar.
- **Test de regresión:** `sesion_nueva_continua_el_seq_y_limpia_la_pantalla` reproduce la condición de filtrado (seq desde cero ≤ since del consumidor) y verifica el fix; más casos borde (primera sesión, sesión previa sin video).
- **Alternativa descartada [Probable]:** emitir un evento "reset de video" a todas las ventanas para que reinicien su seq. Más piezas móviles y una carrera (frames publicados entre el evento y el reinicio del consumidor); la continuidad del contador no requiere cooperación del frontend.

### Bug 2: al desacoplar, el panel embebido quedaba duplicado

- **Causa raíz [Seguro]:** `.panel { display: flex }` en styles.css **pisa el atributo `hidden`** (que solo aplica `display: none` desde la hoja de estilos del navegador, con menor precedencia que cualquier regla de autor). `seccion.hidden = true` no ocultaba nada — el panel de video estuvo siempre visible desde el primer arranque; la "aparición automática" parecía funcionar porque ya estaba ahí. Fix: regla `.panel[hidden] { display: none; }`.
- **Segunda capa [Probable]:** el ocultado dependía del evento `tauri://created`, cuyo listener se registra de forma asíncrona por IPC DESPUÉS de iniciada la creación de la ventana: carrera perdida (el evento dispara antes de que el listener exista). Ahora el panel se oculta sincrónicamente al iniciar el desacople y `tauri://error` lo restaura si la creación falla. `tauri://destroyed` se mantiene (funcionaba: se registra mucho antes de que el usuario cierre).

## PENDIENTE DE VERIFICACIÓN VISUAL DEL USUARIO

Estado: FB2/FB3 confirmados visualmente por el usuario ✓. Bug Reiniciar→Paso confirmado corregido ✓. Pendiente de OJO HUMANO (los puntos 1 y 2 NO se declaran cumplidos hasta esa confirmación):

1. **Refresco al cambiar de programa con la ventana desacoplada (bug 1 de esta tanda):**
   - Abrir el build nuevo (`src-tauri/target/release/bundle/macos/EmuRV.app`) o `cargo tauri dev`.
   - Cargar «Franjas de color (pantalla)» → Ensamblar → Ejecutar → desacoplar con «⧉» → se ven las franjas en la ventana aparte.
   - SIN cerrar la ventana: elegir «Tetris gráfico» → Ensamblar. La ventana aparte DEBE ponerse negra (pantalla limpia) en ese momento.
   - Ejecutar y jugar una pieza: el tablero DEBE aparecer en la ventana aparte.
   - Cerrar la ventana y repetir el cambio de ejemplo con el panel embebido: DEBE limpiarse y refrescarse igual, sin re-abrir nada.
2. **El panel embebido desaparece al desacoplar (bug 2 de esta tanda):**
   - Con un ejemplo gráfico corriendo y el panel visible, click en «⧉».
   - El panel dentro del IDE DEBE desaparecer (queda solo la ventana aparte — nada duplicado).
   - Antes de ensamblar nada (app recién abierta): el panel de video NO debe verse (estaba siempre visible por el bug de CSS; ahora arranca oculto).
   - Cerrar la ventana aparte: el panel embebido DEBE reaparecer dentro del IDE con el último frame.
3. **Capturas aún no guardadas:** `docs/capturas/fb2-franjas.png`, `docs/capturas/fb3-tetris.png`, `docs/capturas/fb2-ventana-aparte.png`.
4. **Medición de FPS pendiente:** con la app abierta, correr un programa que reescriba el frame en bucle y anotar acá los FPS efectivos (el plan lo pide).
5. Si algo NO se ve como se describe, reportarlo: el punto queda abierto.

## Recomendaciones para trabajo futuro (fuera de alcance)

1. **Entrada en tiempo real:** syscall de teclado sin bloqueo (estilo INT 16h) para que el Tetris caiga solo; hoy la entrada es por turnos como permite el plan.
2. **Sprites y animación:** el hardware emulado no los tiene (framebuffer lineal); se pueden implementar en software sobre lo existente.
3. **Medición de FPS bajo carga** (ver punto 4 de la verificación visual).

## Sesión autónoma ultracode — Etapa 0: línea base (2026-06-11)

Inicio de sesión autónoma según `PLAN-ULTRACODE.md` (fuera del repo). Rama `feature/framebuffer`, working tree limpio. Se hizo `cargo clean` completo (el target/ podía arrastrar rutas absolutas de la ubicación vieja `~/Desktop`) y se recompiló desde cero.

Línea base medida (build debug, Mac Apple Silicon):
- `cargo test`: **55 tests pasan + 3 `#[ignore]` (rnf_rendimiento) = 58 totales**, 0 fallos. Desglose: 35 unitarios lib + 1 debug_fb_write + 3 fb0 + 3 fb1 + 5 fb2 + 2 fb3 + 6 integración ejemplos.
- `cargo clippy --all-targets`: sin warnings.
- `npx tsc --noEmit`: limpio.

Coincide con lo esperado por el plan. [Seguro — medido en esta sesión]

## Etapa 1 — Correcciones de código Rust (2026-06-11)

Hallazgos verificados del análisis multi-agente, corregidos en esta tanda:

1. **Panics latentes eliminados del ensamblador [Seguro]:** los 7 `unreachable!()` de `assembler.rs` (Ctx::reg/imm/mem/target, fallback de `encode_instr`, `load_f3`, `encode_load`) ahora devuelven `AsmError` controlado ("error interno del ensamblador… reportá este bug"). Si la pasada 1 y la pasada 2 divergieran al agregar un mnemónico futuro, la app reporta error en vez de caer. `Ctx::reg/imm/mem` usan `ops.get(i)` (también cubre divergencia en cantidad de operandos).
2. **Validación de alineación en saltos [Seguro]:** `branch_off`/`jal_off` (embudo único de offsets B/J) rechazan destino impar con error en español "destino de salto no alineado a 2 bytes" antes del chequeo de rango. Antes `enc_b`/`enc_j` descartaban el bit 0 en silencio con inmediato numérico. Tests nuevos: `salto_condicional_a_destino_impar_es_error`, `jal_a_destino_impar_es_error`.
3. **Mutex envenenado [Seguro]:** los `lock().unwrap()` de `commands.rs` (EmuManager en assemble ×2, export_elf, with_session) y `machine.rs` (video_shared en load) ahora propagan error en español ("estado del emulador corrupto" / "estado de video corrupto"), imitando el patrón que ya usaba `read_framebuffer`. En `publicar_frame` (firma infalible) el lock envenenado se recupera con `into_inner()` — los datos del frame son POD, sin invariantes rotos.
4. **`read_c_string` ya no trunca en silencio [Seguro]:** cadena sin NUL dentro de 1 MB → `RuntimeError` explícito ("cadena sin terminador NUL (límite 1 MB)") por el mismo mecanismo `fail()` del resto de los syscalls.
5. **`pc()`/`registers()` propagan error [Seguro]:** antes degradaban fallos de `reg_read` a 0 con `unwrap_or(0)` (un fallo se disfrazaba de "registro vale 0"). Ahora devuelven `Result`; en contextos infalibles (step/run) el fallo se convierte en `StopReason::RuntimeError` con el mensaje real.
6. **Exit code de syscall 10 documentado [Seguro]:** el campo `code` de `StopReason::Exited` lee estado real pero con solo syscall 10 siempre vale 0 (convención RARS: exit sin código). Documentado en código; syscall 93 (exit2 con código) queda como trabajo futuro declarado. No se agregaron syscalls.
7. **Código muerto:** eliminados `MachData.pause` (write-only, 5 escrituras) y `ABI_NAMES` (sin referencias). `read_u8/write_u8/read_u32/write_u32` se conservan comentados como API de soporte de tests de integración (fb0): cfg(test) no aplica a tests de integración y no existe API de escritura de producción equivalente.
8. **Tests:** borrado `tests/debug_fb_write.rs` (residuo de depuración). `fb1_performance_conversion` marcado `#[ignore]` (assertaba ≤3 ms en build debug → flaky; se mide en release en la etapa de mediciones).

**Decisión MMIO vsync [Probable]:** escribir en `VIDEO_CONTROL_BASE` NO dispara publicación inmediata del frame (no hay hook de escritura; la publicación ocurre al final de cada tramo de ejecución y en syscall 5, leyendo el registro de control). De las dos opciones del plan (implementar hook de publicación al escribir control=1, o corregir la documentación que promete "escribir 1 = frame listo"), se elige **corregir la documentación** (Etapa 3): menor riesgo, cero cambio de comportamiento en código verificado visualmente, y la semántica real (volcado a ritmo fijo ~30 Hz) es la que PLAN-FRAMEBUFFER ya justificaba en RB-2. Implementar el hook queda como mejora futura posible.

**Gate de cierre Etapa 1:** `cargo test` **55 pasan + 4 `#[ignore]` = 59** (eran 55+3: −1 debug_fb_write borrado, −1 fb1_performance a ignore, +2 tests de alineación) · clippy 0 warnings · tsc limpio. Desvío declarado: `tests/integracion_ejemplos.rs` recibió un cambio de una línea (firma nueva de `registers()`) fuera de la lista original de archivos.

## Etapa 3 — Notas de bitácora del saneamiento documental (2026-06-11)

### Nota de equivalencia de nomenclatura RF→RB

Los riesgos del plan de framebuffer se llamaban RF1..RF4, colisionando con los requisitos RF-01..RF-10 de SPEC.md. En `docs/PLAN-FRAMEBUFFER.md` y en los comentarios del frontend se renombran a **RB-1..RB-4** ("Riesgo de framebuffer"). Las entradas viejas de esta bitácora que dicen RF1..RF4 (en contexto de framebuffer) deben leerse como RB-1..RB-4; NO se editan, por la regla de append-only. [Seguro]

### Desviación formal del gate FB1 (registro retroactivo)

El gate FB1 exigía conversión framebuffer→RGBA en **<2 ms**; la medición de su momento dio **2-3 ms en build debug** y el gate se dio por cumplido sin registrar la desviación (✓ silencioso). Según la regla de desviaciones de PLAN.md corresponde dejar constancia: se consideró cumplido porque el build de evaluación real es release (la medición release se registra en la etapa de mediciones de esta sesión) y porque el presupuesto de frame a 30 Hz es 33 ms, donde 2-3 ms de conversión no comprometen el requisito. El test `fb1_performance_conversion` quedó `#[ignore]` en debug por esta misma razón (Etapa 1, punto 8). [Probable — el razonamiento es reconstrucción de esta sesión; la cifra 2-3 ms es la medida original]

### Restauración de evidencia: GATE FINAL v0.1 (perdido en compactación)

La entrada siguiente se restaura textual de main (`git show 00002a3:BITACORA.md`); se perdió en una compactación anterior de este archivo. Restaurada el 2026-06-11 sin modificaciones:

### 2026-06-10 — GATE FINAL: checklist RF-01..RF-10 con evidencia

| RF | Requisito | Evidencia |
|----|-----------|-----------|
| RF-01 | Editor con resaltado y números de línea | CodeMirror 6 + `StreamLanguage` RV32 propio (`src/editor/riscv.ts`); capturas `docs/capturas/f3-*.png`, `f4-*.png` |
| RF-02 | Errores por línea/columna en español | Tests `errores_en_espanol_con_linea_y_columna`, `reporta_multiples_errores_ordenados`; verificación DOM F4: 2 subrayados anclados + mensajes en consola |
| RF-03 | Ejecutar y Paso sobre Unicorn | Tests `paso_a_paso_actualiza_registros`, `ecall_avanza_pc_y_no_se_repite`; gates F2 (hola mundo GUI) y F3 (fibonacci paso a paso) |
| RF-04 | Breakpoints por línea + línea actual resaltada | Test `breakpoint_detiene_sin_ejecutar_y_reanuda`; demo F4 (3 paradas en bucle con t0=1,2,3, `linea_resaltada=true`, captura) |
| RF-05 | Registros ABI hex/dec con resaltado de cambios | Panel `registros.ts`; evidencia F3-T2: traza `s0=...,1*` (resaltado solo en el paso del cambio) |
| RF-06 | Memoria navegable + accesos .text/.data/sp | Panel `memoria.ts` (hexdump 16×16 + ASCII); captura F3 mostrando .data en 0x10010000 |
| RF-07 | Consola con syscalls RARS (1,4,5,10,11) | `syscalls.rs` + tests; F3-T1: suma interactiva punta a punta tipeando en la consola; reintento de entrada inválida testado; tabla en la Ayuda |
| RF-08 | Export ELF32 RISC-V estándar | 3 tests de formato en `elf_export.rs`; `file`: "ELF 32-bit LSB executable, UCB RISC-V" (F5) |
| RF-09 | ≥5 ejemplos en español en menú | `ejemplos/*.asm` (5), menú «Ejemplos» verificado en gates F3/F4/F5; RNF-06: 6 tests de integración |
| RF-10 | Reiniciar al estado post-ensamblado | Test `reset_vuelve_al_estado_post_ensamblado` (registros y .data restaurados); botón verificado en GUI (F3) |

**RNF**: RNF-01 arranque 0.34 s ✓ · RNF-02 ensamblado 500 líneas 0.37 ms ✓ · RNF-03 1460 M instr/s (109 M con bp) ✓ · RNF-04 bundle 21 MB (.app) / 7.9 MB (.dmg) < 30 MB ✓ · RNF-05 i18n `es.json` completo ✓ · RNF-06 un test de integración por ejemplo (6 en total) ✓

**Estado final**: `cargo test` 34/34 verdes (+3 benchmarks RNF ejecutables aparte en release) · `cargo clippy --all-targets` 0 warnings · `cargo tauri build` exitoso · bundle en `src-tauri/target/release/bundle/`.

**Pendiente de verificación presencial** (la sesión autónoma no puede desbloquear la pantalla): re-captura visual del bundle final con el fix del overlay — la evidencia actual es la regla CSS presente en el `dist/` embebido y la determinismo del selector. Pasos: abrir `EmuRV.app`, confirmar que la ayuda NO está abierta al inicio y que el botón «Ayuda» la abre/cierra.

### Corrección de conteo (sin editar la entrada vieja)

La entrada de la línea ~119 dice "Tests: 49/49 verde". Ese conteo quedó corto respecto del estado real del repo al inicio de esta sesión: **55 pasando + 3 `#[ignore]` = 58** (ver línea base de Etapa 0 de esta sesión). La entrada vieja no se edita (append-only); este asiento la corrige hacia adelante. [Seguro — medido]
### Cierre de Etapa 3 (2026-06-11)

Saneamiento documental ejecutado (3 agentes + hilo principal):

- **Borrado `docs/PLAN_FRAMEBUFFER.md`** (guion bajo): borrador corrupto — mojibake binario desde la línea 28 — con mapa de memoria obsoleto (0x1002_xxxx). El plan vigente siempre fue `PLAN-FRAMEBUFFER.md` (guion medio). [Seguro]
- **`PLAN-FRAMEBUFFER.md`:** direcciones corregidas a las reales (paleta 0x9002_0000, control 0x9003_0000) con nota de divergencia fechada que conserva los valores originales del plan; motivo inferido del código: el framebuffer ocupa 76 800 bytes > 64 KiB, así que la paleta en 0x9001_0000 habría caído dentro de la región de video mapeada (0x20000). Semántica real del registro de control documentada (sin hook de escritura; publicación al final de cada tramo; refresco por timer ~30 Hz). RF1..RF4 → RB-1..RB-4. "Modo 13h clásico" → "inspirado en el modo 13h de VGA (que es 320×200), con resolución 320×240". [Seguro — verificado contra core/mod.rs, machine.rs, commands.rs, syscalls.rs]
- **`MEMORIA-GRAFICA.md`:** eliminada la contradicción interna de la base de paleta (4 menciones 0x9001→0x9002); ejemplo de paleta reescrito con código válido (entrada 10 = offset 30 dec = 0x1E; `li t0, base / li t1, 0xFF / sb t1, 0x1E(t0)`… — `sb` porque los canales son de 1 byte y 0x1E no está alineado a word; el original `sw 0xFF, dir` era sintaxis inválida); semántica del registro de control corregida en todo el doc con nota de divergencia; título reformulado (13h). [Seguro]
- **`ENSAMBLADOR.md`:** "35/35 tests" y "6 ejemplos" anotados como estado al cierre de v0.1; estado actual fechado: 55 verdes + 4 `#[ignore]` = 59, y 7 ejemplos (06-tetris ASCII reemplazado por 08-tetris-grafico; 07-franjas agregado en FB2). **`docs/README.md`:** ruta `~/Desktop/...` → `~/Documents/UNI/Tesis/EmuRV-TFG`. **`ARQUITECTURA.md`:** `/tests` reubicado en el árbol a `src-tauri/tests` + agregado `video.rs` faltante, con notas fechadas. [Seguro]
- **Frontend y núcleo:** comentarios RF1/RF2/RF4 → RB-1/RB-2/RB-4 en `src/pantalla.ts`, `src/main.ts`, `src/paneles/video.ts`; doc-comments de `core/mod.rs` actualizados (13h y semántica real del registro de control).
- La Etapa 2 (frontend) cerró en el commit `3d515e1`; su detalle quedó en el mensaje de commit y los gates se registran acá: 55+4 tests, clippy 0 warnings, tsc limpio, `vite build` ok.

## Etapa 4 — Mediciones y evidencia en release (2026-06-11)

Hardware: Mac Apple Silicon (Darwin 25.5.0). Build: `--release`. Comando: `cargo test --release -- --ignored --nocapture`. Cifras EXACTAS medidas hoy:

| Benchmark | Resultado | Requisito | Margen |
|-----------|-----------|-----------|--------|
| RNF-02 ensamblar 502 líneas | **1.56 ms** (2392 bytes de .text) | < 200 ms | ~128× |
| RNF-03 velocidad de ejecución | **1146.8 M instr/s** (40 000 004 instr en 0.035 s) | ≥ 10 M instr/s | ~115× |
| RNF-03 con hook de breakpoint activo | **89.9 M instr/s** (4 000 003 instr en 0.044 s) | ≥ 10 M instr/s | ~9× |
| FB1 conversión framebuffer→RGBA | **58 µs** promedio (10 corridas) | ≤ 3 ms | ~52× |
| FB4 FPS lado Rust bajo carga | **835 FPS** (1000 frames en 1.197 s) | ≥ 30 | ~28× |

Notas:
- La cifra 2-3 ms de FB1 que motivó la desviación registrada en Etapa 3 era de build debug; en release la conversión cuesta 58 µs (el test ahora mide en µs para no perder precisión por división entera de ms). El gate FB1 original (<2 ms) se cumple en release con margen ~34×. [Seguro — medido]
- **Bench nuevo `tests/fb4_fps.rs`** (gate FB4): el guest repinta el framebuffer completo (76 800 `sb`, ~230 000 instr/frame) y cierra cada frame con syscall 5 (el hook publica antes de bloquear — camino de producción verificado por `fb2_publica_antes_de_bloquear_por_entrada`), con las 1000 líneas pre-encoladas. Se verifica además que cada repintado publica exactamente una vez (flag sucio correcto bajo carga). El FPS de UI real (canvas a 30 Hz) queda **pendiente de medición humana** con la app abierta.
- La comparación RNF-03 sin/con hook (1146.8 → 89.9 M instr/s) reconfirma la decisión de ARQUITECTURA.md de instalar el hook de código solo cuando hay breakpoints. [Seguro]

### Hallazgos colaterales del diseño del bench (fuera de alcance, NO corregidos)

1. **El timeout de `Machine::run(ms)` no interrumpe un bucle apretado sin syscalls:** un guest que solo hace `sb/addi/bne` en bucle no retorna nunca del `emu_start` con timeout (verificado: el primer diseño del bench colgó indefinidamente con `run(5)`). Implicancia potencial en producción: el botón «Detener» podría no responder durante un bucle infinito puro (sin syscalls ni breakpoints), porque el hilo de emulación procesa comandos solo entre tramos. Pendiente de verificación humana en la app; si se confirma, es candidato a fix futuro (p. ej. hook de conteo o `emu_stop` desde otro hilo). [Probable — reproducido en test, no verificado en la GUI]
2. **El camino del `ebreak` tiene un bug latente:** al reanudar tras un `ebreak`, la ejecución cae con "instrucción inválida" más allá de la instrucción siguiente (el pc entregado al hook ya viene avanzado y el flujo post-hook avanza de nuevo). Ningún ejemplo ni test del proyecto usa `ebreak`, por eso nunca se vio. Registrado acá; corregirlo queda fuera del alcance de esta sesión (el segundo diseño del bench lo evitó usando syscall 5). [Seguro — reproducido con test sonda hoy]
### Bundle final (2026-06-11)

`cargo tauri build` exitoso: **EmuRV.app 21 MB** y **EmuRV_0.1.0_aarch64.dmg 7.2 MB** — ambos bajo el límite RNF-04 de 30 MB. A diferencia de la corrida de v0.1, esta vez `bundle_dmg.sh` completó el DMG estilado sin el plan B de `hdiutil`. [Seguro — medido]

## Etapa 7 — Revisión final multi-agente y cierre de sesión (2026-06-11)

### Revisión adversarial del código de la sesión

3 revisores por dimensión (núcleo Rust / frontend-frontera Tauri / tests-benchmarks) + 1 verificador adversarial por hallazgo (10 hallazgos reportados → 5 confirmados, 5 refutados). Los 5 confirmados se corrigieron en esta misma etapa:

1. **[medio] «Abrir»/«Guardar» habilitados durante la ejecución** (`main.ts`): abrir un archivo con el emulador corriendo dejaba el hilo de emulación vivo sin botón «Detener». Fix: `setModo` los deshabilita en `corriendo`/`esperando` (mismo criterio que «Exportar»).
2. **[bajo] Validación de breakpoints contra `line_to_addr` obsoleto tras editar**: flag `programaDesactualizado` (se setea con `docChanged` del editor, se limpia al ensamblar): con el texto editado se permite el alta (no hay forma confiable de validar) y el rechazo con aviso queda solo para programa vigente.
3. **[bajo] Puntos rojos en líneas sin código filtrados en silencio al ensamblar**: tras un ensamblado exitoso se podan los marcadores sin entrada en `line_to_addr` con aviso por consola (`breakpoint.lineaSinCodigo`), antes de enviar los breakpoints.
4. **[bajo] Carrera en `video.ts` al detener**: si el volcado final llegaba con una lectura en vuelo, se perdía. Fix: flag `repetirAlTerminar` que reejecuta `refrescar()` al terminar la lectura en curso.
5. **[bajo] Assert de `fb4_fps` sobredeclaraba**: decía verificar "cada frame publicado exactamente una vez"; lo que verifica es que ningún frame distinto queda suprimido por el flag sucio (la supresión de frames idénticos la cubre `fb2_publicacion_con_flag_sucio`). Mensaje y comentario corregidos; esta entrada corrige también la frase "flag sucio correcto bajo carga" de la entrada de Etapa 4. Cifra re-medida tras los cambios: **840 FPS** (vs 835 de la primera corrida — estable).

Refutados (ejemplos): "ebreak se ignora en Ejecutar" (el camino tiene el bug latente ya registrado, pero el reporte específico no aplicaba), "para_sesion_nueva resetea seq con mutex envenenado", "desuscripción no cubre promesas en vuelo".

### Verificación de capítulos de tesis (fuera del repo, en `Tesis/partes/`)

6 verificadores (uno por capítulo + índice/plantilla V). Resultado: **5/5 capítulos APROBADOS CON CORRECCIONES MENORES, 0 citas inválidas, 0 datos de evaluación inventados**. Las correcciones (≈15: teclado no es dispositivo emulado, instalador Windows inexistente aún, 35 tests eran de la suite v0.1 completa, atribuciones de "registrado en bitácora" que eran propuestas del capítulo, licenciamiento aún no definido, etc.) se aplicaron y se re-verificó la ausencia de las frases observadas.

### Suite final de la sesión

- `cargo test`: **55 pasan + 5 `#[ignore]` = 60** · `cargo clippy --all-targets`: 0 warnings · `npx tsc --noEmit`: limpio · `npx vite build`: ok.
- `cargo test --release -- --ignored`: 5/5 benchmarks verdes (cifras en Etapa 4; fb4 re-medido 840 FPS).
- `cargo tauri build` final con todos los fixes: **EmuRV.app 21 MB / DMG 7.2 MB**.

Sesión ultracode completada: Etapas 0-7 del plan ejecutadas. Los capítulos I-IV y VI + REFERENCIAS.md (37 entradas: 31 verificadas, 5 corregidas, 1 descartada por inverificable) + INDICE-MEMORIA + plantilla vacía del Cap. V viven en `Tesis/partes/`, fuera de este repo. El Capítulo V NO se escribió (no hay datos de evaluación; la plantilla lo advierte). Esta rama NO se mergea a main (decisión humana pendiente).

## TEC0 — Spike de interrumpibilidad y mecanismo MMIO de teclado (2026-06-12)

Sesión ultracode sobre rama nueva `feature/teclado` (desde `feature/framebuffer`, plan en `Tesis/PLAN-TECLADO.md`). Dos PoC contra Unicorn crudo (tests `spike_count.rs` y `spike_mmio.rs`, 12 tests) ANTES de tocar `Machine`. Cifras release, Apple Silicon:

### PoC A — Interrumpibilidad por count: VERDE

- `emu_start(pc, NEVER, 0, count=10M)` sobre `j .` (bucle apretado puro) **retorna en 46.8 ms** — lo que el timeout jamás logró (hallazgo BITACORA Etapa 4). Timeout+count conviven (gana count).
- **Count exacto al 100%**: frío, con caché JIT caliente, y con count cambiado entre corridas (verificado con contador real en x5). `count=1` tras corridas con count>0 ejecuta exactamente 1 instrucción SIN `ctl_flush_tb` — el bug JIT v0.1 solo aparece tras corridas LIBRES (count=0).
- Throughput con slicing: **430.1 M instr/s** con tramos de 10M (100M instr, pc retomado entre tramos, sin pérdida ni duplicación); incluso con tramos de count=1000: 237.6 M instr/s (~4.2 µs de overhead por re-entrada a `emu_start`).

### PoC B — Hook MEM_READ para MMIO: VERDE

- Patrón MMIO estándar funciona en unicorn-engine 2.1.5: `mem_write` DENTRO del callback MEM_READ deja el valor fresco visible a la lectura del guest (sin recursión de hooks ni panic de borrow).
- Semántica leer-consume estilo RARS verificada en registros del guest: CONTROL=1 con dato → leer DATA da el byte y consume → CONTROL=0. `lb` y `lw` funcionan (el hook ve el size real).
- Dispara igual con caché JIT caliente. Costo con la página NO accedida: **nulo** (relación 1.004; 2.668 vs 2.680 G instr/s en bucle aritmético).
- Alternativa `mmio_map` (callback devuelve el valor) también verificada verde, pero **se elige hook + memoria con respaldo real**: las lecturas de host (visor de memoria de la UI) no disparan hooks y por tanto NO consumen el dato del teclado — importa en un emulador didáctico. [Seguro — ambos PoC reproducibles]

### Decisiones TEC0

1. **Interrumpibilidad: tramos por count** (`RUN_SLICE_COUNT` ≈ 1M instr ≈ 2.3 ms a 430 M/s), con timeout de respaldo opcional. Resolverá el hallazgo "Detener no responde en bucle apretado".
2. **Teclado: hook MEM_READ** sobre página 0xFFFF_0000 (RECEIVER_CONTROL +0, RECEIVER_DATA +4, direcciones RARS/MARS).
3. El refresco `emu://registros` se desacopla del tramo (con tramos de ~2 ms inundaría la UI): cadencia ~100 ms por `Instant`, a definir en TEC1.

Limitaciones registradas por los propios PoC: B5 no mide guests con muchas cargas a RAM normal (chequeo de rango del hook en camino lento); el patrón combinado con los hooks existentes de `machine.rs` se prueba recién en TEC2. Gate TEC0 ✓: 67 tests + 5 ignore, clippy 0, tsc limpio.

## TEC1 — Detener responde siempre: tramos por count (2026-06-12)

- `Machine::run_slice(timeout_ms, max_instr)` nuevo; `run(timeout_ms)` delega con max_instr=0 (cero call sites rotos). Count agotado → `Paused` (tramo terminado, NO pausa de usuario; el bucle solo emite pausa real bajo `stop_flag`, lógica intacta).
- `commands.rs`: `RUN_SLICE_COUNT = 1_000_000` (~2-3 ms/tramo); `stop_flag` y comandos auxiliares se atienden en CADA tramo → **latencia de Detener ~2-3 ms** incluso en `loop: j loop` puro. El refresco `emu://registros` se desacopló a cadencia de 100 ms por `Instant` (sin eso: ~500 Hz de eventos). `publish_frame` queda por tramo (el flag sucio lo abarata y el último frame nunca queda sin publicar al detenerse). `RUN_SLICE_MS=100` queda como red de seguridad.
- `tb_de_corrida`/flush se conserva intacto (conservador: el spike mostró que tras count>0 no haría falta, pero el flush de `step()` es barato y cubre el caso histórico count=0). [Seguro]
- Tests nuevos (`tec1_detener.rs`): `detener_interrumpe_bucle_apretado` (bucle apretado retorna Paused, guardián anticuelgue), `slicing_no_pierde_instrucciones` (contador exacto: 300 000), `run_sin_limite_sigue_funcionando`. Regresiones de la familia JIT: `paso_funciona_inmediatamente_despues_de_reset` ✓, `breakpoint_puesto_despues_de_correr_frena` ✓, integración 6/6, fb2 5/5, fb3 2/2.
- **RNF-03 re-medido (release)**: corrida libre 1124.5-2310.4 M instr/s; **con slicing 302.5-397.0 M instr/s** (benchmark nuevo `rnf03_velocidad_con_slicing`, #[ignore]; el viejo usa `run(0)` y no refleja el cambio); con breakpoint 91.6-157.1 M. Gate ≥10 M: superado ~30-40×. Costo del slicing ~3-6× por re-entrada al JIT — esperado por el spike. [Seguro — medido]
- **Cierre del hallazgo de Etapa 4 (2026-06-11)**: "el timeout de run() no interrumpe bucles apretados → Detener podría no responder" queda RESUELTO por diseño con los tramos por count (pendiente la verificación visual humana con la app).
- Edge case preexistente registrado (no tocado, fuera de alcance): borde de tramo cayendo exacto sobre un breakpoint puede suprimirlo una vez (`suppress_bp`); ya existía con tramos por timeout y con breakpoints el hook baja la velocidad ~100×, así que el tramo corta por timeout igual que antes. [Probable]

Gate TEC1 ✓: 70 tests + 6 ignore, clippy 0, tsc limpio.

## TEC2 — Dispositivo de teclado MMIO compatible RARS/MARS (2026-06-12)

- **Mapa**: página 0xFFFF_0000 (4 KB) con `RECEIVER_CONTROL` (+0, bit 0 = hay dato; bit 1 —interrupciones, semántica MARS— RESERVADO y documentado como no implementado) y `RECEIVER_DATA` (+4, leerlo CONSUME). Mismas direcciones que RARS/MARS a propósito: el material didáctico existente aplica sin cambios. [Seguro]
- **Mecanismo (decisión TEC0)**: hook `MEM_READ` permanente sobre los 8 bytes; en cada lectura refresca CONTROL = estado de la cola y, si es DATA, escribe el byte poppeado ANTES de que la lectura del guest complete (patrón validado en spike B1-B4). Página con respaldo real: el visor de memoria de la UI lee sin disparar hooks → NO consume teclas (razón del descarte de `mmio_map`, spike B6). `lb` desalineado se normaliza con `& !0x3`.
- **Cola**: `KeyboardShared` (`VecDeque<u8>` cap 64, descarte del más viejo al llenarse, sin panic) en `MachData` como espejo de `video_shared`; el Arc viaja a la `Session` por el canal `init` existente (Unicorn no es Send: la máquina nace dentro del hilo). Comando Tauri `push_key(code)` con recuperación de mutex envenenado (mismo criterio que `publicar_frame`).
- `load()`/`reset()`: cola vaciada + CONTROL/DATA en 0 — sin teclas fantasma entre sesiones ni reinicios.
- **Cableado anticipado para TEC3**: `MachData.stop_request: Arc<AtomicBool>` conectado al `stop_flag` de la sesión vía `set_stop_request` (setter para no romper call sites); aún sin uso — lo consumirá el sleep interrumpible de syscall 32.
- Tests (`tec2_teclado.rs`, 6): lectura CONTROL/DATA/CONTROL con lw y con lb; cola vacía → 0/0; overflow 65→64 perdiendo la más vieja; reset limpia; **eco end-to-end**: programa asm que pollea CONTROL, lee DATA, imprime con syscall 11 y sale con Esc — cola "ab\x1b" inyectada → consola "ab", Exited{0}.

Gate TEC2 ✓: 76 tests + 6 ignore (integración 6/6, regresiones JIT y fb* intactas), clippy 0, tsc limpio.

## TEC3 — Syscalls de tiempo 30 y 32 (2026-06-12)

- **a7=30** (tiempo del sistema en ms, compatible RARS: bajos en a0, altos en a1): reloj **monotónico anclado a época** — al primer uso se captura `(Instant, SystemTime)` en un `OnceLock` estático y se devuelve `época_ancla + transcurrido_monotónico`. Diferencia documentada con RARS: `System.currentTimeMillis()` puede saltar hacia atrás (NTP); EmuRV garantiza valor no decreciente dentro del proceso. Ancla compartida entre máquinas del proceso: deseable para monotonía global. [Seguro]
- **a7=32** (dormir a0 ms, u32): publica el frame ANTES de dormir (mismo patrón que `read_int`: la pantalla se ve durante el sleep) y duerme contra deadline en trozos ≤10 ms chequeando `stop_request` (el Arc cableado en TEC2); al detectar stop corta YA y hace `emu_stop()` sin marcar error — semántica de pausa de usuario idéntica a la rama `Cancel` de `read_int`. Latencia de corte ≤10 ms.
- Mensaje de "syscall desconocida" actualizado (lista ahora 1, 4, 5, 10, 11, 30, 32).
- Tests (`tec3_tiempo.rs`, 4): plausibilidad y no-decrecimiento del par a1:a0 entre dos llamadas; sleep de 80 ms → delta ∈ [60, 5000) ms; **sleep de 10 000 ms interrumpido a los ~50 ms retorna `Paused` en <1 s** (y ≥30 ms: durmió de verdad), con guardián anticuelgue; animación durante sleep: 3 frames con color distinto + sleep 30 ms → `seq ≥ 2` (el publish pre-sleep es el mecanismo).

Gate TEC3 ✓: 80 tests + 6 ignore, clippy 0. (tsc se gatea con TEC4, que está en curso sobre src/.)

## TEC4 — Frontend: captura de teclado (2026-06-12)

- `src/teclado.ts` (módulo único de mapeo): imprimibles ASCII (<128), Enter 10, Esc 27, Backspace 8, flechas 0x80-0x83; `null` con Ctrl/Cmd/Alt (no robar atajos) y para teclas no mapeadas. El criterio de `preventDefault` es exactamente "mapeada": flechas/espacio no scrollean mientras se captura.
- Panel embebido (`video.ts`): canvas con `tabindex=0`; la captura SOLO arranca por click explícito del usuario en el canvas (jamás roba foco al editor); indicador i18n «Teclado capturado — clic afuera para soltar» visible con foco; blur suelta. Los listeners viven en el canvas (no hay listener global que limpiar); `blur()` explícito al ocultar el panel y al desacoplar.
- Ventana desacoplada (`pantalla.ts`): keydown a nivel window siempre (toda la ventana es la pantalla), mismo módulo de mapeo.
- Ayuda: filas de syscalls 30/32 + sección nueva "Teclado y tiempo" (mapa MMIO con bit 1 documentado reservado, tabla de códigos, ejemplo de polling de 5 líneas). Sigue el patrón existente de la Ayuda (prosa por i18n, literales técnicos en el template). `api.pushKey` fire-and-forget.
- **Pendiente de verificación visual humana (no se declara cumplido):** foco/indicador real al click, preventDefault del scroll en WKWebView, no-interferencia con CodeMirror ni atajos del sistema, flujo end-to-end tecla→cola→guest, estética del indicador.

Gate TEC4 ✓: tsc limpio, vite build ok (47 módulos).

## TEC5 — Ejemplos en tiempo real: 09-eco-teclado y 10-tetris-tiempo-real (2026-06-12)

- **`09-eco-teclado.asm`** (didáctico mínimo, comentado línea por línea: qué es MMIO, polling, leer-consume, por qué dormir): pollea CONTROL → lee DATA → Esc termina → imprime con syscall 11 y dibuja bloque 8×8 de color (código&7)+1 — el &0xF sugerido por el plan se cambió porque la entrada 16 de la paleta VGA es negra y la 'o' de "hola" habría dejado bloque invisible. El bucle de espera chequea CONTROL ANTES de dormir 10 ms: con teclas encoladas jamás duerme. Tests: eco de "hola\x1b" → consola "hola", Exited{0}, ≥4 bloques no negros; y oráculo anti-quemado: 40 teclas+Esc en <200 ms de run (un diseño que durmiera antes de chequear tardaría ≥400 ms).
- **`10-tetris-tiempo-real.asm`**: motor de juego COPIADO LITERAL del 08 (LCG semilla 0x1234, colisiones, fijado, líneas, puntaje — sección marcada en el fuente); solo cambió el bucle de entrada: polling MMIO no bloqueante (a/A/← izquierda, d/D/→ derecha, w/W/↑ rotar, s/S/↓ caída suave, Esc sale), gravedad por syscall 30 contra `gravedad_ms: .word 500` (`ultimo_descenso` se inicializa con syscall 30 en main — sin eso el primer delta sería gigante), resta en 32 bits con `bltu` (wrap de ~49 días documentado como irrelevante), vueltas sin evento duermen 15 ms, rutina nueva `dibujar_pieza` pinta la pieza activa SOLO en framebuffer (jamás en `tablero[]` — la identidad lógica con el oráculo del 08 se preserva). **08 y su oráculo fb3: INTACTOS y verdes.**
- Tests del 10: (a) oráculo determinista con gravedad congelada (0x7FFF_FFFF escrito vía símbolo `gravedad_ms` con `program.symbols`, mismo mecanismo que fb3): secuencia que ejercita letras Y flechas → pieza J fijada exacta en (18,1),(19,1),(19,2),(19,3) color 7, invariante ocupadas==4, "SCORE: 0", Exited{0}; estable en 5 corridas. (b) smoke: gravedad 50 ms, cola vacía, `run_slice` con tope 2 s muestreando `ultimo_descenso` entre tramos → ≥2 descensos por reloj; centinela `stop_request` + guardián.
- Menú de ejemplos + i18n: «Eco de teclado (tiempo real)» y «Tetris en tiempo real» agregados (08 sigue en el menú).

Gate TEC5 ✓: 84 tests + 6 ignore (fb3 verde, integración 6/6), clippy 0, tsc limpio, vite build ok.

## TEC6 — Docs, revisión adversarial y cierre de sesión (2026-06-12)

### Documentación

`docs/TECLADO.md` nuevo (mapa MMIO, semántica leer-consume con advertencia lb/extensión de signo para flechas 0x80+, tabla de códigos, syscalls 30/32, decisiones del spike con cifras, limitaciones). ARQUITECTURA.md con notas fechadas (bucle por count, dispositivos emulados), README/docs/README con las features y los 9 ejemplos, MEMORIA-GRAFICA con referencia cruzada. Matiz nuevo documentado: el visor de memoria muestra el último valor refrescado de CONTROL/DATA, no el estado instantáneo de la cola.

### Revisión adversarial del diff (d047ccf..HEAD)

3 dimensiones (concurrencia/hook, syscalls de tiempo, ejemplos/frontend) + verificador adversarial por hallazgo: **10 confirmados, 0 refutados**. Corregidos en esta fase:

1. **stop_flag colgado [medio]:** Reiniciar/Detener dejaban el flag en true y el próximo «Paso» sobre syscall 32 salteaba el sleep (dependencia de historia de botones). Fix: los brazos Step y Reset limpian el flag como ya hacía Run.
2. **Breakpoint salteado en borde de count [medio]:** si el tramo agotaba el count EXACTO sobre la dirección de un breakpoint, la continuación lo suprimía y se ejecutaba de largo sin reportar (reproducido determinista). Fix: la supresión del bp del pc de arranque vale solo para el PRIMER tramo de una orden Run (`run_slice_continuation` nuevo); test de regresión rojo→verde `breakpoint_en_borde_de_count_se_reporta_en_la_continuacion`.
3. **Hilo zombi al re-ensamblar durante un sleep [medio]:** la sesión vieja quedaba dormida hasta a0 ms (49.7 días con `li a0,-1`) reteniendo su Unicorn y podía emitir registros viejos al despertar. Fix: `assemble` setea el stop_flag viejo + manda `Cancel` antes de reemplazar (mismo gesto que Detener).
4. **AltGr bloqueado [medio]:** `mapearTecla` rechazaba Alt → en teclados español/LATAM (el público de esta tesis) @ # [ ] eran imposibles de tipear. Fix: imprimibles 32-126 se aceptan con Alt/AltGr (incluido el pseudo-Ctrl del AltGr de Windows vía `getModifierState("AltGraph")`); Ctrl/Cmd reales siguen bloqueados.
5. **Eco de caracteres de control [bajo]:** el 09 imprimía basura con flechas/Backspace por syscall 11. Fix: filtro didáctico — solo eco de imprimibles 32-126 y Enter; el resto solo dibuja su bloque.

**Limitación aceptada y documentada (cluster sleep, 5 hallazgos restantes):** la syscall 32 duerme dentro del hook de Unicorn, así que durante un sleep no se atienden `read_mem`/`get_state` (visor congelado) y un «Paso» sobre un sleep largo no ofrece «Detener» en la UI (el botón está deshabilitado en ese modo). Vías de rescate REALES: «Reiniciar» y «Ensamblar» despiertan el sleep en ≤10 ms (setean el mismo stop_request; verificado por los fixes 1 y 3). a0 es u32 sin cota (hasta ~49.7 días), documentado en TECLADO.md. El rediseño (sleep como deadline entre tramos del bucle de comandos) queda como propuesta fuera de alcance. [Probable — el rediseño es la solución completa; las vías de rescate están testadas]

### Suite final y cifras de cierre (release, Apple Silicon)

- `cargo test`: **85 pasan + 6 `#[ignore]` = 91** · clippy 0 warnings · tsc limpio · vite build ok.
- Benchmarks release: RNF-02 **1.01 ms** · RNF-03 libre **1562.4 M instr/s** · **con slicing 330.6 M instr/s** (gate ≥10 M: ~33×) · con breakpoint 104.9 M · FB1 conversión **105 µs** · FB4 **831 FPS** lado Rust.
- **Latencia de tecla percibida** = tramo de emulación (~2-3 ms) + timer UI (33 ms) + intervalo de polling del programa (10-15 ms en los ejemplos) ≈ **<50 ms** — dentro del objetivo del checklist humano.
- `cargo tauri build`: **EmuRV.app 21 MB / DMG 7.2 MB** (<30 MB ✓).
- Hallazgo de Etapa 4 (2026-06-11) "Detener podría no responder": **CERRADO** (TEC1 + test + fixes de esta fase).

## PENDIENTE DE VERIFICACIÓN VISUAL DEL USUARIO (sesión teclado)

1. **Jugar `10-tetris-tiempo-real`** (build nuevo): pieza cae sola cada `gravedad_ms` (500); WASD y flechas responden <50 ms percibidos; Esc sale limpio.
2. **Detener** durante: bucle infinito puro (`inicio: j inicio`), Tetris RT corriendo, y sleep largo (syscall 32 con 10000) — los tres deben frenar al instante. (Durante un «Paso» sobre sleep largo, el rescate es «Reiniciar».)
3. Captura de teclado: click en pantalla captura (indicador «Teclado capturado» visible), click en editor suelta, flechas NO scrollean capturado; en ventana desacoplada las teclas van directo. Probar signos con AltGr/Option (@, #, [) en teclado español/LATAM.
4. Consola clásica intacta: «Suma interactiva» (syscall 5) pide y lee igual que siempre.
5. Heredadas de FB aún abiertas: refresco con ventana desacoplada al cambiar de programa; panel oculto al desacoplar/arrancar; FPS real del canvas.
6. Captura nueva para la tesis: `docs/capturas/fb5-tetris-tiempo-real.png` (+ las 3 pendientes de FB2/FB3).
7. Tras validar todo: decisión con tutor — aval framebuffer+teclado → merge a main + SPEC v0.2.

Sesión TEC0-TEC6 completada. Rama `feature/teclado` (desde `feature/framebuffer`). **NO mergeada.**

## RP1 Fase 1 — Test sonda del bug de ebreak (2026-06-12)

Plan `docs/PLAN-RP1-ebreak.md` (punto RP-1 del roadmap). Test `tests/rp1_ebreak.rs::ebreak_pausa_y_reanuda_en_la_instruccion_siguiente`: programa con `ebreak` en 0x0040_0004 entre `li t0,1` y `li t1,2`/`li t2,3`/exit; corre con `run_slice(0, 1000)` y reanuda con `run_slice_continuation`.

**Evidencia observada [Seguro — test reproducible]:**
- La pausa SÍ ocurre y los efectos previos están bien: razón no-Exited, t0=1, t1=0 (nada posterior ejecutado).
- **pc en la pausa = 0x0040_0004 — la dirección DEL `ebreak`, SIN avanzar** (esperado por convención RARS/gdb: 0x0040_0008). El assert falla exactamente ahí.
- Dato nuevo respecto del registro original del bug (Etapa 4 del 2026-06-11, que observó caída en «instrucción inválida» más allá de la siguiente): en el camino `run_slice` con count el pc queda EN el `ebreak` (no avanzado); el "doble avance" del registro original se observó por el camino `run(0)`. La Fase 2 debe explicar ambas observaciones con una sola causa raíz.

Estado de la suite: 17 suites verdes intactas + SOLO el sonda en rojo (permitido por la compuerta F1). Clippy 0 warnings.

## RP1 Fase 2 — Causa raíz del bug de ebreak (2026-06-12)

Instrumentación temporal de la rama `CAUSE_BREAKPOINT` (eprintln del pc entregado, palabra en pc y pc-4) + sonda descartable con `run_slice`, `run_slice_continuation` y `step()`. Evidencia [Seguro — reproducible]:

- **El hook de interrupción JAMÁS se dispara para `ebreak`**: cero trazas de la rama instrumentada en ningún camino. La rama `CAUSE_BREAKPOINT` de `handle_trap` es **código muerto desde que existe** — por eso "nunca se vio": no es que el pc se avanzara dos veces; es que el flujo nunca pasó por ahí.
- Lo que ocurre de verdad: **Unicorn 2.1.5 (RISC-V) no traduce `ebreak` en una llamada al hook `INTR`; `emu_start` aborta con `uc_error::INSN_INVALID`**. `interpret()` lo convierte en el mensaje didáctico de "instrucción inválida" (camino `Err` genérico).
- Valores exactos: con count instalado (camino de PRODUCCIÓN: `run_slice` y `step` siempre llevan count) el pc del error queda **EN la dirección del `ebreak`** (`run_slice` → `RuntimeError{instrucción inválida en 0x00400004}`, pc=0x00400004; `step` → ídem en 0x00400000). En corrida libre (`run(0)`, sin count hook — solo tests/benchs) el pc cae en ebreak+8 (observado en el spike del 2026-06-11: error en 0x00400024 con ebreak en 0x0040001C).
- La continuación desde ese estado re-ejecuta el `ebreak` y repite el error para siempre.
- **Corrección del diagnóstico original (Etapa 4, 2026-06-11):** la entrada vieja atribuía el bug a un "doble avance de pc" — interpretación errónea del pc del mensaje en corrida libre. No se edita (append-only); esta entrada la corrige hacia adelante. [Seguro]

Diseño del fix que habilita esta evidencia (Fase 3): rescatar el caso en `interpret()` — si `emu_start` falla con `INSN_INVALID` y la palabra en pc es exactamente `EBREAK` (0x0010_0073), es una pausa por software: fijar pc = dir+4 UNA vez y devolver una razón de pausa propia. La rama del hook se conserva como defensa documentada por si futuras versiones de Unicorn sí entregan el trap. Instrumentación y sonda temporal eliminadas.

## RP1 Fase 3 — Fix de la pausa por ebreak (2026-06-12)

**Diseño aplicado (según causa raíz de F2):**
- **Rescate en `Machine::interpret()` [el camino real]:** si `emu_start` falla con `INSN_INVALID` y la palabra en el pc es exactamente `EBREAK_WORD` (0x0010_0073, ahora const compartida en `core/mod.rs`), no es un error: es la pausa por software. Se fija pc = dir_ebreak + 4 UNA sola vez y se devuelve la variante nueva **`StopReason::EbreakPause { addr }`**. En el camino con count (producción: `run_slice` y `step` siempre llevan count) el pc del error queda exactamente EN el ebreak, así que la detección por palabra-en-pc es determinista.
- **Variante nueva `EbreakPause`** en vez de reusar `Breakpoint` o `Paused`: `Paused` habría hecho que el bucle de Ejecutar siguiera de largo (trata Paused como tramo terminado), y `Breakpoint` mentiría la causa en la UI. Costo: un caso más en el switch del frontend + clave i18n «Pausado por ebreak (línea {line})». La línea resaltada es la SIGUIENTE al ebreak (la próxima a ejecutar), consistente con la semántica de paso. [Seguro]
- **Rama `CAUSE_BREAKPOINT` del hook: conservada como defensa documentada** (hoy inalcanzable — F2). Si una versión futura de Unicorn entregara el trap: localiza la dirección real del ebreak (palabra en pc o pc-4), fija pc=dir+4, marca `MachData.ebreak_pause` y `interpret()` la convierte en la misma `EbreakPause`. `reset()`/`load()` limpian la marca (sin pausas fantasma entre sesiones).
- Convención de pc documentada en código, espejo del comentario de `CAUSE_ECALL_*`.
- Limitación conocida registrada: en corrida libre (`run(0)`, sin count — solo tests/benchs legacy, ningún camino de UI) el pc del INSN_INVALID cae en ebreak+8 y el rescate no aplica: queda el mensaje genérico. No se rescata desde pc-4/pc-8 a propósito: esa heurística produce falsos positivos peligrosos (ebreak como última instrucción entraría en bucle de pausas eternas al confundir el fin del programa con un nuevo ebreak). [Seguro — analizado en F2/F3]

**Compuerta F3 ✓:** test sonda de F1 VERDE (pausa con pc=ebreak+4, t0=1/t1=0 en pausa; reanudación → Exited{0} con t1=2, t2=3) · 18 suites verdes · clippy 0 · tsc limpio.

## RP1 Fase 4 — Cobertura de bordes del ebreak (2026-06-12)

Seis tests nuevos en `tests/rp1_ebreak.rs` (7 totales con el sonda):

1. **ebreak como última instrucción**: pausa correcta; reanudar cae en el `RuntimeError` didáctico «siguió de largo» (la palabra después del ebreak no es EBREAK → el rescate no re-aplica), con assert explícito anti-bucle (jamás otra EbreakPause). Confirma el análisis de F3: rescatar desde pc-4 habría producido pausas eternas acá.
2. **Dos ebreak consecutivos**: dos pausas exactas, una por cada dirección; el pc tras la primera queda exactamente sobre el segundo.
3. **ebreak en bucle ×5**: exactamente 5 pausas, las 5 con la misma dirección, contador final correcto.
4. **step() sobre el ebreak**: devuelve `EbreakPause` (no StepDone) con pc en la siguiente; el paso posterior ejecuta normal. Sin sorpresas.
5. **Convivencia con breakpoints de UI**: bp antes del ebreak ✓, ebreak antes del bp ✓, y el sub-caso fino:

**Hallazgo real del sub-caso 5C [Seguro — reproducido]:** en el camino REAL de la GUI (orden Run nueva → primer tramo `run_slice` con supresión del bp del pc de arranque), un breakpoint de UI puesto EXACTAMENTE en `dir_ebreak+4` se suprimía y jamás se reportaba: la supresión no distinguía si la pausa previa fue por ESE bp (supresión legítima) o por ebreak (incorrecta).

**Fix aplicado (dentro del criterio del plan: "ninguno interfiere con el otro"):** `Machine` ahora recuerda `last_bp` — la dirección del último `Breakpoint` REPORTADO (None si la última parada fue por otra causa; `run_slice_imp` y `step` lo actualizan en cada parada). El primer tramo de una orden Run suprime el bp del pc de arranque SOLO si `last_bp == pc`. Beneficio adicional: también cierra el residuo del hallazgo de TEC6 («breakpoint en borde de count») para el caso Run→stop→Run entre ÓRDENES (el fix de TEC6 solo cubría continuaciones dentro de la misma orden). `step()` no cambia (un paso nunca frena en el bp del pc actual — semántica correcta). Test nuevo del camino GUI: `bp_en_la_siguiente_al_ebreak_se_reporta_en_run_nuevo` (rojo sin el fix, verde con él).

**Compuerta F4 ✓:** 7/7 tests de rp1_ebreak + 18 suites verdes + clippy 0.

## RP1 Fase 5 — Documentación y cierre (2026-06-12)

- **SPEC.md**: nota fechada — `ebreak` soportado como pausa por software (complemento de RF-03/RF-04, convención RARS/gdb), declarado RECIÉN después del fix, como exige el criterio de aceptación del roadmap.
- **ENSAMBLADOR.md §4.1**: agregada la fila de instrucciones de sistema (ecall, ebreak, fence) con la nota fechada del comportamiento de pausa.
- **Ayuda integrada**: párrafo nuevo en la sección de syscalls («ebreak pausa el programa… reanudar con Ejecutar o Paso desde la instrucción siguiente»), clave i18n `ayuda.ebreak`.
- **Ejemplo didáctico opcional**: NO se agregó (decisión registrada: el ejemplo + test de integración + menú + i18n supera los 15 minutos del criterio del plan; el repertorio de 10 ejemplos ya cubre los conceptos y `ebreak` queda documentado en Ayuda y ENSAMBLADOR). Queda como propuesta para cuando se agregue material de depuración.
- **docs/ROADMAP-PRO.md**: NO tocado (lo gestiona la sesión coordinadora).

**Convención de pc cerrada [Seguro]:** Unicorn 2.1.5 no entrega el trap de `ebreak` al hook INTR; el caso real es `Err(INSN_INVALID)` de `emu_start` con el pc EXACTAMENTE en el `ebreak` (camino con count = producción). El rescate vive en `Machine::interpret()` (palabra en pc == 0x0010_0073 → pc=dir+4 una sola vez → `StopReason::EbreakPause{addr}`); la rama `CAUSE_BREAKPOINT` del hook queda como defensa documentada para versiones futuras. Tests: `rp1_ebreak.rs` (7) — sonda de F1 en verde, bordes de F4 y camino GUI del fix `last_bp`.

**Compuerta F5 (cierre RP1) ✓:** `cargo test` 18 suites verdes (incl. 7/7 rp1_ebreak) · `cargo clippy --all-targets` 0 warnings · `npx tsc --noEmit` limpio · `cargo tauri build` exitoso (EmuRV.app 21 MB).

## RP3 Fase 1 — Binario emurv-cli con subcomando asm (2026-06-12)

Plan `docs/PLAN-RP3-cli.md`. `src-tauri/src/bin/emurv-cli.rs` + `[[bin]]` explícito en Cargo.toml (convive con el bin Tauri; compilar el CLI no exige frontend).

- **Parsing manual con `std::env` [Seguro]:** dos subcomandos y cuatro flags no justifican `clap` (regla de dependencias de CLAUDE.md); el costo es ~60 líneas de match legibles. Si el CLI creciera (más flags, autocompletado), reabrir con BITACORA.
- **Canales [Seguro — regla crítica del plan]:** stdout queda EXCLUSIVO para la salida del programa invitado; uso, versión, errores y avisos van TODOS por stderr (incluso `--help`: así un script que capture stdout jamás recibe texto del CLI). Códigos: 0/1/2/3/124 según diseño acordado.
- **`asm`:** lee fuente → `assemble()` → `export_elf()` → escribe (default: entrada con extensión `.elf`). Errores de ensamblado conservan el formato didáctico en español (uno por línea) + resumen, exit 2; errores de E/S exit 1.
- Estructura: un solo archivo (un módulo separado en `src/bin/` se interpretaría como otro binario; si `run` crece en F2, la alternativa documentada es el layout `src/bin/emurv-cli/main.rs`). `run` responde "todavía no disponible" + exit 3 hasta F2.
- Tests (`tests/rp3_cli.rs`, como proceso real vía `CARGO_BIN_EXE`): ELF con magic `\x7fELF` y class 32; error de sintaxis → exit 2 con diagnóstico en español y stdout vacío; sin argumentos → exit 3 con uso y stdout vacío.

Compuerta F1 ✓: 3/3 nuevos + 19 suites verdes, clippy 0.

## RP3 Fase 2 — Subcomando run con puente de consola (2026-06-12)

- **Arquitectura del puente [Seguro]:** la máquina se BLOQUEA dentro del tramo al esperar entrada (syscall 5: `input.recv()` dentro del hook) — en un solo hilo el CLI se trabaría. El puente corre en un hilo propio: `Output` → stdout con flush por evento y SIN saltos propios; `InputRequest` → una línea de stdin → `Line` (sin el salto final), EOF → `Cancel` + marca. Es el espejo exacto del modelo GUI (UI en un hilo, emulación en otro), con la máquina en el hilo principal porque Unicorn no es Send.
- **Bucle de tramos de producción:** `run_slice`/`run_slice_continuation` con SLICE=1M (espejo documentado de `RUN_SLICE_COUNT`, privado de commands.rs) + timeout de respaldo 100 ms. `--max-instr` se descuenta por tramo EMITIDO (cota superior: un tramo cortado antes cuenta entero — precisión suficiente para acotar, documentado en código). `--timeout-s` con doble despertador: hilo temporizador que setea `stop_request` (corta sleeps de syscall 32 en ≤10 ms) Y manda `Cancel` (despierta un `read_int` bloqueado).
- **Semántica headless aplicada (diseño acordado):** `EbreakPause`/`Breakpoint` → aviso por stderr («ebreak en 0x… — continuando») y la corrida sigue con continuación. `Paused` con marca de EOF → diagnóstico didáctico + exit 1; `Paused` por tramo/stop → según límite.
- **Cierre sin deadlock [Probable — razonado y testeado]:** al salir, `drop(maquina)` cierra el canal y el puente termina tras drenar; si el puente quedó bloqueado leyendo stdin, el canal está vacío por construcción (la máquina no produce salida mientras espera entrada), así que se sale sin join. Ventana de carrera teórica (puente entrando a stdin justo tras el chequeo) es inocua por el mismo argumento.
- Tests como proceso (5 nuevos, guardián anticuelgue propio con try_wait+kill): 01 stdout EXACTO; 02 con pipe "8\n34\n" → 42; 03/04/05 salidas conocidas; EOF prematuro → exit 1 con diagnóstico; bucle infinito + --max-instr 1M → exit 124 acotado con stdout limpio.

Compuerta F2 ✓: 8/8 rp3_cli + 20 suites verdes, clippy 0.

## RP3 Fase 3 — Bordes headless del CLI (2026-06-12)

Cinco tests nuevos (13 totales en rp3_cli):

- **Gráficos acotados, documentado quién termina cómo:** 07-franjas termina SOLO (exit 0). 08-tetris-grafico es por turnos: sin stdin termina por **EOF de entrada con exit 1** y diagnóstico — sin pánico (no por límite de instrucciones: el programa se bloquea pidiendo jugada antes de agotar cualquier presupuesto razonable).
- **09/10 en headless → exit 124 limpio por `--timeout-s 1`:** la cola MMIO está siempre vacía y ambos duermen en su bucle de polling, así que el presupuesto de instrucciones se consume lentísimo — el límite ÚTIL para tiempo real es el de tiempo de pared, no `--max-instr` (el temporizador corta los sleeps vía stop_request). Sin pánico.
- **ebreak headless:** aviso «ebreak en 0x… — continuando» por stderr, exit 0, stdout EXACTO («antes-despues\n») — la pausa no contamina la salida del invitado.
- **`--timeout-s` vs sleep de 60 s:** corta en ~1 s (stop_request interrumpe el sleep en ≤10 ms), exit 124, sin esperar el sueño.
- **Rutas inválidas:** entrada inexistente y `-o` sin directorio → exit 1 con mensaje claro, sin pánico ni backtrace.

Detalle de la fase: el fuente del test de ebreak usaba `.asciz` (grafía GNU) y el ensamblador soporta `.asciiz` (grafía RARS, coherente con el resto del proyecto) — el propio mensaje didáctico del ensamblador lo aclaró. [Seguro]

Compuerta F3 ✓: 13/13 rp3_cli + 20 suites verdes, clippy 0.

## RP3 Fase 4 — Documentación y cierre del CLI (2026-06-12)

- README: sección «Uso por línea de comandos» con subcomandos, flags, reglas de canal, códigos de salida, dos ejemplos reales y el caso de uso de corrección automática por diff.
- **La compuerta final cazó la regresión que vigilaba [Seguro]:** con el `[[bin]]` nuevo, `cargo tauri build` falló («failed to find main binary») — con dos binarios Tauri exige `default-run`. Fix: `default-run = "emurv"` en `[package]`. Re-build verde (.app + .dmg). Exactamente el motivo por el que el plan exigía `cargo tauri build` en el cierre.
- Deuda menor registrada (anticipada por el plan): compilar `emurv-cli` arrastra las dependencias de Tauri del crate (son dependencias normales del paquete). Aceptado en esta versión; feature-gate o workspace con crate `emurv-core` separado quedan como PROPUESTAS FUERA DE ALCANCE.
- Decisiones ya registradas en F1-F3: parsing manual sin clap, stdout exclusivo del invitado, semántica headless de ebreak/breakpoint (aviso + continuar; flag `--pause-on-ebreak` = propuesta futura), código 124 estilo timeout(1), --timeout-s como límite útil para tiempo real.
- Verificación humana sugerida (el coordinador la corre): `echo "8\n34" | emurv-cli run ejemplos/02-suma-interactiva.asm` → "La suma es: 42"; smoke ya hecho en esta sesión con el binario release real.

**Compuerta F4 (cierre RP3) ✓:** `cargo test` 20 suites verdes (13/13 rp3_cli) · clippy 0 warnings · `cargo build --release --bin emurv-cli` ok · `cargo tauri build` ok (.app 21 MB + .dmg). 

### RP3 Fase 4 — addendum: el CLI se movió a un crate propio (2026-06-12)

La compuerta final destapó una cadena de problemas que obligó a una desviación estructural del plan, documentada acá con la evidencia completa:

1. `cargo tauri build` falló con el `[[bin]]` nuevo («failed to find main binary») → fix `default-run = "emurv"`. Pero el build siguiente empaquetó **ambos** binarios: la .app pasó de 21 MB a **34 MB**, violando RNF-04 (<30 MB, gate medido de v0.1). [Seguro — medido]
2. Intentos en orden, todos verificados empíricamente: (a) `beforeBundleCommand` que borra el artefacto antes de empaquetar → el bundler EXIGE el artefacto y corta con error (hook corre desde la raíz del repo, confirmado). (b) `required-features = ["cli"]` → evita compilarlo, pero el bundler lee los targets de la metadata de cargo sin mirar features: con el artefacto presente lo empaqueta (34 MB de nuevo) y sin él corta con error. **Conclusión [Seguro]: un `[[bin]]` adicional no puede convivir con el paquete Tauri sin terminar dentro de la .app.**
3. Salida elegida: **crate separado `emurv-cli/`** (raíz del repo) que depende de la lib existente por path (`emurv = { path = "../src-tauri" }`) — CERO cambios al crate Tauri (revertidos default-run y feature; un solo bin otra vez). NO es un workspace ni un refactor del núcleo: el plan prohibía "refactor a workspace/crates separados", pero la alternativa era violar RNF-04 (SPEC manda) o un hack semántico (CLI como example de cargo). El diseño acordado (subcomandos, puente, códigos de salida, semántica headless) quedó INTACTO; solo cambió dónde vive el binario. [Seguro — justificación: conflicto entre plan y SPEC se resuelve a favor de SPEC]
4. Detalle de build: el crate nuevo sin lockfile resolvió versiones transitivas más nuevas y pisó una incompatibilidad de compilación del crate `time` (E0119); se sembró `Cargo.lock` desde el de src-tauri (mismas versiones probadas). [Seguro]
5. **Comandos finales (cambian respecto del plan, avisar al coordinador):** compilar: `cd emurv-cli && cargo build --release`; tests del CLI: `cd emurv-cli && cargo test` (13 tests, los mismos); la suite de src-tauri vuelve a 18 suites y `cargo tauri build` queda limpio (.app 21 MB, solo el binario de la app). Smoke release verificado: `echo "8\n34" | emurv-cli run ejemplos/02-suma-interactiva.asm` → «La suma es: 42».

## RP4 Fase 1 — Workflow de CI validado estáticamente (2026-06-12)

Plan `docs/PLAN-RP4-ci.md`. Restricción rectora: el repo es SOLO LOCAL hasta el final del proyecto — este punto es autoría + validación estática + espejo local; el primer run real queda DIFERIDO a la creación del remote (checklist de activación como comentario al final del propio ci.yml).

- `.github/workflows/ci.yml` con los 4 jobs del diseño acordado: `test-macos` (obligatorio: frontend tsc+vite, tests+clippy -D warnings en AMBOS crates, build release del CLI), `bundle-macos` (obligatorio: `npx tauri build` + gate de peso), `build-linux` y `build-windows` (`continue-on-error`: su valor es el log de fallos como insumo del port, Cap. VI §6.2.1).
- **Gate de RNF-04 automatizado [Seguro]:** `scripts/check-bundle-size.sh` (du -sm ≤ 30 MB) compartido entre el job `bundle-macos` y el espejo local — un solo chequeo, no dos copias que diverjan. Este gate habría cazado automáticamente la regresión de RP-3 (34 MB).
- **Decisión: `npx tauri build` en vez de `cargo install tauri-cli` [Seguro]:** `@tauri-apps/cli` ya es devDependency del repo — el CI no paga la compilación del cli de tauri y el comando es idéntico en local (principio espejo).
- **actionlint instalado vía `brew install actionlint` (v1.7.12)** — método registrado según pedía el plan. Validación: actionlint limpio + el YAML parsea con PyYAML.
- Badge en README como comentario placeholder OWNER/REPO («activar al crear el remote») — no se inventó URL real.
- Triggers: push/PR a `main` y `feature/**`. Caché: `Swatinem/rust-cache@v2` por crate + npm (sin caché cada run compila Unicorn/CMake desde cero).

Compuerta F1 ✓: actionlint limpio, YAML parsea; la revisión cruzada comando-a-comando contra el script espejo se cierra en F2 (el script se escribe ahí con los MISMOS comandos).

## RP4 Fase 2 — Script espejo ci-local.sh verde (2026-06-12)

`scripts/ci-local.sh` (bash, `set -euo pipefail`, `--skip-bundle` opcional) replica en secuencia los jobs `test-macos` y `bundle-macos` con los MISMOS comandos del ci.yml (revisión cruzada comando a comando hecha: npm ci / tsc / vite build / cargo test y clippy -D warnings en ambos crates / build release del CLI / npx tauri build / check-bundle-size.sh). Compuerta F1 cerrada del todo con esta revisión.

Resumen de la corrida completa en esta máquina (exit 0):

```
== Frontend (npm ci + tsc + vite build)        ✓ (49 módulos, 400 ms)
== Tests y clippy (src-tauri)                  ✓ (18 suites, clippy limpio)
== Tests y clippy (emurv-cli)                  ✓ (13/13, clippy limpio)
== Build release del CLI                       ✓
== Bundle de la app (npx tauri build)          ✓ (.app + .dmg)
== Peso del bundle ≤ 30 MB (RNF-04)            ✓ 22 MB
== CI LOCAL: TODO VERDE
```

(El .app pasó de 21 a 22 MB por el redondeo de `du -sm` sobre el bundle con el frontend nuevo — sigue holgado contra el límite.) [Seguro — corrida real registrada]

## RP4 Fase 3 — Piernas experimentales y deps por plataforma (2026-06-12)

- **Lista de deps Linux verificada contra la doc oficial de Tauri v2** (tauri.app/start/prerequisites, consultada 2026-06-12): la lista del diseño coincidía con la oficial; el ci.yml agrega además `cmake` y `pkg-config`, que NO los pide Tauri sino la compilación C de Unicorn Engine — distinción documentada en el README (es el riesgo principal del port, Cap. VI §6.2.1). [Seguro — fuente citada]
- README §Instalación: tabla de dependencias por plataforma (macOS probado / Linux y Windows SIN PROBAR — piernas experimentales del CI) con fuente y fecha. Windows según doc oficial: MS C++ Build Tools + WebView2 Runtime (+ CMake por Unicorn; preinstalado en los runners de GitHub).
- Los jobs experimentales quedaron como en F1 (`continue-on-error: true`); su valor declarado es el LOG de fallos como insumo del port — no se promete que pasen.
- actionlint re-corrido tras los cambios: limpio.

Compuerta F3 ✓.

## RP4 Fase 4 — Docs de CI, checklist de activación y cierre (2026-06-12)

- README: sección «Integración continua» (qué corre cada job, qué es experimental, cómo correr el espejo local con y sin bundle).
- **Checklist de activación al crear el remote** (también como comentario al final de ci.yml): (a) reemplazar placeholder OWNER/REPO del badge; (b) push y verificar los 4 jobs en Actions (test-macos y bundle-macos deben quedar verdes; los experimentales pueden fallar y ese log se conserva); (c) registrar en BITACORA los fallos de Linux/Windows como insumo del port (Cap. VI §6.2.1); (d) opcional: proteger main exigiendo los dos jobs obligatorios.
- Decisiones consolidadas del punto: matriz macOS-obligatorio + Linux/Windows-experimental [Seguro]; caché Swatinem/rust-cache@v2 por crate + npm [Seguro — sin caché cada run recompila Unicorn]; gate de 30 MB automatizado y compartido [Seguro]; actionlint vía brew v1.7.12 [Seguro]; `npx tauri build` como comando único local/CI [Seguro]. Propuesta fuera de alcance registrada: cargo-audit/dependabot cuando exista el remote.

**Compuerta F4 (cierre RP4) ✓ [Seguro — corridas reales]:** `cargo test` + `clippy -D warnings` verdes en AMBOS crates (18 suites src-tauri + 13/13 emurv-cli, dentro del espejo) · `scripts/ci-local.sh` exit 0 con TODO VERDE incluido bundle 22 MB ≤ 30 · actionlint limpio.

## RP5 Fase 1 — Constantes .eqv/.equ y operadores %hi/%lo (2026-06-12)

Plan `docs/PLAN-RP5-multiarchivo.md` (punto de mayor riesgo del roadmap: el invariante es el source_map). Esta fase NO toca el flujo de archivos ni el source_map — solo el léxico y los operandos.

- **Constantes**: tabla en Pass1; `.eqv NOMBRE valor` (RARS) y `.equ NOMBRE, valor` (GNU), ambas grafías con o sin coma. **Sustitución al parsear operandos** (Sym→Imm temprano): por eso deben definirse ANTES de usarse — la decisión de tamaño de pasada 1 (li corto/largo, lw absoluto) necesita el valor ya. Encadenadas (una constante desde otra) ✓. Errores didácticos: redefinición, nombre de registro, choque constante↔etiqueta en AMBOS sentidos. Usables en todo operando inmediato, en `.word/.half/.byte` (con chequeo de rango) y como dirección absoluta en `la`/cargas/almacenamientos (expanden lui+op, mismo camino que etiquetas). [Seguro]
- **%hi/%lo**: token `%` nuevo (muere el rechazo didáctico del lexer); `%hi(x)`/`%lo(x)` como inmediato en `lui`, `addi`, `ori`, y `%lo(x)(base)` como desplazamiento de cargas/almacenamientos. Resolución en pasada 2 (referencias hacia adelante funcionan como etiquetas). Semántica = `hi20`/`lo12` EXISTENTES (las mismas funciones de la expansión de `la`): el par lui+addi reconstruye el valor exacto con ajuste de signo — test que lo demuestra byte a byte contra `la`, incluyendo el caso lo12 negativo (offset 2052 y 0x...FFF). [Seguro]
- **Decisión didáctica [Seguro]:** `%hi` con `auipc` se RECHAZA con mensaje explicativo (sería `%pcrel_hi`, semántica relativa al pc que no soportamos — aceptarlo en silencio codificaría direcciones erróneas).
- Gap cazado por los tests de la propia fase: `encode_load` no tenía brazo para el operando `%lo(x)(base)` (el de stores ya pasaba por `c.mem`); agregado.
- 9 tests nuevos en el módulo del ensamblador (46 lib en total).

Compuerta F1 ✓: 18 suites src-tauri + 13/13 emurv-cli, clippy 0 en ambos.

## RP5 Fase 2 — Preprocesador e .include con tabla de procedencia (2026-06-12)

La fase de mayor riesgo del punto de mayor riesgo: el source_map sobrevive por diseño, no por suerte.

- **`core/preprocess.rs` nuevo**: aplana `.include` ANTES de las dos pasadas, con `PpLine { text, file, line, main_line }`. **Regla de atribución main_line** (el corazón del invariante): línea del principal → su propia línea; línea traída por include (directo o anidado) → la línea de la directiva `.include` DEL PRINCIPAL que la introdujo. El SourceMap sigue hablando SOLO de líneas del principal → la UI actual (breakpoints, resaltado, un buffer) funciona sin cambios. [Seguro — diseño del plan aplicado tal cual]
- **Errores con procedencia REAL**: `AsmError.file: Option<String>` (None = principal; el Display antepone «archivo.s: » solo en Some — cero cambio de formato para el caso de siempre). Pass1/Pass2/fixups adjuntan el archivo vía cursor de procedencia en Pass1 (`cur_file`/`cur_main_line`) y campos nuevos de `PInstr`/`DataFixup` — sin enhebrar firmas por todo el parser.
- **`assemble_with_includes(source, base_dir, main_name)`** nueva; `assemble()` queda como wrapper con `base_dir: None` → CERO cambios en los ~20 archivos de tests existentes (la suite vieja verde lo prueba). Sin base_dir, `.include` da el error didáctico «necesita que el archivo esté guardado en disco».
- `.include "ruta"` relativo al archivo CONTENEDOR; pila anti-ciclos con la cadena completa en el error («a.s → b.s → a.s»); profundidad máx. 16; archivo faltante → error con la ruta intentada. `Program.origins/files` registran la procedencia real palabra a palabra (para la futura vista multi-archivo; la UI no lo consume aún).
- CLI: `assemble_with_includes` con el directorio del archivo de entrada (cambio puntual en emurv-cli).
- Tests (`rp5_include.rs`, 6): símbolos en ambas direcciones, anidado + en .data, ciclo con cadena, faltante + sin base_dir, error dentro del include con archivo:línea REALES, y **el INVARIANTE: breakpoint sobre la línea del `.include` frena en la PRIMERA instrucción traída por el include** (test a nivel Machine, verde a la primera).

Compuerta F2 ✓: 19 suites src-tauri (TEC/FB/RP1/RNF intactas = invariante vivo) + 13/13 CLI, clippy 0 ambos.

## RP5 Fase 3 — Macros RARS con atribución a la invocación (2026-06-12)

- `.macro NOMBRE (%p1, %p2)` … `.end_macro`/`.endm` (paréntesis y comas opcionales, sintaxis RARS) en el preprocesador: definición recolectada (las líneas del cuerpo NO van al ensamblador), sustitución textual de `%param` (parámetros más largos primero, para que %a no pise %abc), invocación = primer token de la línea que coincide con una macro conocida, **con etiquetas opcionales delante** (`et: macro arg` — la etiqueta se emite como línea propia y define la dirección de la primera instrucción expandida; salió del propio rnf02 extendido).
- **Atribución (invariante):** las líneas expandidas llevan `main_line` de la INVOCACIÓN (heredando la regla si la invocación vino de un include) y `file/line` REALES del cuerpo para errores. Test clave: 3 invocaciones de la misma macro → 3 grupos de direcciones en `line_to_addr` apuntando a las 3 líneas de invocación, y las direcciones intermedias vuelven a su línea. [Seguro]
- Macros en includes visibles tras el punto de inclusión (preprocesado secuencial); macro dentro de macro: invocar SÍ, definir NO (error); recursión cortada por **profundidad** de expansión (límite 64). **Corrección de diseño cazada por rnf02 [Seguro]:** la primera versión contaba invocaciones TOTALES — 230 invocaciones secuenciales legítimas reventaban el límite; el límite correcto es de ANIDAMIENTO (el plan decía "cortar recursión": N invocaciones planas no son recursión).
- Limitación documentada (también en el doc del módulo): una macro que define etiquetas e invocada 2+ veces da «etiqueta duplicada» (RARS renombra por expansión; queda como propuesta fuera de alcance).
- `rnf02_ensamblar_500_lineas_con_include_y_macro` (release): **520 líneas en 1.90 ms** (< 200 ms, gate del plan; la variante original: 1.34 ms). [Seguro — medido]

Compuerta F3 ✓: suites completas verdes en ambos crates (19 + CLI 13/13), clippy 0, rnf02 extendido verde.

## RP5 Fase 4 — UI, ejemplo multi-archivo, docs y cierre (2026-06-12)

- **UI:** el frontend ahora recuerda la ruta del buffer (`rutaArchivo`, seteada al Abrir y al Guardar, limpiada al cargar un ejemplo embebido) y la manda como parámetro OPCIONAL nuevo del comando `assemble` — firma retrocompatible (None = comportamiento de siempre; el error didáctico de «.include necesita archivo guardado» llega al panel de errores como cualquier AsmError). Los errores con `file` muestran «archivo.s: » antepuesto en la consola. `AsmError.file` agregado al tipo TS.
- **Ejemplo `ejemplos/11-multiarchivo/`** (main.s + constantes.s + utiles.s): usa TODO el repertorio RP5 — `.include`, `.eqv`, macro con parámetro, `%hi/%lo` — e imprime salida verificable (3 saludos + despedida). Diseño didáctico del ORDEN de inclusión documentado en los propios fuentes: constantes y macro (no emiten código) se incluyen ARRIBA; las rutinas con código, ABAJO del main para no robar el punto de entrada. Corre por CLI (`emurv-cli run ejemplos/11-multiarchivo/main.s`, verificado con el binario release) y por la app vía «Abrir». **Limitación registrada:** el menú de ejemplos embebe strings (`?raw` de Vite) sin carpeta base → el 11 NO va al menú; queda para la fase visual multi-archivo. Tests CLI nuevos: `run` con stdout exacto y `asm` → ELF32 válido (15/15 en emurv-cli).
- **Docs:** ENSAMBLADOR.md §4.4 nueva (includes/constantes/macros/%hi-%lo + diagrama de texto de la regla de atribución), Ayuda integrada (lista de directivas + párrafo RP5), README (tabla de directivas + ejemplo 11; «10 ejemplos»).
- Incidente menor de sesión sin consecuencias: la carpeta del ejemplo se creó primero en `src-tauri/ejemplos/` por un cwd heredado del shell; movida a `ejemplos/` antes de commitear.

**Compuerta F4 (cierre RP5) ✓ [Seguro — corridas reales]:** `scripts/ci-local.sh` completo exit 0 — frontend (tsc+vite), `cargo test` + `clippy -D warnings` en AMBOS crates (20 suites src-tauri con rp5_include 6 y rp5_macros 6; emurv-cli 15/15), build release del CLI, `npx tauri build` y **bundle dentro del límite RNF-04**. El invariante del source_map queda demostrado por: suite TEC/FB/RP1/RNF intacta + tests específicos (breakpoint vía .include, atribución de N invocaciones de macro).

## RP6 Fase 1 — Spike udbserver (PoC RSP) (2026-06-12)

Plan `docs/PLAN-RP6-gdb.md`. Feature `gdb` opcional en la lib (`udbserver = "0.3"`, `dep:` sin default); `emurv-cli` la activa; re-export `emurv_lib::udbserver` para que CLI y tests usen la misma versión. **El crate Tauri sin la feature no arrastra nada: `cargo tree` da CERO menciones de udbserver/gdbstub** y compila idéntico. `Machine::unicorn_for_debug()` (solo con feature, `impl Machine<'static>` propio) documentada como puerta exclusiva del stub.

**Spike verde (`emurv-cli/tests/spike_gdb.rs` — cliente RSP crudo por TCP, sin gdb externo), evidencia:**
`qSupported → PacketSize=1000;…;swbreak+;hwbreak+;qXfer:features:read+` · `? → T05` · `g → 264 chars` (33 regs RV32; respuesta con run-length encoding del RSP — el cliente del test lo descomprime) · `pc = 0x00400000` ✓ · `m` lee la primera palabra REAL de .text (0x00100293 = `addi t0,x0,1`) ✓ · `Z0` → OK · `c` → S05 con **pc exacto en el breakpoint (0x00400008)** ✓.

**Respuestas del spike [Seguro — código fuente de udbserver leído + spike corrido con los hooks de Machine instalados]:**
1. **Convivencia con hooks propios:** udbserver agrega un code hook GLOBAL vacío, un MEM_READ GLOBAL vacío y hooks por-breakpoint; NO toca el hook INTR (syscalls) ni remueve hooks ajenos. El spike corrió con INTR + MEM_INVALID + teclado MMIO instalados por `Machine::new` y todo respondió. Orden: los nuestros primero (constructor), udbserver después — sin pisadas.
2. **Modelo de ejecución:** CONFIRMADA la hipótesis del plan — `udbserver(uc, port, 0)` BLOQUEA sirviendo al cliente; al primer continue/step devuelve el control y el host corre `emu_start` LIBRE (count=0, una sola llamada); los hooks del stub retoman el control en cada breakpoint/step (`udbserver_resume` bloquea dentro del hook). El subcomando debug NO usa `run_slice`: es un modo de ejecución propio. Consecuencia: el code hook global del stub baja la velocidad a la clase ~100 M instr/s — irrelevante para depuración interactiva.
3. **Z0 sobre RV32:** SÍ, funciona (verificado con dirección exacta).
4. **Costo y licencias:** compilación incremental de udbserver+gdbstub en segundos; `unicorn-engine` UNIFICADO en 2.1.5 (sin duplicados en `cargo tree -d`: una sola compilación nativa). Licencias: udbserver **MIT**, gdbstub **MIT OR Apache-2.0** — compatibles con el stack (Unicorn GPLv2 sigue siendo el punto a consultar de siempre, sin cambios).

**Decisión [Seguro]:** udbserver viable; plan B (mini-RSP propio sobre gdbstub) DESCARTADO — no hay incompatibilidad que lo justifique.

## RP6 Fase 2 — Subcomando debug con protocolo RSP verificado (2026-06-12)

- **`emurv-cli debug <archivo.s> [--port N]`** (3333 por defecto): ensambla (con includes RP-5), arma la máquina, instala udbserver y BLOQUEA esperando al cliente; tras el primer continue/step la emulación corre libre con `machine.run(0)` (los hooks del stub retoman en cada bp/step — modelo del spike). stdout sigue siendo EXCLUSIVO del invitado; «esperando conexión gdb en 127.0.0.1:N…» y el resto del stub van por stderr.
- **Puerto ocupado:** udbserver con start_addr=0 PANIQUEA si el bind falla (`.expect` interno — leído en su fuente): pre-chequeo propio del puerto con error claro y exit 1 (ventana de carrera mínima entre soltar y re-bindear, documentada en código). Cliente que desconecta: el programa sigue hasta terminar y el proceso sale con mensaje — sin colgarse. Ctrl-C: terminación por señal estándar, sin pánico.
- **Refactor:** el puente de consola de `run` se extrajo a `puente_consola(...)` compartido con `debug` (un solo puente, no dos copias).
- **Tests RSP como proceso real** (`tests/rp6_debug.rs`, 4 + cliente común `tests/rsp_common/`): (a) dos `s` → t0=5, t1=7, pc=+8 (semántica conocida del fuente); (b) `m` lee `.word 0xCAFEBABE` de .data (little-endian verificado); (c) `Z0`+`c` clava el pc EXACTO en la dirección de la etiqueta `meta` calculada con `Program.symbols` (t0 ejecutado, t2 no); (d) `z0`+`c` → corre a fin, proceso exit 0; (e) syscalls bajo el depurador: «hola desde gdb\n» EXACTO en stdout, stub en stderr; (f) puerto ocupado → exit 1 sin pánico.

Compuerta F2 ✓: emurv-cli 20 tests verdes + src-tauri 20 suites, clippy 0 en ambos.

## RP6 Fase 3 — Convivencia, símbolos en el ELF y sesión gdb real (2026-06-12)

### Dos hallazgos del camino, ambos resueltos vendoreando udbserver

1. **udbserver fuerza `dynamic_linkage` en unicorn-engine** (visto en su Cargo.toml): la unificación de features de cargo linkeaba Unicorn DINÁMICO en todo el grafo del CLI y el binario release moría con «Library not loaded: @rpath/libunicorn.2.dylib» (sin LC_RPATH). La app Tauri NO se ve afectada (grafo sin la feature gdb → estático). **Fix: copia vendoreada 1:1 en `vendor/udbserver/` (MIT lo permite) con la feature `dynamic_linkage` removida** — la API de udbserver no la necesita; binario standalone verificado. [Seguro — reproducido y verificado]
2. **udbserver imprime «Disconnect!» y otros mensajes por STDOUT**: contaminaba el canal EXCLUSIVO del programa invitado (regla crítica RP-3). En el vendor: 4 `println!` → `eprintln!`. [Seguro]

El vendor queda documentado en su propio Cargo.toml; total de cambios sobre upstream: 1 línea de features + 4 println. Si upstream lo arregla, se vuelve a crates.io.

### Símbolos en el ELF exportado (F3.3 — resultó barato)

`elf_export.rs` ahora escribe **`.symtab` + `.strtab`** con el Writer de bajo nivel de `object`: cada etiqueta de `Program.symbols` como símbolo GLOBAL (FUNC si cae en .text, OBJECT si en .data). `file` reporta «not stripped»; `objdump -t` muestra main/factorial/caso_base/msj/nl. Detalle ELF cuidado: `sh_info` del symtab = índice del primer símbolo no-local (=1: solo el nulo es local). Beneficio directo: `break factorial` por NOMBRE en gdb real. [Seguro]

### Convivencia bajo el stub (tests 6/6 en rp6_debug.rs)

- Teclado MMIO: `m ffff0000,4` del host devuelve 0 SIN consumir (página con respaldo — la decisión de TEC2 paga acá) y el guest que hace `lw` ve 0 con cola vacía; el hook MEM_READ convive con los del stub.
- Multi-archivo RP-5: breakpoint en `saludar` (etiqueta definida DENTRO de un include) clava el pc 3 veces (una por llamada) y la salida completa llega a stdout.

### Sesión gdb REAL (riscv64-elf-gdb 17.2, instalado vía brew) — transcript

```
$ emurv-cli debug ejemplos/05-pila-funciones.asm --port 3413   # terminal A
$ riscv64-elf-gdb /tmp/pila_sym.elf                             # terminal B (batch equivalente)
The target architecture is set to "riscv:rv32".
0x00400000 in main ()
pc             0x400000	0x400000 <main>
Breakpoint 1 at 0x40003c          ← «break factorial» por NOMBRE (símbolos del ELF;
                                     gdb saltea el prólogo: factorial+4)
Breakpoint 1, 0x0040003c in factorial ()
pc             0x40003c	0x40003c <factorial+4>
a0             0x5	5             ← el argumento real del factorial
ra             0x400018	0x400018 <main+24>
0x00400040 in factorial ()        ← stepi
pc             0x400040	0x400040 <factorial+8>
0x10010000:	0x3d202135	0x000a0020 ← x/2xw .data: "5! =" en bytes LE
factorial + 8 in section .text    ← info symbol $pc
Remote connection closed
--- stdout del proceso debug (exclusivo del invitado): 5! = 120
--- stderr: esperando conexión gdb… / Debugger connected / programa terminado bajo el depurador
```

Nota: lldb (intento previo) conecta y frena (SIGTRAP) pero su soporte riscv32 por gdb-remote no inicializa el contexto de registros en batch — bloqueo documentado; el cliente canónico es gdb y funciona completo.

Compuerta F3 ✓: 20 suites src-tauri + 20 tests CLI, clippy 0 ambos, transcript real pegado.

## RP6 Fase 4 — Documentación y cierre del stub GDB (2026-06-12)

- `docs/DEPURACION-GDB.md`: arranque, conexión con gdb (canónico, probado) y VS Code (`launch.json` con `miDebuggerServerAddress`), nota sobre lldb (conecta pero no inicializa registros riscv32 — usar gdb), transcript de la sesión real, tabla del protocolo verificado por tests, modelo de ejecución, convivencia y limitaciones (sin DWARF — direcciones y SÍMBOLOS; un cliente; vendor documentado).
- README: sección «Depuración remota (GDB)» enlazando a la guía.
- Decisiones consolidadas del punto: udbserver 0.3 viable y elegido (plan B descartado con evidencia del spike) [Seguro]; feature `gdb` sin default — la app Tauri fuera del grafo [Seguro]; vendor con 2 cambios mínimos por linkeo dinámico forzado y stdout contaminado [Seguro]; símbolos .symtab en el ELF exportado — `break etiqueta` real [Seguro]; licencias MIT / MIT+Apache compatibles [Seguro].
- Propuesta fuera de alcance registrada: integración del depurador en la UI Tauri (fase visual) y `--pause-on-ebreak` siguen pendientes de otros puntos.

**Compuerta F4 (cierre RP6) ✓ [Seguro — corridas reales]:** `cargo test` + clippy 0 en AMBOS crates (20 suites + 21 tests CLI) · `scripts/ci-local.sh --skip-bundle` exit 0 · `cargo tauri build` ok · **bundle 22 MB ≤ 30 con SOLO el binario de la app adentro** (la feature gdb no entró: verificado en Contents/MacOS) · tsc limpio.

## RP7 Fase 1 — Watchpoints con hooks condicionales (2026-06-12)

Plan `docs/PLAN-RP7-depuracion.md`. `Watchpoint { addr, len 1|2|4, kind Read|Write|Both }`, `Machine::set_watchpoints` reemplazar-todo, **hooks MEM_READ/MEM_WRITE globales instalados SOLO si hay watchpoints y removidos al vaciar** (el patrón-LEY del bp_hook, con flush de TB en ambas transiciones). Regiones válidas: .data, pila, framebuffer, paleta; **página del teclado MMIO vetada** con error didáctico (vigilarla cambiaría la semántica leer-consume); fuera de toda región o len inválido → error claro. `StopReason::Watchpoint { addr, access, value, pc }` (campo `access` y no `kind`: choca con el tag serde del enum).

**Hallazgo propio de la fase [Seguro — reproducido por el test de solapamiento]:** el `emu_stop` dentro del hook de memoria ABORTA la instrucción que accedía; al reanudar, esa instrucción se re-ejecuta y el mismo watchpoint dispararía para siempre. Solución espejo de `suppress_bp`: `suppress_watch` (clave: dirección del acceso + si fue escritura) se arma al REPORTAR la parada y se consume UNA vez al reanudar — la instrucción completa su acceso y la ejecución sigue. Se limpia en `load()` y al reemplazar la lista. En un bucle que escribe N veces, cada escritura dispara una vez (verificado por diseño del test Both: write → continuación → read).

Tests (rp7_watchpoints.rs, 6): write con valor y pc exactos; read con valor leído de memoria (el hook MEM_READ no trae el dato); Both + solapamiento parcial (sw de 4 pisa watch de 1 byte interior); **lectura del HOST no dispara** (garantía TEC2 fijada por test); vetos y validaciones; hooks instalados/retirados según lista (introspección `watch_hooks_instalados`).

**Medición [Seguro]:** RNF-03 con watchpoint activo y bucle CON carga de memoria por iteración (el caso caro): **71.4 M instr/s** (12 M instr en 0.168 s) — gate ≥10 M superado ~7×. Sin watchpoints: cero hooks (el test lo fija), corrida libre intacta.

Compuerta F1 ✓: 21 suites verdes, clippy 0.

## RP7 Fase 2 — Breakpoints condicionales (2026-06-12)

- **`core/cond.rs`**: gramática mínima `REG OP VALOR` (OP ∈ == != < <= > >=, comparación SIGNED i32; VALOR decimal con signo, hex 0x… —0xFFFFFFFF es -1, coherente—, o segundo registro). Parseo AL DEFINIR con errores didácticos («"t9" no es un registro; valen los nombres ABI…», «"=" solo no compara: usá "=="», «sobra texto al final», `<>` rechazado como operador). El hook evalúa un `CondAst` Copy ya validado: costo por instrucción mínimo.
- **`set_breakpoints` genérico** (`impl IntoIterator<Item: Into<BreakpointSpec>>` + `From<u32>`): los ~10 usos existentes compilan sin cambios (HashSet<u32> y arrays de u32 entran directo); `MachData.breakpoints` pasa de HashSet a `HashMap<u32, Option<CondAst>>`.
- **Regla de oro respetada (RP-1 F4):** condición FALSA = el programa pasa de largo — NO es parada, no setea `last_bp`, no consume supresión. Test dedicado: corte por count en medio del bucle (Paused) → la continuación frena EXACTO cuando t0==2, sin supresión espuria. Suite completa intacta (los tests de RP-1 vigilan y pasan).
- Tests (rp7_condicionales.rs, 5): **el caso estrella «parar cuando t0 == 5» en un bucle** (frena con t0==5 exacto; reanudar termina); registro contra registro; hex signed; 6 errores de parseo; convivencia condicional+watchpoint en la misma corrida con orden de paradas exacto (w1,w2,w3,b3,w4 — la condición se evalúa al ENTRAR a la vuelta, antes del addi: trazado documentado en el test).

Compuerta F2 ✓: 22 suites verdes, clippy 0.

## RP7 Fase 3 — Edición en vivo de registros y memoria (2026-06-12)

- `Machine::write_reg(n, val)` (x0 → «es cero por diseño del ISA»; x>31 → inválido), `write_pc(val)` (**resetea `last_bp`**: el próximo Run desde un pc elegido a mano no debe suprimir ningún breakpoint — coherencia RP-1 F4, con test que lo fija), `write_mem(addr, 1..4 bytes)` (.data/pila/framebuffer/paleta; `.text` → «el programa es inmutable mientras corre: editá el fuente y re-ensamblá»; página del teclado → rechazo; sin flush de TB porque no se toca código). El guard de "solo pausado" vive en commands.rs (F4), que es quien conoce el estado de la sesión.
- Tests (rp7_edicion.rs, 4): **el criterio del ROADMAP literal** — editar t0 pausado y la SIGUIENTE instrucción usa el valor (addi lee 99); editar pc salta ahí y un bp en el destino FRENA (sin reset de last_bp la supresión lo dejaría pasar — el test lo demuestra); los 6 rechazos didácticos; write_mem word+half+byte compuesto (0x7FAD_BEEF) visible por host Y por el programa, y en pila.

Compuerta F3 ✓: 23 suites verdes, clippy 0.

## RP7 Fase 4 — Comandos, API TS y panel de depuración (2026-06-12)

- **Comandos Tauri**: `set_breakpoints` extendido a `[{addr, cond}]` — el parseo de la condición ocurre en el hilo de la UI y el error didáctico llega al panel SIN tocar la máquina; `set_watchpoints` (validación de regiones en la máquina, error por canal de respuesta); `write_reg` (ABI/xN/«pc») y `write_mem` con **guard de pausa**: `Session.corriendo` (AtomicBool que el brazo Run del hilo prende/apaga) → corriendo = «pausá la ejecución para editar». El CLI ganó un brazo defensivo para la variante `Watchpoint` (no instala watchpoints; el match era exhaustivo y la variante nueva lo rompió — cazado por la compuerta).
- **UI** (panel `Depuracion` nuevo + ediciones inline): lista de breakpoints con condición editable (las condiciones viven en un Map por línea; el texto se conserva si el backend rechaza, para corregirlo); watchpoints con alta/baja y **rollback al último juego aceptado** si el backend rechaza (semántica reemplazar-todo: una entrada inválida envenenaría los envíos siguientes); ambos se RE-APLICAN tras re-ensamblar (la sesión nueva nace limpia). Doble click en registros (incl. pc; x0 también — el rechazo del backend lo explica) y en palabras del hexdump → input inline (decimal o 0x…, Enter/Esc/blur); el refresco a ~10 Hz no pisa una celda en edición. Al frenar por watchpoint: estado «Watchpoint: el programa escribió 0x… en 0x… (línea N)» + el panel de memoria navega a la dirección (irA en vez de refrescar: eran async en paralelo y se pisaban). 13+ claves i18n nuevas.
- **Pendiente de verificación visual humana (6 flujos, NO declarados cumplidos):** condición que para solo al cumplirse + error visible; watchpoints alta/baja/rechazo/disparo con navegación; edición inline de registros (Enter/Esc/rechazos/resaltado); edición de memoria; persistencia tras re-ensamblar; colapso del panel y layout con 4 paneles.

Compuerta F4 ✓: tsc limpio, vite ok (50 módulos), 23 suites src-tauri + CLI verdes, clippy 0 ambos.

## RP7 Fase 5 — Documentación y cierre (2026-06-12)

- `docs/DEPURACION.md`: watchpoints (regiones, veto MMIO con porqué, cifra de rendimiento), gramática exacta de condiciones con ejemplos, edición en vivo con los rechazos didácticos, límites (TEC6 sleep, re-aplicación tras re-ensamblar, qué queda para gdb). Ayuda integrada: párrafo nuevo en Breakpoints. README enlaza ambas guías de depuración.
- Decisiones consolidadas: hooks instalar-solo-si-hace-falta como LEY cumplida (watch_hooks espejo de bp_hook, con introspección de test); supresión de re-disparo de watchpoint (hallazgo propio F1, espejo de suppress_bp); condición FALSA ≠ parada (last_bp intacto, RP-1 F4 vigente); guard de pausa en Session.corriendo; parseo de condiciones en el hilo UI. Mediciones: **71.4 M instr/s con watchpoint activo y carga por iteración** (gate ≥10 M ~7×); corrida libre sin watchpoints: cero hooks (test).

**Compuerta F5 (cierre RP7) ✓ [Seguro — corridas reales]:** `cargo test` + clippy 0 en AMBOS crates (23 suites + CLI) · tsc limpio · `scripts/ci-local.sh --skip-bundle` exit 0 · `cargo tauri build` ok · bundle dentro del límite RNF-04.

## RP8 Fase 1 — Spike: CSRs y traps en Unicorn 2.1.5 (2026-06-12)

Plan `docs/PLAN-RP8-csrs-traps.md`. `tests/spike_csr.rs` (12 PoC, encodings a mano verificados contra la espec Zicsr, patrón spike_mmio). **Tabla causa→comportamiento [Seguro — todo reproducible]:**

| Caso | Sin hook (mtvec seteado por el guest) | Con hook |
|---|---|---|
| csrrw/csrrs/csrrwi | **NATIVOS** ✓ (P1/P1b: el host ve mscratch=0x2A; csrr lee) | — |
| mret | **NATIVO** ✓ (P2: pc←mepc y sigue) | — |
| IDs host | — | P2b: guest escribe con csrw y el host lee EXACTO por id crudo (mepc=128, mtvec=121 confirmados) |
| ecall | **NO vectoriza**: Err(EXCEPTION), Unicorn NO escribe mcause/mepc (P3a) | hook INTR recibe (intno=8); **REINYECCIÓN funciona end-to-end** (P3b2): el hook escribe mepc/mcause y pc←mtvec SIN emu_stop → el handler corre (leyó mcause), mret vuelve a mepc y el programa sigue (t1=7, res Ok) |
| instrucción ilegal | NO vectoriza: Err(EXCEPTION) (P3c) | llega al hook INTR con causa 2 (sabido de producción) → misma reinyección aplicable |
| acceso inválido | Err(READ_UNMAPPED) — nivel host, mtvec irrelevante (P3d) | **P3f: la reinyección desde el hook MEM_INVALID NO funciona** — devolver true reintenta el ACCESO original ignorando el pc escrito (Err(MAP), el handler jamás corrió). |
| ebreak | Err(INSN_INVALID) — **mtvec NO lo cambia** (P3e): RP-1 queda intacto por comportamiento de Unicorn, no solo por política |

**Advertencia medida (P3b):** el pc leído DENTRO del hook INTR crudo puede venir basura (mepc=pc-4 saltó a 0x459xxxx); en producción `handle_trap` ya deriva el pc confiablemente (toda la suite de syscalls lo prueba) — la reinyección debe usar ESA fuente, no una lectura ingenua.

**Decisión del mecanismo [Seguro — fundada en la tabla]:**
1. **ecall desconocido e instrucción ilegal**: reinyección DESDE el hook INTR (escribir mepc/mcause/mtval + pc←mtvec, sin emu_stop) — P3b2 lo valida de punta a punta. mcause se escribe con los valores de la ESPEC (11 ecall M-mode, 2 ilegal), no con el intno interno de Unicorn (8).
2. **Accesos inválidos**: delegación en `interpret()` (rescate post-uc_error, como el de ebreak de RP-1): mcause 5/7 + mtval=dirección + mepc=pc culpable + pc←mtvec y el bucle de tramos reanuda — P3f probó que el camino del hook NO sirve.
3. **Doble falta**: protocolo fiel a la espec con **mstatus.MIE** — load() la enciende; cada delegación la apaga (y arma MPIE); `mret` nativo la restaura (autolimpiante, sin estado fantasma del host). Delegar con MIE==0 ⇒ «excepción dentro del manejador de excepciones» → RuntimeError didáctico, sin loop.
4. ebreak: jamás se delega (política + comportamiento nativo coinciden). mtvec==0 ⇒ todo como hoy.

Compuerta F1 ✓: spike 12/12 + suite completa intacta (24 suites), clippy 0.

## RP8 Fase 2 — Zicsr y mret en el ensamblador (2026-06-12)

- Instrucciones: csrrw/csrrs/csrrc + variantes inmediatas (uimm5 0-31 con error didáctico) + `mret`; pseudos csrr/csrw/csrs/csrc. Encoding `enc_csr` (formato I sobre SYSTEM 0x73, f3 1-3/5-7). Tabla de nombres: mstatus, mie, mtvec, mscratch, mepc, mcause, mtval, mip (`csr_number` pública) + número crudo 0x000-0xFFF para el resto (solo los 8 con nombre están garantizados por Unicorn — irá en docs). CSR desconocido → error que SUGIERE los soportados.
- Tests: encodings de referencia calculados a mano contra la espec privilegiada y CONSISTENTES con el spike F1 (que los ejecutó nativos — la misma palabra 0x34029073 que el spike validó); 4 errores didácticos; **round-trip con Machine**: csrw+csrr+csrrwi dan la vuelta completa (0x1234 y 9 en registros) y `mret` ensamblado salta a mepc salteando la instrucción intermedia.

Compuerta F2 ✓: 25 suites verdes (la nueva rp8_zicsr incluida), clippy 0.

## RP8 Fase 3 — Delegación didáctica de traps (2026-06-12)

Implementación del mecanismo decidido en F1, en dos sitios según la tabla del spike:

- **`delegar_trap` (syscalls.rs)** — reinyección desde el hook INTR para **ecall desconocido** (mcause=11 de la ESPEC, no el intno 8 de Unicorn; mepc = dirección del ecall, FIEL a la espec: el handler ajusta mepc+4 antes de mret, como un kernel real — decisión didáctica registrada) e **instrucción ilegal** (mcause=2, mtval=palabra culpable). Escribe mepc/mcause/mtval, protocolo mstatus (MPIE←MIE, MIE←0), pc←mtvec, SIN frenar.
- **`interpret()` (machine.rs)** — **accesos a memoria inválidos** (READ/WRITE × UNMAPPED/PROT) con mtvec≠0: mcause 5/7, mtval=dirección (de `bad_mem`), mepc=pc, y devuelve `Paused`: el bucle de tramos (GUI, CLI run y los tests) reanuda ejecutando el handler — el spike P3f probó que el camino del hook MEM_INVALID no sirve.
- **Doble falta**: si al delegar mstatus.MIE ya está en 0 (estamos dentro de un handler; `load()` la inicializa en 1 y `mret` nativo la restaura — autolimpiante, sin estado fantasma del host) → RuntimeError didáctico «excepción dentro del manejador de excepciones (doble falta)…». `load()` además limpia mtvec/mepc/mcause/mtval entre sesiones/reinicios.
- **Política dura intacta**: mtvec==0 ⇒ caminos de siempre (regresión fijada por test con el mensaje literal); ecall conocido JAMÁS delegado (test: imprime normal y el handler no corre); ebreak JAMÁS delegado (test: EbreakPause con mtvec configurado — RP-1 intacto). Evento de consola «trap delegado» DESCARTADO: contaminaría el stdout exclusivo del CLI (regla RP-3 > nice-to-have; el panel de CSRs muestra mcause).

Tests (rp8_traps.rs, 7): el criterio del ROADMAP literal (ecall 777 → handler lee mcause=11/mepc exacto, ajusta +4, mret, el programa sigue y termina); conocido no delegado; ilegal delegada (mcause=2, vía determinista: caída a los ceros de .text) + regresión sin mtvec; acceso inválido (mcause=5, mtval=0x20000000); doble falta corta sin colgarse; ebreak pausa; convivencia RP-7 (breakpoint DENTRO del handler + watchpoint sobre dato que el handler escribe, orden exacto).

Compuerta F3 ✓: 26 suites verdes, clippy 0.

## RP8 Fase 4 — Panel CSRs, ejemplo trap handler y docs (2026-06-12)

- **Backend del panel**: `Machine::read_csrs()/write_csr()` (orden fijo mstatus…mip) + comandos `get_csrs`/`write_csr` (escritura con guard de pausa, mismo de RP-7).
- **UI**: sección plegable «CSRs (avanzado)» al final del panel de registros (`<details>`, CERRADA por defecto), 8 filas hex+dec, edición inline reusando el mecanismo RP-7 generalizado a callbacks. **Cerrada = cero invokes** durante el refresco a ~10 Hz (el costo solo existe si el estudiante la abre). Flujos manuales pendientes anotados (apertura sin sesión, edición pausado/corriendo, no-pisado de input activo, estética).
- **Ejemplo `12-trap-handler.asm`** (el germen de la lección 10 de RP-9): instala mtvec, dispara ecall 777, el manejador lee mcause/mepc, imprime, ajusta mepc+4 (comentario didáctico: «probalo comentando el ajuste») y mret. Sin `.include` → SÍ entra al menú de ejemplos de la app (11 ejemplos ahora). **Verificado por CLI con salida exacta** (test `run_ejemplo_trap_handler`) — el criterio del ROADMAP, literal.
- **Docs**: `docs/TRAPS.md` (tabla de CSRs, política de delegación completa con su porqué, mepc fiel a la espec y el ajuste +4, doble falta, límites — sin interrupciones asíncronas, M-mode plano); ENSAMBLADOR §4.1 ampliada (Zicsr); Ayuda con párrafo de traps; README con sección y enlace.

**Compuerta F4 (cierre RP8) ✓ [Seguro — corridas reales]:** `cargo test` + clippy 0 en AMBOS crates (26 suites src-tauri + CLI con el test nuevo) · tsc limpio · `ci-local.sh --skip-bundle` exit 0 · `cargo tauri build` ok · **bundle 22 MB ≤ 30**.

## 2026-06-12 — RP9-F1: estructura de lecciones y lecciones 1-3

[Seguro] Estructura creada: `lecciones/NN-tema/leccion.s` + `lecciones/README.md`
(índice esqueleto; el mapa completo llega en F3). Los ejemplos existentes NO se
movieron ni renombraron (la UI los embebe).

[Seguro] Lecciones escritas y verificadas por CLI con salida exacta:
- 01-registros: li/mv/la, nombres ABI, x0, servicios 1/11/4/10.
  Salida: `40\nregistros listos\n`.
- 02-aritmetica: add/sub/mul/div/rem, complemento a 2 DEMOSTRADO
  (0x7FFFFFFF+1 → -2147483648 ante los ojos del lector), and/or/xor sobre
  0xF0/0x3C, srai vs srli con -8. Salida: `3 resto 2\n-2147483648\n48 252 204\n-4\n`.
- 03-memoria: .word/.byte/.asciiz, lw/sw/lbu/sb, endianness DEMOSTRADO
  (leer los bytes 0 y 3 de 0x11223344 → 68 y 17), navegación del panel
  «Memoria» con el botón «.data» (0x10010000). Salida: `68 17\n42\nhola\n`.

[Seguro] Tests: `emurv-cli/tests/lecciones.rs` — un test por lección, stdout
EXACTO por assert_eq, guard de timeout (60 s debug / 20 s release) con kill.
3/3 verdes.

[Seguro] Tropiezo y arreglo: escribí `sra/srl` con inmediato; el ensamblador
(correctamente) exige 3 registros para las formas R. Cambiado a `srai/srli` y
la lección ahora EXPLICA la diferencia de mnemónicos (la «i» final) — el error
se volvió contenido didáctico.

[Probable] Autorrevisión didáctica («¿un principiante absoluto puede seguirla
solo?»): cada lección abre con OBJETIVOS y QUÉ PANEL mirar usando los nombres
exactos de la UI («Registros», «Memoria», «Consola», «Ensamblar», «Paso»);
cada bloque tiene el comentario ANTES del código y predice lo que el panel va
a mostrar (el lector verifica, no adivina); los conceptos con trampa
(complemento a 2, endianness, srl vs sra) se DEMUESTRAN con un experimento de
un Paso, no se enuncian; los 3 ejercicios van de modificación trivial a
desafío con pista. Riesgo restante: el primo no sabe qué es hexadecimal — la
lección 2 lo amortigua («cada dígito son 4 bits: F = 1111»), pero la
validación real es la prueba con el tester humano (pendiente, checklist).

Compuerta F1: 3/3 lecciones con salida exacta por CLI ✓ · suite emurv-cli +
src-tauri verdes ✓ · clippy ambos crates sin avisos ✓.

## 2026-06-12 — RP9-F2: lecciones 4-7

[Seguro] Cuatro lecciones nuevas, mismas plantilla y voz que las 1-3:
- 04-condicionales: beq/bne, if/else con saltos (el «if» no existe: se salta
  por encima), máximo de dos números, trampa blt vs bltu con -1 (conecta con
  el complemento a 2 de la 02). Observación didáctica: el pc en «Registros»
  brinca cuando el salto se toma.
- 05-bucles: suma 1..10 (=55), tablita del 3 con bucle anidado SIN mul (suma
  repetida: el anidamiento es genuino). REQUISITO F2 cumplido: bloque dedicado
  «frenar SOLO en la vuelta 5» que explica EN TEXTO el breakpoint condicional
  `t0 == 5` con el flujo exacto del panel «Depuración» (verificado contra
  docs/DEPURACION.md y es.json); el contador del programa ES t0.
- 06-funciones: jal/ret, ra, a0-a7, función hoja (duplicar) y no-hoja
  (cuadruple) con prólogo/epílogo; comentario explica QUÉ se rompe sin
  guardar ra (el 2.º jal lo pisa → bucle infinito) y el ejercicio 3 lo hace
  observable. Mirar la pila en «Memoria» botón «pila (sp)»: ra guardado en
  0x7FFFFFEC. Referencia al ejemplo «Pila y funciones» para el factorial.
- 07-syscalls: tabla SOLO de servicios existentes (1/4/5/10/11/30/32 — nada
  de 8/9/93, regla del plan), ciclo leer-validar-procesar con reintento
  (stdin de prueba 25→7), y 30/32 en modo seguro: dormir 100 ms y comprobar
  con bge que pasaron ≥100, imprimiendo cadena FIJA (jamás la hora cruda —
  el determinismo del test depende de eso).

[Seguro] Ejecución: las 4 deterministas (doble corrida idéntica), exit 0.
Tests exactos agregados a emurv-cli/tests/lecciones.rs: 7/7 verdes.
Retoque mío post-agente: tilde en «máximo» (UTF-8 en .asciiz ya probado).

[Probable] Autorrevisión didáctica F2: cada lección abre la puerta con lo ya
visto (04 cita a 02; 05 cita a 04; 07 cita a 02 y 04) — el orden del
currículo se sostiene solo. Riesgo: la 06 es el primer salto conceptual
fuerte (pila); se amortigua con la predicción byte a byte de 0x7FFFFFEC.

Compuerta F2: 4/4 con salida exacta por CLI ✓ · lección 5 demuestra el
breakpoint condicional en texto ✓ · suite emurv-cli (7 lecciones) +
src-tauri (26 ok / 0 fail) ✓ · clippy sin avisos ✓.

## 2026-06-12 — RP9-F3: lecciones 8-9 e índice completo

[Seguro] Lección 08 (multiarchivo): proyecto de 3 archivos en
lecciones/08-multiarchivo/ (main.s + constantes.s + utiles.s) — cuenta de
entradas de cine: 7 .eqv con nombre, macro «escribir %etiqueta», función
entero_nl llamada con jal; el texto enseña macro-se-copia vs
función-se-salta con el observable real (la línea de la macro queda
resaltada 3 Pasos; al entrar a la función el resaltado salta a la línea del
.include — regla main_line del source_map de RP-5 convertida en contenido).
Hallazgo del agente verificado: la ejecución arranca en la PRIMERA
instrucción de .text, así que main.s usa un trampolín `j main` ANTES del
.include de utiles.s — y lo explica didácticamente, contrastando con
ejemplos/11-multiarchivo (que pone el include al final). Corre por CLI:
salida exacta de 4 líneas, exit 0, determinista.

[Seguro] Lección 09 (MMIO y tiempo real): raya roja de 8 píxeles en (10,5)
del framebuffer 0x90000000 (1 byte/píxel, base + y*320 + x, verificado
contra ejemplos/07-franjas.asm y docs/MEMORIA-GRAFICA.md), aviso al control
de video 0x90030000, y encuesta del teclado 0xFFFF0000/0xFFFF0004
(CONTROL bit0 / DATA leer-consume, contra docs/TECLADO.md) con LÍMITE DURO
de 30 vueltas × 50 ms (servicio 32) ≈ 1,5 s: TERMINA SOLA sin teclas con
salida fija «…teclas atendidas: 0» — el requisito headless del plan. Con la
app abierta cada tecla deja una marca de color (sin tocar el stdout
determinista). Cierra con «concurrencia aparente» (mono-hart, sin prometer
hilos — regla del plan). Duración medida: 1,5 s por corrida.

[Seguro] Tests: lecciones.rs ahora 9/9 (la 09 incluida: la CI vigila para
siempre que el poll termine solo). Suite emurv-cli completa verde
(16+9+6+1), clippy 0 avisos.

[Seguro] lecciones/README.md completo: cómo trabajar una lección, mapa de
aprendizaje (BASE→CONTROL→ESTRUCTURA→MUNDO REAL→AVANZADA, con los atajos
válidos), tabla qué-aprendés-dónde con prerrequisitos por lección. La fila
de la 10 dice «llega con la fase 4».

[PENDIENTE HUMANO] Verificación manual de la lección 09 con la app: abrirla,
«Ensamblar» + «Ejecutar», click en la pantalla («Teclado capturado») y
teclear durante la encuesta → debe aparecer una marca de color por tecla en
la fila 20 y el contador final > 0. Headless quedó probado por test; la
reacción a teclas reales requiere la app (mismo patrón que los 6 flujos
RP7-F4).

Compuerta F3: lección 8 corre por CLI ✓ · lección 9 determinista headless ✓
(verificación con app: pendiente humano, anotada) · README completo ✓ ·
suite + clippy ✓.

## 2026-06-12 — RP9-F4: lección 10, integración y cierre de RP-9

[Seguro] Lección 10 (traps): escrita sobre el material de RP-8 con su propio
programa (no copia del ejemplo 12): main instala mtvec, dispara el servicio
777, el «mini-kernel» imprime mcause (11), atiende, ajusta mepc+4 y mret
vuelve. El texto guía la observación del panel «CSRs (avanzado)» (plegado:
click en el título) en el Paso exacto del trap; recalca las reglas RP-8
(los servicios conocidos JAMÁS van a mtvec). Ejercicio 1 = el trap eterno
(comentar el ajuste de mepc, frenar con «Detener»); el desafío (servicio
que DEVUELVE un valor en a0) cierra con la idea-tesis: «la semilla de un
sistema operativo». Salida determinista de 4 líneas, exit 0.

[Seguro] Integración: README del repo con sección «Aprendé RISC-V con
EmuRV» (enlaza lecciones/README.md, resume el recorrido 01→10); Ayuda
integrada con párrafo nuevo en «Ciclo de trabajo» (clave i18n
«ayuda.lecciones» — sin strings hardcodeados, regla 6); índice de
lecciones con las 10 filas en ✓. Capítulos de la memoria NO tocados
(decisión de la coordinadora, regla del plan).

[Seguro] CIERRE RP-9 — compuerta F4 completa:
- 10/10 lecciones con test de salida EXACTA en emurv-cli/tests/lecciones.rs ✓
- cargo test emurv-cli: 16+10+6+1 ok / 0 fail ✓
- cargo test src-tauri: 26 binarios ok / 0 fail ✓
- clippy ambos crates: 0 avisos ✓
- tsc --noEmit: limpio ✓
- ci-local.sh --skip-bundle: TODO VERDE ✓
- cargo tauri build: ok; bundle 22 MB ≤ 30 MB (RNF-04) ✓

[Seguro] Balance RP-9: 10 lecciones (12 archivos .s — la 08 son 3), índice
con mapa de aprendizaje, 10 tests CI, plantilla idéntica en todas
(OBJETIVOS / PANELES PARA OBSERVAR / CÓMO USARLA / bloques con comentario
predictivo / 3 ejercicios con desafío). Los ejemplos existentes intactos
(regla de alcance). Syscalls de la lección 7 = solo las existentes
(1/4/5/10/11/30/32); cuando RP-10 agregue 8/9/93, esa lección se amplía.

[PENDIENTE HUMANO] (1) Lección 09 con la app: teclear durante la encuesta →
marcas de color + contador > 0 (headless ya probado por CI). (2) La prueba
de fuego del plan: el primo del autor siguiendo las lecciones sin ayuda —
las lecciones son el guion de ese test externo.

## 2026-06-12 — RP10-F1: Exit2 (93) con código real y rechazo didáctico del 17

[Seguro — fuente verificada por la coordinadora 2026-06-12: wiki oficial de
RARS (repo TheThirdOne/rars), página «Environment Calls»] RESOLUCIÓN de la
duda [SIN VERIFICAR] del Cap. VI §6.2.3: en RARS, Exit2 es el servicio
**93** (número del ABI Linux RISC-V); ReadString=8, Sbrk=9, Exit=10. **El
17 NO existe en RARS** — era el Exit2 de MARS (MIPS). Decisión: EmuRV
implementa el 93 y RECHAZA el 17 con mensaje didáctico que apunta al 93
(mejor que el genérico «syscall desconocida»: el 17 es el error típico de
quien viene de material MIPS). La coordinadora sincroniza el Cap. VI.

[Seguro] Núcleo: syscall 93 → `Exited { code: a0 as i32 }` (el núcleo
conserva el i32 REAL; ni clamp ni máscara — eso es política del CLI). El 17
delega PRIMERO si hay mtvec (para RP-8 es un desconocido más, mcause=11:
test de ambos caminos); sin mtvec, el mensaje didáctico. Syscall 10 intacta
(siempre código 0, regresión fijada).

[Seguro] CLI (`codigo_salida_invitado`): primero `code & 0xFF` (estilo
Unix: -1 → 255), después tope en 125 con aviso por stderr — 126/127/128+
tienen significado propio del shell (no ejecutable / no encontrado /
señales). Aplica a `run` y a `debug`. Colisión aceptada y documentada (F4
la lleva al README): si el invitado elige 124, es indistinguible del
timeout/--max-instr del CLI.

[Seguro] UI: sin cambios — `estado.terminado` ya mostraba «Programa
terminado (código {code})»; ahora el code es el real del 93.

Tests: src-tauri/tests/rp10_syscalls.rs (6: 93 con 7/0/125/-3, 17 ambos
caminos, regresión 10) + 6 de proceso en emurv-cli/tests/cli.rs (7 real,
125 sin aviso, 126→125+aviso, -1→255→125+aviso, regresión 10, stdout
limpio). Compuerta F1: suite completa AMBOS crates verde, clippy 0.

## 2026-06-12 — RP10-F2: ReadString (8) y Sbrk (9)

[Seguro] Syscall 8 (ReadString): misma vía bloqueante que la 5
(InputRequest/InputMsg, frame de video publicado antes). Guarda a lo sumo
a1−1 bytes + NUL en a0; trunca SIN error (al byte: un UTF-8 multibyte puede
quedar partido — semántica de bytes documentada en el código); a1 ≤ 0 no
escribe nada NI pide entrada; a1 == 1 escribe solo el NUL. Buffer validado
con `validar_buffer_escribible` (espejo exacto de los rechazos didácticos
de write_mem RP-7: .text inmutable / teclado MMIO / zona inválida, ahora
para rango arbitrario).

[Probable → decisión registrada] APARTAMIENTO de RARS: fgets (y RARS)
agregan el '\n' final si cabe; EmuRV NO lo agrega. Razón didáctica: una
cadena con salto invisible es la trampa clásica («¿por qué mi comparación
falla?»); el CLI además ya entrega la línea sin '\n' (puente_consola la
recorta). Alternativa considerada: fidelidad bit a bit con RARS — descartada
porque el público es principiante, no migrador de RARS.

[Seguro] Syscall 9 (Sbrk): HEAP_BASE = 0x1004_0000 (constante en
core/mod.rs, canónico RARS, dentro de DATA_REGION: sin colisión heap/pila
posible). Puntero creciente en MachData.heap_next, próximo bloque alineado
a 4, reset en load()/reset() (sin heap fantasma entre corridas), sin free
(como RARS).

[Seguro — decisión registrada] APARTAMIENTO de RARS en el agotamiento:
RuntimeError claro («el heap se agotó: pediste N y quedan M») en vez de un
-1 silencioso — un principiante que recibe -1 lo usa de dirección y revienta
20 instrucciones después con un error incomprensible. Cantidad negativa:
mismo criterio (el heap solo crece). Carga: .data que alcanza 0x1004_0000 →
error didáctico «tus datos pisan el heap» (test del límite exacto: 196608
bytes caben, 196612 no).

Tests: 8 nuevos en rp10_syscalls.rs (14 total) + combinado CLI por pipe
(sbrk → readstring al heap → imprimir; verifica también la ausencia del
'\n'). Compuerta F2: suites completas verdes (27 binarios src-tauri, CLI
22+10+6+1), clippy 0 ambos.

## 2026-06-12 — RP10-F3: limpiar consola (botón y syscall 60)

[Seguro] `ConsoleEvent::Clear` nuevo en el núcleo; syscall 60 (extensión
EmuRV, NO RARS — documentada como tal) lo emite sin frenar la ejecución.
Test de secuencia: Output("A") → Clear → Output("B") en orden.

[Seguro] UI: evento `emu://consola-limpiar` → `Consola.clear()` (mismo
método que usa Reiniciar). Botón «Limpiar» en el panel Consola: siempre
visible (con o sin sesión), solo borra el texto — la máquina ni se entera;
i18n `consola.limpiar` + tooltip. CSS discreto arriba a la derecha.

[Seguro — decisión registrada] CLI: el 60 emite `\x1B[2J\x1B[H` SOLO si
stdout es una terminal real (`IsTerminal` de std, estable desde Rust 1.70).
En pipe/CI: NADA — el stdout sigue siendo exclusivamente la salida del
invitado y los diffs no se ensucian. Test de proceso: "A" + 60 + "B" por
pipe da exactamente "AB", ni un byte ESC. La rama TTY no es testeable por
pipe (por definición); verificación manual pendiente.

[PENDIENTE HUMANO] (1) Botón «Limpiar» en la app: borra el texto, no
interrumpe una corrida, funciona sin sesión. (2) Syscall 60 en terminal
real: limpia la pantalla de verdad (correr lecciones/.. cualquier programa
con li a7,60 SIN pipe).

Compuerta F3: suites verdes (27 binarios src-tauri; CLI 23 en cli.rs +
resto), tsc limpio, clippy 0 ambos.

## 2026-06-12 — RP10-F4: lección 7 ampliada, docs y cierre de RP-10

[Seguro] Lección 7: tabla COMPLETA (1/4/5/8/9/10/11/30/32/60/93, el 60
marcado «extensión de EmuRV, no RARS»), bloque nuevo que encadena los tres
servicios nuevos (sbrk 32 bytes → leer el nombre con el 8 al bloque →
saludar → salir con 93 código 0), ejercicio 2 reemplazado: salir con el
número elegido como código y verificarlo con «echo $?». Test actualizado
(stdin 25/7/Ana, salida exacta con «¡Hola, Ana!»; doble corrida idéntica).

[Seguro] Docs: Ayuda integrada con la tabla completa (8/9/60/93, el 60
como extensión); README con tabla ampliada + nota del 17 + párrafo de
propagación del código de salida (& 0xFF → tope 125) y la colisión-124
documentada; docs/TRAPS.md: servicios conocidos actualizados y nota
explícita de que el 17 es «desconocido» para la delegación (handler del
guest primero si hay mtvec); índice de lecciones actualizado.

[Seguro] CIERRE RP-10 — resumen del punto:
- 93 → exit code real propagado núcleo→UI→CLI (acotado en CLI) ✓ F1
- 17 NO implementado (no existe en RARS — duda del Cap. VI §6.2.3 resuelta
  con fuente, wiki oficial de RARS); rechazo didáctico ✓ F1
- 8 con límite de buffer y validación de zonas (espejo RP-7) ✓ F2
- 9 con HEAP_BASE canónico RARS 0x10040000, alineado, error didáctico al
  agotarse, .data-pisa-heap rechazado en carga ✓ F2
- botón «Limpiar» + syscall 60 con TTY-detect (pipes limpios) ✓ F3
- lección 7 ampliada + Ayuda + README + TRAPS ✓ F4
Apartamientos de RARS registrados: sbrk sin -1 silencioso; readstring sin
el '\n' de fgets. ROADMAP-PRO.md y Cap. VI intactos (coordinadora).

[PENDIENTE HUMANO] (heredados de F3): botón «Limpiar» en la app y syscall
60 en terminal real (TTY). Nuevo: «echo $?» tras un 93 (el dueño del repo
dijo que lo verifica él mismo con un programa propio).

## 2026-06-12 — RP11-F1: spike capstone vs decoder propio (medición de peso)

[Seguro — números medidos en esta máquina, cargo tauri build + du -sk
sobre EmuRV.app, rama spike/rp11-capstone commit 3f0a167, NO mergeada]
- Base (HEAD rp10-fase-4): **22032 KB** (~21,5 MiB; binario 21956 KB).
- capstone 0.14 default (todas las arquitecturas, full): **29044 KB** →
  delta **+6,9 MB**. Pasa el límite de 30 pero VIOLA la regla del margen
  (≥3 MB libres): quedaban ~1 MB. DESCARTADO así.
- capstone diet (default-features=false, std+arch_riscv): 22084 KB
  (+52 KB)… pero DIET elimina los mnemónicos (mnemonic() = None, el
  listado muestra «?»): INSERVIBLE para un desensamblador didáctico.
  Documentado para que nadie lo «redescubra».
- **capstone full + arch_riscv solo** (default-features=false, features
  std+full+arch_riscv): **22132 KB → delta +100 KB**, mnemónicos completos
  (las 10 palabras del spike: addi/lw/add/sub/mul/beq/jal/csrrw/ecall
  decodificadas). Margen sobre RNF-04: ~8,4 MB ≥ 3 MB. Compilación de
  capstone-sys: ~40 s la primera vez (aceptable, cachea).

DECISIÓN [Seguro]: **capstone con default-features=false y features
["std","full","arch_riscv"]**. La regla del plan se cumple con margen
holgado; el decoder propio (plan B) queda descartado por innecesario —
más trabajo para el mismo resultado. La interfaz disasm() encapsula la
elección por si esto cambiara.

[Seguro] PoC object read_core: parsea nuestro ELF round-trip (formato
Elf32, arch Riscv, entry 0x00400000, 2 segmentos, símbolos main y v del
.symtab de RP-6). Es agregar el feature read_core a la dep existente:
cero dependencias nuevas para el import.

[Seguro] `brew install riscv64-elf-gcc`: FUNCIONÓ. Fixture real compilada
con los flags canónicos (-march=rv32im -mabi=ilp32 -Ttext=0x00400000
-Tdata=0x10010000 -nostdlib): ELF32 EXEC RISC-V con **entry 0x400068**
(≠ TEXT_BASE pero dentro de .text → el caso «entry variable» del diseño
es REAL, no teórico) y **PT_LOAD con p_vaddr 0x003ff000**: gcc mapea la
página de cabeceras ELF por debajo de TEXT_BASE. Implicación de diseño
para F2 [Probable→a validar]: cargar por SECCIONES ALLOC cuando existan
(evita la página de cabeceras); segmentos como fallback con regla de
intersección. La fixture y su Makefile entran en F2.

Compuerta F1: números medidos ✓ · decisión fundamentada ✓ · rama spike
sin mergear (queda como evidencia) ✓.

## 2026-06-12 — RP11-F2: import de ELF32 RISC-V con validaciones didácticas

[Seguro] `core/elf_import.rs`: import_elf(bytes) → Program normal (la
abstracción de Machine NO cambió — la prueba de que RP-3/RP-6 cortaron
bien). Program ganó dos campos: `entry: u32` (el ensamblador pone
TEXT_BASE; un ELF puede traer otro e_entry DENTRO de .text — la fixture
gcc real entra por 0x400068) y `from_binary: bool` (source_map vacío).

[Seguro] Estrategia de carga: por SECCIONES SHF_ALLOC cuando existen
(PROGBITS se copia, NOBITS/.bss reserva ceros; ejecutable→.text, resto→
.data) — esquiva la página de cabeceras que gcc mapea bajo TEXT_BASE
(hallazgo del spike F1). Fallback para binarios strip: segmentos PT_LOAD
con recorte del preámbulo de cabeceras. Validaciones didácticas en orden
de diagnóstico: magic / clase 32 / little-endian / EM_RISCV(243) /
ET_EXEC (ET_DYN → «agregá -no-pie»; ET_REL → «falta el linkeo»); sección
fuera del mapa y entry fuera de .text cortan CON los flags canónicos
completos en el mensaje (-march=rv32im -mabi=ilp32 -Ttext=0x00400000
-Tdata=0x10010000 -nostdlib). object: feature read_core agregado a la dep
EXISTENTE (cero deps nuevas, como verificó la coordinadora).

[Seguro] CLI: run/debug detectan ELF por MAGIC (la extensión no importa);
error de import → exit 2 (mismo significado que «el fuente no ensambla»).
`asm` sigue siendo solo para fuentes. Fixture real commiteada en
src-tauri/tests/fixtures/rp11/ (fixture.c + Makefile de regeneración +
fixture.elf — gcc de brew instalado en F1).

[Seguro] Tests (5 núcleo + 3 CLI + 1 gdb): round-trip asm→ELF→import→
correr con MISMA salida y símbolos preservados; cada validación parcheando
bytes de un ELF válido (incl. sección movida a 0x1000 → error con flags);
fixture gcc importa y corre («42\n», exit 0 por syscall 93, entry
variable real); gdb stub sobre ELF importado con breakpoint en la
dirección del símbolo del .symtab IMPORTADO (Z0/c/g, RP-6 intacto).

Compuerta F2: suites verdes (28 binarios src-tauri; CLI 44 tests en sus
suites), clippy 0 ambos.

## 2026-06-12 — RP11-F3: desensamblador y modo binario

[Seguro] `core/disasm.rs`: capstone con la config del spike (default-
features=false, std+full+arch_riscv). Interfaz única disasm(words, base)
→ Vec<DisasmLine{addr, word, texto}> — capstone encapsulado en UN módulo
(si la decisión cambiara, se reemplaza solo acá). Palabra que no
decodifica → `.word 0x...` SIN cortar el listado (decodificación palabra
por palabra, no disasm_all: la basura en .text no arrastra a las vecinas).

[Seguro] Modo binario (UI): «Abrir» acepta .elf y detecta por MAGIC en el
backend (comando abrir_programa → fuente | binario). El caso binario crea
la sesión (crear_sesion extraído de assemble SIN cambiar semántica) y
puebla el source_map CON EL LISTADO (línea n ↔ dirección): el resaltado
de ejecución, la validación de breakpoints, los condicionales RP-7 y
get_state funcionan SIN tocar una línea de ese código. Editor solo-
lectura (Compartment readOnly+editable), banner «binario importado»,
Ensamblar deshabilitado; Ejecutar/Paso/Detener/Reiniciar intactos. Salida
del modo: abrir un fuente o cargar un ejemplo.

[Seguro] Panel «Memoria»: columna de instrucción decodificada al final de
cada fila SOLO cuando la fila cae en .text (comando get_disasm; vale para
fuentes normales también — beneficio general, no solo modo binario).

[Seguro] CLI: `emurv-cli disasm <programa.elf|fuente.s>` — listado por
stdout `dirección:  palabra  instrucción`; fuente y su ELF dan listado
IDÉNTICO (test). Tests F3: repertorio completo RV32IM+Zicsr (~55 fuentes)
con round-trip de mnemónicos (alias de capstone aceptados: mv/nop/csrw…);
.word sin pánico con vecinas intactas; breakpoint en la dirección de la
línea 3 del listado frena exacto; salida de proceso estable.

[Seguro — LA compuerta de la fase] Bundle RE-MEDIDO con capstone adentro
y el módulo enlazado de verdad: **22224 KB (~21,7 MiB) ≤ 30 MB**, margen
~8,3 MB. La predicción del spike (+100 KB) se sostuvo: +192 KB total
contra la base de rp10-fase-4 (incluye también el código nuevo de import
y disasm).

[PENDIENTE HUMANO] Verificación visual del modo binario en la app: Abrir
→ .elf (exportá uno con «Exportar ELF» o usá la fixture de
src-tauri/tests/fixtures/rp11/), banner visible, listado solo-lectura,
breakpoint sobre una línea del listado, Paso con resaltado moviéndose por
el listado, columna de instrucción en «Memoria» → «.text».

Compuerta F3: suites verdes (29 binarios src-tauri, CLI 46) · clippy 0
ambos · tsc limpio · bundle ≤ 30 MB re-verificado ✓.

## 2026-06-12 — RP11-F4: docs de import/desensamblador y cierre de RP-11

[Seguro] Docs: README con sección «Abrir binarios ELF (import y
desensamblador)» — flags de gcc copiables, límites honestos (solo
ET_EXEC absoluto, syscalls RARS no Linux), subcomando disasm con salida
de ejemplo, fixture señalada; docs/DEPURACION.md §Modo binario (cada
línea del listado = una dirección; todo RP-7 funciona igual);
docs/DEPURACION-GDB.md §Binarios importados (b nombre funciona si hay
.symtab: el cliente lee símbolos del mismo archivo); Ayuda integrada con
párrafo «ayuda.binario» (flags incluidos).

[Seguro] CIERRE RP-11 — resumen del punto contra los criterios:
- Cargar y correr ELF round-trip ✓ (test núcleo + proceso) y uno de gcc
  REAL ✓ (toolchain instalada en F1; fixture commiteada con Makefile).
- Desensamblado verificado contra el repertorio propio ✓ (~55 fuentes,
  alias documentados); objdump no se usó (no hacía falta: la fixture gcc
  y el round-trip cubren el contrato).
- Depuración por dirección sobre binario ✓ (breakpoint en línea del
  listado) + gdb stub sobre ELF importado ✓ (símbolo del .symtab).
- Decisión capstone-vs-propio POR MEDICIÓN con RNF-04 intocable ✓:
  capstone full+arch_riscv, bundle final 22 MB ≤ 30 (margen ~8 MB);
  rama spike/rp11-capstone como evidencia, sin mergear.

[PENDIENTE HUMANO] (heredado de F3) modo binario visual en la app; y la
verificación del dueño: round-trip a mano (asm→import→run), listado de
disasm contra el ensamblador, y el peso del bundle.

## 2026-06-12 — RP12-F1: spike FP (mstatus.FS y registros)

[Seguro — todo medido por tests/spike_fp.rs, Unicorn 2.1.5 crudo]
Respuestas del spike, en orden del plan:

1. **fadd.s en estado crudo: ILLEGAL.** mstatus arranca en 0x0 → FS
   (bits 14:13) = Off → emu_start da Err(EXCEPTION) en la primera
   instrucción FP. La trampa anunciada es real.
2. **El arreglo: FS=Initial desde el host.** `mstatus |= 1<<13` ANTES de
   arrancar y fadd.s ejecuta (1.5+0.25 → bits exactos de 1.75f32).
   mstatus queda 0x80006000 tras la corrida: el motor mismo lo promovió
   a FS=Dirty (11) + SD (bit 31) al ensuciar registros — autocoherente.
   DECISIÓN: load() inicializa mstatus = MIE | FS_INITIAL (1<<3 | 1<<13),
   comentado con la cita de la espec privilegiada (mstatus.FS, §3.1.6).
3. **IDs del binding: el dato de la coordinadora estaba ERRADO** (decía
   F0=57..88, FCSR=141). Extraídos del bindings.rs GENERADO (OUT_DIR):
   **F0=158..F31=189, FCSR=43**; MSTATUS=116 sí coincidía (RP-8 lo usa).
   Round-trip host↔guest verificado: host escribe F0/F1 (ids 158/159),
   guest suma, host lee F2 (160) = 3.0f32 exacto.
4. **NaN-boxing: flw SÍ boxea** (f0 = 0xFFFFFFFF_3FC00000 tras flw de
   1.5f32) **pero el RESULTADO de operaciones .s NO** (fadd.s deja los
   32 altos en CERO — apartamiento de la espec D, medido y fijado por
   test como centinela). Regla derivada para el panel F y la UI:
   alto==0xFFFFFFFF O (alto==0 y bajo≠0) → interpretar como f32(bajos);
   si no → f64. (Un f64 real con alto==0 es un denormal < 1e-308: no
   aparece en uso didáctico.)
5. **fadd.d funciona** (1.5+0.25 = bits exactos de 1.75f64).
6. **FCSR accesible** por id host 43 y por csrr 0x003 del guest;
   fmv.x.w mueve los bits f→x correctos.

Tropiezo del spike (documentado para el ensamblador): `addi t0, t0,
0x800` NO da +2048 — el inmediato de 12 bits es con signo (0x800 =
-2048); el primer intento leyó 0x3FF800 no mapeada. Nuestro encoder ya
lo maneja (RP-0), pero el spike lo re-descubrió a mano.

Compuerta F1: 6/6 preguntas respondidas [Seguro], decisión de
inicialización tomada, spike verde, clippy 0.

## 2026-06-12 — RP12-F2: ensamblador F/D verificado contra GNU as

[Seguro — EL criterio de la compuerta] Oráculo GNU: el repertorio F/D
completo (60 instrucciones reales: loads/stores, aritmética, sqrt, fma R4,
conversiones, comparaciones, sign-inject, fclass, fmv x↔w, y los pseudos
fmv/fabs/fneg de un word) ensamblado por NOSOTROS y por riscv64-elf-as
(-march=rv32imfd -mabi=ilp32d) da las MISMAS palabras, una por una
(tests/rp12_gnu_oracle.rs; sin toolchain el test se saltea con aviso).
Verde a la PRIMERA corrida — los detalles finos coincidieron: rm=dyn(7)
en aritmética/conversiones inexactas, rm=0 en las conversiones EXACTAS
(fcvt.d.s / fcvt.d.w / fcvt.d.wu, que GNU también fija en 0), funct3
fijos de sgnj/min-max/comparaciones/fclass/fmv.

[Seguro] Ensamblador: tabla propia freg_number (ft0-11/fs0-11/fa0-7/fN,
convención de llamada estándar), OpKind::FReg (los .eqv no pueden pisar
nombres f: el registro gana), shapes con errores didácticos
(«fadd.s … necesita registros f … usaste un registro entero»; fcvt con
destino equivocado dice cuál va dónde), flw/fld/fsw/fsd con etiqueta =
lui(aux)+acceso con AUXILIAR ENTERO obligatorio (el destino f no puede
hacer de puntero, a diferencia de lw — documentado en el error).
Rechazos del diseño: fmv.x.d/fmv.d.x («no existe en RV32: el camino es
fsd + lw/lw») y modo de redondeo explícito como 4.º operando
(rne/rtz/rdn/rup/rmm/dyn → «EmuRV usa siempre dyn»).

[Seguro] Máquina: load() inicializa mstatus = MIE | FS_Initial (1<<3 |
1<<13) con cita de la espec (§3.1.6) — sin eso la primera flw da ilegal
(spike F1); registros f y fcsr a cero por sesión/reinicio (sin valores
fantasma); registers_f() → [u64;32] con la nota del no-boxeo de
resultados .s. Constantes FREG_ID_BASE=158 / CSR_ID_FCSR=43 en core
(las verificadas por el spike, no las erradas).

[Seguro] Ejecución con bits IEEE exactos (tests/rp12_fp_exec.rs):
1.5+0.25=1.75 en f32 Y f64 (por memoria y por registros), fmadd
2*3+0.5=6.5, fneg/fabs por sgnj, round-trip fcvt 7→7.0→7, feq/flt a
registro entero, acceso por etiqueta en ambas direcciones.

Compuerta F2: oráculo palabra a palabra ✓ · suites verdes (32 binarios
src-tauri, CLI 46) · clippy 0 ambos ✓.

## 2026-06-12 — RP12-F3: syscalls float y panel de registros F

[Seguro] Syscalls RARS: 2 (PrintFloat, fa0 como f32 de los bits bajos),
3 (PrintDouble, fa0 entero), 6 (ReadFloat → fa0 NAN-BOXED: 0xFFFFFFFF en
los altos, como exige la espec D — test lo fija bit a bit), 7 (ReadDouble
→ bits crudos). Lectura con la vía bloqueante de read_int y reintento
didáctico («…usá punto decimal, ej. 1.5» — la coma rioplatense se rechaza
con guía, test incluido).

[Seguro — divergencia registrada] Formato de impresión = el «{}» de Rust:
1.5 / -0 / NaN / inf. RARS (Java) imprime p. ej. Infinity y -0.0.
Alternativa considerada: imitar a Java — descartada (frágil, sin valor
didáctico); el contrato es NUESTRO formato y el test
print_float_formato_fijado lo vigila.

[Seguro] Panel «Registros F (flotante)»: plegable patrón CSRs RP-8
(cerrado = CERO invokes — get_fregs solo viaja con la sección abierta),
32 filas nombre ABI + hex de 64 + valor interpretado, y fcsr al pie.
Interpretación según la regla del spike F1: alto==0xFFFFFFFF (boxed) o
alto==0 con bajo≠0 (quirk de Unicorn: no boxea resultados .s) → f32;
si no → f64 — la celda lo etiqueta («1.75 (f32)»). u64 viaja como hex de
16 dígitos (JSON no representa u64 sin perder precisión).

[Seguro] Edición inline (guard de pausa de RP-7, write_reg_f por canal):
«0x…» = bits crudos de 64; decimal con punto se codifica según la
interpretación ACTUAL del registro (f32 → NaN-boxed, f64 → bits double).
write_reg_f valida índice (f0–f31) con rechazo claro.

[PENDIENTE HUMANO] Panel F en la app: abrir la sección (cerrada por
defecto), correr la lección/ejemplo de floats, ver «1.75 (f32)» y editar
un registro pausado (decimal y 0x crudo); fcsr mostrando flags tras una
división por cero.

Compuerta F3: suites verdes (34 binarios src-tauri), clippy 0, tsc limpio.

## 2026-06-12 — RP12-F4: ejemplo flotantes y docs F/D (cierre RP-12)

[Seguro] Ejemplo `ejemplos/13-flotantes.asm` (11.º del menú): promedio de
3 notas leídas con el servicio 6, acumuladas con fadd.s, divididas con
fdiv.s (divisor por fcvt.s.w), impreso con el 2. Determinista por CLI
(7.5+8.0+10.0=25.5/3=8.5 — todos exactos en f32): doble corrida idéntica,
exit 0. En el menú «Ejemplos» (i18n ejemplo.flotantes).

[Seguro] Desensamblador: capstone arch_riscv decodifica F/D — VERIFICADO
(no asumido): test núcleo disasm_cubre_fd (fadd.s/fmul.d/fcvt.w.s/flw/
fmadd.s) + test CLI disasm_decodifica_fd sobre el ejemplo 13. El disasm
del ejemplo muestra fcvt.s.w/fadd.s/fdiv.s/fmv.s legibles.

[Seguro] Docs: ENSAMBLADOR.md §4.1 con subsección F/D completa (registros,
repertorio S y D, fmv.x.d rechazado, pseudos, NaN-boxing, syscalls
2/3/6/7) y §5 actualizada (oráculo GNU ahora disponible, reemplaza el
«no había oráculo externo»); Ayuda integrada con filas 2/3/6/7 en la
tabla y párrafo «ayuda.flotantes» (panel «Registros F»); README con
syscalls 2/3/6/7, característica F/D y conteo de ejemplos a 11.

[Probable — PROPUESTA REGISTRADA, no se hace acá] Lección de floats para
el currículo de RP-9 (lección 11): IEEE 754 visto en el panel «Registros F»
(hex64 + interpretado), el «engaño» de comparar floats con ==/feq, el
promedio del ejemplo 13 como base. El plan RP-12 la dejó fuera de alcance
explícitamente; queda anotada para una futura ampliación del currículo.

[Seguro] CIERRE RP-12 — criterios del ROADMAP:
- f0–f31 ABI + repertorio F y D completo (sin rm explícito) ✓ F2
- encodings == GNU as palabra a palabra (60 instrucciones) ✓ F2
- panel de registros flotantes con NaN-boxing interpretado ✓ F3
- syscalls 2/3/6/7 con formato documentado (divergencia {} vs Java) ✓ F3
- ejemplo por UI y CLI; IEEE 754 con valores exactos ✓ F2/F4
- desensamblador cubre F/D ✓ F4
Suites verdes (33 binarios src-tauri, CLI 48), clippy 0 ambos, tsc limpio,
ci-local --skip-bundle verde, tauri build ok, bundle 22 MB ≤ 30.

[PENDIENTE HUMANO] (heredados de F3) panel F en la app: abrir la sección,
correr el ejemplo 13, ver «8.5 (f32)», editar un registro pausado.
Verificación del dueño: ejemplo 13 por CLI con valores IEEE, un fadd.s
propio contra GNU as, el panel F.

## 2026-06-13 — RP13-F1: spike A nativo y viabilidad de C

[Seguro — medido por tests/spike_ac.rs, Unicorn 2.1.5 crudo]
**A (atómicas): nativo, sin sorpresas.**
- amoadd.w t0,t1,(t2): memoria 100→110, rd recibió el VIEJO (100). ✓
- lr.w + sc.w: store-conditional EXITOSO mono-hart (sc devuelve 0), la
  memoria quedó con el valor. ✓
- Encodings a mano == GNU as -march=rv32ima (amoadd.w=0x0063a2af,
  lr.w=0x1003a2af, sc.w=0x1863a2af). A es el piso: cero riesgo.

[Seguro] **C (comprimidas): VIABLE.** La pregunta central respondida:
- Unicorn RISCV32 decodifica y ejecuta instrucciones C de 2 bytes **SIN
  flag** con el modo actual (Mode::RISCV32). c.addi/c.li ejecutan.
- El pc **avanza de a 2** tras una C (C-P2: una c.li deja pc=BASE+2).
- Secuencia MIXTA 4-2-2-4 (addi + c.addi + c.addi + addi): a0=8 y
  pc=BASE+12 exactos (C-P1). Mezclar tamaños no descarrila el motor.
- Encodings C == GNU as -march=rv32imac (c.addi a0,1=0x0505, c.nop=0x0001,
  c.li a2,5=0x4615, c.mv a1,a0=0x85aa).

**Auditoría de consumidores del supuesto-4-bytes (la condición del plan):**
1. `assemble` pasada 2 (assembler.rs:644-657): emite `w.to_le_bytes()` (4
   bytes) y calcula `addr = ins.addr + k*4`. Cambio acotado: un encoder de
   c.* que devuelve 2 bytes + el loop emitiendo tamaño real.
2. `instr_size` (assembler.rs): YA es variable (li/la/loads con etiqueta
   dan 8). c.* → size=2 es mecánico; `text_off += size` y `cur_addr` ya
   manejan offsets de bytes (no asumen 4 en su estructura).
3. `disasm` + `words_de` (disasm.rs:27-58): asumen 4 bytes (chunks_exact,
   i*4). Cambio acotado: avanzar por longitud (2 bits bajos != 0b11 → 2
   bytes); Capstone arch_riscv decodifica C (verificado RP-11, a confirmar
   en F3).
4. source_map / breakpoints (RP-7) / resaltado: el source_map es un mapa
   addr↔line ARBITRARIO — no asume 4 bytes en su forma, solo el CÓDIGO que
   lo llena (punto 1). Con direcciones reales, los breakpoints por
   dirección y el step quedan exactos.

**Por qué NINGÚN test de RP-1..RP-12 se rompe:** el ensamblador NO comprime
automáticamente (decisión del autor) — una instrucción de 32 bits sigue
emitiendo 32 bits idénticos; un programa sin c.* produce las MISMAS
direcciones de hoy. C aparece solo si el usuario escribe c.* o se importa
un ELF. El disasm reescrito para longitud variable debe dar resultados
idénticos sobre input de solo-4-bytes (lo vigilan los tests RP-11/12).

DECISIÓN [Seguro]: **C VIABLE** — los cambios son acotados y enumerables, y
el invariante se preserva porque la compresión no es automática. Se
implementa en F3.

Compuerta F1: spike A verde + veredicto C=VIABLE con evidencia ✓.

## 2026-06-13 — RP13-F2: extensión A (atómicas) verificada contra GNU

[Seguro] Ensamblador: repertorio RV32A completo — lr.w, sc.w, amoswap.w,
amoadd.w, amoand.w, amoor.w, amoxor.w, amomin.w, amomax.w, amominu.w,
amomaxu.w. Sufijos de ordenamiento `.aq`/`.rl`/`.aqrl` opcionales (sin
sufijo = ambos en 0). Helper `parse_amo` (funct5 + aq/rl + es_lr/es_sc),
encoder `enc_amo` (opcode AMO 0x2F, funct3=010). La dirección es «(rs1)»
PURA: un desplazamiento (`4(t2)`) se rechaza con error didáctico (la espec
no lo permite). lr.w toma rd+(rs1); el resto rd,rs2,(rs1).

[Seguro — oráculo] Repertorio A (16 formas, con y sin sufijos) ensamblado
por nosotros == `riscv64-elf-as -march=rv32ima` palabra a palabra
(tests/rp13_atomicas.rs). Verde a la primera.

[Seguro] Ejecución (Unicorn nativo, spike F1): amoadd.w actualiza la
memoria y devuelve el VIEJO en rd (100+10=110, rd=100); lr.w+sc.w
store-conditional EXITOSO mono-hart (sc→0, memoria=42); amomaxu trata -1
como el mayor sin signo (0xFFFFFFFF gana). Disasm A por capstone
(amoadd.w/lr.w/sc.w legibles).

[Seguro — semántica mono-hart, documentada] EmuRV es de un solo hart: una
reserva lr nunca es invalidada por otro procesador, así que sc SIEMPRE
tiene éxito en uso normal. lr/sc y amo* son secuencialmente consistentes
triviales — NO se promete concurrencia real (no hay multi-hart). Es el
modelo correcto para enseñar el patrón lr/sc, no para paralelismo.

Compuerta F2: encodings == GNU ✓ · suite completa (35 binarios src-tauri,
CLI 48) ✓ · clippy 0 ambos ✓. C entra en F3 (veredicto F1 = VIABLE).

## 2026-06-13 — RP13-F3: extensión C compatible con source_map y breakpoints

[Seguro] C IMPLEMENTADA (veredicto F1 = VIABLE confirmado en la práctica).
Set didáctico: c.nop, c.addi, c.li, c.mv, c.jr, c.j, c.lw, c.sw, c.beqz,
c.bnez — las que un alumno escribe a mano para ver compresión. El
ensamblador NO comprime automáticamente: una instrucción de 32 bits sigue
emitiendo 32 bits; C aparece solo si el usuario la escribe (o se importa
un ELF).

[Seguro — oráculo] Repertorio C ensamblado por nosotros == `riscv64-elf-as
-march=rv32imac` BYTE A BYTE (tests/rp13_comprimidas.rs, comparación de
buffers completos, no solo palabra a palabra). Encodings verificados a
mano contra GNU antes de codificar (c.nop=0x0001, c.addi a0,1=0x0505,
c.li a2,5=0x4615, c.mv a1,a0=0x85aa, c.jr ra=0x8082, c.j off=10=0xa029,
c.lw a0,4(a1)=0x41c8, c.sw a0,8(a1)=0xc588, c.beqz a0,+4=0xc111,
c.bnez a0,+2=0xe109) — el scrambling de inmediatos CJ/CB/CL es el de la
espec.

[Seguro] El invariante de tamaño variable, resuelto con cambios ACOTADOS:
- `instr_size`: c.* → 2 (ya era variable para li/la/loads con etiqueta).
- pasada 2: rama de 2 bytes (`encode_compressed -> u16`) ANTES del camino
  de 4; el source_map se llena con la dirección REAL de cada instrucción.
  El camino de 4 bytes quedó INTACTO → cero regresión.
- `disasm_bytes(text, base)`: nuevo, recorre por longitud (2 bits bajos !=
  0b11 → 2 bytes) con capstone + extra_mode RiscVC (sin ese flag capstone
  devuelve la C como dato — medido). Para .text de solo-4-bytes da
  resultado idéntico a `disasm(words_de(...))`. commands.rs (modo binario)
  y `emurv-cli disasm` ahora usan disasm_bytes.

[Seguro] CERO regresión verificada: los 36 binarios de test de src-tauri y
los 48 del CLI siguen verdes (breakpoints RP-7, resaltado, disasm RP-11,
import RP-11). Pruebas nuevas del invariante: source_map con tamaño mixto
(meta tras 4+2+2 está en +8), breakpoint sobre una C frena exacto en +6,
ejecución mixta 4/2 (a0=8), disasm_bytes con direcciones correlativas,
ELF round-trip con C importadas.

Compuerta F3: todo verde, ningún test previo roto, clippy 0 ambos, tsc
limpio. C ENTRÓ limpia.

## 2026-06-13 — RP13-F4: ejemplo atómicas y docs A/C (cierre RP-13)

[Seguro] Ejemplo `ejemplos/14-atomicas.asm` (12.º del menú): contador
atómico con amoadd.w (+10, +5) y lr.w/sc.w (=100), con instrucciones C
intercaladas (c.li/c.addi/c.mv). Imprime 100, determinista por CLI (doble
corrida idéntica), exit 0. Detalle didáctico: c.li 100 NO cabe (6 bits) →
la línea usa `li` de 4 bytes, comentado — el alumno ve POR QUÉ no toda
constante entra en una C.

[Seguro] Docs: ENSAMBLADOR.md §4.1 con subsecciones A y C (repertorios,
semántica mono-hart, restricciones x8–x15 e inmediato de 6 bits de C, RV64
fuera); Ayuda integrada con párrafo «ayuda.atomicas»; README con la
característica A/C y conteo de ejemplos a 12. Tests CLI: ejemplo 14 corre
(100) y su disasm lista A (amoadd.w) y C (c.li) mezcladas.

[Seguro] CIERRE RP-13 — criterios del ROADMAP:
- Extensión A completa, encodings == GNU, mono-hart documentada, ejecuta
  por UI y CLI ✓ F2
- Extensión C: VIABLE (F1) e IMPLEMENTADA sin romper invariantes ✓ F3 —
  source_map/breakpoints/resaltado/disasm soportan tamaño variable; cero
  regresión (36 binarios src-tauri + 50 CLI verdes)
- RV64 explícitamente fuera (decisión del autor) ✓
- Cero regresión RP-1..RP-12 ✓

**Veredicto del spike (resumen para el reporte): C VIABLE.** Razón:
Unicorn ejecuta C nativo sin flag y el pc avanza de a 2; los consumidores
del supuesto-4-bytes son acotados (encode loop, disasm, source_map fill) y
el invariante se preserva porque la compresión NO es automática — un
programa sin c.* produce bytes y direcciones idénticos a los de antes.
Ambos desenlaces eran cierre válido; entró el A+C completo.

Suites verdes (36 src-tauri, 50 CLI), clippy 0 ambos, tsc limpio,
ci-local --skip-bundle verde, tauri build ok, bundle 22 MB ≤ 30.

## 2026-06-13 — RP14-F1: tokens ISA al día (RV32IMAFDC) y fuente única anti-drift

[Seguro] DEUDA SALDADA: las listas MNEMONICS/REGISTERS del editor estaban
en RV32IM viejo — NO reconocían F/D (RP-12), A ni C (RP-13). El resaltado
ahora cubre el repertorio completo.

[Seguro] Fuente única `src/editor/isa.ts`: MNEMONICS (reales),
PSEUDOS, REGISTERS_X, REGISTERS_F + helpers `esMnemonico` (tolera sufijos
.aq/.rl/.aqrl de las atómicas) y `esRegistro`. `riscv.ts` ya NO tiene
listas propias: consume isa.ts (una sola fuente en el frontend). NO incluye
fmv.x.d/fmv.d.x (el assembler las rechaza: no existen en RV32) ni modos de
redondeo (son operandos).

[Seguro — decisión de diseño anti-drift, registrada] Elegí la opción B del
plan: `assembler::known_mnemonics()` (Rust) es la lista CANÓNICA;
tests/rp14_anti_drift.rs (1) ensambla cada entrada canónica sola y exige
que NO dé «instrucción desconocida» (prueba que known_mnemonics no tiene
fantasmas vs check_shape), y (2) lee isa.ts (include_str!) y exige
MNEMONICS ∪ PSEUDOS == known_mnemonics() (ni falta ni sobra). Si el
ensamblador gana/pierde una instrucción y isa.ts no se actualiza, el test
falla. Tercer test explícito: isa.ts incluye fadd.s/fmul.d/amoadd.w/lr.w/
c.addi/c.lw (la deuda, por si alguien recorta sin querer). Por qué B y no
el cross-assemble TS↔Rust: parsear TS desde Rust es frágil; con
include_str! + comparación de conjuntos el binding es directo y barato.

[Probable] Verificación manual del resaltado (vía tsx sobre isa.ts):
esMnemonico devuelve true para fadd.s/fmul.d/amoadd.w/amoadd.w.aqrl/lr.w/
c.addi/c.lw/add/li y false para basura; esRegistro true para ft0/fa0/f5/
t0/x10/zero y false para q9. En la app el resaltado de esas instrucciones
queda pendiente de ojo humano (clave: abrir ejemplos/13-flotantes.asm y
14-atomicas.asm y ver fadd.s/amoadd.w/c.li coloreadas como instrucción).

Compuerta F1: anti-drift verde (3 tests) · resaltado verificado por helper
· tsc limpio · suite Rust intacta (37 binarios) · clippy 0 ambos.

## 2026-06-13 — RP14-F2: autocompletado contextual

[Seguro] Dep nueva `@codemirror/autocomplete` (mismo ecosistema CM6, solo
JS). Bundle RE-MEDIDO con `cargo tauri build`: 22 MB (sin cambio
apreciable; el peso de autocomplete es JS embebido en el WebView, no toca
el binario). RNF-04 ≤30 holgado.

[Seguro] `src/editor/autocompletar.ts`: núcleo PURO `opcionesAutocompletado
(lineaAntes, doc)` (testeable sin DOM) + adapter `fuenteAutocompletado`
(CompletionContext→núcleo). Contexto: primer token → mnemónicos+pseudos
(de isa.ts, ya con F/D/A/C); tras un mnemónico → registros x y f; tras un
op de salto/rama/la → además las etiquetas del buffer (regex `^etiqueta:`,
fuente local: funciona ANTES de ensamblar, sin backend). Filtrado por
prefijo. Inmediatos (dígito/signo) no se autocompletan. RP-14 NO
re-ensambla al tipear — el provider solo lee el buffer.

[Seguro — decisión de tooling, registrada] No había runner de tests TS.
Elegí `tsx` (devDep liviano) + `node:test`, no vitest: vitest arrastra más
y el proyecto solo necesita correr funciones puras. Script `npm run
test:ui` (`tsx --test "src/**/*.test.ts"`) enganchado en ci-local.sh y
ci.yml (validado con actionlint). Es tooling de test (dev-only, no entra
al bundle): no contradice la regla de «no frameworks pesados» de CLAUDE.md.

[Seguro] Tests del provider (src/editor/autocompletar.test.ts, 8 casos):
inicio de línea ofrece add/fadd.s/amoadd.w/c.addi/li; prefijo «fad» →
[fadd.s, fadd.d]; tras `add ` registros (incl. ft0) sin mnemónicos; tras
`j ` etiquetas del buffer; beq ofrece registros + etiqueta en el último
operando; add NO ofrece etiquetas; etiquetasDelBuffer tolera indentación y
deduplica. 8/8 verde.

Compuerta F2: tests del provider verdes · tsc limpio · bundle re-medido
22 MB ≤ 30 ✓.

## 2026-06-13 — RP14-F3: hover de ayuda y navegación de etiquetas

[Seguro] `src/editor/ayuda-isa.ts`: diccionario sintaxis+descripción en
español para CADA mnemónico del repertorio (RV32IMAFDC+Zicsr+pseudos). El
material CONDENSA Ayuda integrada + lecciones + docs/ENSAMBLADOR.md — sin
inventar semántica (citado en la cabecera). `ayudaDe` tolera sufijos
.aq/.rl/.aqrl (resuelve a la base). `hoverTooltip` de CM6 muestra el
tooltip al pasar sobre un mnemónico; mnemónico SIN entrada o palabra que
no es mnemónico → SIN tooltip (nunca vacío). `descripcionInstr` alimenta
el `info` del autocompletado (F2 quedó enchufado).

[Seguro — cobertura 100%, anti-drift del hover] Test
ayuda-isa.test.ts: cada entrada de MNEMONICS∪PSEUDOS tiene ayuda (falla si
falta una) y el diccionario no inventa claves fuera del ISA. Si se agrega
una instrucción a isa.ts sin su ayuda, el test rojo.

[Seguro] `src/editor/etiquetas.ts` (funciones PURAS, sin backend):
definicionDeEtiqueta (offset de `nombre:`), usosDeEtiqueta (todos los
rangos), palabraEn (identificador en una posición). En index.ts:
- Ir-a-definición: Cmd/Ctrl-click sobre el uso de una etiqueta mueve el
  cursor a su definición; sin definición no rompe (deja pasar el click).
- Resaltar usos: con el cursor sobre una etiqueta con definición, decora
  todas sus apariciones (Decoration.mark, sin tocar el texto); no se
  activa sobre mnemónicos/registros.

[Seguro] Tests: 18 UI verdes (8 autocompletado + 5 hover + 5 etiquetas).
Verificación manual por tsx: hover de fadd.s/c.addi devuelve sintaxis+desc,
amoadd.w.aqrl resuelve a amoadd.w, basura→undefined; def/usos de «bucle»
correctos.

[PENDIENTE HUMANO] En la app: pasar el mouse sobre fadd.s/amoadd.w/c.addi
y ver el tooltip; Cmd/Ctrl-click sobre una etiqueta salta a su definición;
cursor sobre una etiqueta resalta sus usos; Ctrl-Espacio abre el
autocompletado.

Compuerta F3: tests verdes · cobertura hover 100% de mnemónicos · tsc
limpio.

## 2026-06-13 — RP14-F4: docs editor profesional (cierre RP-14)

[Seguro] Docs: README con la característica de resaltado RV32IMAFDC y la de
«Editor profesional» (autocompletado/hover/ir-a-etiqueta); Ayuda integrada
con sección «Editor» nueva (clave i18n ayuda.editor.*) que describe los
cuatro flujos y recuerda que el editor NO re-ensambla al tipear.

[Seguro] CIERRE RP-14 — criterios del ROADMAP:
- Autocompletado de mnemónicos y registros con la lista AL DÍA (no la
  vieja) ✓ F1/F2
- Hover con ayuda por instrucción en español, cobertura 100% ✓ F3
- Ir-a-definición de etiquetas + resaltar usos ✓ F3
- Deuda del resaltado desactualizado (sin F/D/A/C) SALDADA ✓ F1
- Anti-drift: la lista del editor no puede desincronizarse del ensamblador
  (test cruzado isa.ts ↔ known_mnemonics) ✓ F1
Suites verdes (37 src-tauri, 50 CLI, 18 UI), clippy 0 ambos, tsc limpio,
ci-local --skip-bundle verde, tauri build ok, bundle 22 MB ≤ 30 (dep nueva
@codemirror/autocomplete sin impacto medible).

Tooling agregado: `tsx` (devDep) + `npm run test:ui` (node:test) en
ci-local.sh y ci.yml (actionlint OK) — primer runner de tests del
frontend; cubre las funciones puras del editor.

[PENDIENTE HUMANO] Verificación visual en la app (heredada de F3):
autocompletado (Ctrl-Espacio), hover sobre fadd.s/amoadd.w/c.addi,
Cmd/Ctrl-click a definición de etiqueta, resaltado de usos.

## 2026-06-13 — RP15-F1: lecciones embebidas, parser de cabecera e índice

[Seguro] Las 10 lecciones embebidas con `?raw` (patrón de los ejemplos); la
08 embebe sus 3 archivos (main.s + utiles.s + constantes.s). `src/lecciones/
index.ts` arma el currículo ORDENADO (orden de aprendizaje, no alfabético).

[Seguro — anti-drift, patrón RP-14] Metadatos del navegador (número, título,
objetivos, paneles) SIN duplicar: `src/lecciones/parser.ts::parseLeccion`
los extrae de la cabecera de plantilla fija de RP-9. Separé el parser PURO
(parser.ts, sin imports `?raw`) del índice embebido (index.ts) para que el
test corra bajo node:test/tsx — los `?raw` de Vite no resuelven fuera del
bundle. El test (parser.test.ts) corre el parser sobre los .s REALES del
disco (misma fuente que ?raw embebe): las 10 parsean número+título+≥1
objetivo+paneles, la 08 tiene sus 3 archivos, y separa objetivos uniendo
continuaciones. Si una lección rompe la plantilla, el test rojo.

Compuerta F1: test verde (22 UI: 18 previos + 4 lecciones) · tsc limpio ·
suite Rust intacta (37 binarios) · clippy 0.

## 2026-06-13 — RP15-F2: navegador de lecciones

[Seguro] Panel «Lecciones» SEPARADO del selector «Ejemplos» (son cosas
distintas: ejemplos = programas sueltos; lecciones = currículo ordenado).
Botón «Lecciones» en la toolbar abre un overlay modal (patrón de Ayuda):
lista ordenada (número + título, con tilde de completada en F3), y al
seleccionar una, encabezado con sus OBJETIVOS y QUÉ PANELES mirar
(extraídos por el parser de F1) ANTES de cargar el código — el estudiante
sabe a qué entra. «Abrir lección» carga el .s al editor (buffer sin ruta,
como un ejemplo) y la marca completada. i18n: rótulos del navegador en
es.json (lecciones.*/leccion.*); el contenido de las lecciones NO se
traduce (es material didáctico dentro del .s).

[Seguro] El panel ya trae el esqueleto de progreso y navegación
siguiente/anterior y el aviso de multi-archivo; el cableado fino (carpeta +
base_dir, tests de progreso) se cierra en F3.

[Probable] Verificación manual pendiente de ojo humano: abrir 3-4 lecciones
desde el navegador, ver objetivos/paneles correctos y que «Abrir lección»
carga el código al editor.

Compuerta F2: tsc limpio · suite Rust intacta (37) · tests UI 22 verdes.

## 2026-06-13 — RP15-F3: progreso, navegación y caso multi-archivo

[Seguro] `src/lecciones/progreso.ts`: funciones PURAS (parsear/serializar,
alternarCompletada inmutable, vecina con bordes) + wrappers de localStorage
(clave versionada `emurv.lecciones.v1`, tolerantes a modo privado/dato
corrupto). El navegador marca «completada» al abrir una lección y con un
botón por lección; indicador ✓ en la lista; botón «Reiniciar progreso».
Navegación siguiente/anterior por orden de CURRÍCULO (no alfabético),
deshabilitada en los bordes (01 sin anterior, 10 sin siguiente).

[Seguro] Caso 08 (multi-archivo) resuelto coherente con RP-5: «Abrir
lección» pide una carpeta, guarda main.s + utiles.s + constantes.s ahí
(api.guardarArchivo) y abre main.s con `rutaArchivo` = base_dir, de modo
que el `.include` resuelve al Ensamblar. Si el usuario cancela la carpeta:
aviso didáctico (sin abrir nada roto) explicando por qué esta lección
necesita guardarse. Verificado que los 3 archivos ensamblan juntos por CLI
(mismo contenido embebido): salida correcta, exit 0.

[Seguro] Tests (5 nuevos, 27 UI total): parsearProgreso tolera
null/basura/no-array; round-trip estable; alternarCompletada inmutable;
vecina respeta orden y bordes; leer/guardar/reset con stub de localStorage.

[PENDIENTE HUMANO] En la app: marcar lecciones y recargar (persistencia);
siguiente/anterior; abrir la 08 → elegir carpeta → main.s ensambla con sus
includes; cancelar la carpeta → aviso.

Compuerta F3: tests verdes (27 UI) · tsc limpio · caso 08 verificado por
CLI (flujo de carpeta: pendiente humano).

## 2026-06-13 — RP15-F4: docs navegador de lecciones (cierre RP-15)

[Seguro] Docs: README §«Aprendé RISC-V con EmuRV» ahora apunta al botón
«Lecciones» de la app (no solo a la carpeta); Ayuda integrada (ayuda.
lecciones) describe el navegador + el caso de la carpeta de la 08;
lecciones/README.md con nota de acceso desde la app.

[Seguro] CIERRE RP-15 — criterios del ROADMAP:
- Panel/menú de lecciones con el mapa de aprendizaje; abrir con objetivos
  visibles ANTES de cargar ✓ F2
- Progreso persistido (localStorage versionado) + navegación
  siguiente/anterior por orden de currículo ✓ F3
- Sin gamificación (registrada como NO-alcance) ✓
- Metadatos SIN duplicar: parser de la cabecera fija + anti-drift por test
  (patrón RP-14) ✓ F1
- Caso multi-archivo (08) coherente con RP-5 (guardar a carpeta + base_dir)
  ✓ F3
Suites verdes (37 src-tauri, 50 CLI, 27 UI), clippy 0 ambos, tsc limpio,
ci-local --skip-bundle verde, tauri build ok, bundle 22 MB ≤ 30 (10
lecciones embebidas con ?raw, sin impacto medible — texto liviano).

[PENDIENTE HUMANO] Verificación visual: navegador (objetivos/paneles por
lección), progreso persistente al recargar, siguiente/anterior, y el flujo
de la 08 (elegir carpeta → ensambla con includes / cancelar → aviso).

## 2026-06-13 — RP16-F1: loader multi-idioma, selector y persistencia

[Seguro] i18n pasó de dict estático a multi-idioma: `index.ts` con idioma
ACTIVO (es default), `setLang`/`getLang`, fallback a español en `t()`, y
persistencia en localStorage (`emurv.lang.v1`, patrón RP-15). La resolución
es una función PURA `traducir(dicts, lang, key, params)` (idioma → fallback
es → la clave; interpola {params}) — separada para testearla sin
localStorage ni los JSON reales.

[Seguro — decisión registrada] Aplicar el cambio de idioma = PERSISTIR +
`location.reload()`. La UI se construye de forma imperativa (toolbar y
paneles leen `t()` al crearse, sin re-render reactivo); recargar la ventana
es lo más simple y robusto, sin estado a medio traducir. Alternativa
(re-render reactivo de todos los paneles) descartada por invasiva y de bajo
ROI para un cambio que el usuario hace una vez.

[Seguro] Selector «ES/EN» en la toolbar (español primero, lengua primaria);
al cambiar persiste y recarga. en.json arranca como COPIA de es.json (164
claves) — el mecanismo ya funciona; la traducción inglesa es F2. Clave
nueva `toolbar.idioma` en ambos JSON (165 claves c/u por ahora).

[Seguro] Tests (5 nuevos, 32 UI): traducir lee del activo, fallback a es si
falta la clave, clave inexistente→la clave, interpolación por idioma,
setLang/getLang persisten con stub de localStorage.

Compuerta F1: tests verdes (32 UI) · tsc limpio · suite Rust intacta (37).

## 2026-06-13 — RP16-F2: traducción inglesa de la UI y anti-drift es/en

[Seguro] en.json: las 165 claves traducidas a inglés técnico de
herramientas de desarrollo (Assemble/Step/Run/Stop/Reset, Registers/Memory/
Console/Debug, Breakpoints/Watchpoints…). Términos RISC-V intactos (ABI,
ecall, mtvec, sbrk, lr/sc, MMIO…). Interpolaciones {line}/{addr}/{path}/…
preservadas. `toolbar.idioma` queda «Idioma / Language» en ambos (rótulo
del selector, bilingüe a propósito).

[Seguro — anti-drift es/en, patrón RP-14/15] Test idiomas.test.ts:
es.json y en.json tienen EXACTAMENTE el mismo conjunto de claves (ni
faltantes ni sobrantes), cada clave preserva sus interpolaciones {…}, y
ninguna traducción quedó vacía. Si un RP futuro agrega una clave a es y
olvida en, el test rojo.

[Seguro] Auditoría de hardcodeos de UI fuera de t(): CERO. Los únicos
literales con palabras "españolas" en .ts son un id de DOM
(«pantalla-canvas») y dos nombres de comando Tauri («leer_archivo_texto»,
«guardar_archivo_texto») — no son texto de usuario. index.html: el único
texto visible es la marca «EmuRV»; los 5 títulos de panel usan data-i18n
(traducidos por t()). El placeholder «0x10010000» de Depuración es un
ejemplo de dirección, locale-neutral.

[Probable] Verificación manual pendiente: cambiar a EN recarga la ventana y
muestra la UI en inglés; volver a ES funciona; el idioma persiste al
reabrir.

Compuerta F2: anti-drift verde (35 UI) · auditoría hardcodeos = 0 · tsc
limpio · suite Rust intacta (37).

## 2026-06-13 — RP16-F3: docs internacionalización (cierre RP-16)

[Seguro] Docs: README con la característica de idioma (selector ES/EN
persistido) y la limitación explícita (diagnósticos del ensamblador y
lecciones siguen en español en modo inglés — decisión de alcance, el
español es el material de tesis, NO un olvido). Ayuda integrada con sección
«Idioma»/«Language» (claves ayuda.idioma.* en ambos catálogos, anti-drift
las cubre).

[Seguro] CIERRE RP-16 — criterios del ROADMAP:
- UI traducible es/en con selector y persistencia ✓ F1/F2
- en.json completo (166 claves), anti-drift es==en por test ✓ F2
- Cero strings de UI hardcodeados fuera de t() (auditados) ✓ F2
- Limitación de v1 (capas 2 mensajes del core y 3 lecciones en español)
  documentada como decisión, no omisión ✓ F3
- i18n ya era base (CLAUDE.md regla 6): esto fue traducción + selector, no
  reingeniería ✓
Suites verdes (37 src-tauri, 50 CLI, 35 UI), clippy 0 ambos, tsc limpio,
ci-local --skip-bundle verde, tauri build ok, bundle 22 MB ≤ 30.

ALCANCE NO HECHO (registrado como futuro CON RAZÓN): capa 2 (~98 mensajes
del ensamblador/runtime hardcodeados en Rust — externalizarlos es un
refactor grande de bajo ROI para público hispanohablante) y capa 3 (las 10
lecciones — traducirlas es rehacer el currículo). Propuestas: pasar un
locale al core / catálogo de mensajes; `lecciones-en/` paralelo el día que
haya demanda.

[PENDIENTE HUMANO] Verificación visual: cambiar ES↔EN (recarga, UI en el
idioma elegido), persistencia al reabrir, y comprobar que un error de
ensamblado en modo EN sale en español (limitación esperada).

## 2026-06-13 — RP17-F1: design tokens (refactor invisible de la estética)

[Seguro] Toda la estética TEMABLE extraída a `src/temas/tokens.css` como
variables CSS en `:root` (= tema «Educativo», default). styles.css ya NO
tiene literales de color: solo `var(--…)`. Importado en index.html ANTES
de styles.css (define las variables que styles consume). Vite los
concatena en un solo CSS bundleado (tokens presentes, verificado).

[Seguro] Tokens nuevos (capturan literales que estaban sueltos, valor
EXACTO): --texto-sobre-acento (#10151b, texto del botón principal),
--resaltado-linea (rgba acento .18, línea en ejecución), --resaltado-cambio
(rgba cambio .16, registro modificado), --fila-par (zebra), --overlay-fondo
(backdrop de modales, 2 usos), --sombra-modal, --fondo-pantalla (#000 del
framebuffer). Los color-mix con var(--acento) (14/22/12%) se dejan: son
derivados que ya SIGUEN al acento, ideal para temas.

[Seguro — alcance del refactor] Tokenizados TODOS los colores/sombras (la
dimensión que un tema varía). La geometría (paddings, border-radius en px)
queda como literal: no varía entre Educativo y Retro y tokenizar cada px
sería desproporcionado y arriesgaría la invisibilidad. styles.css quedó sin
literales de COLOR (grep verificado: solo color-mix con var).

[Seguro — refactor INVISIBLE] Los tokens toman los valores actuales exactos
→ apariencia idéntica. pantalla.html (ventana de pantalla aparte) tiene su
propio <style> inline, no usa tokens ni styles.css: no afectado. tsc/vite
build limpios; suite Rust intacta (37); UI 35; bundle 22 MB ≤ 30.

Compuerta F1: estética 100% por tokens · refactor invisible (valores
copiados exactos) · tsc/build limpios · Rust intacto · bundle OK.

## 2026-06-13 — RP17-F2: fuente monoespaciada self-hosted e iconos SVG

[Seguro — licencia registrada] Fuente: **JetBrains Mono Regular**, licencia
**SIL Open Font License 1.1** (texto en src/assets/fonts/OFL.txt). Self-
hosted (src/assets/fonts/JetBrainsMono-Regular.woff2, 92 KB) vía @font-face
en tokens.css; token `--mono` = "JetBrains Mono" con fallback al mono del
sistema (SF Mono/ui-monospace) si no cargara. Solo Regular: las pocas
celdas mono en negrita usan síntesis del navegador (aceptable, ahorra peso).
Editor, registros, memoria y consola ya la usaban vía --mono → la toman
sola.

[Seguro — licencia registrada] Iconos: **Lucide** (lucide.dev), licencia
**ISC**. Copiados como path data en src/iconos.ts — CERO dependencia npm.
SVG inline en los botones de la toolbar (ensamblar=package, ejecutar=play,
paso=chevron, detener=square, reiniciar=rotate-ccw, lecciones=graduation-
cap, video=monitor, abrir=folder-open, guardar=save, exportar=download,
ayuda=circle-help). Decorativos (aria-hidden); el botón conserva su TEXTO
(etiqueta accesible) y se le añadió aria-label explícito. Un id sin icono
cae a solo-texto sin romper.

[Seguro] Bundle RE-MEDIDO con la fuente + iconos: 22 MB ≤ 30 (la fuente
92 KB y los SVG inline no mueven la aguja). vite build incluye la woff2
hasheada en dist; tsc/UI/Rust verdes.

[PENDIENTE HUMANO] Verificación visual: la tipografía mono cambió a
JetBrains Mono en editor/registros/memoria/consola; los botones muestran
icono + texto alineados.

Compuerta F2: licencias registradas (OFL/ISC) · bundle re-medido 22 MB ·
tsc/build limpios · suite Rust intacta (37) · UI 35.

## 2026-06-13 — RP17-F3: mecanismo de temas con Educativo y Retro-CRT

[Seguro] Tema = valor de `data-tema` en <html> que selecciona un bloque de
tokens. «Educativo» (default) = :root, sin atributo; «Retro-CRT» =
`[data-tema="retro"]` sobrescribe SOLO variables (cero CSS duplicado).
`src/temas/index.ts`: esTema/leerTema/aplicarTema/setTema/inicializarTema,
persistencia localStorage `emurv.tema.v1` (patrón RP-15/16). Cambio EN VIVO
sin recargar (las variables CSS aplican al instante — ventaja sobre el
idioma, que sí recarga). Selector en la toolbar junto al de idioma
(Educativo primero); inicializarTema() aplica el guardado al arrancar.

[Seguro] Retro-CRT: fósforo ÁMBAR sobre negro cálido (identidad del
dominio, nace del framebuffer — no necesitó inspiración externa) +
scanlines sutiles (capa fija decorativa `body::after`, solo con el tema
activo, pointer-events:none). i18n: tema.educativo/tema.retro/toolbar.tema
en ambos catálogos (anti-drift es/en sigue verde, 170 claves c/u).

[Seguro — LEGIBILIDAD, regla innegociable] Test contraste.test.ts calcula
el ratio WCAG de 11 pares críticos (texto/fondos, texto-suave, error,
acento, ok, cambio) de CADA tema leyendo los hex de tokens.css (fuente
única, sin duplicar). Umbral AA = 4.5:1. Resultados: Educativo mín 4.99,
Retro-CRT mín 7.19 — ambos pasan con holgura. Si un tema futuro no llega,
el test rojo (se corrige el tema, no se baja la vara).

[Seguro] Tests (7 nuevos, 42 UI): mecanismo (esTema/leer/aplicar/set/
default/basura) + contraste AA de los 2 temas.

[PENDIENTE HUMANO] Verificación visual: cambiar Educativo↔Retro-CRT en
vivo (sin recargar), ver el ámbar + scanlines, y que persiste al reabrir.

Compuerta F3: tests verdes (42 UI) · contraste AA verificado por test en
ambos temas · tsc/build limpios · bundle 22 MB ≤ 30.

## 2026-06-13 — RP17-F4: docs sistema de temas (cierre Parte A)

[Seguro] Docs: README con la característica de temas (selector, cambio en
vivo, contraste AA, fuente JetBrains Mono OFL); Ayuda integrada con sección
«Temas»/«Themes» (ayuda.temas.*, anti-drift es/en cubre, 172 claves c/u);
nota en tokens.css «CÓMO AGREGAR UN TEMA» para la Parte B (un tema = un
bloque [data-tema] + entrada en TEMAS + traducción + el selector del test
de contraste; sin tocar styles.css ni lógica).

[Seguro] CIERRE RP-17 PARTE A — criterios:
- Estética 100% por tokens; refactor invisible verificado ✓ F1
- Fuente mono self-hosted (JetBrains Mono, OFL) + iconos SVG (Lucide, ISC),
  licencias registradas, bundle OK ✓ F2
- Mecanismo de temas con 2 built-in (Educativo default + Retro-CRT), cambio
  EN VIVO sin recargar, persistencia localStorage ✓ F3
- Legibilidad AA en ambos temas (test de contraste, mín 4.99 / 7.19) ✓ F3
- Agregar un tema = un bloque de tokens, documentado para Parte B ✓ F4
- Sin frameworks, CSS puro ✓
Suites verdes (37 src-tauri, 50 CLI, 42 UI), clippy 0 ambos, tsc limpio,
ci-local --skip-bundle verde, tauri build ok, bundle 22 MB ≤ 30.

PARTE B (NO en este plan): temas experimentales del diseño con Claude
Desktop + tema de usuario; se montan sobre este mecanismo (bloques de
tokens), sin reingeniería.

[PENDIENTE HUMANO] Verificación visual del conjunto: tipografía JetBrains
Mono, iconos en la toolbar, cambio Educativo↔Retro-CRT en vivo con
scanlines, persistencia al reabrir.

## 2026-06-13 — RP17B-F1: temas Pergamino, Retro Kawaii y Disquete 90s

[Seguro] Tres temas nuevos del diseño aprobado (claude.ai →
docs/diseno-ui/propuestas-ui.html) como bloques de tokens en tokens.css:
Pergamino (papel cálido claro), Retro Kawaii (vaporwave pastel) y Disquete
90s (escritorio beige). Cero CSS duplicado: cada tema solo sobrescribe
variables; lo demás hereda de :root/«educativo». TEMAS pasa a 5 entradas
(orden = selector, educativo primero); el selector y los nombres i18n se
derivan solos de TEMAS (no hubo que tocar main.ts). esTema valida contra
TEMAS. i18n: tema.pergamino/kawaii/disquete en es Y en (anti-drift verde).

[Seguro — LEGIBILIDAD, regla innegociable, «no bajar la vara»] Los valores
EXACTOS del diseño NO cumplían AA 4.5:1 en varios pares sobre sus fondos
claros — bastante más que los dos caveats que el plan anticipaba. Medición
con el algoritmo WCAG de contraste.test.ts ANTES de escribir:
- Pergamino: ok/fondo 4.35.
- Kawaii: ok/fondo 2.83, error/fondo 3.73, acento/fondo 4.38, cambio 3.52.
- Disquete: acento/fondo 3.10, ok/fondo 2.96, texto-suave/fondo 3.54.
- Sintaxis (--sx-* sobre --fondo, que es el fondo del editor): casi toda
  la de Kawaii y Disquete por debajo de 4.5 (Disquete sx-com 2.31).
Decisión (autorizada por el plan: «corregí el token, no bajes la vara»):
oscurecer cada token que raspa DENTRO de su mismo tono (escala RGB hacia
negro por búsqueda binaria) hasta ≥4.6 (margen). Cada valor corregido
queda con el valor del diseño en comentario en tokens.css. NO se tocó la
estructura del diseño (eso es RP-19); solo el valor de los tokens que la
legibilidad exigía. Tokens corregidos:
- Pergamino: ok #2e7d32→#2c7830; sx-com/dir/str.
- Kawaii: acento #c0327d→#ba3079, ok #1f9e6e→#177753, error #d83a5e→
  #be3353, cambio #b97400→#9e6300; sx-com/dir/str/num. resaltado-* rgba
  realineados al acento/cambio corregidos.
- Disquete: texto-suave #6a6047→#584f3b, acento #b14a14→#86380f, ok
  #3c7a3f→#2c592e, error #b23022→#95281d, cambio #9a6b00→#855c00; los 6
  --sx-* (beige #cdc3a8 es un «claro oscuro», la sintaxis necesitaba el
  mayor ajuste). resaltado-* rgba realineados.
Mínimos resultantes por tema (texto + sintaxis): Educativo 4.99, Retro
7.19, Pergamino/Kawaii/Disquete 4.60. Los 5 verdes.

[Seguro] --sx-* (sintaxis del editor) se agregaron a LOS 5 temas, incluido
:root (valores del diseño, sx-com = mirror de --texto-suave como hex para
que el test lo lea) y Retro-CRT (derivados ámbar coherentes: casi monocromo,
la diferencia es brillo no tono; AA holgado ≥7 sobre negro cálido). El test
de contraste ahora cubre, por tema, los 11 pares de UI MÁS los 6 --sx-*
sobre --fondo: la legibilidad del código es tan innegociable como la de la
UI. Consumidos por el theme de CodeMirror en F2.

[Seguro] Tests UI 50 (antes 42): +5 «sintaxis del editor» (uno por tema) +
los pares críticos de los 3 temas nuevos. tsc estricto limpio, vite build
ok. Anti-drift es/en verde.

[PENDIENTE HUMANO] Verificación visual de los 5 temas en vivo: que el
selector muestre los 5, que cada uno se vea como el diseño (papel/pastel/
beige) y que el contraste se sienta cómodo, no solo que pase el número.

Compuerta F1: contraste AA verde en los 5 temas (texto + sintaxis) ·
selector deriva 5 de TEMAS · anti-drift i18n verde · tsc/build limpios.

## 2026-06-13 — RP17B-F2: theme de sintaxis del editor por tokens

[Seguro] Problema: CodeMirror usaba oneDark FIJO, así que el código del
editor NO cambiaba de color con el tema (el resto de la UI sí, por las CSS
vars). Objetivo: que la sintaxis siga los --sx-* del tema activo, en vivo.

[Seguro — punto técnico del plan] CodeMirror NO lee CSS vars de forma
nativa en un HighlightStyle (los colores se resuelven al construir el
estilo, no por elemento). Enfoque elegido y verificado: un TagStyle admite
la propiedad `class` (en @codemirror/language toma precedencia sobre las
props de color: `class: style.class || …`). Entonces el HighlightStyle
emite CLASES (.cm-sx-com/dir/lbl/ins/str/num) en vez de colores, y esas
clases en styles.css pintan con `color: var(--sx-*)`. Cambiar de tema
recolorea los spans sin recrear el editor — la cascada CSS aplica al
instante. NO hizo falta mini-spike: el mapeo tag→clase salió directo.

[Seguro] Mapeo tag→clase (los tokens del StreamLanguage de riscv.ts pasan
por createTokenType → tags.<nombre>): comment→.cm-sx-com, meta(directiva)→
.cm-sx-dir, labelName→.cm-sx-lbl, keyword(mnemónico)→.cm-sx-ins, string→
.cm-sx-str, number→.cm-sx-num. Registros (variableName.special) y nombres
sueltos quedan con el color de texto por defecto — igual que el diseño, que
tampoco los colorea. `src/editor/tema.ts` exporta `temaEditor` = chrome +
syntaxHighlighting; index.ts lo monta donde estaba oneDark.

[Seguro] El chrome del editor (fondo, cursor, selección, gutter, línea
activa) pasó de oneDark a un EditorView.theme cuyo color sale TODO de
tokens de la app (var(--fondo), var(--acento), var(--fondo-panel),
var(--fila-par)…). El fondo del editor es --fondo, que es justo el fondo
contra el que F1 verificó los --sx-*. La línea en ejecución
(.cm-linea-actual, !important) y los breakpoints (.cm-breakpoint-*) se
estilan en styles.css y mandan sobre el chrome: intactos.

[Seguro] Test anti-drift (src/editor/tema.test.ts, 3): las 6 clases que
declara el HighlightStyle existen como regla `color:var(--sx-*)` en
styles.css (un typo de cualquier lado dejaría la sintaxis sin color sin
romper el build — el test lo caza), y index.ts ya no referencia oneDark.
No se testea el recoloreo en sí (necesita DOM/navegador) → verificación
humana. Dependencia @codemirror/theme-one-dark queda declarada pero sin
importar (no entra al bundle); se quitará si se hace limpieza de deps.

[Seguro] Suites: UI 53 (antes 50: +3 anti-drift del theme). Rust intacto
(F2 es solo frontend): src-tauri y emurv-cli verdes. tsc estricto y vite
build limpios.

[PENDIENTE HUMANO] Verificación visual del recoloreo: cambiar entre los 5
temas y confirmar que el CÓDIGO del editor recolorea en vivo (comentarios/
directivas/etiquetas/instrucciones/cadenas/números), que la línea en
ejecución, los breakpoints y los números de línea siguen bien, y que la
sintaxis se lee cómoda en cada tema (sobre todo Disquete, el más oscuro de
los claros).

Compuerta F2: highlight por clases siguiendo --sx-* · chrome por tokens ·
exec-line/breakpoints/gutter intactos · anti-drift de clases verde · tsc/
build limpios · Rust intacto.

## 2026-06-13 — RP17B-F3: docs temas nuevos (cierre Parte B)

[Seguro] Docs: README pasa a listar los 5 temas y aclara que el editor
recolorea con el tema (sigue los tokens --sx-*) y que el contraste AA cubre
texto Y sintaxis. Ayuda integrada (ayuda.temas.texto, es y en) lista los 5
con su descripción y menciona el recoloreo del editor (RP-19/RP-20 rehará
el modal de ayuda; acá solo el dato, anti-drift es/en verde). Nota «CÓMO
AGREGAR UN TEMA» en tokens.css ampliada: un tema puede definir sus --sx-*
(o heredarlos), el test cubre AA de UI + sintaxis sobre --fondo, y se
corrige el token (no el umbral) si raspa. ROADMAP-PRO y Cap. VI: NO se
tocaron (los gestiona otra sesión).

[Seguro] CIERRE RP-17 PARTE B — criterios:
- 3 temas nuevos (Pergamino, Retro Kawaii, Disquete 90s) seleccionables,
  persistidos, contraste AA ✓ F1
- Retro-CRT y Educativo intactos: 5 temas en total ✓ F1
- El editor recolorea con el tema (syntax highlight por --sx-*, por clases
  no por color fijo) ✓ F2
- i18n es/en de los nombres + ayuda (anti-drift verde) ✓ F1/F3
- Legibilidad AA innegociable en los 5 temas, texto Y sintaxis: valores del
  diseño corregidos dentro de tono donde no llegaban (mín 4.60) ✓ F1
- NO se tocó la estética Cálido / radio / IBM Plex / acento violeta del
  Educativo / tema de usuario — todo eso es RP-19+ ✓
Suites verdes (src-tauri y emurv-cli: cargo test + clippy -D warnings 0
ambos · UI 53 con node:test), tsc estricto limpio, ci-local.sh completo
verde, tauri build ok, bundle EmuRV.app 22 MB ≤ 30 (RNF-04). La fuente IBM
Plex es de RP-19, no entra acá.

[PENDIENTE HUMANO] Verificación visual de cierre: recorrer los 5 temas en
vivo (selector), ver que cada uno se vea como el diseño y que el código del
editor recolorea, con la línea en ejecución/breakpoints/números de línea
correctos; confirmar que la persistencia aguanta al reabrir.

## 2026-06-13 — RP18-F1: fix responsive de la toolbar (flex-wrap)

[Seguro] BUG (UI-BRIEF «Problemas de layout conocidos»): `#toolbar` era
`display:flex` SIN `flex-wrap`, así que en ventana angosta (no maximizada)
los controles de la derecha (idioma, tema, ayuda) se cortaban/tapaban.
Fix mínimo y aislado: `flex-wrap: wrap; row-gap: 8px;` → los controles que
no entran bajan a una 2ª fila y siguen usables a cualquier ancho.
Maximizada se ve igual que antes (una sola fila). NO toca ids ni JS; el
menú overflow «⋯» con ResizeObserver queda como mejora futura (el plan lo
deja fuera; flex-wrap es el fix por defecto). Antes/después: angosta antes
= controles clipeados; después = 2ª fila visible.

[Seguro] tsc estricto y vite build limpios. Sin tocar Rust (CSS puro) →
suite intacta. Estética por tokens, sin literales nuevos.

[PENDIENTE HUMANO] Verificación visual a varios anchos (incl. bien
angosto): ningún control se tapa; maximizada idéntica.

Compuerta F1: bug resuelto (flex-wrap) · tsc/build limpios · sin ids/JS
tocados · Rust intacto.

## 2026-06-13 — RP18-F2: lateral flexible, canvas acotado y registros compactos

[Seguro] Raíz del aplastamiento de Registros: `#lateral` era un grid de
SOLO 2 pistas (`1.2fr 1fr`) para 4 paneles. Ahora `display:flex;
flex-direction:column` con prioridades por panel (del diseño aprobado):
video `flex:0 0 auto`, registros `flex:2.2 1 0; min-height:160px`
(prioritario, es lo que las lecciones piden mirar), memoria
`flex:1.5 1 0; min-height:120px`, depuración `flex:0 1 auto;
max-height:200px`. Flex maneja solo `#panel-video[hidden]` (no reserva
espacio). Se reescribió SOLO `#lateral` (no `grid-template-areas` de `#app`,
prohibido).

[Seguro — interacción que el plan no detalló, la cacé al revisar] Con
`#lateral` en flex, un panel plegado con `flex:2.2`/`1.5` + `min-height`
seguiría reservando su espacio → hueco vacío al colapsar. Regla añadida
`.panel.colapsado { flex:0 0 auto !important; min-height:0 !important }`
(el diseño la tiene como `[data-colapsado]`; uso la clase `.colapsado` que
ya existe, equivalente). `!important` justificado: gana a las reglas por id
`#panel-registros{flex;min-height}`. Sin esto F3 (plegado) rompería el
layout.

[Seguro] Canvas acotado: `#video-canvas` pasa de `width:100%` a
`width:auto; height:auto; max-height:170px; margin:8px auto` (centrado);
la resolución interna 320×240 (atributos width/height del elemento) NO se
toca, solo el tamaño de presentación. `aspect-ratio:4/3` mantiene la
proporción al limitar el alto. La ventana desacoplada `pantalla.html` tiene
su PROPIO style → no se afecta.

[Seguro] Registros compactos: `.registros-tabla { table-layout:fixed }`
(global, como el diseño) + `<colgroup>` con anchos (nombre 30 / hex 38 /
dec 32 %). DESVIACIÓN deliberada del diseño (autorizada por «cero
regresión»): el diseño solo dio colgroup a la tabla principal, pero las
tablas de CSRs y F comparten la clase `.registros-tabla`; con `fixed`
global y sin colgroup, la columna «valor» de la tabla F (flotantes largos,
`white-space:nowrap`) se cortaría. Solución: colgroup también en CSRs
(mismos 30/38/32) y en F (28/30/42, «valor» más ancho). No cambia el look
del mockup, solo evita el clip.

[Seguro — regla DURA, NO reordenar] Separadores de grupo: clase `.grupo`
en la PRIMERA fila de cada bloque ABI (ra=x1, t0=x5, s0=x8, a0=x10, s2=x18,
t3=x28) con `tr.grupo td { border-top:1px solid var(--borde) }`. El ORDEN
x0..x31 (convención RISC-V, RARS/gdb) queda INTACTO — solo una línea sutil.
Tooltips de rol (`title`) en la celda del nombre de cada registro (9 roles
i18n agrupados: zero/ra/sp/gp/tp + temporal/guardado/argumento). El orden,
los grupos y la cobertura de roles se EXTRAJERON a un módulo puro
`registros-meta.ts` y los fija un test (`registros-meta.test.ts`): si
alguien reordena, el test rojo.

[Seguro] Tests UI 56 (antes 53: +3 de registros-meta). i18n: 9 claves
`registros.rol.*` en es Y en (anti-drift verde). tsc estricto y vite build
limpios. Sin tocar ids del JS ni Rust (F2 es frontend) → suite intacta;
clippy y bundle se verifican en el cierre F4 (delta de frontend = pocos KB
sobre los 22 MB, lejísimos de 30).

[PENDIENTE HUMANO] Verificación visual: Registros legible sin hueco y con
los separadores en su sitio; la pantalla no domina el lateral; al editar un
valor la tabla no «salta».

Compuerta F2: lateral flex con Registros prioritario · canvas acotado ·
registros compactos + separadores + tooltips · orden x0..x31 fijado por
test · i18n anti-drift · tsc/build limpios · Rust/ids intactos.

## 2026-06-13 — RP18-F3: progressive disclosure y jerarquía de toolbar

[Seguro] Progressive disclosure (público principiante: lo esencial a la
vista, lo avanzado bajo demanda):
- #panel-memoria: plegable. main.ts `hacerPanelPlegable("panel-memoria")`
  pone `.panel-titulo-clickeable` en el <h2> y togglea `.colapsado` al
  click. ABIERTO por defecto (memoria es contextual, no avanzada).
- #panel-depuracion: arranca PLEGADO (`class="panel colapsado"` en
  index.html) y se expande SOLO al poner el primer breakpoint
  (`alCambiarBreakpoints` quita `.colapsado` si hay ≥1 breakpoint) — ahí
  empieza a importar. El click manual sigue alternando (depuracion.ts ya lo
  cableaba). Título con nota «avanzado» a la derecha (span aparte del
  data-i18n, porque traducirTitulos pisa el textContent del elemento con
  data-i18n; van dos spans hermanos en el <h2>).
- Watchpoints: pasaron de `<div class="depuracion-seccion">` a un
  `<details class="depuracion-watchpoints">` plegado, con mini-ayuda
  «¿Qué es esto?…» (i18n, texto exacto del diseño) visible al desplegar.
  Los breakpoints quedan directos a la vista; los watchpoints (más
  abstractos) bajo demanda. La cajita usa `color-mix(... var(--acento) 10%
  ...)` (ya usado en styles.css, sin token nuevo).

[Seguro — interacción con F2] El `.panel.colapsado { flex:0 0 auto;
min-height:0 }` de F2 es lo que hace que un panel plegado en el lateral
flex ENCOJA al título en vez de dejar hueco. Sin F2, el plegado de F3 se
vería mal.

[Seguro] Jerarquía de toolbar:
- De-emphasis: clase `.deemph` (opacity .78; hover→1) en los secundarios
  (Lecciones, Pantalla, Abrir, Guardar, Exportar, Ayuda). Ensamblar
  (`.principal`) sigue siendo el ÚNICO con acento. El fondo transparente
  cálido del `.deemph` es RP-19; acá solo la opacidad.
- `grupo-ejec`: Ejecutar/Paso/Detener/Reiniciar envueltos en un
  `<div class="grupo-ejec">` (segmentado: bordes compartidos, esquinas
  redondeadas solo afuera). El helper `boton()` ganó un parámetro `parent`
  opcional para meterlos en el contenedor sin tocar nada más; los ids de
  los botones (contrato con el JS) intactos. DESVIACIÓN del diseño: usa
  `border-radius:6px` en vez de `var(--radio)` — `--radio` es un token de
  RP-19 que todavía no existe; 6px es el valor que ya usa toda la app y
  RP-19 lo tokenizará en su barrido. El plan autoriza dejar el pulido de
  radios/hover para RP-19 (acá solo estructura + segmentado).

[Seguro] i18n: +2 claves en es Y en (panel.depuracion.nota,
depuracion.watchpointsAyuda; anti-drift verde). Las funciones de plegado
NO son puras (togglean clases del DOM) → sin test unitario, verificación
manual (el plan lo contempla: «testeadas si son puras»). tsc estricto y
vite build limpios. Sin renombrar ids; Rust intacto (frontend).

[PENDIENTE HUMANO] Verificación del flujo: Depuración plegada al abrir la
app y que se expande sola al poner un breakpoint; Memoria se pliega/despliega
al click; Watchpoints despliega su mini-ayuda; solo Ensamblar resalta, el
resto atenuado; los 4 botones de ejecución se ven como UN bloque.

Compuerta F3: progressive disclosure (Memoria plegable · Depuración plegada
y auto-revelada · watchpoints con ayuda) · jerarquía (deemph + grupo-ejec)
· i18n anti-drift · tsc/build limpios · ids/Rust intactos.

## 2026-06-13 — RP18-F4: docs ajustes de layout y UX (cierre)

[Seguro] Docs de cara al usuario (flujos que cambiaron): README suma la
divulgación progresiva de los paneles (Registros prioritario y siempre
visible; Memoria plegable; Depuración plegada hasta el primer breakpoint;
watchpoints con mini-ayuda) y la toolbar adaptable a cualquier ancho, más
la nota de orden RISC-V + separadores + tooltips en registros. Ayuda
integrada: `ayuda.depuracion.texto` (es y en) actualizada — el panel
arranca plegado y se abre solo al primer breakpoint, watchpoints en
sub-sección con ayuda. Anti-drift es/en verde. ROADMAP-PRO y Cap. VI: NO
tocados.

[Seguro] CIERRE RP-18 — criterios de aceptación:
- Toolbar usable a cualquier ancho (bug flex-wrap resuelto) ✓ F1
- Registros legible sin aplastar, con separadores y tooltips; orden x0..x31
  INTACTO (fijado por test) ✓ F2
- Pantalla no domina el lateral (canvas acotado) ✓ F2
- Progressive disclosure: Depuración plegada por defecto + auto-revelada,
  Memoria plegable, watchpoints con ayuda ✓ F3
- Jerarquía: solo Ensamblar con acento, secundarios atenuados, ejecución
  segmentada ✓ F3
- Cero regresión funcional: ids del JS intactos, `grid-template-areas` de
  `#app` sin tocar, suites verdes; legibilidad AA (los 5 temas de RP-17B
  siguen verdes en contraste.test) y bundle ≤30 ✓ todas
- i18n es/en de los textos nuevos (anti-drift verde) ✓ F2/F3
Suites verdes (src-tauri y emurv-cli: cargo test + clippy -D warnings 0
ambos · UI 56 con node:test), tsc estricto limpio, ci-local.sh completo
verde, tauri build ok, bundle EmuRV.app 22 MB ≤ 30 (RNF-04).

[Seguro] DEJADO COMO FUTURO (fuera del alcance de este plan, registrado):
- Ensanche de la consola cuando el video está oculto: requiere reescribir
  `grid-template-areas` de `#app` (más invasivo) — el plan lo excluye.
- Menú overflow «⋯» con ResizeObserver para la toolbar: mejora sobre el
  fix por defecto (flex-wrap); no hacía falta.
- Pulido cálido (radios via --radio, sombras, IBM Plex, acento violeta del
  Educativo, hover/fondo transparente del .deemph, chevrons en los títulos
  plegables): es RP-19. El `border-radius:6px` del grupo-ejec es provisorio
  hasta que RP-19 tokenice los radios.

[PENDIENTE HUMANO] Verificación visual final a varios anchos de ventana:
toolbar que envuelve sin cortar; lateral con Registros prioritario y sin
hueco al plegar; pantalla acotada; Depuración que se revela al breakpoint;
jerarquía (solo Ensamblar resalta). Recomendado en los 5 temas de RP-17B.

## 2026-06-13 — RP19-F1: estética Cálido (tokens, IBM Plex, paneles-tarjeta)

[Seguro — decisión del autor] «Cálido» es el ÚNICO look de la app (no hay
toggle de estética). Por eso todo lo que en el mock (propuestas-ui.html)
está bajo `[data-estetica="calido"]` se aplica SIEMPRE: se trasladó a
`:root` (tokens) y a selectores SIN el prefijo de estética (styles.css).
Fidelidad: valores, espaciados y tinte copiados del mock al pie de la letra.

[Seguro] Tokens nuevos en :root (valores exactos del mock): `--radio:9px`,
`--radio-panel:14px`, `--fuente-ui:"IBM Plex Sans", system-ui…`,
`--sombra-panel:0 1px 2px rgba(0,0,0,.16), 0 6px 18px rgba(0,0,0,.10)`.
Viven en :root → todos los temas los heredan.

[Seguro — LEGIBILIDAD, riesgo marcado del plan] Acento VIOLETA del Educativo
(reemplaza el azul #4ea1ff): `--acento:#9b8cff; --texto-sobre-acento:#16122b;
--resaltado-linea:rgba(155,140,255,.20); --sx-lbl:#a99bff; --sx-ins:#c4b6ff`.
Verificado AA con el algoritmo de contraste.test ANTES de aplicar:
acento/fondo 5.78, acento/fondo-panel 5.36, sx-lbl 6.72, sx-ins 8.74,
texto-sobre-acento sobre acento 6.57 — todos ≥4.5, NO hubo que ajustar. Los
otros 4 temas no cambian su acento. contraste.test sigue verde para los 5.

[Seguro] IBM Plex Sans self-hosted (MISMO patrón que JetBrains Mono de
RP-17A): `@font-face` en tokens.css, Regular 400 + SemiBold 600, subconjunto
latino (fontsource), `src/assets/fonts/IBMPlexSans-{Regular,SemiBold}.woff2`
(~47 KB total). Licencia SIL OFL 1.1 (© 2017 IBM Corp.) en
`IBMPlexSans-OFL.txt`. Es la `--fuente-ui`: la usa el CHROME de la UI
(toolbar, cabeceras de panel/tabla, estado, depuracion-vacio, wp-ayuda,
consola-entrada — lista del mock); el CÓDIGO y los DATOS siguen en --mono.

[Seguro] Lenguaje visual Cálido (todo del mock, aplicado siempre): #app con
`gap:10px; padding:10px` (lienzo aireado); #editor/#toolbar/#estado y .panel
como TARJETAS (borde + `border-radius:var(--radio-panel)`; .panel además con
`box-shadow:var(--sombra-panel)`); #lateral `gap:10px`; cabeceras de panel y
de tabla con tinte `color-mix(var(--acento) 9%, var(--fondo-panel))`;
densidad aireada (registros td 4×12, th 7×12, botones toolbar 7×13,
consola-salida 10×14, código con line-height 1.65 + padding 10px 0);
jerarquía de toolbar (principal con sombra de acento; .deemph «fantasma»
fondo/borde transparentes que el hover revela). Decoraciones por tema del
mock (Disquete: barra de título en color; Kawaii: tinte rosa más marcado).

[Seguro — adaptación mock→CodeMirror] El mock dibuja un editor falso
(`.cm .ln`); el real es CodeMirror. `.cm { line-height; padding }` se tradujo
a `#editor .cm-content { padding }` + `#editor .cm-line { line-height }`.
(El hover de breakpoint y la línea activa se adaptan en F2.) El `.secundario`
del mock es el `.panel-nota` real; no hay `.chev`.

[Seguro] Tests UI 56 (sin cambios; el contraste de los 5 temas sigue verde,
incluido el violeta). tsc estricto y vite build limpios. Bundle re-medido
con la fuente nueva: EmuRV.app 22 MB ≤ 30 (IBM Plex suma ~47 KB,
despreciable). Sin tocar Rust ni ids del JS.

[PENDIENTE HUMANO] Verificación visual contra el mock: tarjetas con aire,
IBM Plex en la UI, violeta del Educativo, headers tintados, densidad — en
los 5 temas.

Compuerta F1: se ve como el mock (visual) · contraste AA 5 temas verde
(violeta incluido) · IBM Plex self-hosted (OFL) · bundle 22 MB ≤30 ·
tsc/build limpios · Rust/ids intactos.

## 2026-06-13 — RP19-F2: microinteracciones y grupo-ejec tokenizado

[Seguro] grupo-ejec tokenizado: `border-radius:6px` (literal que RP18 dejó
para acá) → `var(--radio)`. Cierra la deuda de RP18-F3.

[Seguro] Microinteracciones (valores EXACTOS del mock):
- Transiciones baratas (solo color/borde/sombra, sin reflow ni filtros) en
  button/select/input/summary, .12s ease.
- `:focus-visible { outline:2px solid var(--acento); outline-offset:2px }`
  en botones/selects/inputs/summary (+ border-color acento en inputs). El
  anillo usa el acento del TEMA activo → coherente en los 5 (violeta en
  Educativo, ámbar en Retro, etc.).
- Íconos unificados: `.ico { stroke-width:1.8; …round }`,
  `.ico:not(.ico-fill){ fill:none; stroke:currentColor }`,
  `.ico-fill{ fill:currentColor; stroke:none }`. El play (Ejecutar) pasa a
  relleno por convención: iconos.ts le pone clase `ico ico-fill`.
- Destello one-shot de registros: `@keyframes emurv-cambio-flash` (de
  `--cambio 50%` al fondo persistente) en `tr.cambiado td`, 0.7s, UNA vez —
  NO bucle. registros.ts ya togglea .cambiado por paso.

[Seguro — adaptaciones mock→app, fidelidad con criterio]:
- Caret de consola: el mock dibuja un caret falso parpadeante (`.ce-caret`,
  keyframe infinito) porque su consola es estática. La app real tiene un
  `<input>` con caret NATIVO → en vez de un span falso (bucle innecesario),
  se tiñe el caret real con `caret-color:var(--acento)` y el prompt `›` va
  en acento+negrita (`.consola-prompt`, = `.ce-prompt` del mock). Resultado:
  CERO animaciones en bucle (la directriz «sin bucles que distraigan» se
  cumple incluso mejor que el mock); el flash es la única animación y es
  one-shot.
- Hover de breakpoint: el mock usa `.ln:not(.bp):hover .num::after` sobre su
  editor falso. Traducido a la gutter real de CodeMirror:
  `.cm-breakpoint-gutter .cm-gutterElement:not(:has(.cm-breakpoint-marker)):hover::after`
  → ● tenue (opacity .3) en líneas sin breakpoint, para invitar a poner uno.
  `:has()` (soportado por el WebKit de Tauri) detecta la ausencia del
  marcador.
- Número de línea de ejecución en acento (`.ln.act .num` del mock): se
  omite — CodeMirror no expone el número de la línea de EJECUCIÓN (≠ línea
  del cursor) de forma simple; la exec-line ya se marca con la barra de
  acento (box-shadow de `.cm-linea-actual`). Registrado como omisión menor.

[Seguro] Sin lógica nueva testeable salvo el `ico-fill` (string del SVG); no
hay test de iconos y el cambio es trivial. tsc estricto y vite build
limpios, UI 56, Rust intacto (frontend).

[PENDIENTE HUMANO] Verificación visual: que las microinteracciones se sientan
sutiles (flash one-shot al cambiar un registro, foco con anillo de acento,
caret de consola en acento, ● tenue al pasar por el margen) y NO molesten.

Compuerta F2: microinteracciones one-shot/no molestas · focus-visible
coherente por tema · grupo-ejec con var(--radio) · tsc/build limpios ·
suite intacta.

## 2026-06-13 — RP19-F3: refinados de ruido visual y wordmark

[Seguro] Refinados de ruido (valores del mock, aplicados siempre):
- Cabeceras de tabla (th): de 600/posible-mayúscula a `font-weight:500;
  font-size:11.5px; text-transform:none; letter-spacing:0` — son etiquetas,
  no títulos a gritos.
- Número crudo xN: `opacity:.38; font-size:10.5px` (era .5/11px) — dato
  secundario, más tenue.
- Volcado de memoria: bytes en cero atenuados (`.cero { color:var(--texto-suave);
  opacity:.45 }`). Para marcarlos, memoria.ts pasó de un `textContent` por
  palabra a un span por byte (clase `.cero` en los 00). La edición inline
  sigue intacta: lee `dataset.addr/valor` y `textContent` (que concatena los
  spans → mismo "00 1a 2f 00"); el dblclick es delegado con
  `closest('.memoria-palabra')`, así que el target sub-span resuelve al padre.
- Sin cebra: `tr:nth-child(even){ background:transparent }` + hover
  `tbody tr:hover td { background: color-mix(var(--acento) 7%, transparent) }`
  — la tarjeta y el hover ya separan, menos líneas compitiendo.
- Separadores de grupo más suaves: `color-mix(var(--borde) 50%, transparent)`
  (insinúan, no cortan). El ORDEN x0..x31 sigue intacto.

[Seguro] Wordmark de marca en la toolbar (al inicio, antes del primer
separador). El SVG del diseño (`docs/diseno-ui/assets/emurv-wordmark.svg`)
traía dos verdes fijos; se adaptó programáticamente (sed) a
`src/assets/emurv-wordmark.svg` con clases en vez de fills: el path «Emu» →
`.logo-emu`, el path «RV» → `.logo-rv`, y `class="logo-emurv"` en el <svg>
(sin width/height, el alto lo fija el CSS). Se importa con `?raw` y se
inyecta inline (necesario para que el CSS pinte los paths). CSS:
`.logo-emu { fill:var(--acento) }`, `.logo-rv { fill:var(--texto) }` → el
wordmark RECOLOREA con el tema (violeta en Educativo, ámbar en Retro, etc.).
El bundle no emite un asset SVG aparte (queda inline). RP-20 reusará el mismo
SVG en el «Acerca de».

[Seguro] tsc estricto (el import `?raw` está tipado por vite/client) y vite
build limpios. UI 56. Contraste AA de los 5 temas sigue verde (los refinados
son opacidad/transparencia/tipografía, no tocan los pares del test). Sin
tocar Rust ni ids del JS.

[PENDIENTE HUMANO] Verificación visual: ruido reducido como el mock (th sin
gritos, xN tenue, ceros de memoria apagados, sin cebra) y el wordmark con su
bi-tono adaptándose al cambiar de tema.

Compuerta F3: wordmark se adapta al tema (bi-tono por tokens) · ruido
reducido como el mock · contraste AA intacto · tsc/build limpios.

## 2026-06-13 — RP19-F4: docs estética Cálido (cierre)

[Seguro] Docs: README estrena un punto «Estética Cálido» (paneles-tarjeta,
aire, IBM Plex Sans de UI, acento violeta, wordmark bi-tono que recolorea,
microinteracciones sutiles) y el punto «Temas» menciona el wordmark + el
acento violeta del Educativo. La Ayuda integrada NO se tocó: la estética es
GLOBAL (no hay toggle ni setting que explicar), así que documentarla en un
modal de ayuda sería ruido para el usuario; vive en el README. ROADMAP-PRO y
Cap. VI: NO tocados.

[Seguro] CIERRE RP-19 — criterios de aceptación:
- La app se ve como el mock: estética Cálido aplicada SIEMPRE (sin toggle),
  trasladando lo que el mock tenía bajo [data-estetica="calido"] a :root y a
  selectores sin prefijo ✓ F1-F3
- IBM Plex Sans self-hosted (Regular+SemiBold, OFL 1.1 © IBM, licencia en
  src/assets/fonts/IBMPlexSans-OFL.txt); bundle 22 MB ≤30 (suma ~47 KB) ✓ F1
- Educativo con acento violeta #9b8cff cumpliendo AA (5.78/5.36, sin ajuste);
  los 5 temas siguen verdes en contraste.test ✓ F1
- Microinteracciones del mock: foco con anillo de acento, destello one-shot
  de registros, caret de consola en acento, hover de breakpoint ✓ F2
- grupo-ejec tokenizado (var(--radio)); refinados de ruido (th, xN, ceros de
  memoria, sin cebra, separadores suaves); wordmark adaptable al tema ✓ F2-F3
- 5 temas siguen AA; ids del JS / orden de registros / Rust intactos ✓ todas
- Modales / ayuda 2-col / Acerca de: NO acá (RP-20) ✓
Suites verdes (src-tauri y emurv-cli: cargo test + clippy -D warnings 0
ambos · UI 56 con node:test), tsc estricto limpio, ci-local.sh completo
verde, tauri build ok, bundle EmuRV.app 22 MB ≤ 30 (RNF-04, con IBM Plex).

[Seguro] Las 3 ÚNICAS desviaciones permitidas del mock, registradas:
1. Contraste AA: el violeta del Educativo se VERIFICÓ (pasó sin ajuste); no
   hubo que mover ningún token.
2. ids del JS: intactos (el wordmark es un <span> nuevo sin id contratado).
3. Orden de registros: intacto (fijado por registros-meta.test de RP-18).
Adaptaciones mock→app (no son cambios de diseño, son el mismo diseño sobre
tecnología real): editor falso del mock → CodeMirror (.cm-content/.cm-line,
gutter de breakpoint con :has(), caret nativo en vez de span falso);
`.secundario`→`.panel-nota`. El número de la línea de EJECUCIÓN en acento se
omitió (CM no lo expone simple; la barra de acento ya marca la exec-line).

[PENDIENTE HUMANO] Verificación visual FINAL contra el mock, en los 5 temas:
tarjetas con aire e IBM Plex, violeta del Educativo, headers tintados,
densidad, wordmark bi-tono recoloreando, microinteracciones sutiles (foco,
flash one-shot, caret, hover de breakpoint), ruido reducido. RP-20 (modales +
ayuda 2-col + Acerca de) cierra el rediseño.

## 2026-06-13 — RP20-F1: shell de modales y ayuda 2 columnas

[Seguro] Shell común de modales (`src/paneles/modal.ts`, clase `Modal`):
overlay `.emurv-modal` + `.modal-backdrop` (cierra al click, anim
`emurv-fade`) + `.modal-card` (anim `emurv-pop`, radio/sombra/`--fuente-ui`),
del mock. Escape cierra (listener que se registra al abrir y se quita al
cerrar); el foco entra a la card al abrir y VUELVE al disparador al cerrar
(accesibilidad — el botón Ayuda se pasa como disparador desde main.ts). El
`.modal-x` se cablea por delegación. Adaptaciones mock→app: `position:fixed`
(el mock usa absolute dentro de su demo) y UN overlay por modal (el mock
conmuta tres cards con `data-modal`; instancias separadas son más simples).

[Seguro] Ayuda reescrita a 2 columnas (`ayuda.ts`): `.ay-body` grid 200px/1fr
con `.ay-nav` (índice agrupado: Empezar / Referencia / Herramientas /
Proyecto) y `.ay-cont` scrolleable con las secciones EXACTAS del mock
(ay-ciclo, ay-ejemplos, ay-syscalls con `.ay-tabla`/`.ay-num`, ay-memoria,
ay-teclado, ay-directivas con `.ay-chips`, ay-editor, ay-depuracion,
ay-idioma, ay-about). PRESENTACIÓN nueva, CONTENIDO el de siempre: todos los
textos salen de las claves i18n `ayuda.*` existentes (NO se reescribió
ninguna). El About queda como sección vacía con su título (lo llena F2).

[Seguro] Scroll-spy: lógica PURA `seccionActiva(scrollTop, secciones)` en
`scroll-spy.ts` (la última sección cuya cabecera ya pasó el tope) con 6
tests. La usa ayuda.ts en los dos sentidos: click en el índice → `scrollTo`
suave + marca activa; scroll a mano → recalcula la activa.

[Seguro] i18n: +8 claves en es Y en (ayuda.subtitulo, ayuda.grupo.*,
ayuda.depuracion.titulo, ayuda.idiomaTemas.titulo, ayuda.about.titulo;
anti-drift verde). Se eliminó el CSS viejo del overlay de Ayuda
(`#ayuda-overlay`/`.ayuda-caja`/`.ayuda-cuerpo`), reemplazado por el de
modales. `.ayuda-ejemplo` se conserva (restyle del mock) para los bloques de
código de la ayuda.

[Seguro] Contraste: el modal reusa pares ya cubiertos por contraste.test
(--texto sobre --fondo-panel, --acento, --texto-suave); los 5 temas siguen
verdes sin cambiar tokens. tsc estricto y vite build limpios. UI 62 tests
(+6 scroll-spy). Sin renombrar ids; Rust intacto (frontend).

[PENDIENTE HUMANO] Verificación visual del modal de Ayuda en los 5 temas:
2 columnas, índice agrupado con la activa resaltada, scroll-spy al
desplazarse, tablas/chips/notas como el mock; Escape cierra y el foco vuelve
al botón Ayuda.

Compuerta F1: ayuda 2-col como el mock · scroll-spy (pura + testeada) ·
Escape/foco · i18n anti-drift · contraste AA 5 temas (pares ya cubiertos) ·
tsc/build limpios · Rust/ids intactos.

## 2026-06-13 — RP20-F2: sección Acerca de

[Seguro] `#ay-about` con el contenido EXACTO del mock (líneas 842-881):
`.ay-about-id` (wordmark grande + tagline + `.ay-ver` v0.1), `.ay-autor`
(«Enrique Emmanuel Ríos Chyrnia · Estudiante de Ingeniería Informática ·
Paraguay» — sin edad ni universidad, decisión del autor), `.ay-nota` IA,
tres grupos de créditos (`.ay-cred-grupo` + `.ay-cred`) con sus licencias en
`.ay-lic`, y `.ay-nota` de licencia GPLv2 (Unicorn condiciona el proyecto,
Cap. VI). El wordmark es el SVG bi-tono de RP-19 (import `?raw`) → se adapta
al tema (logo-emu = acento, logo-rv = texto).

[Seguro — verificación de créditos] Las versiones del mock se cotejaron
contra los manifiestos reales: TypeScript ~5.6.2 → 5.6, Vite ^6 → 6, Capstone
0.14, object 0.39.1 → 0.39, @tauri-apps/api ^2, Tauri 2, CodeMirror 6, app
0.1.0 → v0.1. Todas correctas.

[Seguro — corrección de honestidad vs el mock] El mock (anterior a RP-19) NO
listaba IBM Plex Sans. Como RP-19 la agregó (fuente de UI, bundleada, OFL
1.1), se SUMÓ al grupo «Tipografía e iconos» del Acerca de. Es la única
adición al contenido del mock; el resto va idéntico.

[Seguro] Versión: hardcodeada «v0.1» (igual que el mock; el manifiesto dice
0.1.0). NOTA DE MANTENIMIENTO: si sube la versión del proyecto, actualizar el
literal en ayuda.ts (`.ay-ver` y la tagline) — no se leyó del manifiesto para
no agregar un import de JSON por un dato que casi no cambia.

[Seguro — decisión idioma] El Acerca de va en español también en modo inglés
(como las lecciones y los mensajes del ensamblador: material del proyecto).
Son nombres propios + etiquetas de licencia + prosa breve; el plan pide
«contenido EXACTO del mock» (español). El TÍTULO de la sección sí es i18n
(ayuda.about.titulo, es/en).

[Seguro] tsc estricto y vite build limpios. UI 62. Sin tocar Rust ni ids.

[PENDIENTE HUMANO] Verificación visual del Acerca de en los 5 temas: wordmark
con su bi-tono adaptándose, autor/IA/créditos legibles, licencias correctas.

Compuerta F2: about completo y fiel (wordmark adaptable, autor, IA, créditos
con licencias verificadas + IBM Plex, GPLv2) · tsc/build limpios.

## 2026-06-13 — RP20-F3: modales de Ejemplos y Lecciones

[Seguro] Ejemplos: el `<select id="ejemplos">` de la toolbar pasó a un BOTÓN
«Ejemplos» que abre un modal con la cara del mock (`modal-cuerpo-ejemplos`:
lista de `.modal-item` con `.mi-titulo` + `.mi-desc`). Elegir uno hace lo
MISMO que antes: `cargarEjemplo(i)` (la lógica del viejo onchange, extraída a
un método). `src/paneles/ejemplos.ts` nuevo, sobre el shell `Modal`. i18n:
+12 descripciones `ejemplo.*.desc` + `ejemplos.sub` (es/en, anti-drift verde;
contenido NUEVO — las descripciones no existían, no es «reescribir» la ayuda).

[Seguro] Lecciones: `src/paneles/lecciones.ts` reescrito a la cara del mock
(`modal-cuerpo-lecciones`: `.modal-progreso` con barra + lista de
`.modal-item` con `.modal-check` si está completada / `.modal-num` si está
pendiente, «· en curso» en la primera no hecha). La barra y los checks leen
el PROGRESO REAL de RP-15 (`leerProgreso`); elegir una lección la abre
directo (un click) vía los mismos callbacks `onAbrir`/`onAbrirMultiarchivo`;
`marcarCompletada` intacto. La LÓGICA y la PERSISTENCIA (`src/lecciones/`:
datos, parser, progreso + sus tests) NO se tocaron — solo la presentación.

[DECISIÓN — DESVIACIÓN A REVISAR, marcada para la verificación visual] El
mock de Lecciones es una lista de UN click; el navegador de RP-15 tenía una
2ª columna de DETALLE (objetivos + «qué paneles mirar» + Abrir/marcar/
prev-next). Al darle «la cara del mock» esa columna de detalle DESAPARECE:
ahora el click abre la lección directo. Se preservó algo de su valor poniendo
el PRIMER objetivo de cada lección como `.mi-desc` del item. Lectura del plan
que resolvió la tensión («cara del mock» vs «no reescribir el navegador»): lo
protegido es la LÓGICA/PERSISTENCIA (`src/lecciones/`, intacta); «darle cara»
= cambiar la presentación (`paneles/lecciones.ts`). Si el autor quiere de
vuelta la vista de objetivos, es un agregado chico sobre esta base. El reset
de progreso se conservó como un enlace discreto (`.ml-reset`) en la cabecera
de la barra (el mock no lo mostraba; no quise perder la función).

[Seguro] i18n lecciones: +`lecciones.progresoSub` («{hechas} de {total}
completadas», interpolaciones verificadas es/en), `lecciones.tuProgreso`,
`lecciones.enCurso`. Las claves del detalle viejo (objetivos/paneles/abrir/
prev/next) quedan en i18n sin usar (inocuas; no las borré para no arriesgar
el anti-drift). CSS viejo del navegador (#lecciones-overlay/.lecciones-*)
eliminado; reemplazado por el modal + `.ml-reset`.

[Seguro] tsc estricto, vite build y UI 62 verdes. El id contratado del JS no
cambió (el `<select>` tenía id="ejemplos", que NO está en la lista de ids
contratados de UI-BRIEF; el botón nuevo lo reusa). Rust intacto.

[PENDIENTE HUMANO] Verificación visual de los 3 modales en los 5 temas:
Ejemplos (lista carga al click), Lecciones (barra de progreso real, check/
número, abre al click) y Ayuda; Escape cierra y el foco vuelve. CONFIRMAR la
decisión de Lecciones (lista sin la 2ª columna de objetivos del navegador
viejo): si se prefiere la vista de detalle, avisar.

Compuerta F3: Ejemplos y Lecciones con la cara del mock sobre la lógica
existente · progreso real · ids/registros/Rust intactos · i18n anti-drift ·
tsc/build limpios.

## 2026-06-13 — RP20-F4: docs rediseño de modales (cierre RP-20 y del rediseño)

[Seguro] Docs de cara al usuario (flujos que cambiaron): README actualiza el
flujo de Lecciones (de «navegador con objetivos + siguiente/anterior» a
«modal-lista con barra de progreso; al elegir, se abre directo»), Ejemplos
(«menú»→«botón») y suma una línea de los 3 modales (Ayuda 2-col con scroll-spy
+ Acerca de; Escape y foco). Ayuda integrada: `ayuda.lecciones` (es/en)
reescrita para reflejar el nuevo flujo (era inexacta tras F3; corrección de
exactitud, no un rediseño de contenido). ROADMAP-PRO y Cap. VI: NO tocados.

[Seguro] CIERRE RP-20 — criterios de aceptación:
- 3 modales (Ejemplos/Lecciones/Ayuda) con la cara del mock, sobre la lógica
  existente ✓ F1/F3
- Ayuda en 2 columnas con índice agrupado + scroll-spy; contenido i18n
  existente reorganizado (no reescrito) ✓ F1
- Acerca de completo y fiel (wordmark adaptable, autor, IA, créditos con
  licencias + IBM Plex, GPLv2) ✓ F2
- Progreso de lecciones real (RP-15), no duplicado ✓ F3
- 5 temas AA (el modal reusa pares ya cubiertos), ids/registros/Rust intactos,
  bundle ≤30 ✓ todas
Suites verdes (src-tauri y emurv-cli: cargo test + clippy -D warnings 0 ambos
· UI 62 con node:test, +6 scroll-spy), tsc estricto limpio, ci-local.sh
completo verde, tauri build ok, bundle EmuRV.app 22 MB ≤ 30 (RNF-04).

[Seguro] CIERRE DEL REDISEÑO VISUAL (RP-17B → RP-18 → RP-19 → RP-20):
la app quedó como el mock aprobado (propuestas-ui.html) — 5 temas, layout
flex con divulgación progresiva, estética Cálido (IBM Plex, violeta,
paneles-tarjeta, wordmark, microinteracciones) y los 3 modales rediseñados
con Ayuda 2-col + Acerca de. Las únicas desviaciones del mock, todas
registradas: contraste AA (verificado, sin ajustes en RP-20), ids del JS,
orden de registros, y la simplificación de Lecciones (lista de un click sin
la 2ª columna de detalle de RP-15 — marcada para revisión visual del autor).
Único pendiente del roadmap: tema-de-usuario (futuro, fuera de alcance).

[Seguro] ADAPTACIONES mock→app de RP-20 (mismo diseño, tecnología real):
overlay por modal con `position:fixed` (el mock conmuta uno con data-modal,
absolute en su demo); caret de consola nativo; el «en curso» y la barra de
progreso de Lecciones se calculan del progreso real; el version «v0.1» del
About es literal (nota de mantenerlo).

[PENDIENTE HUMANO] Verificación visual FINAL de los 3 modales en los 5 temas
contra el mock: Ayuda (2-col, índice con activa, scroll-spy, tablas/chips/
notas, Acerca de con wordmark adaptable), Ejemplos (lista carga al click),
Lecciones (barra de progreso real, check/número, abre al click). Escape
cierra y el foco vuelve al disparador. CONFIRMAR la simplificación de
Lecciones (sin la 2ª columna de objetivos del navegador viejo).

## 2026-06-13 — RP21-F1: modal de lecciones a dos columnas con objetivos

[Seguro] Fix de la regresión que RP-20 dejó marcada: el modal de Lecciones
recupera la VISTA DE OBJETIVOS de RP-15. Pasó de «lista de un click» a DOS
COLUMNAS (mismo lenguaje que la Ayuda 2-col, por coherencia): izquierda la
lista (conservada — `.modal-item` con check/num + barra de progreso real),
derecha el DETALLE de la lección elegida. Decisión del autor: acá la
fidelidad al mock CEDE ante el valor pedagógico (el mock simplificó sin
conocer la feature de RP-15).

[Seguro] Cambio de interacción: elegir un item en la lista ahora MUESTRA su
detalle (no abre). El detalle trae: título («Lección NN: …»), OBJETIVOS
(lista de `l.objetivos`), QUÉ PANELES mirar (`l.paneles`), botón «Abrir
lección» (`.principal`, carga al editor y cierra), y navegación
«anterior/siguiente». El item seleccionado se marca `.activa` (como el
`ay-nav .activo`). Primera apertura: el detalle muestra la lección «en
curso» (primera no completada) o la 1ª.

[Seguro — NO reescribir la lógica] La DATA ya estaba en el parser
(`MetaLeccion.objetivos[]/paneles`); sig/ant usa `vecina(numero, ±1, total)`
de RP-15 (ya testeada en progreso.test.ts) mapeando numero→índice; el
progreso (barra, checks, «en curso») lee `leerProgreso()` real; el reset y
`marcarCompletada` intactos. `src/lecciones/` (datos/parser/progreso) NO se
tocó. El caso multiarchivo (lección 8) se dispara recién en «Abrir lección»,
igual que hoy; en el detalle se muestra su nota informativa
(`lecciones.multiarchivoNota`).

[Seguro] i18n: CERO claves nuevas — los rótulos del detalle
(lecciones.objetivos/paneles/abrir/anterior/siguiente/leccion/
multiarchivoNota) YA existían (RP-15 los dejó; RP-20 los había dejado sin
usar). Anti-drift es/en sigue verde sin tocar nada. Estética por tokens
(`--radio`, `--fuente-ui`, `color-mix` del acento): `.lec-body` grid 260px/1fr,
`.lec-detalle` con h3 (barra de acento como la Ayuda), h4, lista, botones.

[Seguro] Contraste: el detalle reusa pares ya cubiertos por contraste.test
(--texto/--texto-suave sobre --fondo-panel, --acento, texto-sobre-acento del
botón principal); los 5 temas siguen verdes. tsc estricto y vite build
limpios. UI 62 (la lógica testeable —vecina, parser— ya estaba cubierta; el
resto es DOM/visual). API pública de Lecciones intacta (main.ts no cambia).
Sin tocar ids ni Rust.

[PENDIENTE HUMANO] Verificación visual en los 5 temas: elegir una lección
muestra sus objetivos y paneles a la derecha; «Abrir lección» carga y cierra;
anterior/siguiente recorre y respeta los bordes (deshabilitados en 1 y 10);
la barra de progreso y los checks reflejan el progreso real.

Compuerta F1: detalle con objetivos+paneles · abrir y sig/ant andando ·
progreso real intacto · contraste AA 5 temas · anti-drift i18n (sin claves
nuevas) · tsc/build limpios · Rust/ids intactos.

## 2026-06-13 — RP21-F2: docs vista de objetivos de lecciones (cierre)

[Seguro] Docs del flujo recuperado: README revierte el texto que RP-20-F4
había puesto («se carga directo») y vuelve a describir el modal de Lecciones
de DOS columnas (lista + objetivos/paneles a la derecha, anterior/siguiente,
«Abrir lección»); el bullet de modales aclara que Ejemplos es lista y
Lecciones es 2-col. Ayuda integrada: `ayuda.lecciones` (es/en) reescrita en
el mismo sentido (anti-drift verde, sin claves nuevas). ROADMAP-PRO y Cap.
VI: NO tocados.

[Seguro] CIERRE RP-21 — criterios de aceptación:
- Al elegir una lección se ven sus OBJETIVOS y QUÉ PANELES mirar ANTES de
  abrir (feature de RP-15 recuperada) ✓ F1
- Navegación siguiente/anterior funcionando (vecina, bordes deshabilitados) ✓ F1
- Lógica y progreso de RP-15 intactos (src/lecciones/ sin tocar); barra real ✓ F1
- Estética coherente con el rediseño (2 columnas como la Ayuda); contraste
  AA en los 5 temas ✓ F1
- i18n es/en de los rótulos: REUSADOS los de RP-15 (cero claves nuevas),
  anti-drift verde ✓ F1
Suites verdes (src-tauri y emurv-cli: cargo test + clippy -D warnings 0
ambos · UI 62 con node:test), tsc estricto limpio, ci-local.sh completo
verde, tauri build ok, bundle EmuRV.app 22 MB ≤ 30 (RNF-04).

[Seguro] La regresión que RP-20 había dejado marcada queda CERRADA: la
vista de objetivos de RP-15 está de vuelta, ahora con la estética del
rediseño. Decisión del autor registrada: acá la fidelidad al mock cede ante
el valor pedagógico (el mock simplificó sin conocer la feature). Pendiente
del roadmap: RP-22 (desacoplar paneles a ventanas) y el tema-de-usuario
(futuro).

[PENDIENTE HUMANO] Verificación visual del detalle en los 5 temas (objetivos
y paneles legibles, botón Abrir destacado, sig/ant correcto).

## 2026-06-13 — RP22-F1: mecanismo de desacople genérico + Registros

[Seguro — RESPUESTAS DEL SPIKE] Las tres preguntas que el plan pedía:
1. ¿La clase del panel se reusa TAL CUAL en otro documento? SÍ. `new
   Registros(cuerpo)` en `panel.ts` (la ventana host) funciona idéntico al
   lateral: la clase solo necesita un contenedor DOM + api/i18n, presentes en
   cualquier documento. CERO duplicación de lógica (no hay 3 implementaciones
   ad-hoc; un host parametrizado por `?panel=`).
2. ¿Los eventos llegan a la ventana 2da? SÍ. `app.emit` de Tauri hace
   BROADCAST a todas las webviews; el `listen()` de la ventana recibe
   emu://registros (durante la ejecución, ~10 Hz) y emu://detenido (al
   pausar). Requisito: la capability debía cubrir las ventanas nuevas →
   `"panel-*"` (glob) en capabilities/default.json (validado por build.rs).
3. ¿Cómo se propaga el tema/idioma? AL ABRIR (sólido): `inicializarTema()`
   lee localStorage `emurv.tema.v1` (las webviews comparten origen → mismo
   storage) y aplica data-tema; el módulo i18n autocarga `emurv.lang.v1` al
   importarse. EN VIVO (best-effort): listener `storage` re-aplica el tema
   sin recargar y recarga al cambiar idioma. Al abrir SIEMPRE es correcto.

[Seguro] Mecanismo genérico (`src/paneles/desacople.ts`): `desacoplarPanel({
id, seccionId, titulo, width, height, alReacoplar })` abre `panel.html?panel=
id` como `WebviewWindow` label `panel-<id>` (patrón de video.ts), en modo
MOVER — oculta la `<section>` del lateral YA (antes de crear la ventana, para
no duplicar; el flex de RP-18 recompone) y la RESTAURA en `tauri://destroyed`
/`tauri://error`. `botonDesacople()` arma el ⧉ del `<h2>` con
`stopPropagation` (no dispara el plegado de Depuración). Host: `panel.html`
(carga tokens.css + styles.css) + `src/panel.ts` (instancia la clase, se
suscribe, aplica tema/idioma).

[Seguro] Registros desacoplado (F1): la ventana instancia `Registros`, se
suscribe a onRegisters + onStopped y pide `get_state()` al abrir; editar
(doble-click → write_reg) ya andaba en la clase. COHERENCIA en ediciones:
write_reg/write_mem NO emiten evento, así que el panel embebido (oculto
durante el desacople) podría quedar viejo; se resuelve en el REACOPLE
(`alReacoplar` → get_state + actualizar). Durante run/step/stop ambos se
sincronizan solos por el broadcast de eventos. NO hubo que tocar Rust.

[Seguro] Pantalla NO se rompe: video.ts queda igual; el mecanismo es
paralelo (otro host, otras labels). build multipágina suma `panel.html` al
input de vite (junto a index/pantalla). i18n: +1 clave `desacople.titulo`
(es/en, anti-drift verde). index.html: el `<h2>` de Registros pasó a
`panel-titulo-con-boton` con un `<span data-i18n>` (para poder colgarle el ⧉
sin que traducirTitulos lo pise). CSS `.panel-desacoplar` (botón discreto).

[Seguro] Suites verdes: UI 62, src-tauri y emurv-cli (cargo test + clippy 0)
— el cambio de capability lo valida build.rs y pasó. tsc estricto y vite
build limpios. Sin renombrar ids del JS ni tocar Rust.

[PENDIENTE HUMANO] Verificación visual: ⧉ en Registros lo manda a una
ventana (sale del lateral, que se recompone); la ventana aplica el tema y el
idioma activos; correr/paso/detener refresca la ventana; editar un registro
ahí funciona y al cerrar la ventana el panel vuelve al lateral con el valor
editado.

Compuerta F1: Registros desacopla/recompone/edita/sincroniza · tema+idioma
en la ventana · contraste AA (pares ya cubiertos) · mecanismo genérico ·
Pantalla intacta · tsc/build · Rust intacto.

## 2026-06-13 — RP22-F2: desacople de Memoria

[Seguro] Memoria desacoplable con el MISMO mecanismo de F1 (cero código
nuevo de infraestructura): `agregarDesacople("memoria", …)` en main.ts +
rama `memoria` en panel.ts. La ventana instancia `Memoria`, se refresca por
emu://detenido (la memoria se relee al pausar, no por instrucción), navega
con sus botones .text/.data/.pila e Ir, y edita con doble-click → write_mem
(ya en la clase). Al reacoplar, `alReacoplar` → `memoria.refrescar()` recoge
las ediciones hechas en la ventana.

[Seguro — columna disasm sin tocar Rust ni la clase] La columna de
instrucción (RP11) necesita el «programa» (rango .text) que la ventana no
tiene. Se SINTETIZA: `get_disasm()` devuelve addr→texto y sus claves SON las
direcciones de .text → text_base = mín, text_len = máx−mín+4; data_base
(0x10010000) y sp_init (0x7FFFFFF0) son los estándar fijos del emulador. Con
ese `AssembleOk` sintético + `setDisasm()` la columna aparece igual que en el
lateral, y los botones de navegación van a las direcciones correctas. Sin
comando nuevo, sin modificar la clase Memoria. Si no hay sesión, dump en
blanco.

[Seguro] index.html: el `<h2>` de Memoria pasó a `panel-titulo-con-boton` +
`<span data-i18n>` (igual que Registros) para colgarle el ⧉. tsc estricto y
vite build limpios, UI 62. Sin claves i18n nuevas (reusa `desacople.titulo` +
`panel.memoria`). Sin tocar Rust ni ids.

[PENDIENTE HUMANO] Verificación visual: ⧉ en Memoria abre la ventana (sale
del lateral); navega/edita/se refresca al detener; la columna disasm aparece
con un programa ensamblado; al cerrar vuelve al lateral con lo editado.

Compuerta F2: Memoria desacoplable igual que Registros (navega, edita,
sincroniza, reaparece) · columna disasm · tsc/build · Rust intacto.

## 2026-06-13 — RP22-F3: desacople de Depuración (el delicado)

[Seguro — el problema] A diferencia de Registros/Memoria (estado del backend,
puro evento), los breakpoints se CREAN en el gutter del editor (ventana
principal) y la lista + las condiciones + los watchpoints son estado del
PANEL. Con la lista en otra ventana hay que sincronizar editor↔ventana sin
que las dos instancias de Depuración se pisen.

[Seguro — diseño] Protocolo de eventos entre ventanas (`paneles/dep-sync.ts`,
Tauri emit/listen, un solo lugar para nombres+payloads):
- `app://dep-filas` (main→ventana): la lista [{linea,addr,cond}].
- `app://dep-pedir` (ventana→main): handshake al abrir (pide el estado).
- `app://dep-quitar` (ventana→main): quitar el bp de una línea (main lo saca
  del editor → re-emite filas).
- `app://dep-condiciones` (ventana→main): condiciones, para que main las
  ABSORBA y no se pierdan al reacoplar.
AUTORIDAD del ENVÍO mientras está desacoplado = la VENTANA (tiene las
condiciones): main pone un flag `depDesacoplada` y, en `alCambiarBreakpoints`,
en vez de `enviarBreakpoints()` emite las filas; la ventana arma y manda
`set_breakpoints` (filas+condiciones). Así no se pisan las condiciones. Al
reacoplar, main re-sincroniza desde el editor y REASUME el envío (ya con las
condiciones absorbidas). Watchpoints: los maneja la clase en la ventana
(set_watchpoints), aplicados al backend.

[Seguro] Reuso de la clase: `Depuracion` se instancia en la ventana TAL CUAL;
sus callbacks se cablean al protocolo (onQuitarBreakpoint→emit quitar;
onCondicionCambiada→envía set_breakpoints + emit condiciones). Único agregado
ADITIVO a la clase: `setCondicionExterna(linea, cond)` (fija una condición sin
re-enviar) — la usan la ventana (sembrar lo que tenía main) y main (absorber
lo editado en la ventana). No cambia el comportamiento acoplado.

[Seguro] Detalles: el `<h2>` de Depuración se reestructuró (título+nota en un
span, clase `panel-titulo-con-boton`) para colgarle el ⧉ sin romper el
plegado; el ⧉ usa `stopPropagation` (no togglea el plegado). En la ventana, la
clase cablea el plegado al título → se NEUTRALIZA por CSS en panel.html
(`.panel.colapsado .panel-cuerpo { display:block }`), la ventana ES el panel.
Handshake doble (emit al desacoplar + pedir al cargar) cubre la carrera de
registro de listeners.

[Limitación documentada — futuro] Los WATCHPOINTS agregados en la ventana
desacoplada quedan aplicados en el backend (set_watchpoints) pero NO se
reflejan en el panel embebido al reacoplar (la instancia de main no los
rastrea; sí las condiciones, que se absorben). Como persistir posición/tamaño
de ventana o duplicar (en vez de mover): mejora futura.

[Seguro] tsc estricto, vite build, UI 62 verdes. Sin claves i18n nuevas
(reusa las de Depuración). Sin renombrar ids ni tocar Rust.

[PENDIENTE HUMANO] Verificación: ⧉ en Depuración abre la ventana; poner un
breakpoint en el editor aparece en la lista de la ventana; quitarlo desde la
ventana lo saca del editor; editar una condición y agregar un watchpoint
desde la ventana funcionan; al reacoplar, la lista y las condiciones siguen
bien.

Compuerta F3: Depuración desacoplable y sincronizada con el editor (bp
editor→ventana y quitar ventana→editor; condición y watchpoint desde la
ventana) · tsc/build · Rust intacto.

## 2026-06-13 — RP22-F4: docs desacople de paneles (cierre)

[Seguro] Docs: README estrena un punto «Paneles desacoplables» (botón ⧉ en
la cabecera de Registros/Memoria/Depuración → ventana propia, modo mover,
con tema/idioma y sincronizada). Ayuda integrada: `ayuda.ciclo.texto` (es/en)
suma la frase del ⧉ (anti-drift verde, sin claves nuevas). ROADMAP-PRO y Cap.
VI: NO tocados.

[Seguro] CIERRE RP-22 — criterios de aceptación:
- Registros, Memoria y Depuración desacoplables a ventana, modo MOVER (se van
  del lateral, vuelven al cerrar) ✓ F1-F3
- Ventanas 2das sincronizadas por eventos (broadcast de Tauri); edición desde
  la ventana funciona (write_reg/write_mem; set_breakpoints/set_watchpoints) ✓
- Tema + idioma aplicados en las ventanas (localStorage al abrir + storage en
  vivo best-effort) ✓ F1
- Mecanismo GENÉRICO (un host `panel.html?panel=…` + `desacople.ts`, no 3
  ad-hoc); Pantalla NO se rompe (mecanismo paralelo) ✓ F1
- 5 temas AA (las ventanas reusan pares ya cubiertos por contraste.test),
  ids/registros/Rust intactos, bundle ≤30 ✓ todas
Suites verdes (src-tauri y emurv-cli: cargo test + clippy -D warnings 0
ambos · UI 62 con node:test), tsc estricto limpio, ci-local.sh completo
verde, tauri build ok (capability `panel-*` validada), bundle 22 MB ≤ 30.

[Seguro] DEJADO COMO FUTURO (registrado): persistir posición/tamaño de las
ventanas entre sesiones; modo DUPLICAR (en vez de mover); multi-instancia del
mismo panel; reflejar en el panel embebido, al reacoplar, los watchpoints
agregados en la ventana de Depuración (hoy quedan en el backend pero no en la
lista de main); propagación de tema/idioma en vivo garantizada (hoy
best-effort por `storage`, siempre correcta al abrir).

[Seguro] Sin tocar Rust en TODO RP-22: el backend ya tenía los comandos y los
eventos; la sincronización y la coherencia se resolvieron en el frontend
(eventos de app + refresco al reacoplar). El único cambio fuera de TS fue la
capability (`panel-*`) y el input multipágina de vite.

[PENDIENTE HUMANO] Verificación visual de los 3 paneles desacoplados en
varios temas: ⧉ abre cada uno en su ventana (sale del lateral, que se
recompone); la ventana se ve con el tema/idioma activos; correr/paso/detener
la refresca; editar funciona; Depuración sincroniza breakpoints con el
editor; al cerrar, el panel vuelve al lateral con lo editado.

## 2026-06-13 — RP23-F1: cerrar ventanas hijas al cerrar la principal

[Seguro] Bug: al cerrar la ventana PRINCIPAL (la X), las ventanas hijas
desacopladas (panel-registros/memoria/depuracion, pantalla) quedaban
HUÉRFANAS abiertas. Fix FRONTEND (como todo RP-22):
`getCurrentWindow().onCloseRequested(...)` toma el control —
`evento.preventDefault()`—, cierra las hijas por su label conocida
(`WebviewWindow.getByLabel(label).close()`) y luego `principal.destroy()`.
Se usa `destroy()` (no `close()`) en la principal a propósito: NO re-dispara
`onCloseRequested`, así que no hay bucle de cierre.

[Seguro] Por LABELS conocidas (panel-* + pantalla) en vez de
`getAllWebviewWindows()`: `getByLabel` ya se usa en la app (desacople.ts) y
no agrega preguntas de permisos; el set de labels es fijo y chico. Modo
MOVER (RP-22): al cerrar las hijas por este camino se dispara su
`tauri://destroyed` → el `restaurar` de desacople pone visible la sección en
el lateral, pero da igual: la app entera se está cerrando (el plan lo
contempla).

[Seguro] Capability: la principal necesita cerrar/destruir ventanas →
`core:window:allow-close` y `core:window:allow-destroy` agregados a
capabilities/default.json (validados por build.rs; cargo test/clippy verdes).

[Seguro] tsc estricto y vite build limpios, UI 62. Sin tocar Rust (solo la
capability JSON). Sin claves i18n nuevas. Sin romper Pantalla (su cierre por
el botón sigue igual; este camino es adicional, al cerrar la principal).

[PENDIENTE HUMANO] Verificación manual: desacoplar un panel (o la pantalla) y
cerrar la ventana principal con la X → la hija se cierra también (no queda
huérfana) y la app sale limpia.

Compuerta F1: cerrar la principal cierra las hijas (frontend, sin bloquear
el cierre) · tsc/build · Rust intacto.

## 2026-06-13 — RP23-F2: Nuevo / Guardar como / Guardar inteligente + reagrupar

[Seguro] «Guardar» INTELIGENTE (`guardarArchivo`): antes SIEMPRE abría el
diálogo (se comportaba como «Guardar como», ignorando `rutaArchivo`). Ahora,
si `rutaArchivo != null`, guarda DIRECTO ahí sin re-preguntar (como un IDE);
si es null, cae en «Guardar como». La lógica común se extrajo a
`escribirArchivo(path)` (escribe + actualiza ruta + marca limpio + estado).

[Seguro] «Guardar como» (`guardarComo`): SIEMPRE pide ruta; `defaultPath` =
la ruta actual o «programa.asm». Botón propio en la toolbar.

[Seguro] «Nuevo» (`nuevoArchivo`): si hay cambios sin guardar, `confirm()`
didáctico antes de descartar; luego sale de modo binario, carga una PLANTILLA
mínima runnable (comentario + `.text`/`main:` + `li a7,10`/`ecall` que
termina limpio — decisión: una plantilla guía mejor que un buffer en blanco
para el principiante), resetea `programa`/`rutaArchivo`, limpia errores y
consola, estado «programa nuevo».

[Seguro — detección de «sucio»] Snapshot `docLimpio` = contenido del editor
en la última CARGA o GUARDADO; sucio = `getDoc() !== docLimpio` (y nunca en
modo binario, que es solo lectura). Para mantenerlo exacto se canalizaron las
5 cargas programáticas (ejemplo, lección, lección multi-archivo, abrir
archivo, listado binario) por un helper `setDocLimpio(contenido)` (setDoc +
docLimpio); el inicial = «Hola mundo». Robusto y barato; sin flags que se
desincronicen.

[Seguro] Toolbar reagrupada (RP23): sección de archivo `Nuevo · Abrir ·
Guardar · Guardar como` entre separadores; «Exportar ELF» queda APARTE
(produce binario, no guarda fuente). Todos `.deemph` (coherente con la
estética). Iconos Lucide nuevos sin dep npm (patrón RP-17/19): `nuevo` =
file-plus, `guardarComo` = save-all, mapeados en `icono(id)`.

[Seguro] i18n: +4 claves en es Y en (`toolbar.nuevo`, `toolbar.guardarComo`,
`archivo.descartarConfirm`, `archivo.nuevo`; anti-drift verde). NO se rompe
modo binario (rutaArchivo/lectura), ejemplos/lecciones (rutaArchivo=null), ni
el base_dir de `.include` (RP-5: escribirArchivo sigue actualizando
`rutaArchivo`). tsc estricto y vite build limpios, UI 62. Sin tocar Rust.

[Opcional NO incluido] Título de ventana con el archivo actual: requería el
permiso `core:window:allow-set-title`; se dejó fuera para no sumar permisos
por un extra menor (el plan lo marcaba opcional «si es barato»).

[PENDIENTE HUMANO] Verificación manual: Nuevo (con confirm si editaste),
Guardar como (pide ruta), Guardar (NO re-pregunta si ya hay ruta; sí la pide
la 1ª vez), Abrir; nada se rompe con ejemplos/lecciones/binario.

Compuerta F2: Nuevo (+confirm) · Guardar como · Guardar inteligente (no
re-pregunta con ruta) · sección de archivo agrupada · i18n anti-drift · sin
romper binario/ejemplos/lecciones/.include · tsc/build · Rust intacto.

## 2026-06-13 — RP23-F3: docs gestión de archivos y cierre

[Seguro] Docs: README estrena «Gestión de archivos tipo IDE» (Nuevo con
confirm, Abrir, Guardar inteligente, Guardar como; Exportar ELF aparte; y el
cierre de ventanas hijas al cerrar la principal). Ayuda integrada:
`ayuda.ciclo.texto` (es/en) suma la frase de Nuevo/Abrir/Guardar/Guardar como
(anti-drift verde, sin claves nuevas). ROADMAP-PRO y Cap. VI: NO tocados.

[Seguro] CIERRE RP-23 — criterios de aceptación:
- Cerrar la ventana principal cierra también las desacopladas ✓ F1
- Botón Nuevo (con confirm si hay cambios) + Guardar como + Guardar
  inteligente (no re-pregunta con ruta conocida) ✓ F2
- Sección de archivo agrupada y coherente con la estética (deemph, iconos
  Lucide, separadores) ✓ F2
- i18n es/en de todo lo nuevo (anti-drift verde) ✓ F2
- Sin romper modo binario, ejemplos/lecciones (rutaArchivo null), `.include`
  (base_dir) ✓ F2
Suites verdes (src-tauri y emurv-cli: cargo test + clippy -D warnings 0
ambos · UI 62 con node:test), tsc estricto limpio, ci-local.sh completo
verde, tauri build ok (capability con los permisos de cierre validada),
bundle EmuRV.app 22 MB ≤ 30 (RNF-04).

[Seguro] Todo RP-23 fue FRONTEND salvo la capability (permisos
`core:window:allow-close/destroy` para que la principal cierre a las hijas).
Sin tocar código Rust, sin renombrar ids, sin reordenar registros.

[PENDIENTE HUMANO] Verificación manual (parte del cierre): (a) desacoplar un
panel o la pantalla y cerrar la principal → la hija se cierra; (b) Nuevo /
Guardar / Guardar como / Abrir andan y «Guardar» no re-pregunta cuando ya hay
ruta (sí la pide la primera vez).

## 2026-06-14 — RP24-F1: find/replace en el editor

[Seguro] Búsqueda/reemplazo del editor con `@codemirror/search` (v6.7.0, dep
nueva; `^6.7.0` en package.json — única dependencia agregada). En `createEditor`
(`src/editor/index.ts`) se SUMARON a las extensions: `search({ top: true })`,
`keymap.of(searchKeymap)` y `highlightSelectionMatches()`. Cmd/Ctrl+F abre el
panel ARRIBA (no tapa la consola de abajo), con buscar, reemplazar uno/todos,
anterior/siguiente, distinguir mayúsculas, palabra completa y regexp. El resto
del editor quedó INTACTO: gutter de breakpoints, línea de ejecución
(`cm-linea-actual`), lint, autocompletado, hover e ir-a-definición — se
agregaron extensions, no se tocó ninguna existente.

[Seguro] Modo binario (read-only, RP11): el panel SOLO busca. CodeMirror oculta
los controles de reemplazo cuando `state.readOnly` es true (no hubo que
programarlo); el `Compartment` de read-only ya estaba y no se tocó.

[Probable] DECISIÓN — traducir el panel (frases de CodeMirror). El panel de
`@codemirror/search` resuelve cada etiqueta con `state.phrase(<frase fuente en
inglés>)`. El plan permitía dejarlo en inglés si traducir era engorroso, pero el
español es la lengua primaria de la tesis, así que se tradujo. Mecanismo: módulo
nuevo `src/editor/buscar.ts` con `frasesBusqueda()`, que mapea las 17 frases
fuente del panel (extraídas del `dist` del paquete: Find, Replace, next,
previous, all, match case, by word, regexp, replace, replace all, close, current
match, «replaced $ matches», «replaced match on line $», on line, Go to line,
go) a claves i18n nuevas (`editor.buscar.*`, AL FINAL de es.json/en.json para
minimizar conflicto de merge con los otros worktrees del lote). En las
extensions: `EditorState.phrases.of(frasesBusqueda())`. Las frases con `$` son
plantillas que CodeMirror sustituye con el conteo/línea: el `$` se conserva
LITERAL en la traducción (no es un parámetro i18n `{...}`). Alternativas
descartadas: (a) dejar el panel en inglés — rechazada por el principio de
español primario; (b) hardcodear las cadenas en español dentro del módulo —
rechazada por la regla 6 (nada de strings hardcodeados). Confianza [Probable] y
no [Seguro] porque la lista de 17 frases se ató al `dist` de la v6.7.0: un
cambio de copy en una futura versión de `@codemirror/search` dejaría esa
etiqueta puntual en su default inglés (degradación benigna, no rotura) —
`buscar.test.ts` fija la lista para detectarlo.

[Seguro] Sin tocar main.ts: NO hay handler global de Cmd+F (verificado por
grep), así que el `searchKeymap` dentro del editor alcanza; el atajo solo actúa
con el editor enfocado. Sin tocar Rust, paneles, temas ni el núcleo.

[Seguro] Tests: `src/editor/buscar.test.ts` (4 casos) cubre el mapa de frases —
cobertura de las 17 frases fuente, traducción al español por defecto (sin
localStorage), ningún valor vacío ni clave sin traducir (no cae al fallback) y
conservación del `$` en las plantillas. El grueso de la verificación de UI es
visual (abajo). Compuerta F1: tsc --noEmit verde; suite UI 66/66 verde (62
previas + 4 nuevas); clippy src-tauri limpio. Commit `rp24-fase-1`.

[PENDIENTE HUMANO] Verificación visual de F1: con la app corriendo —
(a) Cmd/Ctrl+F abre el panel arriba, busca y resalta coincidencias;
(b) reemplazar uno y reemplazar todos funcionan; (c) las etiquetas del panel
salen en español (y en inglés al cambiar idioma y recargar); (d) con un
breakpoint puesto, ejecutar hasta él con el panel de búsqueda abierto — el
gutter y la línea de ejecución resaltada siguen correctos; (e) abrir un ELF
(modo binario) — el panel solo busca, sin controles de reemplazo.

## 2026-06-14 — RP24-F2: docs find/replace y cierre

[Seguro] Docs: README estrena la mención de buscar/reemplazar (Cmd/Ctrl+F) en el
punto «Editor profesional»; la Ayuda in-app (`ayuda.editor.texto` en es/en) y su
espejo `docs/AYUDA-CONTENIDO.md` suman el atajo (resalta coincidencias; en modo
binario solo busca). NO se tocó ROADMAP-PRO ni el Cap. VI.

[Seguro] CIERRE RP-24 — criterios de aceptación:
- Cmd/Ctrl+F: buscar + reemplazar + resaltar. ✓ (cableado y testeado en F1; la
  verificación visual queda como [PENDIENTE HUMANO], arriba).
- Editor existente intacto (breakpoints, exec-line, binario). ✓ (solo se
  agregaron extensions; ninguna existente se modificó).
- Bundle ≤30 MB. ✓ La .app pesa 22 MB (la dep nueva del ecosistema CM es
  liviana: +~1 paquete, sin binarios).

[Seguro] Compuerta F2 (vía `scripts/ci-local.sh`): tsc estricto verde; UI 66/66;
vite build OK; `src-tauri` cargo test verde + clippy `-D warnings` limpio;
`emurv-cli` cargo test verde + clippy `-D warnings` limpio; build release del
CLI OK; `cargo tauri build` produce la `.app` (22 MB ≤ 30 MB, RNF-04 verde).

[Suposición] El empaquetado del **.dmg** (paso `bundle_dmg.sh` / `create-dmg`,
posterior a la `.app`) falló en esta sesión headless: `create-dmg` usa
AppleScript/Finder para acomodar la ventana del DMG y necesita una sesión GUI
con permiso de automatización que no existe en una corrida no interactiva. NO es
una regresión de RP-24: el diff de esta rama (una dep JS, el cableado del editor,
i18n y docs) no toca la config de bundling, los iconos ni el AppleScript del DMG;
y el sujeto de RNF-04 es la `.app`, que sí se construyó y mide 22 MB. Confianza
[Suposición] respecto a que basta re-correr `cargo tauri build` en una sesión de
escritorio para obtener el `.dmg`.

[PENDIENTE HUMANO] Re-correr `cargo tauri build` (o `scripts/ci-local.sh` sin
`--skip-bundle`) en una sesión de escritorio macOS para generar el `.dmg` y
confirmar que el único fallo era el entorno headless. Commit `rp24-fase-2`.
## 2026-06-14 — RP26-F1: búsqueda en el panel de memoria

[Seguro] Lógica de búsqueda AISLADA en un módulo puro `src/paneles/
memoria-buscar.ts` (sin DOM ni dependencias de Tauri): `parsearPatron`,
`buscarPatron` y el escaneo por bloques `buscarAdelante`/`buscarAtras` sobre
un lector inyectable (`LectorMem`). Motivo: poder testear los bordes con
`node:test` sin montar el DOM ni el puente Tauri (mismo criterio anti-drift
que `scroll-spy.ts`/`i18n`). Alternativa descartada: meter todo en
`memoria.ts` y mockear `api.readMem` — habría arrastrado el import de
`@tauri-apps/api` al test. El panel (`memoria.ts`) solo orquesta UI.

[Seguro] Endianness — el word de 32 bits se busca LITTLE-ENDIAN, tal como se
almacena en RV32: `parsearPatron("0000002A", "word")` → bytes `2A 00 00 00`.
Es la forma natural de localizar un dato en memoria (coincide con lo que
muestra el volcado). Documentado en la Ayuda (`ayuda.memoria.buscar`).

[Probable] Rango escaneado = la REGIÓN DE DATOS mapeada
`[data_base, 0x10400000)` — espejo de `DATA_REGION_BASE`/`DATA_REGION_SIZE`
del backend (`src-tauri/src/core/mod.rs`: base 0x10000000, tamaño 4 MiB).
Cubre `.data`, `.bss` y el heap (la zona donde vive el dato del usuario).
Acotado a propósito: NO incluye la pila, que está 8 MiB por debajo de
0x80000000 con un hueco NO mapeado de ~1,8 GB en medio; escanear ese hueco
dispararía errores de `read_mem` («dirección no mapeada») y/o colgaría la UI.
Limitación consciente (buscar en la pila queda fuera de alcance de RP-26).

[Seguro] Escaneo POR BLOQUES de 16 KiB (`BLOQUE`) con solape de
`patron.length-1` bytes entre bloques, para no perder una coincidencia
partida entre dos lecturas. Async (un `await api.readMem` por bloque) → no
congela la UI; corta en la 1ª coincidencia. 16 KiB equilibra nº de
round-trips IPC (≈240 para recorrer los 4 MiB en el peor caso) y tamaño del
payload JSON de cada lectura. «Anterior» reusa `buscarAdelante` recolectando
la última coincidencia con dirección < ancla (rango acotado, sin lógica de
escaneo inverso duplicada).

[Seguro] NO se agregó ningún comando Rust nuevo: la búsqueda reusa
`api.readMem` (comando `read_mem` ya existente). Todo RP-26 es FRONTEND
(`memoria.ts`, `memoria-buscar.ts`, `styles.css`, i18n, `ayuda.ts`, README).
Sin tocar `src/editor/`, `src/temas/`, `src-tauri/core` ni `main.ts`.

[Seguro] UI: fila de controles bajo los de dirección (input + selector
byte/word/ASCII + Buscar + ‹/›). Resaltado de la coincidencia POR BYTE en el
volcado (clase `.memoria-coincidencia`), así funciona aunque el patrón cruce
palabras o filas. `irA` (navegación manual) limpia la marca; la búsqueda usa
un `saltar` interno que la conserva. Avisos didácticos: patrón inválido para
el tipo, sin programa ensamblado, o no encontrado (con el patrón citado).

## 2026-06-14 — RP26-F2: docs y cierre

[Seguro] Docs: README estrena la búsqueda en la viñeta de la vista de
memoria; Ayuda integrada suma `ayuda.memoria.buscar` (es/en) en la sección
«Mapa de memoria» (tipo de patrón, endianness del word, ‹/›, rango = región
de datos). Claves i18n nuevas AL FINAL de `es.json` y `en.json` (es/en con
las mismas claves: test `idiomas` verde). ROADMAP-PRO y Cap. VI: NO tocados.

[Seguro] FLAKE PREEXISTENTE detectado en el cierre (NO es de RP-26): la suite
`emurv-cli/tests/rp6_debug.rs` (servidor GDB RSP) falla de forma
intermitente bajo ejecución PARALELA con «Connection reset by peer» (race de
socket entre tests que abren cada uno su puerto RSP). Verificado: 7/7 verdes
con `--test-threads=1` y cada test verde al reintentarlo aislado. RP-26 es
frontend puro: no toca Rust, así que no puede ser la causa. Queda registrado
como deuda del harness del CLI (fuera del área de RP-26).

[Seguro] CIERRE RP-26 — criterios de aceptación:
- Buscar byte/word/ASCII en memoria, saltar a la dirección y resaltar ✓ F1
- Siguiente/anterior coincidencia (‹/›, Enter/Shift+Enter) ✓ F1
- Sin congelar la UI: escaneo async por bloques de 16 KiB / rango acotado ✓ F1
- Lógica de búsqueda testeada (función pura, bordes) ✓ F1
- Docs README + Ayuda + i18n es/en ✓ F2
Compuerta final verde: `cargo test` + `cargo clippy --all-targets -D warnings`
verdes en AMBOS crates (src-tauri y emurv-cli), `tsc --noEmit` limpio,
`ci-local.sh --skip-bundle` verde (con el flake paralelo de rp6_debug
verificado verde en serie), `cargo tauri build` ok (EmuRV.app y .dmg), bundle
EmuRV.app 22 MB ≤ 30 (RNF-04). UI 75 tests con node:test (62 previos + 13 de
`memoria-buscar`).

[PENDIENTE HUMANO] Verificación manual: ensamblar un programa con datos
conocidos (p. ej. `.word 0x0000002A` o una cadena `.asciz "Hola"`), buscar
`2A` (byte), `0000002A` (word) y `Hola` (ASCII) → salta y resalta; ‹/›
recorren; un valor ausente avisa «no encontrado»; un patrón inválido para el
tipo avisa.
## 2026-06-14 — RP28-F1: tema de usuario (editor de tokens + persistencia)

Punto RP-28 del ROADMAP-PRO: la «Parte B» de RP17 (que el USUARIO arme su
tema). Ejecutado en worktree propio (rama `rp28/tema-usuario`), área
`src/temas/` + un panel/modal nuevo, disjunta del resto del lote paralelo.

[Seguro] DECISIÓN — variables inline vs bloque CSS editado por JS. El tema
«usuario» NO es un bloque `[data-tema="usuario"]` en tokens.css escrito por
JS, sino un objeto de tokens en localStorage (`emurv.tema-usuario.v1`)
aplicado como variables CSS INLINE sobre `:root`
(`documentElement.style.setProperty`). Alternativas consideradas: (a) editar
un bloque CSS vía CSSOM (frágil: hay que localizar la regla, pelear
especificidad, y el preview es más caro); (b) generar el bloque como string e
inyectarlo. El estilo inline gana la cascada sobre cualquier `[data-tema]`
sin tocar tokens.css → preview en vivo = un `setProperty`, sin parsear CSS.
Contracara conocida y resuelta: esas vars inline taparían a CUALQUIER otro
tema, así que `aplicarTema()` SIEMPRE limpia las vars de usuario al entrar, y
solo las repinta si el tema es «usuario» (ver `index.ts` →
`limpiarVarsUsuario`/`aplicarVarsUsuario`). Confianza: Seguro (cubierto por
`usuario.test.ts` e `index.test.ts`).

[Seguro] DECISIÓN — alcance de tokens editables. 17 tokens: superficies
(fondo, fondo-panel, fondo-claro, borde), texto (texto, texto-suave,
texto-sobre-acento), acento/estados (acento, ok, error, cambio) y la sintaxis
del editor (sx-com/dir/lbl/ins/str/num). Se INCLUYÓ la sintaxis —el plan la
daba como opcional «si entra»— porque si no, al cambiar `--fondo` a un tono
claro la sintaxis heredada del Educativo (pensada para fondo oscuro) podría
quedar ilegible y el aviso de contraste no tendría cómo corregirse: un
callejón sin salida. Con los `--sx-*` editables, el aviso AA puede vigilarlos
y el usuario arreglarlos. Default = copia EXACTA del Educativo
(`DEFAULT_USUARIO`), atada al bloque `educativo` de tokens.css por un test
anti-drift (lee el CSS y compara) → fuente única, sin copia desincronizada.

[Seguro] DECISIÓN — aviso de contraste, NO bloqueo. Al editar/guardar se
calculan los pares críticos (misma lógica WCAG que el test de los built-in,
ahora extraída a `contraste.ts` y compartida por test y UI) y se AVISA en el
modal si alguno < 4.5:1, sin impedir guardar: es el tema del usuario, la
legibilidad de los 5 built-in sigue siendo innegociable (test verde) pero la
del tema propio es su responsabilidad informada. Por eso «usuario» NO entra
en `contraste.test.ts` (no tiene valores estáticos que testear).

[Probable] DECISIÓN — export/importar JSON: SÍ en v1. El plan lo daba como
opcional «si es barato». Es barato (Blob + `<a download>` para exportar;
`<input type=file>` + FileReader para importar, sin plugin de Tauri ni
permisos nuevos) y completa el «cargar» del título de RP-28. Riesgo: el
comportamiento de descarga/diálogo de archivo dentro del webview de Tauri no
se verificó en automático → queda como PENDIENTE HUMANO. Importar normaliza
el objeto (claves conocidas con `#hex` válido; lo demás hereda del default),
así un archivo parcial o ajeno no rompe.

[Probable] DECISIÓN — CSS del modal INYECTADO por el componente
(`editor-tema.ts`, `<style id="estilo-editor-tema">`) en vez de tocar
`styles.css`. Razón: RP28 corre en un worktree disjunto del lote paralelo y
`styles.css` es un archivo muy compartido; mantener el modal autocontenido
evita conflictos de integración y respeta el «quedate en tu área». Todo el
CSS usa `var(--…)`, así el editor recolorea con el tema en edición. Si en el
futuro se unifica, el bloque `.et-*` se puede mover a styles.css sin cambios.

[Seguro] Todo RP28 es FRONTEND: sin tocar `src-tauri/`, `emurv-cli`, ni el
núcleo. `main.ts` mínimo (import + campo + construir el modal + un botón
«Editar tema» junto al selector + sincronizar el `<select>`). El selector
suma «Personalizado» solo (deriva de `TEMAS`). i18n es/en de todo lo nuevo
al final de ambos catálogos (anti-drift verde).

[PENDIENTE HUMANO] Verificación manual (lo visual, que el test no cubre):
(a) «Editar tema» abre el modal; mover un color o tipear un hex se ve al
instante en toda la app (preview en vivo); (b) cerrar SIN guardar revierte al
tema previo; «Guardar tema» persiste y deja «Personalizado» seleccionado;
reabrir la app conserva el tema; (c) poner un texto casi igual al fondo →
aparece el aviso de contraste (y NO impide guardar); «Restaurar Educativo»
vuelve al punto de partida; (d) Exportar baja `emurv-tema.json` e Importar lo
vuelve a cargar (verificar el diálogo dentro del .app empaquetada).

## 2026-06-14 — RP28-F2: docs y cierre

[Seguro] Docs: README estrena el bullet «Tema personalizado (crear/editar/
cargar)»; Ayuda integrada suma `ayuda.temas.usuario` (es/en) en la sección
«Idioma y temas»; tokens.css estrena una nota que explica por qué el tema
«usuario» no tiene bloque (vars inline desde localStorage). ROADMAP-PRO y el
Cap. VI: NO tocados.

[Seguro] CIERRE RP-28 — criterios de aceptación:
- El usuario crea/edita un tema, preview en vivo, persistido ✓ F1
- Aviso de contraste AA (no bloqueante) ✓ F1
- Los 5 temas built-in y su contraste intactos ✓ F1 (contraste.test verde,
  bloques built-in de tokens.css sin tocar)
- Diferenciador del nicho (Tabla 2.1): ninguna herramienta lo tiene ✓
Compuerta de cierre: cargo test + clippy AMBOS crates, tsc estricto, UI con
node:test, ci-local.sh --skip-bundle, tauri build y bundle ≤30 MB (RNF-04)
— resultados al pie de esta sesión.
## 2026-06-14 — RP29-F1: spike de viabilidad del ELF reubicable

Objetivo del punto RP-29 (Cap. VI §6.2.3): poder exportar un ELF cuyo CÓDIGO no
dependa de cargarse en una dirección fija. Hoy `export_elf` arma un ET_EXEC con
direcciones ABSOLUTAS del mapa RARS (.text 0x00400000, .data 0x10010000) y el
ensamblador codifica `la`/cargas con etiqueta como `lui rd, %hi(abs)` +
`addi rd, rd, %lo(abs)`: el binario asume cargarse en esas direcciones.

[Seguro] DECISIÓN — vía elegida: **Vía 1 (PC-relativo directo)**. En el modo
reubicable el ensamblador emitirá las referencias a símbolos como
`auipc rd, %pcrel_hi(sym)` + `addi rd, rd, %pcrel_lo` (estándar RISC-V),
codificando `sym - pc` en lugar de la dirección absoluta. Se descarta la Vía 2
(emitir relocations R_RISCV_PCREL_HI20/LO12 y delegar en un linker externo):
produciría un .o, no un ejecutable, exigiría un `ld` en runtime y rompería el
flujo educativo autocontenido. La Vía 1 es la más didáctica y la más barata
(ver costo abajo).

[Seguro] EVIDENCIA — `src-tauri/tests/spike_reubicable.rs` (oráculo GNU
riscv64-elf-as/ld/objcopy 2.46.1, sin relax para obtener el par canónico):
- `pcrel_es_reubicable_y_el_absoluto_no`: el MISMO objeto pcrel, enlazado en
  bases 0x00010000 y 0x80000000 (conservando el hueco .text↔.data), da un
  `.text` IDÉNTICO byte a byte (`00001517 00050513`); el par absoluto `lui/%hi`
  cambia (`00011537` vs `80011537`). → PC-relativo es independiente de la
  posición; el absoluto no. Demostración directa de la reubicabilidad.
- `nuestro_auipc_addi_reproduce_el_pcrel_de_gnu`: para huecos 0x1000/0x2234/
  0x1800/0x2ffc (cubre %lo=0, positivo+acarreo, negativo, negativo+acarreo)
  nuestro `auipc a0, hi` + `addi a0, a0, lo` con `hi=hi20(off)`,`lo=lo12(off)`
  da EXACTAMENTE las dos palabras que GNU resuelve para `%pcrel_hi/%pcrel_lo`.

[Seguro] COSTO / viabilidad: la matemática de `%pcrel_hi`/`%pcrel_lo` ES la de
los helpers `hi20`/`lo12` que ya usa el par `lui+addi` absoluto (compensación
de acarreo +0x800 y signo del %lo). La codificación de `auipc` (opcode 0x17) ya
existe en el ensamblador. Por lo tanto el modo reubicable es, en esencia,
alimentar `sym - pc` en vez de `sym` y cambiar el opcode de `lui` (0x37) por
`auipc` (0x17) en la expansión de `la`/cargas/almacenamientos con etiqueta. Es
una vía PROPORCIONADA: no requiere infraestructura nueva (sin relocations, sin
linker, sin tabla de fixups extra). No se aplica la regla de conflictos del
ROADMAP (frenar por desproporción): el spike sale en verde.

[Suposición → a confirmar en F2] Alcance de «reubicable» para EmuRV: el par
pcrel preserva `sym - pc`, así que el artefacto es reubicable mientras .text y
.data se desplacen JUNTOS (misma delta), conservando su distancia relativa —
que es el caso de `ld -Ttext`/cargadores que mueven la imagen como unidad. NO
es PIE con relocations dinámicas (fuera de alcance por diseño). El export
normal (absoluto) y la ejecución en la app NO cambian: el reubicable es una
OPCIÓN del artefacto exportado.

Compuerta F1: viabilidad demostrada + decisión fundada (Vía 1) con evidencia
del oráculo GNU · suite src-tauri intacta (incl. los 2 tests del spike) ·
clippy --all-targets -D warnings limpio.

## 2026-06-14 — RP29-F2: export ELF reubicable verificado contra GNU

[Seguro] IMPLEMENTACIÓN (Vía 1, decidida en F1). Modo reubicable en el
ensamblador: las pseudo-instrucciones con etiqueta —`la`/`lla`, cargas
(`lw`/`lh`/…/`flw`/`fld`) y almacenamientos (`sw`/…/`fsw`/`fsd`) con etiqueta—
expanden a `auipc rd, %pcrel_hi(sym)` + `op rd, %pcrel_lo` (codifican
`sym - pc`) en vez del par absoluto `lui rd, %hi(sym)` + `op rd, %lo`. La
matemática es la misma (`hi20`/`lo12`); el cambio es alimentar `c.pcrel(addr)
= addr - ins.addr` y usar el opcode de `auipc` (0x17) en lugar de `lui`
(0x37). Las direcciones de pasada 1 y el `source_map` NO cambian: el modo solo
afecta los BYTES emitidos en pasada 2.

[Seguro] API y alcance del cambio (respetando el área core + CLI):
- `Program` gana `reubicable: bool`; `export_elf` lo lee y marca el ELF
  `ET_DYN` (PIE) cuando es true, `ET_EXEC` (absoluto) cuando es false. NO
  cambió la firma de `export_elf` → `commands.rs`/GUI intactos (la GUI nunca
  ensambla reubicable; su `Program.reubicable` es siempre false).
- Nuevas entradas públicas `assemble_reubicable` y
  `assemble_with_includes_reubicable`; las existentes (`assemble`,
  `assemble_with_includes`) delegan en un `assemble_impl(.., reubicable)`
  con `false`, sin cambiar su firma.
- CLI: `emurv-cli asm --reubicable` (flag opcional; el parser a mano sigue sin
  `clap`). `run`/`disasm`/`debug` siguen absolutos.

[Seguro] VERIFICACIÓN — oráculo GNU as/ld/objcopy 2.46.1, `tests/
rp29_reubicable.rs` (5 tests; los que linkan se saltean sin toolchain):
- `reubicable_igual_al_pcrel_de_gnu`: nuestro `.text` reubicable de un
  programa con `la`+`lw`+`sw` es BYTE A BYTE el par auipc/%pcrel que GNU
  resuelve en el mapa de EmuRV (.text 0x00400000, .data 0x10010000). P.ej.
  `la a0, msj` → `0fc10517 00050513` (auipc a0,0xfc10 ; addi a0,a0,0).
- `pcrel_no_cambia_al_reubicar`: el MISMO objeto pcrel, enlazado en
  0x00400000 y en 0x20400000/0x30010000 (delta +0x20000000 a ambos
  segmentos), da un `.text` IDÉNTICO, y coincide con el nuestro → reubicable:
  misma conducta cargado en otra dirección.
- `export_normal_sigue_absoluto`: el `.text` del modo normal sigue siendo el
  par `lui/%hi` de GNU, byte a byte (regresión: la Vía 1 no tocó el camino
  absoluto).
- `reubicable_usa_auipc_y_el_normal_lui` y `elf_reubicable_es_et_dyn_y_normal_
  et_exec`: chequeos sin toolchain (opcode 0x17 vs 0x37; e_type 3 vs 2).
- Humo CLI: `emurv-cli asm --reubicable` → readelf Type=DYN y objdump muestra
  `auipc` resolviendo a `msj` (0x10010000); el normal → Type=EXEC y `lui`.

[Seguro] LIMITACIONES (documentadas, dentro del alcance del plan):
- El modo reubicable convierte las PSEUDO-instrucciones con etiqueta. Las
  formas MANUALES `lui rd, %hi(sym)` / `%lo(sym)(base)` que el usuario escribe
  a mano siguen absolutas (el par %hi/%lo es elección explícita del autor; el
  ensamblador no las re-aparea a %pcrel). Para reubicabilidad total: usar
  `la`/cargas-con-etiqueta. Los saltos (`j`/`call`/`b*`/`jal`) y las
  comprimidas (`c.j`/`c.beqz`) YA eran PC-relativos: no cambian.
- «Reubicable» = la imagen se desplaza como UNIDAD (`.text` y `.data` por la
  misma delta), conservando su distancia relativa; es lo que hace `ld -Ttext`
  y los cargadores de PIE. NO es PIE con relocations dinámicas (fuera de
  alcance por diseño).
- EmuRV solo IMPORTA `ET_EXEC` absoluto (elf_import.rs rechaza ET_DYN con
  mensaje didáctico): el artefacto reubicable es para OTRAS herramientas, no
  para recargar en la propia app. El mapa de memoria y la ejecución de la app
  no cambian.

Compuerta F2: reubicable verificado contra GNU (byte a byte + invariancia al
reubicar) · export normal absoluto intacto (regresión) · CLI `--reubicable` ·
suites verdes y clippy -D warnings limpio en AMBOS crates (src-tauri 219
tests, emurv-cli verde).

## 2026-06-14 — RP29-F3: docs ELF reubicable y cierre

[Seguro] DOCS. `README.md`: el subcomando `asm` documenta `--reubicable` (con
ejemplo) y un párrafo de «cuándo usarlo» (artefacto independiente de la
dirección de carga vs. export normal absoluto recomendado). `docs/
ENSAMBLADOR.md`: nueva §4.6 «Export reubicable: PC-relativo (RP29)» con la
expansión `auipc/%pcrel`, la equivalencia matemática con `hi20`/`lo12`, el
porqué de la independencia de posición (la imagen se desplaza como unidad), los
límites y la verificación con el oráculo GNU. NO se tocó `docs/ROADMAP-PRO.md`
ni el Cap. VI (regla del lote).

[Seguro] CIERRE RP-29 — criterios de aceptación (del plan):
- Opción de export que produce un ELF reubicable (no atado a 0x00400000),
  verificado con la toolchain GNU ✓ F1 (spike) / F2 (implementación + oráculo
  byte a byte + invariancia al reubicar).
- Export normal (absoluto) y ejecución en la app intactos ✓ F2 (regresión
  `export_normal_sigue_absoluto`; la GUI nunca ensambla reubicable; mapa de
  memoria sin cambios).
- Flag en emurv-cli (`asm --reubicable`) ✓ F2 (+ humo CLI: readelf Type=DYN,
  objdump muestra `auipc`).
- La regla de conflictos del ROADMAP (frenar si la vía es desproporcionada) NO
  se activó: el spike F1 mostró que la Vía 1 reusa primitivas existentes.

[Seguro] Compuerta F3 (cierre): `scripts/ci-local.sh --skip-bundle` TODO VERDE
— frontend (`npm ci` · `tsc --noEmit` · `npm run test:ui` · `vite build`) +
`cargo test` y `cargo clippy --all-targets -- -D warnings` en AMBOS crates
(src-tauri y emurv-cli) + build release del CLI. `npx tauri build` construyó
`EmuRV.app` y `scripts/check-bundle-size.sh` da **22 MB ≤ 30** (RNF-04).

[PENDIENTE HUMANO] El paso `.dmg` de `npx tauri build` no se ejecutó en esta
sesión headless (`create-dmg` usa AppleScript/Finder y necesita sesión de
escritorio); el sujeto de RNF-04 es la `.app` (verde). Re-correr el bundle
completo en sesión de escritorio para emitir el `.dmg`.

[PENDIENTE HUMANO] Verificación manual sugerida (no bloqueante): exportar por
CLI `asm --reubicable` un programa con `la`/`lw etiqueta` y abrirlo en una
herramienta externa (objdump/Ripes) — confirmar que las referencias usan
`auipc/%pcrel` y resuelven al símbolo; el export normal sigue mostrando `lui`.

[Seguro] ÁREA. RP-29 tocó solo `src-tauri/src/core/` (assembler, elf_export,
elf_import) y `emurv-cli/src/main.rs` (el flag), más tests y docs. Sin tocar el
frontend (`src/`), sin `commands.rs`, sin merges ni cambios de rama.

## 2026-06-14 — RP32-F1: spike RV64 (Unicorn + arquitectura del cambio xlen)

Punto RP-32 del ROADMAP-PRO: soporte RV64 (64 bits) CONMUTABLE con RV32 (no
reemplazo). RV32 sigue siendo el default y debe quedar 100 % intacto. Es el
punto más grande del roadmap: toca el núcleo entero, por eso la Fase 1 es un
spike que FIJA la arquitectura del cambio ANTES de tocar `machine.rs`.

### Evidencia del spike (`tests/spike_rv64.rs`, 5 tests verdes)

PoC con Unicorn 2.1.5 CRUDO (mismo patrón que spike_csr/spike_mmio: medir
primero, comprometer después). Encodings armados a mano contra la espec base
(opcodes OP-IMM 0x13, OP-IMM-32 0x1B, LOAD 0x03, STORE 0x23); la verificación
palabra-a-palabra contra `riscv64-elf-as -march=rv64im` queda para la Fase 3.

- **P1 [Seguro]** `Mode::RISCV64` ejecuta instrucciones RV64-only y retiene
  valores > 2³². `slli t0, t0, 40` → `t0 = 0x100_0000_0000` (2⁴⁰);
  `addiw t1, x0, -1` → `t1 = 0xFFFF_FFFF_FFFF_FFFF` (resultado de 32 bits
  SIGN-extendido a 64, semántica RV64 de las W-instructions).
- **P2 [Seguro]** El binding de registros es el MISMO que RV32 (`xN = id N+1`,
  `Arch::RISCV`): round-trip de 64 bits `reg_write`/`reg_read` →
  `0x0123_4567_89AB_CDEF` exacto. No hace falta otra dependencia ni otro
  esquema de IDs (los de CSR/F de RP8/RP12 también valen).
- **P3 [Seguro]** `sd`/`ld` mueven 8 bytes: `0xCAFE_BABE_DEAD_BEEF` ida y
  vuelta por memoria, registro y bytes en RAM coinciden.
- **P4 [Seguro]** RV32 INTACTO. El MISMO binario RV32I (`li/li/add`) da `t2=42`
  tanto en `RISCV32` como en `RISCV64`; y una instrucción RV64-only es ILEGAL
  en RV32 (`addiw` y `slli shamt=40` → `Err(EXCEPTION)`). Es decir: el modo del
  motor enforce el repertorio correcto sin que nosotros validemos nada.
- **P5 [Seguro] (el hallazgo decisivo)** En `Mode::RISCV32` el motor TRUNCA los
  registros a 32 bits ÉL MISMO: se escribió `0x1_0000_0001` y `reg_read`
  devolvió `0x1`; en `RISCV64` devolvió `0x1_0000_0001`. Consecuencia: pasar la
  API interna a `u64` NO altera la semántica de RV32 — en modo 32 los valores
  ya vuelven acotados a 32 bits por Unicorn, igual que hoy (`reg_read(..) as
  u32` sigue siendo correcto y redundante).

### Decisión [Seguro]: u64-interno + `xlen` conmutable (Vía elegida)

1. **`xlen` viaja con el `Program` y con la `Machine`.** Enum `Xlen { Rv32,
   Rv64 }`, default `Rv32`. El ensamblado produce el `xlen` y la máquina lo
   recibe en `new`. La conmutación es elegir, en `Machine::new`, `Mode::RISCV32`
   o `Mode::RISCV64` según `xlen` — TODO el resto del setup (mapa de memoria,
   hooks, carga) es idéntico.
2. **El núcleo opera en `u64`.** `reg_read`/`reg_write` de Unicorn ya son `u64`;
   en RV32 el motor garantiza el truncado a 32 (P5). `Machine::registers()`
   pasa a `[u64; 32]` (el «cambio mecánico u32→u64» que el plan autoriza); los
   pocos tests que comparan contra `SP_INIT`/`GP_INIT` (`u32`) se ajustan con
   `u64::from(..)` en F2 sin cambiar semántica.
3. **Las DIRECCIONES siguen en 32 bits.** El mapa de memoria educativo (estilo
   RARS: `TEXT_BASE`, `SP_INIT`, framebuffer, teclado MMIO) se mantiene BAJO y
   cabe en 32 bits → cabe en 64 sin tocar nada. Por eso en `StopReason` los
   campos de DIRECCIÓN (`addr`, `pc`) quedan `u32`; lo único que se ensancha a
   `u64` es el campo de VALOR (`Watchpoint.value`), que es un dato de registro/
   memoria y sí puede ser de 64 bits en RV64. Esto resuelve la aparente
   contradicción del plan («StopReason pasa a u64» vs «las direcciones caben en
   32»): se ensanchan los VALORES, no las direcciones.
4. **Frontend↔backend (para F4):** los valores de registro de 64 bits cruzan el
   límite como STRINGS hex (igual que `get_fregs` en RP12), NO como `number` de
   JS — un `number` pierde precisión por encima de 2⁵³ y no representa
   `0xCAFE_BABE_DEAD_BEEF`. El panel mostrará 16 dígitos hex en RV64 y 8 en
   RV32.

### Alcance PROPORCIONADO (regla de conflictos del ROADMAP: NO se activó)

El spike confirma que la Vía reusa primitivas existentes (mismo `Arch`, mismo
binding, mismo mapa, mismos hooks): el cambio queda ACOTADO a (a) elegir `Mode`
por `xlen`, (b) ensanchar valores de registro a `u64` en la API de `Machine` y
en `Watchpoint.value`, (c) parametrizar el ensamblador por `xlen` (F3) y (d) el
selector + registros de 64 bits en la UI (F4). NO hay que reescribir la máquina
ni duplicarla. El alcance es razonable → se PROCEDE (no se frena ni se trocea).

**Alternativas consideradas y descartadas:**
- *Tipo `Machine` genérico sobre el ancho, o un `Rv64Machine` aparte* —
  duplicación enorme de los ~1400 renglones de `machine.rs` (hooks, syscalls,
  watchpoints, video, teclado) para nada: Unicorn ya conmuta el ancho con
  `Mode`. Descartada [Seguro].
- *Mantener `registers() -> [u32;32]` y agregar `registers_x64()` aparte* —
  evitaría tocar tests, pero parte la API en dos vistas y obliga a la UI a
  elegir cuál llamar según el modo; con `u64` unificado el RV32 sigue dando
  valores de 32 bits «gratis» (P5). Se prefiere la API única en `u64`.
- *Direcciones a `u64` también* — innecesario y costoso: el mapa educativo es
  bajo por diseño (compatibilidad RARS) y NO se mueve a direcciones altas
  (fuera de alcance del plan). Se dejan `u32`.

### Compuerta F1 — VERDE

Spike verde (5/5) · arquitectura decidida y fundada (esta entrada) · suite
actual INTACTA: `cargo test` de src-tauri da **224 passed, 0 failed** = 219
previos + 5 del spike (los 219 sin tocar). `cargo clippy --test spike_rv64 -- -D
warnings` limpio. Commit `rp32-fase-1: spike RV64 (Unicorn + arquitectura xlen)`.

## 2026-06-14 — RP32-F2: núcleo conmutable RV32/RV64 (valores u64, RV32 intacto)

Implementación de la arquitectura decidida en F1: la `Machine` es CONMUTABLE.
El cambio es el ACOTADO que predijo el spike — reusa todas las primitivas
(mismo `Arch`, mismo binding, mismo mapa, mismos hooks); solo cambia el `Mode`
elegido y el ancho de los VALORES.

[Seguro] CAMBIOS DEL NÚCLEO:
- `core::Xlen { Rv32 (default), Rv64 }` (Serialize/Deserialize, `bits()`):
  viaja con el `Program` (`assembler::Program.xlen`) y la `Machine` lo lee.
- `Machine::new_with_video` elige `Mode::RISCV32`/`RISCV64` con `mode_for(xlen)`
  (única línea conmutada del setup; mapa, hooks y carga IDÉNTICOS).
- `Machine::registers()` pasa a `[u64; 32]`. En RV32 el motor ya acota a 32
  bits (P5 del spike), así que el rango observado es el mismo de antes; se
  borró el `as u32` redundante. Las DIRECCIONES (`pc()`, `StopReason.addr/pc`)
  siguen `u32`: el mapa educativo es bajo por diseño.
- `StopReason::Watchpoint.value` y el `hit_watchpoint` interno pasan a `u64`;
  `atender_watch` lee hasta 8 bytes (`u64::from_le_bytes`), así un `sd` de RV64
  reporta el doubleword completo. Un acceso de 4 bytes da el mismo valor que
  antes (el alto sin leer queda en cero).
- `commands::MachineState.registers` → `[u64; 32]` (serde). En RV32 los valores
  caben holgados en `number` de JS; el panel de 64 bits (F4) cruzará los
  valores grandes como string hex (un `u64` > 2^53 pierde precisión como
  `number`). La GUI en F2 solo crea programas RV32.
- `assembler` y `elf_import` fijan `xlen: Rv32` (el ensamblador RV64 es F3; el
  importador usa `ElfFile32`, es RV32 por construcción).

[Seguro] REGRESIÓN — REGLA DURA CUMPLIDA: los ~219 tests previos siguen verdes.
El único toque a tests existentes fue MECÁNICO por el ensanche `u32→u64` de
`registers()` (sin cambiar ninguna aserción de comportamiento):
`u64::from(SP_INIT/GP_INIT/HEAP_BASE)` en las comparaciones contra constantes
del mapa (integracion_ejemplos, rp10_syscalls, machine.rs), `u64::from(b'…')`
en tec2_teclado, tipos de los helpers `run_program`/`Observado`/`par_a_u64`
(tec3, rp1_ebreak). Ninguna lógica de test cambió.

[Seguro] TESTS NUEVOS (`tests/rp32_rv64_maquina.rs`, 4 verdes): construyen el
`Program` con palabras RV64 a mano (el ensamblador aún no las produce) y
prueban — (1) round-trip de 64 bits: `slli 40` → 2^40 y `addiw -1` →
`0xFFFF…FFFF`; (2) `ld`/`sd` mueven 8 bytes a `.data` (parte alta 0x100 que un
`sw`/`lw` perdería); (3) CONMUTACIÓN real: el MISMO `addiw` corre en `Rv64` y
es ilegal en `Rv32`; (4) regresión: un programa ensamblado (RV32 por defecto)
corre y da `t2 = 42`, con `program.xlen == Rv32`.

[Seguro] Compuerta F2 — VERDE: `cargo test` src-tauri **228 passed, 0 failed**
(224 + 4 nuevos) · `emurv-cli` verde · `cargo clippy --all-targets -- -D
warnings` limpio en AMBOS crates. Commit `rp32-fase-2: núcleo conmutable
RV32/RV64`.
