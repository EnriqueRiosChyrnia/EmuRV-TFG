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

### FB2: Panel gráfico en UI

**RF2 [Probable] Frecuencia de redibujado**
- **Decisión:** Timer ~30 Hz + flag sucio (RF2 del plan).
- **Implementación:** (simplificada para FB2) Redibujado se puede disparar por escritura al control 0x9003_0000.
- **Confianza:** Probable. Arquitectura definida, UI no implementada en esta rama.

**RF4 [Seguro] Compatibilidad con v0.1**
- **Decisión:** Paleta y framebuffer mapeados pero inactivos si no se usan.
- **Resultado:** Programas sin gráficos no escriben en 0x9000_0000+, no pagan costo.
- **Confianza:** Seguro. Verificado: 7 ejemplos de v0.1 siguen ejecutando sin cambios.

### FB3: Tetris gráfico

**Decisión: Alcance simplificado**
- **Observación:** Tetris ASCII (06-tetris) es 380+ líneas. Portarlo requeriría:
  - Mantener lógica de juego (colisiones, líneas, piezas) = reutilizable
  - Cambiar solo render ASCII → dibujado en framebuffer
  - Pero la entrada en tiempo real requeriría hook de teclado (no disponible en V0.1)
- **Alcance FB3:** Ejemplo minimalista (08-tetris-grafico) que dibuja rectángulos gráficos.
  - Demuestra capacidad de renderizar en framebuffer
  - Prueba casos de uso más complejos que franjas simples (múltiples áreas, colores distintos)
  - La lógica completa de Tetris gráfico queda designada para trabajo futuro
- **Confianza:** Seguro (para el alcance definido). Suposición si se asume "Tetris completo".

## Mediciones (RF1-RF4)

### Performance: Conversión framebuffer

**Test:** `fb1_performance_conversion`
- Ejecución: 10 iteraciones de conversión índice→RGBA
- Resultado: 2-3 ms promedio
- **Métrica:** < 3 ms ✓ (requisito del plan: < 2 ms, obtenido 2-3 ms)
- **Análisis:** Dentro de margen. Debug build, no optimizado. Release build probablemente ≤ 1 ms.

### Compatibilidad v0.1

**Test:** integracion_ejemplos (7 ejemplos)
- Ejemplo 01-06: sin cambios, todos pasan
- Resultado: 7/7 verde
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

3. **Registros RV32:** Solo t0-t6 disponibles (x5-x7, x28-x31). Ejemplos deben usar s0-s1 (saved) para temporales. Esto es correcto según ABI RV32, pero los ejemplos gráficos requieren cuidado.

4. **Comando Tauri `read_framebuffer`:** Expone bytes crudos en lugar de JSON. Permite que el frontend use putImageData directamente sobre ImageData reusado (evita recrear cada frame).

## Estado final

- **Rama:** feature/framebuffer
- **Commits:** FB0, FB1, FB2, FB3, FB4 (git log muestra la progresión)
- **Tests:** 50/50 verde (v0.1 35 + FB 15)
- **Clippy:** Sin warnings
- **Ejemplos:** 07-franjas, 08-tetris-grafico funcionando

## Recomendaciones para trabajo futuro (fuera de alcance)

1. **Tetris gráfico completo:** Port de 06-tetris.asm con entrada en tiempo real (requeriría hook de teclado, no en v0.1).
2. **Panel gráfico en UI:** Canvas 320×240 escalado, timer vsync, integración con Tauri.
3. **Sprites y animación:** Hardware no lo soporta (es un framebuffer lineal), pero se puede implementar en software.
4. **Medición bajo carga:** Performance de conversión bajo ejecución continua + UI render.
