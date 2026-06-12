# TECLADO — Dispositivo de teclado MMIO y syscalls de tiempo

> Documento de la fase de teclado (rama `feature/teclado`, 2026-06-12). Fuentes de verdad: `src-tauri/src/core/mod.rs` (constantes), `core/keyboard.rs` (cola), `core/machine.rs` (hook y mapeo), `core/syscalls.rs` (syscalls 30 y 32), `src-tauri/src/commands.rs` (comando `push_key`, tramos de ejecución), `src/teclado.ts` (mapeo de teclas en el frontend) y las entradas TEC0–TEC5 de `BITACORA.md` (decisiones y cifras).

EmuRV incorpora entrada de teclado **en tiempo real**: el programa emulado ya no depende únicamente de la syscall 5 (que bloquea hasta que el usuario tipea una línea), sino que puede consultar un **dispositivo de teclado mapeado en memoria (MMIO)** sin detenerse, al estilo de los simuladores RARS y MARS. Junto con las syscalls de tiempo (30) y de dormir (32), esto habilita programas interactivos genuinos: juegos con gravedad por reloj, animaciones y bucles de polling "educados".

## 1. Mapa MMIO

| Registro | Dirección | Semántica |
|---|---|---|
| `RECEIVER_CONTROL` | `0xFFFF_0000` | **Bit 0**: vale 1 cuando hay al menos una tecla pendiente en `RECEIVER_DATA`. **Bit 1** (habilitar interrupciones de teclado, semántica de MARS): **RESERVADO — no implementado**; el programa debe consultar por encuesta (*polling*). Los demás bits leen 0. |
| `RECEIVER_DATA` | `0xFFFF_0004` | Código de la tecla **más vieja** pendiente. **Leerlo CONSUME la tecla**: la saca de la cola del dispositivo; si la cola queda vacía, la siguiente lectura de `RECEIVER_CONTROL` devuelve 0 y la de `RECEIVER_DATA` devuelve 0. |

La página completa mide 4 KB (`KEYBOARD_BASE = 0xFFFF_0000`, `KEYBOARD_SIZE = 0x1000`, el mínimo mapeable de Unicorn); solo los primeros 8 bytes tienen semántica de dispositivo, el resto es memoria común.

**Por qué estas direcciones.** Son **las mismas del receptor de teclado de RARS/MARS**, a propósito. La compatibilidad es una decisión pedagógica de primer orden en todo el proyecto (el mapa de memoria entero es estilo RARS): el material didáctico existente en español para esos simuladores —apuntes, guías de MMIO, ejercicios de polling— funciona en EmuRV **sin cambiar una sola dirección**. La decisión está registrada en BITÁCORA TEC2 y en los comentarios de `core/mod.rs`.

## 2. Semántica exacta de lectura

El mecanismo (elegido en el spike TEC0, ver §8) es un **hook `MEM_READ` permanente** sobre los 8 bytes `0xFFFF_0000`–`0xFFFF_0007`, instalado al crear la máquina (`machine.rs`). En **cada lectura del programa emulado** sobre ese rango, el hook escribe el valor fresco en la memoria de respaldo **antes** de que la lectura del guest se complete:

- Lectura de `RECEIVER_CONTROL`: el hook refresca `CONTROL` = estado actual de la cola (1 si hay dato, 0 si no). **No consume nada**: leer CONTROL mil veces es inocuo.
- Lectura de `RECEIVER_DATA`: el hook saca la tecla más vieja de la cola (`pop`; si está vacía escribe 0), deposita el código en `DATA` como palabra de 32 bits *little-endian*, y además refresca `CONTROL` para que refleje el estado posterior al consumo.

**`lb` vs `lw`.** El hook dispara con cualquier ancho de acceso (el spike midió ambos; tests `tec2_teclado.rs`). Un `lb` que cae desalineado dentro del registro se normaliza a la palabra base con `& !0x3`, de modo que `lb` sobre cualquiera de los 4 bytes de `RECEIVER_DATA` también consume la tecla. Salvedad de RV32 a tener en cuenta: `lb` **extiende el signo**, así que los códigos de flecha (`0x80`–`0x83`) leídos con `lb` llegan como negativos; conviene `lw` (como en los ejemplos 09 y 10) o `lbu`.

**El visor de memoria de la UI NO consume teclas.** La página del teclado está mapeada con **respaldo real de memoria** (no con `mmio_map` de Unicorn): las lecturas del lado host —el comando `read_mem` que alimenta el visor— leen los bytes directamente **sin disparar hooks**. Inspeccionar `0xFFFF_0000` desde la interfaz, por lo tanto, no altera el estado del dispositivo; en un emulador didáctico, donde mirar memoria es parte del flujo normal de trabajo, esto es decisivo (fue la razón del descarte de `mmio_map`; spike B6, ver §8). Matiz de precisión: el visor muestra el último valor que el hook dejó escrito en el respaldo (que se refresca en cada lectura *del programa*), no necesariamente el estado instantáneo de la cola — por ejemplo, puede mostrarse `CONTROL = 0` con teclas ya encoladas, hasta que el programa vuelva a leer.

## 3. Cola del dispositivo

La cola (`KeyboardShared` en `core/keyboard.rs`) es un `VecDeque<u8>` con **capacidad 64**, compartido vía `Arc<Mutex<…>>` entre la UI y el hilo de emulación (espejo del patrón del estado de video):

- La UI **empuja** códigos con el comando Tauri `push_key(code)` (`commands.rs`), que escribe directo en la cola **sin pasar por el hilo de emulación** (que puede estar corriendo un tramo o dormido en la syscall 32).
- El hook `MEM_READ` **consume** cuando el programa lee `RECEIVER_DATA`.
- **Cola llena**: se descarta la tecla **MÁS VIEJA** para hacer lugar a la nueva, sin error ni pánico (política de RARS para entrada no consumida: lo último tipeado es lo que importa).
- **Reiniciar / sesión nueva** (`load()`/`reset()` en `machine.rs`): la cola se vacía y `CONTROL`/`DATA` vuelven a 0 — sin "teclas fantasma" de la corrida anterior.

En la interfaz, la captura de teclado pertenece a la pantalla gráfica: en el panel embebido arranca **solo con un click explícito en el canvas** (jamás le roba el foco al editor; un indicador avisa «Teclado capturado») y se suelta al perder el foco; en la ventana de pantalla desacoplada toda la ventana captura siempre (`src/paneles/video.ts`, `src/pantalla.ts`, fase TEC4).

## 4. Tabla de códigos

El mapeo navegador→dispositivo vive en un único módulo, `src/teclado.ts`, compartido por el panel embebido y la ventana desacoplada:

| Tecla | Código recibido por `RECEIVER_DATA` |
|---|---|
| Imprimibles ASCII (letras, dígitos, espacio, signos) | su propio byte (en la práctica 32–126; se acepta todo carácter de un punto de código < 128 — «ñ», «á», etc. quedan afuera) |
| Enter | 10 (LF, igual que la consola de RARS) |
| Esc | 27 |
| Backspace | 8 |
| Flecha izquierda | `0x80` |
| Flecha derecha | `0x81` |
| Flecha arriba | `0x82` |
| Flecha abajo | `0x83` |
| Todo lo demás (F1–F12, modificadores solos, etc.) | no se envía nada |

Las flechas usan códigos fuera del rango ASCII para que el programa las distinga de los caracteres imprimibles. Las combinaciones con Ctrl, Cmd o Alt **no se capturan**: son atajos del sistema o de la aplicación y no se le quitan al usuario. Toda tecla mapeada recibe `preventDefault()` (las flechas y el espacio scrollearían la página si llegaran al navegador).

## 5. Syscall 30 — tiempo del sistema en milisegundos

Compatible con RARS: devuelve los milisegundos desde la época Unix como valor de 64 bits repartido en el **par `a1:a0`** (bits bajos en `a0`, bits altos en `a1`; en RV32 un valor de 64 bits viaja en dos registros).

**Diferencia documentada con RARS — reloj monotónico anclado.** RARS implementa este servicio con `System.currentTimeMillis()`, que puede **saltar hacia atrás** (ajuste NTP, cambio manual de hora). EmuRV, en cambio, captura **una sola vez por proceso** (en un `OnceLock` estático, al primer uso) el par *(instante monotónico, ms de época Unix)* y a partir de ahí devuelve `época_del_ancla + transcurrido_monotónico`. El valor es por construcción **no decreciente** dentro de la sesión: medir un delta con dos syscalls 30 (el patrón de la gravedad del ejemplo 10) jamás da negativo. El costo asumido: un salto del reloj de pared posterior al ancla no se refleja, solo desplaza el origen unos milisegundos; y el ancla es compartida entre máquinas del mismo proceso, lo cual para la monotonía es incluso deseable (registrado en BITÁCORA TEC3, [Seguro]).

## 6. Syscall 32 — dormir (interrumpible)

Duerme `a0` milisegundos (sin signo). Dos propiedades de implementación importan (`dormir` en `syscalls.rs`):

1. **Publica el frame de video ANTES de dormir** (mismo patrón que la syscall 5): una animación que duerme entre cuadros muestra cada cuadro durante el sueño, no todos juntos al final.
2. **Es interrumpible**: duerme contra su plazo en trozos de **≤ 10 ms**, chequeando la señal de stop de la sesión entre trozos. Un «Detener» durante un sleep largo corta el sueño con **latencia ≤ 10 ms** y hace `emu_stop()` **sin marcar error**: es la misma semántica de pausa de usuario que cancelar una espera de entrada (verificado en `tec3_tiempo.rs`: un sleep de 10 000 ms detenido a los ~50 ms retorna en < 1 s).

## 7. Latencia de tecla percibida

Cadena completa de una tecla, del dedo a la pantalla:

1. `keydown` → `push_key` deposita el código en la cola **al instante** (no pasa por el hilo de emulación).
2. El programa la ve en su **próxima lectura de `RECEIVER_CONTROL`**; si está corriendo, el tramo de emulación en curso dura ~2–3 ms (§8).
3. El efecto visible (por ejemplo, redibujar el tablero) se publica al final del tramo o antes del siguiente sleep.
4. El canvas repinta con el timer de la UI de **~30 Hz (33 ms)**.

Estimación del lado del emulador: **tramo de emulación (~2–3 ms) + timer de la UI (33 ms) ≈ < 40 ms**, imperceptible para un juego de este tipo. A eso se suma el intervalo de encuesta del propio programa (los ejemplos duermen 10–15 ms entre encuestas), que sigue dejando el total muy por debajo del umbral de molestia. La verificación con la app corriendo es visual (BITÁCORA TEC4 registra qué quedó pendiente de ojo humano).

## 8. Decisiones de diseño (spike TEC0, con cifras)

Antes de tocar `Machine`, la fase abrió con dos pruebas de concepto contra Unicorn crudo (`tests/spike_count.rs` y `tests/spike_mmio.rs`; cifras release, Apple Silicon; BITÁCORA TEC0):

**Interrumpibilidad: count, no timeout.** El timeout de `emu_start` **no corta bucles apretados sin syscalls** (hallazgo previo de la Etapa 4: `Detener` podía no responder ante `loop: j loop`). El tope por cantidad de instrucciones (*count*) sí: `count = 10 M` sobre `j .` retorna en **46,8 ms**, con exactitud del 100 % incluso con la caché JIT caliente. El throughput con slicing se midió en **430,1 M instr/s** (tramos de 10 M) y el overhead de re-entrada a `emu_start` en **~4,2 µs** (con tramos absurdamente chicos de count = 1000 todavía rinde 237,6 M instr/s). De ahí el valor de producción `RUN_SLICE_COUNT = 1_000_000` (~2–3 ms por tramo, `commands.rs`): la señal de stop y las órdenes auxiliares se atienden en **cada** tramo, así que la latencia de «Detener» es de **~2–3 ms** en cualquier programa. RNF-03 re-medido con slicing: **302,5–397,0 M instr/s** (gate de 10 M superado ~30-40×). El refresco de registros a la UI quedó desacoplado a ~100 ms (emitir por tramo significaría ~500 eventos/s).

**Mecanismo MMIO: hook `MEM_READ`, no `mmio_map`, no sincronización por tramo.** Tres alternativas evaluadas:

| Alternativa | Veredicto |
|---|---|
| **Hook `MEM_READ` + página con respaldo real** (elegida) | Semántica leer-consume **exacta** verificada en registros del guest (`CONTROL=1` → leer `DATA` consume → `CONTROL=0`), con `lb` y `lw`, con JIT caliente. Costo con la página no accedida: **nulo** (relación 1,004: 2,668 vs 2,680 G instr/s en bucle aritmético). Las lecturas de host no disparan hooks → el visor de memoria no consume teclas. |
| `mmio_map` (el callback devuelve el valor) | Verificada verde también, pero **toda** lectura pasa por el callback, incluidas las del host: inspeccionar memoria desde la UI consumiría el dato del teclado. Descartada por eso (spike B6). |
| Sincronización por tramo (sin hook: volcar el estado del teclado a la página entre tramos) | No puede dar la semántica leer-consume exacta dentro de un tramo (dos lecturas de `DATA` en el mismo tramo verían la misma tecla) y exigiría un protocolo de acuse adicional. Descartada en favor del hook, cuyo costo medido es nulo (alternativa planteada en `PLAN-TECLADO.md`; decisión en BITÁCORA TEC0, [Seguro]). |

## 9. Ejemplo mínimo de polling

```asm
# Esperar una tecla y dejar su código en a0.
poll:
        li   t0, 0xFFFF_0000    # base del teclado MMIO
        lw   t1, 0(t0)          # RECEIVER_CONTROL: bit 0 = ¿hay tecla?
        beqz t1, espera         # no hay: dormir un poco y volver a preguntar
        lw   a0, 4(t0)          # RECEIVER_DATA: ¡esta lectura CONSUME la tecla!
        j    listo
espera:
        li   a0, 10             # dormir 10 ms entre encuestas: polling
        li   a7, 32             # "educado", no quema CPU en espera activa
        ecall
        j    poll
listo:
        # ... a0 tiene el código (tabla de §4) ...
```

Dos reglas que el ejemplo encarna: **leer `DATA` una sola vez** y guardar el código (cada lectura consume), y **preguntar antes de dormir** (si hay teclas encoladas se atienden de corrido, sin pausas artificiales). Las versiones completas y comentadas línea por línea son `ejemplos/09-eco-teclado.asm` (eco con bloques de color) y `ejemplos/10-tetris-tiempo-real.asm` (polling no bloqueante + gravedad por syscall 30).

## 10. Limitaciones

- **Sin interrupciones de hardware: solo polling.** El bit 1 de `RECEIVER_CONTROL` (habilitar interrupciones, semántica de MARS) está reservado y documentado como no implementado; no existe mecanismo de traps de dispositivo hacia el programa. Todo programa interactivo debe encuestar.
- **Un solo dispositivo, y solo receptor.** RARS/MARS ofrecen además un transmisor MMIO (display en `0xFFFF_0008`/`0xFFFF_000C`); EmuRV no lo implementa — la salida va por syscalls o por la pantalla gráfica.
- **Códigos de un byte, solo pulsaciones.** No hay eventos de soltado de tecla (*key-up*), ni estado de modificadores, ni autorepetición controlable: llega lo que el sistema operativo repita.
- **Repertorio acotado**: ASCII < 128 más las cuatro flechas. Caracteres fuera de ASCII («ñ», acentos) y combinaciones con Ctrl/Cmd/Alt no llegan al programa.
- **Cola acotada a 64** con descarte del más viejo: un programa que no consume pierde lo tipeado hace más tiempo (comportamiento deliberado, §3).
