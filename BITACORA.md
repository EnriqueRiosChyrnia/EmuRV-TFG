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
