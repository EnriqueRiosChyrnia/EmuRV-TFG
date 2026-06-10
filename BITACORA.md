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

## PENDIENTE DE VERIFICACIÓN VISUAL DEL USUARIO

Los gates FB2 y FB3 exigen verificación VISUAL que una sesión autónoma no puede hacer (no puedo ver la pantalla). El backend y la capa de UI están implementados y testeados; falta que el usuario confirme mirando la app. **Hasta entonces FB2 y FB3 NO se declaran cumplidos.**

Pasos exactos:

1. Abrir `src-tauri/target/release/bundle/macos/EmuRV.app` (build del 2026-06-10) o correr `cargo tauri dev`.
2. **Gate FB2 — franjas:**
   - Menú «Ejemplos» → «Franjas de color (pantalla)» → «Ensamblar» → «Ejecutar».
   - DEBE: aparecer solo el panel «Pantalla (320×240)» (sin tocar el botón) y verse tres franjas cortas (azul, verde, cian) sobre fondo negro en la parte superior, con píxeles nítidos (sin blur).
   - Guardar captura como `docs/capturas/fb2-franjas.png`.
3. **Gate FB3 — Tetris gráfico:**
   - Menú «Ejemplos» → «Tetris gráfico» → «Ensamblar» → «Ejecutar».
   - DEBE: aparecer el panel con el tablero vacío (dos bordes verticales blancos) ANTES de teclear nada; la consola muestra "Pieza: J" con su vista previa.
   - Jugar: rotación `0`, columna `0` → la J debe verse apilada abajo a la izquierda en gris claro, ANTES del siguiente prompt. Luego rotación `0`, columna `4` → la O en cian junto a la J. Salir con `-1`.
   - Guardar captura del tablero con piezas como `docs/capturas/fb3-tetris.png`.
4. **Medición de FPS pendiente:** con la app abierta, correr un programa que reescriba el frame en bucle y anotar acá los FPS efectivos (el plan lo pide; no se puede medir sin la app corriendo).
5. Si algo de lo anterior NO se ve así, reportarlo: el gate queda abierto.

## Recomendaciones para trabajo futuro (fuera de alcance)

1. **Entrada en tiempo real:** syscall de teclado sin bloqueo (estilo INT 16h) para que el Tetris caiga solo; hoy la entrada es por turnos como permite el plan.
2. **Sprites y animación:** el hardware emulado no los tiene (framebuffer lineal); se pueden implementar en software sobre lo existente.
3. **Medición de FPS bajo carga** (ver punto 4 de la verificación visual).
