# =====================================================
# Ejemplo 5 — Pila y funciones: factorial recursivo
# Calcula 5! llamando a una función recursiva.
# Muestra la convención de llamada RISC-V:
#   - a0 lleva el argumento y trae el resultado
#   - ra (dirección de retorno) se guarda en la pila
#   - sp baja al entrar y sube al salir (marco de pila)
# =====================================================

        .data
msj:    .asciiz "5! = "
nl:     .asciiz "\n"

        .text
main:
        la   a0, msj
        li   a7, 4
        ecall

        li   a0, 5          # argumento: n = 5
        call factorial      # a0 ← 5!

        li   a7, 1          # imprimir el resultado
        ecall
        la   a0, nl
        li   a7, 4
        ecall
        li   a7, 10         # terminar
        ecall

# -----------------------------------------------------
# factorial(n): devuelve n! en a0
# caso base: n < 2 → 1
# caso recursivo: n * factorial(n - 1)
# -----------------------------------------------------
factorial:
        li   t0, 2
        blt  a0, t0, caso_base

        addi sp, sp, -8     # reservar marco de pila
        sw   ra, 4(sp)      # guardar dirección de retorno
        sw   a0, 0(sp)      # guardar n

        addi a0, a0, -1     # argumento: n - 1
        call factorial      # a0 ← (n-1)!

        lw   t1, 0(sp)      # recuperar n
        mul  a0, a0, t1     # a0 ← n · (n-1)!

        lw   ra, 4(sp)      # restaurar ra y liberar el marco
        addi sp, sp, 8
        ret

caso_base:
        li   a0, 1          # 0! = 1! = 1
        ret
