# EmuRV

Entorno educativo de programación en ensamblador **RISC-V (RV32IM)** para macOS, concebido como reemplazo moderno de emu8086: **editar → ensamblar → ejecutar / paso a paso → observar registros y memoria**, con mensajes de error en español pensados para principiantes.

Prototipo técnico de TFG — FIUNI, Ingeniería en Computación.

![EmuRV](docs/capturas/f4-breakpoint-bucle.png)

## Características

- Editor con resaltado de sintaxis RV32, números de línea y breakpoints por click en el margen.
- Ensamblador propio RV32IM (enteros + multiplicación/división) con errores **por línea y columna, en español**.
- Ejecución completa o paso a paso sobre [Unicorn Engine](https://www.unicorn-engine.org/); la línea por ejecutarse queda resaltada.
- Vista de registros (x0–x31 + pc, nombres ABI, hex y decimal) con **resaltado de los que cambiaron** en el último paso.
- Vista de memoria navegable con accesos rápidos a `.text`, `.data` y la pila.
- Consola integrada con syscalls estilo RARS (`ecall` con selector en `a7`): imprimir/leer enteros, cadenas, caracteres, terminar, tiempo en ms y dormir.
- **Teclado en tiempo real**: dispositivo MMIO compatible RARS/MARS (`0xFFFF_0000`) que el programa consulta por polling sin bloquearse — ver [`docs/TECLADO.md`](docs/TECLADO.md).
- Exportación a **ELF32 RISC-V estándar**, cargable en otras herramientas (p. ej. Ripes).
- 9 ejemplos comentados en español, accesibles desde el menú «Ejemplos» — incluido un **Tetris gráfico**, port a RV32IM de un Tetris 8086 de la materia Microprocesadores, dibujado en la pantalla de 320×240, y su versión **en tiempo real** (teclado MMIO + gravedad por reloj).
- Mapa de memoria compatible con RARS: `.text` en `0x00400000`, `.data` en `0x10010000`, `sp` inicial `0x7FFFFFF0`.

## Modo gráfico (Framebuffer VGA 13h)

**Rama `feature/framebuffer`** — Extensión experimental: pantalla gráfica estilo VGA modo 13h (320×240, paleta indexada de 256 colores), visible en el panel «Pantalla» de la app.

- **Framebuffer:** 320×240 píxeles, 1 byte por píxel (índice de paleta), mapeado en `0x9000_0000`.
- **Paleta:** 256 colores RGB888 en `0x9002_0000` — por defecto, los 16 CGA + rampa de grises; el programa puede reescribirla.
- **Vsync:** escribir 1 en `0x9003_0000` señala "frame listo".
- **Panel en la UI:** aparece solo cuando el programa usa la VRAM (o con el botón «Pantalla»); canvas con `image-rendering: pixelated`, refresco a ~30 Hz con flag sucio y bytes crudos por el puente (sin JSON).
- **Ejemplos:** «Franjas de color» (mínimo) y **«Tetris gráfico»**: el mismo Tetris port del 8086 de v0.1, ahora dibujando el tablero como bloques de color en la pantalla (la lógica de juego —LCG, colisiones, líneas, puntaje— es idéntica y está verificada por el mismo oráculo de tests).
- **Compatibilidad:** los programas sin gráficos no pagan ningún costo (RF4).

Detalles: ver [`docs/MEMORIA-GRAFICA.md`](docs/MEMORIA-GRAFICA.md) y [`BITACORA.md`](BITACORA.md) (decisiones técnicas, mediciones RF1-RF4).

## Teclado en tiempo real (rama `feature/teclado`)

Entrada interactiva sin bloquear el programa: un **dispositivo de teclado mapeado en memoria**, con las mismas direcciones y semántica que RARS/MARS (el material didáctico existente funciona sin cambios).

- **`RECEIVER_CONTROL` (`0xFFFF_0000`):** bit 0 en 1 cuando hay una tecla pendiente (el bit 1 —interrupciones— queda reservado: solo polling).
- **`RECEIVER_DATA` (`0xFFFF_0004`):** código de la tecla más vieja; **leerlo la consume**. Cola de 64 teclas con descarte de la más vieja.
- **Captura en la UI:** click en el canvas del panel «Pantalla» (o ventana de pantalla desacoplada); imprimibles ASCII, Enter (10), Esc (27), Backspace (8) y flechas (`0x80`–`0x83`).
- **Syscalls nuevas:** 30 (tiempo del sistema en ms, par `a1:a0`, reloj no decreciente) y 32 (dormir `a0` ms, interrumpible por «Detener»).
- **«Detener» responde siempre:** la ejecución corre en tramos de 1 M de instrucciones (~2-3 ms), así que hasta un `loop: j loop` se detiene al instante.
- **Ejemplos:** «Eco de teclado» (polling mínimo comentado) y **«Tetris en tiempo real»** (mismo motor del Tetris gráfico; a/d/w/s o flechas, gravedad por reloj).

Detalles completos —mapa MMIO, semántica exacta, tabla de códigos, decisiones de diseño con mediciones—: [`docs/TECLADO.md`](docs/TECLADO.md).

## Instalación

### Desde el bundle

Abrir `EmuRV.app` (o el `.dmg`) generado en `src-tauri/target/release/bundle/`. Requiere macOS en Apple Silicon.

> La app no está firmada/notarizada: la primera vez, click derecho → «Abrir» (o autorizarla en Ajustes del Sistema → Privacidad y seguridad).

### Compilar desde el código fuente

Requisitos comunes: [Rust](https://rustup.rs) (estable), Node.js ≥ 20, CMake y pkg-config (los exige la compilación de Unicorn).

**macOS:**

```bash
brew install cmake pkg-config
```

**Linux (Debian/Ubuntu):** además de cmake/pkg-config, las librerías de sistema de Tauri 2:

```bash
sudo apt update
sudo apt install -y cmake pkg-config build-essential curl wget file \
  libwebkit2gtk-4.1-dev libxdo-dev libssl-dev \
  libayatana-appindicator3-dev librsvg2-dev
```

(En Fedora: `webkit2gtk4.1-devel openssl-devel libappindicator-gtk3-devel librsvg2-devel`; en Arch: `webkit2gtk-4.1 base-devel openssl appmenu-gtk-module libappindicator-gtk3 librsvg`.)

**Compilar y ejecutar (igual en todas las plataformas):**

```bash
npm install
cargo install tauri-cli --locked   # una sola vez
cargo tauri build        # bundle en src-tauri/target/release/bundle/
# o, para desarrollo:
cargo tauri dev
```

En Linux el bundle sale como `.deb`/`.rpm`/AppImage según la distro. Nota: esta versión solo se probó en macOS Apple Silicon; el código es multiplataforma pero el build en Linux no está validado.

## Tu primer programa (5 minutos)

1. Abrí EmuRV. El editor ya trae el ejemplo «Hola mundo»:

```asm
        .data
saludo: .asciiz "¡Hola, mundo!\n"   # cadena terminada en NUL

        .text
main:
        la   a0, saludo     # a0 ← dirección de la cadena
        li   a7, 4          # servicio 4: imprimir cadena
        ecall               # llamada al sistema

        li   a7, 10         # servicio 10: terminar el programa
        ecall
```

2. **Ensamblar** — si hay errores, aparecen subrayados en el editor y listados en la consola, con línea y columna.
3. **Ejecutar** — la salida aparece en la consola integrada. O **Paso** para ir instrucción por instrucción mirando cómo cambian los registros (los modificados quedan resaltados).
4. Click en el margen izquierdo del editor para poner un **breakpoint** (punto rojo): «Ejecutar» se detiene ahí, antes de ejecutar esa línea.
5. **Reiniciar** vuelve la máquina al estado inicial (registros y memoria como recién ensamblado).

Probá después «Suma interactiva» (Ejemplos → escribe en la consola cuando el programa pide entrada), «Fibonacci» paso a paso, «Tetris gráfico» para un programa grande de verdad (tablero en memoria, bitmaps, colisiones, líneas completas, dibujado en la pantalla gráfica), y «Tetris en tiempo real» o «Eco de teclado» para jugar con el teclado MMIO (click en la pantalla gráfica para capturar las teclas).

## Syscalls (convención RARS)

| a7 | Servicio | Argumentos |
|----|----------|------------|
| 1  | imprimir entero | `a0` = valor |
| 4  | imprimir cadena | `a0` = dirección de cadena NUL |
| 5  | leer entero | devuelve en `a0` |
| 10 | terminar | — |
| 11 | imprimir carácter | `a0` = código |
| 30 | tiempo del sistema en ms | devuelve `a0` = bits bajos, `a1` = bits altos |
| 32 | dormir | `a0` = milisegundos (interrumpible con «Detener») |

Tabla completa, mapa de memoria y directivas: botón **Ayuda** dentro de la app.

## Estructura del proyecto

```
src-tauri/src/core/   núcleo Rust: assembler, machine (Unicorn), syscalls, elf_export, video, keyboard
src-tauri/src/commands.rs   comandos Tauri + hilo de emulación
src/                  frontend TS: editor (CodeMirror 6), paneles, i18n
ejemplos/             los 9 programas de ejemplo
docs/                 contrato de trabajo del TFG y capturas
BITACORA.md           registro de decisiones técnicas (material de tesis)
```

```bash
cd src-tauri && cargo test     # suite completa (núcleo + integración de ejemplos)
```

## Licencia / alcance

Prototipo académico (v0.1). Primera iteración: macOS Apple Silicon; la base es multiplataforma (Tauri) pero los builds de Windows/Linux quedan fuera de esta versión.
