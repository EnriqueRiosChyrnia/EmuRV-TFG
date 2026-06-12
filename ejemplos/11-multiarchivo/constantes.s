# constantes.s — constantes y macros compartidas del ejemplo multi-archivo.
# Se incluye al COMIENZO de main.s: ni las constantes ni la definición de
# la macro emiten código, así que no alteran el punto de entrada.

.eqv VECES 3            # cuántas veces saludar
.eqv SYS_PRINT_STR 4    # syscall: imprimir cadena
.eqv SYS_EXIT 10        # syscall: terminar

# Macro: imprime la cadena en la etiqueta dada. El parámetro %etiqueta se
# sustituye textualmente en cada invocación.
.macro imprimir (%etiqueta)
  la   a0, %etiqueta
  li   a7, SYS_PRINT_STR
  ecall
.end_macro
