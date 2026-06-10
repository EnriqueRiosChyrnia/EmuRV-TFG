# =====================================================
# Ejemplo 4 — Fibonacci
# Imprime los primeros 10 términos de la sucesión:
# 0 1 1 2 3 5 8 13 21 34
# Ideal para ver, paso a paso, cómo cambian s0 y s1.
# =====================================================

        .data
sep:    .asciiz " "
nl:     .asciiz "\n"

        .text
main:
        li   s0, 0          # s0 ← F(0)
        li   s1, 1          # s1 ← F(1)
        li   s2, 10         # s2: términos que faltan imprimir

bucle:
        mv   a0, s0         # imprimir el término actual
        li   a7, 1
        ecall
        la   a0, sep        # espacio separador
        li   a7, 4
        ecall

        add  t0, s0, s1     # t0 ← siguiente término
        mv   s0, s1         # avanzar la ventana (s0, s1)
        mv   s1, t0

        addi s2, s2, -1     # un término menos
        bgtz s2, bucle      # repetir mientras falten

        la   a0, nl
        li   a7, 4
        ecall
        li   a7, 10         # terminar
        ecall
