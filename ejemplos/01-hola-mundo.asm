# =====================================================
# Ejemplo 1 — Hola mundo
# Imprime un saludo en la consola y termina.
# Syscalls: 4 (imprimir cadena), 10 (terminar)
# =====================================================

        .data
saludo: .asciiz "¡Hola, mundo!\n"   # cadena terminada en NUL

        .text
main:
        la   a0, saludo     # a0 ← dirección de la cadena
        li   a7, 4          # servicio 4: imprimir cadena
        ecall               # llamada al sistema

        li   a7, 10         # servicio 10: terminar el programa
        ecall
