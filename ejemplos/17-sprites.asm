# Ejemplo 17: sprites — overlay por hardware sobre el framebuffer (RP31)
# Dos regiones MMIO nuevas:
#   tabla de sprites    0x9005_0000  (64 entradas x 8 bytes:
#                        x:i16 +0, y:i16 +2, patron:u8 +4, flags:u8 +5)
#   memoria de patrones 0x9006_0000  (64 patrones de 16x16 indexados, 256 B c/u)
# El indice 0 de un patron es TRANSPARENTE (color-key). El overlay se compone
# sobre el framebuffer antes de mostrarse: mover el sprite = actualizar su (x,y).
# Este programa pinta un fondo azul, define el patron 0 como un cuadrado
# amarillo de 16x16 y coloca un sprite en el centro de la pantalla.

.text
  # --- Fondo azul (color 1) en todo el framebuffer ---
  li t0, 0x9000_0000     # base framebuffer
  li t1, 0               # indice de pixel
  li t2, 76800           # 320 x 240
  li t3, 1               # color 1 = azul
bg_loop:
  bge t1, t2, bg_done
  add t4, t0, t1
  sb t3, 0(t4)
  addi t1, t1, 1
  j bg_loop
bg_done:

  # --- Patron 0: cuadrado amarillo (color 14) de 16x16 = 256 bytes ---
  li t0, 0x9006_0000     # base de patrones (patron 0)
  li t1, 0
  li t2, 256             # 16 x 16
  li t3, 14              # color 14 = amarillo
pat_loop:
  bge t1, t2, pat_done
  add t4, t0, t1
  sb t3, 0(t4)
  addi t1, t1, 1
  j pat_loop
pat_done:

  # --- Sprite 0: patron 0 en el centro (152, 112) ---
  li t0, 0x9005_0000     # base de la tabla de sprites (entrada 0)
  li t1, 152
  sh t1, 0(t0)           # x = 152
  li t1, 112
  sh t1, 2(t0)           # y = 112
  li t1, 0
  sb t1, 4(t0)           # patron = 0
  li t1, 1
  sb t1, 5(t0)           # flags = habilitado (bit 0)

  # Señalar frame listo (vsync) y terminar
  li t0, 0x9003_0000
  li t1, 1
  sw t1, 0(t0)
  li a7, 10
  ecall
