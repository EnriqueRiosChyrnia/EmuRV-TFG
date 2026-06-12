# utiles.s — rutinas auxiliares del ejemplo multi-archivo.
# Se incluye al FINAL de main.s: este archivo SÍ emite código, y al ir
# después de main el punto de entrada (0x00400000) sigue siendo main.

# saludar: imprime el saludo cargando su dirección con %hi/%lo — el par
# lui+addi reconstruye la dirección exacta (es lo que la pseudo «la» hace
# por dentro; acá se muestra a mano con fines didácticos).
saludar:
  lui  a0, %hi(saludo)
  addi a0, a0, %lo(saludo)
  li   a7, SYS_PRINT_STR
  ecall
  ret
