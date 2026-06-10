# Ejemplo 07: franjas de color en framebuffer (FB2)
# Pinta rectángulos de color en el framebuffer.

.text
  li t0, 0x9000_0000    # base framebuffer

  # Llenar 256 píxeles con color 1 (azul)
  li t1, 0
  li t2, 1
  li t3, 256

loop1:
  bge t1, t3, loop1_done
  add t4, t0, t1
  sb t2, 0(t4)
  addi t1, t1, 1
  j loop1

loop1_done:
  # Llenar píxeles 320-576 con color 2 (verde)
  li t1, 320
  li t2, 2
  li t3, 576

loop2:
  bge t1, t3, loop2_done
  add t4, t0, t1
  sb t2, 0(t4)
  addi t1, t1, 1
  j loop2

loop2_done:
  # Llenar píxeles 640-896 con color 3 (cian)
  li t1, 640
  li t2, 3
  li t3, 896

loop3:
  bge t1, t3, loop3_done
  add t4, t0, t1
  sb t2, 0(t4)
  addi t1, t1, 1
  j loop3

loop3_done:
  # Llenar píxeles 960-1216 con color 4 (rojo)
  li t1, 960
  li t2, 4
  li t3, 1216

loop4:
  bge t1, t3, loop4_done
  add t4, t0, t1
  sb t2, 0(t4)
  addi t1, t1, 1
  j loop4

loop4_done:
  # Señalar frame listo (vsync)
  li t0, 0x9003_0000
  li t1, 1
  sw t1, 0(t0)

  # Terminar
  li a7, 10
  ecall
