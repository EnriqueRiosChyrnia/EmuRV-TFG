# ==========================================================
# Lección 01 — Registros y movimiento de datos
# ==========================================================
# OBJETIVOS — al terminar esta lección vas a poder:
#   * explicar qué es un registro y para qué sirve
#   * cargar valores con li, copiarlos con mv y cargar
#     direcciones con la
#   * reconocer los nombres ABI (t0, a0, sp…) en el panel
#     de Registros
#   * imprimir un entero por la Consola
# PANELES PARA OBSERVAR: «Registros» (mirá la columna
#   Hexadecimal y Decimal; cada vez que des un Paso, el
#   registro que cambió queda RESALTADO) y «Consola».
# CÓMO USARLA: presioná «Ensamblar» y después «Paso» varias
#   veces, leyendo los comentarios. Cada bloque te dice qué
#   mirar. («Ejecutar» corre todo de una; para aprender,
#   mejor de a un paso.)
# ----------------------------------------------------------

        .text
main:
# Un registro es una cajita de 32 bits DENTRO del procesador.
# Hay 32, y cada uno tiene dos nombres: el técnico (x5) y el
# de la convención ABI (t0 = «temporal 0»). El panel de
# Registros muestra los dos.

# li = "load immediate": carga un número fijo en un registro.
# Dale UN Paso y mirá cómo t0 se resalta y pasa a valer 7.
        li   t0, 7

# Otro Paso: ahora t1 vale 40. Fijate que t0 NO cambió:
# cada registro guarda lo suyo hasta que alguien lo pise.
        li   t1, 40

# mv = "move" (en realidad COPIA): t2 queda igual a t0.
# Después de este Paso, t0 y t2 valen 7 los dos.
        mv   t2, t0

# Los registros aN son los "argumentos": las llamadas al
# sistema (syscalls) leen de ahí. Para imprimir un entero:
#   a0 = el valor    a7 = 1 (el número del servicio "imprimir
#   entero")         ecall = "ejecutá el servicio"
        mv   a0, t1          # a0 ← 40
        li   a7, 1
        ecall                # mirá la Consola: aparece 40

# Imprimir un salto de línea: servicio 11 (imprimir carácter)
# con el código 10 (el carácter de "nueva línea").
        li   a0, 10
        li   a7, 11
        ecall

# la = "load address": carga la DIRECCIÓN de una etiqueta,
# no su contenido. Mirá t3 en el panel: vale 0x10010000 — esa
# es la dirección donde vive «saludo» en la memoria (.data).
        la   t3, saludo

# Para imprimir una cadena: a0 = su dirección, servicio 4.
        mv   a0, t3
        li   a7, 4
        ecall

# x0 (alias «zero») SIEMPRE vale 0: escribirlo no hace nada.
# Probá con Paso: t4 queda en 0 aunque "copiamos" desde zero.
        mv   t4, zero

# Terminar el programa: servicio 10. SIEMPRE cerrá tus
# programas así (si no, el procesador sigue de largo hacia
# memoria vacía y EmuRV te lo va a marcar como error).
        li   a7, 10
        ecall

        .data
saludo: .asciiz "registros listos\n"

# ----------------------------------------------------------
# EJERCICIOS (modificá el código y volvé a ensamblar):
#   1. Cambiá el 40 por tu edad y verificá que la Consola la
#      imprima.
#   2. Agregá un registro más (t5) con el valor 1000 y
#      imprimilo después del 40 (necesitás copiarlo a a0 y
#      repetir el ecall con a7 = 1).
#   3. (desafío) Imprimí t0 + 0 usando SOLO mv y los ecall
#      que ya conocés, sin escribir li t0, 7 de nuevo.
#      Pista: ¿qué registro ya tiene el 7 guardado?
# ==========================================================
