# Ejemplo 08: Tetris gráfico (FB3)
# Versión gráfica del Tetris: tablero 10×20 celdas, cada celda 16×16 píxeles.
# Dibuja un tablero con algunas piezas (L-tetromino) para demostrar renderizado.

.text
  li t0, 0x9000_0000    # base framebuffer

  # Llenar toda la pantalla de negro
  li t1, 0
  li t2, 1000

fill_black:
  bge t1, t2, fill_done
  add t3, t0, t1
  sb zero, 0(t3)
  addi t1, t1, 1
  j fill_black

fill_done:
  # Dibujar rectángulo rojo: fila 100-120, columnas 50-100
  li t1, 100      # fila inicial
  li t2, 120      # fila final
  li t3, 50       # columna inicial
  li t4, 100      # columna final
  li t5, 4        # color rojo
  li s0, 320      # píxeles por fila

rect_rows:
  bge t1, t2, rect_done
  mul s1, t1, s0
  add s1, s1, t0
  li t3, 50

  rect_cols:
    bge t3, t4, next_row
    add s2, s1, t3
    sb t5, 0(s2)
    addi t3, t3, 1
    j rect_cols

  next_row:
    addi t1, t1, 1
    j rect_rows

rect_done:
  # Dibujar rectángulo verde: fila 130-150, columnas 150-200
  li t1, 130      # fila
  li t2, 150      # final
  li t3, 150      # col inicial
  li t4, 200      # col final
  li t5, 2        # color verde
  li s0, 320      # píxeles por fila

rect2_rows:
  bge t1, t2, rect2_done
  mul s1, t1, s0
  add s1, s1, t0
  li t3, 150

  rect2_cols:
    bge t3, t4, next_row2
    add s2, s1, t3
    sb t5, 0(s2)
    addi t3, t3, 1
    j rect2_cols

  next_row2:
    addi t1, t1, 1
    j rect2_rows

rect2_done:
  # Señalar frame listo (vsync)
  li t0, 0x9003_0000
  li t1, 1
  sw t1, 0(t0)

  # Terminar
  li a7, 10
  ecall
