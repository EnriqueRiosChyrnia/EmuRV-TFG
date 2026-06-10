# MEMORIA — Mapa de memoria gráfica (VGA modo 13h)

Extensión del mapa de memoria RARS estándar con soporte para pantalla gráfica de 320×240 píxeles.

## Regiones mapeadas

| Región | Base | Tamaño física | Tamaño mapeado | Propósito | Acceso |
|--------|------|---|---|-----------|--------|
| `.text` | `0x0040_0000` | — | 4 MB | Código del programa | R/X |
| `.data` | `0x1001_0000` | — | 4 MB | Datos estáticos | R/W |
| Pila | `0x7FFF_FFF0` (sp inicial) | — | 8 MB | Stack | R/W |
| **Framebuffer** | **`0x9000_0000`** | **76 800 B** | **128 KB** | Píxeles 320×240 | R/W |
| **Paleta** | **`0x9002_0000`** | **768 B** | **4 KB** | 256 colores RGB888 | R/W |
| **Control** | **`0x9003_0000`** | **4 B** | **4 KB** | Vsync/refresh | R/W |

## Framebuffer (0x9000_0000 — 0x9001_FFFF)

**Formato:** array lineal de 76,800 bytes (320 columnas × 240 filas × 1 byte/píxel).

Indexación:
- Byte en desplazamiento `d` corresponde al píxel en posición `(x, y)` donde `x = d mod 320`, `y = d / 320`.
- Rango válido: `[0, 76800)`.

**Semántica:** cada byte es un índice de 0–255 en la tabla de paleta. Escribir un byte actualiza ese píxel inmediatamente en memoria; el redibujado en pantalla se dispara según RF2 (timer de vsync o escritura al registro de control).

## Paleta (0x9002_0000 — 0x9002_02FF)

**Formato:** 256 entradas × 3 bytes RGB (8 bits cada canal), base 0x9001_0000.

Indexación:
- Entrada `i` (0–255) se almacena en desplazamientos `[3i, 3i+1, 3i+2]` desde la base.
  - Byte en `0x9001_0000 + 3i`: componente R (bits 7–0).
  - Byte en `0x9001_0000 + 3i + 1`: componente G.
  - Byte en `0x9001_0000 + 3i + 2`: componente B.

**Ejemplo:** color rojo puro (R=255, G=0, B=0) en entrada 10 se escribe como:
- `sw 0xFF, 0x9002_0030` (R=255, desplazamiento 3×10=30)
- `sw 0x00, 0x9002_0031` (G=0)
- `sw 0x00, 0x9002_0032` (B=0)

O más compactamente, escribiendo un word (4 bytes) alineado al píxel: `sw 0x000000FF, 0x9002_0030` (si la alineación lo permite; recomendado: bytes individuales).

**Paleta por defecto (FB1):** los primeros 16 colores se inicializan con la paleta CGA estándar (rojo, verde, azul, etc.). Las entradas 16–255 se inicializan como rampa lineal de grises.

## Registro de Control (0x9003_0000)

**Formato:** palabra de 32 bits (lw/sw).

**Semántica:**
- Escribir cualquier valor no nulo dispara un redibujado (vsync lógico) e inmediatamente entrega el framebuffer actual a la interfaz de usuario para que pinte el canvas.
- Leer devuelve 0 (sin información de estado reservada por ahora).
- Escribir 0 no tiene efecto.

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

# Señalar "frame listo" → redibujado
li t0, 0x9003_0000
li t1, 1
sw t1, 0(t0)
```

## Ventajas de este diseño

1. **Semántica simple:** la VRAM es una región lineal normal; no hay acceso indirecto ni comandos especiales.
2. **Eficiencia de emulación:** el framebuffer se lee de golpe al vsync (RF3); no hay overhead de hook por píxel.
3. **Compatibilidad con v0.1:** programas sin uso de video no pagan ningún costo (las regiones no son visitadas).

## Uso desde EmuRV (UI)

Comando Tauri `read_framebuffer` (implementado en FB1):
- Retorna buffer RGBA de 320×240 como bytes crudos.
- Se escala con `image-rendering: pixelated` en CSS para 1:1 pixel art.
- Volcado a canvas con `putImageData` sobre `ImageData` reusado, no recreado cada frame (RF1).
- Timer de redibujado: ~30 Hz o por vsync explícito (RF2).
