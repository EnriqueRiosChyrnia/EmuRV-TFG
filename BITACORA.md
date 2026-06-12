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
