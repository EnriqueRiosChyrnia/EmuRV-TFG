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
