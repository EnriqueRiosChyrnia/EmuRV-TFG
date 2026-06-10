# =====================================================
# Ejemplo 3 — Contador
# Imprime los números del 1 al 10, uno por línea.
# Muestra el patrón básico de bucle con condición.
# =====================================================

        .data
nl:     .asciiz "\n"

        .text
main:
        li   t0, 1          # t0: contador (empieza en 1)
        li   t1, 10         # t1: límite

bucle:
        mv   a0, t0         # imprimir el valor actual
        li   a7, 1
        ecall
        la   a0, nl         # salto de línea
        li   a7, 4
        ecall

        addi t0, t0, 1      # contador + 1
        ble  t0, t1, bucle  # repetir mientras t0 <= 10

        li   a7, 10         # terminar
        ecall
