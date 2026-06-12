# DEPURACIÓN — watchpoints, condiciones y edición en vivo (RP-7, 2026-06-12)

Herramientas de depuración DIDÁCTICAS integradas en la app (panel «Depuración»),
pensadas para el estudiante: mensajes en español, gramática mínima y rechazos
que explican el porqué. Para depuración avanzada (expresiones compuestas,
scripts), está el stub gdb: `docs/DEPURACION-GDB.md`.

## Watchpoints

Vigilan una zona de DATOS: la ejecución se detiene cuando el programa la lee
y/o la escribe.

- **Dónde:** `.data`, pila, framebuffer y paleta. Tamaños: 1, 2 o 4 bytes.
- **Vetada la página del teclado MMIO** (`0xFFFF0000`): leerla consume la
  tecla — vigilarla cambiaría la semántica del dispositivo. El visor de
  memoria sí puede mirarla (las lecturas del host no consumen ni disparan).
- Al frenar: «Watchpoint: el programa escribió 0x2A en 0x10010004 (línea 7)»
  y el panel de memoria navega a la dirección.
- **Rendimiento:** sin watchpoints no se instala NINGÚN hook (la corrida
  libre no paga nada — mismo patrón que los breakpoints). Con watchpoint
  activo y un bucle con acceso a memoria por iteración: 71.4 M instr/s
  medidos (release) — de sobra para depurar.

## Breakpoints condicionales

En el panel «Depuración», cada breakpoint acepta una condición:

```
REGISTRO  OPERADOR  VALOR
t0        ==        5
a0        !=        0x1F
s1        <         t2
t3        >=        -1
```

- REGISTRO: nombre ABI (`t0`–`t6`, `s0`–`s11`, `a0`–`a7`, `sp`, `ra`…) o `xN`.
- OPERADOR: `==` `!=` `<` `<=` `>` `>=` — comparación **con signo** (i32).
- VALOR: decimal (con signo), hex `0x…` (`0xFFFFFFFF` es −1) o un segundo
  registro. Nada más: sin `&&`, sin aritmética (para eso, gdb).
- La condición se valida al escribirla (el error aparece en el panel); con
  condición falsa el programa pasa de largo sin contar como parada.
- El caso típico: breakpoint dentro de un bucle con `t0 == 5` → frena
  exactamente en la pasada en la que `t0` vale 5.

## Edición en vivo (máquina pausada)

Con la ejecución detenida (Detener, breakpoint, watchpoint o paso):

- **Registros:** doble click sobre el valor (hex o decimal) → escribir el
  nuevo (decimal o `0x…`) → Enter. La siguiente instrucción ejecutada usa el
  valor nuevo. El `pc` también se edita (la ejecución continúa desde ahí).
- **Memoria:** doble click sobre una palabra del hexdump → mismo flujo
  (4 bytes little-endian).
- **Rechazos didácticos:** `x0` («es cero por diseño del ISA»); `.text`
  («el programa es inmutable mientras corre: editá el fuente y
  re-ensamblá»); página del teclado; cualquier zona fuera de `.data`, pila,
  framebuffer o paleta; y todo intento con la máquina corriendo («pausá la
  ejecución para editar»).

## Límites conocidos

- Durante un sleep (`syscall 32`) el hilo de emulación no atiende comandos:
  las ediciones se encolan hasta que despierte (limitación TEC6 vigente).
- Los breakpoints/watchpoints y condiciones se re-aplican automáticamente al
  re-ensamblar; la edición en vivo, en cambio, muere con la sesión (es
  estado de la máquina, no del programa).
- Condiciones compuestas, watchpoints condicionales y edición de `.text`:
  fuera de alcance por diseño — gdb (RP-6) cubre los dos primeros.

## Modo binario (RP11)

Con un ELF importado («Abrir» → binario) no hay fuente: el editor muestra el
listado desensamblado y **cada línea del listado es una dirección**. Todo lo
de este documento funciona igual: breakpoints (también condicionales) se
ponen clickeando el margen sobre una línea del listado, el resaltado de
ejecución avanza por el listado, y los watchpoints/edición en vivo no
distinguen binario de fuente. Lo único deshabilitado es «Ensamblar» (no hay
qué ensamblar).
