# constantes.s — todos los números «con nombre» del
# proyecto, juntos en un solo lugar. «.eqv NOMBRE valor» le
# pone nombre a un número; main.s incluye este archivo
# PRIMERO porque cada constante debe definirse antes de usarse.

# Los datos del problema (probá cambiarlos: ejercicio 1):
.eqv PRECIO_ENTRADA 1500    # lo que cuesta una entrada
.eqv ENTRADAS       3       # cuántas compramos

# Los números de servicio (syscalls) de la lección 07:
.eqv SYS_ENTERO     1       # imprimir el entero de a0
.eqv SYS_CADENA     4       # imprimir la cadena que apunta a0
.eqv SYS_CARACTER   11      # imprimir el carácter de a0
.eqv SYS_SALIR      10      # terminar el programa

# El código del carácter «salto de línea»:
.eqv NUEVA_LINEA    10
# ¿Viste que SYS_SALIR y NUEVA_LINEA valen 10 los dos? Son
# cosas DISTINTAS que comparten número: justo el tipo de
# confusión que un buen nombre evita.
