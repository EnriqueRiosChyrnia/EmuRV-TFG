# =====================================================
# Ejemplo 2 — Suma con entrada de usuario
# Pide dos números enteros, los suma y muestra el resultado.
# Syscalls: 4 (imprimir cadena), 5 (leer entero),
#           1 (imprimir entero), 10 (terminar)
# =====================================================

        .data
msj1:   .asciiz "Ingresá el primer número: "
msj2:   .asciiz "Ingresá el segundo número: "
msjr:   .asciiz "La suma es: "
nl:     .asciiz "\n"

        .text
main:
        la   a0, msj1       # pedir el primer número
        li   a7, 4
        ecall
        li   a7, 5          # leer entero → a0
        ecall
        mv   s0, a0         # s0 ← primer número

        la   a0, msj2       # pedir el segundo número
        li   a7, 4
        ecall
        li   a7, 5
        ecall
        mv   s1, a0         # s1 ← segundo número

        la   a0, msjr       # mostrar el resultado
        li   a7, 4
        ecall
        add  a0, s0, s1     # a0 ← s0 + s1
        li   a7, 1          # servicio 1: imprimir entero
        ecall
        la   a0, nl
        li   a7, 4
        ecall

        li   a7, 10         # terminar
        ecall
