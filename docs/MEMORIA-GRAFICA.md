# MEMORIA — Mapa de memoria gráfica (inspirado en el modo 13h de VGA, con resolución 320×240)

Extensión del mapa de memoria RARS estándar con soporte para pantalla gráfica de 320×240 píxeles. El diseño está **inspirado en el modo 13h de VGA** (framebuffer lineal, 1 byte por píxel, paleta indexada de 256 colores), pero no lo replica: el modo 13h real es de 320×200; aquí se adopta una resolución de 320×240.

## Regiones mapeadas

| Región | Base | Tamaño física | Tamaño mapeado | Propósito | Acceso |
|--------|------|---|---|-----------|--------|
| `.text` | `0x0040_0000` | — | 4 MB | Código del programa | R/X |
| `.data` | `0x1001_0000` | — | 4 MB | Datos estáticos | R/W |
| Pila | `0x7FFF_FFF0` (sp inicial) | — | 8 MB | Stack | R/W |
| **Framebuffer** | **`0x9000_0000`** | **76 800 B** | **128 KB** | Píxeles 320×240 | R/W |
| **Paleta** | **`0x9002_0000`** | **768 B** | **4 KB** | 256 colores RGB888 | R/W |
| **Control** | **`0x9003_0000`** | **4 B** | **4 KB** | Marca de uso de pantalla | R/W |
| **Tabla de sprites** | **`0x9005_0000`** | **512 B** | **4 KB** | 64 sprites × 8 B (RP31) | R/W |
| **Patrones de sprites** | **`0x9006_0000`** | **16 384 B** | **16 KB** | 64 patrones de 16×16 (RP31) | R/W |

> **Nota (2026-06-12):** la fase de teclado agregó una región más al mapa, ajena al subsistema gráfico: la página del dispositivo de teclado MMIO en `0xFFFF_0000` (4 KB; `RECEIVER_CONTROL` en +0 y `RECEIVER_DATA` en +4, compatibles con RARS/MARS). Está documentada en `TECLADO.md`.

> **Nota (2026-06-15, RP31):** se agregaron dos subsistemas más, espejando el modelo del framebuffer. (1) El **overlay de sprites** (tabla en `0x9005_0000` + patrones en `0x9006_0000`), documentado abajo en la sección «Sprites». (2) El **dispositivo de sonido** en `0x9004_0000` (4 KB), ajeno al subsistema gráfico, documentado en `SONIDO.md`. Todas las regiones nuevas son disjuntas de las anteriores (verificado en `tests/spike_sonido.rs`).

## Framebuffer (0x9000_0000 — 0x9001_FFFF)

**Formato:** array lineal de 76,800 bytes (320 columnas × 240 filas × 1 byte/píxel).

Indexación:
- Byte en desplazamiento `d` corresponde al píxel en posición `(x, y)` donde `x = d mod 320`, `y = d / 320`.
- Rango válido: `[0, 76800)`.

**Semántica:** cada byte es un índice de 0–255 en la tabla de paleta. Escribir un byte actualiza ese píxel inmediatamente en memoria; el frame se publica hacia la UI al final de cada tramo de ejecución (paso, tramo de corrida continua, detención) y antes de bloquearse en una syscall de entrada, y la UI refresca con un timer de ~30 Hz (RB-2, ex RF2 del plan de framebuffer; ver la sección del registro de control).

## Paleta (0x9002_0000 — 0x9002_02FF)

**Formato:** 256 entradas × 3 bytes RGB (8 bits cada canal), base `0x9002_0000` (`PALETTE_BASE` en `src-tauri/src/core/mod.rs`).

Indexación:
- Entrada `i` (0–255) se almacena en desplazamientos `[3i, 3i+1, 3i+2]` desde la base.
  - Byte en `0x9002_0000 + 3i`: componente R (bits 7–0).
  - Byte en `0x9002_0000 + 3i + 1`: componente G.
  - Byte en `0x9002_0000 + 3i + 2`: componente B.

**Ejemplo:** color rojo puro (R=255, G=0, B=0) en la entrada 10. El desplazamiento de la entrada `i` es `3i`; para `i = 10` es 3×10 = 30 decimal = `0x1E`. Los tres componentes quedan entonces en `0x9002_001E` (R), `0x9002_001F` (G) y `0x9002_0020` (B):

```asm
li   t0, 0x9002_0000   # base de la paleta (PALETTE_BASE)
li   t1, 0xFF
sb   t1, 0x1E(t0)      # R = 255 (entrada 10: desplazamiento 3×10 = 30 = 0x1E)
sb   zero, 0x1F(t0)    # G = 0
sb   zero, 0x20(t0)    # B = 0
```

Conviene `sb` porque cada componente ocupa exactamente 1 byte: un `sw` escribiría 4 bytes, pisando el componente R de la entrada siguiente, y además las entradas de 3 bytes no quedan en general alineadas a word (`0x1E` no es múltiplo de 4). La sintaxis usada (`li` con literal hexadecimal —los guiones bajos como separador son válidos— y store con direccionamiento `desplazamiento(base)`) es la que soporta el ensamblador propio (ver ENSAMBLADOR.md §4.1–4.2).

> **Nota de corrección (2026-06-11):** una versión anterior de esta sección daba la base de la paleta como `0x9001_0000`, contradiciendo la tabla de regiones de este mismo documento y el valor real `PALETTE_BASE = 0x9002_0000` del código. El ejemplo, además, escribía `sw 0xFF, 0x9002_0030`, con dos errores: el desplazamiento de la entrada 10 es 30 **decimal** (`0x1E`), no `0x30`, y `sw inmediato, dirección` no es sintaxis RISC-V válida (los stores exigen un registro fuente y direccionamiento base+desplazamiento; no aceptan direcciones absolutas como operando). Se reescribió el ejemplo con código válido para el ensamblador del proyecto.

**Paleta por defecto (FB1):** los primeros 16 colores se inicializan con la paleta CGA estándar (rojo, verde, azul, etc.). Las entradas 16–255 se inicializan como rampa lineal de grises.

## Registro de Control (0x9003_0000)

**Formato:** palabra de 32 bits (lw/sw).

**Semántica (implementada):**
- Es memoria común mapeada: **no hay hook de escritura**, de modo que escribir en el registro no dispara ninguna acción inmediata.
- El emulador lo lee **pasivamente** al publicar frames (`publicar_frame` en `machine.rs`): un valor no nulo —igual que cualquier píxel no nulo en la VRAM— marca que el programa "usa la pantalla", lo que activa el panel de video en la UI (RB-4, ex RF4). Es la única función del registro hoy.
- La publicación del frame ocurre al final de cada tramo de ejecución (paso, tramo de corrida continua, detención) y desde el hook de syscalls antes de bloquearse esperando entrada; solo se publica si la VRAM o la paleta cambiaron desde la última publicación (flag sucio).
- La UI consume los frames publicados con el comando `read_framebuffer` mediante un timer de ~30 Hz.
- Leer devuelve el último valor escrito (es memoria común); se inicializa en 0 al cargar/reiniciar.

> **Nota de divergencia (2026-06-11):** versiones anteriores de este documento prometían que escribir un valor no nulo "dispara un redibujado (vsync lógico) e inmediatamente entrega el framebuffer actual a la interfaz" y que "leer devuelve 0". La implementación no materializó ese hook de escritura: se eligió la vía pasiva descripta arriba por menor riesgo (decisión registrada en PLAN-FRAMEBUFFER.md, divergencia 2 y riesgo RB-2). Se corrigió esta documentación para reflejar la implementación real.

**Uso típico en un programa gráfico:**

```asm
# Pseudocódigo: dibujar un rectángulo y refrescar
# Llenar píxeles
li t0, 0x9000_0000    # base framebuffer
addi t1, zero, 100    # contador
li t2, 42             # índice de color (amarillo)
loop:
  sb t2, 0(t0)        # escribir píxel
  addi t0, t0, 1      # siguiente píxel
  addi t1, t1, -1
  bne t1, zero, loop

# Marcar uso de pantalla (opcional si ya se escribieron píxeles no nulos):
# el frame se publica solo, al final del tramo de ejecución.
li t0, 0x9003_0000
li t1, 1
sw t1, 0(t0)
```

## Sprites — overlay por hardware (RP31)

Capa de **sprites** compuesta sobre el framebuffer indexado **antes** de
convertirlo a RGBA (en `publicar_frame`, usando `core/sprites.rs`). Permite
"cosas tipo juego": el programa **mueve** un sprite actualizando su `(x, y)` en
la tabla, sin redibujar a mano. Dos regiones MMIO:

### Tabla de sprites (`0x9005_0000`, `SPRITE_TABLE_BASE`)

64 entradas de 8 bytes (little-endian). Entrada `i` en el desplazamiento `8·i`:

| Offset | Campo | Tipo | Significado |
|--------|-------|------|-------------|
| +0 | `x` | i16 | columna de la esquina superior izquierda (con signo: permite recorte por la izquierda) |
| +2 | `y` | i16 | fila de la esquina superior izquierda (con signo: recorte por arriba) |
| +4 | `pattern` | u8 | índice de patrón (0–63) en la memoria de patrones |
| +5 | `flags` | u8 | bit 0 = habilitado; el resto reservado |
| +6 | — | u16 | reservado |

### Memoria de patrones (`0x9006_0000`, `SPRITE_PATTERN_BASE`)

64 patrones de 16×16 píxeles indexados, 256 bytes cada uno. El patrón `p` arranca
en el desplazamiento `256·p`; el píxel `(col, fila)` del patrón está en
`256·p + 16·fila + col`. Cada byte es un índice de paleta, **igual que el
framebuffer**. El **índice 0 es TRANSPARENTE** (color-key): no se pinta, deja ver
el fondo.

### Semántica de composición

- Para cada sprite **habilitado** (flags bit 0 = 1), su patrón se compone sobre
  el framebuffer en `(x, y)`. Los píxeles de índice 0 se saltan (transparencia);
  lo que cae fuera de `320×240` se recorta.
- **Orden de pintado:** sprite 0 al fondo, sprite 63 arriba (los de índice mayor
  tapan a los menores donde se solapan).
- Los sprites cuentan como **uso de pantalla** (RB-4/RF4): un programa que solo
  dibuja sprites sobre negro también enciende el panel de video.
- **Costo cero sin sprites:** si ningún sprite está habilitado, la composición se
  saltea y el framebuffer se convierte tal cual (camino bit-idéntico al de antes
  de RP31; el video queda intacto).

### Uso típico

```asm
# Patrón 0: cuadrado amarillo (color 14) de 16×16 = 256 bytes
li t0, 0x9006_0000     # base de patrones (patrón 0)
li t1, 0
li t2, 256
li t3, 14              # color 14 = amarillo
pat:
  bge t1, t2, fin_pat
  add t4, t0, t1
  sb  t3, 0(t4)
  addi t1, t1, 1
  j pat
fin_pat:

# Sprite 0: patrón 0 en (152, 112), habilitado
li t0, 0x9005_0000     # base de la tabla de sprites (entrada 0)
li t1, 152
sh t1, 0(t0)           # x
li t1, 112
sh t1, 2(t0)           # y
li t1, 0
sb t1, 4(t0)           # patrón = 0
li t1, 1
sb t1, 5(t0)           # flags = habilitado
```

Ver el ejemplo completo en `ejemplos/17-sprites.asm`.

## Ventajas de este diseño

1. **Semántica simple:** la VRAM es una región lineal normal; no hay acceso indirecto ni comandos especiales.
2. **Eficiencia de emulación:** el framebuffer se lee de golpe al publicar cada frame, al final del tramo de ejecución (RB-3, ex RF3); no hay overhead de hook por píxel.
3. **Compatibilidad con v0.1:** programas sin uso de video no pagan ningún costo (las regiones no son visitadas).

## Uso desde EmuRV (UI)

Comando Tauri `read_framebuffer` (implementado en FB1):
- Retorna buffer RGBA de 320×240 como bytes crudos.
- Se escala con `image-rendering: pixelated` en CSS para 1:1 pixel art.
- Volcado a canvas con `putImageData` sobre `ImageData` reusado, no recreado cada frame (RB-1, ex RF1).
- Timer de redibujado en la UI: ~30 Hz (RB-2, ex RF2). No existe vsync explícito disparado por escritura al registro de control (ver la nota de divergencia en la sección de ese registro).

## Rendimiento medido (2026-06-11, build release, Mac Apple Silicon)

Cifras medidas con la suite de benchmarks (`cargo test --release -- --ignored --nocapture`):

| Métrica | Medido | Requisito | Margen |
|---------|--------|-----------|--------|
| Conversión framebuffer→RGBA (`fb1_performance_conversion`) | **58 µs** promedio | ≤ 3 ms | ~52× |
| Frames publicados por segundo, lado Rust, bajo carga (`fb4_fps`: repintado completo de 320×240 por frame, color distinto por pasada, 1000 frames) | **835 FPS** | ≥ 30 FPS (alimentar la UI a 30 Hz) | ~28× |

El bench de FPS mide el techo del lado Rust (emulación del repintado byte a byte ≈ 230 000 instrucciones por frame + detección de cambios + conversión a RGBA + copia al estado compartido). NO incluye el puente Tauri ni el volcado al canvas: la UI consume a ~30 Hz por diseño (RB-2), de modo que el cuello real es el timer de la UI, no el emulador. **El FPS efectivo percibido en pantalla (canvas) requiere medición con la app corriendo y ojo humano: pendiente.**
