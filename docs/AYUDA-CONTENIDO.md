# Contenido del panel de Ayuda de EmuRV (para diseño)

Texto REAL que muestra el modal de Ayuda (`src/paneles/ayuda.ts`, resuelto desde
`src/i18n/es.json`). Es un overlay modal (`#ayuda-overlay` → `.ayuda-caja` con
`header` + `.ayuda-cuerpo`). Estructura: cabecera con título y botón cerrar (✕),
y un cuerpo scrolleable con secciones `<h3>` + párrafos + tablas + bloques `<pre>`
de código. Para diseñarlo: es contenido DENSO y de referencia — el reto es
legibilidad y jerarquía, no decoración. Público principiante.

## Cabecera
**Ayuda de EmuRV**  · botón **✕** (Cerrar)

## Ciclo de trabajo
Escribí tu programa, presioná «Ensamblar» y después «Ejecutar» (corre hasta el
final) o «Paso» (una instrucción por vez). «Reiniciar» vuelve la máquina al
estado inicial sin re-ensamblar. Los paneles de la derecha muestran los registros
(los que cambiaron quedan resaltados) y la memoria.

¿Arrancás de cero? Apretá el botón «Lecciones» de la barra: un navegador con el
curso de 10 lecciones ordenadas. Al elegir una ves sus objetivos y qué paneles
mirar antes de abrirla; «Abrir lección» carga el código al editor para recorrerlo
con «Paso». El navegador marca tu progreso y te lleva a la siguiente. La lección 8
usa varios archivos: al abrirla te pide una carpeta donde guardarlos (lo necesita
el .include).

«Abrir» también acepta ejecutables ELF32 RISC-V: el editor muestra el listado
desensamblado (solo lectura) y Ejecutar/Paso/breakpoints funcionan sobre el
listado. Para compilar con gcc: `-march=rv32im -mabi=ilp32 -Ttext=0x00400000
-Tdata=0x10010000 -nostdlib`.

## Idioma
EmuRV está en español o inglés: usá el selector ES/EN de la barra (tu elección se
recuerda). La traducción cubre la interfaz; los mensajes de error del ensamblador
y las lecciones quedan en español incluso en modo inglés — son el material
didáctico del proyecto.

## Temas
Con el selector de tema de la barra elegís el aspecto: «Educativo» (oscuro sobrio)
o «Retro-CRT» (fósforo ámbar con líneas de barrido, como un monitor viejo). El
cambio se ve al instante y se recuerda. Ambos temas mantienen buen contraste para
leer el código con comodidad.

## Syscalls (convención RARS)
Cargá el número de servicio en a7, los argumentos en a0, y ejecutá «ecall»:

| a7 | Servicio | Argumentos |
|----|----------|------------|
| 1  | imprimir entero | a0 = valor |
| 2  | imprimir float (single) | fa0 = valor (f32) |
| 3  | imprimir double | fa0 = valor (f64) |
| 4  | imprimir cadena | a0 = dirección (terminada en NUL) |
| 5  | leer entero (la consola espera tu entrada) | → a0 = valor leído |
| 6  | leer float (single) | → fa0 = valor (f32) |
| 7  | leer double | → fa0 = valor (f64) |
| 8  | leer cadena: guarda a lo sumo a1−1 caracteres + byte 0 final (trunca sin error) | a0 = buffer, a1 = tamaño |
| 9  | reservar memoria del heap (sbrk; arranca en 0x10040000, sin free) | a0 = bytes → a0 = dirección |
| 10 | terminar el programa | — |
| 11 | imprimir carácter | a0 = código de carácter |
| 30 | tiempo del sistema en milisegundos (época Unix) | → a0 = ms (bits bajos), a1 = ms (bits altos) |
| 32 | dormir: pausa el programa a0 milisegundos | a0 = milisegundos |
| 60 | limpiar la consola (extensión de EmuRV — no existe en RARS) | — |
| 93 | terminar con código de salida (el CLI lo propaga al shell, acotado a 0..125) | a0 = código de salida |

Bloque de código de ejemplo (estilo `<pre class="ayuda-ejemplo">`):
```
    li   a7, 4        # servicio: imprimir cadena
    la   a0, mensaje  # argumento
    ecall             # llamada al sistema
```

Además, la instrucción «ebreak» pausa el programa (pausa por software, como un
breakpoint puesto desde el código): podés inspeccionar registros y memoria, y
reanudar con «Ejecutar» o «Paso» desde la instrucción siguiente.

Avanzado: con «csrw mtvec, reg» el programa instala su propio manejador de
excepciones — los ecall desconocidos, las instrucciones ilegales y los accesos
inválidos saltan ahí (mcause dice qué pasó) y «mret» vuelve.

Coma flotante (RV32F/D): registros f0–f31 con nombres ABI (ft, fs, fa); aritmética
.s (single) y .d (double), conversiones, comparaciones y fma. Los valores se ven
en el panel plegable «Registros F (flotante)».

Extensiones avanzadas: A (atómicas — amoadd.w, lr.w/sc.w; mono-hart, sc siempre
tiene éxito: para aprender el patrón, no concurrencia real) y C (comprimidas —
c.addi, c.li, c.mv, c.lw… de 2 bytes; se escriben a mano).

## Mapa de memoria
| Dirección | Qué |
|-----------|-----|
| `0x00400000` | código (.text) — acá empieza la ejecución |
| `0x10010000` | datos (.data) |
| `0x7FFFFFF0` | tope de la pila (sp inicial; crece hacia abajo) |

## Teclado y tiempo
El teclado es un dispositivo mapeado en memoria (MMIO), como en RARS/MARS: el
programa lo consulta por encuesta (polling) leyendo dos palabras. Para tipear:
clic en la pantalla del panel (queda capturando hasta que hagas clic afuera) o
cualquier tecla con la ventana de pantalla aparte al frente.

| Dirección | Qué |
|-----------|-----|
| `0xFFFF0000` | RECEIVER_CONTROL — bit 0 en 1 si hay una tecla pendiente (bit 1 reservado) |
| `0xFFFF0004` | RECEIVER_DATA — código de la tecla más vieja; leerlo la consume de la cola |

Códigos que entrega RECEIVER_DATA:

| Código | Tecla |
|--------|-------|
| `32–126` | caracteres imprimibles ASCII (letras, dígitos, espacio…) |
| `10` | Enter |
| `27` | Escape |
| `8` | Backspace |
| `0x80–0x83` | flechas: ← 0x80, → 0x81, ↑ 0x82, ↓ 0x83 |

Bloque de ejemplo:
```
espera:
    li   t0, 0xFFFF0000   # RECEIVER_CONTROL del teclado
    lw   t1, 0(t0)        # bit 0 = hay tecla pendiente
    beqz t1, espera       # nada todavía: seguir esperando
    lw   a0, 4(t0)        # RECEIVER_DATA: leer consume la tecla
```

## Directivas soportadas
`.text .data .word .half .byte .ascii .asciiz .string .space .align .globl
.include .eqv .equ .macro .end_macro`

`.include "archivo.s"` inserta otro archivo (el programa debe estar guardado en
disco); `.eqv NOMBRE valor` define constantes; `.macro nombre (%p)…/.end_macro`
define macros; `%hi(x)`/`%lo(x)` parten una dirección para `lui`+`addi`.

## Ejemplos
El menú «Ejemplos» carga programas comentados: hola mundo, suma interactiva,
contador, fibonacci, pila/funciones (factorial recursivo), franjas de color y
Tetris gráfico, eco de teclado y Tetris en tiempo real (teclado MMIO), manejador
de excepciones (CSRs/traps) y coma flotante (promedio de notas con F/D).

## Editor
El editor te ayuda mientras escribís: autocompletado de mnemónicos, registros y
etiquetas (aparece solo al tipear, o con Ctrl-Espacio); al pasar el mouse sobre
una instrucción se muestra su sintaxis y qué hace; Cmd/Ctrl-click sobre una
etiqueta salta a su definición, y al poner el cursor sobre una etiqueta se
resaltan todos sus usos. Los errores se siguen viendo al «Ensamblar».

## Breakpoints
Click en el margen izquierdo del editor (a la izquierda del número de línea) para
poner o sacar un punto rojo. «Ejecutar» se detiene al llegar a esa línea, antes de
ejecutarla; la línea queda resaltada y podés inspeccionar registros y memoria.

En el panel «Depuración» podés sumarle una condición a cada breakpoint (ej.:
«t0 == 5») y vigilar zonas de datos con watchpoints. Con la máquina pausada, doble
clic sobre un registro o una palabra de memoria los edita en vivo.
