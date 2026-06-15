# Ejemplo 16: sonido — acorde por el dispositivo de tono (MMIO, RP31)
# El dispositivo de sonido vive en 0x9004_0000. Registros (u32):
#   +0  frecuencia (Hz)
#   +4  duracion (ms)
#   +8  forma de onda: 0=cuadrada 1=senoidal 2=triangulo 3=sierra
#   +12 disparo: escribir cualquier valor lanza un tono con los valores actuales
# Apagado por defecto: solo suena si el programa dispara (como la pantalla).
# Este programa dispara tres tonos (do-mi-sol): al reproducirse juntos forman
# un acorde de Do mayor.

.text
  li t0, 0x9004_0000     # base del dispositivo de sonido

  li t1, 1
  sw t1, 8(t0)           # forma de onda = senoidal (suave)
  li t1, 600
  sw t1, 4(t0)           # duracion = 600 ms

  li t1, 262             # Do (C4)
  sw t1, 0(t0)
  li t1, 1
  sw t1, 12(t0)          # disparo

  li t1, 330             # Mi (E4)
  sw t1, 0(t0)
  li t1, 1
  sw t1, 12(t0)          # disparo

  li t1, 392             # Sol (G4)
  sw t1, 0(t0)
  li t1, 1
  sw t1, 12(t0)          # disparo

  li a7, 10
  ecall
